//! Z.ai Model API adapter.
//!
//! Transcribed from today's branch sites:
//! - Z.ai wire extensions only for the Z.ai provider kind: `tool_stream`
//!   (fragmented tool argument streaming) and the `thinking` object
//!   (`type`/`clear_thinking`) (`agent/config.rs`).
//! - Z.ai keeps a plain bearer key; no first-party headers, no OpenRouter
//!   metadata/body, and it rejects the non-standard `model_id` metadata.

use super::{AuthScheme, ProviderAdapter, ProviderKind, ProviderPolicy, ReasoningEcho};
use super::{ReasoningWire, RequestContext, RequestExtensions};
use crate::route_context::RouteProviderKind;
use xai_grok_inference_types::ApiBackend;

/// Static policy for Z.ai.
pub static ZAI_PROVIDER_POLICY: ProviderPolicy = ProviderPolicy {
    route_kind: RouteProviderKind::Zai,
    backend_preference: &[ApiBackend::ChatCompletions],
    usage_policy: super::UsagePolicy::new(false, false, false),
    pacing_default: super::PacingDefault::None,
};

/// A stateless Z.ai adapter.
#[derive(Debug, Clone, Copy)]
pub struct ZaiAdapter;

impl ZaiAdapter {
    /// Construct a Z.ai adapter.
    pub fn new() -> Self {
        Self
    }
}

impl Default for ZaiAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderAdapter for ZaiAdapter {
    fn id(&self) -> ProviderKind {
        ProviderKind::Zai
    }

    fn auth(&self) -> AuthScheme {
        AuthScheme::Bearer
    }

    fn policy(&self) -> &'static ProviderPolicy {
        &ZAI_PROVIDER_POLICY
    }

    fn request_extensions(&self, cx: &RequestContext) -> RequestExtensions {
        RequestExtensions {
            // Z.ai request-body extensions (tool_stream / thinking) only when
            // the context actually carries a Z.ai knob.
            zai_body: cx.zai_tool_stream || cx.zai_thinking.is_some(),
            ..RequestExtensions::default()
        }
    }

    fn reasoning_wire(&self) -> ReasoningWire {
        ReasoningWire {
            request_key: "reasoning_content",
            echo: ReasoningEcho::Echo,
        }
    }

    fn shape_delta(&self, delta: &mut xai_grok_inference_types::ChatChunkDelta) {
        // Z.ai does not use the OpenRouter-only `reasoning_details` blocks.
        delta.reasoning_details.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_and_auth() {
        let a = ZaiAdapter::new();
        assert_eq!(a.id(), ProviderKind::Zai);
        assert_eq!(a.auth(), AuthScheme::Bearer);
    }

    #[test]
    fn policy_bundles_zai_route() {
        let policy = ZaiAdapter::new().policy();
        assert_eq!(policy.route_kind, RouteProviderKind::Zai);
        assert_eq!(policy.backend_preference, &[ApiBackend::ChatCompletions]);
        assert!(!policy.usage_policy.include_message_model_id);
        assert!(!policy.usage_policy.first_party);
        assert!(!policy.usage_policy.openrouter_metadata);
        assert_eq!(policy.pacing_default, crate::provider::PacingDefault::None);
    }

    #[test]
    fn request_extensions_enable_zai_body_only_when_knob_present() {
        let a = ZaiAdapter::new();
        let plain = RequestContext::default();
        assert!(!a.request_extensions(&plain).zai_body);

        let with_stream = RequestContext {
            zai_tool_stream: true,
            ..RequestContext::default()
        };
        assert!(a.request_extensions(&with_stream).zai_body);

        let with_thinking = RequestContext {
            zai_thinking: Some(serde_json::json!({"type": "enabled"})),
            ..RequestContext::default()
        };
        assert!(a.request_extensions(&with_thinking).zai_body);

        let ext = a.request_extensions(&with_stream);
        assert!(!ext.first_party_headers);
        assert!(!ext.openrouter_metadata_header);
        assert!(!ext.openrouter_body);
        assert!(!ext.chatgpt_oauth_headers);
        assert!(!ext.include_message_model_id);
    }

    #[test]
    fn reasoning_wire_echoes_reasoning_content() {
        let a = ZaiAdapter::new();
        assert_eq!(a.reasoning_wire().request_key, "reasoning_content");
        assert_eq!(a.reasoning_wire().echo, ReasoningEcho::Echo);
    }

    #[test]
    fn never_reports_fallback() {
        let a = ZaiAdapter::new();
        assert!(!a.detect_fallback("a", "b"));
    }
}
