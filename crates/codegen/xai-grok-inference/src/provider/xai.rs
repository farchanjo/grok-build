//! First-party xAI (grok) adapter.
//!
//! Transcribed from today's branch sites:
//! - First-party `x-grok-*` client identity headers only for xAI
//!   (`client.rs` `is_first_party()`).
//! - Keeps the non-standard `messages[].model_id` metadata
//!   (`include_message_model_id = true`).
//! - Generic rate-limit threshold (no OpenRouter env override).

use super::{AuthScheme, ProviderAdapter, ProviderKind, ProviderPolicy, RequestContext};
use super::RequestExtensions;
use crate::route_context::RouteProviderKind;
use xai_grok_inference_types::{ApiBackend, ChatChunkDelta};

/// Static policy for the first-party xAI provider.
pub static XAI_PROVIDER_POLICY: ProviderPolicy = ProviderPolicy {
    route_kind: RouteProviderKind::Xai,
    backend_preference: &[ApiBackend::ChatCompletions, ApiBackend::Responses],
    usage_policy: super::UsagePolicy::new(true, true, false),
    pacing_default: super::PacingDefault::None,
};

/// A stateless first-party xAI adapter.
#[derive(Debug, Clone, Copy)]
pub struct XaiAdapter;

impl XaiAdapter {
    /// Construct an xAI adapter.
    pub fn new() -> Self {
        Self
    }
}

impl Default for XaiAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderAdapter for XaiAdapter {
    fn id(&self) -> ProviderKind {
        ProviderKind::Xai
    }

    fn auth(&self) -> AuthScheme {
        // First-party xAI session/identity token.
        AuthScheme::FirstPartyToken
    }

    fn policy(&self) -> &'static ProviderPolicy {
        &XAI_PROVIDER_POLICY
    }

    fn request_extensions(&self, _cx: &RequestContext) -> RequestExtensions {
        // xAI is the sole first-party: it emits the stable session/conversation
        // identifiers in `x-grok-*` request headers and keeps `model_id`.
        RequestExtensions {
            first_party_headers: true,
            include_message_model_id: true,
            ..RequestExtensions::default()
        }
    }

    fn reasoning_wire(&self) -> super::ReasoningWire {
        super::ReasoningWire {
            request_key: "reasoning_content",
            echo: super::ReasoningEcho::Echo,
        }
    }

    fn shape_delta(&self, delta: &mut ChatChunkDelta) {
        // xAI does not use the OpenRouter-only `reasoning_details` blocks;
        // clear them so a shared sampling path never accumulates foreign
        // detail blocks.
        delta.reasoning_details.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_and_auth() {
        let a = XaiAdapter::new();
        assert_eq!(a.id(), ProviderKind::Xai);
        assert_eq!(a.auth(), AuthScheme::FirstPartyToken);
    }

    #[test]
    fn policy_bundles_xai_route_and_first_party_usage() {
        let policy = XaiAdapter::new().policy();
        assert_eq!(policy.route_kind, RouteProviderKind::Xai);
        assert_eq!(
            policy.backend_preference,
            &[ApiBackend::ChatCompletions, ApiBackend::Responses]
        );
        assert!(policy.usage_policy.include_message_model_id);
        assert!(policy.usage_policy.first_party);
        assert!(!policy.usage_policy.openrouter_metadata);
        assert_eq!(
            policy.pacing_default,
            crate::provider::PacingDefault::None
        );
    }

    #[test]
    fn request_extensions_enable_first_party_headers_only() {
        let a = XaiAdapter::new();
        let cx = RequestContext::default();
        let ext = a.request_extensions(&cx);
        assert!(ext.first_party_headers);
        assert!(ext.include_message_model_id);
        assert!(!ext.openrouter_metadata_header);
        assert!(!ext.openrouter_body);
        assert!(!ext.zai_body);
        assert!(!ext.chatgpt_oauth_headers);
    }

    #[test]
    fn rate_limit_threshold_ignores_env_override() {
        let a = XaiAdapter::new();
        assert_eq!(
            a.rate_limit_threshold(None),
            crate::provider::RATE_LIMIT_RETRY_THRESHOLD
        );
        assert_eq!(
            a.rate_limit_threshold(Some("9")),
            crate::provider::RATE_LIMIT_RETRY_THRESHOLD
        );
    }

    #[test]
    fn never_reports_fallback() {
        let a = XaiAdapter::new();
        assert!(!a.detect_fallback("grok-3", "grok-4"));
        assert!(!a.detect_fallback("", ""));
    }

    #[test]
    fn reasoning_wire_echoes_reasoning_content() {
        let a = XaiAdapter::new();
        let wire = a.reasoning_wire();
        assert_eq!(wire.request_key, "reasoning_content");
        assert_eq!(wire.echo, crate::provider::ReasoningEcho::Echo);
    }

    #[test]
    fn shape_delta_clears_openrouter_only_details() {
        let a = XaiAdapter::new();
        let mut delta = ChatChunkDelta {
            content: Some("answer".into()),
            reasoning_content: Some("thought".into()),
            reasoning_details: vec![serde_json::json!({"type": "reasoning.text"})],
            ..ChatChunkDelta::default()
        };
        a.shape_delta(&mut delta);
        assert!(delta.reasoning_details.is_empty());
        assert_eq!(delta.content.as_deref(), Some("answer"));
    }
}
