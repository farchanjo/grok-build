//! Retry classification, backoff, and decision-making.
//!
//! Pure logic only: no I/O, no notifications, no logging side-effects.
//! The actor (M4) wraps this with the actual retry loop.
//!
//! # Retry behavior summary
//!
//! **Retried** (up to [`DEFAULT_MAX_RETRIES`] = 15, ~6 min with 30s backoff cap):
//! - 500, 502, 503, 504, 520 (server errors)
//! - Connection errors (timeout, refused, reset)
//! - `EventStreamError` / `StreamError` (mid-stream failures)
//! - `EmptyResponse` (model returned no content/tool calls)
//!
//! **Retried with lower cap** ([`RATE_LIMIT_RETRY_THRESHOLD`] = 2):
//! - 429 (rate limited) — avoids burning long waits
//!
//! **Special handling** (not counted against retry budget):
//! - 413 / image processing errors → strip images and retry once
//!
//! **Not retried** (Fatal immediately):
//! - 400, 401, 403, 404, 408, 422 (client errors)
//! - `Auth` / `InvalidConfiguration` (credential/config issues)
//! - `IdleTimeout` (model stuck, retry would stall again)
//! - `Serialization` (response parsing failure)
//! - `MaxTokensTruncation` (by design)
//!
//! **Server hint** (`x-should-retry` header from CCP):
//! - `false` → Fatal immediately, regardless of status code
//! - `true` / absent → falls through to status-code logic above
//!
//! Today CCP's header mirrors the client's `is_retryable()` logic
//! (4xx except 429 = false, 5xx + 429 = true), so no behavior changes
//! on merge. The header enables future CCP-side refinements (e.g.
//! marking content-caused 500s as non-retryable) without client updates.

use std::time::Duration;

use xai_grok_inference_types::InferenceError;

/// After this many rate-limit (429) retries, escalate to the caller
/// instead of waiting again. Rate-limit waits can be long and there is
/// no point burning a long backoff just to be rate-limited again.
pub const RATE_LIMIT_RETRY_THRESHOLD: u32 = 2;

/// Default 429 retry cap for OpenRouter. OpenRouter enforces per-model
/// limits and a tool-heavy turn can burst several requests in quick
/// succession, so a slightly higher cap than the generic threshold gives
/// the reset window a chance to elapse before giving up. Overridable via
/// the `GROK_OPENROUTER_RATE_LIMIT_RETRIES` environment variable.
pub const OPENROUTER_RATE_LIMIT_RETRY_THRESHOLD: u32 = 3;

/// Resolve the per-provider 429 retry cap.
///
/// OpenRouter honours `GROK_OPENROUTER_RATE_LIMIT_RETRIES` (default
/// [`OPENROUTER_RATE_LIMIT_RETRY_THRESHOLD`]); every other provider keeps
/// the generic [`RATE_LIMIT_RETRY_THRESHOLD`]. A non-positive parsed env
/// value falls back to the default so a misconfiguration can't silently
/// disable all 429 retries.
pub fn resolve_rate_limit_threshold(
    provider_identity: crate::config::ProviderIdentity,
    env_override: Option<&str>,
) -> u32 {
    use crate::config::ProviderIdentity;
    match provider_identity {
        ProviderIdentity::OpenRouter => env_override
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(OPENROUTER_RATE_LIMIT_RETRY_THRESHOLD),
        _ => RATE_LIMIT_RETRY_THRESHOLD,
    }
}

/// Default max retries when no env or model override is set.
/// With 30s backoff cap this gives ~6 min of retry budget:
/// retries 1-4 are exponential (2s+4s+8s+16s ≈ 30s), retries
/// 5-15 are flat at ~30s each (≈ 5.5 min).
pub const DEFAULT_MAX_RETRIES: u32 = 15;

/// Longest single wait on the generic transport/5xx retry path. The 429
/// path intentionally keeps the server's rate-limit wait and uses its lower
/// retry threshold instead.
pub const MAX_RETRY_BACKOFF: Duration = Duration::from_secs(30);

/// Resolve max API retries from an optional env override, model config,
/// or default ([`DEFAULT_MAX_RETRIES`]).
pub(crate) fn resolve_max_retries_with_env(
    env_override: Option<&str>,
    model_max_retries: Option<u32>,
) -> u32 {
    env_override
        .and_then(|value| value.parse::<u32>().ok())
        .or(model_max_retries)
        .unwrap_or(DEFAULT_MAX_RETRIES)
}

/// Resolve max API retries: `GROK_MAX_RETRIES` env > model config > default ([`DEFAULT_MAX_RETRIES`]).
pub fn resolve_max_retries(model_max_retries: Option<u32>) -> u32 {
    let env_override = std::env::var("GROK_MAX_RETRIES").ok();
    resolve_max_retries_with_env(env_override.as_deref(), model_max_retries)
}

/// Backoff for doom-loop resamples: near-immediate with a small jitter.
/// Loops are stochastic at sampling temperature, so a fresh sample is the
/// remedy — waiting buys nothing beyond de-syncing concurrent resamples.
pub fn doom_loop_backoff(retry_count: u32) -> Duration {
    use std::hash::{Hash, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};

    static JITTER_SEQ: AtomicU64 = AtomicU64::new(0);

    let mut hasher = std::hash::DefaultHasher::new();
    JITTER_SEQ.fetch_add(1, Ordering::Relaxed).hash(&mut hasher);
    retry_count.hash(&mut hasher);
    Duration::from_millis(hasher.finish() % 251)
}

