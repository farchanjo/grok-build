//! HTTP client for the xAI sampling APIs.
//!
//! Owns the `reqwest::Client`, default request headers, and per-method
//! defaults. Talks to three backend shapes:
//!
//! * Chat Completions (`/chat/completions`)
//! * Responses API (`/responses`)
//! * Anthropic Messages API (`/messages`)
//!
//! All trace-upload and URL-based header injection is intentionally
//! *not* here. The session is responsible for putting any per-request
//! headers (proxy auth, OTel context, etc.)
//! into [`InferenceConfig::extra_headers`] before constructing the client.

use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use futures_util::stream::BoxStream;
use reqwest::header::{
    ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue, USER_AGENT,
};
use serde::Serialize;

use xai_grok_inference_types::error::{
    parse_error_code, parse_rate_limit_reset, try_parse_stream_error,
    user_facing_api_error_message_for,
};
use xai_grok_inference_types::{
    ApiErrorDiagnostics, ChatCompletionChunk, ChatCompletionRequest, ChatCompletionResponse,
    ConversationRequest, ConversationResponse, CreateResponseWrapper, DOOM_LOOP_CHECK_HEADER,
    InferenceError, MessagesRequestWrapper, ResponseModelMetadata, Result, SentCredential,
    build_messages_request, is_check_event, messages, rs,
};

use crate::config::{AuthScheme, InferenceConfig, OriginClientInfo};

// Re-export ApiBackend from the shared types crate for downstream callers.
pub use xai_grok_inference_types::ApiBackend;

/// Process-level fallback for the `x-grok-client-identifier` header.
const DEFAULT_CLIENT_IDENTIFIER: &str = "grok-shell";

/// Product identifier baked into User-Agent strings.
const AGENT_PRODUCT: &str = "grok-shell";
const ANTHROPIC_DEFAULT_MAX_TOKENS: u32 = 128_000;

/// Per-request `x-grok-*` headers. Optional fields are skipped when empty/`None`.
///
/// These headers carry stable session/conversation identifiers that must
/// never be leaked to third-party providers. `apply` is a no-op unless
/// `first_party` is `true` (only the first-party xAI provider sets it).
struct GrokRequestHeaders<'a> {
    conv_id: &'a str,
    req_id: &'a str,
    model_id: &'a str,
    session_id: &'a str,
    turn_idx: Option<&'a str>,
    agent_id: &'a str,
    deployment_id: Option<&'a str>,
    user_id: Option<&'a str>,
    /// When `false`, `apply` injects no `x-grok-*` headers at all. Only the
    /// first-party xAI provider passes `true`.
    first_party: bool,
}

impl GrokRequestHeaders<'_> {
    fn apply(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if !self.first_party {
            return builder;
        }
        let mut b = builder
            .header("x-grok-conv-id", self.conv_id)
            .header("x-grok-req-id", self.req_id)
            .header("x-grok-model-override", self.model_id)
            .header("x-grok-session-id", self.session_id)
            .header("x-grok-agent-id", self.agent_id);
        if let Some(idx) = self.turn_idx {
            b = b.header("x-grok-turn-idx", idx);
        }
        if let Some(id) = self.deployment_id.filter(|s| !s.is_empty()) {
            b = b.header("x-grok-deployment-id", id);
        }
        if let Some(id) = self.user_id.filter(|s| !s.is_empty()) {
            b = b.header("x-grok-user-id", id);
        }
        b
    }
}

/// SSE `event:` name and JSON `type` tag for Responses transport heartbeats
/// (OpenAI OAuth / Codex / async-openai). Not a model API event.
const RESPONSES_KEEPALIVE_TYPE: &str = "keepalive";

/// Exact SSE `event:` name / JSON `type` for Codex-only Responses control
/// frames that carry optional turn-state headers and verification metadata.
/// Absent from async-openai's strict `ResponseStreamEvent` union.
const RESPONSES_METADATA_TYPE: &str = "response.metadata";

/// True when an SSE frame is a Responses transport keepalive / heartbeat.
///
/// Matches either:
/// - the SSE `event:` field `keepalive` (async-openai skips these the same way), or
/// - a JSON payload whose `"type"` tag is exactly `"keepalive"`
///   (OpenAI OAuth / Codex long-lived Responses streams).
///
/// Unknown semantic `response.*` events are intentionally NOT matched here —
/// those must still fail closed at typed deserialization so forward-incompatible
/// API changes remain observable. SSE comments and empty-data frames never
/// reach this check (`eventsource-stream` discards them).
fn is_responses_keepalive(event_name: &str, data: &str) -> bool {
    if event_name == RESPONSES_KEEPALIVE_TYPE {
        return true;
    }
    // Cheap substring precheck so ordinary traffic never pays a JSON parse.
    // Confirm via the typed `type` tag so a legitimate delta whose text
    // merely quotes "keepalive" is not swallowed.
    data.contains(RESPONSES_KEEPALIVE_TYPE)
        && serde_json::from_str::<serde_json::Value>(data)
            .is_ok_and(|v| v.get("type").and_then(|t| t.as_str()) == Some(RESPONSES_KEEPALIVE_TYPE))
}

/// True when an SSE frame is the Codex-only `response.metadata` control event.
///
/// OpenAI Codex's permissive stream shape includes exact `type ==
/// "response.metadata"` frames with optional top-level `headers` (for example
/// `x-codex-turn-state`) and optional verification/moderation metadata.
/// async-openai's strict `ResponseStreamEvent` union does not model this
/// variant, so unfiltered frames become a user-facing serialization failure.
///
/// Matches either:
/// - the SSE `event:` field exactly `response.metadata`, or
/// - a valid JSON payload whose top-level `"type"` is exactly
///   `"response.metadata"`.
///
/// Absorbed as a no-op: do not emit model output, do not forward headers or
/// turn state (no channel exists today), and do not reset Layer-2 content
/// idle timers. Other unknown `response.*` events remain fail-closed.
fn is_responses_metadata(event_name: &str, data: &str) -> bool {
    if event_name == RESPONSES_METADATA_TYPE {
        return true;
    }
    // Cheap substring precheck; confirm via the exact top-level `type` tag so
    // nested mentions or unrelated events are not swallowed.
    data.contains(RESPONSES_METADATA_TYPE)
        && serde_json::from_str::<serde_json::Value>(data)
            .is_ok_and(|v| v.get("type").and_then(|t| t.as_str()) == Some(RESPONSES_METADATA_TYPE))
}

/// Deserialize a Responses API SSE event, with a fallback for xAI-specific
/// tool types (e.g., `x_search`) that `async_openai` can't parse.
///
/// The API echoes the request's `tools` array in `ResponseCompleted` and
/// `ResponseCreated` events. If we sent `{"type": "x_search"}`, the response
/// includes it, and `rs::Tool` deserialization fails. On failure, we strip
/// unrecognized tools from the raw JSON and retry.
///
/// On `response.completed` / `response.incomplete`, this also rewrites
/// `response.usage.total_tokens` in place to the live context length
/// (`context_details.input_tokens + context_details.output_tokens`)
/// when the API emits the xAI-specific `context_details` field.
/// Async-openai's typed `ResponseUsage` doesn't model `context_details`,
/// so we peek the raw JSON for it. The cumulative `input_tokens` /
/// `output_tokens` / `cached_tokens` continue to flow from the typed
/// `ResponseUsage` unchanged so billing telemetry stays correct. When
/// the API doesn't emit `context_details` (older deployments) `total_tokens`
/// passes through unchanged.
///
/// Callers must filter transport control frames (see
/// [`is_responses_keepalive`] and [`is_responses_metadata`]) before invoking
/// this; those frames are not representable as `ResponseStreamEvent`.
fn deserialize_response_event(data: &str) -> Result<rs::ResponseStreamEvent> {
    let mut event = match serde_json::from_str::<rs::ResponseStreamEvent>(data) {
        Ok(event) => event,
        Err(first_err) => {
            // Try sanitizing: parse as Value, strip unknown tools, retry.
            if let Ok(mut value) = serde_json::from_str::<serde_json::Value>(data) {
                // Strip tools that async_openai's rs::Tool can't deserialize
                // (e.g., xAI-specific "x_search"). Instead of maintaining a
                // hardcoded allowlist, try deserializing each tool entry —
                // if it fails, drop it.
                if let Some(tools) = value
                    .pointer_mut("/response/tools")
                    .and_then(|v| v.as_array_mut())
                {
                    tools.retain(|t| serde_json::from_value::<rs::Tool>(t.clone()).is_ok());
                }
                if let Ok(mut event) = serde_json::from_value::<rs::ResponseStreamEvent>(value) {
                    apply_terminal_event_overrides(&mut event, data);
                    return Ok(event);
                }
            }
            tracing::error!(
                error = %first_err,
                raw_data = %data,
                "Failed to deserialize ResponseStreamEvent from stream"
            );
            return Err(InferenceError::Serialization(first_err));
        }
    };
    apply_terminal_event_overrides(&mut event, data);
    Ok(event)
}

/// On terminal Responses API events (`response.completed` /
/// `response.incomplete`), rewrite `response.usage.total_tokens` to the
/// live context length when the wire includes
/// `response.usage.context_details.{input_tokens, output_tokens}`.
///
/// `total_tokens` drives the CLI's `/context` bar, the auto-compact
/// threshold, and `meta.totalTokens` on persisted sessions. Under
/// server-side multi-turn loops (e.g. `web_search`, `x_search`) the
/// wire's cumulative total inflates as the loop runs; `context_details`
/// reports the final turn's prompt + output tokens — the real live
/// context the model is sitting in. Billing fields
/// (`input_tokens`, `output_tokens`, `input_tokens_details.cached_tokens`,
/// `output_tokens_details.reasoning_tokens`) stay on the cumulative
/// wire values so telemetry is unaffected.
///
/// No-op when:
/// - the event is not terminal,
/// - `response.usage` is `None`,
/// - `context_details` is absent (older backends / non-loop responses),
/// - or either of `context_details.{input_tokens, output_tokens}` is
///   missing — we don't guess the missing half.
fn apply_terminal_event_overrides(event: &mut rs::ResponseStreamEvent, data: &str) {
    let response = match event {
        rs::ResponseStreamEvent::ResponseCompleted(e) => &mut e.response,
        rs::ResponseStreamEvent::ResponseIncomplete(e) => &mut e.response,
        _ => return,
    };
    // Re-parse for fields async_openai's types omit (context total, cost ticks).
    let Ok(value) = serde_json::from_str::<serde_json::Value>(data) else {
        return;
    };
    // Stash cost ticks in metadata for stream_responses.
    if let Some(ticks) = xai_grok_inference_types::reported_cost_ticks(
        value
            .pointer("/response/usage/cost_in_usd_ticks")
            .and_then(|v| v.as_i64()),
    ) {
        response
            .metadata
            .get_or_insert_with(Default::default)
            .insert(COST_USD_TICKS_METADATA_KEY.to_owned(), ticks.to_string());
    }
    let Some(usage) = response.usage.as_mut() else {
        return;
    };
    let Some(total) = extract_context_total(&value) else {
        return;
    };
    usage.total_tokens = total;
}

/// Metadata key for cost ticks past typed Response events.
pub(crate) const COST_USD_TICKS_METADATA_KEY: &str = "xai.cost_usd_ticks";

/// Read `response.usage.context_details.{input_tokens, output_tokens}`
/// from the parsed terminal-event JSON and return their sum. Returns `None`
/// if either field is missing or out of `u32` range.
fn extract_context_total(value: &serde_json::Value) -> Option<u32> {
    let cd = value.pointer("/response/usage/context_details")?;
    let i = u32::try_from(cd.get("input_tokens")?.as_u64()?).ok()?;
    let o = u32::try_from(cd.get("output_tokens")?.as_u64()?).ok()?;
    Some(i.saturating_add(o))
}

/// Record `success=false` + `error` on the active inference span when a stream
/// request fails before any response (transport/connect/TLS errors). Without
/// this the `#[instrument]` span closes with both fields Empty, so an outage
/// shows zero `success=false` and error-rate alerts never fire.
fn record_stream_request_failure(err: &reqwest::Error) {
    let span = tracing::Span::current();
    span.record("success", false);
    span.record("error", err.to_string().as_str());
}

/// Parse the `Retry-After` response header as delta-seconds.
/// Our inference backends only emit integer seconds (never HTTP-date),
/// so we only handle that form. HTTP-dates silently return `None` and
/// the caller falls back to exponential backoff.
/// Capped at 120s to prevent absurdly long sleeps from a misbehaving upstream.
fn extract_retry_after(headers: &reqwest::header::HeaderMap) -> Option<u64> {
    headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .map(|s| s.min(120))
}

fn extract_should_retry(headers: &reqwest::header::HeaderMap) -> Option<bool> {
    headers
        .get("x-should-retry")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| {
            if s.eq_ignore_ascii_case("true") {
                Some(true)
            } else if s.eq_ignore_ascii_case("false") {
                Some(false)
            } else {
                None // unknown value — treat as absent
            }
        })
}

const MAX_DIAGNOSTIC_VALUE_CHARS: usize = 256;

/// Read a response header without retaining unbounded or non-UTF-8 data.
fn diagnostic_header(headers: &reqwest::header::HeaderMap, name: &str) -> Option<String> {
    let value = headers.get(name)?.to_str().ok()?.trim();
    if value.is_empty() {
        return None;
    }
    Some(value.chars().take(MAX_DIAGNOSTIC_VALUE_CHARS).collect())
}

fn diagnostic_json_string(value: Option<&serde_json::Value>) -> Option<String> {
    let value = value?.as_str()?.trim();
    if value.is_empty() {
        return None;
    }
    Some(value.chars().take(MAX_DIAGNOSTIC_VALUE_CHARS).collect())
}

fn diagnostic_json_scalar(value: Option<&serde_json::Value>) -> Option<String> {
    let value = value?;
    if value.is_string() {
        return diagnostic_json_string(Some(value));
    }
    if value.is_number() {
        return Some(
            value
                .to_string()
                .chars()
                .take(MAX_DIAGNOSTIC_VALUE_CHARS)
                .collect(),
        );
    }
    None
}

fn openrouter_attempt_provider(metadata: &serde_json::Value) -> Option<String> {
    metadata
        .get("attempts")
        .and_then(serde_json::Value::as_array)
        .and_then(|attempts| attempts.last())
        .and_then(|attempt| diagnostic_json_string(attempt.get("provider")))
        .or_else(|| {
            metadata
                .pointer("/endpoints/available")
                .and_then(serde_json::Value::as_array)
                .and_then(|endpoints| {
                    endpoints
                        .iter()
                        .find(|endpoint| {
                            endpoint
                                .get("selected")
                                .and_then(serde_json::Value::as_bool)
                                .unwrap_or(false)
                        })
                        .and_then(|endpoint| diagnostic_json_string(endpoint.get("provider")))
                })
        })
}

/// Extract only the documented OpenRouter routing fields. We deliberately do
/// not retain arbitrary metadata or response-body text: error envelopes can
/// contain provider-specific details that are not safe to propagate to logs.
fn extract_api_error_diagnostics(
    headers: &reqwest::header::HeaderMap,
    bytes: &[u8],
    openrouter_metadata_requested: bool,
) -> Option<ApiErrorDiagnostics> {
    let rate_limit_reset_raw = diagnostic_header(headers, "x-ratelimit-reset");
    let mut diagnostics = ApiErrorDiagnostics {
        rate_limit_limit: diagnostic_header(headers, "x-ratelimit-limit"),
        rate_limit_remaining: diagnostic_header(headers, "x-ratelimit-remaining"),
        rate_limit_reset: rate_limit_reset_raw.clone(),
        rate_limit_reset_secs: parse_rate_limit_reset(rate_limit_reset_raw.as_deref()),
        generation_id: diagnostic_header(headers, "x-generation-id"),
        ..Default::default()
    };

    let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        if openrouter_metadata_requested && diagnostics.provider_name.is_none() {
            diagnostics.provider_name = Some("OpenRouter".to_string());
        }
        return (!diagnostics.is_empty()).then_some(diagnostics);
    };
    let error = value.get("error").unwrap_or(&value);
    let error_metadata = error.get("metadata").or_else(|| value.get("metadata"));
    let router_metadata = value.get("openrouter_metadata");
    diagnostics.error_type = diagnostic_json_string(
        error_metadata
            .and_then(|metadata| metadata.get("error_type"))
            .or_else(|| error.get("type")),
    );
    diagnostics.provider_code = diagnostic_json_scalar(
        error_metadata
            .and_then(|metadata| metadata.get("provider_code"))
            .or_else(|| error.get("code")),
    );
    diagnostics.provider_name =
        diagnostic_json_string(error_metadata.and_then(|metadata| metadata.get("provider_name")))
            .or_else(|| router_metadata.and_then(openrouter_attempt_provider));

    if openrouter_metadata_requested && diagnostics.provider_name.is_none() {
        // The request definitively used OpenRouter, but it did not name the
        // selected upstream. Surface the router, not an invented provider.
        diagnostics.provider_name = Some("OpenRouter".to_string());
    }

    (!diagnostics.is_empty()).then_some(diagnostics)
}

fn api_error(
    status: reqwest::StatusCode,
    message: String,
    model_metadata: Option<ResponseModelMetadata>,
    retry_after_secs: Option<u64>,
    should_retry: Option<bool>,
    diagnostics: Option<ApiErrorDiagnostics>,
    error_code: Option<xai_grok_inference_types::ApiErrorCode>,
) -> InferenceError {
    if let Some(diagnostics) = diagnostics.as_ref() {
        tracing::warn!(
            target: crate::inference_log::TARGET,
            event = "api_error_diagnostics",
            status_code = status.as_u16(),
            error_type = diagnostics.error_type.as_deref().unwrap_or("unknown"),
            provider_code = diagnostics.provider_code.as_deref().unwrap_or("unknown"),
            provider_name = diagnostics.provider_name.as_deref().unwrap_or("unknown"),
            rate_limit_limit = ?diagnostics.rate_limit_limit,
            rate_limit_remaining = ?diagnostics.rate_limit_remaining,
            rate_limit_reset = diagnostics.rate_limit_reset.as_deref().unwrap_or("unknown"),
            generation_id = diagnostics.generation_id.as_deref().unwrap_or("unknown"),
            "API error diagnostics"
        );
    }
    InferenceError::Api {
        status,
        message,
        model_metadata,
        retry_after_secs,
        should_retry,
        diagnostics,
        error_code,
    }
}

fn extract_model_metadata(headers: &reqwest::header::HeaderMap) -> Option<ResponseModelMetadata> {
    let context_window = headers
        .get("x-grok-context-window")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());

    let max_completion_tokens = headers
        .get("x-grok-max-completion-tokens")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u32>().ok());

    let models_etag = headers
        .get("x-models-etag")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    if context_window.is_some() || max_completion_tokens.is_some() || models_etag.is_some() {
        Some(ResponseModelMetadata {
            context_window,
            max_completion_tokens,
            models_etag,
        })
    } else {
        None
    }
}

