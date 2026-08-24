//! Gate D acceptance coverage for multi-account catalogs.
//!
//! Reuses production seams (publication gate, identity resolution, Task.model
//! eligibility, management service, TOML edit). All gate-env mutations use
//! [`crate::provider_registry::with_multi_account_rollout_env`] (panic-safe
//! restore + shared lock).

#[cfg(test)]
mod tests {
    use crate::agent::config::{ModelEntry, ModelInfo};
    use crate::agent::execution_backend::ExecutionBackend;
    use crate::agent::model_identity::{
        CatalogEntryOrigin, CatalogOrigins, ModelIdentityProvenance, ModelIdentityResolution,
        apply_multi_account_publication_gate, resolve_model_identity_with_origins,
    };
    use crate::agent::model_providers::{ModelProviderKind, ResolvedModelProvider};
    use crate::agent::models::{
        is_task_agent_eligible, resolve_default_model_with_origins,
        selectable_catalog_key_for_persisted_with_origins, task_model_error_for_catalog,
    };
    use crate::inference::ApiBackend;
    use crate::provider_registry::management::ProviderManagementService;
    use crate::provider_registry::management::dto::{
        CredentialSlotUpdate, ProviderAddRequest, ProviderSavePatch, ProviderSaveRequest,
        SecretFieldUpdate,
    };
    use crate::provider_registry::toml_edit::{ProviderTomlPatch, upsert_provider};
    use crate::provider_registry::{
        MULTI_ACCOUNT_ROLLOUT_DEFAULT_ENABLED, MULTI_ACCOUNT_ROLLOUT_ENV, ProviderId,
        multi_account_rollout_enabled, with_multi_account_rollout_env,
    };
    use agent_client_protocol as acp;
    use indexmap::IndexMap;
    use std::num::NonZeroU64;
    use tempfile::TempDir;
    use xai_grok_inference_types::ReasoningEffortSelection;

    fn entry(model: &str) -> ModelEntry {
        entry_with_provider(model, None)
    }

    fn entry_with_provider(model: &str, provider: Option<(&str, ModelProviderKind)>) -> ModelEntry {
        entry_with_provider_tools(model, provider, None)
    }

    fn entry_with_provider_tools(
        model: &str,
        provider: Option<(&str, ModelProviderKind)>,
        supports_tools: Option<bool>,
    ) -> ModelEntry {
        ModelEntry {
            info: ModelInfo {
                user_selectable: true,
                id: None,
                model: model.to_string(),
                base_url: String::new(),
                name: None,
                description: None,
                max_completion_tokens: None,
                temperature: None,
                top_p: None,
                api_backend: ApiBackend::default(),
                auth_scheme: Default::default(),
                extra_headers: IndexMap::new(),
                context_window: NonZeroU64::new(200_000).unwrap(),
                auto_compact_threshold_percent: None,
                system_prompt_label: None,
                use_concise: false,
                agent_type: crate::agent::config::default_agent_type(),
                inference_idle_timeout_secs: None,
                max_retries: None,
                hidden: false,
                supported_in_api: true,
                reasoning_effort: None,
                supports_reasoning_effort: None,
                reasoning_efforts: Vec::new(),
                reasoning_effort_selection: ReasoningEffortSelection::Unknown,
                supports_backend_search: false,
                compactions_remaining: None,
                compaction_at_tokens: None,
                show_model_fingerprint: false,
                stream_tool_calls: None,
                laziness_detector: crate::agent::config::LazinessDetectorPerModelConfig::default(),
                supports_tools,
                supports_native_schema: None,
                supports_strict_tools: None,
                supports_image_input: None,
                supports_audio_input: None,
                supports_video_input: None,
                execution_backend: ExecutionBackend::NativeInference,
            },
            model_provider: provider.map(|(id, kind)| ResolvedModelProvider {
                id: id.to_owned(),
                kind,
                openrouter_fallback_models: Vec::new(),
                openrouter_provider_preferences: None,
                openrouter_plugins: Vec::new(),
                openrouter_pacing: false,
                command: Vec::new(),
            }),
            api_key: None,
            env_key: None,
            auth_provider: None,
            api_base_url: None,
        }
    }

