//! Sampling error types.
//!
//! TODO: Move from xai-grok-shell/src/sampling/error.rs

use std::fmt;

use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, InferenceError>;

/// Why the model's response was classified as "empty" by [`ConversationResponse::empty_reason`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmptyReason {
    /// The model emitted reasoning tokens but produced no visible content
    /// and no tool calls. The stream completed normally (has `finish_reason`).
    ReasoningOnly,
    /// The stream carried at least one `choice` but the final assistant
    /// message has empty `content` and no tool calls (and no reasoning).
    NoVisibleContent,
}

impl EmptyReason {
    pub fn as_str(self) -> &'static str {
        match self {
            EmptyReason::ReasoningOnly => "reasoning_only",
            EmptyReason::NoVisibleContent => "no_visible_content",
        }
    }
}

impl fmt::Display for EmptyReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Structured context captured at L2 stream completion time when the
/// response is classified as empty. Carries everything needed to
/// root-cause the issue from a single log line or error payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmptyResponseContext {
    pub reason: EmptyReason,
    /// Whether the response contained reasoning tokens.
    pub had_reasoning: bool,
    /// Byte length of the accumulated `content` string (0 for truly empty).
    pub content_len: usize,
    /// Number of tool calls in the final response.
    pub tool_call_count: usize,
    /// The `finish_reason` from the stream, if any.
    pub finish_reason: Option<String>,
    /// Token usage from the response (when available).
    pub completion_tokens: Option<u32>,
    pub reasoning_tokens: Option<u32>,
    pub prompt_tokens: Option<u32>,
    /// Model that produced the response.
    pub model: String,
    /// Whether at least one `choice` was seen in the stream.
    pub first_choice_seen: bool,
}

impl EmptyResponseContext {
    pub fn finish_reason_str(&self) -> &str {
        self.finish_reason.as_deref().unwrap_or("none")
    }
}

/// Model metadata from response headers.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResponseModelMetadata {
    pub context_window: Option<u64>,
    pub max_completion_tokens: Option<u32>,
    /// `x-models-etag` — triggers model catalog refresh when changed.
    pub models_etag: Option<String>,
}

/// Safe, structured diagnostics attached to an API error response.
///
/// These fields contain only provider routing and rate-limit metadata. They
/// never contain response bodies, request content, or credentials, so callers
/// may include them in structured telemetry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiErrorDiagnostics {
    /// Router-provided categorisation of the upstream failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_type: Option<String>,
    /// Upstream provider's machine-readable error code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_code: Option<String>,
    /// Upstream provider selected by a router such as OpenRouter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_name: Option<String>,
    /// Rate-limit ceiling reported by the server, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_limit_limit: Option<String>,
    /// Remaining requests/tokens reported by the server, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_limit_remaining: Option<String>,
    /// Server-defined rate-limit reset value. This remains a string because
    /// providers legitimately use both delta-seconds and absolute timestamps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_limit_reset: Option<String>,
    /// Parsed `x-ratelimit-reset` value expressed as seconds-until-reset,
    /// clamped to `[0, RATE_LIMIT_RESET_MAX_SECS]`. Derived from the raw
    /// [`Self::rate_limit_reset`] string: delta-seconds are used as-is;
    /// epoch seconds are converted to a delta against the current time
    /// (past epochs clamp to 0); unparseable/HTTP-date values are `None`.
    /// Additive and serde-optional, so older payloads keep deserializing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_limit_reset_secs: Option<u64>,
    /// Opaque generation identifier returned by a router for support/debugging.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_id: Option<String>,
}

/// Upper bound (in seconds) for a parsed `x-ratelimit-reset` window. A
/// misbehaving upstream could otherwise report an absurdly large reset and
/// stall sampling indefinitely; values above this clamp down to it.
pub const RATE_LIMIT_RESET_MAX_SECS: u64 = 600;

impl ApiErrorDiagnostics {
    pub fn is_empty(&self) -> bool {
        self.error_type.is_none()
            && self.provider_code.is_none()
            && self.provider_name.is_none()
            && self.rate_limit_limit.is_none()
            && self.rate_limit_remaining.is_none()
            && self.rate_limit_reset.is_none()
            && self.rate_limit_reset_secs.is_none()
            && self.generation_id.is_none()
    }

    /// Parsed seconds-until-reset derived from the raw
    /// [`Self::rate_limit_reset`] string, when present and interpretable.
    /// OpenRouter/proxies send either a delta-seconds value (`"30"`) or an
    /// absolute epoch-seconds timestamp. HTTP-dates and any unparseable form
    /// yield `None` so callers fall back to other backoff sources.
    ///
    /// Delta-seconds are clamped to `[0, RATE_LIMIT_RESET_MAX_SECS]`. Epoch
    /// seconds in the past clamp to `0`; future epoch seconds are converted to
    /// a delta against `now` and then clamped.
    pub fn parsed_reset_secs(&self) -> Option<u64> {
        self.rate_limit_reset_secs
    }
}

/// Parse an `x-ratelimit-reset` header value into seconds-until-reset.
///
/// Providers send one of:
/// - delta-seconds (e.g. `"30"`) — used directly;
/// - epoch seconds (e.g. `"1735689600"`) — converted to a delta against
///   `now` (past epochs clamp to `0`);
/// - HTTP-date or anything unparseable — `None`.
///
/// The result is clamped to `[0, RATE_LIMIT_RESET_MAX_SECS]` so a
/// misbehaving upstream can't stall sampling indefinitely. The heuristic for
/// distinguishing delta from epoch is the value's magnitude: anything at or
/// above `1_000_000_000` (a 10-digit epoch near 2001) is treated as epoch
/// seconds, mirroring common `Retry-After` parsing practice.
pub fn parse_rate_limit_reset(value: Option<&str>) -> Option<u64> {
    let raw = value?.trim();
    if raw.is_empty() {
        return None;
    }
    // Only the integer-seconds form is documented for these providers; an
    // HTTP-date (contains a non-digit such as ':' or letters) is rejected.
    let parsed: u64 = raw.parse().ok()?;
    let secs = if parsed >= 1_000_000_000 {
        // Interpret as epoch seconds relative to now.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        parsed.saturating_sub(now)
    } else {
        // Delta-seconds.
        parsed
    };
    Some(secs.min(RATE_LIMIT_RESET_MAX_SECS))
}

