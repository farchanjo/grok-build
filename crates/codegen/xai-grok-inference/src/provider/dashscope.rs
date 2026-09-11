//! Alibaba DashScope / Model Studio adapter.
//!
//! DashScope's `compatible-mode/v1` endpoint speaks the OpenAI-compatible
//! chat wire: reasoning streams in `delta.reasoning_content` (standard
//! dialect) and tool calls use the canonical OpenAI shapes. It carries its
//! own provider kind for the Qwen3 thinking extensions:
//! - `enable_thinking` (`bool`): turns the hybrid-thinking mode of Qwen3
//!   models on/off per request (mirrors the OpenAI SDK `extra_body` field).
//! - `thinking_budget` (`u32`): caps the reasoning-phase tokens; on budget
//!   exhaustion the model stops thinking and answers.
//!
//! Both serialize only when the identity is DashScope, mirroring the Z.ai
//! `tool_stream`/`thinking` pattern. Plain bearer key; no first-party
//! headers, no OpenRouter metadata/body, and it rejects the non-standard
//! `model_id` metadata.

use super::{AuthScheme, ProviderAdapter, ProviderKind, ProviderPolicy, ReasoningEcho};
use super::{ReasoningWire, RequestContext, RequestExtensions};
use crate::route_context::RouteProviderKind;
use xai_grok_inference_types::ApiBackend;

/// Static policy for DashScope.
pub static DASHSCOPE_PROVIDER_POLICY: ProviderPolicy = ProviderPolicy {
    route_kind: RouteProviderKind::DashScope,
    backend_preference: &[ApiBackend::ChatCompletions],
    usage_policy: super::UsagePolicy::new(false, false, false),
    pacing_default: super::PacingDefault::None,
};

/// A stateless DashScope adapter.
#[derive(Debug, Clone, Copy)]
pub struct DashScopeAdapter;

impl DashScopeAdapter {
    /// Construct a DashScope adapter.
    pub fn new() -> Self {
        Self
    }
}

impl Default for DashScopeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderAdapter for DashScopeAdapter {
    fn id(&self) -> ProviderKind {
        ProviderKind::DashScope
    }

    fn auth(&self) -> AuthScheme {
        AuthScheme::Bearer
    }

    fn policy(&self) -> &'static ProviderPolicy {
        &DASHSCOPE_PROVIDER_POLICY
    }

    fn request_extensions(&self, cx: &RequestContext) -> RequestExtensions {
        RequestExtensions {
            // DashScope request-body extensions (enable_thinking /
            // thinking_budget) only when the context actually carries a
            // DashScope knob.
            dashscope_body: cx.dashscope_enable_thinking.is_some()
                || cx.dashscope_thinking_budget.is_some(),
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
        // DashScope does not use the OpenRouter-only `reasoning_details` blocks.
        delta.reasoning_details.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_and_auth() {
        let a = DashScopeAdapter::new();
        assert_eq!(a.id(), ProviderKind::DashScope);
        assert_eq!(a.auth(), AuthScheme::Bearer);
    }

    #[test]
    fn policy_bundles_dashscope_route() {
        let policy = DashScopeAdapter::new().policy();
        assert_eq!(policy.route_kind, RouteProviderKind::DashScope);
        assert_eq!(policy.backend_preference, &[ApiBackend::ChatCompletions]);
        assert!(!policy.usage_policy.include_message_model_id);
        assert!(!policy.usage_policy.first_party);
        assert!(!policy.usage_policy.openrouter_metadata);
        assert_eq!(policy.pacing_default, crate::provider::PacingDefault::None);
    }

    #[test]
    fn request_extensions_enable_dashscope_body_only_when_knob_present() {
        let a = DashScopeAdapter::new();
        let plain = RequestContext::default();
        assert!(!a.request_extensions(&plain).dashscope_body);

        let with_toggle = RequestContext {
            dashscope_enable_thinking: Some(true),
            ..RequestContext::default()
        };
        assert!(a.request_extensions(&with_toggle).dashscope_body);

        // An explicit `false` is still a knob: the toggle must reach the wire.
        let with_off = RequestContext {
            dashscope_enable_thinking: Some(false),
            ..RequestContext::default()
        };
        assert!(a.request_extensions(&with_off).dashscope_body);

        let with_budget = RequestContext {
            dashscope_thinking_budget: Some(4096),
            ..RequestContext::default()
        };
        assert!(a.request_extensions(&with_budget).dashscope_body);

        let ext = a.request_extensions(&with_toggle);
        assert!(!ext.first_party_headers);
        assert!(!ext.openrouter_metadata_header);
        assert!(!ext.openrouter_body);
        assert!(!ext.zai_body);
        assert!(!ext.chatgpt_oauth_headers);
        assert!(!ext.include_message_model_id);
    }

    #[test]
    fn reasoning_wire_echoes_reasoning_content() {
        let a = DashScopeAdapter::new();
        assert_eq!(a.reasoning_wire().request_key, "reasoning_content");
        assert_eq!(a.reasoning_wire().echo, ReasoningEcho::Echo);
    }

    #[test]
    fn never_reports_fallback() {
        let a = DashScopeAdapter::new();
        assert!(!a.detect_fallback("a", "b"));
    }
}
