//! Anthropic client errors (no API key or file bytes in Display/Debug).

use std::fmt;

use xai_grok_inference_types::InferenceError;
use xai_grok_inference_types::anthropic::{AnthropicErrorBody, AnthropicErrorType};

use super::headers::AnthropicResponseMeta;

/// Result alias for Anthropic client operations.
pub type AnthropicResult<T> = Result<T, AnthropicClientError>;

/// Classification of an Anthropic client failure for retry / UX policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorClass {
    /// 401 — permanent auth failure (bad/missing key).
    PermanentAuth,
    /// 403 — permanent permission denial.
    PermanentPermission,
    /// 400 / 413 — permanent or actionable client error.
    PermanentActionable,
    /// 429 — retryable per headers.
    RetryableRateLimit,
    /// 529 — retryable overload.
    RetryableOverload,
    /// Transient 5xx / network / transport.
    Transient,
    /// SSE error event after HTTP 200, classified by Anthropic error type.
    StreamError,
    /// Request cancelled by the caller.
    Cancelled,
    /// Local preflight (oversized body, invalid config) — never retried.
    Local,
    /// Response decode failure.
    Decode,
}

/// Anthropic-specific client error. Debug/Display never include the API key
/// or raw file bytes.
#[derive(Clone)]
pub enum AnthropicClientError {
    InvalidConfig(String),
    /// Serialized JSON body exceeds [`super::MAX_REQUEST_BYTES`] before send.
    RequestTooLarge {
        size_bytes: usize,
        limit_bytes: usize,
    },
    Http {
        status: u16,
        class: ErrorClass,
        message: String,
        error_type: Option<AnthropicErrorType>,
        /// Boxed so the error enum stays under the workspace large-error threshold.
        meta: Box<AnthropicResponseMeta>,
    },
    /// Mid-stream error after a successful HTTP 200 SSE open.
    Stream {
        error_type: String,
        message: String,
        meta: Box<AnthropicResponseMeta>,
    },
    Transport(String),
    Decode(String),
    Cancelled,
}

impl AnthropicClientError {
    pub fn class(&self) -> ErrorClass {
        match self {
            Self::InvalidConfig(_) | Self::RequestTooLarge { .. } => ErrorClass::Local,
            Self::Http { class, .. } => *class,
            Self::Stream { .. } => ErrorClass::StreamError,
            Self::Transport(_) => ErrorClass::Transient,
            Self::Decode(_) => ErrorClass::Decode,
            Self::Cancelled => ErrorClass::Cancelled,
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(
            self.class(),
            ErrorClass::RetryableRateLimit
                | ErrorClass::RetryableOverload
                | ErrorClass::Transient
                | ErrorClass::StreamError
        )
    }

    pub fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled)
    }

    pub fn meta(&self) -> Option<&AnthropicResponseMeta> {
        match self {
            Self::Http { meta, .. } | Self::Stream { meta, .. } => Some(meta.as_ref()),
            _ => None,
        }
    }

    pub fn retry_after_secs(&self) -> Option<u64> {
        self.meta().and_then(|m| m.retry_after_secs)
    }

    /// Safe single-line message (no secrets, no file bytes).
    pub fn safe_message(&self) -> String {
        match self {
            Self::InvalidConfig(m) => format!("invalid anthropic config: {m}"),
            Self::RequestTooLarge {
                size_bytes,
                limit_bytes,
            } => {
                format!("request body {size_bytes} bytes exceeds limit {limit_bytes}")
            }
            Self::Http {
                status,
                class,
                message,
                error_type,
                meta,
            } => {
                let mut out = format!("HTTP {status} ({class:?})");
                if let Some(t) = error_type {
                    out.push_str(&format!(" type={}", t.as_str()));
                }
                if let Some(id) = meta.request_id.as_ref() {
                    out.push_str(&format!(" request_id={id}"));
                }
                if !message.is_empty() {
                    out.push_str(": ");
                    out.push_str(message);
                }
                out
            }
            Self::Stream {
                error_type,
                message,
                meta,
            } => {
                let mut out = format!("stream error ({error_type}): {message}");
                if let Some(id) = meta.request_id.as_ref() {
                    out.push_str(&format!(" request_id={id}"));
                }
                out
            }
            Self::Transport(m) => format!("transport error: {m}"),
            Self::Decode(m) => format!("decode error: {m}"),
            Self::Cancelled => "cancelled".into(),
        }
    }

    pub(crate) fn from_status(status: u16, body: &[u8], meta: AnthropicResponseMeta) -> Self {
        let parsed = AnthropicErrorBody::try_parse(body);
        let error_type = parsed.as_ref().map(|b| b.error.r#type.clone());
        let message = parsed
            .as_ref()
            .map(|b| b.error.message.clone())
            .or_else(|| {
                let s = String::from_utf8_lossy(body);
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.chars().take(512).collect())
                }
            })
            .unwrap_or_else(|| status_default_message(status).into());

        let class = classify_http_status(status, error_type.as_ref());
        Self::Http {
            status,
            class,
            message,
            error_type,
            meta: Box::new(meta),
        }
    }

    /// Convert into the generic [`InferenceError`] used by retry classification.
    pub fn into_inference_error(self) -> InferenceError {
        match self {
            Self::Cancelled => InferenceError::InvalidConfiguration("request cancelled"),
            Self::InvalidConfig(m) => InferenceError::Auth(m),
            Self::RequestTooLarge {
                size_bytes,
                limit_bytes,
            } => InferenceError::Api {
                status: reqwest::StatusCode::PAYLOAD_TOO_LARGE,
                message: format!("request body {size_bytes} bytes exceeds limit {limit_bytes}"),
                model_metadata: None,
                retry_after_secs: None,
                should_retry: Some(false),
                diagnostics: None,
            },
            Self::Http {
                status,
                message,
                meta,
                class,
                ..
            } => {
                let status_code = reqwest::StatusCode::from_u16(status)
                    .unwrap_or(reqwest::StatusCode::BAD_GATEWAY);
                if class == ErrorClass::PermanentAuth {
                    return InferenceError::Auth(format!(
                        "Unauthorized ({status}) from anthropic: {message}"
                    ));
                }
                InferenceError::Api {
                    status: status_code,
                    message,
                    model_metadata: None,
                    retry_after_secs: meta.retry_after_secs,
                    should_retry: None,
                    diagnostics: meta.to_api_diagnostics(),
                }
            }
            Self::Stream {
                error_type,
                message,
                ..
            } => InferenceError::StreamError {
                error_type,
                message,
            },
            Self::Transport(m) => InferenceError::EventStreamError(m),
            Self::Decode(m) => InferenceError::serialization_message(m),
        }
    }
}