/// Exponential backoff (2s, 4s, 8s, ..., capped 30s) with +/-20% jitter
/// to prevent thundering-herd retry storms.
pub fn retry_backoff_with_jitter(retry_count: u32) -> Duration {
    let shift = retry_count.saturating_sub(1);
    let base_ms = 2000u64
        .checked_shl(shift)
        .unwrap_or(u64::MAX)
        .min(MAX_RETRY_BACKOFF.as_millis() as u64);
    jittered(Duration::from_millis(base_ms))
}

fn jittered(base: Duration) -> Duration {
    use std::hash::{Hash, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};

    static JITTER_SEQ: AtomicU64 = AtomicU64::new(0);
    let base_ms = base.as_millis() as u64;
    let jitter_range = base_ms / 5;
    let mut hasher = std::hash::DefaultHasher::new();
    JITTER_SEQ.fetch_add(1, Ordering::Relaxed).hash(&mut hasher);
    std::thread::current().id().hash(&mut hasher);
    let jitter = hasher.finish() % (jitter_range * 2 + 1);
    Duration::from_millis(base_ms - jitter_range + jitter)
}

/// Resolve the backoff for a 429 (rate-limited) retry.
///
/// Order of precedence:
/// 1. `Retry-After` header (`retry_after_secs`) — the standard, most
///    authoritative source;
/// 2. parsed `x-ratelimit-reset` (`rate_limit_reset_secs`) — used by
///    OpenRouter/proxies that report the actual reset window;
/// 3. exponential jitter — the fallback when the server gave no hint.
///
/// `next_attempt` is the 1-based retry index, used only for the jitter
/// fallback. The function is pure and performs no I/O.
pub fn rate_limit_backoff(err: &InferenceError, next_attempt: u32) -> Duration {
    err.retry_after()
        .map(Duration::from_secs)
        .or_else(|| err.rate_limit_reset_secs().map(Duration::from_secs))
        .unwrap_or_else(|| retry_backoff_with_jitter(next_attempt))
}

/// What the actor should do next given a sampling error and retry context.
///
/// Pure data: callers (the actor's per-request task) are responsible for
/// performing the actual sleep, image strip, client rebuild, or emit.
#[derive(Debug)]
pub enum RetryDecision {
    /// Retry with exponential backoff (transport errors, 5xx,
    /// empty responses).
    Retry { backoff: Duration },

    /// Retry honoring the server's `Retry-After` header (429 rate
    /// limits). `is_rate_limited` distinguishes 429s from generic
    /// retry-with-backoff cases for telemetry.
    RetryWithBackoff {
        backoff: Duration,
        is_rate_limited: bool,
    },

    /// Retry after stripping inline images from the request (413
    /// Payload Too Large or image processing rejection).
    RetryWithImageStrip,

    /// Retry after rebuilding the HTTP client with HTTP/1.1 (transport
    /// error, first retry only).
    RetryWithClientRebuild { backoff: Duration },

    /// Emit the error to the session and let it decide what to do
    /// (auth refresh, encrypted-content mismatch).
    EmitToSession(InferenceError),

    /// Fatal: no further retries possible. Surface to the caller as the
    /// final outcome of the sampling request.
    Fatal(InferenceError),
}

/// Classify a sampling error into a [`RetryDecision`].
///
/// `retry_count` is the number of retries already performed (0 on first
/// failure). `max_retries` is the total budget. `rate_limit_threshold`
/// caps consecutive 429 retries (see [`RATE_LIMIT_RETRY_THRESHOLD`]).
///
/// The function is pure: it does not sleep, log, or perform I/O.
pub fn classify_error(
    err: &InferenceError,
    retry_count: u32,
    max_retries: u32,
    rate_limit_threshold: u32,
) -> RetryDecision {
    // Auth and encrypted-content errors are session-owned. The sampler
    // surfaces the raw error and lets the session refresh credentials
    // or show a friendly message.
    if err.is_auth_error() {
        return RetryDecision::EmitToSession(clone_error(err));
    }
    if err.is_encrypted_content_error() {
        return RetryDecision::EmitToSession(clone_error(err));
    }
    if max_retries == 0 {
        return RetryDecision::Fatal(clone_error(err));
    }

    // 413 Payload Too Large: strip inline images and try once. The
    // caller checks if there are images left after the strip; if not,
    // upgrade to Fatal.
    if err.is_payload_too_large() {
        return RetryDecision::RetryWithImageStrip;
    }

    // Image processing errors (direct 400 or proxy-wrapped 500): strip
    // images and retry, same recovery as 413.
    if err.is_image_processing_error() {
        return RetryDecision::RetryWithImageStrip;
    }

    // Server explicitly said don't retry (x-should-retry: false).
    // Trust the server — it knows if the error is request-content-caused
    // (e.g. malformed tool call in conversation history) vs transient.
    //
    // x-should-retry: true is intentionally NOT handled here — we only
    // use the header to suppress retries (false), not to force them
    // (true). Forcing retries on non-retryable status codes could
    // amplify failures. true falls through to existing status-code logic.
    //
    // Checked AFTER image-strip guards: image stripping changes the
    // request payload, so a server "don't retry" on the original
    // request doesn't apply to the stripped request.
    if let Some(false) = err.should_retry_header() {
        return RetryDecision::Fatal(clone_error(err));
    }

    // Context-window / size overflow is deterministic — re-sending the same (or
    // larger) payload always fails — so never retry it, whatever status the backend
    // used (in-stream `ResponseError`→500, HTTP 400/500, OpenAI/Anthropic variants).
    if err.is_context_length_error() {
        return RetryDecision::Fatal(clone_error(err));
    }

    // Doom-loop failures: always Retry with near-immediate backoff. The
    // recovery loop intercepts these BEFORE classification and runs its own
    // budget (`policy.max_retries`, enforced by disarming the abort); this
    // arm only keeps classification total so a stray doom failure through
    // any other path can never be Fatal.
    if matches!(err, InferenceError::DoomLoopDetected { .. }) {
        return RetryDecision::Retry {
            backoff: doom_loop_backoff(retry_count + 1),
        };
    }

    // Rate-limited (429): cap retries at the rate-limit threshold to
    // avoid burning long waits.
    if err.is_rate_limited() {
        let next_attempt = retry_count + 1;
        let effective_cap = max_retries.min(rate_limit_threshold);
        if effective_cap == 0 {
            return RetryDecision::Fatal(clone_error(err));
        }
        if next_attempt >= effective_cap {
            return RetryDecision::Fatal(clone_error(err));
        }
        let backoff = rate_limit_backoff(err, next_attempt);
        return RetryDecision::RetryWithBackoff {
            backoff,
            is_rate_limited: true,
        };
    }

    // Generic retryable transport / 5xx errors. First retry rebuilds
    // the HTTP client with HTTP/1.1 to escape poisoned HTTP/2 pools;
    // later retries just back off.
    if err.is_retryable() {
        let next_attempt = retry_count + 1;
        if max_retries == 0 || next_attempt >= max_retries {
            return RetryDecision::Fatal(clone_error(err));
        }
        let backoff = err
            .retry_after()
            .map(|seconds| jittered(Duration::from_secs(seconds).min(MAX_RETRY_BACKOFF)))
            .unwrap_or_else(|| retry_backoff_with_jitter(next_attempt));
        if next_attempt == 1 {
            return RetryDecision::RetryWithClientRebuild { backoff };
        }
        return RetryDecision::Retry { backoff };
    }

    // Everything else is fatal.
    RetryDecision::Fatal(clone_error(err))
}

