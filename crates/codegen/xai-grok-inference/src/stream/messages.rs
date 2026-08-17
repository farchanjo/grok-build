//! Layer-2 stream transform for the Anthropic Messages API.
//!
//! Consumes a raw `MessageStreamEvent` stream and produces
//! [`InferenceEvent`]s. Pure: no I/O, no shell coupling.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use futures_util::stream::{BoxStream, Stream};

use xai_grok_inference_types::messages::{self, MessageStreamEvent};
use xai_grok_inference_types::{
    AssistantItem, AssistantProviderPayload, ConversationItem, ConversationResponse,
    InferenceError, MessagesAssistantPayload, ResponseModelMetadata, StopReason, TokenUsage,
    ToolCall, rs,
};

use crate::events::{InferenceChannel, InferenceErrorInfo, InferenceEvent};
use crate::metrics::InferenceLatencyStats;
use crate::types::RequestId;

/// Returns whether a Messages API event reflects real model progress
/// rather than a liveness-only heartbeat (Ping) or an unknown event type.
pub(crate) fn messages_event_has_meaningful_content(event: &MessageStreamEvent) -> bool {
    match event {
        MessageStreamEvent::Ping | MessageStreamEvent::Unknown { .. } => false,
        MessageStreamEvent::MessageStart { .. }
        | MessageStreamEvent::MessageDelta { .. }
        | MessageStreamEvent::MessageStop
        | MessageStreamEvent::ContentBlockStart { .. }
        | MessageStreamEvent::ContentBlockDelta { .. }
        | MessageStreamEvent::ContentBlockStop { .. }
        | MessageStreamEvent::Error { .. } => true,
    }
}

