//! PR5 foundation tests: typed execution backend, durable envelope, switch
//! guards, unavailable stub, and Anthropic peer identity.

use super::*;
use crate::agent::config::{ConfigModelOverride, ModelEntryConfig, ModelInfo};
use crate::agent::execution_backend::{
    ExecutionBackend, ExternalAgentKind, MODEL_SWITCH_CROSS_EXECUTION_MODE,
    ModelSwitchCrossExecutionModeError,
};
use crate::session::persistence::Summary;
use agent_client_protocol as acp;

#[test]
fn old_model_info_json_defaults_native() {
    let json = r#"{
        "model": "grok-4",
        "base_url": "https://api.x.ai/v1",
        "name": null,
        "description": null,
        "max_completion_tokens": null,
        "temperature": null,
        "top_p": null,
        "api_backend": "chat_completions",
        "auth_scheme": "bearer",
        "extra_headers": {},
        "context_window": 200000,
        "auto_compact_threshold_percent": null,
        "use_concise": false,
        "agent_type": "grok-build-plan",
        "inference_idle_timeout_secs": null,
        "max_retries": null,
        "hidden": false,
        "supported_in_api": true,
        "supports_backend_search": false,
        "show_model_fingerprint": false
    }"#;
    let info: ModelInfo = serde_json::from_str(json).expect("deserialize ModelInfo");
    assert!(info.execution_backend.is_native());
}

#[test]
fn old_model_entry_config_toml_defaults_native() {
    let toml = r#"
        model = "claude-sonnet-5"
        base_url = "https://api.anthropic.com/v1"
        context_window = 200000
        api_backend = "messages"
    "#;
    let entry: ModelEntryConfig = toml::from_str(toml).expect("parse ModelEntryConfig");
    assert!(entry.execution_backend.is_native());
    assert_eq!(entry.api_backend, crate::inference::ApiBackend::Messages);
}