impl fmt::Debug for AnthropicClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Deliberately use safe_message so Debug never dumps keys/bytes.
        f.debug_struct("AnthropicClientError")
            .field("message", &self.safe_message())
            .field("class", &self.class())
            .finish()
    }
}

impl fmt::Display for AnthropicClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.safe_message())
    }
}

impl std::error::Error for AnthropicClientError {}

pub(crate) fn classify_http_status(
    status: u16,
    error_type: Option<&AnthropicErrorType>,
) -> ErrorClass {
    match status {
        401 => ErrorClass::PermanentAuth,
        403 => ErrorClass::PermanentPermission,
        400 | 413 | 404 | 422 => ErrorClass::PermanentActionable,
        429 => ErrorClass::RetryableRateLimit,
        529 => ErrorClass::RetryableOverload,
        s if (500..600).contains(&s) => ErrorClass::Transient,
        _ => {
            // Fall back to Anthropic error type when status is unusual.
            match error_type {
                Some(AnthropicErrorType::AuthenticationError) => ErrorClass::PermanentAuth,
                Some(AnthropicErrorType::PermissionError) => ErrorClass::PermanentPermission,
                Some(AnthropicErrorType::RateLimitError) => ErrorClass::RetryableRateLimit,
                Some(AnthropicErrorType::OverloadedError) => ErrorClass::RetryableOverload,
                Some(
                    AnthropicErrorType::InvalidRequestError
                    | AnthropicErrorType::NotFoundError
                    | AnthropicErrorType::RequestTooLarge,
                ) => ErrorClass::PermanentActionable,
                Some(AnthropicErrorType::ApiError) => ErrorClass::Transient,
                _ => ErrorClass::Transient,
            }
        }
    }
}

fn status_default_message(status: u16) -> &'static str {
    match status {
        401 => "authentication failed",
        403 => "permission denied",
        404 => "not found",
        413 => "request too large",
        429 => "rate limited",
        529 => "overloaded",
        s if (500..600).contains(&s) => "server error",
        _ => "request failed",
    }
}

/// Classify an SSE error event that arrived after HTTP 200 using Anthropic's
/// error type string.
pub(crate) fn classify_stream_error_type(error_type: &str) -> ErrorClass {
    match error_type {
        "authentication_error" => ErrorClass::PermanentAuth,
        "permission_error" => ErrorClass::PermanentPermission,
        "invalid_request_error" | "not_found_error" | "request_too_large" => {
            ErrorClass::PermanentActionable
        }
        "rate_limit_error" => ErrorClass::RetryableRateLimit,
        "overloaded_error" => ErrorClass::RetryableOverload,
        "api_error" => ErrorClass::Transient,
        _ => ErrorClass::StreamError,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_and_display_omit_obvious_secrets() {
        let err = AnthropicClientError::Http {
            status: 401,
            class: ErrorClass::PermanentAuth,
            message: "invalid x-api-key".into(),
            error_type: Some(AnthropicErrorType::AuthenticationError),
            meta: Box::new(AnthropicResponseMeta {
                request_id: Some("req_1".into()),
                ..Default::default()
            }),
        };
        let s = format!("{err:?}{err}");
        assert!(!s.contains("sk-ant-"));
        assert!(s.contains("401"));
    }

    #[test]
    fn classify_529_as_overload() {
        assert_eq!(
            classify_http_status(529, Some(&AnthropicErrorType::OverloadedError)),
            ErrorClass::RetryableOverload
        );
    }

    #[test]
    fn cancellation_is_terminal_and_not_retryable() {
        let err = AnthropicClientError::Cancelled;
        assert!(err.is_cancelled());
        assert!(!err.is_retryable());
        assert_eq!(err.class(), ErrorClass::Cancelled);
    }
}
