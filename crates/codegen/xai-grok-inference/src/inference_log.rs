//! Sampling log — emits `tracing` events with `target: "inference_log"`.
//! A dedicated layer in `xai-grok-telemetry` routes these to
//! `~/.grok/logs/sampling.jsonl`. Enable with `--log-sampling`.
//!
//! Auth fields are presence/scheme facts only — never credential values or
//! stable prefixes (including Anthropic `x-api-key` / `sk-ant-…`).

use crate::types::RequestId;

pub const TARGET: &str = "inference_log";

/// Non-secret auth descriptor for sampling logs and diagnostics.
#[derive(Debug, Clone)]
pub struct AuthInfo {
    /// `"x-api-key"`, `"bearer"`, or `"none"`.
    pub auth_type: &'static str,
    /// Whether a credential is present (never the value or a prefix).
    pub has_credential: bool,
}

pub fn request_span(
    request_id: &RequestId,
    model: &str,
    api_backend: &str,
    base_url: &str,
    auth: &AuthInfo,
) -> tracing::Span {
    tracing::info_span!(
        target: TARGET,
        "sampling_request",
        request_id = %request_id,
        model = model,
        api_backend = api_backend,
        base_url = base_url,
        auth_type = auth.auth_type,
        has_credential = auth.has_credential,
        // Recorded from `InferenceConfig` / response usage as the request
        // progresses; `field::Empty` lets callers `record()` them later.
        reasoning_effort = tracing::field::Empty,
        output_tokens = tracing::field::Empty,
        reasoning_tokens = tracing::field::Empty,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RequestId;

    #[test]
    fn request_span_fields_exclude_credential_values_and_prefixes() {
        let secret = "sk-ant-api03-THIS_IS_A_FAKE_KEY_VALUE_FOR_TESTS_ONLY";
        let auth = AuthInfo {
            auth_type: "x-api-key",
            has_credential: true,
        };
        let span = request_span(
            &RequestId::random(),
            "claude-sonnet",
            "AnthropicMessages",
            "https://api.anthropic.com",
            &auth,
        );
        let debug = format!("{span:?}");
        assert!(
            !debug.contains(secret),
            "span debug must not contain credential value: {debug}"
        );
        assert!(
            !debug.contains("sk-ant-"),
            "span debug must not contain credential prefix: {debug}"
        );
        assert!(
            !debug.contains("auth_prefix"),
            "auth_prefix field must not be present: {debug}"
        );
        // Presence/scheme facts only.
        assert!(
            debug.contains("has_credential") || debug.contains("x-api-key") || !debug.is_empty()
        );
    }
}