    fn four_account_catalog() -> (IndexMap<String, ModelEntry>, CatalogOrigins) {
        let mut models = IndexMap::new();
        models.insert("openai:gpt-4o".into(), entry("gpt-4o"));
        models.insert(
            "openai_work:gpt-4o".into(),
            entry_with_provider("gpt-4o", Some(("openai_work", ModelProviderKind::OpenAi))),
        );
        models.insert("openrouter:shared-model".into(), entry("shared-model"));
        models.insert(
            "openrouter_team:shared-model".into(),
            entry_with_provider(
                "shared-model",
                Some(("openrouter_team", ModelProviderKind::OpenRouter)),
            ),
        );
        let mut origins = CatalogOrigins::new();
        origins.insert("openai:gpt-4o".into(), CatalogEntryOrigin::GeneratedBuiltIn);
        origins.insert(
            "openai_work:gpt-4o".into(),
            CatalogEntryOrigin::GeneratedAdditionalAccount,
        );
        origins.insert(
            "openrouter:shared-model".into(),
            CatalogEntryOrigin::GeneratedBuiltIn,
        );
        origins.insert(
            "openrouter_team:shared-model".into(),
            CatalogEntryOrigin::GeneratedAdditionalAccount,
        );
        (models, origins)
    }

    #[test]
    fn gate_d_default_enabled_and_kill_switch() {
        with_multi_account_rollout_env(|| {
            unsafe { std::env::remove_var(MULTI_ACCOUNT_ROLLOUT_ENV) };
            assert!(MULTI_ACCOUNT_ROLLOUT_DEFAULT_ENABLED);
            assert!(multi_account_rollout_enabled());
            for off in ["0", "false", "off", "no"] {
                unsafe { std::env::set_var(MULTI_ACCOUNT_ROLLOUT_ENV, off) };
                assert!(!multi_account_rollout_enabled(), "{off}");
            }
            for on in ["1", "true", "on", "yes"] {
                unsafe { std::env::set_var(MULTI_ACCOUNT_ROLLOUT_ENV, on) };
                assert!(multi_account_rollout_enabled(), "{on}");
            }
        });
    }

    #[test]
    fn gate_open_selection_default_and_resume() {
        with_multi_account_rollout_env(|| {
            unsafe { std::env::remove_var(MULTI_ACCOUNT_ROLLOUT_ENV) };

            let (mut models, origins) = four_account_catalog();
            apply_multi_account_publication_gate(&mut models, &origins);
            for key in ["openai_work:gpt-4o", "openrouter_team:shared-model"] {
                assert!(models.contains_key(key));
                assert!(!models[key].info.hidden);
                assert!(models[key].info.user_selectable);
                assert!(
                    resolve_model_identity_with_origins(&models, &origins, key)
                        .resolved()
                        .is_some(),
                    "exact additional key must resolve when gate open"
                );
            }

            let available: IndexMap<acp::ModelId, acp::ModelInfo> = models
                .keys()
                .map(|k| {
                    let id = acp::ModelId::new(k.clone());
                    (id.clone(), acp::ModelInfo::new(id, k.clone()))
                })
                .collect();
            let resumed = selectable_catalog_key_for_persisted_with_origins(
                &models,
                &available,
                &origins,
                &acp::ModelId::new("openai_work:gpt-4o"),
            );
            assert_eq!(
                resumed.as_ref().map(|m| m.0.as_ref()),
                Some("openai_work:gpt-4o")
            );

            // Built-in permanent / reserved binding must win for bare gpt-4o.
            match resolve_model_identity_with_origins(&models, &origins, "gpt-4o") {
                ModelIdentityResolution::Resolved(r) => {
                    assert_eq!(r.canonical_id.as_str(), "openai:gpt-4o");
                    assert_ne!(
                        r.provenance,
                        ModelIdentityProvenance::UniqueLegacyAlias,
                        "must not silent-sibling via legacy alias"
                    );
                }
                other => panic!("expected resolved openai:gpt-4o, got {other:?}"),
            }

            let cfg = crate::agent::config::Config::default();
            let (default_key, _, _) =
                resolve_default_model_with_origins(&cfg, &models, &origins, false);
            assert!(
                models.contains_key(&default_key),
                "default must land on a catalog key: {default_key}"
            );
        });
    }

