//! OpenRouter recovery baseline — executable regression matrix.
//!
//! This module is the honest contract suite for Milestone 1. It documents
//! **exact** cargo filters that exercise each frozen OpenRouter behavior and
//! runs local deterministic checks (inventory, 401 classification, 402 copy).
//!
//! ## Executable matrix (run under `~/.grokdev` env)
//!
//! | Behavior | Exact command |
//! | --- | --- |
//! | OpenAPI inventory integrity | `cargo test -p xai-grok-inference openrouter_baseline` |
//! | Catalog / credits parse+cache | `cargo test -p xai-grok-shell --lib parse_openrouter_credits` and `cargo test -p xai-grok-shell --lib parse_openrouter_catalog` / openrouter cache tests in `agent::providers` |
//! | Chat wire + fallbacks + cost | `cargo test -p xai-grok-inference openrouter_` (stream/client unit tests) |
//! | Tool deltas / reasoning details | covered by `stream::chat_completions` tests filtered by `tool_call` / `reasoning` |
//! | Mid-stream error | `cargo test -p xai-grok-inference mid_stream_error` |
//! | HTTP 402 copy | `cargo test -p xai-grok-inference status_402_openrouter` |
//! | HTTP 429 pacing/retry | `cargo test -p xai-grok-inference openrouter` (pacing + `resolve_rate_limit_threshold_openrouter`) |
//! | Cancellation | `cargo test -p xai-grok-inference cancel_in_flight` |
//! | Moonshot OpenRouter 401 (shell) | `cargo test -p xai-grok-shell --lib moonshot_openrouter_401` |
//! | OpenRouter 401 (pager) | `cargo test -p xai-grok-pager --lib openrouter_moonshot` |
//! | Compaction OpenRouter 401 | `cargo test -p xai-grok-shell --lib surface_compact_auth_failure_openrouter` |
//! | This checklist | `cargo test -p xai-grok-inference openrouter_regression` |
//!
//! Do **not** assume a broad `openrouter` filter covers shell/pager tests —
//! those live in other crates and require the exact commands above.

use reqwest::StatusCode;
use xai_grok_inference::config::ProviderIdentity;
use xai_grok_inference::openrouter_baseline::{
    coding_agent_priority_endpoints, inventory_has_endpoint, openrouter_endpoint_inventory,
    schema_field_names,
};
use xai_grok_inference_types::{InferenceError, status_user_message_for};

#[test]
fn openrouter_baseline_inventory_is_loadable_and_complete() {
    let inv = openrouter_endpoint_inventory();
    assert_eq!(inv.provider, "openrouter");
    assert_eq!(inv.baseline.content_bytes, 1_653_634);
    assert_eq!(
        inv.baseline.content_sha256,
        "90c87070f5c2bd83c4d8e8b336dc7a4ea265e901198812d300a069a977b3f203"
    );
    assert!(inv.baseline.endpoint_count >= 80);
    for key in coding_agent_priority_endpoints() {
        assert!(inventory_has_endpoint(key), "missing {key}");
    }
    let chat = schema_field_names("ChatRequest").expect("ChatRequest");
    assert!(chat.iter().any(|f| f == "models"));
    assert!(chat.iter().any(|f| f == "provider"));
    assert!(chat.iter().any(|f| f == "plugins"));
    let prefs = schema_field_names("ProviderPreferences").expect("ProviderPreferences");
    assert!(prefs.iter().any(|f| f == "enforce_distillable_text"));
}

#[test]
fn openrouter_402_copy_is_provider_named_and_credits_focused() {
    let msg = status_user_message_for(
        StatusCode::PAYMENT_REQUIRED,
        ProviderIdentity::OpenRouter.label(),
    );
    assert!(msg.contains("OpenRouter"), "{msg}");
    assert!(msg.contains("credits") || msg.contains("credit"), "{msg}");
    assert!(msg.contains("402") || msg.contains("Payment"), "{msg}");
    assert!(!msg.contains("Grok"), "{msg}");
    assert!(!msg.contains("/login"), "{msg}");
}

#[test]
fn openrouter_401_is_auth_error_kind() {
    let err = InferenceError::Api {
        status: StatusCode::UNAUTHORIZED,
        message: "Unauthorized".into(),
        model_metadata: None,
        retry_after_secs: None,
        should_retry: None,
        diagnostics: None,
        error_code: None,
    };
    assert!(err.is_auth_error());
}

#[test]
fn openrouter_identity_label_is_stable() {
    assert_eq!(ProviderIdentity::OpenRouter.label(), "OpenRouter");
    assert!(ProviderIdentity::OpenRouter.is_openrouter());
    assert!(!ProviderIdentity::OpenRouter.is_first_party());
}

/// Matrix self-check: the documented priority endpoints remain present.
#[test]
fn openrouter_regression_matrix_priority_endpoints_documented() {
    let expected = [
        "POST /chat/completions",
        "POST /responses",
        "POST /messages",
        "GET /models",
        "GET /key",
        "GET /credits",
        "GET /generation",
        "POST /embeddings",
    ];
    let inv = coding_agent_priority_endpoints();
    for e in expected {
        assert!(
            inv.iter().any(|p| p == e),
            "priority list missing {e}: {inv:?}"
        );
        assert!(inventory_has_endpoint(e), "endpoint missing {e}");
    }
}