/// Build a human-readable, telemetry-friendly description of a sampling
/// error.
///
/// `retry_count`, when present, is rendered as a "Request failed after
/// N retries." prefix. The function is pure string formatting: no
/// logging, no I/O, no allocation beyond the produced `String`.
pub fn format_inference_error(err: &InferenceError, retry_count: Option<u32>) -> String {
    let retry_prefix = match retry_count {
        Some(count) => format!("Request failed after {} retries. ", count),
        None => String::new(),
    };

    match err {
        InferenceError::Auth { message, .. } => {
            format!(
                "{}Authentication failed: {}. Please check your API key configuration.",
                retry_prefix, message
            )
        }
        InferenceError::InvalidConfiguration(msg) => {
            format!(
                "{}Invalid configuration: {}. Please check your model settings.",
                retry_prefix, msg
            )
        }

        InferenceError::Http(e) => {
            let mut details = Vec::new();
            if e.is_timeout() {
                details.push("timeout".to_string());
            }
            if e.is_connect() {
                details.push("connection failed".to_string());
            }
            if let Some(status) = e.status() {
                details.push(format!("status {}", status));
            }
            if let Some(url) = e.url() {
                details.push(format!("url: {}", url));
            }
            let detail_str = if details.is_empty() {
                e.to_string()
            } else {
                format!("{} ({})", e, details.join(", "))
            };
            format!(
                "{}HTTP request failed: {}. This may be a network issue or the API endpoint may be unavailable.",
                retry_prefix, detail_str
            )
        }
        InferenceError::Serialization(e) => {
            format!(
                "{}Failed to parse API response at line {} column {}: {}. This indicates an unexpected response format from the server.",
                retry_prefix,
                e.line(),
                e.column(),
                e
            )
        }
        InferenceError::Api {
            status,
            message,
            diagnostics,
            ..
        } => {
            // Prefer the diagnostics provider_name (the selected OpenRouter
            // upstream) over the generic provider label derived from the
            // config's ProviderIdentity. This makes 502/520-class and 402
            // copy name the actual upstream that failed rather than the
            // router when that information is available.
            let provider_label = diagnostics
                .as_ref()
                .and_then(|d| d.provider_name.as_deref())
                .filter(|name| !name.is_empty());
            let status_hint = match status.as_u16() {
                400 => " (bad request - check your input)",
                401 | 403 => " (authentication issue - check your API key)",
                404 => " (endpoint not found - check model configuration)",
                413 => " (request too large - try /compact or start new session)",
                429 => " (rate limited - please wait and retry)",
                500 => " (server internal error)",
                #[allow(clippy::manual_range_patterns)]
                502 | 503 | 504 => " (server unavailable - please retry)",
                402 => match provider_label {
                    Some(name) => {
                        return format!(
                            "{retry_prefix}{name} account out of credits — add credits to continue. (HTTP 402): {message}"
                        );
                    }
                    None => " (out of credits - add credits to continue)",
                },
                _ => "",
            };
            format!(
                "{}API error (HTTP {}{}): {}",
                retry_prefix,
                status.as_u16(),
                status_hint,
                message
            )
        }
        InferenceError::EventStreamError(msg) => {
            format!(
                "{}Event stream error: {}. The connection to the server was interrupted.",
                retry_prefix, msg
            )
        }
        InferenceError::StreamError {
            error_type,
            message,
            ..
        } => {
            format!(
                "{}Server stream error ({}): {}. The server encountered an error while streaming the response.",
                retry_prefix, error_type, message
            )
        }
        InferenceError::IdleTimeout { elapsed_secs } => {
            format!(
                "{}Model stopped responding after {}s. The model may be overloaded or stuck. Try again or use a different model.",
                retry_prefix, elapsed_secs
            )
        }
        InferenceError::EmptyResponse { context } => {
            format!(
                "{}Empty response from model ({}): model={}, had_reasoning={}, finish_reason={}, completion_tokens={}",
                retry_prefix,
                context.reason,
                context.model,
                context.had_reasoning,
                context.finish_reason_str(),
                context.completion_tokens.unwrap_or(0),
            )
        }
        InferenceError::MaxTokensTruncation => {
            format!("{}Response truncated by max_tokens.", retry_prefix)
        }
        InferenceError::DoomLoopDetected { triggers, .. } => {
            format!(
                "{}Server detected a reasoning loop ({}); resampling the response.",
                retry_prefix,
                triggers.join(", ")
            )
        }
    }
}

