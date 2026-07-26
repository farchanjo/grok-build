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

const CLAUDE_CLI_FEATURE: &str = "claude-cli-runtime";

/// Cargo manifests that forward or declare `claude-cli-runtime`.
const FEATURE_MANIFESTS: &[&str] = &[
    "crates/codegen/xai-grok-shell/Cargo.toml",
    "crates/codegen/xai-grok-pager/Cargo.toml",
    "crates/codegen/xai-grok-pager-bin/Cargo.toml",
];

/// Composition-root manifests that must declare `release-dist` without the CLI feature.
const RELEASE_DIST_MANIFESTS: &[&str] = &[
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

/// Parse a Cargo.toml and return the exact `[features]` table (key-exact).
fn features_table(manifest: &str) -> toml::map::Map<String, toml::Value> {
    let root: toml::Value =
        toml::from_str(manifest).unwrap_or_else(|e| panic!("parse Cargo.toml: {e}"));
    let table = root
        .as_table()
        .unwrap_or_else(|| panic!("Cargo.toml root must be a table"));
    let features = table
        .get("features")
        .unwrap_or_else(|| panic!("manifest must have [features]"));
    features
        .as_table()
        .cloned()
        .unwrap_or_else(|| panic!("[features] must be a table"))
}

/// Resolve feature assignment value to a flat list of dependency strings.
///
/// Cargo allows `feat = []`, `feat = ["a", "b"]`, or (rarely) `feat = "a"`.
fn feature_deps(features: &toml::map::Map<String, toml::Value>, key: &str) -> Option<Vec<String>> {
    let val = features.get(key)?;
    Some(match val {
        toml::Value::Array(items) => items
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| panic!("feature {key:?} array entry must be a string: {v}"))
            })
            .collect(),
        toml::Value::String(s) => vec![s.clone()],
        other => panic!("feature {key:?} must be array or string, got {other}"),
    })
}

fn feature_deps_contain(deps: &[String], needle: &str) -> bool {
    deps.iter().any(|d| d == needle || d.contains(needle))
}

fn assert_feature_excludes(
    features: &toml::map::Map<String, toml::Value>,
    key: &str,
    forbidden: &str,
    rel: &str,
) {
    let deps = feature_deps(features, key)
        .unwrap_or_else(|| panic!("{rel}: expected exact feature key `{key}` to be present"));
    assert!(
        !feature_deps_contain(&deps, forbidden),
        "{rel}: feature `{key}` must not include `{forbidden}`; deps={deps:?}"
    );
}

/// Shared audit used by the live-manifest test and the adversarial fixture test.
fn audit_features_table(
    features: &toml::map::Map<String, toml::Value>,
    rel: &str,
    require_release_dist: bool,
) {
    // Exact key `default` — must not confuse with `default-bazel`.
    assert!(
        features.contains_key("default"),
        "{rel}: exact feature key `default` must be present"
    );
    assert_feature_excludes(features, "default", CLAUDE_CLI_FEATURE, rel);

    // `default-bazel` may exist; it is a different key and is not `default`.
    if let Some(deps) = feature_deps(features, "default-bazel") {
        // Still fail closed if someone put the experimental feature here.
        assert!(
            !feature_deps_contain(&deps, CLAUDE_CLI_FEATURE),
            "{rel}: default-bazel must not include {CLAUDE_CLI_FEATURE}; deps={deps:?}"
        );
    }

    if require_release_dist {
        assert!(
            features.contains_key("release-dist"),
            "{rel}: exact feature key `release-dist` must be present on composition manifests"
        );
        assert_feature_excludes(features, "release-dist", CLAUDE_CLI_FEATURE, rel);
    }

    // Standalone declaration or forwarding: key must exist as its own feature.
    let decl = feature_deps(features, CLAUDE_CLI_FEATURE).unwrap_or_else(|| {
        panic!("{rel}: expected standalone `{CLAUDE_CLI_FEATURE}` feature declaration/forwarding")
    });
    // Empty array (shell) or forwarding deps (pager / pager-bin) are both valid.
    // The key existence is the audit surface; values must not smuggle into default.
    let _ = decl;
}

#[test]
fn claude_cli_runtime_absent_from_default_and_release_dist_features() {
    for rel in FEATURE_MANIFESTS {
        let text = read_manifest(rel);
        let features = features_table(&text);
        let require_rd = RELEASE_DIST_MANIFESTS.contains(rel);
        audit_features_table(&features, rel, require_rd);
        if !require_rd {
            // shell: release-dist is optional; if present it still must exclude.
            if features.contains_key("release-dist") {
                assert_feature_excludes(&features, "release-dist", CLAUDE_CLI_FEATURE, rel);
            }
        }
    }
}

#[test]
fn feature_table_parser_uses_exact_keys_not_prefix_match() {
    // Adversarial: multiline default that sneaks claude-cli-runtime onto line 2
    // must be detected. `default-bazel` containing the string "default" must not
    // be confused with the exact key `default`.
    let adversarial = r#"
[package]
name = "fixture"
version = "0.0.0"

[features]
default = [
    "jemalloc",
    "claude-cli-runtime",
]
default-bazel = [
    "jemalloc",
]
release-dist = [
    "sandbox-enforce",
]
claude-cli-runtime = []
"#;
    let features = features_table(adversarial);
    // Exact keys resolve independently.
    let default_deps = feature_deps(&features, "default").unwrap();
    assert!(
        feature_deps_contain(&default_deps, CLAUDE_CLI_FEATURE),
        "fixture default must include the forbidden feature so the audit can catch it"
    );
    let bazel_deps = feature_deps(&features, "default-bazel").unwrap();
    assert!(
        !feature_deps_contain(&bazel_deps, CLAUDE_CLI_FEATURE),
        "default-bazel is a different key"
    );
    // Audit must fail (panic) on this fixture for `default`.
    let result = std::panic::catch_unwind(|| {
        audit_features_table(&features, "adversarial-fixture", true);
    });
    assert!(
        result.is_err(),
        "audit must reject multiline default that lists claude-cli-runtime"
    );
}

#[test]
fn feature_table_parser_accepts_clean_multiline_default() {
    let clean = r#"
[features]
default = [
    "jemalloc",
    "sandbox-enforce",
]
default-bazel = [
    "jemalloc",
    "sandbox-enforce",
    "test-support",
]
release-dist = ["xai-grok-pager/release-dist"]
claude-cli-runtime = [
    "xai-grok-pager/claude-cli-runtime",
    "xai-grok-shell/claude-cli-runtime",
]
"#;
    let features = features_table(clean);
    audit_features_table(&features, "clean-fixture", true);
    let decl = feature_deps(&features, CLAUDE_CLI_FEATURE).unwrap();
    assert!(
        feature_deps_contain(&decl, "xai-grok-shell/claude-cli-runtime"),
        "forwarding declaration preserved: {decl:?}"
    );
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