/// Per-block streaming accumulator. The Anthropic Messages API reports
/// content as a sequence of indexed blocks (text / thinking /
/// tool_use / redacted_thinking / …), each with start / delta / stop
/// events. We accumulate per-index and finalize each block on
/// `ContentBlockStop`.
struct BlockState {
    block_type: BlockType,
    /// Fully-formed block captured at start for variants that do not
    /// stream deltas (redacted_thinking, server tools, unknown, …).
    seed_block: Option<messages::ContentBlock>,
    text_acc: String,
    tool_name: String,
    tool_id: String,
    args_acc: String,
    thinking_acc: String,
    signature: String,
    /// Text-block citations accumulated if a future delta carries them.
    citations: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockType {
    Text,
    ToolUse,
    Thinking,
    RedactedThinking,
    /// Any non-executable / non-projected block preserved only in the
    /// ordered Messages payload (server tools, documents, unknown, …).
    PayloadOnly,
}

/// Transform a raw Anthropic Messages API stream into a stream of
/// [`InferenceEvent`]s.
///
/// Yields exactly one terminal event ([`InferenceEvent::Completed`] or
/// [`InferenceEvent::Failed`]) per request. Server-side `Error` events
/// translate to `InferenceError::Api { status: 500, .. }` so the actor's
/// retry loop treats them as retryable transport-level errors.
pub fn stream_messages<'a>(
    raw_stream: BoxStream<'a, Result<MessageStreamEvent, InferenceError>>,
    model_metadata: Option<ResponseModelMetadata>,
    request_id: RequestId,
    idle_timeout: Duration,
) -> impl Stream<Item = InferenceEvent> + Send + 'a {
    async_stream::stream! {
        use messages::{ContentBlock, StreamDelta};

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

        // Per-block accumulators keyed by content block index.
        let mut blocks: BTreeMap<u32, BlockState> = BTreeMap::new();

        // Final-message-level accumulators
        let mut final_model: Option<String> = None;
        // Anthropic Messages API `input_tokens` is the uncached portion; cache hits and writes are reported
        // in separate buckets and must be summed for the true total prompt size.
        let mut final_input_tokens: u32 = 0;
        let mut final_cache_read_input_tokens: u32 = 0;
        let mut final_cache_creation_input_tokens: u32 = 0;
        let mut final_output_tokens: u32 = 0;
        let mut final_stop_reason: Option<StopReason> = None;
        let mut final_stop_message: Option<String> = None;

        // Assistant-response accumulators (built up as ContentBlockStop
        // events fire). Reasoning is collected as N sibling
        // `ConversationItem::Reasoning` items (one per thinking /
        // redacted_thinking block) so multi-block thinking is lossless —
        // never last-write-wins.
        let mut assistant_text = String::new();
        let mut assistant_tool_calls: Vec<ToolCall> = Vec::new();
        let mut reasoning_items: Vec<rs::ReasoningItem> = Vec::new();
        // Ordered wire content for durable Messages replay.
        let mut ordered_content: Vec<ContentBlock> = Vec::new();

        // Index counters
        let mut chunk_index: u64 = 0;
        let mut message_chunk_count: u64 = 0;
        let mut first_token_emitted = false;
        let mut last_content_chunk_at = Instant::now();

        // Tool-call index counter for per-tool deltas (separate from
        // the block index, which can be interleaved with text/thinking
        // blocks).
        let mut next_tool_index: u32 = 0;
        let mut block_to_tool_index: BTreeMap<u32, u32> = BTreeMap::new();

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

            let event_has_content = messages_event_has_meaningful_content(&event);

            match event {
                MessageStreamEvent::MessageStart { message } => {
                    final_model = Some(message.model.clone());
                    final_input_tokens = message.usage.input_tokens;
                    final_cache_read_input_tokens = message.usage.cache_read_input_tokens;
                    final_cache_creation_input_tokens = message.usage.cache_creation_input_tokens;
                }

                MessageStreamEvent::ContentBlockStart {
                    index,
                    content_block,
                } => match content_block {
                    ContentBlock::Thinking {
                        thinking,
                        signature,
                    } => {
                        blocks.insert(
                            index,
                            BlockState {
                                block_type: BlockType::Thinking,
                                seed_block: None,
                                text_acc: String::new(),
                                tool_name: String::new(),
                                tool_id: String::new(),
                                args_acc: String::new(),
                                thinking_acc: thinking,
                                signature,
                                citations: None,
                            },
                        );
                        if !first_token_emitted {
                            first_token_emitted = true;
                            yield InferenceEvent::FirstToken {
                                request_id: request_id.clone(),
                            };
                        }
                    }
                    ContentBlock::RedactedThinking { data } => {
                        blocks.insert(
                            index,
                            BlockState {
                                block_type: BlockType::RedactedThinking,
                                seed_block: Some(ContentBlock::RedactedThinking {
                                    data: data.clone(),
                                }),
                                text_acc: String::new(),
                                tool_name: String::new(),
                                tool_id: String::new(),
                                args_acc: String::new(),
                                thinking_acc: String::new(),
                                signature: data,
                                citations: None,
                            },
                        );
                    }
                    ContentBlock::Text {
                        text, citations, ..
                    } => {
                        blocks.insert(
                            index,
                            BlockState {
                                block_type: BlockType::Text,
                                seed_block: None,
                                text_acc: text,
                                tool_name: String::new(),
                                tool_id: String::new(),
                                args_acc: String::new(),
                                thinking_acc: String::new(),
                                signature: String::new(),
                                citations,
                            },
                        );
                        if !first_token_emitted {
                            first_token_emitted = true;
                            yield InferenceEvent::FirstToken {
                                request_id: request_id.clone(),
                            };
                        }
                    }
                    ContentBlock::ToolUse {
                        id,
                        name,
                        input: _,
                        ..
                    } => {
                        let tool_index = next_tool_index;
                        next_tool_index += 1;
                        block_to_tool_index.insert(index, tool_index);

                        blocks.insert(
                            index,
                            BlockState {
                                block_type: BlockType::ToolUse,
                                seed_block: None,
                                text_acc: String::new(),
                                tool_name: name.clone(),
                                tool_id: id.clone(),
                                // Anthropic Messages API streams arguments via
                                // InputJsonDelta events; starting from
                                // "{}" then appending fragments would
                                // produce invalid JSON.
                                args_acc: String::new(),
                                thinking_acc: String::new(),
                                signature: String::new(),
                                citations: None,
                            },
                        );

                        // Emit initial id+name so subscribers can pre-allocate
                        // UI for the tool call before arguments stream in.
                        yield InferenceEvent::ToolCallDelta {
                            request_id: request_id.clone(),
                            tool_index,
                            id: Some(id),
                            name: Some(name),
                            arguments_delta: None,
                        };
                    }
                    // Server tools, documents, search results, compaction,
                    // unknown blocks: never become client tool calls. Keep
                    // the seed block so ContentBlockStop can append it to
                    // the ordered payload for lossless replay.
                    other => {
                        if let ContentBlock::Unknown { type_name, raw } = &other {
                            tracing::debug!(
                                type_name = %type_name,
                                preview = %ContentBlock::raw_diagnostic_preview(raw, 256),
                                "ignoring unknown Messages content block for execution; preserving for replay"
                            );
                        }
                        blocks.insert(
                            index,
                            BlockState {
                                block_type: BlockType::PayloadOnly,
                                seed_block: Some(other),
                                text_acc: String::new(),
                                tool_name: String::new(),
                                tool_id: String::new(),
                                args_acc: String::new(),
                                thinking_acc: String::new(),
                                signature: String::new(),
                                citations: None,
                            },
                        );
                    }
                },

                MessageStreamEvent::ContentBlockDelta { index, delta } => {
                    if let Some(state) = blocks.get_mut(&index) {
                        match delta {
                            StreamDelta::ThinkingDelta { thinking } => {
                                if !thinking.is_empty() {
                                    state.thinking_acc.push_str(&thinking);
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
                                        text: thinking,
                                        chunk_index,
                                    };
                                }
                            }
                            StreamDelta::SignatureDelta { signature } => {
                                state.signature = signature;
                            }
                            StreamDelta::TextDelta { text } => {
                                if !text.is_empty() {
                                    state.text_acc.push_str(&text);
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
                                        text,
                                        chunk_index,
                                    };
                                }
                            }
                            StreamDelta::InputJsonDelta { partial_json } => {
                                state.args_acc.push_str(&partial_json);
                                if let Some(&tool_index) = block_to_tool_index.get(&index) {
                                    yield InferenceEvent::ToolCallDelta {
                                        request_id: request_id.clone(),
                                        tool_index,
                                        id: None,
                                        name: None,
                                        arguments_delta: Some(partial_json),
                                    };
                                }
                            }
                            StreamDelta::CitationsDelta { citation } => {
                                // Append to the current text block's citations
                                // so streamed replay matches non-streamed text
                                // blocks that already carry citations[].
                                if state.block_type == BlockType::Text {
                                    state
                                        .citations
                                        .get_or_insert_with(Vec::new)
                                        .push(citation);
                                } else {
                                    tracing::debug!(
                                        block_type = ?state.block_type,
                                        "citations_delta on non-text block ignored for projection"
                                    );
                                }
                            }
                            StreamDelta::Unknown { type_name, raw } => {
                                tracing::debug!(
                                    type_name = %type_name,
                                    preview = %ContentBlock::raw_diagnostic_preview(&raw, 128),
                                    "ignoring unknown Messages stream delta"
                                );
                            }
                        }
                    }
                }

                MessageStreamEvent::ContentBlockStop { index } => {
                    if let Some(state) = blocks.remove(&index) {
                        match state.block_type {
                            BlockType::Text => {
                                if !state.text_acc.is_empty() {
                                    if !assistant_text.is_empty() {
                                        assistant_text.push('\n');
                                    }
                                    assistant_text.push_str(&state.text_acc);
                                }
                                ordered_content.push(ContentBlock::Text {
                                    text: state.text_acc,
                                    cache_control: None,
                                    citations: state.citations,
                                });
                            }
                            BlockType::Thinking => {
                                if !state.thinking_acc.is_empty() || !state.signature.is_empty() {
                                    let summary = if state.thinking_acc.is_empty() {
                                        vec![]
                                    } else {
                                        vec![rs::SummaryPart::SummaryText(
                                            rs::SummaryTextContent {
                                                text: state.thinking_acc.clone(),
                                            },
                                        )]
                                    };
                                    let encrypted_content = if state.signature.is_empty() {
                                        None
                                    } else {
                                        Some(state.signature.clone())
                                    };
                                    reasoning_items.push(rs::ReasoningItem {
                                        id: String::new(),
                                        summary,
                                        content: None,
                                        encrypted_content,
                                        status: None,
                                    });
                                }
                                ordered_content.push(ContentBlock::Thinking {
                                    thinking: state.thinking_acc,
                                    signature: state.signature,
                                });
                            }
                            BlockType::RedactedThinking => {
                                let data = state
                                    .seed_block
                                    .and_then(|b| match b {
                                        ContentBlock::RedactedThinking { data } => Some(data),
                                        _ => None,
                                    })
                                    .unwrap_or(state.signature);
                                if !data.is_empty() {
                                    reasoning_items.push(rs::ReasoningItem {
                                        id: String::new(),
                                        summary: vec![],
                                        content: None,
                                        encrypted_content: Some(data.clone()),
                                        status: None,
                                    });
                                }
                                ordered_content
                                    .push(ContentBlock::RedactedThinking { data });
                            }
                            BlockType::ToolUse => {
                                assistant_tool_calls.push(ToolCall {
                                    id: std::sync::Arc::<str>::from(state.tool_id.clone()),
                                    name: state.tool_name.clone(),
                                    arguments: std::sync::Arc::<str>::from(state.args_acc.clone()),
                                });
                                let input = serde_json::from_str(&state.args_acc)
                                    .unwrap_or(serde_json::json!({}));
                                ordered_content.push(ContentBlock::ToolUse {
                                    id: state.tool_id,
                                    name: state.tool_name,
                                    input,
                                    cache_control: None,
                                });
                            }
                            BlockType::PayloadOnly => {
                                if let Some(seed) = state.seed_block {
                                    ordered_content.push(seed);
                                }
                            }
                        }
                    }
                }

                MessageStreamEvent::MessageDelta { delta, usage } => {
                    // Normalize the provider's stop detail to a plain message;
                    // the shell logs it when it surfaces a refusal.
                    if let Some(details) = delta.stop_details {
                        final_stop_message = details.explanation;
                    }
                    final_stop_reason = delta.stop_reason.map(|sr| match sr {
                        messages::StopReason::EndTurn => StopReason::Stop,
                        messages::StopReason::MaxTokens => StopReason::Length,
                        messages::StopReason::StopSequence => StopReason::Stop,
                        messages::StopReason::ToolUse => StopReason::ToolCalls,
                        // The model declined to continue; whatever streamed is
                        // the complete response, so end the turn cleanly.
                        messages::StopReason::Refusal => StopReason::ContentFilter,
                        messages::StopReason::PauseTurn => {
                            // Anthropic Messages API expects a resend-to-continue; we end the
                            // turn instead, so leave a triage trail.
                            tracing::warn!(
                                wire_stop_reason = "pause_turn",
                                "pause_turn ended the turn like stop (no auto-continue)"
                            );
                            StopReason::Stop
                        }
                        messages::StopReason::ModelContextWindowExceeded => {
                            // Output-side overflow on a successful stream: stays in the
                            // max_tokens truncation class — compact-on-error recovery needs
                            // an Api error carrying model metadata plus a prompt-side
                            // overflow, neither of which exists here.
                            tracing::warn!(
                                wire_stop_reason = "model_context_window_exceeded",
                                "context window hit mid-generation; surfacing as max_tokens truncation"
                            );
                            StopReason::Length
                        }
                        messages::StopReason::Unknown(wire) => {
                            tracing::warn!(
                                wire_stop_reason = %wire,
                                "unrecognized stop_reason in messages stream; treating as stop"
                            );
                            StopReason::Stop
                        }
                    });
                    final_output_tokens = usage.output_tokens;
                    // Optional on the delta; preserve message_start values when omitted.
                    if let Some(input) = usage.input_tokens {
                        final_input_tokens = input;
                    }
                    if let Some(cache_read) = usage.cache_read_input_tokens {
                        final_cache_read_input_tokens = cache_read;
                    }
                    if let Some(cache_creation) = usage.cache_creation_input_tokens {
                        final_cache_creation_input_tokens = cache_creation;
                    }
                }

                MessageStreamEvent::MessageStop => {
                    // Final message complete; the loop exits naturally
                    // when the underlying stream ends.
                }

                MessageStreamEvent::Ping => {
                    // Liveness only, no action; the inner timeout was
                    // already reset above by the successful `next()`.
                }

                MessageStreamEvent::Unknown { type_name, raw } => {
                    tracing::debug!(
                        type_name = %type_name,
                        preview = %ContentBlock::raw_diagnostic_preview(&raw, 128),
                        "ignoring unknown Messages stream event"
                    );
                }

                MessageStreamEvent::Error { error } => {
                    let error_message = format!("{}: {}", error.r#type, error.message);
                    let err = InferenceError::Api {
                        status: reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                        message: error_message,
                        model_metadata: None,
                        retry_after_secs: None,
                        should_retry: None,
                        diagnostics: None,
                        error_code: None,
                    };
                    yield InferenceEvent::Failed {
                        request_id: request_id.clone(),
                        error: InferenceErrorInfo::from(&err),
                    };
                    return;
                }
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
        }

        if final_stop_reason == Some(StopReason::Length) {
            yield InferenceEvent::Failed {
                request_id: request_id.clone(),
                error: InferenceErrorInfo::from(&InferenceError::MaxTokensTruncation),
            };
            return;
        }

        // ── Build the final response ─────────────────────────────────
        let model_id = final_model.unwrap_or_default();
        // Match the OAI Responses convention: prompt_tokens = full prompt, cached_prompt_tokens = cache hits only.
        let total_prompt_tokens = final_input_tokens
            .saturating_add(final_cache_read_input_tokens)
            .saturating_add(final_cache_creation_input_tokens);
        let usage = if total_prompt_tokens > 0 || final_output_tokens > 0 {
            Some(TokenUsage {
                prompt_tokens: total_prompt_tokens,
                completion_tokens: final_output_tokens,
                total_tokens: total_prompt_tokens.saturating_add(final_output_tokens),
                reasoning_tokens: 0,
                cached_prompt_tokens: final_cache_read_input_tokens,
            })
        } else {
            None
        };

        let stop_reason = if !assistant_tool_calls.is_empty() {
            // Completed tool_use blocks win even over Refusal: the calls are
            // real model output the agent loop must resolve.
            Some(StopReason::ToolCalls)
        } else {
            final_stop_reason
        };

        let provider_payload = if ordered_content.is_empty() {
            None
        } else {
            Some(AssistantProviderPayload {
                messages: Some(MessagesAssistantPayload {
                    content: ordered_content,
                    replayable: true,
                }),
            })
        };

        let assistant_item = ConversationItem::Assistant(AssistantItem {
            content: std::sync::Arc::<str>::from(assistant_text),
            tool_calls: assistant_tool_calls,
            model_id: Some(model_id),
            model_fingerprint: None,
            // The Messages API does not echo the applied reasoning effort.
            reasoning_effort: None,
            reasoning_details: Vec::new(),
            provider_payload,
        });

        let mut items: Vec<ConversationItem> = Vec::new();
        for r in reasoning_items {
            items.push(ConversationItem::Reasoning(r));
        }
        items.push(assistant_item);

        let stream_end = Instant::now();
        let metrics =
            InferenceLatencyStats::from_timestamps(stream_start, &chunk_timestamps, stream_end);

        let response = ConversationResponse {
            items,
            stop_reason,
            usage,
            // Anthropic Messages API carries no cost on the wire.
            cost_usd_ticks: None,
            message_chunks_emitted: message_chunk_count,
            doom_loop_signals: Vec::new(),
            stop_message: final_stop_message,
            fallback_served_model: None,
        };

        yield InferenceEvent::Completed {
            request_id: request_id.clone(),
            response: Box::new(response),
            metrics,
        };
    }
}

#[cfg(test)]
#[path = "messages_tests.rs"]
mod tests;