#[test]
fn config_model_override_absent_execution_backend_stays_none() {
    let over: ConfigModelOverride = toml::from_str(r#"model = "x""#).unwrap();
    assert_eq!(over.execution_backend, None);
}

#[test]
fn config_model_override_external_claude_cli() {
    let over: ConfigModelOverride = toml::from_str(
        r#"
        model = "claude-via-cli"
        execution_backend = { external_agent = "claude_cli" }
        "#,
    )
    .expect("parse override with external backend");
    assert_eq!(
        over.execution_backend,
        Some(ExecutionBackend::ExternalAgent(
            ExternalAgentKind::ClaudeCli
        ))
    );
}

fn test_info(id: &str, cwd: &str) -> crate::session::info::Info {
    crate::session::info::Info {
        id: acp::SessionId::new(id),
        cwd: cwd.into(),
    }
}

#[test]
fn old_summary_json_defaults_native_without_envelope() {
    let info = test_info("sess-old", "/tmp");
    // Minimal old-shaped summary (no execution_backend / external_runtime keys).
    let json = serde_json::json!({
        "info": info,
        "session_summary": "",
        "created_at": "2020-01-01T00:00:00Z",
        "updated_at": "2020-01-01T00:00:00Z",
        "num_messages": 0,
        "num_chat_messages": 0,
        "current_model_id": "grok-4",
        "chat_format_version": 1,
        "next_trace_turn": 0,
        "title_is_manual": false
    });
    let summary: Summary = serde_json::from_value(json).expect("old summary loads");
    assert!(summary.execution_backend.is_native());
    assert!(summary.external_runtime.is_none());
    // Must not invent Codex field names.
    let round = serde_json::to_string(&summary).unwrap();
    assert!(!round.contains("codex_thread_id"));
    assert!(!round.contains("codex_provider"));
    assert!(!round.contains("codex_sandbox"));
}

#[test]
fn external_envelope_roundtrip_on_summary() {
    let info = test_info("sess-ext", "/workspace");
    let mut summary = Summary::new(&info, acp::ModelId::new("claude-sonnet-5")).unwrap();
    summary.execution_backend = ExecutionBackend::ExternalAgent(ExternalAgentKind::ClaudeCli);
    summary.external_runtime = Some(ExternalRuntimeEnvelope {
        kind: ExternalAgentKind::ClaudeCli,
        session_pointer: Some("ext-sess-1".into()),
        observed_version: Some("9.9.9".into()),
        capabilities: vec!["tools".into()],
        selected_model: Some("claude-sonnet-5".into()),
        resolved_model: None,
        reasoning_effort: Some("high".into()),
        token_budget: Some(50_000),
        cwd: Some("/workspace".into()),
        worktree_identity: Some("wt-abc".into()),
        result: Some(ExternalResultMetadata {
            status: "ok".into(),
            stop_reason: Some("end_turn".into()),
        }),
        usage: Some(ExternalUsageMetadata {
            input_tokens: Some(11),
            output_tokens: Some(22),
            total_tokens: Some(33),
        }),
    });
    let json = serde_json::to_string_pretty(&summary).unwrap();
    assert!(!json.contains("codex"));
    assert!(!json.contains("api_key"));
    assert!(!json.contains("argv"));
    let back: Summary = serde_json::from_str(&json).unwrap();
    assert_eq!(
        back.execution_backend,
        ExecutionBackend::ExternalAgent(ExternalAgentKind::ClaudeCli)
    );
    let env = back.external_runtime.expect("envelope");
    assert_eq!(env.session_pointer.as_deref(), Some("ext-sess-1"));
    assert_eq!(env.selected_model.as_deref(), Some("claude-sonnet-5"));
    assert_eq!(env.usage.as_ref().and_then(|u| u.total_tokens), Some(33));
}

#[test]
fn cross_mode_first_turn_allowed_post_turn_rejected() {
    let native = ExecutionBackend::NativeInference;
    let claude = ExecutionBackend::ExternalAgent(ExternalAgentKind::ClaudeCli);

    // turn_count == 0 semantics: cross-mode is allowed (guard only fires when
    // turn_count > 0). Pure helper mirrors the handler rule.
    fn reject_cross_mode(
        active: ExecutionBackend,
        target: ExecutionBackend,
        turn_count: u32,
    ) -> bool {
        active.is_cross_mode_with(target) && turn_count > 0
    }

    assert!(!reject_cross_mode(native, claude, 0));
    assert!(!reject_cross_mode(claude, native, 0));
    assert!(reject_cross_mode(native, claude, 1));
    assert!(reject_cross_mode(claude, native, 3));
    // Same-mode model switches never rejected by this rule.
    assert!(!reject_cross_mode(native, native, 10));
    assert!(!reject_cross_mode(claude, claude, 10));
}

#[test]
fn cross_mode_error_payload_suggests_new_session() {
    let err = ModelSwitchCrossExecutionModeError::new(
        ExecutionBackend::NativeInference,
        ExecutionBackend::ExternalAgent(ExternalAgentKind::ClaudeCli),
        "claude-cli-model",
    );
    assert_eq!(err.code, MODEL_SWITCH_CROSS_EXECUTION_MODE);
    assert_eq!(err.suggestion, "start_new_session");
    let acp_err = err.clone().into_acp_error();
    let parsed = ModelSwitchCrossExecutionModeError::from_acp_error(&acp_err).expect("parse");
    assert_eq!(parsed.model_id, "claude-cli-model");
    assert!(parsed.user_message().contains("start /new") || parsed.user_message().contains("/new"));
}

#[test]
fn api_backend_stays_distinct_from_execution_backend() {
    // Direct Anthropic API models: Messages wire + native execution.
    let mut info = ModelInfo::fallback("claude-sonnet-5");
    info.api_backend = crate::inference::ApiBackend::Messages;
    info.execution_backend = ExecutionBackend::NativeInference;
    assert_eq!(info.api_backend, crate::inference::ApiBackend::Messages);
    assert!(info.execution_backend.is_native());

    // Hypothetical CLI model would still list Anthropic peer identity via
    // ExternalAgentKind, not via ModelProviderKind overload.
    let cli = ExecutionBackend::ExternalAgent(ExternalAgentKind::ClaudeCli);
    assert_eq!(cli.external_kind().unwrap().provider_peer_id(), "anthropic");
    // Wire protocol and execution mode are independent axes.
    assert!(matches!(
        info.api_backend,
        crate::inference::ApiBackend::Messages
    ));
    assert!(cli.is_external());
}

#[test]
fn anthropic_provider_kind_not_overloaded_for_cli() {
    use crate::agent::model_providers::ModelProviderKind;
    // Provider kind remains Anthropic for HTTP API; CLI is execution mode only.
    assert_eq!(ModelProviderKind::Anthropic.as_config_str(), "anthropic");
    assert_eq!(
        ExternalAgentKind::ClaudeCli.provider_peer_id(),
        ModelProviderKind::Anthropic.as_config_str()
    );
    // Execution backend string is not a ModelProviderKind variant.
    assert_ne!(
        ExecutionBackend::ExternalAgent(ExternalAgentKind::ClaudeCli).mode_key(),
        "anthropic"
    );
}

#[test]
fn no_codex_magic_fields_on_execution_types() {
    let backend = ExecutionBackend::ExternalAgent(ExternalAgentKind::ClaudeCli);
    let env = ExternalRuntimeEnvelope::for_kind(ExternalAgentKind::ClaudeCli);
    let blob = format!(
        "{}{}",
        serde_json::to_string(&backend).unwrap(),
        serde_json::to_string(&env).unwrap()
    );
    for forbidden in [
        "codex_thread_id",
        "codex_provider",
        "codex_sandbox",
        "x-grok-native-agent-provider",
        "is_agent",
    ] {
        assert!(
            !blob.contains(forbidden),
            "must not reuse {forbidden} in execution backend serde"
        );
    }
}

#[tokio::test]
async fn unavailable_stub_deterministic_non_auth_invalid_request() {
    let runtime = UnavailableExternalRuntime::new(ExternalAgentKind::ClaudeCli);
    let err = runtime.probe().await.unwrap_err();
    assert_eq!(err.code(), EXTERNAL_RUNTIME_UNAVAILABLE);
    assert!(!err.is_auth_error());
    assert_eq!(err.kind, ExternalRuntimeErrorKind::Unavailable);

    let acp_err = err.into_acp_error();
    // Intentional unavailability is InvalidRequest, not InternalError/infra pause.
    let expected = agent_client_protocol::Error::new(
        agent_client_protocol::ErrorCode::InvalidRequest.into(),
        "x",
    );
    assert_eq!(acp_err.code, expected.code);
    let data = acp_err.data.as_ref().expect("data");
    assert_eq!(
        data.get("code").and_then(|v| v.as_str()),
        Some(EXTERNAL_RUNTIME_UNAVAILABLE)
    );
    assert_eq!(data.get("authError").and_then(|v| v.as_bool()), Some(false));
}

#[test]
#[serial_test::serial(claude_cli_env)]
fn catalog_visibility_hides_unselectable_claude_cli() {
    use crate::agent::config::ModelEntry;
    use indexmap::IndexMap;

    // Force gates closed for this assertion (opt-in may be set by parallel tests).
    let prior = std::env::var(super::gates::CLAUDE_CLI_ENV_OPT_IN).ok();
    unsafe {
        std::env::remove_var(super::gates::CLAUDE_CLI_ENV_OPT_IN);
    }

    let mut catalog = IndexMap::new();
    let mut entry = ModelEntry::fallback("claude-cli-model", &Default::default());
    entry.info.execution_backend = ExecutionBackend::ExternalAgent(ExternalAgentKind::ClaudeCli);
    entry.info.hidden = false;
    entry.info.user_selectable = true;
    catalog.insert("claude-cli-model".into(), entry);

    // Native Anthropic peer stays visible.
    let mut native = ModelEntry::fallback("claude-sonnet-5", &Default::default());
    native.info.execution_backend = ExecutionBackend::NativeInference;
    native.info.hidden = false;
    native.info.user_selectable = true;
    catalog.insert("claude-sonnet-5".into(), native);

    // Explicit probe-fail keeps Claude CLI hidden even if feature is compiled.
    capability_matrix::apply_catalog_visibility_with_probe(&mut catalog, Some(false));

    let cli = catalog.get("claude-cli-model").unwrap();
    assert!(
        cli.info.hidden,
        "Claude CLI must be hidden when not selectable"
    );
    assert!(
        !cli.info.user_selectable,
        "Claude CLI must not be user-selectable when flag is false"
    );

    let api = catalog.get("claude-sonnet-5").unwrap();
    assert!(!api.info.hidden);
    assert!(api.info.user_selectable);

    unsafe {
        match prior {
            Some(v) => std::env::set_var(super::gates::CLAUDE_CLI_ENV_OPT_IN, v),
            None => std::env::remove_var(super::gates::CLAUDE_CLI_ENV_OPT_IN),
        }
    }
}

#[test]
fn summary_execution_mode_survives_serde_with_envelope() {
    let info = test_info("sess-resume", "/ws");
    let mut summary = Summary::new(&info, acp::ModelId::new("claude-sonnet-5")).unwrap();
    summary.execution_backend = ExecutionBackend::ExternalAgent(ExternalAgentKind::ClaudeCli);
    let mut env = ExternalRuntimeEnvelope::for_kind(ExternalAgentKind::ClaudeCli);
    env.session_pointer = Some("ext-resume-1".into());
    env.selected_model = Some("claude-sonnet-5".into());
    env.validate().unwrap();
    summary.external_runtime = Some(env);

    let json = serde_json::to_string(&summary).unwrap();
    let back: Summary = serde_json::from_str(&json).unwrap();
    assert_eq!(
        back.execution_backend,
        ExecutionBackend::ExternalAgent(ExternalAgentKind::ClaudeCli)
    );
    assert_eq!(
        back.external_runtime
            .as_ref()
            .and_then(|e| e.session_pointer.as_deref()),
        Some("ext-resume-1")
    );
    // Catalog-native model id on the summary does not flip mode.
    assert!(back.execution_backend.is_external());
}

#[test]
fn envelope_rejects_ndjson_shaped_blobs() {
    let mut env = ExternalRuntimeEnvelope::for_kind(ExternalAgentKind::ClaudeCli);
    env.session_pointer = Some("line1\n{\"type\":\"event\"}\n".into());
    assert!(env.validate().is_err());
}

#[test]
fn capability_matrix_keeps_claude_cli_experimental_and_gated() {
    use super::capability_matrix::{self, CLAUDE_CLI_MODEL_SELECTABLE};
    // Without the compile feature, the static selectable flag is false.
    // With the feature, the flag is true but runtime opt-in still gates
    // is_selectable_now / claude_cli_selectable.
    assert_eq!(
        CLAUDE_CLI_MODEL_SELECTABLE,
        cfg!(feature = "claude-cli-runtime")
    );
    let d = capability_matrix::for_backend(ExecutionBackend::ExternalAgent(
        ExternalAgentKind::ClaudeCli,
    ))
    .unwrap();
    assert!(d.experimental);
    assert_eq!(d.selectable, CLAUDE_CLI_MODEL_SELECTABLE);
    assert_eq!(d.provider_peer_id, Some("anthropic"));
    assert!(d.label.contains("Experimental"));
    // Default test process has no GROK_CLAUDE_CLI_RUNTIME opt-in → not selectable now.
    if std::env::var(super::gates::CLAUDE_CLI_ENV_OPT_IN).is_err() {
        assert!(!d.is_selectable_now());
        assert!(!capability_matrix::claude_cli_selectable());
    }

    let native = capability_matrix::for_backend(ExecutionBackend::NativeInference).unwrap();
    assert!(!native.experimental);
    assert!(native.selectable);
}

#[test]
fn subagent_meta_legacy_codex_fields_still_roundtrip() {
    let meta = crate::agent::subagent::SubagentMeta {
        subagent_id: "s1".into(),
        parent_session_id: "p1".into(),
        child_session_id: "c1".into(),
        subagent_type: "general-purpose".into(),
        description: "t".into(),
        prompt: "do work".into(),
        status: "running".into(),
        started_at: chrono::Utc::now(),
        completed_at: None,
        duration_ms: None,
        tool_calls: None,
        turns: None,
        error: None,
        effective_context_source: None,
        context_normalized: false,
        fork_copy_error: None,
        persona: None,
        resumed_from: None,
        child_cwd: None,
        worktree_path: None,
        snapshot_ref: None,
        effective_model_id: None,
        codex_thread_id: Some("thread-1".into()),
        codex_provider: Some("codex".into()),
        codex_sandbox: Some("workspace-write".into()),
        external_runtime_kind: None,
        external_session_pointer: None,
    };
    let json = serde_json::to_string(&meta).unwrap();
    assert!(json.contains("codex_thread_id"));
    let back: crate::agent::subagent::SubagentMeta = serde_json::from_str(&json).unwrap();
    assert_eq!(back.codex_thread_id.as_deref(), Some("thread-1"));
    assert!(back.external_session_pointer.is_none());
}