/// Reconstruct an owned [`InferenceError`] from a borrowed one.
///
/// `InferenceError` does not implement `Clone` because its `Http` and
/// `Serialization` variants wrap non-`Clone` types. The retry loop
/// only borrows the error during classification, then needs to surface
/// it; this helper produces a faithful copy where possible. `Http`
/// falls back to a structured `EventStreamError` (still retryable, like
/// the original transport error). `Serialization` must stay
/// `Serialization`: laundering it into `EventStreamError` would flip a
/// fatal response-parse failure into a retryable one and burn the full
/// retry budget re-generating a response that fails the same way.
pub(crate) fn clone_error(err: &InferenceError) -> InferenceError {
    match err {
        InferenceError::Auth {
            message,
            credential,
        } => InferenceError::Auth {
            message: message.clone(),
            credential: *credential,
        },
        InferenceError::InvalidConfiguration(msg) => InferenceError::InvalidConfiguration(msg),
        InferenceError::Http(e) => {
            // reqwest::Error is not Clone; preserve the rendered message
            // as an EventStreamError (the closest retryable transport
            // variant) so callers see an equivalent description.
            InferenceError::EventStreamError(e.to_string())
        }
        InferenceError::Serialization(e) => {
            // serde_json::Error is not Clone; its Display already carries the
            // original line/column exactly once.
            InferenceError::serialization_message(e)
        }
        InferenceError::Api {
            status,
            message,
            model_metadata,
            retry_after_secs,
            should_retry,
            diagnostics,
            error_code,
        } => InferenceError::Api {
            status: *status,
            message: message.clone(),
            model_metadata: model_metadata.clone(),
            retry_after_secs: *retry_after_secs,
            should_retry: *should_retry,
            diagnostics: diagnostics.clone(),
            error_code: error_code.clone(),
        },
        InferenceError::EventStreamError(msg) => InferenceError::EventStreamError(msg.clone()),
        InferenceError::StreamError {
            error_type,
            message,
            code,
        } => InferenceError::StreamError {
            error_type: error_type.clone(),
            message: message.clone(),
            code: code.clone(),
        },
        InferenceError::IdleTimeout { elapsed_secs } => InferenceError::IdleTimeout {
            elapsed_secs: *elapsed_secs,
        },
        InferenceError::EmptyResponse { context } => InferenceError::EmptyResponse {
            context: context.clone(),
        },
        InferenceError::MaxTokensTruncation => InferenceError::MaxTokensTruncation,
        InferenceError::DoomLoopDetected {
            triggers,
            aborted_at_chunk,
        } => InferenceError::DoomLoopDetected {
            triggers: triggers.clone(),
            aborted_at_chunk: *aborted_at_chunk,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::StatusCode;

    fn api_err(status: StatusCode, message: &str) -> InferenceError {
        InferenceError::Api {
            status,
            message: message.to_string(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
            diagnostics: None,
            error_code: None,
        }
    }

    fn api_err_with_retry_after(status: StatusCode, retry_after: u64) -> InferenceError {
        InferenceError::Api {
            status,
            message: "x".to_string(),
            model_metadata: None,
            retry_after_secs: Some(retry_after),
            should_retry: None,
            diagnostics: None,
            error_code: None,
        }
    }

    fn api_err_with_reset(status: StatusCode, reset_secs: u64) -> InferenceError {
        InferenceError::Api {
            status,
            message: "x".to_string(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
            diagnostics: Some(xai_grok_inference_types::ApiErrorDiagnostics {
                rate_limit_reset_secs: Some(reset_secs),
                ..Default::default()
            }),
            error_code: None,
        }
    }

    fn api_err_with_retry_after_and_reset(
        status: StatusCode,
        retry_after: u64,
        reset_secs: u64,
    ) -> InferenceError {
        InferenceError::Api {
            status,
            message: "x".to_string(),
            model_metadata: None,
            retry_after_secs: Some(retry_after),
            should_retry: None,
            diagnostics: Some(xai_grok_inference_types::ApiErrorDiagnostics {
                rate_limit_reset_secs: Some(reset_secs),
                ..Default::default()
            }),
            error_code: None,
        }
    }

    #[test]
    fn resolve_max_retries_env_override_takes_precedence() {
        assert_eq!(resolve_max_retries_with_env(Some("9"), Some(3)), 9);
    }

    #[test]
    fn resolve_max_retries_falls_back_to_model() {
        assert_eq!(resolve_max_retries_with_env(None, Some(7)), 7);
    }

    #[test]
    fn resolve_max_retries_default() {
        assert_eq!(
            resolve_max_retries_with_env(None, None),
            DEFAULT_MAX_RETRIES
        );
    }

    #[test]
    fn resolve_max_retries_invalid_env_falls_through() {
        assert_eq!(resolve_max_retries_with_env(Some("abc"), Some(4)), 4);
    }

    #[test]
    fn backoff_first_retry_is_around_two_seconds() {
        let backoff = retry_backoff_with_jitter(1);
        // Base 2000ms +/- 20% jitter (400ms range).
        assert!(
            backoff >= Duration::from_millis(1600) && backoff <= Duration::from_millis(2400),
            "first retry backoff out of range: {:?}",
            backoff
        );
    }

    #[test]
    fn backoff_doubles_then_caps_at_thirty_seconds() {
        // retry_count=2: base 4s
        let r2 = retry_backoff_with_jitter(2);
        assert!(r2 >= Duration::from_millis(3200) && r2 <= Duration::from_millis(4800));

        // retry_count=10: base would be 2^10 * 2000 = 2.048s but capped to 30s
        let r10 = retry_backoff_with_jitter(10);
        assert!(r10 >= Duration::from_millis(24_000) && r10 <= Duration::from_millis(36_000));
    }

    #[test]
    fn backoff_zero_retry_count_is_well_defined() {
        // retry_count = 0 corresponds to "before the first retry"; ensure
        // it does not panic and stays in the lowest backoff bucket.
        let backoff = retry_backoff_with_jitter(0);
        assert!(backoff >= Duration::from_millis(1600) && backoff <= Duration::from_millis(2400));
    }

    #[test]
    fn classify_auth_error_emits_to_session() {
        let err = InferenceError::auth_unknown("bad token");
        match classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::EmitToSession(InferenceError::Auth { .. }) => {}
            other => panic!("expected EmitToSession(Auth), got {other:?}"),
        }
    }

    #[test]
    fn classify_unauthorized_emits_to_session() {
        let err = api_err(StatusCode::UNAUTHORIZED, "no");
        match classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::EmitToSession(InferenceError::Api { status, .. }) => {
                assert_eq!(status, StatusCode::UNAUTHORIZED);
            }
            other => panic!("expected EmitToSession(Api 401), got {other:?}"),
        }
    }

    #[test]
    fn classify_encrypted_content_emits_to_session() {
        let err = api_err(
            StatusCode::BAD_REQUEST,
            "Could not decrypt the provided encrypted_content",
        );
        match classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::EmitToSession(_) => {}
            other => panic!("expected EmitToSession, got {other:?}"),
        }
    }

    #[test]
    fn classify_payload_too_large_strips_images() {
        let err = api_err(StatusCode::PAYLOAD_TOO_LARGE, "too big");
        assert!(matches!(
            classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD),
            RetryDecision::RetryWithImageStrip
        ));
    }

    #[test]
    fn classify_image_processing_error_400_strips_images() {
        let err = api_err(StatusCode::BAD_REQUEST, "Could not process image");
        assert!(matches!(
            classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD),
            RetryDecision::RetryWithImageStrip
        ));
    }

    #[test]
    fn classify_image_processing_error_500_wrapped_strips_images() {
        let err = api_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "upstream: 400 Bad Request: Could not process image",
        );
        assert!(matches!(
            classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD),
            RetryDecision::RetryWithImageStrip
        ));
    }

    #[test]
    fn classify_image_processing_error_takes_priority_over_5xx_retry() {
        // A 500 wrapping "Could not process image" is retryable by status
        // code alone — verify the image-processing guard intercepts first.
        let err = api_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not process image: bad format",
        );
        assert!(
            err.is_retryable(),
            "500 is retryable without the image-processing guard"
        );
        assert!(matches!(
            classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD),
            RetryDecision::RetryWithImageStrip
        ));
    }

    #[test]
    fn classify_rate_limited_uses_retry_after() {
        let err = api_err_with_retry_after(StatusCode::TOO_MANY_REQUESTS, 7);
        match classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::RetryWithBackoff {
                backoff,
                is_rate_limited,
            } => {
                assert!(is_rate_limited);
                assert_eq!(backoff, Duration::from_secs(7));
            }
            other => panic!("expected RetryWithBackoff, got {other:?}"),
        }
    }

    #[test]
    fn classify_rate_limited_capped_at_threshold() {
        let err = api_err(StatusCode::TOO_MANY_REQUESTS, "slow");
        // retry_count=1, threshold=2 -> next_attempt=2 >= 2 -> Fatal.
        match classify_error(&err, 1, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::Fatal(InferenceError::Api { status, .. }) => {
                assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
            }
            other => panic!("expected Fatal at threshold, got {other:?}"),
        }
    }

    /// 429 with only `x-ratelimit-reset` falls back to the parsed reset
    /// (no `Retry-After`).
    #[test]
    fn classify_rate_limited_uses_reset_when_no_retry_after() {
        let err = api_err_with_reset(StatusCode::TOO_MANY_REQUESTS, 30);
        match classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::RetryWithBackoff {
                backoff,
                is_rate_limited,
            } => {
                assert!(is_rate_limited);
                assert_eq!(backoff, Duration::from_secs(30));
            }
            other => panic!("expected RetryWithBackoff, got {other:?}"),
        }
    }

    /// `Retry-After` wins over `x-ratelimit-reset` when both are present.
    #[test]
    fn classify_rate_limited_retry_after_wins_over_reset() {
        let err = api_err_with_retry_after_and_reset(StatusCode::TOO_MANY_REQUESTS, 7, 30);
        match classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::RetryWithBackoff { backoff, .. } => {
                assert_eq!(backoff, Duration::from_secs(7));
            }
            other => panic!("expected RetryWithBackoff, got {other:?}"),
        }
    }

    /// 429 with neither `Retry-After` nor reset falls back to jitter.
    #[test]
    fn classify_rate_limited_no_hints_uses_jitter() {
        let err = api_err(StatusCode::TOO_MANY_REQUESTS, "slow");
        match classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::RetryWithBackoff { backoff, .. } => {
                // Jitter base for next_attempt=1 is ~2s +/- 20%.
                assert!(backoff >= Duration::from_millis(1600));
                assert!(backoff <= Duration::from_millis(2400));
            }
            other => panic!("expected RetryWithBackoff, got {other:?}"),
        }
    }

    /// `rate_limit_backoff` precedence: Retry-After > reset > jitter.
    #[test]
    fn rate_limit_backoff_precedence() {
        // Retry-After wins.
        let err = api_err_with_retry_after_and_reset(StatusCode::TOO_MANY_REQUESTS, 9, 30);
        assert_eq!(rate_limit_backoff(&err, 1), Duration::from_secs(9));

        // Reset wins when no Retry-After.
        let err = api_err_with_reset(StatusCode::TOO_MANY_REQUESTS, 30);
        assert_eq!(rate_limit_backoff(&err, 1), Duration::from_secs(30));

        // Jitter when neither.
        let err = api_err(StatusCode::TOO_MANY_REQUESTS, "slow");
        let b = rate_limit_backoff(&err, 1);
        assert!(b >= Duration::from_millis(1600) && b <= Duration::from_millis(2400));
    }

    /// OpenRouter gets the higher default threshold (3) via the resolver.
    #[test]
    fn resolve_rate_limit_threshold_openrouter_default() {
        assert_eq!(
            resolve_rate_limit_threshold(crate::config::ProviderIdentity::OpenRouter, None),
            OPENROUTER_RATE_LIMIT_RETRY_THRESHOLD
        );
    }

    /// Non-OpenRouter providers keep the generic threshold (2).
    #[test]
    fn resolve_rate_limit_threshold_non_openrouter_keeps_generic() {
        for identity in [
            crate::config::ProviderIdentity::Custom,
            crate::config::ProviderIdentity::Xai,
            crate::config::ProviderIdentity::OpenAi,
        ] {
            assert_eq!(
                resolve_rate_limit_threshold(identity, None),
                RATE_LIMIT_RETRY_THRESHOLD
            );
        }
    }

    /// Env override is honoured for OpenRouter.
    #[test]
    fn resolve_rate_limit_threshold_openrouter_env_override() {
        assert_eq!(
            resolve_rate_limit_threshold(crate::config::ProviderIdentity::OpenRouter, Some("5")),
            5
        );
    }

    /// A non-positive env override falls back to the default (don't silently
    /// disable all 429 retries via a misconfiguration).
    #[test]
    fn resolve_rate_limit_threshold_openrouter_env_zero_falls_back() {
        assert_eq!(
            resolve_rate_limit_threshold(crate::config::ProviderIdentity::OpenRouter, Some("0")),
            OPENROUTER_RATE_LIMIT_RETRY_THRESHOLD
        );
    }

    /// Env override does not leak into non-OpenRouter providers.
    #[test]
    fn resolve_rate_limit_threshold_env_ignored_for_non_openrouter() {
        assert_eq!(
            resolve_rate_limit_threshold(crate::config::ProviderIdentity::Xai, Some("9")),
            RATE_LIMIT_RETRY_THRESHOLD
        );
    }

    /// With the OpenRouter threshold (3), a 429 is retried on attempt 2
    /// (next_attempt=2 < 3) but escalates on attempt 3.
    #[test]
    fn classify_rate_limited_openrouter_threshold_allows_third_retry() {
        let err = api_err_with_retry_after(StatusCode::TOO_MANY_REQUESTS, 1);
        // retry_count=1 -> next_attempt=2 < 3 -> retry.
        assert!(matches!(
            classify_error(&err, 1, 15, OPENROUTER_RATE_LIMIT_RETRY_THRESHOLD),
            RetryDecision::RetryWithBackoff { .. }
        ));
        // retry_count=2 -> next_attempt=3 >= 3 -> Fatal.
        assert!(matches!(
            classify_error(&err, 2, 15, OPENROUTER_RATE_LIMIT_RETRY_THRESHOLD),
            RetryDecision::Fatal(_)
        ));
    }

    #[test]
    fn zero_retry_budget_never_reuses_a_model_output_cap() {
        for err in [
            api_err(StatusCode::INTERNAL_SERVER_ERROR, "boom"),
            api_err(StatusCode::PAYLOAD_TOO_LARGE, "too big"),
            api_err(StatusCode::BAD_REQUEST, "Could not process image"),
            InferenceError::EmptyResponse {
                context: xai_grok_inference_types::EmptyResponseContext {
                    reason: xai_grok_inference_types::EmptyReason::NoVisibleContent,
                    had_reasoning: false,
                    content_len: 0,
                    tool_call_count: 0,
                    finish_reason: Some("stop".into()),
                    completion_tokens: Some(1),
                    reasoning_tokens: Some(0),
                    prompt_tokens: Some(10),
                    model: "m".into(),
                    first_choice_seen: true,
                },
            },
        ] {
            assert!(matches!(
                classify_error(&err, 0, 0, RATE_LIMIT_RETRY_THRESHOLD),
                RetryDecision::Fatal(_)
            ));
        }
    }

    #[test]
    fn classify_5xx_first_retry_rebuilds_client() {
        let err = api_err(StatusCode::INTERNAL_SERVER_ERROR, "boom");
        match classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::RetryWithClientRebuild { backoff } => {
                assert!(backoff >= Duration::from_millis(1600));
            }
            other => panic!("expected RetryWithClientRebuild, got {other:?}"),
        }
    }

    #[test]
    fn classify_529_overloaded_is_retryable_like_5xx() {
        let status = StatusCode::from_u16(529).expect("529");
        let err = api_err(status, "overloaded_error: Overloaded");
        assert!(err.is_retryable());
        match classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::RetryWithClientRebuild { .. } => {}
            other => panic!("expected RetryWithClientRebuild for 529, got {other:?}"),
        }
        match classify_error(&err, 1, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::Retry { .. } => {}
            other => panic!("expected Retry for subsequent 529, got {other:?}"),
        }
    }

    #[test]
    fn transient_edge_5xx_retry_but_origin_tls_is_terminal() {
        for code in [500, 501, 520, 521, 522, 523, 524, 529, 530] {
            let err = api_err(StatusCode::from_u16(code).unwrap(), "edge failure");
            assert!(err.is_retryable(), "{code} should retry");
        }
        for code in [525, 526] {
            let err = api_err(StatusCode::from_u16(code).unwrap(), "origin TLS failure");
            assert!(!err.is_retryable(), "{code} must be terminal");
            assert!(matches!(
                classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD),
                RetryDecision::Fatal(_)
            ));
        }
    }

    #[test]
    fn generic_retry_after_is_clamped_and_jittered_but_429_is_not() {
        let generic = api_err_with_retry_after(StatusCode::BAD_GATEWAY, 120);
        let backoff = match classify_error(&generic, 0, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::RetryWithClientRebuild { backoff } => backoff,
            other => panic!("expected generic retry, got {other:?}"),
        };
        assert!(backoff >= Duration::from_secs(24));
        assert!(backoff <= Duration::from_secs(36));

        let limited = api_err_with_retry_after(StatusCode::TOO_MANY_REQUESTS, 120);
        match classify_error(&limited, 0, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::RetryWithBackoff { backoff, .. } => {
                assert_eq!(backoff, Duration::from_secs(120));
            }
            other => panic!("expected rate-limit retry, got {other:?}"),
        }
    }

    #[test]
    fn classify_5xx_subsequent_retry_uses_plain_retry() {
        let err = api_err(StatusCode::BAD_GATEWAY, "boom");
        match classify_error(&err, 1, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::Retry { backoff } => {
                assert!(backoff >= Duration::from_millis(3200));
            }
            other => panic!("expected Retry, got {other:?}"),
        }
    }

    #[test]
    fn classify_5xx_exhausted_retries_is_fatal() {
        let err = api_err(StatusCode::SERVICE_UNAVAILABLE, "boom");
        match classify_error(&err, 4, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::Fatal(InferenceError::Api { .. }) => {}
            other => panic!("expected Fatal, got {other:?}"),
        }
    }

    #[test]
    fn classify_event_stream_error_is_retryable() {
        let err = InferenceError::EventStreamError("connection reset".into());
        match classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::RetryWithClientRebuild { .. } => {}
            other => panic!("expected RetryWithClientRebuild, got {other:?}"),
        }
    }

    #[test]
    fn classify_stream_error_is_retryable() {
        let err = InferenceError::StreamError {
            error_type: "transient".into(),
            message: "x".into(),
            code: None,
        };
        match classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::RetryWithClientRebuild { .. } => {}
            other => panic!("expected RetryWithClientRebuild for StreamError, got {other:?}"),
        }
    }

    #[test]
    fn classify_idle_timeout_is_fatal() {
        let err = InferenceError::IdleTimeout { elapsed_secs: 300 };
        match classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::Fatal(InferenceError::IdleTimeout { elapsed_secs: 300 }) => {}
            other => panic!("expected Fatal(IdleTimeout), got {other:?}"),
        }
    }

    #[test]
    fn classify_invalid_config_is_fatal() {
        let err = InferenceError::InvalidConfiguration("missing model");
        assert!(matches!(
            classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD),
            RetryDecision::Fatal(InferenceError::InvalidConfiguration(_))
        ));
    }

    #[test]
    fn classify_api_400_non_encrypted_is_fatal() {
        let err = api_err(StatusCode::BAD_REQUEST, "Invalid model parameter");
        assert!(matches!(
            classify_error(&err, 0, 5, RATE_LIMIT_RETRY_THRESHOLD),
            RetryDecision::Fatal(_)
        ));
    }

    fn serialization_err() -> InferenceError {
        InferenceError::Serialization(serde_json::from_str::<i32>("not a number").unwrap_err())
    }

    /// Regression: `clone_error` used to launder `Serialization` into the
    /// retryable `EventStreamError`, turning a deterministic parse failure
    /// into a full-budget retry storm.
    #[test]
    fn clone_error_preserves_serialization_and_non_retryability() {
        let cloned = clone_error(&serialization_err());
        assert!(
            matches!(cloned, InferenceError::Serialization(_)),
            "expected Serialization, got {cloned:?}"
        );
        assert!(!cloned.is_retryable());
        assert!(
            cloned.to_string().contains("line 1 column"),
            "original position text must survive the clone: {cloned}"
        );
    }

    /// The tee cell captures mid-stream errors via `clone_error`; dropping
    /// the code there would silently disable mid-stream strip recovery.
    #[test]
    fn clone_error_preserves_stream_error_code() {
        let cloned = clone_error(&InferenceError::StreamError {
            error_type: "invalid_request_error".into(),
            message: "bad image".into(),
            code: Some(xai_grok_inference_types::ApiErrorCode::InvalidImage),
        });
        let InferenceError::StreamError { code, .. } = &cloned else {
            panic!("expected StreamError, got {cloned:?}");
        };
        assert_eq!(
            *code,
            Some(xai_grok_inference_types::ApiErrorCode::InvalidImage)
        );
        assert!(cloned.is_image_processing_error());
    }

    #[test]
    fn classify_serialization_is_fatal_on_first_attempt() {
        match classify_error(&serialization_err(), 0, 15, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::Fatal(InferenceError::Serialization(_)) => {}
            other => panic!("expected Fatal(Serialization) on attempt 1, got {other:?}"),
        }
    }

    #[test]
    fn format_includes_retry_prefix_when_count_present() {
        let err = InferenceError::auth_unknown("bad");
        let s = format_inference_error(&err, Some(3));
        assert!(s.starts_with("Request failed after 3 retries."));
    }

    #[test]
    fn format_omits_retry_prefix_when_count_absent() {
        let err = InferenceError::auth_unknown("bad");
        let s = format_inference_error(&err, None);
        assert!(!s.starts_with("Request failed after"));
        assert!(s.starts_with("Authentication failed:"));
    }

    #[test]
    fn format_includes_status_hint_for_known_codes() {
        let err = api_err(StatusCode::PAYLOAD_TOO_LARGE, "big");
        let s = format_inference_error(&err, None);
        assert!(s.contains("HTTP 413"));
        assert!(s.contains("request too large"));
    }

    #[test]
    fn format_idle_timeout_includes_elapsed_secs() {
        let err = InferenceError::IdleTimeout { elapsed_secs: 240 };
        let s = format_inference_error(&err, None);
        assert!(s.contains("240s"));
    }

    #[test]
    fn should_retry_false_overrides_retryable_status() {
        let err = InferenceError::Api {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "boom".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: Some(false),
            diagnostics: None,
            error_code: None,
        };
        assert!(matches!(
            classify_error(&err, 0, 15, RATE_LIMIT_RETRY_THRESHOLD),
            RetryDecision::Fatal(_)
        ));
    }

    #[test]
    fn context_length_overflow_is_fatal_even_as_500() {
        // The backend streams a size overflow as a ResponseError that becomes a 500 with no
        // should_retry hint; without the context-length check it would retry the full budget.
        let err = InferenceError::Api {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "invalid_request_error: Your input exceeds the context window of this model."
                .into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
            diagnostics: None,
            error_code: None,
        };
        assert!(matches!(
            classify_error(&err, 0, 15, RATE_LIMIT_RETRY_THRESHOLD),
            RetryDecision::Fatal(_)
        ));
    }

    #[test]
    fn should_retry_true_falls_through_to_existing_logic() {
        let err = InferenceError::Api {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "boom".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: Some(true),
            diagnostics: None,
            error_code: None,
        };
        assert!(matches!(
            classify_error(&err, 0, 15, RATE_LIMIT_RETRY_THRESHOLD),
            RetryDecision::RetryWithClientRebuild { .. }
        ));
    }

    #[test]
    fn should_retry_absent_falls_through() {
        let err = InferenceError::Api {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "boom".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
            diagnostics: None,
            error_code: None,
        };
        assert!(matches!(
            classify_error(&err, 0, 15, RATE_LIMIT_RETRY_THRESHOLD),
            RetryDecision::RetryWithClientRebuild { .. }
        ));
    }

    #[test]
    fn classify_doom_loop_detected_is_retry_with_immediate_backoff() {
        let err = InferenceError::DoomLoopDetected {
            triggers: vec!["tail_repetition:8@thinking".into()],
            aborted_at_chunk: None,
        };
        // Whatever the counters say, classification is Retry — the recovery
        // loop owns the budget by disarming the abort when it is spent.
        for retry_count in [0, 5, 99] {
            match classify_error(&err, retry_count, 2, RATE_LIMIT_RETRY_THRESHOLD) {
                RetryDecision::Retry { backoff } => {
                    assert!(backoff <= Duration::from_millis(250), "near-immediate");
                }
                other => panic!("expected Retry, got {other:?}"),
            }
        }
    }

    #[test]
    fn should_retry_false_on_429_is_fatal() {
        // Server says don't retry, even though 429 is normally retryable.
        // should_retry check runs before rate-limit check.
        let err = InferenceError::Api {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: "rate limited".into(),
            model_metadata: None,
            retry_after_secs: Some(10),
            should_retry: Some(false),
            diagnostics: None,
            error_code: None,
        };
        assert!(matches!(
            classify_error(&err, 0, 15, RATE_LIMIT_RETRY_THRESHOLD),
            RetryDecision::Fatal(_)
        ));
    }

    /// HTTP 402 (Payment Required) is OpenRouter's out-of-credits signal. It
    /// must NOT be retried — there is no point burning the retry budget on a
    /// billing failure. `is_retryable` excludes 402 (only 429/500/502/503/
    /// 504/520 are retryable), so it falls through to the final Fatal arm.
    #[test]
    fn classify_402_payment_required_is_fatal() {
        let err = api_err(StatusCode::PAYMENT_REQUIRED, "out of credits");
        assert!(
            !err.is_retryable(),
            "402 must not be retryable — billing failure is deterministic"
        );
        assert!(matches!(
            classify_error(&err, 0, 15, RATE_LIMIT_RETRY_THRESHOLD),
            RetryDecision::Fatal(_)
        ));
    }

    /// `format_inference_error` produces a dedicated, actionable credits
    /// message for 402, naming the provider when diagnostics carry the
    /// upstream `provider_name`.
    #[test]
    fn format_402_with_diagnostics_provider_name_dedicated_message() {
        let err = InferenceError::Api {
            status: StatusCode::PAYMENT_REQUIRED,
            message: "insufficient_credits".into(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
            diagnostics: Some(xai_grok_inference_types::ApiErrorDiagnostics {
                provider_name: Some("OpenRouter".into()),
                ..Default::default()
            }),
            error_code: None,
        };
        let s = format_inference_error(&err, None);
        assert!(s.contains("OpenRouter account out of credits"));
        assert!(s.contains("add credits"));
        assert!(s.contains("402"));
    }

    /// Without a diagnostics `provider_name`, the 402 arm falls back to the
    /// generic "(out of credits - add credits to continue)" status hint.
    #[test]
    fn format_402_without_diagnostics_uses_generic_hint() {
        let err = api_err(StatusCode::PAYMENT_REQUIRED, "insufficient_credits");
        let s = format_inference_error(&err, None);
        assert!(s.contains("402"));
        assert!(s.contains("out of credits"));
    }
}