/// Display prefix of [`InferenceError::Serialization`]. Shared with the
/// variant's `#[error(...)]` template so [`InferenceError::serialization_from_rendered`]
/// can never drift from what Display actually emits.
const SERIALIZATION_DISPLAY_PREFIX: &str = "serialization error: ";

#[derive(Debug, Error)]
pub enum InferenceError {
    #[error("{0}")]
    Auth(String),
    #[error("invalid client configuration: {0}")]
    InvalidConfiguration(&'static str),
    #[error("request error: {0}")]
    Http(reqwest::Error),
    #[error("{prefix}{0}", prefix = SERIALIZATION_DISPLAY_PREFIX)]
    Serialization(serde_json::Error),
    #[error("API error (status {status}): {message}")]
    Api {
        status: StatusCode,
        message: String,
        model_metadata: Option<ResponseModelMetadata>,
        /// Parsed from the `Retry-After` response header (seconds).
        retry_after_secs: Option<u64>,
        /// Parsed from the `x-should-retry` response header.
        /// `Some(true)` = transient, retry may help.
        /// `Some(false)` = request-content error, don't retry.
        /// `None` = header absent (old server or non-proxy origin).
        should_retry: Option<bool>,
        /// Router/provider and rate-limit metadata safe for diagnostics.
        diagnostics: Option<ApiErrorDiagnostics>,
    },
    #[error("reqwest error stream: {0}")]
    EventStreamError(String),
    /// Server-side stream error (sent as JSON within the SSE stream)
    #[error("stream error ({error_type}): {message}")]
    StreamError { error_type: String, message: String },
    /// Per-chunk idle timeout — no SSE chunk received from the model within the
    /// configured deadline. NOT retryable: the model (or network path) is stuck,
    /// and replaying the same request would likely stall again.
    #[error("inference idle timeout after {elapsed_secs}s with no chunks")]
    IdleTimeout { elapsed_secs: u64 },
    #[error("empty response from model ({})", context.reason)]
    EmptyResponse { context: EmptyResponseContext },
    #[error("response truncated by max_tokens")]
    MaxTokensTruncation,
    /// A confident server-reported doom loop on the attempt (mid-stream or
    /// on the completed response). Retryable on the recovery loop's own
    /// budget, separate from the transport budget. Carries the raw trigger
    /// labels (never generation content) plus, for telemetry only, the
    /// stream chunk index the mid-stream abort fired at (`None` when the
    /// signal was only seen on the completed response).
    #[error("doom loop detected: {}", triggers.join(", "))]
    DoomLoopDetected {
        triggers: Vec<String>,
        aborted_at_chunk: Option<u64>,
    },
}

impl InferenceError {
    /// Rebuild a `Serialization` error from a rendered message for non-`Clone`
    /// contexts; it must stay `Serialization` so it remains non-retryable.
    pub fn serialization_message(msg: impl fmt::Display) -> Self {
        Self::Serialization(serde::de::Error::custom(msg))
    }

    /// Rebuild from this variant's full rendered Display (e.g. a round-tripped
    /// `InferenceErrorInfo` message), stripping the Display prefix so the
    /// rebuilt error does not render it twice.
    pub fn serialization_from_rendered(rendered: &str) -> Self {
        Self::serialization_message(
            rendered
                .strip_prefix(SERIALIZATION_DISPLAY_PREFIX)
                .unwrap_or(rendered),
        )
    }

    pub fn is_auth_error(&self) -> bool {
        // Only 401 Unauthorized means the credentials themselves were rejected
        // and warrant a token refresh / re-auth. 403 Forbidden means the
        // request was authenticated successfully but the action is not
        // permitted (e.g. content-safety blocks, ZDR-blocked operations,
        // or other policy denials unrelated to credentials). Treating 403
        // as an auth error triggers a pointless
        // OIDC refresh and then surfaces as acp::Error::auth_required on
        // the client, which in the desktop app tears down the session and
        // can race with invalid_grant_threshold to wipe auth.json.
        matches!(
            self,
            InferenceError::Auth(_)
                | InferenceError::Api {
                    status: StatusCode::UNAUTHORIZED,
                    ..
                }
        )
    }

    pub fn is_rate_limited(&self) -> bool {
        matches!(
            self,
            InferenceError::Api {
                status: StatusCode::TOO_MANY_REQUESTS,
                ..
            }
        )
    }

    pub fn is_payload_too_large(&self) -> bool {
        matches!(
            self,
            InferenceError::Api {
                status: StatusCode::PAYLOAD_TOO_LARGE,
                ..
            }
        )
    }

    /// `true` when the error looks like a connection reset or broken pipe
    /// during request upload — the pattern nginx produces when it rejects an
    /// oversized payload by closing the connection instead of responding 413.
    ///
    /// Timeouts and connect failures are excluded: those are unrelated to
    /// payload size and stripping images on them would lose context for no
    /// reason.
    pub fn is_likely_body_rejected(&self) -> bool {
        match self {
            InferenceError::Http(err) => {
                // `is_request()` covers broken-pipe / connection-reset during
                // body upload.  `is_body()` covers stream-write failures.
                // Exclude timeouts and connect errors — those are unrelated.
                (err.is_request() || err.is_body()) && !err.is_timeout() && !err.is_connect()
            }
            _ => false,
        }
    }

    /// The server rejected the request because the conversation history
    /// contains `encrypted_content` from a different model family that the
    /// current model cannot decrypt. Never retryable — the user must start
    /// a new session.
    pub fn is_encrypted_content_error(&self) -> bool {
        matches!(
            self,
            InferenceError::Api {
                status: StatusCode::BAD_REQUEST,
                message,
                ..
            } if message.contains("encrypted_content")
        )
    }

