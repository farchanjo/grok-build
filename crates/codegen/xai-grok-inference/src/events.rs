//! Outbound events emitted by the sampler.

use serde::{Deserialize, Serialize};

use xai_grok_inference_types::{
    ApiErrorCode, ApiErrorDiagnostics, ConversationResponse, EmptyResponseContext, InferenceError,
    ResponseModelMetadata, SentCredential,
};

use crate::metrics::InferenceLatencyStats;
use crate::types::RequestId;

/// Which content channel a token belongs to.
///
/// Extensible — adding a new channel (e.g., `Planning`) only requires a
/// new variant here, not new [`InferenceEvent`] variants. Mirrors the
/// agentic-sampler's `AgentChannel` pattern.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InferenceChannel {
    Text,
    Reasoning,
}

/// Why inline images were stripped from an in-flight request.
/// Consumers decide whether the strip is safe to persist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StripReason {
    /// The server deterministically rejected this payload with HTTP 400 and
    /// the typed `invalid_image` code.
    ServerRejected,
    /// A size or transport heuristic, proxy-wrapped failure, phrase-only
    /// match, or mid-stream error. These strips must remain request-local.
    PayloadHeuristic,
}

impl StripReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ServerRejected => "server_rejected",
            Self::PayloadHeuristic => "payload_heuristic",
        }
    }
}

/// Events emitted by the sampler for a single in-flight request.
///
/// Sent on the shared event channel that callers subscribe to. The
/// session translates these into ACP notifications.
#[derive(Debug, Clone)]
pub enum InferenceEvent {
    /// HTTP stream established, headers read. Emitted before any content.
    StreamStarted {
        request_id: RequestId,
        timestamp_ms: i64,
    },

    /// First content token received for a request.
    FirstToken { request_id: RequestId },

    /// Content token in a named channel (text or reasoning).
    ChannelToken {
        request_id: RequestId,
        channel: InferenceChannel,
        text: String,
        chunk_index: u64,
    },

    /// Streaming delta carrying a fragment of a tool call.
    ///
    /// Emitted by the L2 transforms (Chat Completions, Responses, Messages)
    /// per-chunk as the model streams tool-call arguments. Any single
    /// `arguments_delta` is NOT necessarily valid JSON in isolation.
    ToolCallDelta {
        request_id: RequestId,
        tool_index: u32,
        id: Option<String>,
        name: Option<String>,
        arguments_delta: Option<String>,
    },

    /// Streaming completed successfully.
    Completed {
        request_id: RequestId,
        response: Box<ConversationResponse>,
        metrics: InferenceLatencyStats,
    },

    /// Inline images were removed before retrying this request.
    ImagesStripped {
        request_id: RequestId,
        /// Exact URLs actually removed, including duplicate occurrences.
        stripped_urls: Vec<std::sync::Arc<str>>,
        reason: StripReason,
    },

    /// Request is being retried.
    Retrying {
        request_id: RequestId,
        attempt: u32,
        max_retries: u32,
        /// Typed retry class so consumers never have to sniff `reason`
        /// (e.g. the shell's doom-loop recovery counter).
        kind: InferenceErrorKind,
        reason: String,
        /// Doom-loop telemetry payload when `kind == DoomLoopDetected`:
        /// raw trigger labels + the chunk index the mid-stream abort fired
        /// at (`None` for terminal-response detections). Labels only.
        doom_loop_triggers: Option<Vec<String>>,
        doom_loop_aborted_at_chunk: Option<u64>,
        /// Exact delay selected by retry classification before the next attempt.
        /// `None` means the retry proceeds immediately (for example, after
        /// stripping an image from the request).
        backoff_ms: Option<u64>,
        /// Safe router/provider diagnostics from the failed API request.
        diagnostics: Option<ApiErrorDiagnostics>,
    },

    /// Request failed (after exhausting retries or non-retryable error).
    Failed {
        request_id: RequestId,
        error: InferenceErrorInfo,
    },

    /// Model metadata received from response headers.
    ModelMetadata {
        request_id: RequestId,
        metadata: ResponseModelMetadata,
    },

    /// A backend-hosted tool call has started execution on the server
    /// (e.g., web search is in progress). The client does NOT execute
    /// these — the backend's agentic sampler handles them.
    BackendToolCallStarted {
        request_id: RequestId,
        call_id: String,
        name: String,
    },