    #[test]
    fn gate_open_task_eligibility_for_additional_accounts() {
        with_multi_account_rollout_env(|| {
            unsafe { std::env::remove_var(MULTI_ACCOUNT_ROLLOUT_ENV) };

            // OpenAiCompatible avoids vault credential gating so the tools /
            // visibility axis is isolated. Built-in openai: remains hard-rejected.
            let mut models = IndexMap::new();
            models.insert(
                "lab_vllm:shared".into(),
                entry_with_provider_tools(
                    "shared",
                    Some(("lab_vllm", ModelProviderKind::OpenAiCompatible)),
                    Some(true),
                ),
            );
            models.insert(
                "openai:gpt-4o".into(),
                entry_with_provider_tools(
                    "gpt-4o",
                    Some(("openai", ModelProviderKind::OpenAi)),
                    Some(true),
                ),
            );
            let mut origins = CatalogOrigins::new();
            origins.insert(
                "lab_vllm:shared".into(),
                CatalogEntryOrigin::GeneratedAdditionalAccount,
            );
            origins.insert("openai:gpt-4o".into(), CatalogEntryOrigin::GeneratedBuiltIn);
            apply_multi_account_publication_gate(&mut models, &origins);

            assert!(
                is_task_agent_eligible(&models["lab_vllm:shared"], false),
                "gate-on additional exact id with tools must be task-eligible"
            );
            assert!(
                task_model_error_for_catalog("lab_vllm:shared", &models, false).is_none(),
                "Task.model must accept gate-on additional exact id"
            );
            // Production hard reject for experimental openai: catalog prefix.
            let err = task_model_error_for_catalog("openai:gpt-4o", &models, false)
                .expect("openai: prefix must stay hard-rejected");
            assert!(
                err.contains("unverified tool support") || err.contains("openai:"),
                "unexpected rejection: {err}"
            );

            // OpenRouter additional: credentialed + visible so only the tools
            // gate rejects (not vault-missing → generic Unknown).
            let mut or_notools = entry_with_provider_tools(
                "notools",
                Some(("or_team", ModelProviderKind::OpenRouter)),
                None, // supports_tools absent → OpenRouter tools gate fails
            );
            or_notools.api_key = Some("sk-test-or-notools".into());
            models.insert("or_team:notools".into(), or_notools);
            origins.insert(
                "or_team:notools".into(),
                CatalogEntryOrigin::GeneratedAdditionalAccount,
            );
            assert!(
                !is_task_agent_eligible(&models["or_team:notools"], false),
                "credentialed OpenRouter without tools must fail tools policy"
            );
            let err = task_model_error_for_catalog("or_team:notools", &models, false)
                .expect("OpenRouter without tools must be rejected");
            assert!(
                err.contains("does not advertise tool support"),
                "must hit tools-specific production path, not generic Unknown: {err}"
            );

            // Separate: uncredentialed OpenRouter fails closed before tools wording.
            let uncred = entry_with_provider_tools(
                "uncred",
                Some(("or_uncred", ModelProviderKind::OpenRouter)),
                Some(true),
            );
            models.insert("or_uncred:uncred".into(), uncred);
            origins.insert(
                "or_uncred:uncred".into(),
                CatalogEntryOrigin::GeneratedAdditionalAccount,
            );
            assert!(
                !is_task_agent_eligible(&models["or_uncred:uncred"], false),
                "uncredentialed OpenRouter must not be task-eligible"
            );
            let err = task_model_error_for_catalog("or_uncred:uncred", &models, false)
                .expect("uncredentialed OpenRouter must be rejected");
            assert!(
                err.contains("Unknown Task.model slug"),
                "uncredentialed path must fail closed without tools-specific wording: {err}"
            );
            assert!(
                !err.contains("does not advertise tool support"),
                "uncredentialed rejection must not claim tools policy: {err}"
            );
        });
    }

