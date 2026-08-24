//! `handle_set_session_model` must be all-or-nothing: a disabled, tombstoned,
//! or otherwise unusable canonical route fails closed before any mutation of
//! chat state, credentials, compaction, selection, image budget, external
//! runtime, or sampler.

use super::support::*;
use super::*;
use crate::agent::execution_backend::ExecutionBackend;
use crate::agent::model_providers::{ModelProviderKind, ResolvedModelProvider};

#[tokio::test(flavor = "current_thread")]
async fn unusable_route_leaves_selection_inference_credentials_and_route_unchanged() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _) =
                tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _) = tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
            let actor = create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await;

            let mut original_settings = actor
                .chat_state_handle
                .get_inference_settings()
                .await
                .expect("original inference settings");
            original_settings.model = "original-wire".to_owned();
            original_settings.base_url = "https://original.example/v1".to_owned();
            original_settings.temperature = Some(0.2);
            actor
                .chat_state_handle
                .update_inference_settings(original_settings.clone());
            actor
                .chat_state_handle
                .update_credentials(xai_chat_state::Credentials {
                    api_key: Some("original-key".to_owned()),
                    auth_type: xai_chat_state::AuthType::ApiKey,
                    alpha_test_key: Some("original-alpha".to_owned()),
                    client_version: Some("original-client".to_owned()),
                });
            *actor.selection_model_id.borrow_mut() =
                acp::ModelId::new("original-selection".to_owned());
            let original_route = xai_grok_inference::ProviderRouteContext::legacy_from_config(
                &xai_grok_inference::InferenceConfig {
                    model: "original-wire".to_owned(),
                    base_url: "https://original.example/v1".to_owned(),
                    provider_identity: xai_grok_inference::config::ProviderIdentity::Custom,
                    ..xai_grok_inference::InferenceConfig::default()
                },
            );
            *actor.route_context.borrow_mut() = Some(original_route.clone());
            actor
                .execution_backend
                .set(ExecutionBackend::NativeInference);
            actor.compaction.threshold_percent.set(85);

            let mut unusable = crate::agent::config::ModelEntry {
                info: crate::agent::config::ModelInfo::fallback("unusable-wire"),
                model_provider: Some(ResolvedModelProvider {
                    id: "unusable-test-provider-instance".to_owned(),
                    kind: ModelProviderKind::OpenAiCompatible,
                    openrouter_fallback_models: Vec::new(),
                    openrouter_provider_preferences: None,
                    openrouter_plugins: Vec::new(),
                    openrouter_pacing: false,
                    command: Vec::new(),
                }),
                api_key: Some("must-not-be-installed".to_owned()),
                env_key: None,
                auth_provider: None,
                api_base_url: None,
            };
            unusable.info.id = Some("unusable-selection".to_owned());
            unusable.info.base_url = "https://unusable.example/v1".to_owned();
            actor
                .models_manager
                .insert_test_entry("unusable-selection", unusable);

            let incoming = xai_grok_inference::InferenceConfig {
                api_key: Some("must-not-be-installed".to_owned()),
                base_url: "https://unusable.example/v1".to_owned(),
                model: "unusable-wire".to_owned(),
                context_window: 128_000,
                temperature: Some(0.9),
                client_version: Some("incoming-client".to_owned()),
                provider_identity: xai_grok_inference::config::ProviderIdentity::Custom,
                ..xai_grok_inference::InferenceConfig::default()
            };
            let err = actor
                .handle_set_session_model(
                    acp::ModelId::new("unusable-selection".to_owned()),
                    incoming,
                    false,
                    false,
                    true,
                    50,
                    ExecutionBackend::NativeInference,
                )
                .await
                .expect_err("unusable canonical route must fail closed");
            let err_text = err.to_string();
            assert!(
                err_text.contains("provider route unusable for model switch")
                    || err_text.contains("unusable-test-provider-instance"),
                "fail-closed error must identify the unusable route, got {err_text}"
            );

            assert_eq!(
                actor.selection_model_id.borrow().0.as_ref(),
                "original-selection",
                "selection_model_id must stay unchanged on route failure"
            );
            let settings = actor
                .chat_state_handle
                .get_inference_settings()
                .await
                .expect("inference settings after failed switch");
            assert_eq!(settings.model, "original-wire");
            assert_eq!(settings.base_url, "https://original.example/v1");
            assert_eq!(settings.temperature, Some(0.2));
            let creds = actor.chat_state_handle.get_credentials().await;
            assert_eq!(creds.api_key.as_deref(), Some("original-key"));
            assert_eq!(creds.alpha_test_key.as_deref(), Some("original-alpha"));
            assert_eq!(creds.client_version.as_deref(), Some("original-client"));
            assert_eq!(
                actor.route_context.borrow().as_ref(),
                Some(&original_route),
                "route_context must stay unchanged on route failure"
            );
            assert_eq!(
                actor.execution_backend.get(),
                ExecutionBackend::NativeInference
            );
            assert_eq!(actor.compaction.threshold_percent.get(), 85);
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn current_model_route_keeps_canonical_selection_distinct_from_wire_model() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _) =
                tokio::sync::mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
            let (persistence_tx, _) = tokio::sync::mpsc::unbounded_channel::<PersistenceMsg>();
            let actor = create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await;

            let mut settings = actor
                .chat_state_handle
                .get_inference_settings()
                .await
                .expect("inference settings");
            settings.model = "gpt-5.6-sol".to_owned();
            settings.base_url = crate::auth::chatgpt_oauth::CODEX_RESPONSES_BASE_URL.to_owned();
            settings.extra_headers.insert(
                crate::agent::model_providers::NATIVE_AGENT_PROVIDER_HEADER.to_owned(),
                "native-test".to_owned(),
            );
            actor.chat_state_handle.update_inference_settings(settings);
            *actor.selection_model_id.borrow_mut() =
                acp::ModelId::new("chatgpt-gpt-5.6-sol".to_owned());

            let route = actor.current_model_route().await;
            assert_eq!(route.selection_model_id, "chatgpt-gpt-5.6-sol");
            assert_eq!(route.wire_model, "gpt-5.6-sol");
            assert_eq!(route.native_provider.as_deref(), Some("native-test"));
        })
        .await;
}