    /// A backend-hosted tool call has completed execution on the server.
    BackendToolCallCompleted {
        request_id: RequestId,
        call_id: String,
        name: String,
        /// Structured result data from the backend tool (tool-specific).
        /// For web search: `{"query": "...", "sources": [{"url": "..."}, ...]}`
        result: Option<serde_json::Value>,
    },
}

/// Serializable mirror of [`InferenceError`].
///
/// The rich `InferenceError` carries non-serializable inner values
/// (`reqwest::Error`, `serde_json::Error`) so it cannot cross a network
/// boundary. `InferenceErrorInfo` extracts the bits that downstream
/// consumers (UIs, gRPC adapters) actually need.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceErrorInfo {
    pub kind: InferenceErrorKind,
    pub status_code: Option<u16>,
    pub message: String,
    pub is_retryable: bool,
    pub retry_after_secs: Option<u64>,
    pub model_metadata: Option<ResponseModelMetadata>,
    /// Safe router/provider and rate-limit metadata attached to API failures.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<ApiErrorDiagnostics>,
    /// The server error envelope's `code` slot (e.g. `invalid_image`).
    /// Serializes as the plain wire string; `None` when absent or from an
    /// older peer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<ApiErrorCode>,
    /// Present only when `kind == EmptyResponse`. Carries the structured
    /// context from the L2 stream so downstream consumers can distinguish
    /// reasoning-only completions from transport failures.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub empty_response_context: Option<EmptyResponseContext>,
    /// Present only when `kind == DoomLoopDetected`. Raw trigger labels
    /// (never generation content) so the retry loop can reconstruct the
    /// rich error from a synthesized L2 failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doom_loop_triggers: Option<Vec<String>>,
    /// Stream chunk index the mid-stream doom-loop abort fired at.
    /// Telemetry only; `None` for terminal-response detections.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doom_loop_aborted_at_chunk: Option<u64>,
    /// Meaningful for auth failures: whether the rejected request actually
    /// carried a credential. Older payloads default to fail-closed `Unknown`.
    #[serde(default, skip_serializing_if = "SentCredential::is_unknown")]
    pub credential: SentCredential,
}

/// Coarse-grained classification of a sampling failure.
///
/// Intentionally narrow — context-window-exceeded does not have its own
/// variant. The session detects explicit overflow messages and also uses model
/// metadata plus its tracked token estimate when response headers provide a
/// revised context window. Streamed errors commonly have no model metadata.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum InferenceErrorKind {
    Auth,
    Http,
    Api,
    Serialization,
    IdleTimeout,
    RateLimited,
    EmptyResponse,
    MaxTokensTruncation,
    DoomLoopDetected,
}

impl InferenceErrorKind {
    /// Stable, lowercase string form suitable for telemetry tags
    /// (e.g., analytics `error_type` columns and signals histograms).
    /// Mirrors the strings used in the shell's
    /// `stream_conversation_with_retries` error classifier so tags stay
    /// consistent across surfaces.
    pub fn as_str(self) -> &'static str {
        match self {
            InferenceErrorKind::Auth => "auth",
            InferenceErrorKind::Http => "http",
            InferenceErrorKind::Api => "api",
            InferenceErrorKind::Serialization => "serialization",
            InferenceErrorKind::IdleTimeout => "idle_timeout",
            InferenceErrorKind::RateLimited => "rate_limited",
            InferenceErrorKind::EmptyResponse => "empty_response",
            InferenceErrorKind::MaxTokensTruncation => "max_tokens_truncation",
            InferenceErrorKind::DoomLoopDetected => "doom_loop_detected",
        }
    }
}