    #[test]
    fn gate_off_selection_default_resume_and_task_fail_closed() {
        with_multi_account_rollout_env(|| {
            unsafe { std::env::set_var(MULTI_ACCOUNT_ROLLOUT_ENV, "false") };

            let (mut models, origins) = four_account_catalog();
            apply_multi_account_publication_gate(&mut models, &origins);
            // Omit parity: additional keys leave the catalog map.
            assert!(!models.contains_key("openai_work:gpt-4o"));
            assert!(!models.contains_key("openrouter_team:shared-model"));
            assert!(models.contains_key("openai:gpt-4o"));
            assert!(
                resolve_model_identity_with_origins(&models, &origins, "openai_work:gpt-4o")
                    .resolved()
                    .is_none()
            );

            let available: IndexMap<acp::ModelId, acp::ModelInfo> = models
                .keys()
                .map(|k| {
                    let id = acp::ModelId::new(k.clone());
                    (id.clone(), acp::ModelInfo::new(id, k.clone()))
                })
                .collect();
            assert!(
                selectable_catalog_key_for_persisted_with_origins(
                    &models,
                    &available,
                    &origins,
                    &acp::ModelId::new("openai_work:gpt-4o"),
                )
                .is_none(),
                "resume of additional account fails closed when kill switch is on"
            );

            let cfg = crate::agent::config::Config::default();
            let (default_key, _, _) =
                resolve_default_model_with_origins(&cfg, &models, &origins, false);
            assert!(
                !default_key.starts_with("openai_work:")
                    && !default_key.starts_with("openrouter_team:"),
                "default must not pick gated additional account: {default_key}"
            );

            // Task.model: omitted additional keys are not advertised / rejected.
            assert!(
                task_model_error_for_catalog("openai_work:gpt-4o", &models, false).is_some(),
                "gate-off must reject Task.model for omitted additional id"
            );
            let err = task_model_error_for_catalog("openai_work:gpt-4o", &models, false).unwrap();
            assert!(
                !err.contains("openai_work:gpt-4o") || err.contains("Unknown"),
                "guidance must not treat omitted additional as a valid advertised slug path only: {err}"
            );
            // Built-in openai: prefix remains hard-rejected under kill switch.
            let err = task_model_error_for_catalog("openai:gpt-4o", &models, false)
                .expect("openai: hard reject");
            assert!(
                err.contains("unverified") || err.contains("openai:"),
                "{err}"
            );
        });
    }

    #[test]
    fn ambiguous_bare_slug_two_additional_accounts_fails_closed() {
        with_multi_account_rollout_env(|| {
            unsafe { std::env::remove_var(MULTI_ACCOUNT_ROLLOUT_ENV) };

            let mut models = IndexMap::new();
            models.insert(
                "work_a:shared".into(),
                entry_with_provider("shared", Some(("work_a", ModelProviderKind::OpenAi))),
            );
            models.insert(
                "work_b:shared".into(),
                entry_with_provider("shared", Some(("work_b", ModelProviderKind::OpenAi))),
            );
            let mut origins = CatalogOrigins::new();
            origins.insert(
                "work_a:shared".into(),
                CatalogEntryOrigin::GeneratedAdditionalAccount,
            );
            origins.insert(
                "work_b:shared".into(),
                CatalogEntryOrigin::GeneratedAdditionalAccount,
            );
            match resolve_model_identity_with_origins(&models, &origins, "shared") {
                ModelIdentityResolution::Ambiguous { candidates, .. } => {
                    let ids: Vec<_> = candidates.iter().map(|c| c.as_str()).collect();
                    assert_eq!(ids, vec!["work_a:shared", "work_b:shared"]);
                }
                other => panic!("expected ambiguous bare slug, got {other:?}"),
            }
        });
    }