/// Wrapper for streaming chat completion requests that adds `stream` and
/// `stream_options` fields without modifying the original `ChatCompletionRequest`.
///
/// Uses `#[serde(flatten)]` to inline all fields from the inner request,
/// allowing single-pass serialization instead of the previous two-pass
/// approach (serialize to `Value`, mutate, serialize to bytes).
#[derive(Serialize)]
struct StreamingChatRequest<'a> {
    #[serde(flatten)]
    inner: &'a ChatCompletionRequest,
    /// OpenRouter extension: models to try after the request's primary
    /// `model`. Absent for native OpenAI/xAI/Codex requests.
    #[serde(skip_serializing_if = "Option::is_none")]
    models: Option<&'a [String]>,
    /// OpenRouter extension: native `provider` preferences. Absent for native
    /// OpenAI/xAI/Codex requests and when no preferences are configured.
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<&'a crate::config::OpenRouterProviderPreferences>,
    /// OpenRouter extension: native `plugins` array. Absent for native
    /// OpenAI/xAI/Codex requests and when no plugins are configured.
    #[serde(skip_serializing_if = "Option::is_none")]
    plugins: Option<&'a [crate::config::OpenRouterPlugin]>,
    /// OpenRouter normalized `reasoning` object. Absent for non-OpenRouter
    /// identities; derived from flat `reasoning_effort` when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<&'a crate::config::OpenRouterReasoning>,
    /// Z.ai extension: fragmented tool argument streaming.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    tool_stream: bool,
    /// Z.ai extension: thinking object.
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<&'a serde_json::Value>,
    stream: bool,
    stream_options: StreamOptions,
}

/// Wrapper for non-streaming chat completion requests with OpenRouter's
/// optional `models` fallback extension.
#[derive(Serialize)]
struct ChatRequestWithFallbacks<'a> {
    #[serde(flatten)]
    inner: &'a ChatCompletionRequest,
    #[serde(skip_serializing_if = "Option::is_none")]
    models: Option<&'a [String]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<&'a crate::config::OpenRouterProviderPreferences>,
    /// OpenRouter extension: native `plugins` array. Absent for native
    /// OpenAI/xAI/Codex requests and when no plugins are configured.
    #[serde(skip_serializing_if = "Option::is_none")]
    plugins: Option<&'a [crate::config::OpenRouterPlugin]>,
    /// OpenRouter normalized `reasoning` object. Absent for non-OpenRouter.
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<&'a crate::config::OpenRouterReasoning>,
    /// Z.ai extension: fragmented tool argument streaming.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    tool_stream: bool,
    /// Z.ai extension: thinking object.
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<&'a serde_json::Value>,
}

#[derive(Serialize)]
struct StreamOptions {
    include_usage: bool,
}

/// Add OpenRouter's documented `models` extension to an OpenAI-compatible
/// request body. The array contains fallback models only; `model` remains the
/// primary. Kept as a small helper so Chat Completions and Responses share the
/// same wire contract.
fn add_openrouter_fallback_models(
    request_body: &mut serde_json::Value,
    fallback_models: Option<&[String]>,
) {
    if let Some(models) = fallback_models.filter(|models| !models.is_empty()) {
        request_body["models"] = serde_json::json!(models);
    }
}

/// Attach OpenRouter-native `provider` / `plugins` when present.
///
/// These member types implement [`Serialize`] with only string-keyed maps and
/// plain scalars, so conversion is infallible in practice. A panic here would
/// indicate a programming error (not a user config problem); silently omitting
/// requested routing policy is forbidden.
fn add_openrouter_provider_and_plugins(
    request_body: &mut serde_json::Value,
    provider: Option<&crate::config::OpenRouterProviderPreferences>,
    plugins: Option<&[crate::config::OpenRouterPlugin]>,
) {
    if let Some(prefs) = provider {
        request_body["provider"] =
            serde_json::to_value(prefs).expect("OpenRouterProviderPreferences must serialize");
    }
    if let Some(list) = plugins.filter(|p| !p.is_empty()) {
        request_body["plugins"] =
            serde_json::to_value(list).expect("OpenRouterPlugin list must serialize");
    }
}

/// OpenRouter Responses beta is stateless: always `store = false` and never
/// send `previous_response_id` (null or otherwise). Full conversation history
/// is kept locally and re-sent as `input` each turn.
fn enforce_openrouter_responses_stateless(request_body: &mut serde_json::Value) {
    request_body["store"] = serde_json::json!(false);
    if let Some(obj) = request_body.as_object_mut() {
        obj.remove("previous_response_id");
    }
}

/// HTTP client for sampling. Cheap to clone; carries an `Arc`-backed
/// `reqwest::Client` and the default headers/request-defaults computed
/// from a [`InferenceConfig`] at construction time.
#[derive(Clone)]
pub struct InferenceClient {
    http: reqwest::Client,
    default_headers: HeaderMap,
    base_url: String,
    defaults: ClientDefaults,
    /// Optional 401-attribution hook. The shell wires this to emit a
    /// structured event at every UNAUTHORIZED arm so 401s can be
    /// bucketed by stale-snapshot vs. live-token-rejected. `None` for
    /// sampler-only callers and tests.
    attribution_callback: Option<crate::attribution::SharedAttributionCallback>,
    /// Per-request bearer override. See `InferenceConfig::bearer_resolver`.
    bearer_resolver: Option<crate::config::SharedBearerResolver>,
    /// Per-request header injection (OTel traceparent).
    header_injector: Option<crate::config::SharedHeaderInjector>,
    /// True only when the config targets the first-party xAI provider. Gates
    /// injection of `x-grok-*` request headers so third-party providers never
    /// see stable session/conversation identifiers.
    first_party: bool,
    /// True only when the config targets OpenRouter, so error diagnostics
    /// treat the upstream metadata as explicitly requested.
    openrouter_metadata_requested: bool,
    /// User-facing provider label derived from [`InferenceConfig::provider_identity`]
    /// at construction time. Used as the fallback for provider-aware error
    /// copy (502/520-class, 402) when the diagnostics `provider_name` (the
    /// selected OpenRouter upstream) is unavailable at the call site.
    provider_label: String,
    /// Exact provider route retained for auxiliary sampling (operation
    /// partition, exact-route 401 attribution). Absent on legacy constructors.
    route_context: Option<crate::route_context::ProviderRouteContext>,
}

impl std::fmt::Debug for InferenceClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InferenceClient")
            .field("base_url", &self.base_url)
            .field("defaults", &self.defaults)
            .field(
                "has_attribution_callback",
                &self.attribution_callback.is_some(),
            )
            .field("has_bearer_resolver", &self.bearer_resolver.is_some())
            .field(
                "openrouter_metadata_requested",
                &self.openrouter_metadata_requested,
            )
            .field("provider_label", &self.provider_label)
            .field(
                "route_instance",
                &self.route_context.as_ref().map(|r| r.instance_id()),
            )
            .field(
                "route_operation",
                &self.route_context.as_ref().map(|r| r.operation_partition()),
            )
            .finish()
    }
}

/// Request builder coupled to the auth state captured when its headers were
/// built. A later credential refresh must not change how a 401 is accounted.
struct SentRequest {
    builder: reqwest::RequestBuilder,
    sent_credential: SentCredential,
    sent_bearer_tail: Option<String>,
}

fn auth_rejected(message: String, credential: SentCredential) -> InferenceError {
    InferenceError::Auth {
        message,
        credential,
    }
}

#[derive(Clone, Debug, Default)]
struct ClientDefaults {
    model: String,
    max_completion_tokens: Option<u32>,
    temperature: Option<f32>,
    top_p: Option<f32>,
    openrouter_fallback_models: Vec<String>,
    openrouter_provider_preferences: Option<crate::config::OpenRouterProviderPreferences>,
    openrouter_plugins: Vec<crate::config::OpenRouterPlugin>,
    zai_tool_stream: bool,
    zai_thinking: Option<serde_json::Value>,
    api_backend: ApiBackend,
    include_message_model_id: bool,
    auth_scheme: AuthScheme,
    stream_tool_calls: bool,
    doom_loop_recovery: Option<xai_grok_inference_types::DoomLoopRecoveryPolicy>,
}

// =============================================================================
// User-Agent helpers
// =============================================================================

#[derive(Clone, Debug, Eq, PartialEq)]
struct PlatformInfo {
    os: String,
    arch: String,
}

impl PlatformInfo {
    fn current() -> Self {
        let os = match std::env::consts::OS {
            "macos" => "macos",
            "windows" => "windows",
            other => other,
        }
        .to_string();

        let arch = match std::env::consts::ARCH {
            "arm64" => "aarch64",
            "x86_64" => "x86_64",
            other => other,
        }
        .to_string();

        Self { os, arch }
    }
}

fn agent_version() -> String {
    xai_grok_version::VERSION.to_string()
}

/// Render a User-Agent string for the given origin client.
///
/// Mirrors the shell's `user_agent_string_for` but uses sampler-local
/// constants. The session typically owns the canonical User-Agent
/// rendering for process-wide HTTP clients; this helper is for
/// per-session sampling clients that want to override it.
pub fn user_agent_string_for(origin: &OriginClientInfo) -> String {
    let agent_version = agent_version();
    let platform = PlatformInfo::current();

    if origin.product == AGENT_PRODUCT && origin.version.as_deref() == Some(agent_version.as_str())
    {
        return format!(
            "{}/{} ({}; {})",
            AGENT_PRODUCT, agent_version, platform.os, platform.arch
        );
    }

    match origin.version.as_deref() {
        Some(origin_version) => format!(
            "{}/{} {}/{} ({}; {})",
            origin.product,
            origin_version,
            AGENT_PRODUCT,
            agent_version,
            platform.os,
            platform.arch
        ),
        None => format!(
            "{} {}/{} ({}; {})",
            origin.product, AGENT_PRODUCT, agent_version, platform.os, platform.arch
        ),
    }
}

// =============================================================================
// InferenceClient
// =============================================================================

impl InferenceClient {
    /// Construct a sampling client from a [`InferenceConfig`].
    ///
    /// Grabs the process-wide shared `reqwest::Client` (HTTP/2 by
    /// default, HTTP/1.1 when `config.force_http1` is set) and
    /// pre-computes the default request headers. This does not perform
    /// any network I/O.
    pub fn new(config: InferenceConfig) -> Result<Self> {
        Self::new_with_route_context(config, None)
    }

    /// Construct a sampling client that retains an exact
    /// [`ProviderRouteContext`] for auxiliary pacing/attribution.
    ///
    /// Primary session turns should prefer the actor path
    /// (`spawn_with_route_context`); this constructor is the
    /// production seam for one-shot auxiliary clients (compaction,
    /// media, title, suggest, goal evaluator).
    pub fn new_with_route_context(
        config: InferenceConfig,
        route_context: Option<crate::route_context::ProviderRouteContext>,
    ) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        if let Some(ref api_key) = config.api_key {
            match config.auth_scheme {
                AuthScheme::XApiKey => {
                    let header_value = HeaderValue::from_str(api_key).map_err(|_| {
                        // Never log the key value (any provider).
                        tracing::debug!(
                            key_len = api_key.len(),
                            "Invalid api_key: cannot be converted to a valid HTTP header"
                        );
                        InferenceError::auth_unknown(
                            "Invalid api_key: cannot be converted to a valid HTTP header",
                        )
                    })?;
                    headers.insert(HeaderName::from_static("x-api-key"), header_value);
                }
                AuthScheme::Bearer => {
                    let bearer = format!("Bearer {}", api_key);
                    let header_value = HeaderValue::from_str(&bearer).map_err(|_| {
                        // Never log the key value (any provider).
                        tracing::debug!(
                            key_len = api_key.len(),
                            "Invalid api_key: cannot be converted to a valid HTTP Authorization header"
                        );
                        InferenceError::auth_unknown(
                            "Invalid api_key: cannot be converted to a valid HTTP Authorization header",
                        )
                    })?;
                    headers.insert(AUTHORIZATION, header_value);
                }
            }
        }

        // Apply all extra headers verbatim. This is the single
        // injection point for proxy-auth headers and any other URL- or
        // environment-specific headers the session decides to set.
        for (key, value) in &config.extra_headers {
            let header_name = HeaderName::try_from(key.as_str())
                .map_err(|_| InferenceError::InvalidConfiguration("Invalid extra header name"))?;
            let header_value = HeaderValue::from_str(value)
                .map_err(|_| InferenceError::InvalidConfiguration("Invalid extra header value"))?;
            headers.insert(header_name, header_value);
        }

        // Default `x-grok-client-*` / deployment / user headers are first-party
        // only. Direct Anthropic (and other third-party identities) must not
        // receive stable Grok client identifiers on the wire.
        if config.provider_identity.is_first_party() {
            // Add x-grok-client-version header for version gating at the proxy.
            if let Some(client_version) = config.client_version.as_ref()
                && let Ok(header_value) = HeaderValue::from_str(client_version)
            {
                headers.insert(
                    HeaderName::from_static("x-grok-client-version"),
                    header_value,
                );
            }

            if let Some(deployment_id) = config.deployment_id.as_ref()
                && let Ok(header_value) = HeaderValue::from_str(deployment_id)
            {
                headers.insert(
                    HeaderName::from_static("x-grok-deployment-id"),
                    header_value,
                );
            }

            if let Some(user_id) = config.user_id.as_ref()
                && let Ok(header_value) = HeaderValue::from_str(user_id)
            {
                headers.insert(HeaderName::from_static("x-grok-user-id"), header_value);
            }

            {
                let client_id = config
                    .client_identifier
                    .clone()
                    .unwrap_or_else(|| DEFAULT_CLIENT_IDENTIFIER.to_string());
                if let Ok(header_value) = HeaderValue::from_str(&client_id) {
                    headers.insert(
                        HeaderName::from_static("x-grok-client-identifier"),
                        header_value,
                    );
                }
            }
        }

        // Always set User-Agent: per-session origin if available, else fallback.
        {
            let ua_string = match config.origin_client.as_ref() {
                Some(origin) => user_agent_string_for(origin),
                None => user_agent_string_for(&OriginClientInfo {
                    product: AGENT_PRODUCT.to_string(),
                    version: Some(agent_version()),
                }),
            };
            if let Ok(v) = HeaderValue::from_str(&ua_string) {
                headers.insert(USER_AGENT, v);
            }
        }

        let http = if config.force_http1 {
            tracing::info!("Using HTTP/1.1 for sampling client (force_http1=true)");
            crate::shared_http::client_http1().map_err(InferenceError::Http)?
        } else {
            crate::shared_http::client().map_err(InferenceError::Http)?
        };

        tracing::info!(
            target: crate::inference_log::TARGET,
            event = "client_new",
            base_url = %config.base_url,
            model = %config.model,
            api_backend = ?config.api_backend,
            auth_scheme = ?config.auth_scheme,
            // "unset" (not "none"): `ReasoningEffort::None` is a real wire value;
            // logging the absent Option as "none" looked like we were sending it.
            reasoning_effort = config.reasoning_effort.map_or("unset", |e| e.as_str()),
            openrouter_fallback_count = config.openrouter_fallback_models.len(),
            has_api_key = config.api_key.is_some(),
            has_bearer_resolver = config.bearer_resolver.is_some(),
            has_authorization_header = headers.get(AUTHORIZATION).is_some(),
            has_x_api_key_header = headers.get(HeaderName::from_static("x-api-key")).is_some(),
        );

        let defaults = ClientDefaults {
            model: config.model,
            max_completion_tokens: config.max_completion_tokens,
            temperature: config.temperature,
            top_p: config.top_p,
            openrouter_fallback_models: config.openrouter_fallback_models,
            openrouter_provider_preferences: config.openrouter_provider_preferences,
            openrouter_plugins: config.openrouter_plugins,
            zai_tool_stream: config.zai_tool_stream,
            zai_thinking: config.zai_thinking,
            api_backend: config.api_backend,
            include_message_model_id: config.include_message_model_id,
            auth_scheme: config.auth_scheme,
            stream_tool_calls: config.stream_tool_calls,
            doom_loop_recovery: config.doom_loop_recovery,
        };