    /// The API rejected the request because an inline image could not be
    /// processed. Matches both direct 400 and proxy-wrapped 500 responses.
    /// Exact-case match — consistent with `is_encrypted_content_error`.
    pub fn is_image_processing_error(&self) -> bool {
        matches!(
            self,
            InferenceError::Api {
                status,
                message,
                ..
            } if matches!(status.as_u16(), 400 | 500) && message.contains("Could not process image")
        )
    }

    pub fn is_retryable(&self) -> bool {
        match self {
            InferenceError::Auth(_) => false,
            InferenceError::InvalidConfiguration(_) => false,
            InferenceError::Http(err) => is_retryable_reqwest(err),
            InferenceError::Serialization(_) => false,
            InferenceError::Api { status, .. } => {
                matches!(status.as_u16(), 429 | 500 | 502 | 503 | 504 | 520)
            }
            InferenceError::EventStreamError(_) => true,
            InferenceError::StreamError { .. } => true,
            InferenceError::IdleTimeout { .. } => false,
            InferenceError::EmptyResponse { .. } => true,
            InferenceError::MaxTokensTruncation => false,
            InferenceError::DoomLoopDetected { .. } => true,
        }
    }

    pub fn model_metadata(&self) -> Option<&ResponseModelMetadata> {
        match self {
            InferenceError::Api { model_metadata, .. } => model_metadata.as_ref(),
            _ => None,
        }
    }

    pub fn retry_after(&self) -> Option<u64> {
        match self {
            InferenceError::Api {
                retry_after_secs, ..
            } => *retry_after_secs,
            _ => None,
        }
    }

    /// Parsed `x-ratelimit-reset` seconds-until-reset from the error's
    /// [`ApiErrorDiagnostics`], when available. See
    /// [`ApiErrorDiagnostics::parsed_reset_secs`].
    pub fn rate_limit_reset_secs(&self) -> Option<u64> {
        match self {
            InferenceError::Api { diagnostics, .. } => {
                diagnostics.as_ref().and_then(|d| d.parsed_reset_secs())
            }
            _ => None,
        }
    }

    /// Server hint on whether this error is worth retrying.
    pub fn should_retry_header(&self) -> Option<bool> {
        match self {
            InferenceError::Api { should_retry, .. } => *should_retry,
            _ => None,
        }
    }

    /// Structured router/provider diagnostics from an API error response.
    pub fn diagnostics(&self) -> Option<&ApiErrorDiagnostics> {
        match self {
            InferenceError::Api { diagnostics, .. } => diagnostics.as_ref(),
            _ => None,
        }
    }