impl From<&InferenceError> for InferenceErrorInfo {
    fn from(err: &InferenceError) -> Self {
        let is_retryable = err.is_retryable();
        let message = err.to_string();

        let (kind, status_code, retry_after_secs, model_metadata, diagnostics) = match err {
            InferenceError::Auth { .. } => (InferenceErrorKind::Auth, None, None, None, None),
            InferenceError::InvalidConfiguration(_) => {
                (InferenceErrorKind::Api, None, None, None, None)
            }
            InferenceError::Http(_) => (InferenceErrorKind::Http, None, None, None, None),
            InferenceError::Serialization(_) => {
                (InferenceErrorKind::Serialization, None, None, None, None)
            }
            InferenceError::Api {
                status,
                model_metadata,
                retry_after_secs,
                diagnostics,
                ..
            } => {
                let kind = if err.is_rate_limited() {
                    InferenceErrorKind::RateLimited
                } else {
                    InferenceErrorKind::Api
                };
                (
                    kind,
                    Some(status.as_u16()),
                    *retry_after_secs,
                    model_metadata.clone(),
                    diagnostics.clone(),
                )
            }
            InferenceError::EventStreamError(_) => {
                (InferenceErrorKind::Http, None, None, None, None)
            }
            InferenceError::StreamError { .. } => (InferenceErrorKind::Api, None, None, None, None),
            InferenceError::IdleTimeout { .. } => {
                (InferenceErrorKind::IdleTimeout, None, None, None, None)
            }
            InferenceError::EmptyResponse { .. } => {
                (InferenceErrorKind::EmptyResponse, None, None, None, None)
            }
            InferenceError::MaxTokensTruncation => (
                InferenceErrorKind::MaxTokensTruncation,
                None,
                None,
                None,
                None,
            ),
            InferenceError::DoomLoopDetected { .. } => {
                (InferenceErrorKind::DoomLoopDetected, None, None, None, None)
            }
        };

        let empty_response_context = match err {
            InferenceError::EmptyResponse { context } => Some(context.clone()),
            _ => None,
        };
        let (doom_loop_triggers, doom_loop_aborted_at_chunk) = match err {
            InferenceError::DoomLoopDetected {
                triggers,
                aborted_at_chunk,
            } => (Some(triggers.clone()), *aborted_at_chunk),
            _ => (None, None),
        };
        let error_code = match err {
            InferenceError::Api { error_code, .. } => error_code.clone(),
            InferenceError::StreamError { code, .. } => code.clone(),
            _ => None,
        };
        let credential = match err {
            InferenceError::Auth { credential, .. } => *credential,
            _ => SentCredential::Unknown,
        };

        Self {
            kind,
            status_code,
            message,
            is_retryable,
            retry_after_secs,
            model_metadata,
            diagnostics,
            error_code,
            empty_response_context,
            doom_loop_triggers,
            doom_loop_aborted_at_chunk,
            credential,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::StatusCode;

    #[test]
    fn auth_variant_classified_as_auth() {
        let err = InferenceError::auth_unknown("bad token");
        let info = InferenceErrorInfo::from(&err);
        assert_eq!(info.kind, InferenceErrorKind::Auth);
        assert_eq!(info.status_code, None);
        assert!(!info.is_retryable);
        assert_eq!(info.retry_after_secs, None);
        assert!(info.model_metadata.is_none());
        assert!(info.message.contains("bad token"));
        assert_eq!(info.credential, SentCredential::Unknown);
    }

    #[test]
    fn auth_info_preserves_credential_provenance_and_legacy_defaults_unknown() {
        let sent = InferenceError::Auth {
            message: "rejected".into(),
            credential: SentCredential::Sent,
        };
        assert_eq!(
            InferenceErrorInfo::from(&sent).credential,
            SentCredential::Sent
        );

        let legacy: InferenceErrorInfo = serde_json::from_str(
            r#"{"kind":"Auth","status_code":401,"message":"x","is_retryable":false,
                "retry_after_secs":null,"model_metadata":null}"#,
        )
        .unwrap();
        assert_eq!(legacy.credential, SentCredential::Unknown);
    }

    #[test]
    fn invalid_configuration_classified_as_api() {
        let err = InferenceError::InvalidConfiguration("missing model");
        let info = InferenceErrorInfo::from(&err);
        assert_eq!(info.kind, InferenceErrorKind::Api);
        assert_eq!(info.status_code, None);
        assert!(!info.is_retryable);
    }

    #[test]
    fn serialization_variant_classified_as_serialization() {
        let json_err = serde_json::from_str::<i32>("not a number").unwrap_err();
        let err: InferenceError = json_err.into();
        let info = InferenceErrorInfo::from(&err);
        assert_eq!(info.kind, InferenceErrorKind::Serialization);
        assert!(!info.is_retryable);
    }

    #[test]
    fn api_500_classified_as_api_and_retryable() {
        let err = InferenceError::Api {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "boom".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
            diagnostics: None,
            error_code: None,
        };
        let info = InferenceErrorInfo::from(&err);
        assert_eq!(info.kind, InferenceErrorKind::Api);
        assert_eq!(info.status_code, Some(500));
        assert!(info.is_retryable, "5xx should be retryable");
    }

    #[test]
    fn api_429_classified_as_rate_limited_and_extracts_retry_after() {
        let err = InferenceError::Api {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: "slow down".into(),
            model_metadata: None,
            retry_after_secs: Some(15),
            should_retry: None,
            diagnostics: None,
            error_code: None,
        };
        let info = InferenceErrorInfo::from(&err);
        assert_eq!(info.kind, InferenceErrorKind::RateLimited);
        assert_eq!(info.status_code, Some(429));
        assert_eq!(info.retry_after_secs, Some(15));
        assert!(info.is_retryable, "429 should be retryable");
    }

    #[test]
    fn api_error_diagnostics_are_preserved_for_event_consumers() {
        let err = InferenceError::Api {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: "slow down".into(),
            model_metadata: None,
            retry_after_secs: Some(60),
            should_retry: None,
            diagnostics: Some(ApiErrorDiagnostics {
                provider_name: Some("OpenRouter".into()),
                provider_code: Some("rate_limit_exceeded".into()),
                rate_limit_remaining: Some("0".into()),
                ..Default::default()
            }),
            error_code: None,
        };
        let info = InferenceErrorInfo::from(&err);
        let diagnostics = info.diagnostics.expect("diagnostics preserved");
        assert_eq!(diagnostics.provider_name.as_deref(), Some("OpenRouter"));
        assert_eq!(
            diagnostics.provider_code.as_deref(),
            Some("rate_limit_exceeded")
        );
        assert_eq!(diagnostics.rate_limit_remaining.as_deref(), Some("0"));
    }

    #[test]
    fn api_400_classified_as_api_and_not_retryable() {
        let err = InferenceError::Api {
            status: StatusCode::BAD_REQUEST,
            message: "context window exceeded".into(),
            model_metadata: Some(ResponseModelMetadata {
                context_window: Some(8000),
                ..Default::default()
            }),
            retry_after_secs: None,
            should_retry: None,
            diagnostics: None,
            error_code: None,
        };
        let info = InferenceErrorInfo::from(&err);
        assert_eq!(info.kind, InferenceErrorKind::Api);
        assert_eq!(info.status_code, Some(400));
        assert!(!info.is_retryable, "4xx (non-429) should not be retryable");
        let metadata = info.model_metadata.expect("metadata preserved");
        assert_eq!(metadata.context_window, Some(8000));
    }

    #[test]
    fn event_stream_error_classified_as_http_and_retryable() {
        let err = InferenceError::EventStreamError("conn reset".into());
        let info = InferenceErrorInfo::from(&err);
        assert_eq!(info.kind, InferenceErrorKind::Http);
        assert!(info.is_retryable);
    }

    #[test]
    fn stream_error_classified_as_api_and_retryable() {
        let err = InferenceError::StreamError {
            error_type: "server_error".into(),
            message: "transient".into(),
            code: None,
        };
        let info = InferenceErrorInfo::from(&err);
        assert_eq!(info.kind, InferenceErrorKind::Api);
        assert_eq!(info.status_code, None);
        assert!(info.is_retryable, "stream errors should be retryable");
    }

    #[test]
    fn idle_timeout_classified_as_idle_timeout_and_not_retryable() {
        let err = InferenceError::IdleTimeout { elapsed_secs: 300 };
        let info = InferenceErrorInfo::from(&err);
        assert_eq!(info.kind, InferenceErrorKind::IdleTimeout);
        assert!(!info.is_retryable);
        assert!(info.message.contains("300s"));
    }

    #[test]
    fn from_inference_error_carries_error_code() {
        let api = InferenceError::Api {
            status: StatusCode::BAD_REQUEST,
            message: "bad image".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
            diagnostics: None,
            error_code: Some(ApiErrorCode::InvalidImage),
        };
        assert_eq!(
            InferenceErrorInfo::from(&api).error_code,
            Some(ApiErrorCode::InvalidImage)
        );

        let stream = InferenceError::StreamError {
            error_type: "invalid_request_error".into(),
            message: "bad image".into(),
            code: Some(ApiErrorCode::InvalidImage),
        };
        assert_eq!(
            InferenceErrorInfo::from(&stream).error_code,
            Some(ApiErrorCode::InvalidImage)
        );

        assert_eq!(
            InferenceErrorInfo::from(&InferenceError::auth_unknown("x")).error_code,
            None
        );
    }

    /// Older persisted/wire payloads that omit `error_code` keep deserializing.
    #[test]
    fn inference_error_info_legacy_payload_deserializes_without_error_code() {
        let info: InferenceErrorInfo = serde_json::from_str(
            r#"{"kind":"Api","status_code":400,"message":"x","is_retryable":false,
                "retry_after_secs":null,"model_metadata":null}"#,
        )
        .unwrap();
        assert_eq!(info.error_code, None);
        assert_eq!(info.kind, InferenceErrorKind::Api);
    }
}