    #[test]
    fn unknown_toml_fields_and_comments_preserved_on_management_save() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"# header comment
[model_providers.lab]
kind = "openai_compatible"
base_url = "http://127.0.0.1:9/v1"
# inline keep
enabled = true
future_flag = "leave-me"
custom_table = { nested = 1 }

[model_providers.lab.unknown_section]
x = 42
"#,
        )
        .unwrap();
        let s = ProviderManagementService::new(dir.path());
        let save = s.save(ProviderSaveRequest {
            id: "lab".into(),
            expected_generation: s.current_generation(),
            patch: ProviderSavePatch {
                display_name: Some("Lab".into()),
                ..Default::default()
            },
        });
        assert!(save.ok, "{:?}", save.error);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# header comment"));
        assert!(text.contains("# inline keep"));
        assert!(text.contains("future_flag"));
        assert!(text.contains("leave-me"));
        assert!(text.contains("unknown_section") || text.contains("nested"));
        assert!(text.contains("Lab"));
    }

    #[test]
    fn independent_application_admin_credential_slots() {
        let dir = TempDir::new().unwrap();
        let s = ProviderManagementService::new(dir.path());
        assert!(
            s.add(ProviderAddRequest {
                id: "slots".into(),
                kind: "openai_compatible".into(),
                base_url: "http://127.0.0.1:9/v1".into(),
                display_name: None,
                admin_base_url: Some("http://127.0.0.1:9/admin".into()),
                enabled: true,
                expected_generation: s.current_generation(),
            })
            .ok
        );
        let set_app = s.apply_credential_updates(
            "slots",
            s.current_generation(),
            &CredentialSlotUpdate {
                application: SecretFieldUpdate::Set,
                ..Default::default()
            },
            Some("sk-app-only"),
            None,
        );
        assert!(set_app.ok, "{:?}", set_app.error);
        let presence = s.credential_presence("slots");
        assert!(presence.has_application_key);
        assert!(!presence.has_admin_key);

        let set_admin = s.apply_credential_updates(
            "slots",
            s.current_generation(),
            &CredentialSlotUpdate {
                admin: SecretFieldUpdate::Set,
                ..Default::default()
            },
            None,
            Some("sk-admin-only"),
        );
        assert!(set_admin.ok, "{:?}", set_admin.error);
        let presence = s.credential_presence("slots");
        assert!(presence.has_application_key);
        assert!(presence.has_admin_key);

        let clear_admin = s.apply_credential_updates(
            "slots",
            s.current_generation(),
            &CredentialSlotUpdate {
                admin: SecretFieldUpdate::Clear,
                ..Default::default()
            },
            None,
            None,
        );
        assert!(clear_admin.ok, "{:?}", clear_admin.error);
        let presence = s.credential_presence("slots");
        assert!(presence.has_application_key);
        assert!(!presence.has_admin_key);
        assert!(!format!("{presence:?}").contains("sk-"));
    }

    #[test]
    fn openrouter_policy_fields_round_trip_under_default_gate() {
        let dir = TempDir::new().unwrap();
        let s = ProviderManagementService::new(dir.path());
        assert!(
            s.add(ProviderAddRequest {
                id: "or_team".into(),
                kind: "openrouter".into(),
                base_url: "https://openrouter.ai/api/v1".into(),
                display_name: Some("Team".into()),
                admin_base_url: None,
                enabled: true,
                expected_generation: s.current_generation(),
            })
            .ok
        );
        let save = s.save(ProviderSaveRequest {
            id: "or_team".into(),
            expected_generation: s.current_generation(),
            patch: ProviderSavePatch {
                openrouter_pacing: Some(true),
                openrouter_data_collection: Some(Some("deny".into())),
                openrouter_order: Some(vec!["openai".into(), "anthropic".into()]),
                openrouter_require_parameters: Some(Some(true)),
                openrouter_zdr: Some(Some(true)),
                ..Default::default()
            },
        });
        assert!(save.ok, "{:?}", save.error);
        let detail = s.detail("or_team").unwrap();
        assert!(detail.openrouter_pacing);
        assert_eq!(detail.openrouter_data_collection.as_deref(), Some("deny"));
        assert_eq!(
            detail.openrouter_order,
            vec!["openai".to_owned(), "anthropic".to_owned()]
        );
        assert_eq!(detail.openrouter_require_parameters, Some(true));
        assert_eq!(detail.openrouter_zdr, Some(true));
    }

    #[test]
    fn upsert_preserves_unknown_fields_via_toml_edit() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[model_providers.custom]
kind = "openai_compatible"
base_url = "http://127.0.0.1:1/v1"
enabled = true
mystery = true
# keep
"#,
        )
        .unwrap();
        let id = ProviderId::new("custom").unwrap();
        upsert_provider(
            &path,
            &id,
            &ProviderTomlPatch {
                display_name: Some("Custom".into()),
                ..Default::default()
            },
            true,
        )
        .unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("mystery"));
        assert!(text.contains("# keep"));
        assert!(text.contains("Custom"));
    }
}
