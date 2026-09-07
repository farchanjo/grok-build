//! Direct Anthropic Messages API adapter.
//!
//! Transcribed from today's branch sites:
//! - Sole `x-api-key` + `anthropic-version` emitter (`anthropic/client.rs`).
//! - Never first-party xAI, never OpenRouter routing/diagnostics.
//! - The Anthropic client runs its own wire codec, so this adapter only
//!   supplies auth/extensions/reasoning policy (no responses delta hook).
//! - Error classification favors Anthropic's error-type vocabulary
//!   (`anthropic/error.rs`).

use super::{
    AuthScheme, ProviderAdapter, ProviderKind, ProviderPolicy, ReasoningEcho,
};
use super::{ApiError, ErrorClass, RequestContext, RequestExtensions, ReasoningWire};
use crate::route_context::RouteProviderKind;
use xai_grok_inference_types::ApiBackend;

/// Static policy for direct Anthropic.
pub static ANTHROPIC_PROVIDER_POLICY: ProviderPolicy = ProviderPolicy {
    route_kind: RouteProviderKind::Anthropic,
    backend_preference: &[ApiBackend::Messages],
    usage_policy: super::UsagePolicy::new(false, false, false),
    pacing_default: super::PacingDefault::None,
};

/// A stateless Anthropic adapter.
#[derive(Debug, Clone, Copy)]
pub struct AnthropicAdapter;

impl AnthropicAdapter {
    /// Construct an Anthropic adapter.
    pub fn new() -> Self {
        Self
    }
}

impl Default for AnthropicAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderAdapter for AnthropicAdapter {
    fn id(&self) -> ProviderKind {
        ProviderKind::Anthropic
    }

    fn auth(&self) -> AuthScheme {
        AuthScheme::XApiKey
    }

    fn policy(&self) -> &'static ProviderPolicy {
        &ANTHROPIC_PROVIDER_POLICY
    }

    fn request_extensions(&self, _cx: &RequestContext) -> RequestExtensions {
        // Anthropic stays isolated: no first-party headers, no OpenRouter
        // body/metadata, no Z.ai body. `model_id` metadata is rejected.
        RequestExtensions::default()
    }

    fn reasoning_wire(&self) -> ReasoningWire {
        ReasoningWire {
            request_key: "thinking",
            echo: ReasoningEcho::Echo,
        }
    }

    fn classify_error(&self, raw: &ApiError) -> ErrorClass {
        // Prefer Anthropic's error-type vocabulary embedded in the message;
        // fall back to status-code classification.
        let lower = raw.message.to_ascii_lowercase();
        if lower.contains("authentication") {
            ErrorClass::PermanentAuth
        } else if lower.contains("permission") {
            ErrorClass::PermanentPermission
        } else if lower.contains("rate_limit") {
            ErrorClass::RetryableRateLimit
        } else if lower.contains("overloaded") {
            ErrorClass::RetryableOverload
        } else if lower.contains("not_found") || lower.contains("not found") {
            ErrorClass::NotFound
        } else {
            super::classify_error_by_status(raw.status)
        }
    }

    fn shape_delta(&self, delta: &mut xai_grok_inference_types::ChatChunkDelta) {
        // Anthropic Messages codec lives inside its own client; clear the
        // OpenRouter-only `reasoning_details` anyway so a shared sampling path
        // never accumulates foreign detail blocks.
        delta.reasoning_details.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_and_auth() {
        let a = AnthropicAdapter::new();
        assert_eq!(a.id(), ProviderKind::Anthropic);
        assert_eq!(a.auth(), AuthScheme::XApiKey);
    }

    #[test]
    fn policy_bundles_anthropic_route_and_messages_backend() {
        let policy = AnthropicAdapter::new().policy();
        assert_eq!(policy.route_kind, RouteProviderKind::Anthropic);
        assert_eq!(policy.backend_preference, &[ApiBackend::Messages]);
        assert!(!policy.usage_policy.include_message_model_id);
        assert!(!policy.usage_policy.first_party);
        assert!(!policy.usage_policy.openrouter_metadata);
        assert_eq!(policy.pacing_default, crate::provider::PacingDefault::None);
    }

    #[test]
    fn request_extensions_add_nothing() {
        let a = AnthropicAdapter::new();
        let ext = a.request_extensions(&RequestContext::default());
        assert!(!ext.first_party_headers);
        assert!(!ext.openrouter_metadata_header);
        assert!(!ext.openrouter_body);
        assert!(!ext.zai_body);
        assert!(!ext.chatgpt_oauth_headers);
        assert!(!ext.include_message_model_id);
    }

    #[test]
    fn reasoning_wire_uses_thinking_key() {
        let a = AnthropicAdapter::new();
        assert_eq!(a.reasoning_wire().request_key, "thinking");
        assert_eq!(a.reasoning_wire().echo, ReasoningEcho::Echo);
    }

    #[test]
    fn classify_error_favors_anthropic_vocabulary() {
        let a = AnthropicAdapter::new();
        assert_eq!(
            a.classify_error(&ApiError::new(500, "authentication_error")),
            ErrorClass::PermanentAuth
        );
        assert_eq!(
            a.classify_error(&ApiError::new(500, "rate_limit_error")),
            ErrorClass::RetryableRateLimit
        );
        assert_eq!(
            a.classify_error(&ApiError::new(500, "overloaded_error")),
            ErrorClass::RetryableOverload
        );
        // Known status still wins when the message is neutral.
        assert_eq!(
            a.classify_error(&ApiError::new(401, "Unauthorized")),
            ErrorClass::PermanentAuth
        );
        assert_eq!(
            a.classify_error(&ApiError::new(404, "resource not_found")),
            ErrorClass::NotFound
        );
    }

    #[test]
    fn never_reports_fallback() {
        let a = AnthropicAdapter::new();
        assert!(!a.detect_fallback("claude-opus-4", "claude-sonnet-4"));
    }
}
