//! OpenAI adapter (API key or ChatGPT subscription OAuth).
//!
//! Transcribed from today's branch sites:
//! - OpenAI never receives first-party `x-grok-*` headers and rejects the
//!   non-standard `messages[].model_id` metadata
//!   (`include_message_model_id = false`).
//! - ChatGPT subscription OAuth (Codex) adds OAuth wire headers when the base
//!   URL is the Chatgpt backend (`client.rs` / `agent/config.rs`).
//! - Generic rate-limit threshold (no OpenRouter env override).

use super::{
    AuthScheme, ProviderAdapter, ProviderKind, ProviderPolicy, RequestContext,
    RequestExtensions, ReasoningEcho, ReasoningWire,
};
use crate::route_context::RouteProviderKind;
use xai_grok_inference_types::ApiBackend;

/// Static policy for OpenAI.
pub static OPENAI_PROVIDER_POLICY: ProviderPolicy = ProviderPolicy {
    route_kind: RouteProviderKind::OpenAi,
    backend_preference: &[ApiBackend::Responses, ApiBackend::ChatCompletions],
    usage_policy: super::UsagePolicy::new(false, false, false),
    pacing_default: super::PacingDefault::None,
};

/// A stateless OpenAI adapter.
#[derive(Debug, Clone, Copy)]
pub struct OpenAiAdapter;

impl OpenAiAdapter {
    /// Construct an OpenAI adapter.
    pub fn new() -> Self {
        Self
    }
}

impl Default for OpenAiAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderAdapter for OpenAiAdapter {
    fn id(&self) -> ProviderKind {
        ProviderKind::OpenAi
    }

    fn auth(&self) -> AuthScheme {
        AuthScheme::Bearer
    }

    fn policy(&self) -> &'static ProviderPolicy {
        &OPENAI_PROVIDER_POLICY
    }

    fn request_extensions(&self, cx: &RequestContext) -> RequestExtensions {
        // ChatGPT subscription OAuth (Codex) carries Codex/OpenAI wire
        // headers, not Grok-branded. No first-party, no OpenRouter, no Z.ai.
        let chatgpt_oauth = cx.base_url.contains("chatgpt.com/backend-api/codex");
        RequestExtensions {
            chatgpt_oauth_headers: chatgpt_oauth,
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
        // OpenAI does not use the OpenRouter-only `reasoning_details` blocks;
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
        let a = OpenAiAdapter::new();
        assert_eq!(a.id(), ProviderKind::OpenAi);
        assert_eq!(a.auth(), AuthScheme::Bearer);
    }

    #[test]
    fn policy_bundles_openai_route() {
        let policy = OpenAiAdapter::new().policy();
        assert_eq!(policy.route_kind, RouteProviderKind::OpenAi);
        assert_eq!(
            policy.backend_preference,
            &[ApiBackend::Responses, ApiBackend::ChatCompletions]
        );
        assert!(!policy.usage_policy.include_message_model_id);
        assert!(!policy.usage_policy.first_party);
        assert!(!policy.usage_policy.openrouter_metadata);
        assert_eq!(policy.pacing_default, crate::provider::PacingDefault::None);
    }

    #[test]
    fn request_extensions_add_chatgpt_oauth_headers_for_codex_base_url() {
        let a = OpenAiAdapter::new();
        let codex = RequestContext {
            base_url: "https://chatgpt.com/backend-api/codex".into(),
            ..RequestContext::default()
        };
        let ext = a.request_extensions(&codex);
        assert!(ext.chatgpt_oauth_headers);

        let plain = RequestContext {
            base_url: "https://api.openai.com/v1".into(),
            ..RequestContext::default()
        };
        let ext = a.request_extensions(&plain);
        assert!(!ext.chatgpt_oauth_headers);
        assert!(!ext.first_party_headers);
        assert!(!ext.openrouter_metadata_header);
        assert!(!ext.openrouter_body);
        assert!(!ext.zai_body);
    }

    #[test]
    fn reasoning_wire_echoes_reasoning_content() {
        let a = OpenAiAdapter::new();
        assert_eq!(a.reasoning_wire().request_key, "reasoning_content");
        assert_eq!(a.reasoning_wire().echo, ReasoningEcho::Echo);
    }

    #[test]
    fn never_reports_fallback() {
        let a = OpenAiAdapter::new();
        assert!(!a.detect_fallback("gpt-4", "gpt-5"));
    }
}
