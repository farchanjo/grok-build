//! OpenAI-compatible adapter, parameterized by wire dialect.
//!
//! One adapter serves every OpenAI-compatible backend. The [`WireDialect`]
//! tunes the request-side reasoning key, the reasoning echo policy, and the
//! per-delta [`shape_delta`](super::ProviderAdapter::shape_delta) hook:
//!
//! - `Standard`: legacy OpenAI-compatible wire. `reasoning_content` key,
//!   echo verbatim, no delta shaping beyond clearing OpenRouter-only
//!   `reasoning_details`.
//! - `Vllm` / `Sglang`: reasoning arrives under the `reasoning` key, replayed
//!   assistant reasoning is stripped (`Strip` echo), the delta hook
//!   hoists reasoning out of `content` when a vLLM/SGLang server interleaves
//!   reasoning prose into the content field, and the configured
//!   `chat_template_kwargs` object (vLLM-native, e.g. Qwen3 `enable_thinking`)
//!   serializes into the request body.

use super::{AuthScheme, ProviderAdapter, ProviderKind, ProviderPolicy, ReasoningEcho};
use super::{RequestContext, RequestExtensions, WireDialect};
use crate::route_context::RouteProviderKind;
use xai_grok_inference_types::{ApiBackend, ChatChunkDelta};

/// Static policy for the OpenAI-compatible family.
pub static COMPATIBLE_PROVIDER_POLICY: ProviderPolicy = ProviderPolicy {
    route_kind: RouteProviderKind::OpenAiCompatible,
    backend_preference: &[ApiBackend::ChatCompletions],
    usage_policy: super::UsagePolicy::new(false, false, false),
    pacing_default: super::PacingDefault::None,
};

/// Static policy for the vLLM/SGLang dialects of the compatible family.
///
/// Identical routing to [`COMPATIBLE_PROVIDER_POLICY`], but it accepts
/// `finish_reason=length` as a truncated completion instead of a fatal
/// `MaxTokensTruncation`: open-weight servers routinely truncate long
/// generations, the turn's output has already streamed, and a re-issue would
/// duplicate it on screen.
static COMPATIBLE_VLLM_PROVIDER_POLICY: ProviderPolicy = ProviderPolicy {
    route_kind: RouteProviderKind::OpenAiCompatible,
    backend_preference: &[ApiBackend::ChatCompletions],
    usage_policy: super::UsagePolicy::with_length_truncation(true),
    pacing_default: super::PacingDefault::None,
};

/// An OpenAI-compatible adapter bound to a concrete wire dialect.
#[derive(Debug, Clone, Copy)]
pub struct OpenAiCompatibleAdapter {
    dialect: WireDialect,
}

impl OpenAiCompatibleAdapter {
    /// Construct an adapter bound to `dialect`.
    pub fn new(dialect: WireDialect) -> Self {
        Self { dialect }
    }

    /// The wire dialect this adapter serves.
    pub fn dialect(&self) -> WireDialect {
        self.dialect
    }

    /// The static policy for this adapter's dialect: the vLLM/SGLang family
    /// accepts `finish_reason=length` as a truncated completion, the
    /// standard wire keeps the historic fatal behavior.
    fn policy_for(&self) -> &'static ProviderPolicy {
        match self.dialect {
            WireDialect::Vllm | WireDialect::Sglang => &COMPATIBLE_VLLM_PROVIDER_POLICY,
            // Standard and every future dialect keep the historic fatal class.
            _ => &COMPATIBLE_PROVIDER_POLICY,
        }
    }
}

/// Hoists reasoning out of `content` into `reasoning_content` when a
/// vLLM/SGLang delta carries both in the same object.
///
/// Some vLLM/SGLang builds emit reasoning prose in `content` alongside a
/// separate `reasoning` field (already folded into `reasoning_content` by the
/// lenient deserializer). Without this hoist, that prose would be emitted as
/// assistant text instead of a reasoning token. The transformation keeps only
/// the final assistant prose in `content` and moves the reasoning into the
/// `reasoning_content` channel.
pub(crate) fn hoist_reasoning(delta: &mut ChatChunkDelta) {
    if delta.reasoning_content.is_none() {
        return;
    }
    let Some(content) = delta.content.take() else {
        return;
    };
    if content.is_empty() {
        return;
    }
    let mut merged = delta.reasoning_content.take().unwrap_or_default();
    if !merged.is_empty() {
        merged.push(' ');
    }
    merged.push_str(&content);
    delta.reasoning_content = Some(merged);
}

impl ProviderAdapter for OpenAiCompatibleAdapter {
    fn id(&self) -> ProviderKind {
        ProviderKind::OpenAiCompatible
    }

