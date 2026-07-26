//! Anthropic request/response header helpers.

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use xai_grok_inference_types::ApiErrorDiagnostics;
use xai_grok_inference_types::anthropic::{
    ANTHROPIC_VERSION, AnthropicBetaSet, AnthropicRateLimitHeaders,
};

use super::error::AnthropicClientError;

/// Metadata extracted from every Anthropic response (success or error).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AnthropicResponseMeta {
    pub request_id: Option<String>,
    pub retry_after_secs: Option<u64>,
    pub rate_limit: AnthropicRateLimitHeaders,
}

impl AnthropicResponseMeta {
    pub fn from_headers(headers: &HeaderMap) -> Self {
        Self {
            request_id: header_string(headers, "request-id")
                .or_else(|| header_string(headers, "x-request-id")),
            retry_after_secs: extract_retry_after(headers),
            rate_limit: parse_anthropic_rate_limit_headers(headers),
        }
    }

    /// Map into generic diagnostics without clobbering OpenAI/OpenRouter fields
    /// when those paths never set Anthropic headers.
    pub fn to_api_diagnostics(&self) -> Option<ApiErrorDiagnostics> {
        let mut d = ApiErrorDiagnostics::default();
        // Prefer request-scoped rate remaining when present.
        if let Some(rem) = self
            .rate_limit
            .requests_remaining
            .clone()
            .or_else(|| self.rate_limit.tokens_remaining.clone())
        {
            d.rate_limit_remaining = Some(rem);
        }
        if let Some(lim) = self
            .rate_limit
            .requests_limit
            .clone()
            .or_else(|| self.rate_limit.tokens_limit.clone())
        {
            d.rate_limit_limit = Some(lim);
        }
        if let Some(reset) = self
            .rate_limit
            .requests_reset
            .clone()
            .or_else(|| self.rate_limit.tokens_reset.clone())
        {
            d.rate_limit_reset = Some(reset);
        }
        d.generation_id = self.request_id.clone();
        if d.is_empty() && self.request_id.is_none() {
            None
        } else {
            // Keep generation_id even when rate headers absent.
            if d.is_empty() {
                d.generation_id = self.request_id.clone();
            }
            Some(d)
        }
    }
}

/// Parse documented Anthropic rate-limit headers only. Does not touch
/// `x-ratelimit-*` (OpenAI/OpenRouter).
pub fn parse_anthropic_rate_limit_headers(headers: &HeaderMap) -> AnthropicRateLimitHeaders {
    AnthropicRateLimitHeaders {
        requests_limit: header_string(headers, "anthropic-ratelimit-requests-limit"),
        requests_remaining: header_string(headers, "anthropic-ratelimit-requests-remaining"),
        requests_reset: header_string(headers, "anthropic-ratelimit-requests-reset"),
        tokens_limit: header_string(headers, "anthropic-ratelimit-tokens-limit"),
        tokens_remaining: header_string(headers, "anthropic-ratelimit-tokens-remaining"),
        tokens_reset: header_string(headers, "anthropic-ratelimit-tokens-reset"),
        input_tokens_limit: header_string(headers, "anthropic-ratelimit-input-tokens-limit"),
        input_tokens_remaining: header_string(
            headers,
            "anthropic-ratelimit-input-tokens-remaining",
        ),
        input_tokens_reset: header_string(headers, "anthropic-ratelimit-input-tokens-reset"),
        output_tokens_limit: header_string(headers, "anthropic-ratelimit-output-tokens-limit"),
        output_tokens_remaining: header_string(
            headers,
            "anthropic-ratelimit-output-tokens-remaining",
        ),
        output_tokens_reset: header_string(headers, "anthropic-ratelimit-output-tokens-reset"),
    }
}

const MAX_HEADER_CHARS: usize = 256;

fn header_string(headers: &HeaderMap, name: &str) -> Option<String> {
    let value = headers.get(name)?.to_str().ok()?.trim();
    if value.is_empty() {
        return None;
    }
    Some(value.chars().take(MAX_HEADER_CHARS).collect())
}

fn extract_retry_after(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .map(|s| s.min(120))
}

/// Build the fixed Anthropic identity headers for a request.
///
/// Always sets:
/// - `x-api-key` (never `Authorization`)
/// - `anthropic-version: 2023-06-01`
/// - optional `anthropic-beta` from the explicit set
pub(crate) fn build_request_headers(
    api_key: &str,
    betas: &AnthropicBetaSet,
) -> Result<HeaderMap, AnthropicClientError> {
    let mut headers = HeaderMap::new();
    let key = HeaderValue::from_str(api_key).map_err(|_| {
        AnthropicClientError::InvalidConfig(
            "api key cannot be converted to a valid HTTP header".into(),
        )
    })?;
    headers.insert(HeaderName::from_static("x-api-key"), key);
    headers.insert(
        HeaderName::from_static("anthropic-version"),
        HeaderValue::from_static(ANTHROPIC_VERSION),
    );
    if let Some(beta_val) = betas.header_value() {
        let hv = HeaderValue::from_str(&beta_val).map_err(|_| {
            AnthropicClientError::InvalidConfig("anthropic-beta header value is invalid".into())
        })?;
        headers.insert(HeaderName::from_static("anthropic-beta"), hv);
    }
    Ok(headers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rate_headers_ignores_openai_style() {
        let mut headers = HeaderMap::new();
        headers.insert("x-ratelimit-remaining", HeaderValue::from_static("1"));
        headers.insert(
            "anthropic-ratelimit-requests-remaining",
            HeaderValue::from_static("42"),
        );
        headers.insert(
            "anthropic-ratelimit-requests-reset",
            HeaderValue::from_static("2026-01-01T00:00:00Z"),
        );
        let rl = parse_anthropic_rate_limit_headers(&headers);
        assert_eq!(rl.requests_remaining.as_deref(), Some("42"));
        assert_eq!(rl.requests_reset.as_deref(), Some("2026-01-01T00:00:00Z"));
        // OpenAI header must not be absorbed into Anthropic struct.
        assert!(rl.tokens_remaining.is_none());
    }

    #[test]
    fn build_headers_uses_x_api_key_not_authorization() {
        let headers = build_request_headers("test-key", &AnthropicBetaSet::new()).unwrap();
        assert_eq!(
            headers.get("x-api-key").and_then(|v| v.to_str().ok()),
            Some("test-key")
        );
        assert!(headers.get(reqwest::header::AUTHORIZATION).is_none());
        assert_eq!(
            headers
                .get("anthropic-version")
                .and_then(|v| v.to_str().ok()),
            Some(ANTHROPIC_VERSION)
        );
        assert!(headers.get("anthropic-beta").is_none());
    }

    #[test]
    fn build_headers_includes_explicit_betas_only() {
        let mut betas = AnthropicBetaSet::new();
        betas.insert("prompt-caching-2024-07-31");
        let headers = build_request_headers("k", &betas).unwrap();
        assert_eq!(
            headers.get("anthropic-beta").and_then(|v| v.to_str().ok()),
            Some("prompt-caching-2024-07-31")
        );
    }
}