    /// True when this error is a context-window/size overflow — deterministic,
    /// so retrying the same payload can't help. See [`is_context_length_error`].
    pub fn is_context_length_error(&self) -> bool {
        match self {
            InferenceError::Api { message, .. } | InferenceError::StreamError { message, .. } => {
                is_context_length_error(message)
            }
            _ => false,
        }
    }
}

impl From<reqwest::Error> for InferenceError {
    fn from(value: reqwest::Error) -> Self {
        Self::Http(value)
    }
}

impl From<serde_json::Error> for InferenceError {
    fn from(value: serde_json::Error) -> Self {
        tracing::debug!("Serde deserialization error: {:?}", &value);
        Self::Serialization(value)
    }
}

/// OpenAI-standard provider error format: `{"error": {"message": "...", "type": "..."}}`.
#[derive(Debug, Deserialize)]
struct ErrorResponse {
    error: ErrorBody,
}

#[derive(Debug, Deserialize)]
struct ErrorBody {
    message: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
}

/// Flat error from the Grok proxy/gateway: `{"code": "...", "error": "..."}`.
#[derive(Debug, Deserialize)]
struct FlatErrorResponse {
    error: String,
    #[serde(default)]
    code: Option<String>,
}

/// Extract `(error_type, message)` from either error format.
fn try_parse_error(data: &str) -> Option<(String, String)> {
    // Prefer Value-based OpenRouter hybrid detection first: a
    // chat.completion.chunk with finish_reason=error also deserializes as a
    // loose ErrorResponse (extra fields ignored, type missing → "unknown"),
    // which would hide rate-limit / provider metadata.
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(data) {
        let finish = value
            .pointer("/choices/0/finish_reason")
            .and_then(|v| v.as_str());
        if finish == Some("error") {
            if let Some((kind, message)) = openrouter_error_fields(value.get("error")) {
                return Some((kind, message));
            }
            return Some((
                "finish_reason_error".to_string(),
                "Upstream model stream ended with finish_reason=error".to_string(),
            ));
        }
        // Top-level error object without choices (pure error envelope that
        // still carries code/metadata instead of OpenAI `type`).
        if value.get("choices").is_none()
            && let Some((kind, message)) = openrouter_error_fields(value.get("error"))
        {
            return Some((kind, message));
        }
    }
    if let Ok(resp) = serde_json::from_str::<ErrorResponse>(data) {
        return Some((
            resp.error.kind.unwrap_or_else(|| "unknown".to_string()),
            resp.error
                .message
                .unwrap_or_else(|| "unknown error".to_string()),
        ));
    }
    if let Ok(flat) = serde_json::from_str::<FlatErrorResponse>(data) {
        return Some((
            flat.code.unwrap_or_else(|| "server_error".to_string()),
            flat.error,
        ));
    }
    None
}

/// Pull `(type, message)` from OpenRouter's flexible `error` object.
/// `code` may be a string or number; `type` / `metadata.error_type` are optional.
fn openrouter_error_fields(error: Option<&serde_json::Value>) -> Option<(String, String)> {
    let error = error?.as_object()?;
    let message = error
        .get("message")
        .and_then(|v| v.as_str())
        .map(str::to_owned)
        .filter(|s| !s.is_empty())?;
    let kind = error
        .get("type")
        .and_then(|v| v.as_str())
        .map(str::to_owned)
        .or_else(|| {
            error
                .get("metadata")
                .and_then(|m| m.get("error_type"))
                .and_then(|v| v.as_str())
                .map(str::to_owned)
        })
        .or_else(|| {
            error.get("code").and_then(|v| match v {
                serde_json::Value::String(s) => Some(s.clone()),
                serde_json::Value::Number(n) => Some(n.to_string()),
                _ => None,
            })
        })
        .unwrap_or_else(|| "finish_reason_error".to_string());
    Some((kind, message))
}

/// Max chars of a structured (JSON) error message shown to users.
pub const MAX_USER_ERROR_BODY_CHARS: usize = 280;
/// Head kept by [`truncate_user_error`] when a structured message exceeds
/// [`MAX_USER_ERROR_BODY_CHARS`]. OpenRouter error bodies front-load
/// boilerplate and put the upstream's useful message at the end, so we keep
/// both the beginning and the end (see [`TAIL_CHARS`]).
const HEAD_CHARS: usize = 120;
/// Tail kept by [`truncate_user_error`] when a structured message exceeds
/// [`MAX_USER_ERROR_BODY_CHARS`]. Together `HEAD_CHARS + TAIL_CHARS` (≈ 260)
/// leaves room for the ellipsis separator under the 280-char cap.
const TAIL_CHARS: usize = 140;

/// Short status-based copy when the body is not a structured JSON error.
///
/// Edge proxies (Cloudflare 52x, 502/503/504) return HTML pages; we never
/// sniff body text — only the HTTP status drives this fallback. The copy is
/// **provider-aware**: `provider_label` names the actual upstream (e.g.
/// "OpenRouter", "OpenAI") instead of hardcoding "Grok". Use
/// [`status_user_message`] for the historical xAI-only wording; new code
/// should prefer [`status_user_message_for`] with the resolved provider label.
pub fn status_user_message(status: StatusCode) -> String {
    status_user_message_for(status, "Grok")
}

/// Provider-aware variant of [`status_user_message`].
///
/// `provider_label` is the user-facing name of the selected upstream (e.g.
/// "Grok" for xAI, "OpenRouter", "OpenAI", or "the model provider" for
/// unknown/custom providers). When the diagnostics `provider_name` (the
/// specific OpenRouter upstream) is known, callers should pass that instead
/// of the generic provider label.
///
/// HTTP 402 (Payment Required) — OpenRouter's out-of-credits signal —
/// produces a dedicated, actionable credits message naming the provider. It
/// is fatal (never retried; see [`crate::InferenceError::is_retryable`]).
pub fn status_user_message_for(status: StatusCode, provider_label: &str) -> String {
    match status.as_u16() {
        402 => format!(
            "{provider_label} account out of credits — add credits to continue. (HTTP 402)."
        ),
        code @ 502..=504 => format!(
            "{provider_label} is temporarily unavailable. Please try again in a moment. (HTTP {code})."
        ),
        // Cloudflare edge codes (origin down / connect fail / timeout / …).
        code @ 520..=524 => format!(
            "Connection to {provider_label} timed out or was interrupted. Please try again. (HTTP {code})."
        ),
        code if status.is_server_error() => {
            format!("Something went wrong on the server (HTTP {code}).")
        }
        code if status.is_client_error() => format!("Request failed (HTTP {code})."),
        code => format!("Request failed (HTTP {code})."),
    }
}

fn truncate_user_error(s: &str) -> String {
    let s = s.trim();
    let count = s.chars().count();
    if count <= MAX_USER_ERROR_BODY_CHARS {
        return s.to_owned();
    }
    // OpenRouter (and other routers) front-load boilerplate and put the
    // upstream's useful message at the end of the body. Keep both the head
    // and the tail with an ellipsis between so the actionable portion is
    // never lost. HEAD_CHARS + TAIL_CHARS (≈ 260) + 1 ellipsis stays under
    // the 280-char cap.
    let head: String = s.chars().take(HEAD_CHARS).collect();
    let tail: String = s.chars().skip(count.saturating_sub(TAIL_CHARS)).collect();
    format!("{head}\u{2026}{tail}")
}

/// Format a known JSON error envelope; `None` if the body is not structured.
fn structured_error_message(bytes: &[u8]) -> Option<String> {
    let (error_type, message) = std::str::from_utf8(bytes).ok().and_then(try_parse_error)?;
    let msg = if error_type == "unknown" || error_type == "server_error" {
        message
    } else {
        format!("{error_type}: {message}")
    };
    Some(truncate_user_error(&msg))
}

/// Parse an API error body into a short string.
///
/// Only structured JSON error envelopes are surfaced. Non-JSON bodies
/// (HTML edge pages, plain text dumps) return a fixed placeholder — never
/// the raw bytes. Prefer [`user_facing_api_error_message_for`] when a
/// status code and provider label are available.
pub fn parse_error_bytes(bytes: &[u8]) -> String {
    structured_error_message(bytes).unwrap_or_else(|| "upstream error".into())
}

/// User-facing message for a failed API call.
///
/// Structured JSON error envelopes keep their message. Everything else
/// (including Cloudflare HTML) maps to a status-based string — no body
/// content matching. Uses the historical xAI-only "Grok" wording; new code
/// should prefer [`user_facing_api_error_message_for`] with the resolved
/// provider label.
pub fn user_facing_api_error_message(status: StatusCode, bytes: &[u8]) -> String {
    user_facing_api_error_message_for(status, bytes, "Grok")
}

/// Provider-aware variant of [`user_facing_api_error_message`].
///
/// `provider_label` is the user-facing name of the selected upstream (e.g.
/// "Grok" for xAI, "OpenRouter", "OpenAI", or "the model provider" for
/// unknown/custom providers). It is used only for the status-based fallback
/// (non-JSON bodies such as Cloudflare HTML). Structured JSON error
/// envelopes keep their own message regardless of provider.
///
/// HTTP 402 (Payment Required) is OpenRouter's out-of-credits signal. Its
/// status copy is a dedicated credits message naming the provider. When the
/// body is a structured envelope (e.g. `{"error":{"message": "..."}}`) the
/// envelope message is still surfaced, so the dedicated 402 copy only
/// applies to non-JSON 402 responses — which matches the existing contract
/// where structured bodies always win over status copy.
pub fn user_facing_api_error_message_for(
    status: StatusCode,
    bytes: &[u8],
    provider_label: &str,
) -> String {
    structured_error_message(bytes)
        .unwrap_or_else(|| status_user_message_for(status, provider_label))
}

pub fn try_parse_stream_error(data: &str) -> Option<InferenceError> {
    let (error_type, message) = try_parse_error(data)?;
    tracing::warn!(error_type, message, "Server-side stream error");
    Some(InferenceError::StreamError {
        error_type,
        message,
    })
}

/// True when an error message indicates a context-window overflow. Backends report
/// this inconsistently with no stable error code, so we match the message text; it's
/// deterministic (re-sending the same payload always fails), so callers must not retry.
pub fn is_context_length_error(message: &str) -> bool {
    let m = message.to_ascii_lowercase();
    m.contains("too long for this model")
        || m.contains("prompt is too long")
        || m.contains("maximum prompt length")
        || m.contains("maximum context length")
        || m.contains("context_length_exceeded")
}

/// Decide whether a [`reqwest::Error`] is worth retrying.
pub fn is_retryable_reqwest(err: &reqwest::Error) -> bool {
    if err.is_timeout() || err.is_connect() {
        return true;
    }

    if err.is_status() {
        return matches!(
            err.status(),
            Some(status) if status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS
        );
    }

    if err.is_request() || err.is_body() {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_length_error_matches_backend_variants() {
        for msg in [
            "This model's maximum prompt length is 256000 but the request contains 1500000",
            "The prompt is too long for this model's context window.",
            "none: The prompt is too long for this model's context window.",
            "This model's maximum context length is 200000 tokens",
            "invalid_request_error: prompt is too long: 300000 tokens > 200000 maximum",
            "error type: context_length_exceeded",
        ] {
            assert!(is_context_length_error(msg), "should match: {msg}");
        }
        for msg in ["rate limited", "internal server error", "connection reset"] {
            assert!(!is_context_length_error(msg), "should not match: {msg}");
        }
        // The method delegates for the Api/StreamError variants.
        let api = InferenceError::Api {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "none: The prompt is too long for this model's context window.".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
            diagnostics: None,
        };
        assert!(api.is_context_length_error());
        assert!(
            InferenceError::StreamError {
                error_type: "overloaded_error".into(),
                message: "prompt is too long".into(),
            }
            .is_context_length_error()
        );
        assert!(!InferenceError::Auth("nope".into()).is_context_length_error());
    }

    #[test]
    fn serialization_message_stays_serialization_and_non_retryable() {
        let err = InferenceError::serialization_message("bad payload at line 1 column 7");
        assert!(matches!(err, InferenceError::Serialization(_)));
        assert!(!err.is_retryable());
        assert!(err.to_string().contains("bad payload at line 1 column 7"));
    }

    #[test]
    fn serialization_from_rendered_round_trips_display() {
        // Derived from a REAL error's Display so a template rewording cannot
        // silently desynchronize the strip from the prefix it mirrors.
        let original =
            InferenceError::Serialization(serde_json::from_str::<i32>("not a number").unwrap_err());
        let rendered = original.to_string();
        let rebuilt = InferenceError::serialization_from_rendered(&rendered);
        assert!(matches!(rebuilt, InferenceError::Serialization(_)));
        assert!(!rebuilt.is_retryable());
        assert_eq!(
            rebuilt.to_string(),
            rendered,
            "rendered Display must round-trip without double-prefixing"
        );
        // Bare (non-rendered) input gains the prefix exactly once.
        assert_eq!(
            InferenceError::serialization_from_rendered("bare message").to_string(),
            format!("{SERIALIZATION_DISPLAY_PREFIX}bare message"),
        );
    }

    #[test]
    fn idle_timeout_is_not_retryable() {
        let err = InferenceError::IdleTimeout { elapsed_secs: 300 };
        assert!(
            !err.is_retryable(),
            "IdleTimeout must not be retried — would cause 3× amplification"
        );
    }

    #[test]
    fn event_stream_error_is_retryable() {
        // Verify the existing contract hasn't changed — EventStreamError is retryable.
        let err = InferenceError::EventStreamError("connection reset".into());
        assert!(err.is_retryable());
    }

    #[test]
    fn idle_timeout_display() {
        let err = InferenceError::IdleTimeout { elapsed_secs: 120 };
        let msg = err.to_string();
        assert!(
            msg.contains("120s"),
            "Display should include elapsed_secs: {msg}"
        );
    }

    #[test]
    fn try_parse_stream_error_flat_format() {
        let data = r#"{"code":"The service is currently unavailable","error":"Service temporarily unavailable. The model did not respond to this request."}"#;
        let err = try_parse_stream_error(data).expect("should parse flat error");
        match err {
            InferenceError::StreamError {
                error_type,
                message,
            } => {
                assert_eq!(error_type, "The service is currently unavailable");
                assert_eq!(
                    message,
                    "Service temporarily unavailable. The model did not respond to this request."
                );
            }
            other => panic!("expected StreamError, got {other:?}"),
        }
    }

    #[test]
    fn try_parse_stream_error_valid_chunk_returns_none() {
        let data = r#"{"id":"abc","object":"chat.completion.chunk","created":0,"model":"test","choices":[]}"#;
        assert!(
            try_parse_stream_error(data).is_none(),
            "valid chunk should not be parsed as error"
        );
    }

    #[test]
    fn try_parse_stream_error_openrouter_finish_reason_error() {
        // Hybrid mid-stream error chunk (OpenRouter docs shape).
        let data = r#"{
            "id":"gen-abc123",
            "object":"chat.completion.chunk",
            "created":1234567890,
            "model":"z-ai/glm-5.2",
            "provider":"Z.AI",
            "error":{
                "code":429,
                "message":"Rate limit exceeded",
                "metadata":{"error_type":"rate_limit_exceeded"}
            },
            "choices":[{
                "index":0,
                "delta":{"content":""},
                "finish_reason":"error"
            }]
        }"#;
        let err = try_parse_stream_error(data).expect("hybrid error chunk must parse");
        match err {
            InferenceError::StreamError {
                error_type,
                message,
            } => {
                assert_eq!(error_type, "rate_limit_exceeded");
                assert_eq!(message, "Rate limit exceeded");
            }
            other => panic!("expected StreamError, got {other:?}"),
        }
    }

    #[test]
    fn try_parse_stream_error_finish_reason_error_without_envelope() {
        let data = r#"{
            "id":"gen-x",
            "object":"chat.completion.chunk",
            "created":1,
            "model":"z-ai/glm-5.2",
            "choices":[{"index":0,"delta":{},"finish_reason":"error"}]
        }"#;
        let err = try_parse_stream_error(data).expect("finish_reason=error alone must parse");
        match err {
            InferenceError::StreamError { error_type, .. } => {
                assert_eq!(error_type, "finish_reason_error");
            }
            other => panic!("expected StreamError, got {other:?}"),
        }
    }

