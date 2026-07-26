//! Anthropic JSON error envelope types.

use serde::{Deserialize, Serialize};

/// Documented Anthropic error `type` values, with a forward-compatible catch-all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnthropicErrorType {
    InvalidRequestError,
    AuthenticationError,
    PermissionError,
    NotFoundError,
    RequestTooLarge,
    RateLimitError,
    ApiError,
    OverloadedError,
    /// Preserve unknown wire strings without failing deserialize.
    #[serde(untagged)]
    Unknown(String),
}

impl AnthropicErrorType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::InvalidRequestError => "invalid_request_error",
            Self::AuthenticationError => "authentication_error",
            Self::PermissionError => "permission_error",
            Self::NotFoundError => "not_found_error",
            Self::RequestTooLarge => "request_too_large",
            Self::RateLimitError => "rate_limit_error",
            Self::ApiError => "api_error",
            Self::OverloadedError => "overloaded_error",
            Self::Unknown(s) => s.as_str(),
        }
    }
}

/// Inner `{ "type", "message" }` object of an Anthropic error response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnthropicErrorObject {
    #[serde(rename = "type")]
    pub r#type: AnthropicErrorType,
    pub message: String,
}

/// Top-level Anthropic error body: `{ "type": "error", "error": { ... } }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnthropicErrorBody {
    #[serde(rename = "type")]
    pub r#type: String,
    pub error: AnthropicErrorObject,
    /// Request id when present in the body (also often in `request-id` header).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl AnthropicErrorBody {
    /// Best-effort parse from response bytes. Returns `None` when the body is
    /// not a recognized Anthropic error envelope.
    pub fn try_parse(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice(bytes).ok()
    }

    pub fn message(&self) -> &str {
        &self.error.message
    }

    pub fn error_type(&self) -> &AnthropicErrorType {
        &self.error.r#type
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_error_envelope() {
        let raw = br#"{"type":"error","error":{"type":"rate_limit_error","message":"Rate limited"},"request_id":"req_1"}"#;
        let body = AnthropicErrorBody::try_parse(raw).expect("parse");
        assert_eq!(body.r#type, "error");
        assert_eq!(body.error.r#type, AnthropicErrorType::RateLimitError);
        assert_eq!(body.error.message, "Rate limited");
        assert_eq!(body.request_id.as_deref(), Some("req_1"));
    }

    #[test]
    fn unknown_error_type_is_preserved() {
        let raw = br#"{"type":"error","error":{"type":"future_error","message":"x"}}"#;
        let body = AnthropicErrorBody::try_parse(raw).expect("parse");
        match body.error.r#type {
            AnthropicErrorType::Unknown(s) => assert_eq!(s, "future_error"),
            other => panic!("expected Unknown, got {other:?}"),
        }
    }
}
