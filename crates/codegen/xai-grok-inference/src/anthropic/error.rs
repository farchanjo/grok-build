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
    /// SSE error event after HTTP 200 with unknown / generic type (retryable).
    StreamError,
    /// Request cancelled by the caller.
    Cancelled,
    /// Local preflight (oversized body, invalid config) — never retried.
    Local,
    /// Response decode failure.
    Decode,
}

impl ErrorClass {
    pub fn is_retryable(self) -> bool {
        matches!(
            self,
            Self::RetryableRateLimit
                | Self::RetryableOverload
                | Self::Transient
                | Self::StreamError
        )
    }
}

/// Anthropic-specific client error. Debug/Display never include the API key
/// or raw file bytes.
#[derive(Clone)]
pub enum AnthropicClientError {
    InvalidConfig(String),
    /// Serialized JSON body exceeds the request size limit before send.
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
        /// Classification from [`classify_stream_error_type`].
        class: ErrorClass,
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
            Self::Stream { class, .. } => *class,
            Self::Transport(_) => ErrorClass::Transient,
            Self::Decode(_) => ErrorClass::Decode,
            Self::Cancelled => ErrorClass::Cancelled,
        }
    }

    pub fn is_retryable(&self) -> bool {
        self.class().is_retryable()
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
                class,
                meta,
            } => {
                let mut out = format!("stream error ({error_type}/{class:?}): {message}");
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

    pub(crate) fn stream_error(
        error_type: impl Into<String>,
        message: impl Into<String>,
        meta: AnthropicResponseMeta,
    ) -> Self {
        let error_type = error_type.into();
        let class = classify_stream_error_type(&error_type);
        Self::Stream {
            error_type,
            message: message.into(),
            class,
            meta: Box::new(meta),
        }
    }

    /// Convert into the generic [`InferenceError`] used by retry classification.
    ///
    /// Local preflight failures and permanent client errors map to **non-auth**,
    /// **non-413-image-strip** fatals. Rate-limit / overload preserve status so
    /// [`crate::retry::classify_error`] applies the correct backoff path.
    pub fn into_inference_error(self) -> InferenceError {
        match self {
            Self::Cancelled => InferenceError::InvalidConfiguration("request cancelled"),
            // Never Auth: config mistakes must not trigger reauthentication.
            Self::InvalidConfig(m) => InferenceError::Api {
                status: reqwest::StatusCode::BAD_REQUEST,
                message: format!("invalid anthropic config: {m}"),
                model_metadata: None,
                retry_after_secs: None,
                should_retry: Some(false),
                diagnostics: None,
                error_code: None,
            },
            // Never 413: generic 413 triggers RetryWithImageStrip. Local size
            // preflight is permanent and unrelated to inline images.
            Self::RequestTooLarge {
                size_bytes,
                limit_bytes,
            } => InferenceError::Api {
                status: reqwest::StatusCode::BAD_REQUEST,
                message: format!("request body {size_bytes} bytes exceeds limit {limit_bytes}"),
                model_metadata: None,
                retry_after_secs: None,
                should_retry: Some(false),
                diagnostics: None,
                error_code: None,
            },
            Self::Http {
                status,
                message,
                meta,
                class,
                ..
            } => bridge_http_to_inference(status, class, message, meta),
            Self::Stream {
                error_type,
                message,
                class,
                meta,
            } => bridge_stream_to_inference(&error_type, class, message, meta),
            Self::Transport(m) => InferenceError::EventStreamError(m),
            Self::Decode(m) => InferenceError::serialization_message(m),
        }
    }
}

fn bridge_http_to_inference(
    status: u16,
    class: ErrorClass,
    message: String,
    meta: Box<AnthropicResponseMeta>,
) -> InferenceError {
    // Permanent auth stays on the Auth variant so session reauth can run.
    if class == ErrorClass::PermanentAuth || status == 401 {
        return InferenceError::Auth {
            message: format!("Unauthorized ({status}) from anthropic: {message}"),
            credential: xai_grok_inference_types::SentCredential::Sent,
        };
    }

    // Anthropic HTTP 413 is PermanentActionable at this layer, but a literal
    // 413 InferenceError is treated as RetryWithImageStrip by the generic
    // sampler retry path (OpenAI-centric image recovery). Remap to 400 so
    // classify_error is Fatal while keeping the actionable message.
    let (status_code, should_retry) = if status == 413 {
        (reqwest::StatusCode::BAD_REQUEST, Some(false))
    } else if matches!(
        class,
        ErrorClass::PermanentActionable
            | ErrorClass::PermanentPermission
            | ErrorClass::Local
            | ErrorClass::Decode
    ) {
        (
            reqwest::StatusCode::from_u16(status).unwrap_or(reqwest::StatusCode::BAD_REQUEST),
            Some(false),
        )
    } else {
        (
            reqwest::StatusCode::from_u16(status).unwrap_or(reqwest::StatusCode::BAD_GATEWAY),
            None,
        )
    };

    let message = if status == 413 && !message.to_ascii_lowercase().contains("too large") {
        format!("request too large (HTTP 413): {message}")
    } else {
        message
    };

    InferenceError::Api {
        status: status_code,
        message,
        model_metadata: None,
        retry_after_secs: meta.retry_after_secs,
        should_retry,
        diagnostics: meta.to_api_diagnostics(),
        error_code: None,
    }
}