    #[test]
    fn parse_error_bytes_flat_format() {
        let bytes =
            br#"{"code":"The service is currently unavailable","error":"Service temporarily unavailable."}"#;
        let msg = parse_error_bytes(bytes);
        assert_eq!(
            msg,
            "The service is currently unavailable: Service temporarily unavailable."
        );
    }

    #[test]
    fn parse_error_bytes_rejects_non_json_body() {
        let html = br#"<!DOCTYPE html>
<html lang="en-US">
<head><title>grok.com | 524: A timeout occurred</title></head>
<body><h1>A timeout occurred Error code 524</h1></body>
</html>"#;
        let msg = parse_error_bytes(html);
        assert_eq!(msg, "upstream error");
        // Plain non-JSON text is also rejected (no body sniffing).
        assert_eq!(
            parse_error_bytes(b"some random gateway text"),
            "upstream error"
        );
    }

    #[test]
    fn user_facing_api_error_message_maps_non_json_by_status() {
        let html = br#"<!DOCTYPE html><html><body>timeout</body></html>"#;
        let msg = user_facing_api_error_message(StatusCode::from_u16(524).unwrap(), html);
        assert_eq!(msg, status_user_message(StatusCode::from_u16(524).unwrap()));

        let msg_503 =
            user_facing_api_error_message(StatusCode::SERVICE_UNAVAILABLE, b"not json either");
        assert_eq!(
            msg_503,
            status_user_message(StatusCode::SERVICE_UNAVAILABLE)
        );
    }

