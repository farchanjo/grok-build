//! Anthropic-specific rate-limit response headers.
//!
//! Parsed independently from OpenAI/OpenRouter `x-ratelimit-*` diagnostics so
//! existing provider paths stay unchanged.

use serde::{Deserialize, Serialize};

/// Rate-limit snapshot from Anthropic response headers.
///
/// Header names follow the documented set:
/// `anthropic-ratelimit-{requests,tokens,input-tokens,output-tokens}-{limit,remaining,reset}`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnthropicRateLimitHeaders {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requests_limit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requests_remaining: Option<String>,
    /// RFC 3339 timestamp when the request rate limit fully replenishes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requests_reset: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_limit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_remaining: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_reset: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens_limit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens_remaining: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens_reset: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens_limit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens_remaining: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens_reset: Option<String>,
}

impl AnthropicRateLimitHeaders {
    pub fn is_empty(&self) -> bool {
        self.requests_limit.is_none()
            && self.requests_remaining.is_none()
            && self.requests_reset.is_none()
            && self.tokens_limit.is_none()
            && self.tokens_remaining.is_none()
            && self.tokens_reset.is_none()
            && self.input_tokens_limit.is_none()
            && self.input_tokens_remaining.is_none()
            && self.input_tokens_reset.is_none()
            && self.output_tokens_limit.is_none()
            && self.output_tokens_remaining.is_none()
            && self.output_tokens_reset.is_none()
    }
}
