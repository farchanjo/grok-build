//! Gate D acceptance coverage for multi-account catalogs.
//!
//! Reuses production seams (publication gate, identity resolution, management
//! service, TOML edit). Shared-process env mutations hold the rollout lock.

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
        resolve_default_model_with_origins, selectable_catalog_key_for_persisted_with_origins,
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
        multi_account_rollout_enabled, multi_account_rollout_env_lock,
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
                supports_tools: None,
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
        let _gate = multi_account_rollout_env_lock();
        let previous = std::env::var(MULTI_ACCOUNT_ROLLOUT_ENV).ok();
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
        match previous {
            Some(v) => unsafe { std::env::set_var(MULTI_ACCOUNT_ROLLOUT_ENV, v) },
            None => unsafe { std::env::remove_var(MULTI_ACCOUNT_ROLLOUT_ENV) },
        }
    }

    #[test]
    fn gate_open_selection_default_resume_and_task_eligibility() {
        let _gate = multi_account_rollout_env_lock();
        let previous = std::env::var(MULTI_ACCOUNT_ROLLOUT_ENV).ok();
        unsafe { std::env::remove_var(MULTI_ACCOUNT_ROLLOUT_ENV) };

        let (mut models, origins) = four_account_catalog();
        apply_multi_account_publication_gate(&mut models, &origins);
        for key in ["openai_work:gpt-4o", "openrouter_team:shared-model"] {
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

        // Built-in permanent / reserved binding must not silent-sibling to
        // additional accounts for bare slugs that map to a reserved key.
        match resolve_model_identity_with_origins(&models, &origins, "gpt-4o") {
            ModelIdentityResolution::Resolved(r) => {
                assert_eq!(r.canonical_id.as_str(), "openai:gpt-4o");
                assert_ne!(r.provenance, ModelIdentityProvenance::UniqueLegacyAlias);
            }
            ModelIdentityResolution::Ambiguous { .. } => {}
            ModelIdentityResolution::Missing { .. } => {
                panic!("gpt-4o should not be missing with openai:gpt-4o present")
            }
        }

        let cfg = crate::agent::config::Config::default();
        let (default_key, _, _) =
            resolve_default_model_with_origins(&cfg, &models, &origins, false);
        assert!(
            models.contains_key(&default_key),
            "default must land on a catalog key: {default_key}"
        );

        match previous {
            Some(v) => unsafe { std::env::set_var(MULTI_ACCOUNT_ROLLOUT_ENV, v) },
            None => unsafe { std::env::remove_var(MULTI_ACCOUNT_ROLLOUT_ENV) },
        }
    }

    #[test]
    fn gate_off_selection_default_and_resume_fail_closed() {
        let _gate = multi_account_rollout_env_lock();
        let previous = std::env::var(MULTI_ACCOUNT_ROLLOUT_ENV).ok();
        unsafe { std::env::set_var(MULTI_ACCOUNT_ROLLOUT_ENV, "false") };

        let (mut models, origins) = four_account_catalog();
        apply_multi_account_publication_gate(&mut models, &origins);
        assert!(models["openai_work:gpt-4o"].info.hidden);
        assert!(!models["openai_work:gpt-4o"].info.user_selectable);
        assert!(
            resolve_model_identity_with_origins(&models, &origins, "openai_work:gpt-4o")
                .resolved()
                .is_none()
        );

        let available: IndexMap<acp::ModelId, acp::ModelInfo> = models
            .iter()
            .filter(|(_, e)| e.info.user_selectable && !e.info.hidden)
            .map(|(k, _)| {
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

        match previous {
            Some(v) => unsafe { std::env::set_var(MULTI_ACCOUNT_ROLLOUT_ENV, v) },
            None => unsafe { std::env::remove_var(MULTI_ACCOUNT_ROLLOUT_ENV) },
        }
    }

    #[test]
    fn ambiguous_bare_slug_two_additional_accounts_fails_closed() {
        let _gate = multi_account_rollout_env_lock();
        let previous = std::env::var(MULTI_ACCOUNT_ROLLOUT_ENV).ok();
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

        match previous {
            Some(v) => unsafe { std::env::set_var(MULTI_ACCOUNT_ROLLOUT_ENV, v) },
            None => unsafe { std::env::remove_var(MULTI_ACCOUNT_ROLLOUT_ENV) },
        }
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