    #[test]
    fn user_facing_keeps_json_error_message() {
        let bytes = br#"{"error":{"message":"rate limit exceeded","type":"rate_limit_error"}}"#;
        let msg = user_facing_api_error_message(StatusCode::TOO_MANY_REQUESTS, bytes);
        assert_eq!(msg, "rate_limit_error: rate limit exceeded");
    }

    #[test]
    fn structured_error_message_is_length_capped() {
        let long_msg = "x".repeat(MAX_USER_ERROR_BODY_CHARS + 50);
        let bytes = format!(r#"{{"error":{{"message":"{long_msg}","type":"server_error"}}}}"#);
        let msg = parse_error_bytes(bytes.as_bytes());
        assert!(msg.chars().count() <= MAX_USER_ERROR_BODY_CHARS + 1);
        // Head + tail truncation: the message keeps both ends with a single
        // ellipsis separator in the middle, so it neither starts nor ends
        // with the ellipsis.
        assert!(msg.contains('\u{2026}'));
        assert!(!msg.starts_with('\u{2026}'));
        assert!(!msg.ends_with('\u{2026}'));
        assert!(msg.starts_with('x'));
        assert!(msg.ends_with('x'));
    }

    /// Regression test: 403 Forbidden must NOT be classified as an auth
    /// error. The proxy returns 403 for policy denials that are unrelated
    /// to the caller's credentials (content-safety blocks, ZDR-gated
    /// operations, or other usage-policy blocks). Misclassifying these as
    /// auth errors triggers a pointless OIDC
    /// refresh and surfaces as acp::Error::auth_required on the client,
    /// tearing down the session and risking an
    /// `invalid_grant_threshold`-triggered wipe of auth.json.
    #[test]
    fn forbidden_is_not_auth_error() {
        let err = InferenceError::Api {
            status: StatusCode::FORBIDDEN,
            message: "Content violates usage guidelines.".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
            diagnostics: None,
        };
        assert!(
            !err.is_auth_error(),
            "403 Forbidden must not be treated as an auth error"
        );
    }

    #[test]
    fn unauthorized_is_auth_error() {
        let err = InferenceError::Api {
            status: StatusCode::UNAUTHORIZED,
            message: "Invalid or expired credentials".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
            diagnostics: None,
        };
        assert!(
            err.is_auth_error(),
            "401 Unauthorized must be an auth error"
        );
    }

    #[test]
    fn auth_variant_is_auth_error() {
        let err = InferenceError::Auth("bad key".into());
        assert!(err.is_auth_error());
    }

    #[test]
    fn rate_limited_api_error_is_detected() {
        let err = InferenceError::Api {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: "Rate limit exceeded".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
            diagnostics: None,
        };
        assert!(err.is_rate_limited());
        assert!(err.is_retryable(), "429 should be retryable");
        assert!(!err.is_auth_error());
        assert!(!err.is_payload_too_large());
    }

    #[test]
    fn non_rate_limit_errors_are_not_rate_limited() {
        let server_error = InferenceError::Api {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "internal".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
            diagnostics: None,
        };
        assert!(!server_error.is_rate_limited());

        let auth_error = InferenceError::Auth("bad key".into());
        assert!(!auth_error.is_rate_limited());

        let timeout = InferenceError::IdleTimeout { elapsed_secs: 30 };
        assert!(!timeout.is_rate_limited());
    }

    #[test]
    fn retry_after_returns_header_value() {
        let err = InferenceError::Api {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: "slow down".into(),
            model_metadata: None,
            retry_after_secs: Some(42),
            should_retry: None,
            diagnostics: None,
        };
        assert_eq!(err.retry_after(), Some(42));
    }

    #[test]
    fn retry_after_returns_none_when_absent() {
        let err = InferenceError::Api {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: "slow down".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
            diagnostics: None,
        };
        assert_eq!(err.retry_after(), None);
    }

    #[test]
    fn retry_after_returns_none_for_non_api_errors() {
        assert_eq!(InferenceError::Auth("x".into()).retry_after(), None);
        assert_eq!(
            InferenceError::IdleTimeout { elapsed_secs: 10 }.retry_after(),
            None
        );
    }

    #[test]
    fn encrypted_content_400_is_detected() {
        let err = InferenceError::Api {
            status: StatusCode::BAD_REQUEST,
            message: "Could not decrypt the provided encrypted_content. Ensure the value is the unmodified encrypted_content from a previous response.".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
            diagnostics: None,
        };
        assert!(err.is_encrypted_content_error());
        assert!(
            !err.is_retryable(),
            "encrypted_content errors must not be retried"
        );
    }

    #[test]
    fn encrypted_content_wrong_status_not_detected() {
        let err = InferenceError::Api {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "encrypted_content decryption failed".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
            diagnostics: None,
        };
        assert!(
            !err.is_encrypted_content_error(),
            "only 400 should match, not 500"
        );
    }

    #[test]
    fn encrypted_content_unrelated_400_not_detected() {
        let err = InferenceError::Api {
            status: StatusCode::BAD_REQUEST,
            message: "Invalid model parameter".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
            diagnostics: None,
        };
        assert!(
            !err.is_encrypted_content_error(),
            "unrelated 400 errors must not match"
        );
    }

    #[test]
    fn image_processing_error_direct_400_detected() {
        let err = InferenceError::Api {
            status: StatusCode::BAD_REQUEST,
            message: "Could not process image: unsupported format".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
            diagnostics: None,
        };
        assert!(err.is_image_processing_error());
        assert!(!err.is_encrypted_content_error());
    }

    #[test]
    fn image_processing_error_500_wrapped_detected() {
        let err = InferenceError::Api {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "upstream error: 400 Bad Request: Could not process image".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
            diagnostics: None,
        };
        assert!(err.is_image_processing_error());
    }

    #[test]
    fn image_processing_error_unrelated_400_not_detected() {
        let err = InferenceError::Api {
            status: StatusCode::BAD_REQUEST,
            message: "Invalid model parameter".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
            diagnostics: None,
        };
        assert!(!err.is_image_processing_error());
    }

    #[test]
    fn image_processing_error_unrelated_500_not_detected() {
        let err = InferenceError::Api {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "internal server error".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
            diagnostics: None,
        };
        assert!(!err.is_image_processing_error());
    }

    #[test]
    fn image_processing_error_wrong_status_not_detected() {
        let err = InferenceError::Api {
            status: StatusCode::BAD_GATEWAY,
            message: "Could not process image".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
            diagnostics: None,
        };
        assert!(
            !err.is_image_processing_error(),
            "only 400 and 500 should match"
        );
    }

    #[test]
    fn image_processing_error_400_is_not_retryable_standalone() {
        let err = InferenceError::Api {
            status: StatusCode::BAD_REQUEST,
            message: "Could not process image".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
            diagnostics: None,
        };
        assert!(
            !err.is_retryable(),
            "direct 400 must not be retryable by is_retryable()"
        );
    }

    // ── Provider-aware error copy (Phase 2 — OpenRouter compatibility) ──

    /// 502-class status copy names the resolved provider label. xAI keeps the
    /// historical "Grok" wording; non-xAI providers never say "Grok".
    #[test]
    fn status_user_message_for_502_is_provider_aware() {
        let xai = status_user_message_for(StatusCode::BAD_GATEWAY, "Grok");
        assert!(xai.contains("Grok is temporarily unavailable"));
        assert!(xai.contains("502"));

        let openrouter = status_user_message_for(StatusCode::BAD_GATEWAY, "OpenRouter");
        assert!(openrouter.contains("OpenRouter is temporarily unavailable"));
        assert!(!openrouter.contains("Grok"));

        let openai = status_user_message_for(StatusCode::BAD_GATEWAY, "OpenAI");
        assert!(openai.contains("OpenAI is temporarily unavailable"));
        assert!(!openai.contains("Grok"));

        let custom = status_user_message_for(StatusCode::BAD_GATEWAY, "the model provider");
        assert!(custom.contains("the model provider is temporarily unavailable"));
        assert!(!custom.contains("Grok"));
    }

    /// 520-class (Cloudflare edge) status copy names the resolved provider.
    #[test]
    fn status_user_message_for_520_is_provider_aware() {
        let openrouter = status_user_message_for(StatusCode::from_u16(524).unwrap(), "OpenRouter");
        assert!(openrouter.contains("Connection to OpenRouter timed out"));
        assert!(!openrouter.contains("Grok"));

        let xai = status_user_message_for(StatusCode::from_u16(520).unwrap(), "Grok");
        assert!(xai.contains("Connection to Grok timed out"));
    }

    /// HTTP 402 produces a dedicated credits message naming the provider.
    #[test]
    fn status_user_message_for_402_dedicated_credits_message() {
        let openrouter = status_user_message_for(StatusCode::PAYMENT_REQUIRED, "OpenRouter");
        assert!(openrouter.contains("OpenRouter account out of credits"));
        assert!(openrouter.contains("add credits"));
        assert!(openrouter.contains("HTTP 402"));
        assert!(!openrouter.contains("Grok"));

        let openai = status_user_message_for(StatusCode::PAYMENT_REQUIRED, "OpenAI");
        assert!(openai.contains("OpenAI account out of credits"));
    }

    /// `status_user_message` (legacy entry point) keeps the xAI "Grok" wording
    /// so existing callers and tests are unaffected.
    #[test]
    fn status_user_message_legacy_keeps_grok_label() {
        let msg = status_user_message(StatusCode::BAD_GATEWAY);
        assert!(msg.contains("Grok is temporarily unavailable"));
    }

    /// `user_facing_api_error_message_for` prefers structured JSON envelopes
    /// over status copy, even for 402 — the dedicated 402 message only applies
    /// to non-JSON bodies.
    #[test]
    fn user_facing_message_for_402_structured_body_wins() {
        let bytes =
            br#"{"error":{"message":"insufficient credits","type":"insufficient_credits"}}"#;
        let msg =
            user_facing_api_error_message_for(StatusCode::PAYMENT_REQUIRED, bytes, "OpenRouter");
        assert_eq!(msg, "insufficient_credits: insufficient credits");
    }

    #[test]
    fn user_facing_message_for_402_non_json_uses_dedicated_copy() {
        let msg = user_facing_api_error_message_for(
            StatusCode::PAYMENT_REQUIRED,
            b"not json",
            "OpenRouter",
        );
        assert!(msg.contains("OpenRouter account out of credits"));
        assert!(msg.contains("add credits"));
    }

    /// Head + tail truncation: when a structured message exceeds the cap,
    /// both the beginning and the end are preserved with an ellipsis
    /// between. OpenRouter bodies front-load boilerplate and put the
    /// useful upstream message at the end.
    #[test]
    fn truncate_user_error_preserves_head_and_tail() {
        let prefix = "A".repeat(200);
        let suffix = "B".repeat(200);
        // Use a separator that stays valid inside a JSON string (no raw
        // control chars); the truncation is exercised regardless of the
        // separator.
        let long = format!("{prefix}-SEP-{suffix}");
        let bytes = format!(
            r#"{{"error":{{"message":"{}","type":"server_error"}}}}"#,
            long
        );
        let msg = parse_error_bytes(bytes.as_bytes());
        // Stays under the cap (+1 slack for the ellipsis).
        assert!(msg.chars().count() <= MAX_USER_ERROR_BODY_CHARS + 1);
        // `error_type` is "server_error", so the message passes through
        // without the "<type>: " prefix. The head begins with the prefix
        // run of 'A's and the tail ends with the suffix run of 'B's,
        // separated by a single ellipsis.
        assert!(msg.starts_with('A'));
        assert!(msg.ends_with('B'));
        assert_eq!(msg.matches('\u{2026}').count(), 1);
        // Neither end is the ellipsis.
        assert!(!msg.starts_with('\u{2026}'));
        assert!(!msg.ends_with('\u{2026}'));
    }

    /// Short structured messages pass through unchanged (no ellipsis).
    #[test]
    fn truncate_user_error_short_message_unchanged() {
        let bytes = br#"{"error":{"message":"short","type":"server_error"}}"#;
        let msg = parse_error_bytes(bytes);
        assert_eq!(msg, "short");
    }

    // --- parse_rate_limit_reset ---

    #[test]
    fn parse_reset_none_for_absent() {
        assert_eq!(parse_rate_limit_reset(None), None);
        assert_eq!(parse_rate_limit_reset(Some("")), None);
        assert_eq!(parse_rate_limit_reset(Some("   ")), None);
    }

    #[test]
    fn parse_reset_delta_seconds() {
        assert_eq!(parse_rate_limit_reset(Some("30")), Some(30));
        assert_eq!(parse_rate_limit_reset(Some("0")), Some(0));
        assert_eq!(parse_rate_limit_reset(Some("  5  ")), Some(5));
    }

    #[test]
    fn parse_reset_clamps_to_max() {
        // A delta above RATE_LIMIT_RESET_MAX_SECS clamps down to it.
        assert_eq!(
            parse_rate_limit_reset(Some("3600")),
            Some(RATE_LIMIT_RESET_MAX_SECS)
        );
    }

    #[test]
    fn parse_reset_past_epoch_clamps_to_zero() {
        // An epoch far in the past (above the 1_000_000_000 threshold so it
        // is interpreted as epoch seconds, not delta-seconds) resolves to a
        // zero delta. 1_500_000_000 ≈ 2017-07-14, well before now.
        assert_eq!(parse_rate_limit_reset(Some("1500000000")), Some(0));
    }

    #[test]
    fn parse_reset_future_epoch_is_delta() {
        // An epoch ~120s in the future resolves to roughly 120s (allow
        // a couple seconds of wall-clock slack).
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let future = now + 120;
        let parsed = parse_rate_limit_reset(Some(&future.to_string())).unwrap();
        assert!(parsed >= 118 && parsed <= 120, "got {parsed}");
    }

    #[test]
    fn parse_reset_rejects_non_integer() {
        // HTTP-dates and any non-numeric form yield None.
        assert_eq!(
            parse_rate_limit_reset(Some("Wed, 21 Oct 2015 07:28:00 GMT")),
            None
        );
        assert_eq!(parse_rate_limit_reset(Some("abc")), None);
        assert_eq!(parse_rate_limit_reset(Some("12.5")), None);
    }

    #[test]
    fn api_error_diagnostics_is_empty_excludes_parsed_reset() {
        // The new field participates in is_empty (non-None => not empty).
        let with_reset = ApiErrorDiagnostics {
            rate_limit_reset_secs: Some(10),
            ..Default::default()
        };
        assert!(!with_reset.is_empty());
        assert_eq!(with_reset.parsed_reset_secs(), Some(10));

        let empty = ApiErrorDiagnostics::default();
        assert!(empty.is_empty());
        assert_eq!(empty.parsed_reset_secs(), None);
    }

    #[test]
    fn rate_limit_reset_secs_accessor_reads_diagnostics() {
        let err = InferenceError::Api {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: "slow".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
            diagnostics: Some(ApiErrorDiagnostics {
                rate_limit_reset_secs: Some(42),
                ..Default::default()
            }),
        };
        assert_eq!(err.rate_limit_reset_secs(), Some(42));

        let err_no_diag = InferenceError::Api {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: "slow".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
            diagnostics: None,
        };
        assert_eq!(err_no_diag.rate_limit_reset_secs(), None);
    }

    /// Older payloads that omit `rate_limit_reset_secs` keep deserializing
    /// (serde-additive field defaults to None).
    #[test]
    fn api_error_diagnostics_legacy_payload_deserializes() {
        let legacy =
            r#"{"rate_limit_limit":"100","rate_limit_remaining":"0","rate_limit_reset":"30"}"#;
        let d: ApiErrorDiagnostics = serde_json::from_str(legacy).unwrap();
        assert_eq!(d.rate_limit_limit.as_deref(), Some("100"));
        assert_eq!(d.rate_limit_reset.as_deref(), Some("30"));
        assert_eq!(d.rate_limit_reset_secs, None);
    }
}
