//! Layer-2 stream transform for the OpenAI Responses API.
//!
//! Consumes a raw `rs::ResponseStreamEvent` stream and produces
//! [`InferenceEvent`]s. Pure: no I/O, no shell coupling.

use std::collections::BTreeMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use futures_util::stream::{BoxStream, Stream};

use xai_grok_inference_types::{
    ConversationItem, ConversationResponse, InferenceError, ResponseModelMetadata, StopReason,
    TokenUsage, ToolCall, rs,
};

use crate::events::{InferenceChannel, InferenceErrorInfo, InferenceEvent};
use crate::metrics::InferenceLatencyStats;
use crate::types::RequestId;

/// Returns whether a Responses API event reflects real model progress
/// rather than a liveness-only heartbeat / status transition.
pub(crate) fn responses_event_has_meaningful_content(event: &rs::ResponseStreamEvent) -> bool {
    use rs::ResponseStreamEvent;

    match event {
        ResponseStreamEvent::ResponseCreated(_)
        | ResponseStreamEvent::ResponseInProgress(_)
        | ResponseStreamEvent::ResponseQueued(_) => false,
        ResponseStreamEvent::ResponseOutputTextDelta(event) => !event.delta.is_empty(),
        ResponseStreamEvent::ResponseOutputTextDone(event) => !event.text.is_empty(),
        ResponseStreamEvent::ResponseRefusalDelta(event) => !event.delta.is_empty(),
        ResponseStreamEvent::ResponseRefusalDone(event) => !event.refusal.is_empty(),
        ResponseStreamEvent::ResponseFunctionCallArgumentsDelta(event) => !event.delta.is_empty(),
        ResponseStreamEvent::ResponseFunctionCallArgumentsDone(event) => {
            !event.arguments.is_empty() || event.name.as_ref().is_some_and(|name| !name.is_empty())
        }
        ResponseStreamEvent::ResponseReasoningSummaryTextDelta(event) => !event.delta.is_empty(),
        ResponseStreamEvent::ResponseReasoningSummaryTextDone(event) => !event.text.is_empty(),
        ResponseStreamEvent::ResponseReasoningTextDelta(event) => !event.delta.is_empty(),
        ResponseStreamEvent::ResponseReasoningTextDone(event) => !event.text.is_empty(),
        ResponseStreamEvent::ResponseMCPCallArgumentsDelta(event) => !event.delta.is_empty(),
        ResponseStreamEvent::ResponseMCPCallArgumentsDone(event) => !event.arguments.is_empty(),
        ResponseStreamEvent::ResponseCodeInterpreterCallCodeDelta(event) => !event.delta.is_empty(),
        ResponseStreamEvent::ResponseCodeInterpreterCallCodeDone(event) => !event.code.is_empty(),
        ResponseStreamEvent::ResponseCustomToolCallInputDelta(event) => !event.delta.is_empty(),
        ResponseStreamEvent::ResponseCustomToolCallInputDone(event) => !event.input.is_empty(),
        ResponseStreamEvent::ResponseFailed(event) => {
            !event.response.output.is_empty()
                || event
                    .response
                    .usage
                    .as_ref()
                    .is_some_and(|usage| usage.output_tokens > 0)
        }
        ResponseStreamEvent::ResponseCompleted(_)
        | ResponseStreamEvent::ResponseIncomplete(_)
        | ResponseStreamEvent::ResponseOutputItemAdded(_)
        | ResponseStreamEvent::ResponseOutputItemDone(_)
        | ResponseStreamEvent::ResponseContentPartAdded(_)
        | ResponseStreamEvent::ResponseContentPartDone(_)
        | ResponseStreamEvent::ResponseFileSearchCallInProgress(_)
        | ResponseStreamEvent::ResponseFileSearchCallSearching(_)
        | ResponseStreamEvent::ResponseFileSearchCallCompleted(_)
        | ResponseStreamEvent::ResponseWebSearchCallInProgress(_)
        | ResponseStreamEvent::ResponseWebSearchCallSearching(_)
        | ResponseStreamEvent::ResponseWebSearchCallCompleted(_)
        | ResponseStreamEvent::ResponseReasoningSummaryPartAdded(_)
        | ResponseStreamEvent::ResponseReasoningSummaryPartDone(_)
        | ResponseStreamEvent::ResponseImageGenerationCallCompleted(_)
        | ResponseStreamEvent::ResponseImageGenerationCallGenerating(_)
        | ResponseStreamEvent::ResponseImageGenerationCallInProgress(_)
        | ResponseStreamEvent::ResponseImageGenerationCallPartialImage(_)
        | ResponseStreamEvent::ResponseMCPCallCompleted(_)
        | ResponseStreamEvent::ResponseMCPCallFailed(_)
        | ResponseStreamEvent::ResponseMCPCallInProgress(_)
        | ResponseStreamEvent::ResponseMCPListToolsCompleted(_)
        | ResponseStreamEvent::ResponseMCPListToolsFailed(_)
        | ResponseStreamEvent::ResponseMCPListToolsInProgress(_)
        | ResponseStreamEvent::ResponseCodeInterpreterCallInProgress(_)
        | ResponseStreamEvent::ResponseCodeInterpreterCallInterpreting(_)
        | ResponseStreamEvent::ResponseCodeInterpreterCallCompleted(_)
        | ResponseStreamEvent::ResponseOutputTextAnnotationAdded(_)
        | ResponseStreamEvent::ResponseError(_) => true,
    }
}

pub(crate) fn responses_event_may_have_output(event: &rs::ResponseStreamEvent) -> bool {
    !matches!(event, rs::ResponseStreamEvent::ResponseError(_))
        && responses_event_has_meaningful_content(event)
}

/// Transform a raw Responses API event stream into a stream of
/// [`InferenceEvent`]s.
///
/// Yields exactly one terminal event ([`InferenceEvent::Completed`] or
/// [`InferenceEvent::Failed`]) per request. Server-side `ResponseFailed`
/// and `ResponseError` events are translated to
/// `InferenceError::Api { status: 500, .. }` so the actor's retry loop
/// treats them as retryable.
///
/// `doom_loop` is the collector returned alongside `raw_stream` by
/// `InferenceClient::conversation_stream_responses`; any signals the SSE
/// decoder recorded are drained onto the final `ConversationResponse`.
/// `None` (check disabled) leaves the response untouched.
pub fn stream_responses<'a>(
    raw_stream: BoxStream<'a, Result<rs::ResponseStreamEvent, InferenceError>>,
    model_metadata: Option<ResponseModelMetadata>,
    request_id: RequestId,
    idle_timeout: Duration,
    doom_loop: Option<crate::doom_loop::DoomLoopSignalCollector>,
) -> impl Stream<Item = InferenceEvent> + Send + 'a {
    stream_responses_tracked(
        raw_stream,
        model_metadata,
        request_id,
        idle_timeout,
        doom_loop,
        Arc::new(AtomicBool::new(false)),
    )
}