        Ok(Self {
            http,
            default_headers: headers,
            base_url: config.base_url,
            defaults,
            attribution_callback: config.attribution_callback,
            bearer_resolver: config.bearer_resolver,
            header_injector: config.header_injector,
            first_party: config.provider_identity.is_first_party(),
            openrouter_metadata_requested: config.provider_identity.is_openrouter(),
            provider_label: config.provider_identity.label().to_string(),
            route_context,
        })
    }

    /// Exact route retained at construction (auxiliary sampling).
    pub fn route_context(&self) -> Option<&crate::route_context::ProviderRouteContext> {
        self.route_context.as_ref()
    }

    /// The configured API backend for this client.
    pub fn api_backend(&self) -> ApiBackend {
        self.defaults.api_backend.clone()
    }

    /// The configured model the session requested for turns on this client.
    /// Used by the Chat Completions stream transform to detect an OpenRouter
    /// fallback (served model differs from this requested model).
    pub fn model(&self) -> &str {
        &self.defaults.model
    }

    /// Whether this client targets OpenRouter. Gates fallback-model
    /// detection so non-OpenRouter providers never produce a fallback
    /// signal even when the served model id differs.
    pub fn is_openrouter(&self) -> bool {
        self.openrouter_metadata_requested
    }

    /// POST with auth provenance captured from the headers that will be sent.
    /// A wired resolver is authoritative: when it has no value, stale default
    /// credentials are removed and the request is classified `Missing`.
    fn post(&self, url: impl reqwest::IntoUrl) -> SentRequest {
        let mut headers = self.default_headers.clone();
        if let Some(resolver) = &self.bearer_resolver {
            headers.remove(AUTHORIZATION);
            headers.remove(HeaderName::from_static("x-api-key"));
            if let Some(fresh) = resolver.current_bearer() {
                match self.defaults.auth_scheme {
                    AuthScheme::XApiKey => {
                        if let Ok(v) = HeaderValue::from_str(&fresh) {
                            headers.insert(HeaderName::from_static("x-api-key"), v);
                        }
                    }
                    AuthScheme::Bearer => {
                        if let Ok(v) = HeaderValue::from_str(&format!("Bearer {fresh}")) {
                            headers.insert(AUTHORIZATION, v);
                        }
                    }
                }
            }
        }
        {
            // Presence/scheme only — never log Authorization / x-api-key values
            // or stable prefixes (Anthropic sk-ant-… must never enter logs).
            tracing::info!(
                target: crate::inference_log::TARGET,
                event = "client_post",
                base_url = %self.base_url,
                model = %self.defaults.model,
                api_backend = ?self.defaults.api_backend,
                auth_scheme = ?self.defaults.auth_scheme,
                has_bearer_resolver = self.bearer_resolver.is_some(),
                has_authorization_header = headers.get(AUTHORIZATION).is_some(),
                has_x_api_key_header = headers.get(HeaderName::from_static("x-api-key")).is_some(),
            );
        }
        let sent_bearer_tail =
            Self::sent_bearer_tail_from_headers(&headers, self.defaults.auth_scheme);
        let sent_credential = SentCredential::from_sent_bearer_tail(sent_bearer_tail.as_deref());
        if let Some(injector) = &self.header_injector {
            injector.inject(&mut headers);
        }
        SentRequest {
            builder: self.http.post(url).headers(headers),
            sent_credential,
            sent_bearer_tail,
        }
    }

    fn sent_bearer_tail_from_headers(headers: &HeaderMap, scheme: AuthScheme) -> Option<String> {
        // `HeaderValue::to_str` accepts visible ASCII only, while a value built
        // from a Rust string may contain valid UTF-8 obs-text. Attribution must
        // remain Unicode-safe whenever those exact bytes are present on the
        // request; arbitrary non-UTF-8 header bytes still fail closed to None.
        fn utf8(value: &HeaderValue) -> Option<&str> {
            std::str::from_utf8(value.as_bytes()).ok()
        }
        let raw = match scheme {
            AuthScheme::XApiKey => headers
                .get(HeaderName::from_static("x-api-key"))
                .and_then(utf8),
            AuthScheme::Bearer => headers
                .get(AUTHORIZATION)
                .and_then(utf8)
                .and_then(|value| value.strip_prefix("Bearer ")),
        };
        raw.map(|bearer| xai_grok_inference_types::bearer_fragment::bearer_tail(bearer).to_owned())
    }

    /// Best-effort view for request-start diagnostics only. A 401 uses the
    /// build-time tail stored in [`SentRequest`] so refresh races cannot
    /// rewrite attribution or retry accounting.
    fn current_sent_bearer_tail(&self) -> Option<String> {
        if self.bearer_resolver.is_some() {
            return self
                .bearer_resolver
                .as_ref()
                .and_then(|resolver| resolver.current_bearer())
                .map(|bearer| {
                    xai_grok_inference_types::bearer_fragment::bearer_tail(&bearer).to_owned()
                });
        }
        Self::sent_bearer_tail_from_headers(&self.default_headers, self.defaults.auth_scheme)
    }

    /// Extract the configured credential tail from `default_headers`. Tests
    /// only; 401 handling uses the per-request capture.
    #[cfg(test)]
    fn extract_sent_bearer(&self) -> Option<String> {
        Self::sent_bearer_tail_from_headers(&self.default_headers, self.defaults.auth_scheme)
    }

    fn record_401_attribution(
        &self,
        consumer: crate::attribution::InferenceConsumer,
        sent_bearer_tail: Option<&str>,
    ) {
        if let Some(cb) = self.attribution_callback.as_ref() {
            cb.record_401(consumer, sent_bearer_tail);
        }
    }

    /// Resolve the user-facing provider label for an error response, preferring
    /// the diagnostics `provider_name` (the selected OpenRouter upstream) over
    /// the generic provider label derived from [`InferenceConfig::provider_identity`].
    /// This makes 502/520-class and 402 copy name the actual upstream that failed
    /// rather than the router when that information is available.
    fn provider_label_for_diagnostics<'a>(
        &'a self,
        diagnostics: Option<&'a ApiErrorDiagnostics>,
    ) -> &'a str {
        diagnostics
            .and_then(|d| d.provider_name.as_deref())
            .filter(|name| !name.is_empty())
            .unwrap_or(&self.provider_label)
    }

    /// Non-secret auth facts for sampling logs (scheme + presence only).
    /// Never returns credential values or stable prefixes.
    pub fn auth_info(&self) -> crate::inference_log::AuthInfo {
        let has_credential = self.current_sent_bearer_tail().is_some();
        let auth_type = if !has_credential {
            "none"
        } else {
            match self.defaults.auth_scheme {
                AuthScheme::XApiKey => "x-api-key",
                AuthScheme::Bearer => "bearer",
            }
        };
        crate::inference_log::AuthInfo {
            auth_type,
            has_credential,
        }
    }

    /// Check if a header name contains sensitive information that should be redacted.
    fn is_sensitive_header(name: &str) -> bool {
        let lower = name.to_lowercase();
        lower.contains("authorization")
            || lower.contains("api-key")
            || lower.contains("apikey")
            || lower.contains("token")
            || lower.contains("secret")
    }

    /// Short lossy body snippet for error logs (never user-facing).
    fn body_preview(bytes: &[u8]) -> String {
        String::from_utf8_lossy(bytes).chars().take(500).collect()
    }

    /// Log all headers from a request at debug level (redacting sensitive values).
    fn log_request_headers(request: &reqwest::Request, endpoint_name: &str) {
        for (name, value) in request.headers().iter() {
            let value_str = if Self::is_sensitive_header(name.as_str()) {
                "[REDACTED]"
            } else {
                value.to_str().unwrap_or("[non-utf8]")
            };
            tracing::debug!(
                header_name = %name,
                header_value = %value_str,
                "Request header ({})",
                endpoint_name
            );
        }
    }

    fn endpoint(&self, path: &str) -> String {
        let base = self.base_url.trim_end_matches('/');
        let path = path.trim_start_matches('/');
        format!("{base}/{path}")
    }

    fn apply_defaults(&self, mut request: ChatCompletionRequest) -> Result<ChatCompletionRequest> {
        if request.model.is_none() {
            request.model = Some(self.defaults.model.clone());
        }

        if request.max_tokens.is_none() {
            request.max_tokens = self.defaults.max_completion_tokens;
        }

        if request.temperature.is_none() {
            request.temperature = self.defaults.temperature;
        }

        if request.top_p.is_none() {
            request.top_p = self.defaults.top_p;
        }

        if !self.defaults.include_message_model_id {
            for message in &mut request.messages {
                message.model_id = None;
            }
        }

        Ok(request)
    }

    /// OpenRouter's `models` extension contains fallback models only; the
    /// normal request `model` remains the primary. Identity-gated: only
    /// [`ProviderIdentity::OpenRouter`] emits the extension so other
    /// OpenAI-compatible bodies stay unchanged even if a list is configured.
    fn openrouter_fallback_models(&self) -> Option<&[String]> {
        if !self.openrouter_metadata_requested {
            return None;
        }
        (!self.defaults.openrouter_fallback_models.is_empty())
            .then_some(self.defaults.openrouter_fallback_models.as_slice())
    }

    /// OpenRouter's native `provider` request-body preferences. Only emitted
    /// when the identity is OpenRouter and the preferences object is non-empty
    /// (all fields unset). `None` for non-OpenRouter providers and for an
    /// all-empty object, so the `provider` key is omitted from the wire body.
    fn openrouter_provider_preferences(
        &self,
    ) -> Option<&crate::config::OpenRouterProviderPreferences> {
        if !self.openrouter_metadata_requested {
            return None;
        }
        self.defaults
            .openrouter_provider_preferences
            .as_ref()
            .filter(|prefs| !prefs.is_empty())
    }

    /// OpenRouter's native `plugins` request-body array. Only emitted when
    /// the identity is OpenRouter and the list is non-empty. `None` for
    /// non-OpenRouter providers and an empty list, so the `plugins` key is
    /// omitted from the wire body.
    fn openrouter_plugins(&self) -> Option<&[crate::config::OpenRouterPlugin]> {
        if !self.openrouter_metadata_requested {
            return None;
        }
        (!self.defaults.openrouter_plugins.is_empty())
            .then_some(self.defaults.openrouter_plugins.as_slice())
    }

    /// Normalized OpenRouter Chat `reasoning` object derived from flat effort.
    /// Identity-gated: non-OpenRouter providers keep only `reasoning_effort`.
    fn openrouter_reasoning_object(
        &self,
        effort: Option<xai_grok_inference_types::ReasoningEffort>,
    ) -> Option<crate::config::OpenRouterReasoning> {
        if !self.openrouter_metadata_requested {
            return None;
        }
        effort.map(crate::config::OpenRouterReasoning::from_effort)
    }

    async fn handle_response(
        &self,
        response: reqwest::Response,
        sent_credential: SentCredential,
        sent_bearer_tail: Option<&str>,
    ) -> Result<ChatCompletionResponse> {
        let status = response.status();
        let headers = response.headers().clone();
        let model_metadata = extract_model_metadata(&headers);
        let retry_after_secs = extract_retry_after(&headers);
        let should_retry = extract_should_retry(&headers);
        let bytes = response.bytes().await?;
        let diagnostics = extract_api_error_diagnostics(
            &headers,
            bytes.as_ref(),
            self.openrouter_metadata_requested,
        );

        if !status.is_success() {
            if status == reqwest::StatusCode::UNAUTHORIZED {
                self.record_401_attribution(
                    crate::attribution::InferenceConsumer::ChatCompletions,
                    sent_bearer_tail,
                );
                let provider_label = self.provider_label_for_diagnostics(diagnostics.as_ref());
                let server_message =
                    user_facing_api_error_message_for(status, bytes.as_ref(), provider_label);
                return Err(auth_rejected(
                    format!("Unauthorized (401): {server_message}"),
                    sent_credential,
                ));
            }
            let provider_label = self.provider_label_for_diagnostics(diagnostics.as_ref());
            let message = user_facing_api_error_message_for(status, bytes.as_ref(), provider_label);
            return Err(api_error(
                status,
                message,
                model_metadata,
                retry_after_secs,
                should_retry,
                diagnostics,
                parse_error_code(bytes.as_ref()),
            ));
        }

        let completion = serde_json::from_slice::<ChatCompletionResponse>(&bytes).map_err(|e| {
            let raw_body = String::from_utf8_lossy(&bytes);
            tracing::error!(
                error = %e,
                raw_body = %raw_body,
                "Failed to deserialize ChatCompletionResponse"
            );
            InferenceError::Serialization(e)
        })?;
        Ok(completion)
    }

    // =========================================================================
    // Chat Completions API
    // =========================================================================

    pub async fn chat_completion(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse> {
        let payload = self.apply_defaults(request)?;
        let x_grok_conv_id = &payload.x_grok_conv_id.clone().unwrap_or_default();
        let x_grok_req_id = &payload.x_grok_req_id.clone().unwrap_or_default();
        let model_id = payload.model.clone().unwrap_or_default();

        tracing::debug!(
            base_url = %self.base_url,
            model_id = %model_id,
            "Sending chat completion request"
        );

        let grok_headers = GrokRequestHeaders {
            conv_id: x_grok_conv_id,
            req_id: x_grok_req_id,
            model_id: &model_id,
            session_id: payload.x_grok_session_id.as_deref().unwrap_or_default(),
            turn_idx: payload.x_grok_turn_idx.as_deref(),
            agent_id: payload.x_grok_agent_id.as_deref().unwrap_or_default(),
            deployment_id: payload.x_grok_deployment_id.as_deref(),
            user_id: payload.x_grok_user_id.as_deref(),
            first_party: self.first_party,
        };
        let reasoning = self.openrouter_reasoning_object(payload.reasoning_effort);
        let SentRequest {
            builder,
            sent_credential,
            sent_bearer_tail,
        } = self.post(self.endpoint("chat/completions"));
        let http_request = grok_headers.apply(builder).json(&ChatRequestWithFallbacks {
            inner: &payload,
            models: self.openrouter_fallback_models(),
            provider: self.openrouter_provider_preferences(),
            plugins: self.openrouter_plugins(),
            reasoning: reasoning.as_ref(),
            tool_stream: self.defaults.zai_tool_stream,
            thinking: self.defaults.zai_thinking.as_ref(),
        });

        let response = http_request.send().await.map_err(|e| {
            // Log at debug level; errors are surfaced to the caller.
            tracing::debug!("HTTP request failed: {}", e);
            e
        })?;

        self.handle_response(response, sent_credential, sent_bearer_tail.as_deref())
            .await
    }

    /// Start a streaming chat completion request. Returns a stream of typed chunks.
    #[tracing::instrument(
        name = "http.chat_completion_stream",
        skip_all,
        fields(
            endpoint = %self.endpoint("chat/completions"),
            model_id = request.model.as_deref().unwrap_or(""),
            status_code = tracing::field::Empty,
            success = tracing::field::Empty,
            error = tracing::field::Empty,
        )
    )]
    pub async fn chat_completion_stream(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<(
        BoxStream<'static, Result<ChatCompletionChunk>>,
        Option<ResponseModelMetadata>,
    )> {
        let payload = self.apply_defaults(request)?;
        let x_grok_conv_id = &payload.x_grok_conv_id.clone().unwrap_or_default();
        let x_grok_req_id = &payload.x_grok_req_id.clone().unwrap_or_default();
        let model_id = payload.model.clone().unwrap_or_default();

        // Wrap the request with streaming fields and serialize once.
        // Previously this path serialized twice: first to serde_json::Value
        // (to inject `stream` and `stream_options`), then to HTTP body bytes.
        let reasoning = self.openrouter_reasoning_object(payload.reasoning_effort);
        let streaming_request = StreamingChatRequest {
            inner: &payload,
            models: self.openrouter_fallback_models(),
            provider: self.openrouter_provider_preferences(),
            plugins: self.openrouter_plugins(),
            reasoning: reasoning.as_ref(),
            tool_stream: self.defaults.zai_tool_stream,
            thinking: self.defaults.zai_thinking.as_ref(),
            stream: true,
            stream_options: StreamOptions {
                include_usage: true,
            },
        };

        let grok_headers = GrokRequestHeaders {
            conv_id: x_grok_conv_id,
            req_id: x_grok_req_id,
            model_id: &model_id,
            session_id: payload.x_grok_session_id.as_deref().unwrap_or_default(),
            turn_idx: payload.x_grok_turn_idx.as_deref(),
            agent_id: payload.x_grok_agent_id.as_deref().unwrap_or_default(),
            deployment_id: payload.x_grok_deployment_id.as_deref(),
            user_id: payload.x_grok_user_id.as_deref(),
            first_party: self.first_party,
        };
        let SentRequest {
            builder,
            sent_credential,
            sent_bearer_tail,
        } = self.post(self.endpoint("chat/completions"));
        let http_request = grok_headers
            .apply(builder)
            .header(ACCEPT, HeaderValue::from_static("text/event-stream"))
            .json(&streaming_request);

        let built_request = http_request.build().map_err(|e| {
            tracing::error!("Failed to build HTTP request: {}", e);
            InferenceError::Http(e)
        })?;

        tracing::debug!(
            url = %built_request.url(),
            method = %built_request.method(),
            "Sending chat/completions request"
        );
        Self::log_request_headers(&built_request, "chat/completions");

        let response = self.http.execute(built_request).await.map_err(|e| {
            tracing::debug!("HTTP request failed: {}", e);
            record_stream_request_failure(&e);
            e
        })?;

        let status = response.status();
        let response_headers = response.headers().clone();
        let span = tracing::Span::current();
        span.record("status_code", status.as_u16() as i64);
        span.record("success", status.is_success());
        let model_metadata = extract_model_metadata(response.headers());
        let retry_after_secs = extract_retry_after(response.headers());
        let should_retry = extract_should_retry(response.headers());
        if !status.is_success() {
            if status == reqwest::StatusCode::UNAUTHORIZED {
                span.record("error", "unauthorized (401)");
                self.record_401_attribution(
                    crate::attribution::InferenceConsumer::ChatCompletionsStream,
                    sent_bearer_tail.as_deref(),
                );
                let endpoint = self.endpoint("chat/completions");
                let body = response.bytes().await.unwrap_or_default();
                let diagnostics = extract_api_error_diagnostics(
                    &response_headers,
                    body.as_ref(),
                    self.openrouter_metadata_requested,
                );
                let provider_label = self.provider_label_for_diagnostics(diagnostics.as_ref());
                let server_message =
                    user_facing_api_error_message_for(status, body.as_ref(), provider_label);
                return Err(auth_rejected(
                    format!("Unauthorized (401) from {endpoint}: {server_message}"),
                    sent_credential,
                ));
            }

            let bytes = response.bytes().await?;
            let diagnostics = extract_api_error_diagnostics(
                &response_headers,
                bytes.as_ref(),
                self.openrouter_metadata_requested,
            );
            let provider_label = self.provider_label_for_diagnostics(diagnostics.as_ref());
            let message = user_facing_api_error_message_for(status, bytes.as_ref(), provider_label);
            span.record("error", message.as_str());
            tracing::error!(
                status = %status,
                error_message = %message,
                body_preview = %Self::body_preview(bytes.as_ref()),
                model_id = %model_id,
                "chat/completions API error"
            );
            return Err(api_error(
                status,
                message,
                model_metadata,
                retry_after_secs,
                should_retry,
                diagnostics,
                parse_error_code(bytes.as_ref()),
            ));
        }

        // Strip UTF-8 BOM if present: eventsource-stream 0.2.3 incorrectly slices BOM at byte 1 instead of 3.
        const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];
        let mut is_first = true;
        let byte_stream = response.bytes_stream().map(move |result| {
            result.map(|bytes| {
                if is_first {
                    is_first = false;
                    if bytes.starts_with(UTF8_BOM) {
                        return bytes.slice(UTF8_BOM.len()..);
                    }
                }
                bytes
            })
        });

        // Turn raw bytes into SSE events
        let event_stream = byte_stream.eventsource();

        // Map SSE events into ChatCompletionChunk.
        // Uses `scan` so that `[DONE]` and transport errors both terminate the
        // stream (`None`). The first transport error is emitted to the consumer,
        // then subsequent polls return `None` -- preventing an infinite busy-loop
        // when the HTTP/2 connection drops and h2 keeps producing errors.
        let chunks = event_stream
            .scan(false, |had_transport_error, event_res| {
                if *had_transport_error {
                    return std::future::ready(None);
                }
                let item = match event_res {
                    Ok(event) => {
                        let data = &event.data;
                        if data == "[DONE]" {
                            return std::future::ready(None);
                        }

                        tracing::info!(
                            target: crate::inference_log::TARGET,
                            event = "sse_chunk",
                            backend = "chat_completions",
                            data = %data,
                        );

                        if let Some(stream_error) = try_parse_stream_error(data) {
                            Some(Err(stream_error))
                        } else {
                            Some(
                                serde_json::from_str::<ChatCompletionChunk>(data).map_err(|e| {
                                    tracing::error!(
                                        error = %e,
                                        raw_data = %data,
                                        "Failed to deserialize ChatCompletionChunk from stream"
                                    );
                                    InferenceError::Serialization(e)
                                }),
                            )
                        }
                    }
                    Err(e) => {
                        *had_transport_error = true;
                        Some(Err(InferenceError::EventStreamError(e.to_string())))
                    }
                };
                std::future::ready(item)
            })
            .boxed();

        Ok((chunks, model_metadata))
    }

    // =========================================================================
    // Responses API
    // =========================================================================

    /// Apply default configuration to a Responses API request.
    fn apply_response_defaults(&self, request: &mut CreateResponseWrapper) -> Result<()> {
        // Apply model default if not specified
        if request.inner.model.is_none() {
            request.inner.model = Some(self.defaults.model.clone());
        }

        let is_codex = xai_grok_inference_types::is_chatgpt_codex_base_url(&self.base_url);
        if is_codex {
            // ChatGPT Codex rejects OpenAI Platform sampling/budget fields
            // (OpenCode parity: omit maxOutputTokens / temperature / top_p).
            xai_grok_inference_types::clear_chatgpt_codex_create_response_fields(
                &mut request.inner,
            );
        } else {
            // Apply temperature default if not specified
            if request.inner.temperature.is_none() {
                request.inner.temperature = self.defaults.temperature;
            }

            // Apply top_p default if not specified
            if request.inner.top_p.is_none() {
                request.inner.top_p = self.defaults.top_p;
            }

            // Apply max_output_tokens default if not specified
            if request.inner.max_output_tokens.is_none() {
                request.inner.max_output_tokens = self.defaults.max_completion_tokens;
            }

            // Set store to false if not specified (default is true, but that breaks ZDR compliance)
            if request.inner.store.is_none() {
                request.inner.store = Some(false);
            }
        }

        // OpenRouter Responses beta is always stateless — force store=false
        // even if a caller set store=true, and never retain previous_response_id.
        if self.openrouter_metadata_requested {
            request.inner.store = Some(false);
            request.inner.previous_response_id = None;
        }

        // Include encrypted reasoning content if not specified
        let includes = request.inner.include.get_or_insert_with(Vec::new);
        if !includes.contains(&rs::IncludeEnum::ReasoningEncryptedContent) {
            includes.push(rs::IncludeEnum::ReasoningEncryptedContent);
        }

        Ok(())
    }

    /// Post-serialize Responses body shaping (Codex strip + optional xAI extras).
    fn finalize_responses_request_body(
        &self,
        request_body: &mut serde_json::Value,
        extra_tool_entries: Vec<serde_json::Value>,
    ) {
        let is_codex = xai_grok_inference_types::is_chatgpt_codex_base_url(&self.base_url);
        // xAI-only: never inject stream_tool_calls toward ChatGPT Codex.
        if !is_codex && self.defaults.stream_tool_calls {
            request_body["stream_tool_calls"] = serde_json::json!(true);
        }
        add_openrouter_fallback_models(request_body, self.openrouter_fallback_models());
        // OpenRouter-native extensions (identity-gated, shared with Chat).
        if self.openrouter_metadata_requested {
            add_openrouter_provider_and_plugins(
                request_body,
                self.openrouter_provider_preferences(),
                self.openrouter_plugins(),
            );
            enforce_openrouter_responses_stateless(request_body);
        }
        if !extra_tool_entries.is_empty() {
            if let Some(tools) = request_body.get_mut("tools").and_then(|v| v.as_array_mut()) {
                tools.extend(extra_tool_entries);
            } else {
                request_body["tools"] = serde_json::Value::Array(extra_tool_entries);
            }
        }
        xai_grok_inference_types::patch_reasoning_text_types(request_body);
        if is_codex {
            xai_grok_inference_types::shape_chatgpt_codex_responses_body(request_body);
        }
    }

    /// Create a response using the Responses API (non-streaming).
    ///
    /// This uses the Responses API format which provides a simpler interface
    /// for multi-turn conversations and tool calling.
    pub async fn create_response(
        &self,
        mut request: CreateResponseWrapper,
    ) -> Result<rs::Response> {
        self.apply_response_defaults(&mut request)?;

        let x_grok_conv_id = request.x_grok_conv_id.as_deref().unwrap_or_default();
        let x_grok_req_id = request.x_grok_req_id.as_deref().unwrap_or_default();
        let model_id = request.inner.model.clone().unwrap_or_default();

        // The trace field is process-local: it is consumed by upstream
        // session code (which may upload a payload artifact) and is not
        // forwarded by the sampler. Drop it before we send.
        request.trace.take();

        tracing::debug!("create_response: {:?}", &request);
        tracing::debug!("endpoint: {:?}", self.endpoint("responses"));

        let grok_headers = GrokRequestHeaders {
            conv_id: x_grok_conv_id,
            req_id: x_grok_req_id,
            model_id: &model_id,
            session_id: request.x_grok_session_id.as_deref().unwrap_or_default(),
            turn_idx: request.x_grok_turn_idx.as_deref(),
            agent_id: request.x_grok_agent_id.as_deref().unwrap_or_default(),
            deployment_id: request.x_grok_deployment_id.as_deref(),
            user_id: request.x_grok_user_id.as_deref(),
            first_party: self.first_party,
        };
        let mut request_body = serde_json::to_value(&request.inner).map_err(|e| {
            tracing::error!("Failed to serialize responses request: {}", e);
            InferenceError::Serialization(e)
        })?;
        self.finalize_responses_request_body(&mut request_body, Vec::new());
        let SentRequest {
            builder,
            sent_credential,
            sent_bearer_tail,
        } = self.post(self.endpoint("responses"));
        let http_request = grok_headers.apply(builder).json(&request_body);

        let response = http_request.send().await.map_err(|e| {
            tracing::debug!("HTTP request failed: {}", e);
            e
        })?;

        let status = response.status();
        let response_headers = response.headers().clone();
        let model_metadata = extract_model_metadata(&response_headers);
        let retry_after_secs = extract_retry_after(&response_headers);
        let should_retry = extract_should_retry(&response_headers);
        let bytes = response.bytes().await?;
        let diagnostics = extract_api_error_diagnostics(
            &response_headers,
            bytes.as_ref(),
            self.openrouter_metadata_requested,
        );

        if !status.is_success() {
            if status == reqwest::StatusCode::UNAUTHORIZED {
                self.record_401_attribution(
                    crate::attribution::InferenceConsumer::Responses,
                    sent_bearer_tail.as_deref(),
                );
                let endpoint = self.endpoint("responses");
                let provider_label = self.provider_label_for_diagnostics(diagnostics.as_ref());
                let server_message =
                    user_facing_api_error_message_for(status, bytes.as_ref(), provider_label);
                return Err(auth_rejected(
                    format!("Unauthorized (401) from {endpoint}: {server_message}"),
                    sent_credential,
                ));
            }

            let provider_label = self.provider_label_for_diagnostics(diagnostics.as_ref());
            let message = user_facing_api_error_message_for(status, bytes.as_ref(), provider_label);
            tracing::warn!(
                status = %status,
                error_message = %message,
                body_preview = %Self::body_preview(bytes.as_ref()),
                model_id = %model_id,
                "responses API error"
            );
            return Err(api_error(
                status,
                message,
                model_metadata,
                retry_after_secs,
                should_retry,
                diagnostics,
                parse_error_code(bytes.as_ref()),
            ));
        }

        let response_obj = serde_json::from_slice::<rs::Response>(&bytes).map_err(|e| {
            let raw_body = String::from_utf8_lossy(&bytes);
            tracing::error!(
                error = %e,
                raw_body = %raw_body,
                "Failed to deserialize rs::Response"
            );
            InferenceError::Serialization(e)
        })?;
        Ok(response_obj)
    }

    /// Create a streaming response using the Responses API.
    ///
    /// Returns a stream of `rs::ResponseStreamEvent` which includes events like:
    /// - `response.created` - Initial response object
    /// - `response.output_text.delta` - Text content deltas
    /// - `response.function_call_arguments.delta` - Function call argument deltas
    /// - `response.completed` - Final response with all output
    ///
    /// The third tuple element is a per-request doom-loop signal collector,
    /// `Some` only when `InferenceConfig::doom_loop_recovery` is set — the same
    /// gate that adds the opt-in `x-grok-doom-loop-check` request header, so
    /// header and parse protection cannot drift apart. It is filled by the
    /// SSE decoder as the server reports triggers and is meant to be handed
    /// to `stream_responses` so the signals land on the final
    /// `ConversationResponse`.
    #[tracing::instrument(
        name = "http.create_response_stream",
        skip_all,
        fields(
            endpoint = %self.endpoint("responses"),
            model_id = request.inner.model.as_deref().unwrap_or(""),
            status_code = tracing::field::Empty,
            success = tracing::field::Empty,
            error = tracing::field::Empty,
        )
    )]
    #[allow(clippy::type_complexity)]
    pub async fn create_response_stream(
        &self,
        mut request: CreateResponseWrapper,
    ) -> Result<(
        BoxStream<'static, Result<rs::ResponseStreamEvent>>,
        Option<ResponseModelMetadata>,
        Option<crate::doom_loop::DoomLoopSignalCollector>,
    )> {
        self.apply_response_defaults(&mut request)?;

        // Enable streaming
        request.inner.stream = Some(true);

        let x_grok_conv_id = request.x_grok_conv_id.as_deref().unwrap_or_default();
        let x_grok_req_id = request.x_grok_req_id.as_deref().unwrap_or_default();
        let model_id = request.inner.model.clone().unwrap_or_default();

        // Drop process-local trace data (see note in `create_response`).
        request.trace.take();

        tracing::debug!(
            base_url = %self.base_url,
            model_id = model_id.as_str(),
            "Sending responses API stream request"
        );

        let grok_headers = GrokRequestHeaders {
            conv_id: x_grok_conv_id,
            req_id: x_grok_req_id,
            model_id: &model_id,
            session_id: request.x_grok_session_id.as_deref().unwrap_or_default(),
            turn_idx: request.x_grok_turn_idx.as_deref(),
            agent_id: request.x_grok_agent_id.as_deref().unwrap_or_default(),
            deployment_id: request.x_grok_deployment_id.as_deref(),
            user_id: request.x_grok_user_id.as_deref(),
            first_party: self.first_party,
        };
        let extra_tool_entries = std::mem::take(&mut request.extra_tool_entries);
        let mut request_body = serde_json::to_value(&request.inner).map_err(|e| {
            tracing::error!("Failed to serialize responses request: {}", e);
            InferenceError::Serialization(e)
        })?;
        self.finalize_responses_request_body(&mut request_body, extra_tool_entries);
        // Fresh per attempt so signals never leak across retries; `None`
        // (check disabled) sends no header and does no peek work per event.
        let doom_loop = self
            .defaults
            .doom_loop_recovery
            .map(crate::doom_loop::DoomLoopSignalCollector::new);
        let SentRequest {
            builder,
            sent_credential,
            sent_bearer_tail,
        } = self.post(self.endpoint("responses"));
        let mut http_request = grok_headers
            .apply(builder)
            .header(ACCEPT, HeaderValue::from_static("text/event-stream"));
        if doom_loop.is_some() {
            // Presence opts in; the server ignores the value.
            http_request = http_request.header(DOOM_LOOP_CHECK_HEADER, "true");
        }
        let http_request = http_request.json(&request_body);

        let built_request = http_request.build().map_err(|e| {
            tracing::error!("Failed to build HTTP request: {}", e);
            InferenceError::Http(e)
        })?;

        tracing::debug!(
            url = %built_request.url(),
            method = %built_request.method(),
            "Sending responses API stream request"
        );
        Self::log_request_headers(&built_request, "responses");

        let response = self.http.execute(built_request).await.map_err(|e| {
            tracing::debug!("HTTP request failed: {}", e);
            record_stream_request_failure(&e);
            e
        })?;

        let status = response.status();
        let response_headers = response.headers().clone();
        let span = tracing::Span::current();
        span.record("status_code", status.as_u16() as i64);
        span.record("success", status.is_success());
        if !status.is_success() {
            if status == reqwest::StatusCode::UNAUTHORIZED {
                span.record("error", "unauthorized (401)");
                self.record_401_attribution(
                    crate::attribution::InferenceConsumer::ResponsesStream,
                    sent_bearer_tail.as_deref(),
                );
                let endpoint = self.endpoint("responses");
                let body = response.bytes().await.unwrap_or_default();
                let diagnostics = extract_api_error_diagnostics(
                    &response_headers,
                    body.as_ref(),
                    self.openrouter_metadata_requested,
                );
                let provider_label = self.provider_label_for_diagnostics(diagnostics.as_ref());
                let server_message =
                    user_facing_api_error_message_for(status, body.as_ref(), provider_label);
                return Err(auth_rejected(
                    format!("Unauthorized (401) from {endpoint}: {server_message}"),
                    sent_credential,
                ));
            }
            let model_metadata = extract_model_metadata(&response_headers);
            let retry_after_secs = extract_retry_after(&response_headers);
            let should_retry = extract_should_retry(&response_headers);
            let bytes = response.bytes().await?;
            let diagnostics = extract_api_error_diagnostics(
                &response_headers,
                bytes.as_ref(),
                self.openrouter_metadata_requested,
            );
            let provider_label = self.provider_label_for_diagnostics(diagnostics.as_ref());
            let message = user_facing_api_error_message_for(status, bytes.as_ref(), provider_label);
            span.record("error", message.as_str());
            tracing::error!(
                status = %status,
                error_message = %message,
                body_preview = %Self::body_preview(bytes.as_ref()),
                model_id = %model_id,
                "responses API error"
            );
            return Err(api_error(
                status,
                message,
                model_metadata,
                retry_after_secs,
                should_retry,
                diagnostics,
                parse_error_code(bytes.as_ref()),
            ));
        }

        let model_metadata = extract_model_metadata(response.headers());

        // Strip UTF-8 BOM if present
        const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];
        let mut is_first = true;
        let byte_stream = response.bytes_stream().map(move |result| {
            result.map(|bytes| {
                if is_first {
                    is_first = false;
                    if bytes.starts_with(UTF8_BOM) {
                        return bytes.slice(UTF8_BOM.len()..);
                    }
                }
                bytes
            })
        });

        // Turn raw bytes into SSE events
        let event_stream = byte_stream.eventsource();

        let doom_loop_for_stream = doom_loop.clone();

        // The scan item is an `Option`: `Some(None)` skips an absorbed
        // doom-loop event without terminating the stream (`filter_map`
        // below), while an outer `None` still ends it.
        let events = event_stream
            .scan(false, move |had_transport_error, event_res| {
                if *had_transport_error {
                    return std::future::ready(None);
                }
                let item = match event_res {
                    Ok(event) => {
                        let data = &event.data;
                        if data == "[DONE]" {
                            return std::future::ready(None);
                        }

                        // Transport / Codex control frames first — before the
                        // info-level `sse_chunk` log that dumps the payload.
                        // Keepalives can arrive often on long OAuth streams;
                        // `response.metadata` may carry turn-state headers and
                        // moderation fields. Never log either payload.
                        if is_responses_keepalive(&event.event, data) {
                            tracing::debug!(
                                target: crate::inference_log::TARGET,
                                event = "sse_keepalive",
                                backend = "responses",
                                sse_event = %event.event,
                                "ignoring Responses transport keepalive"
                            );
                            return std::future::ready(Some(None));
                        }
                        if is_responses_metadata(&event.event, data) {
                            tracing::debug!(
                                target: crate::inference_log::TARGET,
                                event = "sse_response_metadata",
                                backend = "responses",
                                sse_event = %event.event,
                                "ignoring Responses metadata control event"
                            );
                            return std::future::ready(Some(None));
                        }

                        tracing::info!(
                            target: crate::inference_log::TARGET,
                            event = "sse_chunk",
                            backend = "responses",
                            data = %data,
                        );

                        // Intercept the non-standard doom-loop event before
                        // typed deserialization; async-openai's event enum
                        // does not know it and would fail to parse it. With
                        // the check disabled, the shared name-or-payload-type
                        // predicate guards against a server emitting it
                        // despite no opt-in (rollout skew), named or not.
                        let swallow = match &doom_loop_for_stream {
                            Some(collector) => collector.absorb(&event.event, data),
                            None => is_check_event(&event.event, data),
                        };
                        if swallow {
                            Some(None)
                        } else if let Some(stream_error) = try_parse_stream_error(data) {
                            Some(Some(Err(stream_error)))
                        } else {
                            Some(Some(deserialize_response_event(data)))
                        }
                    }
                    Err(e) => {
                        *had_transport_error = true;
                        Some(Some(Err(InferenceError::EventStreamError(e.to_string()))))
                    }
                };
                std::future::ready(item)
            })
            .filter_map(std::future::ready)
            .boxed();

        Ok((events, model_metadata, doom_loop))
    }

    // =========================================================================
    // Anthropic Messages API
    // =========================================================================

    /// Apply default configuration to a Messages API request.
    fn apply_message_defaults(&self, request: &mut MessagesRequestWrapper) -> Result<()> {
        // Apply model default if not specified
        if request.inner.model.is_empty() {
            request.inner.model = self.defaults.model.clone();
        }

        if request.inner.max_tokens == 0 {
            request.inner.max_tokens = self
                .defaults
                .max_completion_tokens
                .unwrap_or(ANTHROPIC_DEFAULT_MAX_TOKENS);
        }

        // Apply temperature default if not specified
        if request.inner.temperature.is_none() {
            request.inner.temperature = self.defaults.temperature;
        }

        // Apply top_p default if not specified
        if request.inner.top_p.is_none() {
            request.inner.top_p = self.defaults.top_p;
        }

        Ok(())
    }

    /// Create a message using the Anthropic Messages API (non-streaming).
    pub async fn create_message(
        &self,
        mut request: MessagesRequestWrapper,
    ) -> Result<messages::MessagesResponse> {
        self.apply_message_defaults(&mut request)?;

        let x_grok_conv_id = request.x_grok_conv_id.as_deref().unwrap_or_default();
        let x_grok_req_id = request.x_grok_req_id.as_deref().unwrap_or_default();
        let model_id = request.inner.model.clone();

        // Drop process-local trace data.
        request.trace.take();

        tracing::debug!("create_message: {:?}", &request.inner);
        tracing::debug!("endpoint: {:?}", self.endpoint("messages"));

        let grok_headers = GrokRequestHeaders {
            conv_id: x_grok_conv_id,
            req_id: x_grok_req_id,
            model_id: &model_id,
            session_id: request.x_grok_session_id.as_deref().unwrap_or_default(),
            turn_idx: request.x_grok_turn_idx.as_deref(),
            agent_id: request.x_grok_agent_id.as_deref().unwrap_or_default(),
            deployment_id: request.x_grok_deployment_id.as_deref(),
            user_id: request.x_grok_user_id.as_deref(),
            first_party: self.first_party,
        };
        let SentRequest {
            builder,
            sent_credential,
            sent_bearer_tail,
        } = self.post(self.endpoint("messages"));
        let http_request = grok_headers.apply(builder).json(&request.inner);

        let response = http_request.send().await.map_err(|e| {
            tracing::debug!("HTTP request failed: {}", e);
            e
        })?;

        let status = response.status();
        let response_headers = response.headers().clone();
        let model_metadata = extract_model_metadata(&response_headers);
        let retry_after_secs = extract_retry_after(&response_headers);
        let should_retry = extract_should_retry(&response_headers);
        let bytes = response.bytes().await?;
        let diagnostics = extract_api_error_diagnostics(
            &response_headers,
            bytes.as_ref(),
            self.openrouter_metadata_requested,
        );

        if !status.is_success() {
            if status == reqwest::StatusCode::UNAUTHORIZED {
                self.record_401_attribution(
                    crate::attribution::InferenceConsumer::Messages,
                    sent_bearer_tail.as_deref(),
                );
                let endpoint = self.endpoint("messages");
                let provider_label = self.provider_label_for_diagnostics(diagnostics.as_ref());
                let server_message =
                    user_facing_api_error_message_for(status, bytes.as_ref(), provider_label);
                return Err(auth_rejected(
                    format!("Unauthorized (401) from {endpoint}: {server_message}"),
                    sent_credential,
                ));
            }

            let provider_label = self.provider_label_for_diagnostics(diagnostics.as_ref());
            let message = user_facing_api_error_message_for(status, bytes.as_ref(), provider_label);
            tracing::warn!(
                status = %status,
                error_message = %message,
                body_preview = %Self::body_preview(bytes.as_ref()),
                model_id = %model_id,
                "messages API error"
            );
            return Err(api_error(
                status,
                message,
                model_metadata,
                retry_after_secs,
                should_retry,
                diagnostics,
                parse_error_code(bytes.as_ref()),
            ));
        }

        let response_obj =
            serde_json::from_slice::<messages::MessagesResponse>(&bytes).map_err(|e| {
                let raw_body = String::from_utf8_lossy(&bytes);
                tracing::error!(
                    error = %e,
                    raw_body = %raw_body,
                    "Failed to deserialize MessagesResponse"
                );
                InferenceError::Serialization(e)
            })?;
        Ok(response_obj)
    }

    /// Create a streaming message using the Anthropic Messages API.
    ///
    /// Returns a stream of `MessageStreamEvent` which includes events like:
    /// - `message_start` - Initial message object
    /// - `content_block_start` / `content_block_delta` / `content_block_stop` - Content blocks
    /// - `message_delta` / `message_stop` - Final message with stop reason
    #[tracing::instrument(
        name = "http.create_message_stream",
        skip_all,
        fields(
            endpoint = %self.endpoint("messages"),
            model_id = request.inner.model.as_str(),
            status_code = tracing::field::Empty,
            success = tracing::field::Empty,
            error = tracing::field::Empty,
        )
    )]
    pub async fn create_message_stream(
        &self,
        mut request: MessagesRequestWrapper,
    ) -> Result<(
        BoxStream<'static, Result<messages::MessageStreamEvent>>,
        Option<ResponseModelMetadata>,
    )> {
        self.apply_message_defaults(&mut request)?;

        // Enable streaming
        request.inner.stream = Some(true);

        let x_grok_conv_id = request.x_grok_conv_id.as_deref().unwrap_or_default();
        let x_grok_req_id = request.x_grok_req_id.as_deref().unwrap_or_default();
        let model_id = request.inner.model.clone();

        // Drop process-local trace data.
        request.trace.take();

        tracing::debug!(
            base_url = %self.base_url,
            model_id = model_id.as_str(),
            "Sending Messages API stream request"
        );

        let grok_headers = GrokRequestHeaders {
            conv_id: x_grok_conv_id,
            req_id: x_grok_req_id,
            model_id: &model_id,
            session_id: request.x_grok_session_id.as_deref().unwrap_or_default(),
            turn_idx: request.x_grok_turn_idx.as_deref(),
            agent_id: request.x_grok_agent_id.as_deref().unwrap_or_default(),
            deployment_id: request.x_grok_deployment_id.as_deref(),
            user_id: request.x_grok_user_id.as_deref(),
            first_party: self.first_party,
        };
        let SentRequest {
            builder,
            sent_credential,
            sent_bearer_tail,
        } = self.post(self.endpoint("messages"));
        let http_request = grok_headers
            .apply(builder)
            .header(ACCEPT, HeaderValue::from_static("text/event-stream"))
            .json(&request.inner);

        let built_request = http_request.build().map_err(|e| {
            tracing::error!("Failed to build HTTP request: {}", e);
            InferenceError::Http(e)
        })?;

        tracing::debug!(
            url = %built_request.url(),
            method = %built_request.method(),
            "Sending messages API stream request"
        );
        Self::log_request_headers(&built_request, "messages");

        let response = self.http.execute(built_request).await.map_err(|e| {
            tracing::debug!("HTTP request failed: {}", e);
            record_stream_request_failure(&e);
            e
        })?;

        let status = response.status();
        let response_headers = response.headers().clone();
        let span = tracing::Span::current();
        span.record("status_code", status.as_u16() as i64);
        span.record("success", status.is_success());
        if !status.is_success() {
            if status == reqwest::StatusCode::UNAUTHORIZED {
                span.record("error", "unauthorized (401)");
                self.record_401_attribution(
                    crate::attribution::InferenceConsumer::MessagesStream,
                    sent_bearer_tail.as_deref(),
                );
                let endpoint = self.endpoint("messages");
                let body = response.bytes().await.unwrap_or_default();
                let diagnostics = extract_api_error_diagnostics(
                    &response_headers,
                    body.as_ref(),
                    self.openrouter_metadata_requested,
                );
                let provider_label = self.provider_label_for_diagnostics(diagnostics.as_ref());
                let server_message =
                    user_facing_api_error_message_for(status, body.as_ref(), provider_label);
                return Err(auth_rejected(
                    format!("Unauthorized (401) from {endpoint}: {server_message}"),
                    sent_credential,
                ));
            }
            let model_metadata = extract_model_metadata(&response_headers);
            let retry_after_secs = extract_retry_after(&response_headers);
            let should_retry = extract_should_retry(&response_headers);
            let bytes = response.bytes().await?;
            let diagnostics = extract_api_error_diagnostics(
                &response_headers,
                bytes.as_ref(),
                self.openrouter_metadata_requested,
            );
            let provider_label = self.provider_label_for_diagnostics(diagnostics.as_ref());
            let message = user_facing_api_error_message_for(status, bytes.as_ref(), provider_label);
            span.record("error", message.as_str());
            tracing::error!(
                status = %status,
                error_message = %message,
                body_preview = %Self::body_preview(bytes.as_ref()),
                model_id = %model_id,
                "messages API error"
            );
            return Err(api_error(
                status,
                message,
                model_metadata,
                retry_after_secs,
                should_retry,
                diagnostics,
                parse_error_code(bytes.as_ref()),
            ));
        }

        let model_metadata = extract_model_metadata(response.headers());

        // Strip UTF-8 BOM if present
        const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];
        let mut is_first = true;
        let byte_stream = response.bytes_stream().map(move |result| {
            result.map(|bytes| {
                if is_first {
                    is_first = false;
                    if bytes.starts_with(UTF8_BOM) {
                        return bytes.slice(UTF8_BOM.len()..);
                    }
                }
                bytes
            })
        });

        // Turn raw bytes into SSE events
        let event_stream = byte_stream.eventsource();

        // Map SSE events into MessageStreamEvent.
        // Uses `scan` so transport errors terminate the stream after the first
        // error (same pattern as `chat_completion_stream`).
        let events = event_stream
            .scan(false, |had_transport_error, event_res| {
                if *had_transport_error {
                    return std::future::ready(None);
                }
                let item = match event_res {
                    Ok(event) => {
                        let data = &event.data;
                        if data == "[DONE]" {
                            return std::future::ready(None);
                        }

                        tracing::info!(
                            target: crate::inference_log::TARGET,
                            event = "sse_chunk",
                            backend = "messages",
                            data = %data,
                        );

                        if let Some(stream_error) = try_parse_stream_error(data) {
                            Some(Err(stream_error))
                        } else {
                            Some(
                                serde_json::from_str::<messages::MessageStreamEvent>(data).map_err(
                                    |e| {
                                        tracing::error!(
                                            error = %e,
                                            raw_data = %data,
                                            "Failed to deserialize MessageStreamEvent from stream"
                                        );
                                        InferenceError::Serialization(e)
                                    },
                                ),
                            )
                        }
                    }
                    Err(e) => {
                        *had_transport_error = true;
                        Some(Err(InferenceError::EventStreamError(e.to_string())))
                    }
                };
                std::future::ready(item)
            })
            .boxed();

        Ok((events, model_metadata))
    }

    // =========================================================================
    // Unified Conversation API
    // =========================================================================

    /// Apply default configuration to a ConversationRequest.
    fn apply_conversation_defaults(&self, request: &mut ConversationRequest) -> Result<()> {
        if request.model.is_none() {
            request.model = Some(self.defaults.model.clone());
        }

        if request.temperature.is_none() {
            request.temperature = self.defaults.temperature;
        }

        if request.top_p.is_none() {
            request.top_p = self.defaults.top_p;
        }

        if request.max_output_tokens.is_none() {
            request.max_output_tokens = self.defaults.max_completion_tokens;
        }

        Ok(())
    }

    /// Send a conversation request using the Chat Completions API (streaming).
    ///
    /// Converts the `ConversationRequest` to `ChatCompletionRequest` internally.
    /// Returns the stream and any model metadata extracted from response headers.
    pub async fn conversation_stream(
        &self,
        mut request: ConversationRequest,
    ) -> Result<(
        BoxStream<'static, Result<ChatCompletionChunk>>,
        Option<ResponseModelMetadata>,
    )> {
        self.apply_conversation_defaults(&mut request)?;

        let trace = request.trace.take();
        let mut chat_request: ChatCompletionRequest = request.into();
        if let Some(trace) = trace {
            chat_request.trace = Some(trace);
        }

        self.chat_completion_stream(chat_request).await
    }

    /// Send a conversation request using the Chat Completions API (non-streaming).
    ///
    /// Converts the `ConversationRequest` to `ChatCompletionRequest` internally.
    pub async fn conversation(
        &self,
        mut request: ConversationRequest,
    ) -> Result<ChatCompletionResponse> {
        self.apply_conversation_defaults(&mut request)?;

        let trace = request.trace.take();
        let mut chat_request: ChatCompletionRequest = request.into();
        if let Some(trace) = trace {
            chat_request.trace = Some(trace);
        }

        self.chat_completion(chat_request).await
    }

    /// Send a conversation request using the Responses API (streaming).
    ///
    /// Converts the `ConversationRequest` to Responses API format internally.
    /// The third tuple element is the per-request doom-loop signal collector
    /// (see [`Self::create_response_stream`]); callers that don't consume the
    /// signals can ignore it.
    #[allow(clippy::type_complexity)]
    pub async fn conversation_stream_responses(
        &self,
        mut request: ConversationRequest,
    ) -> Result<(
        BoxStream<'static, Result<rs::ResponseStreamEvent>>,
        Option<ResponseModelMetadata>,
        Option<crate::doom_loop::DoomLoopSignalCollector>,
    )> {
        self.apply_conversation_defaults(&mut request)?;

        let trace = request.trace.take();
        let x_grok_conv_id = request.x_grok_conv_id.clone();
        let x_grok_req_id = request.x_grok_req_id.clone();
        let x_grok_session_id = request.x_grok_session_id.clone();
        let x_grok_turn_idx = request.x_grok_turn_idx.clone();
        let x_grok_agent_id = request.x_grok_agent_id.clone();

        // Collect xAI-specific tools that can't be expressed via rs::Tool
        // (e.g., x_search). These are injected as raw JSON after serialization.
        let extra_tools = xai_grok_inference_types::extra_tool_entries(&request.hosted_tools);

        let responses_request: rs::CreateResponse = (&request).into();

        let mut wrapper = CreateResponseWrapper::new(responses_request);
        wrapper.x_grok_conv_id = x_grok_conv_id;
        wrapper.x_grok_req_id = x_grok_req_id;
        wrapper.x_grok_session_id = x_grok_session_id;
        wrapper.x_grok_turn_idx = x_grok_turn_idx;
        wrapper.x_grok_agent_id = x_grok_agent_id;
        wrapper.extra_tool_entries = extra_tools;

        if let Some(trace) = trace {
            wrapper.trace = Some(trace);
        }

        self.create_response_stream(wrapper).await
    }

    /// Send a conversation request using the Responses API (non-streaming).
    ///
    /// Converts the `ConversationRequest` to Responses API format internally.
    pub async fn conversation_responses(
        &self,
        mut request: ConversationRequest,
    ) -> Result<rs::Response> {
        self.apply_conversation_defaults(&mut request)?;

        let trace = request.trace.take();
        let x_grok_conv_id = request.x_grok_conv_id.clone();
        let x_grok_req_id = request.x_grok_req_id.clone();
        let x_grok_session_id = request.x_grok_session_id.clone();
        let x_grok_turn_idx = request.x_grok_turn_idx.clone();
        let x_grok_agent_id = request.x_grok_agent_id.clone();

        let responses_request: rs::CreateResponse = (&request).into();

        let mut wrapper = CreateResponseWrapper::new(responses_request);
        wrapper.x_grok_conv_id = x_grok_conv_id;
        wrapper.x_grok_req_id = x_grok_req_id;
        wrapper.x_grok_session_id = x_grok_session_id;
        wrapper.x_grok_turn_idx = x_grok_turn_idx;
        wrapper.x_grok_agent_id = x_grok_agent_id;

        if let Some(trace) = trace {
            wrapper.trace = Some(trace);
        }

        self.create_response(wrapper).await
    }

    /// Send a conversation request using the Anthropic Messages API (streaming).
    ///
    /// Converts the `ConversationRequest` to Messages API format internally.
    pub async fn conversation_stream_messages(
        &self,
        mut request: ConversationRequest,
    ) -> Result<(
        BoxStream<'static, Result<messages::MessageStreamEvent>>,
        Option<ResponseModelMetadata>,
    )> {
        self.apply_conversation_defaults(&mut request)?;

        let trace = request.trace.take();
        let x_grok_conv_id = request.x_grok_conv_id.clone();
        let x_grok_req_id = request.x_grok_req_id.clone();
        let x_grok_session_id = request.x_grok_session_id.clone();
        let x_grok_turn_idx = request.x_grok_turn_idx.clone();
        let x_grok_agent_id = request.x_grok_agent_id.clone();

        let messages_request = build_messages_request(&request);

        let mut wrapper = MessagesRequestWrapper::new(messages_request);
        wrapper.x_grok_conv_id = x_grok_conv_id;
        wrapper.x_grok_req_id = x_grok_req_id;
        wrapper.x_grok_session_id = x_grok_session_id;
        wrapper.x_grok_turn_idx = x_grok_turn_idx;
        wrapper.x_grok_agent_id = x_grok_agent_id;

        if let Some(trace) = trace {
            wrapper.trace = Some(trace);
        }

        self.create_message_stream(wrapper).await
    }

    /// Send a conversation request using the Anthropic Messages API (non-streaming).
    ///
    /// Converts the `ConversationRequest` to Messages API format internally.
    pub async fn conversation_messages(
        &self,
        mut request: ConversationRequest,
    ) -> Result<messages::MessagesResponse> {
        self.apply_conversation_defaults(&mut request)?;

        let trace = request.trace.take();
        let x_grok_conv_id = request.x_grok_conv_id.clone();
        let x_grok_req_id = request.x_grok_req_id.clone();
        let x_grok_session_id = request.x_grok_session_id.clone();
        let x_grok_turn_idx = request.x_grok_turn_idx.clone();
        let x_grok_agent_id = request.x_grok_agent_id.clone();

        let messages_request = build_messages_request(&request);

        let mut wrapper = MessagesRequestWrapper::new(messages_request);
        wrapper.x_grok_conv_id = x_grok_conv_id;
        wrapper.x_grok_req_id = x_grok_req_id;
        wrapper.x_grok_session_id = x_grok_session_id;
        wrapper.x_grok_turn_idx = x_grok_turn_idx;
        wrapper.x_grok_agent_id = x_grok_agent_id;

        if let Some(trace) = trace {
            wrapper.trace = Some(trace);
        }

        self.create_message(wrapper).await
    }

    /// Backend-aware streaming call that collects the full response.
    ///
    /// `ConversationRequest::project_response_field` is intentionally ignored
    /// here: collect is a non-actor consumer that needs the raw assistant
    /// text for schema validation. Live Text projection is applied only on
    /// the InferenceActor path (`run_one_attempt`).
    pub async fn conversation_collect(
        &self,
        request: ConversationRequest,
    ) -> Result<ConversationResponse> {
        debug_assert!(
            !request.project_response_field,
            "conversation_collect does not apply project_response_field; use the InferenceActor path for live projection"
        );
        let request_id = crate::types::RequestId::random();
        let idle_timeout = std::time::Duration::from_secs(300);
        let result = match self.api_backend() {
            ApiBackend::ChatCompletions => {
                let (raw, meta) = self.conversation_stream(request).await?;
                let events = crate::stream::stream_chat_completions(
                    raw,
                    meta,
                    request_id,
                    idle_timeout,
                    Some(&self.defaults.model),
                    if self.is_openrouter() {
                        crate::config::ProviderIdentity::OpenRouter
                    } else {
                        crate::config::ProviderIdentity::Custom
                    },
                );
                crate::stream::collect_response(events).await
            }
            ApiBackend::Responses => {
                let (raw, meta, doom_loop) = self.conversation_stream_responses(request).await?;
                let events =
                    crate::stream::stream_responses(raw, meta, request_id, idle_timeout, doom_loop);
                crate::stream::collect_response(events).await
            }
            ApiBackend::Messages => {
                let (raw, meta) = self.conversation_stream_messages(request).await?;
                let events = crate::stream::stream_messages(raw, meta, request_id, idle_timeout);
                crate::stream::collect_response(events).await
            }
        };
        result
            .map(|(response, _metrics)| response)
            .map_err(|info| InferenceError::Api {
                status: info
                    .status_code
                    .and_then(|c| reqwest::StatusCode::from_u16(c).ok())
                    .unwrap_or(reqwest::StatusCode::INTERNAL_SERVER_ERROR),
                message: info.message,
                model_metadata: info.model_metadata,
                retry_after_secs: info.retry_after_secs,
                should_retry: None,
                diagnostics: info.diagnostics,
                error_code: info.error_code,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;
    use xai_grok_inference_types::types::ChatRequestMessage;

    fn minimal_config() -> InferenceConfig {
        InferenceConfig {
            api_key: Some("test-key".to_string()),
            base_url: "https://example.test".to_string(),
            model: "test-model".to_string(),
            max_completion_tokens: None,
            temperature: None,
            top_p: None,
            openrouter_fallback_models: Vec::new(),
            openrouter_provider_preferences: None,
            openrouter_plugins: Vec::new(),
            openrouter_pacing: false,
            zai_tool_stream: false,
            zai_thinking: None,

            api_backend: ApiBackend::ChatCompletions,
            include_message_model_id: true,
            auth_scheme: AuthScheme::Bearer,
            extra_headers: IndexMap::new(),
            context_window: 8192,
            force_http1: false,
            max_retries: None,
            stream_tool_calls: false,
            idle_timeout_secs: None,
            reasoning_effort: None,
            origin_client: None,
            client_identifier: None,
            deployment_id: None,
            user_id: None,
            client_version: None,
            attribution_callback: None,
            bearer_resolver: None,
            supports_backend_search: false,
            supports_native_schema: None,
            supports_strict_tools: None,
            // Explicit unknown: None means the model capability is not known.
            supports_image_input: None,
            supports_audio_input: None,
            supports_video_input: None,
            compactions_remaining: None,
            compaction_at_tokens: None,
            doom_loop_recovery: None,
            header_injector: None,
            provider_identity: crate::config::ProviderIdentity::default(),
        }
    }

    /// Verify the serialized shape of StreamingChatRequest matches the
    /// expected wire format: all ChatCompletionRequest fields flattened at
    /// top level, plus `stream: true` and `stream_options.include_usage: true`.
    #[test]
    fn streaming_chat_request_serializes_correctly() {
        let request = ChatCompletionRequest {
            model: Some("test-model".into()),
            messages: vec![ChatRequestMessage::user("hello")],
            temperature: Some(0.7),
            max_tokens: None,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            user: None,
            tools: None,
            tool_choice: None,
            search_parameters: None,
            response_format: None,
            reasoning_effort: None,
            x_grok_conv_id: None,
            x_grok_req_id: None,
            x_grok_session_id: None,
            x_grok_turn_idx: None,
            x_grok_agent_id: None,
            x_grok_deployment_id: None,
            x_grok_user_id: None,
            trace: None,
        };

        let wrapper = StreamingChatRequest {
            inner: &request,
            models: None,
            provider: None,
            plugins: None,
            reasoning: None,
            tool_stream: false,
            thinking: None,
            stream: true,
            stream_options: StreamOptions {
                include_usage: true,
            },
        };

        let json: serde_json::Value = serde_json::to_value(&wrapper).unwrap();
        let obj = json.as_object().unwrap();

        assert_eq!(obj.get("stream").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            obj.get("stream_options")
                .and_then(|v| v.get("include_usage"))
                .and_then(|v| v.as_bool()),
            Some(true)
        );

        assert!(
            obj.get("inner").is_none(),
            "inner field should be flattened"
        );
        assert_eq!(
            obj.get("model").and_then(|v| v.as_str()),
            Some("test-model")
        );
        assert!(obj.get("messages").is_some());
        let temp = obj.get("temperature").and_then(|v| v.as_f64()).unwrap();
        assert!((temp - 0.7).abs() < 0.001, "temperature should be ~0.7");

        assert!(obj.get("max_tokens").is_none());
        assert!(obj.get("tools").is_none());
        assert!(obj.get("models").is_none());
    }

    #[test]
    fn chat_request_serializes_openrouter_fallback_models_only_when_configured() {
        let request = ChatCompletionRequest::new(
            "openai/gpt-oss-120b",
            vec![ChatRequestMessage::user("hello")],
        );
        let fallbacks = vec![
            "openai/gpt-oss-20b".to_string(),
            "meta-llama/llama-3.3-70b-instruct".to_string(),
        ];

        let with_fallbacks = serde_json::to_value(ChatRequestWithFallbacks {
            inner: &request,
            models: Some(&fallbacks),
            provider: None,
            plugins: None,
            reasoning: None,
            tool_stream: false,
            thinking: None,
        })
        .unwrap();
        assert_eq!(
            with_fallbacks["model"],
            serde_json::json!("openai/gpt-oss-120b"),
            "the ordinary model field remains the OpenRouter primary"
        );
        assert_eq!(with_fallbacks["models"], serde_json::json!(fallbacks));

        let without_fallbacks = serde_json::to_value(ChatRequestWithFallbacks {
            inner: &request,
            models: None,
            provider: None,
            plugins: None,
            reasoning: None,
            tool_stream: false,
            thinking: None,
        })
        .unwrap();
        assert!(
            without_fallbacks.get("models").is_none(),
            "an empty configuration must not change OpenAI-compatible bodies"
        );
    }

    #[test]
    fn responses_request_serializes_openrouter_fallback_models_only_when_configured() {
        let fallbacks = vec!["openai/gpt-5-mini".to_string()];
        let mut request_body = serde_json::json!({
            "model": "openai/gpt-oss-120b",
            "input": "hello"
        });

        add_openrouter_fallback_models(&mut request_body, Some(&fallbacks));
        assert_eq!(
            request_body["model"],
            serde_json::json!("openai/gpt-oss-120b")
        );
        assert_eq!(request_body["models"], serde_json::json!(fallbacks));

        let mut standard_body = serde_json::json!({ "model": "gpt-5" });
        add_openrouter_fallback_models(&mut standard_body, None);
        assert!(standard_body.get("models").is_none());
    }

    #[test]
    fn chat_request_serializes_provider_preferences_when_configured() {
        let request = ChatCompletionRequest::new(
            "openai/gpt-oss-120b",
            vec![ChatRequestMessage::user("hello")],
        );
        let prefs = crate::config::OpenRouterProviderPreferences {
            sort: Some(crate::config::OpenRouterSort::name("latency")),
            order: vec!["deepinfra/turbo".to_string()],
            allow_fallbacks: Some(true),
            require_parameters: Some(true),
            data_collection: Some("deny".to_string()),
            zdr: Some(true),
            quantizations: vec!["int8".to_string()],
            max_price: Some(crate::config::OpenRouterMaxPrice {
                prompt: Some(0.5),
                completion: Some(2.0),
            }),
            ..Default::default()
        };

        let serialized = serde_json::to_value(ChatRequestWithFallbacks {
            inner: &request,
            models: None,
            provider: Some(&prefs),
            plugins: None,
            reasoning: None,
            tool_stream: false,
            thinking: None,
        })
        .unwrap();
        let provider = &serialized["provider"];
        assert_eq!(provider["sort"], serde_json::json!("latency"));
        assert_eq!(provider["order"], serde_json::json!(["deepinfra/turbo"]));
        assert_eq!(provider["allow_fallbacks"], serde_json::json!(true));
        assert_eq!(provider["require_parameters"], serde_json::json!(true));
        assert_eq!(provider["data_collection"], serde_json::json!("deny"));
        assert_eq!(provider["zdr"], serde_json::json!(true));
        assert_eq!(provider["quantizations"], serde_json::json!(["int8"]));
        assert_eq!(provider["max_price"]["prompt"], serde_json::json!(0.5));
        assert_eq!(provider["max_price"]["completion"], serde_json::json!(2.0));
    }

    #[test]
    fn chat_request_omits_provider_key_when_preferences_empty() {
        let request = ChatCompletionRequest::new(
            "openai/gpt-oss-120b",
            vec![ChatRequestMessage::user("hello")],
        );
        // An all-empty preferences object must not produce a `provider` key.
        let empty_prefs = crate::config::OpenRouterProviderPreferences::default();
        assert!(empty_prefs.is_empty());
        // Wire gate: empty prefs produce `None` via openrouter_provider_preferences().
        let wire_serialized = serde_json::to_value(ChatRequestWithFallbacks {
            inner: &request,
            models: None,
            provider: None,
            plugins: None,
            reasoning: None,
            tool_stream: false,
            thinking: None,
        })
        .unwrap();
        assert!(
            wire_serialized.get("provider").is_none(),
            "no `provider` key when preferences are absent"
        );
    }

    #[test]
    fn chat_request_omits_none_fields_and_empty_arrays_in_provider() {
        let request = ChatCompletionRequest::new(
            "openai/gpt-oss-120b",
            vec![ChatRequestMessage::user("hello")],
        );
        let prefs = crate::config::OpenRouterProviderPreferences {
            sort: Some(crate::config::OpenRouterSort::name("latency")),
            // only and ignore are empty arrays — must be omitted
            only: vec![],
            ignore: vec![],
            data_collection: Some("deny".to_string()),
            // allow_fallbacks, require_parameters, zdr, quantizations, max_price are None/empty
            ..Default::default()
        };

        let serialized = serde_json::to_value(ChatRequestWithFallbacks {
            inner: &request,
            models: None,
            provider: Some(&prefs),
            plugins: None,
            reasoning: None,
            tool_stream: false,
            thinking: None,
        })
        .unwrap();
        let provider = &serialized["provider"];
        assert_eq!(provider["sort"], serde_json::json!("latency"));
        assert_eq!(provider["data_collection"], serde_json::json!("deny"));
        assert!(
            provider.get("only").is_none(),
            "empty arrays must be omitted"
        );
        assert!(
            provider.get("ignore").is_none(),
            "empty arrays must be omitted"
        );
        assert!(
            provider.get("allow_fallbacks").is_none(),
            "None fields must be omitted"
        );
        assert!(
            provider.get("max_price").is_none(),
            "None fields must be omitted"
        );
    }

    #[test]
    fn client_omits_provider_for_non_openrouter_identity() {
        // A InferenceConfig with provider_identity = OpenAi must not emit
        // the `provider` key even when preferences are configured.
        let prefs = crate::config::OpenRouterProviderPreferences {
            data_collection: Some("deny".to_string()),
            ..Default::default()
        };
        let cfg = InferenceConfig {
            provider_identity: crate::config::ProviderIdentity::OpenAi,
            openrouter_provider_preferences: Some(prefs),
            ..InferenceConfig::default()
        };
        let client = InferenceClient::new(cfg).expect("client should build");
        assert!(
            client.openrouter_provider_preferences().is_none(),
            "non-OpenRouter identity must suppress provider preferences"
        );
    }

    #[test]
    fn client_supplies_provider_for_openrouter_identity() {
        let prefs = crate::config::OpenRouterProviderPreferences {
            data_collection: Some("deny".to_string()),
            require_parameters: Some(true),
            ..Default::default()
        };
        let cfg = InferenceConfig {
            provider_identity: crate::config::ProviderIdentity::OpenRouter,
            openrouter_provider_preferences: Some(prefs.clone()),
            ..InferenceConfig::default()
        };
        let client = InferenceClient::new(cfg).expect("client should build");
        let wire_prefs = client
            .openrouter_provider_preferences()
            .expect("OpenRouter identity should expose preferences");
        assert_eq!(wire_prefs.data_collection.as_deref(), Some("deny"));
        assert_eq!(wire_prefs.require_parameters, Some(true));
    }

    #[test]
    fn client_omits_provider_when_preferences_all_empty() {
        let cfg = InferenceConfig {
            provider_identity: crate::config::ProviderIdentity::OpenRouter,
            openrouter_provider_preferences: Some(
                crate::config::OpenRouterProviderPreferences::default(),
            ),
            ..InferenceConfig::default()
        };
        let client = InferenceClient::new(cfg).expect("client should build");
        assert!(
            client.openrouter_provider_preferences().is_none(),
            "an all-empty preferences object must be omitted from the wire"
        );
    }

    // ── plugins wire shape / suppression ─────────────────────────────────

    #[test]
    fn client_supplies_plugins_for_openrouter_identity() {
        let plugins = vec![
            crate::config::OpenRouterPlugin {
                id: "response-healing".to_string(),
                ..Default::default()
            },
            crate::config::OpenRouterPlugin {
                id: "web".to_string(),
                extra: indexmap::IndexMap::from([(
                    "max_results".to_string(),
                    serde_json::json!(3),
                )]),
            },
        ];
        let cfg = InferenceConfig {
            provider_identity: crate::config::ProviderIdentity::OpenRouter,
            openrouter_plugins: plugins.clone(),
            ..InferenceConfig::default()
        };
        let client = InferenceClient::new(cfg).expect("client should build");
        let wire_plugins = client
            .openrouter_plugins()
            .expect("OpenRouter identity should expose plugins");
        assert_eq!(wire_plugins.len(), 2);
        assert_eq!(wire_plugins[0].id, "response-healing");
        assert_eq!(wire_plugins[1].id, "web");
        assert_eq!(
            wire_plugins[1].extra.get("max_results"),
            Some(&serde_json::json!(3))
        );

        // Wire serialization carries the plugins array next to provider/models.
        let request = ChatCompletionRequest::new(
            "openai/gpt-oss-120b",
            vec![ChatRequestMessage::user("hello")],
        );
        let payload = client.apply_defaults(request).unwrap();
        let serialized = serde_json::to_value(&ChatRequestWithFallbacks {
            inner: &payload,
            models: client.openrouter_fallback_models(),
            provider: client.openrouter_provider_preferences(),
            plugins: client.openrouter_plugins(),
            reasoning: None,
            tool_stream: false,
            thinking: None,
        })
        .unwrap();
        let wire = serialized
            .get("plugins")
            .and_then(|v| v.as_array())
            .expect("plugins array must be present on the wire");
        assert_eq!(wire.len(), 2);
        assert_eq!(wire[0]["id"], "response-healing");
        assert_eq!(wire[1]["id"], "web");
        assert_eq!(wire[1]["max_results"], 3);
        // Flattened extras must not nest under an "extra" key.
        assert!(wire[1].get("extra").is_none());
    }

    #[test]
    fn client_omits_plugins_when_empty_for_openrouter() {
        let cfg = InferenceConfig {
            provider_identity: crate::config::ProviderIdentity::OpenRouter,
            openrouter_plugins: Vec::new(),
            ..InferenceConfig::default()
        };
        let client = InferenceClient::new(cfg).expect("client should build");
        assert!(
            client.openrouter_plugins().is_none(),
            "an empty plugins list must be omitted from the wire"
        );
    }

    #[test]
    fn client_omits_plugins_for_non_openrouter_identity() {
        let plugins = vec![crate::config::OpenRouterPlugin {
            id: "web".to_string(),
            ..Default::default()
        }];
        let cfg = InferenceConfig {
            provider_identity: crate::config::ProviderIdentity::OpenAi,
            openrouter_plugins: plugins,
            ..InferenceConfig::default()
        };
        let client = InferenceClient::new(cfg).expect("client should build");
        assert!(
            client.openrouter_plugins().is_none(),
            "non-OpenRouter identity must suppress plugins"
        );
    }

    #[test]
    fn openai_compatible_history_omits_internal_message_model_id() {
        let mut config = minimal_config();
        config.include_message_model_id = false;
        let client = InferenceClient::new(config).unwrap();
        let request = ChatCompletionRequest::new(
            "openai/gpt-oss-120b",
            vec![
                ChatRequestMessage::user("first"),
                ChatRequestMessage::assistant("answer", "openai/gpt-oss-120b", None),
                ChatRequestMessage::user("second"),
            ],
        );

        let payload = client.apply_defaults(request).unwrap();
        assert!(payload.messages[1].model_id.is_none());
        let json = serde_json::to_value(payload).unwrap();
        assert!(
            json["messages"][1].get("model_id").is_none(),
            "internal model metadata must not reach strict OpenAI-compatible providers"
        );
    }

    #[test]
    fn xai_history_preserves_message_model_id() {
        let client = InferenceClient::new(minimal_config()).unwrap();
        let request = ChatCompletionRequest::new(
            "grok-test",
            vec![ChatRequestMessage::assistant(
                "answer",
                "grok-previous",
                None,
            )],
        );

        let payload = client.apply_defaults(request).unwrap();
        assert_eq!(
            payload.messages[0].model_id.as_deref(),
            Some("grok-previous")
        );
    }

    #[test]
    fn extract_retry_after_parses_seconds() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::RETRY_AFTER, "30".parse().unwrap());
        assert_eq!(extract_retry_after(&headers), Some(30));
    }

    #[test]
    fn extract_retry_after_caps_at_120() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::RETRY_AFTER, "3600".parse().unwrap());
        assert_eq!(extract_retry_after(&headers), Some(120));
    }

    #[test]
    fn extract_retry_after_zero_is_valid() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::RETRY_AFTER, "0".parse().unwrap());
        assert_eq!(extract_retry_after(&headers), Some(0));
    }

    #[test]
    fn extract_retry_after_ignores_http_date() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::RETRY_AFTER,
            "Fri, 31 Dec 2025 23:59:59 GMT".parse().unwrap(),
        );
        assert_eq!(extract_retry_after(&headers), None);
    }

    #[test]
    fn extract_retry_after_none_when_missing() {
        let headers = reqwest::header::HeaderMap::new();
        assert_eq!(extract_retry_after(&headers), None);
    }

    #[test]
    fn extract_should_retry_true() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("x-should-retry", "true".parse().unwrap());
        assert_eq!(extract_should_retry(&headers), Some(true));
    }

    #[test]
    fn extract_should_retry_true_case_insensitive() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("x-should-retry", "TRUE".parse().unwrap());
        assert_eq!(extract_should_retry(&headers), Some(true));
    }

    #[test]
    fn extract_should_retry_false() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("x-should-retry", "false".parse().unwrap());
        assert_eq!(extract_should_retry(&headers), Some(false));
    }

    #[test]
    fn extract_should_retry_unknown_value_is_none() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("x-should-retry", "banana".parse().unwrap());
        assert_eq!(extract_should_retry(&headers), None);
    }

    #[test]
    fn extract_should_retry_absent_is_none() {
        let headers = reqwest::header::HeaderMap::new();
        assert_eq!(extract_should_retry(&headers), None);
    }

    #[test]
    fn extracts_openrouter_error_and_rate_limit_diagnostics() {
        let mut headers = HeaderMap::new();
        headers.insert("x-ratelimit-limit", HeaderValue::from_static("60"));
        headers.insert("x-ratelimit-remaining", HeaderValue::from_static("0"));
        headers.insert("x-ratelimit-reset", HeaderValue::from_static("60"));
        headers.insert("x-generation-id", HeaderValue::from_static("gen_abc"));
        let body = br#"{
            "error": {
                "metadata": {
                    "error_type": "provider_error",
                    "provider_code": "rate_limit_exceeded",
                    "provider_name": "Example Upstream"
                }
            }
        }"#;

        let diagnostics = extract_api_error_diagnostics(&headers, body, true).unwrap();
        assert_eq!(diagnostics.error_type.as_deref(), Some("provider_error"));
        assert_eq!(
            diagnostics.provider_code.as_deref(),
            Some("rate_limit_exceeded")
        );
        assert_eq!(
            diagnostics.provider_name.as_deref(),
            Some("Example Upstream")
        );
        assert_eq!(diagnostics.rate_limit_limit.as_deref(), Some("60"));
        assert_eq!(diagnostics.rate_limit_remaining.as_deref(), Some("0"));
        assert_eq!(diagnostics.rate_limit_reset.as_deref(), Some("60"));
        assert_eq!(diagnostics.generation_id.as_deref(), Some("gen_abc"));
    }

    #[test]
    fn extracts_upstream_from_current_openrouter_metadata_envelope() {
        let body = br#"{
            "error": {
                "code": 429,
                "message": "Provider returned error"
            },
            "openrouter_metadata": {
                "strategy": "fallback",
                "attempt": 2,
                "attempts": [
                    {"provider": "First Provider", "status": 429},
                    {"provider": "Second Provider", "status": 429}
                ]
            }
        }"#;

        let diagnostics = extract_api_error_diagnostics(&HeaderMap::new(), body, true).unwrap();
        assert_eq!(
            diagnostics.provider_name.as_deref(),
            Some("Second Provider")
        );
        assert_eq!(diagnostics.provider_code.as_deref(), Some("429"));
    }

    #[test]
    fn labels_router_when_openrouter_omits_upstream_name() {
        let diagnostics = extract_api_error_diagnostics(&HeaderMap::new(), b"{}", true).unwrap();
        assert_eq!(diagnostics.provider_name.as_deref(), Some("OpenRouter"));
        assert!(diagnostics.provider_code.is_none());
    }

    #[test]
    fn new_with_minimal_config_succeeds() {
        let client = InferenceClient::new(minimal_config()).expect("client should construct");
        assert_eq!(client.api_backend(), ApiBackend::ChatCompletions);
    }

    #[test]
    fn new_applies_extra_headers() {
        let mut cfg = minimal_config();
        cfg.extra_headers
            .insert("x-test-header".to_string(), "test-value".to_string());
        cfg.extra_headers
            .insert("x-XAI-token-auth".to_string(), "xai-grok-cli".to_string());
        let _client =
            InferenceClient::new(cfg).expect("client with extra headers should construct");
    }

    #[test]
    fn messages_plus_anthropic_api_key_uses_x_api_key_and_not_authorization() {
        let cfg = InferenceConfig {
            api_key: Some("anthropic-key-abc123".to_string()),
            api_backend: ApiBackend::Messages,
            auth_scheme: AuthScheme::XApiKey,
            ..minimal_config()
        };
        let client = InferenceClient::new(cfg).expect("client should build");
        assert!(
            client
                .default_headers
                .get(HeaderName::from_static("x-api-key"))
                .is_some()
        );
        assert!(client.default_headers.get(AUTHORIZATION).is_none());
    }

    #[test]
    fn messages_plus_bearer_uses_authorization_and_not_x_api_key() {
        let cfg = InferenceConfig {
            api_key: Some("bearer-key-abc123".to_string()),
            api_backend: ApiBackend::Messages,
            auth_scheme: AuthScheme::Bearer,
            ..minimal_config()
        };
        let client = InferenceClient::new(cfg).expect("client should build");
        assert!(client.default_headers.get(AUTHORIZATION).is_some());
        assert!(
            client
                .default_headers
                .get(HeaderName::from_static("x-api-key"))
                .is_none(),
            "Messages protocol alone must not inject x-api-key"
        );
        // Direct Anthropic identity headers must not appear solely because the
        // backend is Messages; only AnthropicClient pins anthropic-version.
        assert!(
            client
                .default_headers
                .get(HeaderName::from_static("anthropic-version"))
                .is_none(),
            "Messages protocol alone must not inject anthropic-version"
        );
    }

    // Regression: a past change dropped User-Agent from sampling requests.
    #[test]
    fn sampling_client_always_has_user_agent() {
        let client = InferenceClient::new(minimal_config()).expect("build");
        assert!(client.default_headers.contains_key(USER_AGENT));
    }

    // Regression: a past change dropped HeaderInjector (traceparent) from sampling requests.
    #[test]
    fn header_injector_is_called_in_post() {
        #[derive(Debug)]
        struct TestInjector;
        impl crate::config::HeaderInjector for TestInjector {
            fn inject(&self, headers: &mut HeaderMap) {
                headers.insert(
                    HeaderName::from_static("traceparent"),
                    HeaderValue::from_static("00-test-trace-id-00"),
                );
            }
        }

        let mut config = minimal_config();
        config.header_injector = Some(std::sync::Arc::new(TestInjector));
        let client = InferenceClient::new(config).expect("build");
        let req = client
            .post("http://localhost/test")
            .builder
            .build()
            .expect("build request");
        assert!(
            req.headers().contains_key("traceparent"),
            "HeaderInjector should inject traceparent into post() requests"
        );
    }

    #[test]
    fn user_agent_includes_origin_and_agent_product() {
        let origin = OriginClientInfo {
            product: "my-client".to_string(),
            version: Some("1.2.3".to_string()),
        };
        let ua = user_agent_string_for(&origin);
        assert!(ua.contains("my-client/1.2.3"));
        assert!(ua.contains(AGENT_PRODUCT));
    }

    #[test]
    fn user_agent_omits_origin_version_when_absent() {
        let origin = OriginClientInfo {
            product: "my-client".to_string(),
            version: None,
        };
        let ua = user_agent_string_for(&origin);
        // No slash between product and the grok-shell agent product.
        assert!(ua.starts_with("my-client grok-shell/"));
    }

    #[test]
    fn user_agent_collapses_when_origin_matches_agent() {
        let agent_version = xai_grok_version::VERSION.to_string();
        let origin = OriginClientInfo {
            product: AGENT_PRODUCT.to_string(),
            version: Some(agent_version.clone()),
        };
        let ua = user_agent_string_for(&origin);
        // Single product/version slot when the origin and agent match.
        assert!(ua.starts_with(&format!("{}/{}", AGENT_PRODUCT, agent_version)));
    }

    /// Counts callbacks for assertions in the tests below.
    #[derive(Default, Debug)]
    struct CountingCallback {
        invocations: std::sync::Mutex<Vec<(crate::attribution::InferenceConsumer, Option<String>)>>,
    }

    #[derive(Debug)]
    struct StaticBearerResolver(&'static str);

    impl crate::config::BearerResolver for StaticBearerResolver {
        fn current_bearer(&self) -> Option<String> {
            Some(self.0.to_string())
        }
    }

    impl crate::attribution::Auth401AttributionCallback for CountingCallback {
        fn record_401(
            &self,
            consumer: crate::attribution::InferenceConsumer,
            sent_bearer_tail: Option<&str>,
        ) {
            self.invocations
                .lock()
                .unwrap()
                .push((consumer, sent_bearer_tail.map(str::to_owned)));
        }
    }

    /// `extract_sent_bearer` strips `Bearer ` and keeps the shared tail.
    #[test]
    fn extract_sent_bearer_uses_tail_for_openai_compat() {
        let cfg = InferenceConfig {
            api_key: Some("test-bearer-1234567890".to_string()),
            api_backend: ApiBackend::ChatCompletions,
            ..minimal_config()
        };
        let client = InferenceClient::new(cfg).expect("client should build");
        let bearer = client.extract_sent_bearer();
        assert_eq!(bearer.as_deref(), Some("r-1234567890"));
        assert_eq!(
            bearer.as_deref().map(str::chars).map(Iterator::count),
            Some(crate::attribution::BEARER_TAIL_CHARS),
        );
    }

    /// `extract_sent_bearer` reads the `x-api-key` tail for Messages.
    #[test]
    fn extract_sent_bearer_reads_x_api_key_for_messages() {
        let cfg = InferenceConfig {
            api_key: Some("anthropic-key-abc123".to_string()),
            api_backend: ApiBackend::Messages,
            auth_scheme: AuthScheme::XApiKey,
            ..minimal_config()
        };
        let client = InferenceClient::new(cfg).expect("client should build");
        let bearer = client.extract_sent_bearer();
        assert_eq!(bearer.as_deref(), Some("c-key-abc123"));
        assert_eq!(
            bearer.as_deref().map(str::chars).map(Iterator::count),
            Some(crate::attribution::BEARER_TAIL_CHARS),
        );
    }

    #[test]
    fn sent_bearer_tail_is_unicode_safe() {
        let cfg = InferenceConfig {
            api_key: Some("aébcdefghijkl".to_owned()),
            api_backend: ApiBackend::ChatCompletions,
            ..minimal_config()
        };
        let client = InferenceClient::new(cfg).expect("client should build");
        assert_eq!(
            client.extract_sent_bearer().as_deref(),
            Some("ébcdefghijkl")
        );
    }

    /// `extract_sent_bearer` returns `None` when no auth header is set.
    #[test]
    fn extract_sent_bearer_returns_none_when_no_header() {
        let cfg = InferenceConfig {
            api_key: None,
            api_backend: ApiBackend::ChatCompletions,
            ..minimal_config()
        };
        let client = InferenceClient::new(cfg).expect("client should build");
        assert!(client.extract_sent_bearer().is_none());
    }

    #[test]
    fn live_bearer_resolver_uses_authorization_for_messages_plus_bearer() {
        let cfg = InferenceConfig {
            api_key: Some("stale-bearer".to_string()),
            api_backend: ApiBackend::Messages,
            auth_scheme: AuthScheme::Bearer,
            bearer_resolver: Some(std::sync::Arc::new(StaticBearerResolver("fresh-bearer"))),
            ..minimal_config()
        };
        let client = InferenceClient::new(cfg).expect("client should build");
        let request = client
            .post("https://example.test/v1/messages")
            .builder
            .build()
            .expect("request should build");
        let auth = request
            .headers()
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok());
        assert_eq!(auth, Some("Bearer fresh-bearer"));
        assert!(request.headers().get("x-api-key").is_none());
    }

    /// Regression: when `api_key` (which seeds `default_headers` with an
    /// `Authorization: Bearer ...`) AND a `bearer_resolver` are both set,
    /// `post()` must produce **exactly one** `Authorization` header on the
    /// wire. The pre-fix code used `RequestBuilder::header(AUTHORIZATION, ...)`
    /// which appends rather than replaces, causing two identical
    /// `Authorization` headers and a 400 from cli-chat-proxy.
    #[test]
    fn post_emits_single_authorization_with_api_key_and_bearer_resolver() {
        let cfg = InferenceConfig {
            api_key: Some("stale-bearer".to_string()),
            api_backend: ApiBackend::Responses,
            auth_scheme: AuthScheme::Bearer,
            bearer_resolver: Some(std::sync::Arc::new(StaticBearerResolver("fresh-bearer"))),
            ..minimal_config()
        };
        let client = InferenceClient::new(cfg).expect("client should build");
        let request = client
            .post("https://example.test/v1/responses")
            .builder
            .build()
            .expect("request should build");
        let auth_count = request.headers().get_all(AUTHORIZATION).iter().count();
        assert_eq!(
            auth_count, 1,
            "expected exactly one Authorization header, got {auth_count}"
        );
        assert_eq!(
            request
                .headers()
                .get(AUTHORIZATION)
                .and_then(|v| v.to_str().ok()),
            Some("Bearer fresh-bearer"),
        );
    }

    #[test]
    fn live_bearer_resolver_uses_x_api_key_for_messages_plus_anthropic_api_key() {
        let cfg = InferenceConfig {
            api_key: Some("stale-anthropic".to_string()),
            api_backend: ApiBackend::Messages,
            auth_scheme: AuthScheme::XApiKey,
            bearer_resolver: Some(std::sync::Arc::new(StaticBearerResolver("fresh-anthropic"))),
            ..minimal_config()
        };
        let client = InferenceClient::new(cfg).expect("client should build");
        let request = client
            .post("https://example.test/v1/messages")
            .builder
            .build()
            .expect("request should build");
        let api_key = request
            .headers()
            .get("x-api-key")
            .and_then(|v| v.to_str().ok());
        assert_eq!(api_key, Some("fresh-anthropic"));
        assert!(request.headers().get(AUTHORIZATION).is_none());
    }

    /// Bearers shorter than the tail length pass through unchanged.
    #[test]
    fn extract_sent_bearer_short_bearer_passes_through_unchanged() {
        let cfg = InferenceConfig {
            api_key: Some("abc".to_string()),
            api_backend: ApiBackend::ChatCompletions,
            ..minimal_config()
        };
        let client = InferenceClient::new(cfg).expect("client should build");
        assert_eq!(client.extract_sent_bearer().as_deref(), Some("abc"));
    }

    /// The callback receives only the request-captured bearer tail.
    #[test]
    fn record_401_attribution_invokes_callback_with_captured_tail() {
        let cb = std::sync::Arc::new(CountingCallback::default());
        let cb_dyn: crate::attribution::SharedAttributionCallback = cb.clone();
        let cfg = InferenceConfig {
            api_key: Some("the-bearer-1234567890-extra-tail".to_string()),
            api_backend: ApiBackend::ChatCompletions,
            attribution_callback: Some(cb_dyn),
            bearer_resolver: None,
            ..minimal_config()
        };
        let client = InferenceClient::new(cfg).expect("client should build");
        let sent_bearer_tail = client
            .post("https://example.test/v1/chat/completions")
            .sent_bearer_tail;
        client.record_401_attribution(
            crate::attribution::InferenceConsumer::ChatCompletionsStream,
            sent_bearer_tail.as_deref(),
        );
        let calls = cb.invocations.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].0,
            crate::attribution::InferenceConsumer::ChatCompletionsStream
        );
        assert_eq!(calls[0].1.as_deref(), Some("0-extra-tail"));
        assert_eq!(
            calls[0].1.as_deref().map(str::chars).map(Iterator::count),
            Some(crate::attribution::BEARER_TAIL_CHARS),
        );
    }

    /// Regression test: when a bearer_resolver is wired, `post()` must
    /// *replace* the Authorization header from `default_headers`, not
    /// append a second one. Duplicate Authorization headers cause
    /// Cloudflare to return 400 Bad Request.
    #[test]
    fn bearer_resolver_replaces_authorization_header() {
        #[derive(Debug)]
        struct StaticResolver(String);
        impl crate::config::BearerResolver for StaticResolver {
            fn current_bearer(&self) -> Option<String> {
                Some(self.0.clone())
            }
        }

        let resolver: crate::config::SharedBearerResolver =
            std::sync::Arc::new(StaticResolver("fresh-token".to_string()));
        let cfg = InferenceConfig {
            api_key: Some("stale-token".to_string()),
            api_backend: ApiBackend::Responses,
            bearer_resolver: Some(resolver),
            ..minimal_config()
        };
        let client = InferenceClient::new(cfg).expect("client should build");

        // Build a request to inspect the final headers.
        let builder = client.post("https://example.test/v1/responses").builder;
        let request = builder.body("").build().expect("request should build");

        let auth_values: Vec<_> = request.headers().get_all(AUTHORIZATION).iter().collect();
        assert_eq!(
            auth_values.len(),
            1,
            "expected exactly one Authorization header, got {}: {:?}",
            auth_values.len(),
            auth_values
        );
        assert_eq!(
            auth_values[0].to_str().unwrap(),
            "Bearer fresh-token",
            "Authorization header should contain the resolver's fresh token"
        );
    }

    /// `record_401_attribution` is a no-op when `attribution_callback`
    /// is `None` (the BYOK / sampler-only path). The previous tests
    /// in this module construct clients without a callback and rely
    /// on this property holding.
    #[test]
    fn record_401_attribution_is_noop_without_callback() {
        let cfg = InferenceConfig {
            api_key: Some("bearer".to_string()),
            api_backend: ApiBackend::ChatCompletions,
            attribution_callback: None,
            bearer_resolver: None,
            ..minimal_config()
        };
        let client = InferenceClient::new(cfg).expect("client should build");
        // Must not panic.
        client.record_401_attribution(crate::attribution::InferenceConsumer::ChatCompletions, None);
    }

    /// `response.completed` carrying
    /// `usage.context_details.{input_tokens, output_tokens}` rewrites
    /// `usage.total_tokens` in place to the live context length
    /// (`ctx.input + ctx.output`). Billing fields stay on the wire's
    /// cumulative values.
    #[test]
    fn deserialize_response_event_overrides_total_tokens_from_context_details() {
        let sse = r#"{
            "type": "response.completed",
            "sequence_number": 0,
            "response": {
                "id": "resp_1",
                "object": "response",
                "created_at": 0,
                "model": "grok-build",
                "status": "completed",
                "output": [],
                "usage": {
                    "input_tokens": 6003,
                    "input_tokens_details": { "cached_tokens": 1984 },
                    "output_tokens": 711,
                    "output_tokens_details": { "reasoning_tokens": 388 },
                    "total_tokens": 6714,
                    "context_details": {
                        "input_tokens": 5022,
                        "output_tokens": 571
                    }
                }
            }
        }"#;
        let event = deserialize_response_event(sse).expect("parse");
        let rs::ResponseStreamEvent::ResponseCompleted(e) = event else {
            panic!("expected ResponseCompleted");
        };
        let usage = e.response.usage.expect("usage present");
        // Billing fields stay cumulative — unchanged by context_details.
        assert_eq!(usage.input_tokens, 6003);
        assert_eq!(usage.output_tokens, 711);
        assert_eq!(usage.input_tokens_details.cached_tokens, 1984);
        assert_eq!(usage.output_tokens_details.reasoning_tokens, 388);
        // total_tokens rewritten to ctx.input + ctx.output (5022 + 571).
        // NOT the wire's cumulative total (6714).
        assert_eq!(usage.total_tokens, 5_593);
    }

    #[test]
    fn deserialize_response_event_stashes_cost_in_metadata() {
        let make = |ticks: i64| {
            format!(
                r#"{{
                "type": "response.completed",
                "sequence_number": 0,
                "response": {{
                    "id": "resp_1", "object": "response", "created_at": 0,
                    "model": "grok-build", "status": "completed", "output": [],
                    "usage": {{
                        "input_tokens": 10,
                        "input_tokens_details": {{ "cached_tokens": 0 }},
                        "output_tokens": 5,
                        "output_tokens_details": {{ "reasoning_tokens": 0 }},
                        "total_tokens": 15,
                        "cost_in_usd_ticks": {ticks}
                    }}
                }}
            }}"#
            )
        };

        let event = deserialize_response_event(&make(78)).expect("parse");
        let rs::ResponseStreamEvent::ResponseCompleted(e) = event else {
            panic!("expected ResponseCompleted");
        };
        assert_eq!(
            e.response
                .metadata
                .as_ref()
                .and_then(|m| m.get(COST_USD_TICKS_METADATA_KEY))
                .map(String::as_str),
            Some("78")
        );

        // The REST mapper backfills 0 for unbilled requests: no stash.
        let event = deserialize_response_event(&make(0)).expect("parse");
        let rs::ResponseStreamEvent::ResponseCompleted(e) = event else {
            panic!("expected ResponseCompleted");
        };
        assert!(e.response.metadata.is_none());
    }

    #[test]
    fn deserialize_response_event_total_tokens_unchanged_when_context_details_absent() {
        // Older / non-Responses backends omit `context_details`.
        // `total_tokens` passes through from the wire unchanged.
        let sse = r#"{
            "type": "response.completed",
            "sequence_number": 0,
            "response": {
                "id": "resp_1",
                "object": "response",
                "created_at": 0,
                "model": "grok-build",
                "status": "completed",
                "output": [],
                "usage": {
                    "input_tokens": 10000,
                    "input_tokens_details": { "cached_tokens": 0 },
                    "output_tokens": 100,
                    "output_tokens_details": { "reasoning_tokens": 0 },
                    "total_tokens": 10100
                }
            }
        }"#;
        let event = deserialize_response_event(sse).expect("parse");
        let rs::ResponseStreamEvent::ResponseCompleted(e) = event else {
            panic!("expected ResponseCompleted");
        };
        let usage = e.response.usage.expect("usage present");
        assert_eq!(usage.total_tokens, 10_100);
    }

    #[test]
    fn deserialize_response_event_total_tokens_unchanged_when_context_details_partial() {
        // Defensive: if the backend ever ships only one of the two
        // context_details fields, we don't have a complete picture of
        // the live context size, so leave `total_tokens` on the wire's
        // cumulative value instead of guessing (treating the missing
        // half as 0 would silently under-report).
        let sse = r#"{
            "type": "response.completed",
            "sequence_number": 0,
            "response": {
                "id": "resp_1",
                "object": "response",
                "created_at": 0,
                "model": "grok-build",
                "status": "completed",
                "output": [],
                "usage": {
                    "input_tokens": 6003,
                    "input_tokens_details": { "cached_tokens": 1984 },
                    "output_tokens": 711,
                    "output_tokens_details": { "reasoning_tokens": 388 },
                    "total_tokens": 6714,
                    "context_details": {
                        "input_tokens": 5022
                    }
                }
            }
        }"#;
        let event = deserialize_response_event(sse).expect("parse");
        let rs::ResponseStreamEvent::ResponseCompleted(e) = event else {
            panic!("expected ResponseCompleted");
        };
        let usage = e.response.usage.expect("usage present");
        assert_eq!(usage.total_tokens, 6_714);
    }

    #[test]
    fn deserialize_response_event_ignores_context_details_on_non_terminal_events() {
        // Non-terminal events don't carry final usage; even if the backend ever
        // echoed `context_details` on one, we don't touch it.
        let sse = r#"{
            "type": "response.output_text.delta",
            "sequence_number": 0,
            "item_id": "item-1",
            "output_index": 0,
            "content_index": 0,
            "delta": "hello",
            "logprobs": []
        }"#;
        let event = deserialize_response_event(sse).expect("non-terminal event parses");
        assert!(matches!(
            event,
            rs::ResponseStreamEvent::ResponseOutputTextDelta(_)
        ));
    }

    /// OpenAI OAuth / Codex transport heartbeats must be recognized before the
    /// strict `ResponseStreamEvent` deserializer (which does not know them).
    #[test]
    fn is_responses_keepalive_matches_event_name_and_json_type() {
        // async-openai wire: `event: keepalive` (payload may be empty or minimal).
        assert!(is_responses_keepalive("keepalive", ""));
        assert!(is_responses_keepalive("keepalive", "{}"));
        assert!(is_responses_keepalive(
            "keepalive",
            r#"{"type":"keepalive"}"#
        ));

        // Unnamed / default `message` frames with JSON `type: "keepalive"`.
        assert!(is_responses_keepalive("message", r#"{"type":"keepalive"}"#));
        assert!(is_responses_keepalive(
            "message",
            r#"{"type":"keepalive","sequence_number":42}"#
        ));

        // Ordinary API events must not match — including deltas whose text
        // merely quotes the word "keepalive".
        assert!(!is_responses_keepalive(
            "response.output_text.delta",
            r#"{"type":"response.output_text.delta","delta":"keepalive"}"#
        ));
        assert!(!is_responses_keepalive(
            "message",
            r#"{"type":"response.created","sequence_number":0,"response":{"id":"r"}}"#
        ));
        assert!(!is_responses_keepalive("message", "not json at all"));
        assert!(!is_responses_keepalive(
            "response.completed",
            r#"{"type":"response.completed"}"#
        ));
        // Codex metadata control events are a separate filter.
        assert!(!is_responses_keepalive(
            "response.metadata",
            r#"{"type":"response.metadata"}"#
        ));
    }

    /// Codex-only `response.metadata` must be recognized before strict
    /// `ResponseStreamEvent` deserialization (async-openai has no variant).
    #[test]
    fn is_responses_metadata_matches_event_name_and_json_type() {
        // Named SSE event form (payload may be empty or minimal).
        assert!(is_responses_metadata("response.metadata", ""));
        assert!(is_responses_metadata("response.metadata", "{}"));
        assert!(is_responses_metadata(
            "response.metadata",
            r#"{"type":"response.metadata"}"#
        ));

        // Data-only / default `message` frames with exact top-level type.
        assert!(is_responses_metadata(
            "message",
            r#"{"type":"response.metadata"}"#
        ));
        // Realistic Codex payload: optional headers + verification metadata.
        assert!(is_responses_metadata(
            "message",
            r#"{
                "type":"response.metadata",
                "headers":{"x-codex-turn-state":"turn_state_fixture"},
                "metadata":{"verification":{"status":"ok"},"moderation":{"flagged":false}}
            }"#
        ));

        // Ordinary API events and nested mentions must not match.
        assert!(!is_responses_metadata(
            "response.output_text.delta",
            r#"{"type":"response.output_text.delta","delta":"response.metadata"}"#
        ));
        assert!(!is_responses_metadata(
            "message",
            r#"{"type":"response.created","sequence_number":0,"response":{"id":"r"}}"#
        ));
        assert!(!is_responses_metadata("message", "not json at all"));
        assert!(!is_responses_metadata(
            "response.completed",
            r#"{"type":"response.completed"}"#
        ));
        // Keepalives stay on the keepalive filter only.
        assert!(!is_responses_metadata(
            "keepalive",
            r#"{"type":"keepalive"}"#
        ));
        // Substring in nested field with a different top-level type.
        assert!(!is_responses_metadata(
            "message",
            r#"{"type":"response.output_item.added","item":{"type":"response.metadata"}}"#
        ));
    }

    /// Keepalives are not representable as `ResponseStreamEvent`; the SSE scan
    /// path must filter them first. Calling the deserializer directly reproduces
    /// the historical user-facing failure mode.
    #[test]
    fn deserialize_response_event_rejects_keepalive_json_type() {
        let err = deserialize_response_event(r#"{"type":"keepalive"}"#)
            .expect_err("keepalive is not a ResponseStreamEvent");
        let msg = err.to_string();
        assert!(
            msg.contains("serialization error:") && msg.contains("keepalive"),
            "expected serialization error mentioning keepalive, got: {msg}"
        );
    }

    /// `response.metadata` is not in async-openai's union; the scan path must
    /// absorb it. Direct deserialize reproduces the pre-fix failure mode.
    #[test]
    fn deserialize_response_event_rejects_metadata_json_type() {
        let err = deserialize_response_event(
            r#"{"type":"response.metadata","headers":{"x-codex-turn-state":"s"}}"#,
        )
        .expect_err("response.metadata is not a ResponseStreamEvent");
        let msg = err.to_string();
        assert!(
            msg.contains("serialization error:"),
            "expected serialization error, got: {msg}"
        );
        assert!(
            msg.contains("response.metadata") || msg.contains("unknown variant"),
            "expected unknown-variant detail for response.metadata, got: {msg}"
        );
    }

    /// Unknown semantic `response.*` events must remain fail-closed so API
    /// drift is observable (do not silently swallow every unknown type).
    #[test]
    fn deserialize_response_event_rejects_unknown_semantic_response_event() {
        let err = deserialize_response_event(
            r#"{"type":"response.brand_new_semantic_event","sequence_number":1}"#,
        )
        .expect_err("unknown semantic response.* must fail");
        let msg = err.to_string();
        assert!(
            msg.contains("serialization error:"),
            "expected serialization error, got: {msg}"
        );
        assert!(
            msg.contains("response.brand_new_semantic_event") || msg.contains("unknown variant"),
            "expected unknown-variant detail, got: {msg}"
        );
    }

    /// `GrokRequestHeaders::apply` injects the full `x-grok-*` header set only
    /// for the first-party xAI provider (`first_party = true`) and injects
    /// zero `x-grok-*` headers for any third-party identity.
    #[test]
    fn grok_request_headers_gated_by_first_party() {
        // Build a throwaway client just to get a `RequestBuilder` to attach
        // headers to. We only inspect the built request's headers.
        let client = InferenceClient::new(minimal_config()).expect("client should build");
        let builder = || client.post("https://example.test/chat/completions").builder;

        let headers = GrokRequestHeaders {
            conv_id: "conv-1",
            req_id: "req-1",
            model_id: "grok-4",
            session_id: "sess-1",
            turn_idx: Some("3"),
            agent_id: "agent-1",
            deployment_id: Some("dep-1"),
            user_id: Some("user-1"),
            first_party: true,
        };
        let req = headers.apply(builder()).build().expect("build request");
        let h = req.headers();
        assert_eq!(h.get("x-grok-conv-id").unwrap(), "conv-1");
        assert_eq!(h.get("x-grok-req-id").unwrap(), "req-1");
        assert_eq!(h.get("x-grok-model-override").unwrap(), "grok-4");
        assert_eq!(h.get("x-grok-session-id").unwrap(), "sess-1");
        assert_eq!(h.get("x-grok-turn-idx").unwrap(), "3");
        assert_eq!(h.get("x-grok-agent-id").unwrap(), "agent-1");
        assert_eq!(h.get("x-grok-deployment-id").unwrap(), "dep-1");
        assert_eq!(h.get("x-grok-user-id").unwrap(), "user-1");
    }

    /// `GrokRequestHeaders::apply` is a complete no-op for the per-request
    /// `x-grok-*` identity headers when `first_party` is false (third-party
    /// providers). The stable client-identifier header (set from
    /// `InferenceConfig::client_identifier`) is unaffected and still present.
    #[test]
    fn grok_request_headers_skipped_for_third_party() {
        let client = InferenceClient::new(minimal_config()).expect("client should build");
        let builder = || client.post("https://example.test/chat/completions").builder;

        // The per-request identity headers that `GrokRequestHeaders` injects.
        const IDENTITY_HEADERS: &[&str] = &[
            "x-grok-conv-id",
            "x-grok-req-id",
            "x-grok-model-override",
            "x-grok-session-id",
            "x-grok-turn-idx",
            "x-grok-agent-id",
            "x-grok-deployment-id",
            "x-grok-user-id",
        ];

        // Baseline (no apply) carries none of the identity headers.
        let baseline = builder().build().expect("build baseline");
        for name in IDENTITY_HEADERS {
            assert!(
                baseline.headers().get(*name).is_none(),
                "baseline must not carry {name}"
            );
        }

        let headers = GrokRequestHeaders {
            conv_id: "conv-1",
            req_id: "req-1",
            model_id: "grok-4",
            session_id: "sess-1",
            turn_idx: Some("3"),
            agent_id: "agent-1",
            deployment_id: Some("dep-1"),
            user_id: Some("user-1"),
            first_party: false,
        };
        let req = headers.apply(builder()).build().expect("build request");
        for name in IDENTITY_HEADERS {
            assert!(
                req.headers().get(*name).is_none(),
                "third-party request must not carry per-request identity header {name}"
            );
        }
    }

    /// `openrouter_metadata_requested` derives from `provider_identity ==
    /// OpenRouter`, not from scanning `extra_headers` for
    /// `X-OpenRouter-Metadata`. Removing the header from a TOML OpenRouter
    /// provider config does not change diagnostics behavior.
    #[test]
    fn openrouter_metadata_requested_derives_from_identity() {
        use crate::config::ProviderIdentity;

        let mut cfg = minimal_config();
        cfg.provider_identity = ProviderIdentity::OpenRouter;
        // No X-OpenRouter-Metadata header in extra_headers at all.
        let client = InferenceClient::new(cfg).expect("client should build");
        assert!(
            client.openrouter_metadata_requested,
            "OpenRouter identity requests diagnostics metadata even without the header"
        );

        // A non-OpenRouter identity with the header present still does NOT
        // request metadata — identity is the single source of truth.
        let mut cfg = minimal_config();
        cfg.provider_identity = ProviderIdentity::Xai;
        cfg.extra_headers
            .insert("X-OpenRouter-Metadata".to_string(), "enabled".to_string());
        let client = InferenceClient::new(cfg).expect("client should build");
        assert!(
            !client.openrouter_metadata_requested,
            "non-OpenRouter identity must not request OpenRouter metadata"
        );

        // Custom (default) identity: no metadata.
        let client = InferenceClient::new(minimal_config()).expect("client should build");
        assert!(!client.openrouter_metadata_requested);
    }

    /// `first_party` on the client is set only for the `Xai` identity.
    #[test]
    fn first_party_flag_only_for_xai_identity() {
        use crate::config::ProviderIdentity;

        for (identity, expected) in [
            (ProviderIdentity::Xai, true),
            (ProviderIdentity::OpenAi, false),
            (ProviderIdentity::OpenRouter, false),
            (ProviderIdentity::Anthropic, false),
            (ProviderIdentity::Custom, false),
        ] {
            let cfg = InferenceConfig {
                provider_identity: identity,
                ..minimal_config()
            };
            let client = InferenceClient::new(cfg).expect("client should build");
            assert_eq!(
                client.first_party, expected,
                "first_party mismatch for {identity:?}"
            );
        }
    }

    /// Direct Anthropic identity must not receive any default `x-grok-*`
    /// client/deployment/user headers on construction.
    #[test]
    fn anthropic_identity_omits_all_default_x_grok_headers() {
        use crate::config::ProviderIdentity;
        use reqwest::header::HeaderName;

        let cfg = InferenceConfig {
            api_key: Some("sk-ant-test".into()),
            api_backend: ApiBackend::Messages,
            auth_scheme: AuthScheme::XApiKey,
            provider_identity: ProviderIdentity::Anthropic,
            client_version: Some("1.2.3".into()),
            client_identifier: Some("should-not-leak".into()),
            deployment_id: Some("dep-1".into()),
            user_id: Some("user-1".into()),
            ..minimal_config()
        };
        let client = InferenceClient::new(cfg).expect("client should build");
        let req = client
            .post("https://api.anthropic.com/v1/messages")
            .builder
            .build()
            .expect("build request");
        let h = req.headers();
        for name in [
            "x-grok-client-version",
            "x-grok-client-identifier",
            "x-grok-deployment-id",
            "x-grok-user-id",
            "x-grok-conv-id",
            "x-grok-session-id",
            "x-grok-agent-id",
        ] {
            assert!(
                h.get(HeaderName::from_static(name)).is_none(),
                "Anthropic request must not carry {name}"
            );
        }
        // Auth still uses x-api-key for Anthropic Messages.
        assert!(
            h.get(HeaderName::from_static("x-api-key")).is_some(),
            "Anthropic must still send x-api-key"
        );
    }

    // ── Change 2: Chat/Responses conformance ─────────────────────────────

    #[test]
    fn provider_preferences_serializes_object_sort_and_performance_fields() {
        let request = ChatCompletionRequest::new(
            "openai/gpt-oss-120b",
            vec![ChatRequestMessage::user("hello")],
        );
        let prefs = crate::config::OpenRouterProviderPreferences {
            sort: Some(crate::config::OpenRouterSort::by("latency")),
            enforce_distillable_text: Some(true),
            preferred_max_latency: Some(serde_json::json!(250)),
            preferred_min_throughput: Some(serde_json::json!({ "p50": 100 })),
            data_collection: Some("deny".into()),
            ..Default::default()
        };
        let serialized = serde_json::to_value(ChatRequestWithFallbacks {
            inner: &request,
            models: None,
            provider: Some(&prefs),
            plugins: None,
            reasoning: None,
            tool_stream: false,
            thinking: None,
        })
        .unwrap();
        let provider = &serialized["provider"];
        assert_eq!(provider["sort"]["by"], "latency");
        assert_eq!(provider["enforce_distillable_text"], true);
        assert_eq!(provider["preferred_max_latency"], 250);
        assert_eq!(provider["preferred_min_throughput"]["p50"], 100);
        assert_eq!(provider["data_collection"], "deny");
    }

    #[test]
    fn openrouter_chat_emits_normalized_reasoning_object() {
        use xai_grok_inference_types::ReasoningEffort;
        // Pinned OpenRouter ChatRequest inventory includes both `reasoning`
        // (object) and flat `reasoning_effort` (string enum). Dual emission is
        // schema-correct; assert exact wire shapes rather than presence alone.
        let cfg = InferenceConfig {
            provider_identity: crate::config::ProviderIdentity::OpenRouter,
            ..minimal_config()
        };
        let client = InferenceClient::new(cfg).unwrap();
        let mut request = ChatCompletionRequest::new(
            "openai/gpt-oss-120b",
            vec![ChatRequestMessage::user("think")],
        );
        request.reasoning_effort = Some(ReasoningEffort::High);
        let payload = client.apply_defaults(request).unwrap();
        let reasoning = client
            .openrouter_reasoning_object(payload.reasoning_effort)
            .expect("OpenRouter should normalize reasoning");
        assert_eq!(reasoning.effort.as_deref(), Some("high"));
        let serialized = serde_json::to_value(ChatRequestWithFallbacks {
            inner: &payload,
            models: None,
            provider: None,
            plugins: None,
            reasoning: Some(&reasoning),
            tool_stream: false,
            thinking: None,
        })
        .unwrap();
        assert_eq!(
            serialized["reasoning"],
            serde_json::json!({ "effort": "high" }),
            "normalized reasoning object must serialize only effort when \
             derived from flat ReasoningEffort"
        );
        assert_eq!(
            serialized["reasoning_effort"],
            serde_json::json!("high"),
            "flat reasoning_effort remains for OpenAI-compatible clients \
             (pinned ChatRequest inventory lists both fields)"
        );
        let reasoning_obj = serialized["reasoning"]
            .as_object()
            .expect("reasoning must be an object");
        assert!(
            !reasoning_obj.contains_key("max_tokens") && !reasoning_obj.contains_key("exclude"),
            "unset optional reasoning knobs must be omitted"
        );
    }

    #[test]
    fn non_openrouter_chat_omits_reasoning_object() {
        use xai_grok_inference_types::ReasoningEffort;
        let cfg = InferenceConfig {
            provider_identity: crate::config::ProviderIdentity::OpenAi,
            ..minimal_config()
        };
        let client = InferenceClient::new(cfg).unwrap();
        assert!(
            client
                .openrouter_reasoning_object(Some(ReasoningEffort::High))
                .is_none()
        );
    }

    #[test]
    fn responses_finalize_adds_provider_plugins_and_enforces_stateless() {
        let prefs = crate::config::OpenRouterProviderPreferences {
            data_collection: Some("deny".into()),
            require_parameters: Some(true),
            ..Default::default()
        };
        let plugins = vec![crate::config::OpenRouterPlugin {
            id: "response-healing".into(),
            ..Default::default()
        }];
        let cfg = InferenceConfig {
            provider_identity: crate::config::ProviderIdentity::OpenRouter,
            openrouter_provider_preferences: Some(prefs),
            openrouter_plugins: plugins,
            openrouter_fallback_models: vec!["openai/gpt-5-mini".into()],
            ..minimal_config()
        };
        let client = InferenceClient::new(cfg).unwrap();
        let mut body = serde_json::json!({
            "model": "openai/gpt-oss-120b",
            "input": "hello",
            "store": true,
            "previous_response_id": "resp_should_not_leave"
        });
        client.finalize_responses_request_body(&mut body, Vec::new());
        assert_eq!(body["store"], false);
        assert!(
            body.get("previous_response_id").is_none(),
            "OpenRouter Responses must strip previous_response_id"
        );
        assert_eq!(body["models"], serde_json::json!(["openai/gpt-5-mini"]));
        assert_eq!(body["provider"]["data_collection"], "deny");
        assert_eq!(body["plugins"][0]["id"], "response-healing");
    }

    #[test]
    fn responses_finalize_skips_openrouter_extensions_for_openai() {
        let prefs = crate::config::OpenRouterProviderPreferences {
            data_collection: Some("deny".into()),
            ..Default::default()
        };
        let cfg = InferenceConfig {
            provider_identity: crate::config::ProviderIdentity::OpenAi,
            openrouter_provider_preferences: Some(prefs),
            openrouter_plugins: vec![crate::config::OpenRouterPlugin {
                id: "web".into(),
                ..Default::default()
            }],
            openrouter_fallback_models: vec!["should-not-emit".into()],
            ..minimal_config()
        };
        let client = InferenceClient::new(cfg).unwrap();
        let mut body = serde_json::json!({
            "model": "gpt-5",
            "input": "hello",
            "store": true,
            "previous_response_id": "resp_keep_for_non_or"
        });
        client.finalize_responses_request_body(&mut body, Vec::new());
        assert!(body.get("provider").is_none());
        assert!(body.get("plugins").is_none());
        assert!(
            body.get("models").is_none(),
            "fallback models are identity-gated to OpenRouter"
        );
        // Non-OpenRouter keeps caller store / previous_response_id.
        assert_eq!(body["store"], true);
        assert_eq!(body["previous_response_id"], "resp_keep_for_non_or");
    }

    #[test]
    fn apply_response_defaults_forces_store_false_for_openrouter() {
        let cfg = InferenceConfig {
            provider_identity: crate::config::ProviderIdentity::OpenRouter,
            ..minimal_config()
        };
        let client = InferenceClient::new(cfg).unwrap();
        let mut wrapper = CreateResponseWrapper::new(rs::CreateResponse {
            model: Some("openai/gpt-oss-120b".into()),
            store: Some(true),
            previous_response_id: Some("resp_x".into()),
            ..Default::default()
        });
        client.apply_response_defaults(&mut wrapper).unwrap();
        assert_eq!(wrapper.inner.store, Some(false));
        assert!(wrapper.inner.previous_response_id.is_none());
    }

    #[test]
    fn openrouter_chat_and_responses_tool_loop_preserve_tool_and_reasoning_fields() {
        use std::sync::Arc;
        use xai_grok_inference_types::{
            AssistantItem, ConversationItem, ConversationRequest, ConversationToolChoice, ToolCall,
            ToolSpec, conversation_to_chat_messages,
        };

        let detail = serde_json::json!({
            "type": "reasoning.text",
            "text": "plan the tool call"
        });
        let assistant = ConversationItem::Assistant(AssistantItem {
            content: Arc::from("calling tool"),
            tool_calls: vec![ToolCall {
                id: Arc::from("call_1"),
                name: "run_terminal_command".into(),
                arguments: Arc::from(r#"{"command":"echo hi"}"#),
            }],
            model_id: None,
            model_fingerprint: None,
            reasoning_effort: None,
            reasoning_details: vec![detail.clone()],
            provider_payload: None,
        });
        let items = vec![
            ConversationItem::user("use the tool"),
            assistant,
            ConversationItem::tool_result("call_1", "hi\n"),
        ];

        // Chat path: reasoning_details + tool_calls round-trip on wire messages.
        let chat_msgs = conversation_to_chat_messages(items.clone());
        let asst = chat_msgs
            .iter()
            .find(|m| matches!(m.role, xai_grok_inference_types::Role::Assistant))
            .expect("assistant message");
        assert_eq!(asst.tool_calls.len(), 1);
        assert_eq!(asst.reasoning_details, vec![detail.clone()]);

        // Responses path: tools + reasoning effort convert without previous_response_id.
        let mut req = ConversationRequest {
            model: Some("openai/gpt-oss-120b".into()),
            items,
            tools: vec![ToolSpec {
                name: "run_terminal_command".into(),
                description: Some("run a shell command".into()),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": { "command": { "type": "string" } }
                }),
                strict: None,
            }],
            tool_choice: Some(ConversationToolChoice::Auto),
            reasoning_effort: Some(xai_grok_inference_types::ReasoningEffort::Medium),
            ..ConversationRequest::default()
        };
        let responses: rs::CreateResponse = (&req).into();
        assert!(responses.previous_response_id.is_none());
        assert!(responses.tools.as_ref().is_some_and(|t| !t.is_empty()));
        assert!(
            responses
                .reasoning
                .as_ref()
                .and_then(|r| r.effort.as_ref())
                .is_some()
        );

        // OpenRouter finalize keeps tools and enforces statelessness.
        let cfg = InferenceConfig {
            provider_identity: crate::config::ProviderIdentity::OpenRouter,
            openrouter_provider_preferences: Some(crate::config::OpenRouterProviderPreferences {
                require_parameters: Some(true),
                ..Default::default()
            }),
            ..minimal_config()
        };
        let client = InferenceClient::new(cfg).unwrap();
        let mut wrapper = CreateResponseWrapper::new(responses);
        client.apply_response_defaults(&mut wrapper).unwrap();
        let mut body = serde_json::to_value(&wrapper.inner).unwrap();
        client.finalize_responses_request_body(&mut body, Vec::new());
        assert_eq!(body["store"], false);
        assert!(body.get("previous_response_id").is_none());
        assert_eq!(body["provider"]["require_parameters"], true);
        assert!(body.get("tools").is_some());
        // Chat completion conversion still carries tools.
        req.reasoning_effort = Some(xai_grok_inference_types::ReasoningEffort::Medium);
        let chat: ChatCompletionRequest = req.into();
        assert!(chat.tools.as_ref().is_some_and(|t| !t.is_empty()));
        assert_eq!(
            chat.reasoning_effort,
            Some(xai_grok_inference_types::ReasoningEffort::Medium)
        );
    }
}
