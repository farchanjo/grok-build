//! PR8 rollout audit: compile gates, catalog visibility, redaction, and
//! default-feature / release-dist isolation for Claude Agent CLI + Anthropic peer.
//!
//! These tests always compile (no `claude-cli-runtime` feature required) so
//! ordinary `cargo test -p xai-grok-shell` locks the fail-closed release path.

use super::capability_matrix;
use super::envelope::ExternalRuntimeEnvelope;
use super::gates::{self, CLAUDE_CLI_ENV_OPT_IN};
use crate::agent::config::ModelEntry;
use crate::agent::execution_backend::{
    ExecutionBackend, ExternalAgentKind, ModelSwitchCrossExecutionModeError,
};
use indexmap::IndexMap;

/// Cargo manifests that forward or declare `claude-cli-runtime`.
const FEATURE_MANIFESTS: &[&str] = &[
    "crates/codegen/xai-grok-shell/Cargo.toml",
    "crates/codegen/xai-grok-pager/Cargo.toml",
    "crates/codegen/xai-grok-pager-bin/Cargo.toml",
];

fn workspace_root() -> std::path::PathBuf {
    // Crate dir is crates/codegen/xai-grok-shell → three parents to repo root.
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("workspace root")
}

fn read_manifest(rel: &str) -> String {
    let path = workspace_root().join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

/// Extract the `[features]` table body (best-effort, sufficient for gate audit).
fn features_table(manifest: &str) -> &str {
    let start = manifest
        .find("[features]")
        .expect("manifest must have [features]");
    let rest = &manifest[start + "[features]".len()..];
    let end = rest.find("\n[").unwrap_or(rest.len());
    &rest[..end]
}

#[test]
fn claude_cli_runtime_absent_from_default_and_release_dist_features() {
    for rel in FEATURE_MANIFESTS {
        let text = read_manifest(rel);
        let feats = features_table(&text);
        if let Some(def_line) = feats
            .lines()
            .find(|l| l.trim_start().starts_with("default"))
        {
            assert!(
                !def_line.contains("claude-cli-runtime"),
                "{rel}: default features must not include claude-cli-runtime: {def_line}"
            );
        }
        if let Some(rd_line) = feats
            .lines()
            .find(|l| l.trim_start().starts_with("release-dist"))
        {
            assert!(
                !rd_line.contains("claude-cli-runtime"),
                "{rel}: release-dist must not include claude-cli-runtime: {rd_line}"
            );
        }
        assert!(
            text.contains("claude-cli-runtime"),
            "{rel}: expected claude-cli-runtime feature declaration for audit surface"
        );
    }
}

#[test]
fn default_build_feature_flag_matches_cfg() {
    assert_eq!(
        gates::claude_cli_feature_compiled(),
        cfg!(feature = "claude-cli-runtime")
    );
    if !cfg!(feature = "claude-cli-runtime") {
        assert!(!gates::claude_cli_feature_compiled());
        assert!(!gates::claude_cli_both_gates_open());
        assert!(!capability_matrix::CLAUDE_CLI_MODEL_SELECTABLE);
        assert!(!capability_matrix::claude_cli_selectable());
    }
}

#[test]
fn runtime_opt_in_parser_is_fail_closed() {
    for ok in ["1", "true", "TRUE", " Yes ", "on", "ON"] {
        assert!(
            gates::parse_runtime_opt_in_value(ok),
            "expected truthy for {ok:?}"
        );
    }
    for bad in [
        "", "0", "false", "no", "off", "enable", "enabled", "maybe", "2", "claude",
    ] {
        assert!(
            !gates::parse_runtime_opt_in_value(bad),
            "expected false for {bad:?}"
        );
    }
}

#[test]
fn subscription_cli_catalog_hidden_without_gates_and_probe() {
    let mut catalog: IndexMap<String, ModelEntry> = IndexMap::new();
    let mut entry = ModelEntry::fallback(
        capability_matrix::CLAUDE_CLI_CATALOG_MODEL_ID,
        &Default::default(),
    );
    entry.info.execution_backend = ExecutionBackend::ExternalAgent(ExternalAgentKind::ClaudeCli);
    entry.info.user_selectable = true;
    entry.info.hidden = false;
    catalog.insert(
        capability_matrix::CLAUDE_CLI_CATALOG_MODEL_ID.to_owned(),
        entry,
    );

    capability_matrix::apply_catalog_visibility_with_probe(&mut catalog, Some(false));
    let cli = catalog
        .get(capability_matrix::CLAUDE_CLI_CATALOG_MODEL_ID)
        .expect("entry retained");
    assert!(cli.info.hidden, "CLI entry must be hidden without probe");
    assert!(
        !cli.info.user_selectable,
        "CLI entry must not be selectable without probe"
    );

    // Native Anthropic API-style entry remains selectable when present.
    let mut api = ModelEntry::fallback("anthropic-claude-sonnet-5", &Default::default());
    api.info.execution_backend = ExecutionBackend::NativeInference;
    api.info.user_selectable = true;
    api.info.hidden = false;
    catalog.insert("anthropic-claude-sonnet-5".into(), api);
    capability_matrix::apply_catalog_visibility_with_probe(&mut catalog, Some(false));
    let api = catalog.get("anthropic-claude-sonnet-5").unwrap();
    assert!(!api.info.hidden);
    assert!(api.info.user_selectable);
}

#[test]
fn capability_descriptor_never_implies_default_principal() {
    let d = capability_matrix::for_backend(ExecutionBackend::ExternalAgent(
        ExternalAgentKind::ClaudeCli,
    ))
    .expect("descriptor");
    assert!(d.experimental);
    assert_eq!(d.provider_peer_id, Some("anthropic"));
    assert!(
        capability_matrix::CLAUDE_CLI_UI_LIMITATIONS.contains("No API keys"),
        "limitations must state no API keys"
    );
    assert!(
        capability_matrix::CLAUDE_CLI_UI_LIMITATIONS.contains("bypassPermissions"),
        "limitations must forbid bypassPermissions"
    );
    let native = capability_matrix::for_backend(ExecutionBackend::NativeInference).unwrap();
    assert!(!native.experimental);
    assert!(native.selectable);
}

#[test]
fn env_opt_in_constant_is_documented_name() {
    assert_eq!(CLAUDE_CLI_ENV_OPT_IN, "GROK_CLAUDE_CLI_RUNTIME");
}

#[test]
fn cross_mode_error_suggests_new_session_only() {
    let err = ModelSwitchCrossExecutionModeError::new(
        ExecutionBackend::NativeInference,
        ExecutionBackend::ExternalAgent(ExternalAgentKind::ClaudeCli),
        "claude-agent-cli",
    );
    assert_eq!(err.suggestion, "start_new_session");
    let msg = err.user_message();
    assert!(msg.contains("/new") || msg.to_ascii_lowercase().contains("new"));
    assert!(!msg.contains("sk-"));
    assert!(!msg.contains("api_key"));
}

#[test]
fn external_envelope_json_omits_secrets_argv_and_raw_ndjson() {
    let env = ExternalRuntimeEnvelope::for_kind(ExternalAgentKind::ClaudeCli);
    let json = serde_json::to_string(&env).unwrap();
    let lower = json.to_ascii_lowercase();
    for forbidden in [
        "bridge_token",
        "api_key",
        "\"argv\"",
        "ndjson",
        "sk-ant-",
        "authorization",
        "x-api-key",
        "anthropic_api_key",
    ] {
        assert!(
            !lower.contains(forbidden),
            "envelope JSON must not contain {forbidden}: {json}"
        );
    }
    let dbg = format!("{env:?}");
    assert!(!dbg.contains("sk-ant-"));
    assert!(!dbg.contains("bridge_token"));
}

#[test]
fn openrouter_and_anthropic_builtin_defaults_coexist() {
    // Regression lock: Anthropic peer install must not strip OpenRouter privacy
    // defaults on the built-in provider block.
    use crate::agent::model_providers::{ModelProviderConfig, ModelProviderKind};
    use crate::agent::providers::ProviderManager;
    use indexmap::IndexMap;

    let mut providers: IndexMap<String, ModelProviderConfig> = IndexMap::new();
    let mut models = IndexMap::new();
    ProviderManager::install_model_presets_into(&mut providers, &mut models);

    let or = providers
        .get("grok_build_openrouter")
        .expect("openrouter built-in");
    assert_eq!(or.kind, ModelProviderKind::OpenRouter);
    let prefs = or
        .provider_preferences
        .as_ref()
        .expect("openrouter privacy defaults");
    assert_eq!(prefs.data_collection.as_deref(), Some("deny"));
    assert_eq!(prefs.require_parameters, Some(true));
    assert_eq!(
        or.extra_headers
            .get("X-OpenRouter-Title")
            .map(String::as_str),
        Some("Grok Build")
    );

    let anth = providers
        .get("grok_build_anthropic")
        .expect("anthropic built-in");
    assert_eq!(anth.kind, ModelProviderKind::Anthropic);
    // Anthropic must not inherit OpenRouter-only extensions.
    assert!(anth.provider_preferences.is_none());
    assert_eq!(
        anth.extra_headers
            .get("anthropic-version")
            .map(String::as_str),
        Some(xai_grok_inference::ANTHROPIC_VERSION)
    );
    // No literal API key material in the built-in provider block.
    assert!(anth.api_key.is_none());
}

#[cfg(feature = "claude-cli-runtime")]
mod with_feature {
    use super::*;
    use crate::agent::external_runtime::claude_cli::env_scrub::{
        FORBIDDEN_KEYS, build_scrubbed_env, env_contains_secret,
    };
    use crate::agent::external_runtime::claude_cli::provider_status::{
        ApiKeyStatusNote, build_status,
    };
    use std::ffi::OsString;

    #[test]
    fn feature_compiled_still_requires_env_and_probe() {
        assert!(gates::claude_cli_feature_compiled());
        if std::env::var(CLAUDE_CLI_ENV_OPT_IN).is_err() {
            assert!(!gates::claude_cli_both_gates_open());
            assert!(!capability_matrix::claude_cli_selectable());
        }
    }

    #[test]
    fn scrubbed_env_drops_provider_secrets() {
        for key in [
            "ANTHROPIC_API_KEY",
            "OPENAI_API_KEY",
            "OPENROUTER_API_KEY",
            "XAI_API_KEY",
        ] {
            assert!(
                FORBIDDEN_KEYS.contains(&key),
                "{key} must be in FORBIDDEN_KEYS"
            );
        }
        let env = build_scrubbed_env(&[]);
        for key in [
            "ANTHROPIC_API_KEY",
            "OPENAI_API_KEY",
            "OPENROUTER_API_KEY",
            "XAI_API_KEY",
            "GROK_SESSION_TOKEN",
        ] {
            assert!(
                !env_contains_secret(&env, key),
                "scrubbed env must not contain {key}"
            );
        }
        // Explicit extra secrets are also dropped.
        let env = build_scrubbed_env(&[("ANTHROPIC_API_KEY", OsString::from("sk-ant-nope"))]);
        assert!(!env_contains_secret(&env, "ANTHROPIC_API_KEY"));
    }

    #[test]
    fn provider_status_api_key_not_applicable_for_subscription() {
        let st = build_status(true, Some("2.0.0".into()), None, None, true);
        assert_eq!(
            st.anthropic_api_key_status,
            ApiKeyStatusNote::NotApplicableSubscriptionOnly
        );
        assert!(!st.summary.to_ascii_lowercase().contains("sk-"));
    }
}