pub(crate) fn stream_responses_tracked<'a>(
    raw_stream: BoxStream<'a, Result<rs::ResponseStreamEvent, InferenceError>>,
    model_metadata: Option<ResponseModelMetadata>,
    request_id: RequestId,
    idle_timeout: Duration,
    doom_loop: Option<crate::doom_loop::DoomLoopSignalCollector>,
    output_observed: Arc<AtomicBool>,
) -> impl Stream<Item = InferenceEvent> + Send + 'a {
    async_stream::stream! {
        use rs::{ResponseStreamEvent, Status};

        let stream_start = Instant::now();
        let mut chunk_timestamps: Vec<Instant> = Vec::new();

        yield InferenceEvent::StreamStarted {
            request_id: request_id.clone(),
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
        };

        if let Some(metadata) = model_metadata {
            yield InferenceEvent::ModelMetadata {
                request_id: request_id.clone(),
                metadata,
            };
        }

        let mut final_response: Option<rs::Response> = None;
        let mut chunk_index: u64 = 0;
        let mut message_chunk_count: u64 = 0;
        let mut first_token_emitted = false;
        let mut reasoning_acc = String::new();
        // Accumulate visible assistant text from streaming deltas. Some
        // providers (notably ChatGPT Codex OAuth) may emit
        // `response.output_text.delta` events that the UI already painted
        // while the terminal `response.completed` body lacks a Message
        // item. Without this buffer, `empty_reason` treats the turn as
        // empty and the actor resamples — leaving orphan paragraphs on
        // screen (one per retry).
        let mut text_acc = String::new();
        let mut last_content_chunk_at = Instant::now();

        // Maps Responses API `output_index` to our tool-only `tool_index`.
        // Populated when `ResponseOutputItemAdded` carries a `FunctionCall`;
        // later `ResponseFunctionCallArgumentsDelta` events
        // look up `output_index` here to find the matching `tool_index`.
        let mut output_to_tool_index: BTreeMap<u32, u32> = BTreeMap::new();
        let mut next_tool_index: u32 = 0;
        // ChatGPT Codex OAuth streams function calls in item events but often
        // leaves `response.completed.output` empty. Accumulate them so the
        // final ConversationResponse still carries tool calls (otherwise the
        // actor classifies the turn as empty_response and retries forever).
        let mut streamed_tool_calls: BTreeMap<u32, StreamedToolCall> = BTreeMap::new();

        let mut stream = raw_stream;
        loop {
            let event_result = match tokio::time::timeout(idle_timeout, stream.next()).await {
                Ok(Some(event_result)) => event_result,
                Ok(None) => break,
                Err(_elapsed) => {
                    let err = InferenceError::IdleTimeout {
                        elapsed_secs: idle_timeout.as_secs(),
                    };
                    yield InferenceEvent::Failed {
                        request_id: request_id.clone(),
                        error: InferenceErrorInfo::from(&err),
                    };
                    return;
                }
            };

            let event = match event_result {
                Ok(event) => event,
                Err(err) => {
                    yield InferenceEvent::Failed {
                        request_id: request_id.clone(),
                        error: InferenceErrorInfo::from(&err),
                    };
                    return;
                }
            };

            if responses_event_may_have_output(&event) {
                output_observed.store(true, Ordering::Relaxed);
            }

            // A confident server-detected loop aborts the attempt (dropping
            // the SSE connection) so the retry loop can resample instead of
            // streaming the burning tail. Checked before the event is
            // processed so a terminal frame carrying the signal never
            // becomes the accepted response while the abort is armed.
            if let Some(triggers) = doom_loop.as_ref().and_then(|c| c.abort_triggers()) {
                let err = InferenceError::DoomLoopDetected {
                    triggers,
                    aborted_at_chunk: Some(chunk_index),
                };
                yield InferenceEvent::Failed {
                    request_id: request_id.clone(),
                    error: InferenceErrorInfo::from(&err),
                };
                return;
            }

            let event_has_content = responses_event_has_meaningful_content(&event);

            // Track whether ResponseIncomplete should break the loop
            // after the content-aware idle check below.
            let mut should_break = false;

            match event {
                ResponseStreamEvent::ResponseOutputTextDelta(text_delta_event) => {
                    let delta = text_delta_event.delta;
                    if !delta.is_empty() {
                        text_acc.push_str(&delta);
                        if !first_token_emitted {
                            first_token_emitted = true;
                            yield InferenceEvent::FirstToken {
                                request_id: request_id.clone(),
                            };
                        }
                        chunk_timestamps.push(Instant::now());
                        chunk_index += 1;
                        message_chunk_count += 1;
                        yield InferenceEvent::ChannelToken {
                            request_id: request_id.clone(),
                            channel: InferenceChannel::Text,
                            text: delta,
                            chunk_index,
                        };
                    }
                }

                // Codex / some Responses dialects may finish a part with only
                // the Done event (full text, no prior deltas). Capture it so
                // the final assistant item is never empty after a painted turn.
                ResponseStreamEvent::ResponseOutputTextDone(text_done_event) => {
                    let text = text_done_event.text;
                    if !text.is_empty() && text_acc.is_empty() {
                        text_acc = text;
                    }
                }

                ResponseStreamEvent::ResponseReasoningSummaryTextDelta(summary_event) => {
                    let delta = summary_event.delta;
                    if !delta.is_empty() {
                        if !first_token_emitted {
                            first_token_emitted = true;
                            yield InferenceEvent::FirstToken {
                                request_id: request_id.clone(),
                            };
                        }
                        chunk_index += 1;
                        yield InferenceEvent::ChannelToken {
                            request_id: request_id.clone(),
                            channel: InferenceChannel::Reasoning,
                            text: delta,
                            chunk_index,
                        };
                    }
                }

                ResponseStreamEvent::ResponseReasoningTextDelta(reasoning_event) => {
                    let delta = reasoning_event.delta;
                    if !delta.is_empty() {
                        if !first_token_emitted {
                            first_token_emitted = true;
                            yield InferenceEvent::FirstToken {
                                request_id: request_id.clone(),
                            };
                        }
                        chunk_index += 1;
                        reasoning_acc.push_str(&delta);
                        yield InferenceEvent::ChannelToken {
                            request_id: request_id.clone(),
                            channel: InferenceChannel::Reasoning,
                            text: delta,
                            chunk_index,
                        };
                    }
                }

                // Start of a Responses FunctionCall — emit initial id+name
                // and remember the output_index → tool_index mapping.
                ResponseStreamEvent::ResponseOutputItemAdded(added_event) => {
                    if let rs::OutputItem::FunctionCall(fc) = added_event.item {
                        let tool_index = next_tool_index;
                        next_tool_index += 1;
                        output_to_tool_index.insert(added_event.output_index, tool_index);
                        streamed_tool_calls.insert(
                            added_event.output_index,
                            StreamedToolCall {
                                call_id: fc.call_id.clone(),
                                name: fc.name.clone(),
                                arguments: fc.arguments.clone(),
                            },
                        );

                        yield InferenceEvent::ToolCallDelta {
                            request_id: request_id.clone(),
                            tool_index,
                            id: Some(fc.call_id),
                            name: Some(fc.name),
                            arguments_delta: None,
                        };
                    }
                }

                // Continuation chunk for a streaming FunctionCall's args.
                // Drop silently if no preceding OutputItemAdded mapped.
                ResponseStreamEvent::ResponseFunctionCallArgumentsDelta(args_event) => {
                    let delta = args_event.delta;
                    if !delta.is_empty()
                        && let Some(&tool_index) =
                            output_to_tool_index.get(&args_event.output_index)
                    {
                        if let Some(tc) = streamed_tool_calls.get_mut(&args_event.output_index) {
                            tc.arguments.push_str(&delta);
                        }
                        yield InferenceEvent::ToolCallDelta {
                            request_id: request_id.clone(),
                            tool_index,
                            id: None,
                            name: None,
                            arguments_delta: Some(delta),
                        };
                    }
                }

                // Full arguments for a FunctionCall — prefer this over delta
                // concat when present (Codex emits both).
                ResponseStreamEvent::ResponseFunctionCallArgumentsDone(done_event) => {
                    if let Some(tc) = streamed_tool_calls.get_mut(&done_event.output_index) {
                        if !done_event.arguments.is_empty() {
                            tc.arguments = done_event.arguments.clone();
                        }
                        if let Some(name) = done_event.name.clone()
                            && !name.is_empty()
                        {
                            tc.name = name;
                        }
                    }
                }

                ResponseStreamEvent::ResponseCompleted(completed_event) => {
                    final_response = Some(completed_event.response);
                }

                ResponseStreamEvent::ResponseIncomplete(incomplete_event) => {
                    final_response = Some(incomplete_event.response);
                    should_break = true;
                }

                ResponseStreamEvent::ResponseFailed(failed_event) => {
                    let response = failed_event.response;
                    let error_message = response
                        .error
                        .as_ref()
                        .map(|e| format!("{}: {}", e.code, e.message))
                        .unwrap_or_else(|| "Response failed with unknown error".to_string());
                    let err = InferenceError::Api {
                        status: reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                        message: error_message,
                        model_metadata: None,
                        retry_after_secs: None,
                        should_retry: None,
                        diagnostics: None,
                    };
                    yield InferenceEvent::Failed {
                        request_id: request_id.clone(),
                        error: InferenceErrorInfo::from(&err),
                    };
                    return;
                }

                ResponseStreamEvent::ResponseError(error_event) => {
                    let code = error_event.code.unwrap_or_else(|| "error".to_string());
                    let error_message = format!("{}: {}", code, error_event.message);
                    let err = InferenceError::Api {
                        status: reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                        message: error_message,
                        model_metadata: None,
                        retry_after_secs: None,
                        should_retry: None,
                        diagnostics: None,
                    };
                    yield InferenceEvent::Failed {
                        request_id: request_id.clone(),
                        error: InferenceErrorInfo::from(&err),
                    };
                    return;
                }

                // ── Backend-hosted tool lifecycle events ────────────
                // These tools are executed server-side by the agentic
                // sampler. We emit progress events so the shell/pager
                // can show status to the user.

                // Web search
                ResponseStreamEvent::ResponseWebSearchCallInProgress(ev) => {
                    yield InferenceEvent::BackendToolCallStarted {
                        request_id: request_id.clone(),
                        call_id: ev.item_id.clone(),
                        name: "web_search".to_string(),
                    };
                }
                // Completed/Searching carry no data — the real payload
                // arrives via ResponseOutputItemDone(WebSearchCall) below.
                ResponseStreamEvent::ResponseWebSearchCallCompleted(_)
                | ResponseStreamEvent::ResponseWebSearchCallSearching(_) => {}

                // OutputItemDone carries the full result for backend tools.
                // For WebSearchCall this includes the query and source URLs.
                // For CustomToolCall this includes x_search results.
                // For FunctionCall, Codex may only put the complete call here
                // (completed.output stays empty) — capture it for the fallback.
                ResponseStreamEvent::ResponseOutputItemDone(done_event) => {
                    match &done_event.item {
                        rs::OutputItem::FunctionCall(fc) => {
                            streamed_tool_calls.insert(
                                done_event.output_index,
                                StreamedToolCall {
                                    call_id: fc.call_id.clone(),
                                    name: fc.name.clone(),
                                    arguments: fc.arguments.clone(),
                                },
                            );
                            // Ensure tool_index mapping exists if Added was missed.
                            output_to_tool_index
                                .entry(done_event.output_index)
                                .or_insert_with(|| {
                                    let tool_index = next_tool_index;
                                    next_tool_index += 1;
                                    tool_index
                                });
                        }
                        rs::OutputItem::WebSearchCall(ws) => {
                            let result = serde_json::to_value(ws).ok();
                            yield InferenceEvent::BackendToolCallCompleted {
                                request_id: request_id.clone(),
                                call_id: ws.id.clone(),
                                name: "web_search".to_string(),
                                result,
                            };
                        }
                        // X search results arrive as CustomToolCall with
                        // names like x_keyword_search, x_semantic_search, etc.
                        // Use "x_search" consistently (matching the Started event);
                        // the specific sub-type is in the serialized result payload
                        // and extracted by the pager from raw_output.name.
                        rs::OutputItem::CustomToolCall(ct) => {
                            let result = serde_json::to_value(ct).ok();
                            yield InferenceEvent::BackendToolCallCompleted {
                                request_id: request_id.clone(),
                                call_id: ct.id.clone(),
                                name: "x_search".to_string(),
                                result,
                            };
                        }
                        _ => {}
                    }
                }

                // CustomToolCallInputDelta is x_search in-progress streaming.
                // Emit a started event on first delta per item_id.
                ResponseStreamEvent::ResponseCustomToolCallInputDone(ev) => {
                    yield InferenceEvent::BackendToolCallStarted {
                        request_id: request_id.clone(),
                        call_id: ev.item_id.clone(),
                        name: "x_search".to_string(),
                    };
                }

                // All other events (intermediate progress, annotations,
                // image gen, file search, etc.) — no action needed.
                _ => {}
            }

            if event_has_content {
                last_content_chunk_at = Instant::now();
            } else if last_content_chunk_at.elapsed() > idle_timeout {
                let err = InferenceError::IdleTimeout {
                    elapsed_secs: idle_timeout.as_secs(),
                };
                yield InferenceEvent::Failed {
                    request_id: request_id.clone(),
                    error: InferenceErrorInfo::from(&err),
                };
                return;
            }

            if should_break {
                break;
            }
        }

        // ── Build the final response ─────────────────────────────────
        let mut response = match final_response {
            Some(r) => r,
            None => {
                let err = InferenceError::Api {
                    status: reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                    message: "No ResponseCompleted or ResponseIncomplete event received from \
                              Responses API"
                        .to_string(),
                    model_metadata: None,
                    retry_after_secs: None,
                    should_retry: None,
                    diagnostics: None,
                };
                yield InferenceEvent::Failed {
                    request_id: request_id.clone(),
                    error: InferenceErrorInfo::from(&err),
                };
                return;
            }
        };

        // Billing fields (`prompt_tokens`, `completion_tokens`,
        // `cached_prompt_tokens`, `reasoning_tokens`) are the cumulative
        // wire values — they sum across every server-side turn of the
        // agent loop and are what we bill on / log to telemetry.
        //
        // `total_tokens` is the live context length used to drive the
        // CLI `/context` bar, the auto-compact threshold, and
        // `meta.totalTokens` on persisted sessions. The SSE decoder
        // (`deserialize_response_event`) has already rewritten
        // `u.total_tokens` to `context_details.input + output` when
        // the backend emits it; on older deployments the wire
        // value passes through unchanged.
        let usage = response.usage.as_ref().map(|u| TokenUsage {
            prompt_tokens: u.input_tokens,
            completion_tokens: u.output_tokens,
            total_tokens: u.total_tokens,
            reasoning_tokens: u.output_tokens_details.reasoning_tokens,
            cached_prompt_tokens: u.input_tokens_details.cached_tokens,
        });

        let cost_usd_ticks = response
            .metadata
            .as_mut()
            .and_then(|m| m.remove(crate::client::COST_USD_TICKS_METADATA_KEY))
            .and_then(|s| s.parse::<i64>().ok());

        let status = response.status.clone();

        // Convert to ConversationItem(s); patch in accumulated reasoning
        // text as a fallback when the final response lacks `content` /
        // `summary` (the streaming deltas may have arrived out of band).
        // Splice policy lives in `inject_streaming_reasoning_fallback`.
        let mut items = xai_grok_inference_types::response_to_conversation_items(response);
        xai_grok_inference_types::inject_streaming_reasoning_fallback(&mut items, reasoning_acc);
        inject_streaming_text_fallback(&mut items, &text_acc);
        inject_streaming_tool_calls_fallback(
            &mut items,
            streamed_tool_calls.into_values().collect(),
        );

        let has_tool_calls = items.iter().any(|i| match i {
            ConversationItem::Assistant(a) => !a.tool_calls.is_empty(),
            _ => false,
        });

        let stop_reason = if has_tool_calls {
            Some(StopReason::ToolCalls)
        } else {
            match status {
                Status::Completed => Some(StopReason::Stop),
                Status::Incomplete => Some(StopReason::Length),
                _ => None,
            }
        };

        let stream_end = Instant::now();
        let metrics =
            InferenceLatencyStats::from_timestamps(stream_start, &chunk_timestamps, stream_end);

        // Warn-only for now: surface the server-reported triggers once per
        // request (raw labels only — ZDR-safe) and attach them for callers.
        let doom_loop_signals = doom_loop
            .as_ref()
            .map(|collector| collector.take())
            .unwrap_or_default();
        if !doom_loop_signals.is_empty() {
            tracing::warn!(
                request_id = %request_id,
                triggers = ?doom_loop_signals.iter().map(|s| s.raw.as_str()).collect::<Vec<_>>(),
                "server reported doom-loop triggers for this response"
            );
        }

        let conversation_response = ConversationResponse {
            items,
            stop_reason,
            usage,
            cost_usd_ticks,
            message_chunks_emitted: message_chunk_count,
            doom_loop_signals,
            stop_message: None, // not reported on the Responses API
            fallback_served_model: None,
        };

        yield InferenceEvent::Completed {
            request_id: request_id.clone(),
            response: Box::new(conversation_response),
            metrics,
        };
    }
}

