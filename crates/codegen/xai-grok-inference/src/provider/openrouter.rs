//! OpenRouter adapter.
//!
//! Transcribed from today's branch sites:
//! - OpenRouter requests the `X-OpenRouter-Metadata: enabled` diagnostics
//!   header (`agent/config.rs`) and carries native `provider` / `plugins`
//!   request-body extensions.
//! - OpenRouter gets a higher 429 retry cap, overridable via
//!   `GROK_OPENROUTER_RATE_LIMIT_RETRIES` (`retry.rs`).
//! - OpenRouter spaces requests with a default minimum interval and enters a
//!   conservative recovery mode after a 429 (`pacing.rs`).
//! - A served model differing from the requested model is an OpenRouter
//!   fallback (`chat_completions.rs`).
//! - OpenRouter streams structured `reasoning_details` blocks that must be
//!   gated (kept) for multi-turn echo.

use super::{
    AuthScheme, ProviderAdapter, ProviderKind, ProviderPolicy, ReasoningEcho,
    ReasoningWire,
};
use super::{
    OPENROUTER_DEFAULT_PACING, OPENROUTER_RATE_LIMIT_RETRY_THRESHOLD,
    RequestContext, RequestExtensions,
};
use crate::route_context::RouteProviderKind;
use xai_grok_inference_types::ApiBackend;

/// Static policy for OpenRouter.
pub static OPENROUTER_PROVIDER_POLICY: ProviderPolicy = ProviderPolicy {
    route_kind: RouteProviderKind::OpenRouter,
    backend_preference: &[ApiBackend::ChatCompletions, ApiBackend::Responses],
    usage_policy: super::UsagePolicy::new(false, false, true),
    pacing_default: OPENROUTER_DEFAULT_PACING,
};

/// A stateless OpenRouter adapter.
#[derive(Debug, Clone, Copy)]
pub struct OpenRouterAdapter;

impl OpenRouterAdapter {
    /// Construct an OpenRouter adapter.
    pub fn new() -> Self {
        Self
    }
}

impl Default for OpenRouterAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderAdapter for OpenRouterAdapter {
    fn id(&self) -> ProviderKind {
        ProviderKind::OpenRouter
    }

    fn auth(&self) -> AuthScheme {
        AuthScheme::Bearer
    }

    fn policy(&self) -> &'static ProviderPolicy {
        &OPENROUTER_PROVIDER_POLICY
    }

    fn request_extensions(&self, _cx: &RequestContext) -> RequestExtensions {
        // OpenRouter: diagnostics metadata header + native body extensions
        // (provider / plugins / reasoning). `model_id` metadata is rejected.
        RequestExtensions {
            openrouter_metadata_header: true,
            openrouter_body: true,
            ..RequestExtensions::default()
        }
    }

    fn reasoning_wire(&self) -> ReasoningWire {
        // OpenRouter echoes reasoning content AND keeps the structured
        // `reasoning_details` blocks for multi-turn fidelity.
        ReasoningWire {
            request_key: "reasoning_content",
            echo: ReasoningEcho::Details,
        }
    }

    fn rate_limit_threshold(&self, env: Option<&str>) -> u32 {
        env.and_then(|value| value.parse::<u32>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(OPENROUTER_RATE_LIMIT_RETRY_THRESHOLD)
    }

    fn detect_fallback(&self, requested: &str, served: &str) -> bool {
        !requested.is_empty() && !served.is_empty() && requested != served
    }

    fn shape_delta(&self, _delta: &mut xai_grok_inference_types::ChatChunkDelta) {
        // OpenRouter gates `reasoning_details` through untouched so the codec
        // accumulates the structured blocks for echo-back. Undefined here
        // because the trait default is already the identity no-op.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xai_grok_inference_types::ChatChunkDelta;

    #[test]
    fn id_and_auth() {
        let a = OpenRouterAdapter::new();
        assert_eq!(a.id(), ProviderKind::OpenRouter);
        assert_eq!(a.auth(), AuthScheme::Bearer);
    }

    #[test]
    fn policy_bundles_openrouter_route_metadata_and_pacing() {
        let policy = OpenRouterAdapter::new().policy();
        assert_eq!(policy.route_kind, RouteProviderKind::OpenRouter);
        assert_eq!(
            policy.backend_preference,
            &[ApiBackend::ChatCompletions, ApiBackend::Responses]
        );
        assert!(!policy.usage_policy.include_message_model_id);
        assert!(!policy.usage_policy.first_party);
        assert!(policy.usage_policy.openrouter_metadata);
        assert_eq!(
            policy.pacing_default,
            crate::provider::PacingDefault::OpenRouter {
                min_interval_ms: 2_000,
                recovery_requests: 8,
            }
        );
    }

    #[test]
    fn request_extensions_enable_openrouter_header_and_body() {
        let a = OpenRouterAdapter::new();
        let ext = a.request_extensions(&RequestContext::default());
        assert!(ext.openrouter_metadata_header);
        assert!(ext.openrouter_body);
        assert!(!ext.first_party_headers);
        assert!(!ext.zai_body);
        assert!(!ext.chatgpt_oauth_headers);
        assert!(!ext.include_message_model_id);
    }

    #[test]
    fn reasoning_wire_uses_details_echo() {
        let a = OpenRouterAdapter::new();
        assert_eq!(a.reasoning_wire().request_key, "reasoning_content");
        assert_eq!(a.reasoning_wire().echo, ReasoningEcho::Details);
    }

    #[test]
    fn rate_limit_threshold_defaults_to_openrouter_cap() {
        let a = OpenRouterAdapter::new();
        assert_eq!(
            a.rate_limit_threshold(None),
            OPENROUTER_RATE_LIMIT_RETRY_THRESHOLD
        );
        assert_eq!(a.rate_limit_threshold(Some("5")), 5);
        // Non-positive env falls back to the default (never disable retries).
        assert_eq!(
            a.rate_limit_threshold(Some("0")),
            OPENROUTER_RATE_LIMIT_RETRY_THRESHOLD
        );
    }

    #[test]
    fn detect_fallback_when_served_model_differs() {
        let a = OpenRouterAdapter::new();
        assert!(a.detect_fallback("anthropic/claude-opus-4", "openai/gpt-5-mini"));
        assert!(!a.detect_fallback("gpt-4", "gpt-4"));
        assert!(!a.detect_fallback("", "gpt-5"));
        assert!(!a.detect_fallback("gpt-4", ""));
    }

    #[test]
    fn shape_delta_gates_reasoning_details_through() {
        let a = OpenRouterAdapter::new();
        let mut delta = ChatChunkDelta {
            content: Some("answer".into()),
            reasoning_content: Some("thought".into()),
            reasoning_details: vec![serde_json::json!({"type": "reasoning.text"})],
            ..ChatChunkDelta::default()
        };
        a.shape_delta(&mut delta);
        // OpenRouter keeps the structured detail blocks for echo-back.
        assert_eq!(delta.reasoning_details.len(), 1);
        assert_eq!(delta.content.as_deref(), Some("answer"));
    }
}