fn bridge_stream_to_inference(
    error_type: &str,
    class: ErrorClass,
    message: String,
    meta: Box<AnthropicResponseMeta>,
) -> InferenceError {
    match class {
        ErrorClass::PermanentAuth => InferenceError::Auth {
            message: format!("stream authentication_error from anthropic: {message}"),
            credential: xai_grok_inference_types::SentCredential::Sent,
        },
        ErrorClass::PermanentPermission => InferenceError::Api {
            status: reqwest::StatusCode::FORBIDDEN,
            message: format!("{error_type}: {message}"),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: Some(false),
            diagnostics: meta.to_api_diagnostics(),
            error_code: None,
        },
        ErrorClass::PermanentActionable | ErrorClass::Local | ErrorClass::Decode => {
            InferenceError::Api {
                status: reqwest::StatusCode::BAD_REQUEST,
                message: format!("{error_type}: {message}"),
                model_metadata: None,
                retry_after_secs: None,
                should_retry: Some(false),
                diagnostics: meta.to_api_diagnostics(),
                error_code: None,
            }
        }
        ErrorClass::RetryableRateLimit => InferenceError::Api {
            status: reqwest::StatusCode::TOO_MANY_REQUESTS,
            message: format!("{error_type}: {message}"),
            model_metadata: None,
            retry_after_secs: meta.retry_after_secs,
            should_retry: None,
            diagnostics: meta.to_api_diagnostics(),
            error_code: None,
        },
        ErrorClass::RetryableOverload => InferenceError::Api {
            status: reqwest::StatusCode::from_u16(529)
                .unwrap_or(reqwest::StatusCode::SERVICE_UNAVAILABLE),
            message: format!("{error_type}: {message}"),
            model_metadata: None,
            retry_after_secs: meta.retry_after_secs,
            should_retry: None,
            diagnostics: meta.to_api_diagnostics(),
            error_code: None,
        },
        ErrorClass::Transient => InferenceError::Api {
            status: reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("{error_type}: {message}"),
            model_metadata: None,
            retry_after_secs: meta.retry_after_secs,
            should_retry: None,
            diagnostics: meta.to_api_diagnostics(),
            error_code: None,
        },
        // Generic stream errors remain StreamError (retryable transport-class).
        ErrorClass::StreamError | ErrorClass::Cancelled => InferenceError::StreamError {
            error_type: error_type.to_string(),
            message,
            code: None,
        },
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
    use crate::retry::{RATE_LIMIT_RETRY_THRESHOLD, RetryDecision, classify_error};

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

    #[test]
    fn request_too_large_bridge_is_fatal_not_image_strip_not_auth() {
        let err = AnthropicClientError::RequestTooLarge {
            size_bytes: 33 * 1024 * 1024,
            limit_bytes: 32 * 1024 * 1024,
        };
        let inf = err.into_inference_error();
        assert!(!inf.is_auth_error(), "must not trigger reauth");
        assert!(
            !inf.is_payload_too_large(),
            "must not use 413 image-strip path"
        );
        assert!(!inf.is_retryable());
        assert!(
            inf.to_string().contains("exceeds limit"),
            "actionable message preserved: {inf}"
        );
        match classify_error(&inf, 0, 15, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::Fatal(_) => {}
            other => panic!("expected Fatal, got {other:?}"),
        }
        assert!(
            !matches!(
                classify_error(&inf, 0, 15, RATE_LIMIT_RETRY_THRESHOLD),
                RetryDecision::RetryWithImageStrip
            ),
            "must never image-strip on local preflight"
        );
    }

    #[test]
    fn invalid_config_bridge_is_fatal_not_auth() {
        let err = AnthropicClientError::InvalidConfig("api_key must not be empty".into());
        let inf = err.into_inference_error();
        assert!(!inf.is_auth_error());
        assert!(!inf.is_retryable());
        assert!(inf.to_string().contains("invalid anthropic config"));
        match classify_error(&inf, 0, 15, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::Fatal(_) => {}
            RetryDecision::EmitToSession(_) => panic!("must not emit to session for reauth"),
            other => panic!("expected Fatal, got {other:?}"),
        }
    }

    #[test]
    fn stream_invalid_request_is_non_retryable_bridge() {
        let err = AnthropicClientError::stream_error(
            "invalid_request_error",
            "bad tools",
            AnthropicResponseMeta::default(),
        );
        assert_eq!(err.class(), ErrorClass::PermanentActionable);
        assert!(!err.is_retryable());
        let inf = err.into_inference_error();
        assert!(!inf.is_auth_error());
        assert!(!inf.is_retryable());
        match classify_error(&inf, 0, 15, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::Fatal(_) => {}
            other => panic!("expected Fatal, got {other:?}"),
        }
    }

    #[test]
    fn stream_authentication_error_is_auth_emit_to_session() {
        let err = AnthropicClientError::stream_error(
            "authentication_error",
            "key revoked",
            AnthropicResponseMeta::default(),
        );
        assert_eq!(err.class(), ErrorClass::PermanentAuth);
        assert!(!err.is_retryable());
        let inf = err.into_inference_error();
        assert!(inf.is_auth_error());
        match classify_error(&inf, 0, 15, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::EmitToSession(_) => {}
            other => panic!("expected EmitToSession, got {other:?}"),
        }
    }

    #[test]
    fn stream_overloaded_bridges_to_retryable_529() {
        let err = AnthropicClientError::stream_error(
            "overloaded_error",
            "Overloaded",
            AnthropicResponseMeta::default(),
        );
        assert_eq!(err.class(), ErrorClass::RetryableOverload);
        assert!(err.is_retryable());
        let inf = err.into_inference_error();
        assert!(inf.is_retryable());
        match &inf {
            InferenceError::Api { status, .. } => assert_eq!(status.as_u16(), 529),
            other => panic!("expected Api 529, got {other:?}"),
        }
        match classify_error(&inf, 0, 15, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::RetryWithClientRebuild { .. } => {}
            other => panic!("expected RetryWithClientRebuild, got {other:?}"),
        }
    }

    #[test]
    fn stream_rate_limit_preserves_retry_after_and_rate_limited_path() {
        let meta = AnthropicResponseMeta {
            retry_after_secs: Some(11),
            ..Default::default()
        };
        let err = AnthropicClientError::stream_error("rate_limit_error", "slow down", meta);
        assert_eq!(err.class(), ErrorClass::RetryableRateLimit);
        assert!(err.is_retryable());
        let inf = err.into_inference_error();
        assert!(inf.is_rate_limited());
        assert_eq!(inf.retry_after(), Some(11));
        match classify_error(&inf, 0, 15, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::RetryWithBackoff {
                backoff,
                is_rate_limited: true,
            } => {
                assert_eq!(backoff, std::time::Duration::from_secs(11));
            }
            other => panic!("expected rate-limited backoff, got {other:?}"),
        }
    }

    #[test]
    fn http_429_and_529_bridge_through_classify_error() {
        let rate = AnthropicClientError::Http {
            status: 429,
            class: ErrorClass::RetryableRateLimit,
            message: "rate limited".into(),
            error_type: Some(AnthropicErrorType::RateLimitError),
            meta: Box::new(AnthropicResponseMeta {
                retry_after_secs: Some(5),
                ..Default::default()
            }),
        };
        let inf = rate.into_inference_error();
        assert!(inf.is_rate_limited());
        match classify_error(&inf, 0, 15, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::RetryWithBackoff {
                backoff,
                is_rate_limited: true,
            } => assert_eq!(backoff, std::time::Duration::from_secs(5)),
            other => panic!("expected rate-limit backoff, got {other:?}"),
        }

        let overload = AnthropicClientError::Http {
            status: 529,
            class: ErrorClass::RetryableOverload,
            message: "Overloaded".into(),
            error_type: Some(AnthropicErrorType::OverloadedError),
            meta: Box::new(AnthropicResponseMeta::default()),
        };
        let inf = overload.into_inference_error();
        assert!(inf.is_retryable());
        match classify_error(&inf, 0, 15, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::RetryWithClientRebuild { .. } => {}
            other => panic!("expected RetryWithClientRebuild for 529, got {other:?}"),
        }
    }

    #[test]
    fn http_413_bridge_is_fatal_not_image_strip() {
        let err = AnthropicClientError::Http {
            status: 413,
            class: ErrorClass::PermanentActionable,
            message: "request_too_large: Request exceeds size limit".into(),
            error_type: Some(AnthropicErrorType::RequestTooLarge),
            meta: Box::new(AnthropicResponseMeta::default()),
        };
        assert_eq!(err.class(), ErrorClass::PermanentActionable);
        assert!(!err.is_retryable());

        let inf = err.into_inference_error();
        assert!(
            !inf.is_payload_too_large(),
            "bridged 413 must not report as payload-too-large (image-strip gate)"
        );
        assert!(!inf.is_auth_error());
        assert!(!inf.is_retryable());
        assert!(
            inf.to_string().contains("size limit") || inf.to_string().contains("too large"),
            "actionable message preserved: {inf}"
        );
        match classify_error(&inf, 0, 15, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::Fatal(_) => {}
            RetryDecision::RetryWithImageStrip => {
                panic!("server 413 must not trigger RetryWithImageStrip")
            }
            other => panic!("expected Fatal, got {other:?}"),
        }
    }
}