    fn auth(&self) -> AuthScheme {
        // OpenAI-compatible endpoints use a plain API-key bearer by default.
        AuthScheme::Bearer
    }

    fn policy(&self) -> &'static ProviderPolicy {
        self.policy_for()
    }

    fn request_extensions(&self, cx: &RequestContext) -> RequestExtensions {
        // Compatible family: no first-party headers, no OpenRouter metadata,
        // no Z.ai body, and OpenAI-compatible third parties reject `model_id`.
        // `chat_template_kwargs` is vLLM/SGLang-only (the standard
        // OpenAI-compatible wire has no such field and strict servers may
        // reject unknown keys).
        RequestExtensions {
            vllm_body: self.dialect.is_vllm_family()
                && cx
                    .vllm_chat_template_kwargs
                    .as_ref()
                    .is_some_and(|v| v.is_object() && !v.as_object().is_some_and(|o| o.is_empty())),
            ..RequestExtensions::default()
        }
    }

    fn vllm_chat_template_kwargs<'a>(
        &self,
        configured: Option<&'a serde_json::Value>,
    ) -> Option<&'a serde_json::Value> {
        // Dialect gate: only a vLLM-family dialect serializes the object, and
        // only when it is a non-empty JSON object (an empty or non-object
        // value would be noise or a 400 on strict servers).
        self.dialect
            .is_vllm_family()
            .then(|| configured.filter(|v| v.as_object().is_some_and(|o| !o.is_empty())))
            .flatten()
    }

    fn reasoning_wire(&self) -> super::ReasoningWire {
        if self.dialect.uses_reasoning_key() {
            super::ReasoningWire {
                request_key: "reasoning",
                echo: ReasoningEcho::Strip,
            }
        } else {
            super::ReasoningWire {
                request_key: "reasoning_content",
                echo: ReasoningEcho::Echo,
            }
        }
    }

    fn shape_delta(&self, delta: &mut ChatChunkDelta) {
        // vLLM/SGLang: hoist interleaved reasoning prose into the reasoning
        // channel. Standard: leave `content`/`reasoning_content` untouched.
        if self.dialect.uses_reasoning_key() {
            hoist_reasoning(delta);
        }
        // Non-OpenRouter providers never emit/accumulate the OpenRouter-only
        // `reasoning_details` blocks — clear them so the codec does not
        // accumulate foreign detail blocks.
        delta.reasoning_details.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn standard() -> OpenAiCompatibleAdapter {
        OpenAiCompatibleAdapter::new(WireDialect::Standard)
    }

    fn vllm() -> OpenAiCompatibleAdapter {
        OpenAiCompatibleAdapter::new(WireDialect::Vllm)
    }

    #[test]
    fn id_auth_and_dialect() {
        let a = vllm();
        assert_eq!(a.id(), ProviderKind::OpenAiCompatible);
        assert_eq!(a.auth(), AuthScheme::Bearer);
        assert_eq!(a.dialect(), WireDialect::Vllm);
    }

    #[test]
    fn policy_bundles_compatible_route() {
        let policy = OpenAiCompatibleAdapter::new(WireDialect::Standard).policy();
        assert_eq!(policy.route_kind, RouteProviderKind::OpenAiCompatible);
        assert_eq!(policy.backend_preference, &[ApiBackend::ChatCompletions]);
        assert!(!policy.usage_policy.include_message_model_id);
        assert!(!policy.usage_policy.first_party);
        assert!(!policy.usage_policy.openrouter_metadata);
        assert_eq!(policy.pacing_default, crate::provider::PacingDefault::None);
    }

    #[test]
    fn reasoning_wire_depends_on_dialect() {
        // Standard keeps the legacy key + echo.
        let wire = standard().reasoning_wire();
        assert_eq!(wire.request_key, "reasoning_content");
        assert_eq!(wire.echo, ReasoningEcho::Echo);

        // vLLM/SGLang use the `reasoning` key and strip replayed reasoning.
        for dialect in [WireDialect::Vllm, WireDialect::Sglang] {
            let wire = OpenAiCompatibleAdapter::new(dialect).reasoning_wire();
            assert_eq!(wire.request_key, "reasoning");
            assert_eq!(wire.echo, ReasoningEcho::Strip);
        }
    }

    #[test]
    fn vllm_dialect_policy_accepts_length_truncation() {
        // The vLLM/SGLang dialects accept finish_reason=length as a truncated
        // completion; the standard wire keeps the historic fatal class.
        assert!(
            OpenAiCompatibleAdapter::new(WireDialect::Vllm)
                .policy()
                .usage_policy
                .accept_length_truncation
        );
        assert!(
            OpenAiCompatibleAdapter::new(WireDialect::Sglang)
                .policy()
                .usage_policy
                .accept_length_truncation
        );
        assert!(
            !OpenAiCompatibleAdapter::new(WireDialect::Standard)
                .policy()
                .usage_policy
                .accept_length_truncation
        );
        // Both dialect policies keep the same route and backend preference.
        for dialect in [
            WireDialect::Standard,
            WireDialect::Vllm,
            WireDialect::Sglang,
        ] {
            let policy = OpenAiCompatibleAdapter::new(dialect).policy();
            assert_eq!(policy.route_kind, RouteProviderKind::OpenAiCompatible);
            assert_eq!(policy.backend_preference, &[ApiBackend::ChatCompletions]);
        }
    }

    #[test]
    fn chat_template_kwargs_gate() {
        let kwargs = serde_json::json!({"enable_thinking": false});
        let empty = serde_json::json!({});
        let non_object = serde_json::json!(42);

        // vLLM/SGLang dialects pass a non-empty object through.
        for dialect in [WireDialect::Vllm, WireDialect::Sglang] {
            let a = OpenAiCompatibleAdapter::new(dialect);
            assert_eq!(a.vllm_chat_template_kwargs(Some(&kwargs)), Some(&kwargs));
            assert_eq!(a.vllm_chat_template_kwargs(Some(&empty)), None);
            assert_eq!(a.vllm_chat_template_kwargs(Some(&non_object)), None);
            assert_eq!(a.vllm_chat_template_kwargs(None), None);
        }

        // Standard wire never emits the key.
        let a = standard();
        assert_eq!(a.vllm_chat_template_kwargs(Some(&kwargs)), None);
    }

    #[test]
    fn request_extensions_flag_vllm_body_only_for_family_and_non_empty_object() {
        let kwargs = serde_json::json!({"enable_thinking": false});
        let cx = |kwargs: Option<serde_json::Value>| RequestContext {
            vllm_chat_template_kwargs: kwargs,
            ..RequestContext::default()
        };
        assert!(
            OpenAiCompatibleAdapter::new(WireDialect::Vllm)
                .request_extensions(&cx(Some(kwargs.clone())))
                .vllm_body
        );
        assert!(
            !OpenAiCompatibleAdapter::new(WireDialect::Vllm)
                .request_extensions(&cx(Some(serde_json::json!({}))))
                .vllm_body
        );
        assert!(
            !OpenAiCompatibleAdapter::new(WireDialect::Vllm)
                .request_extensions(&cx(None))
                .vllm_body
        );
        assert!(
            !OpenAiCompatibleAdapter::new(WireDialect::Standard)
                .request_extensions(&cx(Some(kwargs)))
                .vllm_body
        );
    }

    #[test]
    fn vllm_hoists_content_reasoning_and_clears_details() {
        let mut delta = ChatChunkDelta {
            content: Some("deep thought".into()),
            reasoning_content: Some("step 1".into()),
            reasoning_details: vec![serde_json::json!({"type": "reasoning.text"})],
            ..ChatChunkDelta::default()
        };
        vllm().shape_delta(&mut delta);
        assert_eq!(delta.content, None);
        assert_eq!(
            delta.reasoning_content.as_deref(),
            Some("step 1 deep thought")
        );
        assert!(delta.reasoning_details.is_empty(), "details cleared");
    }

    #[test]
    fn vllm_leaves_content_alone_when_no_reasoning_channel() {
        let mut delta = ChatChunkDelta {
            content: Some("assistant answer".into()),
            reasoning_content: None,
            ..ChatChunkDelta::default()
        };
        vllm().shape_delta(&mut delta);
        // No reasoning channel present → content is untouched.
        assert_eq!(delta.content.as_deref(), Some("assistant answer"));
        assert_eq!(delta.reasoning_content, None);
    }

    #[test]
    fn standard_clears_openrouter_only_details_but_keeps_content() {
        let mut delta = ChatChunkDelta {
            content: Some("answer".into()),
            reasoning_content: Some("thought".into()),
            reasoning_details: vec![serde_json::json!({"type": "reasoning.text"})],
            ..ChatChunkDelta::default()
        };
        standard().shape_delta(&mut delta);
        assert_eq!(delta.content.as_deref(), Some("answer"));
        assert_eq!(delta.reasoning_content.as_deref(), Some("thought"));
        assert!(delta.reasoning_details.is_empty());
    }

    #[test]
    fn never_reports_fallback() {
        assert!(!standard().detect_fallback("a", "b"));
    }
}