/// When the terminal Responses body has an empty assistant message but the
/// stream already delivered text deltas, back-fill the trailing Assistant
/// item so empty-response retry does not fire after a painted reply.
fn inject_streaming_text_fallback(items: &mut Vec<ConversationItem>, text_acc: &str) {
    if text_acc.is_empty() {
        return;
    }
    match items.iter_mut().rev().find_map(|item| match item {
        ConversationItem::Assistant(a) => Some(a),
        _ => None,
    }) {
        Some(assistant) if assistant.content.is_empty() => {
            assistant.content = std::sync::Arc::<str>::from(text_acc);
        }
        Some(_) => {}
        None => {
            items.push(ConversationItem::assistant(text_acc));
        }
    }
}

/// Streamed function-call capture for dialects (ChatGPT Codex) that omit
/// tool calls from `response.completed.output`.
#[derive(Debug, Clone)]
struct StreamedToolCall {
    call_id: String,
    name: String,
    arguments: String,
}

/// When the terminal Responses body has no tool calls but the stream already
/// delivered FunctionCall items/args, inject them so the shell executes tools
/// instead of treating the turn as empty.
fn inject_streaming_tool_calls_fallback(
    items: &mut Vec<ConversationItem>,
    streamed: Vec<StreamedToolCall>,
) {
    let tool_calls: Vec<ToolCall> = streamed
        .into_iter()
        .filter(|t| !t.call_id.is_empty() && !t.name.is_empty())
        .map(|t| ToolCall {
            id: std::sync::Arc::<str>::from(t.call_id),
            name: t.name,
            arguments: std::sync::Arc::<str>::from(t.arguments),
        })
        .collect();
    if tool_calls.is_empty() {
        return;
    }
    let already_has_tools = items.iter().any(|item| match item {
        ConversationItem::Assistant(a) => !a.tool_calls.is_empty(),
        _ => false,
    });
    if already_has_tools {
        return;
    }
    match items.iter_mut().rev().find_map(|item| match item {
        ConversationItem::Assistant(a) => Some(a),
        _ => None,
    }) {
        Some(assistant) => {
            assistant.tool_calls = tool_calls;
        }
        None => {
            items.push(ConversationItem::assistant_tool_calls(tool_calls));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_openai::types::responses as rs_types;
    use futures_util::stream;
    use std::pin::pin;

    fn rid() -> RequestId {
        RequestId::from("resp-test")
    }

    /// Build a minimal `rs_types::Response` for use in `ResponseCompleted`
    fn build_response(status: rs_types::Status) -> rs_types::Response {
        rs_types::Response {
            background: None,
            billing: None,
            conversation: None,
            created_at: 0,
            completed_at: None,
            error: None,
            id: "resp_1".into(),
            incomplete_details: None,
            instructions: None,
            max_output_tokens: None,
            metadata: None,
            model: "test-model".into(),
            object: "response".into(),
            output: vec![],
            parallel_tool_calls: None,
            previous_response_id: None,
            prompt: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
            reasoning: None,
            safety_identifier: None,
            service_tier: None,
            status,
            temperature: None,
            text: None,
            tool_choice: None,
            tools: None,
            top_logprobs: None,
            top_p: None,
            truncation: None,
            usage: None,
        }
    }

    fn empty_completed_response() -> rs_types::Response {
        build_response(rs_types::Status::Completed)
    }

    fn failed_response_with_error(message: &str) -> rs_types::Response {
        let mut r = build_response(rs_types::Status::Failed);
        r.error = Some(rs_types::ErrorObject {
            code: "server_error".into(),
            message: message.into(),
        });
        r
    }

    fn text_delta_event(delta: &str) -> rs::ResponseStreamEvent {
        rs::ResponseStreamEvent::ResponseOutputTextDelta(rs_types::ResponseTextDeltaEvent {
            sequence_number: 0,
            item_id: "item-1".into(),
            output_index: 0,
            content_index: 0,
            delta: delta.into(),
            logprobs: None,
        })
    }

    fn completed_event() -> rs::ResponseStreamEvent {
        rs::ResponseStreamEvent::ResponseCompleted(rs_types::ResponseCompletedEvent {
            response: empty_completed_response(),
            sequence_number: 0,
        })
    }

    async fn collect(s: impl Stream<Item = InferenceEvent>) -> Vec<InferenceEvent> {
        let mut out = Vec::new();
        let mut s = pin!(s);
        while let Some(ev) = s.next().await {
            out.push(ev);
        }
        out
    }

    #[tokio::test]
    async fn missing_completed_event_yields_failed() {
        let raw =
            stream::iter(Vec::<Result<rs::ResponseStreamEvent, InferenceError>>::new()).boxed();
        let events = collect(stream_responses(
            raw,
            None,
            rid(),
            Duration::from_secs(60),
            None,
        ))
        .await;

        match events.last().unwrap() {
            InferenceEvent::Failed { error, .. } => {
                assert_eq!(error.kind, crate::events::InferenceErrorKind::Api);
                assert_eq!(error.status_code, Some(500));
            }
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn text_delta_then_completed_yields_completed_with_stop() {
        let raw = stream::iter(vec![Ok(text_delta_event("hello")), Ok(completed_event())]).boxed();
        let events = collect(stream_responses(
            raw,
            None,
            rid(),
            Duration::from_secs(60),
            None,
        ))
        .await;

        let text_tokens: Vec<&str> = events
            .iter()
            .filter_map(|e| match e {
                InferenceEvent::ChannelToken {
                    channel: InferenceChannel::Text,
                    text,
                    ..
                } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(text_tokens, vec!["hello"]);

        match events.last().unwrap() {
            InferenceEvent::Completed { response, .. } => {
                assert_eq!(response.stop_reason, Some(StopReason::Stop));
                // Terminal body is empty; streamed text must still back-fill.
                assert_eq!(response.assistant_text(), "hello");
                assert!(
                    response.empty_reason().is_none(),
                    "streamed text must not classify as empty_response"
                );
                assert_eq!(response.message_chunks_emitted, 1);
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    /// ChatGPT Codex streams function_call items but leaves
    /// `response.completed.output` empty — must still surface tool calls.
    #[tokio::test]
    async fn codex_style_function_call_with_empty_completed_output() {
        let added = rs::ResponseStreamEvent::ResponseOutputItemAdded(
            rs_types::ResponseOutputItemAddedEvent {
                sequence_number: 1,
                output_index: 0,
                item: rs_types::OutputItem::FunctionCall(rs_types::FunctionToolCall {
                    arguments: String::new(),
                    call_id: "call_1".into(),
                    name: "list_dir".into(),
                    id: Some("fc_1".into()),
                    status: Some(rs_types::OutputStatus::InProgress),
                }),
            },
        );
        let args_done = rs::ResponseStreamEvent::ResponseFunctionCallArgumentsDone(
            rs_types::ResponseFunctionCallArgumentsDoneEvent {
                name: Some("list_dir".into()),
                sequence_number: 2,
                item_id: "fc_1".into(),
                output_index: 0,
                arguments: r#"{"target_directory":"."}"#.into(),
            },
        );
        let item_done = rs::ResponseStreamEvent::ResponseOutputItemDone(
            rs_types::ResponseOutputItemDoneEvent {
                sequence_number: 3,
                output_index: 0,
                item: rs_types::OutputItem::FunctionCall(rs_types::FunctionToolCall {
                    arguments: r#"{"target_directory":"."}"#.into(),
                    call_id: "call_1".into(),
                    name: "list_dir".into(),
                    id: Some("fc_1".into()),
                    status: Some(rs_types::OutputStatus::Completed),
                }),
            },
        );
        let raw = stream::iter(vec![
            Ok(added),
            Ok(args_done),
            Ok(item_done),
            Ok(completed_event()), // empty output — Codex dialect
        ])
        .boxed();
        let events = collect(stream_responses(
            raw,
            None,
            rid(),
            Duration::from_secs(60),
            None,
        ))
        .await;

        let tool_deltas = events
            .iter()
            .filter(|e| matches!(e, InferenceEvent::ToolCallDelta { .. }))
            .count();
        assert!(
            tool_deltas >= 1,
            "expected ToolCallDelta events from stream"
        );

        match events.last().unwrap() {
            InferenceEvent::Completed { response, .. } => {
                assert_eq!(response.stop_reason, Some(StopReason::ToolCalls));
                let calls = response.tool_calls();
                assert_eq!(calls.len(), 1, "streamed tool call must be recovered");
                assert_eq!(calls[0].name, "list_dir");
                assert_eq!(calls[0].id.as_ref(), "call_1");
                assert_eq!(calls[0].arguments.as_ref(), r#"{"target_directory":"."}"#);
                assert!(
                    response.empty_reason().is_none(),
                    "function_call-only Codex turn must not be empty_response"
                );
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn text_done_without_deltas_fills_assistant_content() {
        let done =
            rs::ResponseStreamEvent::ResponseOutputTextDone(rs_types::ResponseTextDoneEvent {
                sequence_number: 0,
                item_id: "item-1".into(),
                output_index: 0,
                content_index: 0,
                text: "full message".into(),
                logprobs: None,
            });
        let raw = stream::iter(vec![Ok(done), Ok(completed_event())]).boxed();
        let events = collect(stream_responses(
            raw,
            None,
            rid(),
            Duration::from_secs(60),
            None,
        ))
        .await;

        match events.last().unwrap() {
            InferenceEvent::Completed { response, .. } => {
                assert_eq!(response.assistant_text(), "full message");
                assert!(response.empty_reason().is_none());
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    #[test]
    fn inject_text_fallback_fills_empty_assistant() {
        let mut items = vec![ConversationItem::assistant("")];
        inject_streaming_text_fallback(&mut items, "painted");
        match &items[0] {
            ConversationItem::Assistant(a) => assert_eq!(a.content.as_ref(), "painted"),
            other => panic!("expected Assistant, got {other:?}"),
        }
    }

    #[test]
    fn inject_text_fallback_does_not_overwrite_existing_content() {
        let mut items = vec![ConversationItem::assistant("from body")];
        inject_streaming_text_fallback(&mut items, "from stream");
        match &items[0] {
            ConversationItem::Assistant(a) => assert_eq!(a.content.as_ref(), "from body"),
            other => panic!("expected Assistant, got {other:?}"),
        }
    }

    #[test]
    fn empty_failed_response_is_not_treated_as_output() {
        let event = rs::ResponseStreamEvent::ResponseFailed(rs_types::ResponseFailedEvent {
            response: failed_response_with_error("boom"),
            sequence_number: 0,
        });
        assert!(!responses_event_may_have_output(&event));
    }

    #[tokio::test]
    async fn response_failed_yields_failed_500() {
        let failed = rs::ResponseStreamEvent::ResponseFailed(rs_types::ResponseFailedEvent {
            response: failed_response_with_error("boom"),
            sequence_number: 0,
        });
        let raw = stream::iter(vec![Ok(failed)]).boxed();
        let events = collect(stream_responses(
            raw,
            None,
            rid(),
            Duration::from_secs(60),
            None,
        ))
        .await;

        match events.last().unwrap() {
            InferenceEvent::Failed { error, .. } => {
                assert_eq!(error.kind, crate::events::InferenceErrorKind::Api);
                assert_eq!(error.status_code, Some(500));
                assert!(error.message.contains("boom"));
            }
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn mid_stream_transport_error_yields_failed() {
        let raw = stream::iter(vec![
            Ok(text_delta_event("hi")),
            Err(InferenceError::EventStreamError("conn reset".into())),
        ])
        .boxed();
        let events = collect(stream_responses(
            raw,
            None,
            rid(),
            Duration::from_secs(60),
            None,
        ))
        .await;

        assert!(
            events
                .iter()
                .any(|e| matches!(e, InferenceEvent::Failed { .. }))
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, InferenceEvent::Completed { .. }))
        );
    }

    #[tokio::test(start_paused = true)]
    async fn idle_timeout_when_stream_stalls() {
        let raw = stream::iter(vec![Ok(text_delta_event("hi"))])
            .chain(stream::pending())
            .boxed();
        let events = collect(stream_responses(
            raw,
            None,
            rid(),
            Duration::from_millis(100),
            None,
        ))
        .await;

        match events.last().unwrap() {
            InferenceEvent::Failed { error, .. } => {
                assert_eq!(error.kind, crate::events::InferenceErrorKind::IdleTimeout);
            }
            other => panic!("expected Failed(IdleTimeout), got {other:?}"),
        }
    }

    /// Keepalives are absorbed at L1 before Layer-2, so a keepalive-only
    /// Responses wire presents here as a permanently pending stream. The
    /// semantic idle deadline must still fire — transport heartbeats must
    /// not keep a dead stream alive indefinitely.
    #[tokio::test(start_paused = true)]
    async fn idle_timeout_when_only_transport_activity_then_stalls() {
        let raw = stream::pending::<Result<rs::ResponseStreamEvent, InferenceError>>().boxed();
        let events = collect(stream_responses(
            raw,
            None,
            rid(),
            Duration::from_millis(100),
            None,
        ))
        .await;

        match events.last().unwrap() {
            InferenceEvent::Failed { error, .. } => {
                assert_eq!(error.kind, crate::events::InferenceErrorKind::IdleTimeout);
            }
            other => panic!("expected Failed(IdleTimeout), got {other:?}"),
        }
        // No model output / completion from a keepalive-only wire.
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, InferenceEvent::Completed { .. }))
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, InferenceEvent::ChannelToken { .. }))
        );
    }

    /// Non-content status transitions (L2 analogue of filtered keepalives:
    /// they wake the outer poll but do not reset content progress) must not
    /// prevent the content-aware idle deadline from firing.
    #[tokio::test(start_paused = true)]
    async fn idle_timeout_on_repeated_non_content_status_events() {
        use async_stream::stream as async_stream;

        let idle = Duration::from_millis(100);
        // Emit non-content status events spaced beyond the idle window.
        // `start_paused` advances virtual time on each sleep so this is
        // deterministic (no wall-clock flakiness). After the gap, the second
        // non-content event trips content-aware idle before any completion.
        let raw = async_stream! {
            yield Ok(rs::ResponseStreamEvent::ResponseQueued(
                rs_types::ResponseQueuedEvent {
                    sequence_number: 0,
                    response: empty_completed_response(),
                },
            ));
            tokio::time::sleep(idle + Duration::from_millis(10)).await;
            yield Ok(rs::ResponseStreamEvent::ResponseInProgress(
                rs_types::ResponseInProgressEvent {
                    sequence_number: 1,
                    response: empty_completed_response(),
                },
            ));
        }
        .boxed();

        let events = collect(stream_responses(raw, None, rid(), idle, None)).await;

        match events.last().unwrap() {
            InferenceEvent::Failed { error, .. } => {
                assert_eq!(error.kind, crate::events::InferenceErrorKind::IdleTimeout);
            }
            other => panic!("expected Failed(IdleTimeout), got {other:?}"),
        }
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, InferenceEvent::Completed { .. }))
        );
    }

    #[tokio::test]
    async fn model_metadata_yielded_after_stream_started() {
        let raw = stream::iter(vec![Ok(completed_event())]).boxed();
        let metadata = ResponseModelMetadata {
            context_window: Some(8192),
            ..Default::default()
        };
        let events = collect(stream_responses(
            raw,
            Some(metadata),
            rid(),
            Duration::from_secs(60),
            None,
        ))
        .await;

        assert!(matches!(events[0], InferenceEvent::StreamStarted { .. }));
        assert!(matches!(events[1], InferenceEvent::ModelMetadata { .. }));
    }

    #[test]
    fn meaningful_content_classifier_basics() {
        // Text delta with content is meaningful.
        let event = text_delta_event("foo");
        assert!(responses_event_has_meaningful_content(&event));
        // Empty text delta is not.
        let empty = text_delta_event("");
        assert!(!responses_event_has_meaningful_content(&empty));
        // Completed is meaningful (terminal).
        assert!(responses_event_has_meaningful_content(&completed_event()));
    }

    #[test]
    fn output_classifier_covers_non_forwarded_backend_events() {
        let queued = rs::ResponseStreamEvent::ResponseQueued(rs_types::ResponseQueuedEvent {
            sequence_number: 0,
            response: empty_completed_response(),
        });
        assert!(!responses_event_may_have_output(&queued));

        let response_error = rs::ResponseStreamEvent::ResponseError(rs_types::ResponseErrorEvent {
            sequence_number: 1,
            code: Some("server_error".into()),
            message: "failed before output".into(),
            param: None,
        });
        assert!(!responses_event_may_have_output(&response_error));

        let refusal =
            rs::ResponseStreamEvent::ResponseRefusalDelta(rs_types::ResponseRefusalDeltaEvent {
                sequence_number: 1,
                item_id: "item-1".into(),
                output_index: 0,
                content_index: 0,
                delta: "no".into(),
            });
        assert!(responses_event_may_have_output(&refusal));

        let backend_progress = rs::ResponseStreamEvent::ResponseWebSearchCallSearching(
            rs_types::ResponseWebSearchCallSearchingEvent {
                sequence_number: 2,
                output_index: 0,
                item_id: "search-1".into(),
            },
        );
        assert!(responses_event_may_have_output(&backend_progress));
    }

    #[tokio::test]
    async fn tracked_stream_marks_non_forwarded_refusal_as_output() {
        let output_observed = Arc::new(AtomicBool::new(false));
        let refusal =
            rs::ResponseStreamEvent::ResponseRefusalDelta(rs_types::ResponseRefusalDeltaEvent {
                sequence_number: 0,
                item_id: "item-1".into(),
                output_index: 0,
                content_index: 0,
                delta: "no".into(),
            });
        let raw = stream::iter(vec![Ok(refusal), Ok(completed_event())]).boxed();
        let _ = collect(stream_responses_tracked(
            raw,
            None,
            rid(),
            Duration::from_secs(60),
            None,
            Arc::clone(&output_observed),
        ))
        .await;

        assert!(output_observed.load(Ordering::Relaxed));
    }

    fn function_call_added_event(
        output_index: u32,
        call_id: &str,
        name: &str,
    ) -> rs::ResponseStreamEvent {
        rs::ResponseStreamEvent::ResponseOutputItemAdded(rs_types::ResponseOutputItemAddedEvent {
            sequence_number: 0,
            output_index,
            item: rs_types::OutputItem::FunctionCall(rs_types::FunctionToolCall {
                arguments: String::new(),
                call_id: call_id.into(),
                name: name.into(),
                id: None,
                status: None,
            }),
        })
    }

    fn function_call_args_delta_event(output_index: u32, delta: &str) -> rs::ResponseStreamEvent {
        rs::ResponseStreamEvent::ResponseFunctionCallArgumentsDelta(
            rs_types::ResponseFunctionCallArgumentsDeltaEvent {
                sequence_number: 0,
                item_id: format!("item-{output_index}"),
                output_index,
                delta: delta.into(),
            },
        )
    }

    type Delta = (u32, Option<String>, Option<String>, Option<String>);

    /// Extract all ToolCallDelta events as (tool_index, id, name, arguments_delta).
    fn tool_call_deltas(evs: &[InferenceEvent]) -> Vec<Delta> {
        evs.iter()
            .filter_map(|e| match e {
                InferenceEvent::ToolCallDelta {
                    tool_index,
                    id,
                    name,
                    arguments_delta,
                    ..
                } => Some((
                    *tool_index,
                    id.clone(),
                    name.clone(),
                    arguments_delta.clone(),
                )),
                _ => None,
            })
            .collect()
    }

    #[tokio::test]
    async fn function_call_emits_initial_id_name_then_arg_deltas() {
        let events: Vec<Result<rs::ResponseStreamEvent, InferenceError>> = vec![
            Ok(function_call_added_event(0, "call_xyz", "do_thing")),
            Ok(function_call_args_delta_event(0, "{\"x\":")),
            Ok(function_call_args_delta_event(0, "1}")),
            Ok(completed_event()),
        ];
        let raw = stream::iter(events).boxed();
        let evs = collect(stream_responses(
            raw,
            None,
            rid(),
            Duration::from_secs(60),
            None,
        ))
        .await;
        let deltas = tool_call_deltas(&evs);

        assert_eq!(deltas.len(), 3);
        assert_eq!(deltas[0].0, 0);
        assert_eq!(deltas[0].1.as_deref(), Some("call_xyz"));
        assert_eq!(deltas[0].2.as_deref(), Some("do_thing"));
        assert_eq!(deltas[0].3, None);
        assert_eq!(deltas[1].0, 0);
        assert_eq!(deltas[1].1, None);
        assert_eq!(deltas[1].2, None);
        assert_eq!(deltas[1].3.as_deref(), Some("{\"x\":"));
        assert_eq!(deltas[2].3.as_deref(), Some("1}"));
    }

    #[tokio::test]
    async fn function_call_args_delta_without_added_event_is_dropped() {
        // ArgumentsDelta with no preceding OutputItemAdded has no
        // output_index → tool_index mapping; drop silently.
        let events: Vec<Result<rs::ResponseStreamEvent, InferenceError>> = vec![
            Ok(function_call_args_delta_event(7, "{\"oops\":1}")),
            Ok(completed_event()),
        ];
        let raw = stream::iter(events).boxed();
        let evs = collect(stream_responses(
            raw,
            None,
            rid(),
            Duration::from_secs(60),
            None,
        ))
        .await;
        assert_eq!(tool_call_deltas(&evs).len(), 0);
    }

    #[tokio::test]
    async fn multiple_function_calls_get_distinct_tool_indices() {
        let events: Vec<Result<rs::ResponseStreamEvent, InferenceError>> = vec![
            Ok(function_call_added_event(0, "call_a", "tool_a")),
            Ok(function_call_added_event(1, "call_b", "tool_b")),
            Ok(function_call_args_delta_event(0, "a-args")),
            Ok(function_call_args_delta_event(1, "b-args")),
            Ok(completed_event()),
        ];
        let raw = stream::iter(events).boxed();
        let evs = collect(stream_responses(
            raw,
            None,
            rid(),
            Duration::from_secs(60),
            None,
        ))
        .await;
        let deltas = tool_call_deltas(&evs);

        assert_eq!(deltas.len(), 4);
        assert_eq!(deltas[0].0, 0);
        assert_eq!(deltas[0].1.as_deref(), Some("call_a"));
        assert_eq!(deltas[1].0, 1);
        assert_eq!(deltas[1].1.as_deref(), Some("call_b"));
        assert_eq!(deltas[2].0, 0);
        assert_eq!(deltas[2].3.as_deref(), Some("a-args"));
        assert_eq!(deltas[3].0, 1);
        assert_eq!(deltas[3].3.as_deref(), Some("b-args"));
    }

    #[tokio::test]
    async fn doom_loop_collector_signals_land_on_completed_response() {
        use xai_grok_inference_types::doom_loop::{
            DOOM_LOOP_CHECK_EVENT_TYPE, SAMPLE_CHECK_EVENT_DATA,
        };
        let collector = crate::doom_loop::DoomLoopSignalCollector::default();
        assert!(collector.absorb(DOOM_LOOP_CHECK_EVENT_TYPE, SAMPLE_CHECK_EVENT_DATA));
        let raw = stream::iter(vec![Ok(text_delta_event("hello")), Ok(completed_event())]).boxed();
        let events = collect(stream_responses(
            raw,
            None,
            rid(),
            Duration::from_secs(60),
            Some(collector),
        ))
        .await;

        match events.last().unwrap() {
            InferenceEvent::Completed { response, .. } => {
                assert_eq!(response.doom_loop_signals.len(), 1);
                assert_eq!(
                    response.doom_loop_signals[0].raw,
                    "tail_repetition:4@response"
                );
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    /// An armed collector holding a confident signal aborts the attempt with
    /// a retryable doom-loop failure; disarmed, the same stream completes and
    /// the signals ride the response instead.
    #[tokio::test]
    async fn confident_signal_aborts_stream_unless_disarmed() {
        let confident = r#"{"type":"response.doom_loop_check","doom_loop_check":{"triggers":["tail_repetition:8@thinking"]}}"#;

        let collector = crate::doom_loop::DoomLoopSignalCollector::default();
        assert!(collector.absorb("response.doom_loop_check", confident));
        let raw = stream::iter(vec![Ok(text_delta_event("hi")), Ok(completed_event())]).boxed();
        let events = collect(stream_responses(
            raw,
            None,
            rid(),
            Duration::from_secs(60),
            Some(collector),
        ))
        .await;
        match events.last().unwrap() {
            InferenceEvent::Failed { error, .. } => {
                assert_eq!(
                    error.kind,
                    crate::events::InferenceErrorKind::DoomLoopDetected
                );
                assert!(error.is_retryable);
                assert_eq!(
                    error.doom_loop_triggers.as_deref(),
                    Some(&["tail_repetition:8@thinking".to_string()][..])
                );
            }
            other => panic!("expected Failed(DoomLoopDetected), got {other:?}"),
        }
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, InferenceEvent::Completed { .. }))
        );

        let collector = crate::doom_loop::DoomLoopSignalCollector::default();
        assert!(collector.absorb("response.doom_loop_check", confident));
        collector.disarm_abort();
        let raw = stream::iter(vec![Ok(text_delta_event("hi")), Ok(completed_event())]).boxed();
        let events = collect(stream_responses(
            raw,
            None,
            rid(),
            Duration::from_secs(60),
            Some(collector),
        ))
        .await;
        match events.last().unwrap() {
            InferenceEvent::Completed { response, .. } => {
                assert_eq!(response.doom_loop_signals.len(), 1);
            }
            other => panic!("expected Completed after disarm, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn doom_loop_signals_empty_without_collector_or_triggers() {
        let raw = stream::iter(vec![Ok(completed_event())]).boxed();
        let events = collect(stream_responses(
            raw,
            None,
            rid(),
            Duration::from_secs(60),
            None,
        ))
        .await;
        match events.last().unwrap() {
            InferenceEvent::Completed { response, .. } => {
                assert!(response.doom_loop_signals.is_empty());
            }
            other => panic!("expected Completed, got {other:?}"),
        }

        // A collector that never saw a trigger also leaves the field empty.
        let raw = stream::iter(vec![Ok(completed_event())]).boxed();
        let events = collect(stream_responses(
            raw,
            None,
            rid(),
            Duration::from_secs(60),
            Some(crate::doom_loop::DoomLoopSignalCollector::default()),
        ))
        .await;
        match events.last().unwrap() {
            InferenceEvent::Completed { response, .. } => {
                assert!(response.doom_loop_signals.is_empty());
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }
}
