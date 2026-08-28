//! Layer-2 stream transform for the Chat Completions API.
//!
//! Consumes a raw `ChatCompletionChunk` stream and produces
//! [`InferenceEvent`]s. Pure: no I/O, no shell coupling.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use futures_util::stream::{BoxStream, Stream};

use xai_grok_inference_types::{
    AssistantItem, ChatCompletionChunk, ConversationItem, ConversationResponse, InferenceError,
    ResponseModelMetadata, StopReason, TokenUsage, ToolCall,
};

use crate::config::ProviderIdentity;
use crate::events::{InferenceChannel, InferenceErrorInfo, InferenceEvent};
use crate::metrics::InferenceLatencyStats;
use crate::types::RequestId;

/// Transform a raw Chat Completions chunk stream into a stream of
/// [`InferenceEvent`]s.
///
/// The output stream emits exactly one terminal event per request:
/// [`InferenceEvent::Completed`] on normal stream end, or
/// [`InferenceEvent::Failed`] on error / idle timeout. Callers must not
/// consume past the terminal event (the implementation `return`s after
/// yielding it).
///
/// `idle_timeout` covers two cases:
/// 1. The transport stops yielding chunks at all (`tokio::time::timeout`).
/// 2. The transport keeps yielding empty / keepalive chunks but no
///    meaningful content (separate `last_content_chunk_at` timer).
///
/// Both produce `InferenceEvent::Failed { kind: IdleTimeout }`.
pub fn stream_chat_completions<'a>(
    raw_stream: BoxStream<'a, Result<ChatCompletionChunk, InferenceError>>,
    model_metadata: Option<ResponseModelMetadata>,
    request_id: RequestId,
    idle_timeout: Duration,
    requested_model: Option<&'a str>,
    provider_identity: ProviderIdentity,
) -> impl Stream<Item = InferenceEvent> + Send + 'a {
    async_stream::stream! {
        let stream_start = Instant::now();
        let mut chunk_timestamps: Vec<Instant> = Vec::new();

        // Emit StreamStarted before reading any chunks so subscribers
        // can record TTFB / TTLB baselines.
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

        // Per-response accumulators
        let mut first_chunk_seen = false;
        let mut first_choice_seen = false;
        let mut first_token_emitted = false;
        let mut model: String = String::new();
        let mut model_fingerprint: Option<String> = None;
        let mut usage: Option<TokenUsage> = None;
        let mut cost_usd_ticks: Option<i64> = None;
        let mut finish_reason: Option<StopReason> = None;

        let mut content_acc = String::new();
        let mut reasoning_acc = String::new();
        // OpenRouter structured reasoning detail blocks, accumulated across
        // chunks and stored verbatim on the AssistantItem for echo-back on
        // the next turn. Parsed as raw JSON values to tolerate shape drift.
        let mut reasoning_details_acc: Vec<serde_json::Value> = Vec::new();
        // Tool call deltas keyed by positional index. Each entry is
        // (id, name, arguments_buffer); the first chunk for an index
        // carries id+name and starts the arguments buffer, subsequent
        // chunks append to arguments only.
        let mut tool_call_acc: BTreeMap<u32, (String, String, String)> = BTreeMap::new();

        // Index counter spanning text + reasoning chunks (matches the
        // shell's chunk_index used for notification correlation).
        let mut chunk_index: u64 = 0;
        // Separate counter for AgentMessageChunk (text-only) emissions;
        // mirrored onto ConversationResponse.message_chunks_emitted so
        // downstream can detect lost-streaming-events scenarios.
        let mut message_chunk_count: u64 = 0;

        // Content-aware idle timer: the outer
        // `tokio::time::timeout(idle_timeout, stream.next())` already
        // catches "transport stops yielding chunks". This second timer
        // catches the more subtle case where the model keeps emitting
        // keepalive / empty-delta SSE events that satisfy the outer
        // timer but make no real progress -- some inference engines
        // do exactly that.
        let mut last_content_chunk_at = Instant::now();

        let mut stream = raw_stream;
        loop {
            let next = match tokio::time::timeout(idle_timeout, stream.next()).await {
                Ok(Some(next)) => next,
                Ok(None) => break, // stream ended normally
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
            let chunk = match next {
                Ok(chunk) => chunk,
                Err(err) => {
                    yield InferenceEvent::Failed {
                        request_id: request_id.clone(),
                        error: InferenceErrorInfo::from(&err),
                    };
                    return;
                }
            };

            if !first_chunk_seen {
                model = chunk.model.clone();
                model_fingerprint = chunk
                    .system_fingerprint
                    .clone()
                    .filter(|s| !s.is_empty());
                first_chunk_seen = true;
            }

            if let Some(u) = chunk.usage.clone() {
                // Wire cost is cumulative for the response, so last-write-wins.
                // Never clobber a known cost with missing/unreported.
                // Provider routing: xAI reports `cost_in_usd_ticks`; OpenRouter
                // reports `cost` (USD float). When ticks are absent, fall back
                // to the OpenRouter float and convert to the same tick scale
                // (1 USD = 1e10 ticks) so cost display/telemetry work uniformly.
                let chunk_cost =
                    xai_grok_inference_types::reported_cost_ticks(u.cost_in_usd_ticks)
                        .or_else(|| xai_grok_inference_types::usd_to_cost_ticks(u.cost.unwrap_or(0.0)));
                cost_usd_ticks = match (cost_usd_ticks, chunk_cost) {
                    (_, Some(n)) => Some(n),
                    (prev, None) => prev,
                };
                usage = Some(u.into());
            }

            // Track whether this chunk carried meaningful content.
            // Set inside the choices loop and checked at the end.
            let mut chunk_has_content = false;

            for choice in chunk.choices.into_iter() {
                first_choice_seen = true;
                if let Some(fr) = choice.finish_reason {
                    finish_reason = Some(fr.into());
                    chunk_has_content = true;
                }

                let delta = choice.delta;

                if let Some(text) = delta.content
                    && !text.is_empty()
                {
                    if !first_token_emitted {
                        first_token_emitted = true;
                        yield InferenceEvent::FirstToken {
                            request_id: request_id.clone(),
                        };
                    }
                    chunk_has_content = true;
                    chunk_timestamps.push(Instant::now());
                    chunk_index += 1;
                    message_chunk_count += 1;
                    content_acc.push_str(&text);
                    yield InferenceEvent::ChannelToken {
                        request_id: request_id.clone(),
                        channel: InferenceChannel::Text,
                        text,
                        chunk_index,
                    };
                }

                if let Some(thought) = delta.reasoning_content
                    && !thought.is_empty()
                {
                    if !first_token_emitted {
                        first_token_emitted = true;
                        yield InferenceEvent::FirstToken {
                            request_id: request_id.clone(),
                        };
                    }
                    chunk_has_content = true;
                    chunk_index += 1;
                    reasoning_acc.push_str(&thought);
                    yield InferenceEvent::ChannelToken {
                        request_id: request_id.clone(),
                        channel: InferenceChannel::Reasoning,
                        text: thought,
                        chunk_index,
                    };
                }

                for tc_delta in delta.tool_calls.into_iter() {
                    chunk_has_content = true;

                    let entry = tool_call_acc
                        .entry(tc_delta.index)
                        .or_insert_with(|| (String::new(), String::new(), String::new()));

                    let mut id_for_event: Option<String> = None;
                    let mut name_for_event: Option<String> = None;
                    let mut args_for_event: Option<String> = None;

                    if let Some(id) = tc_delta.id {
                        entry.0 = id.clone();
                        id_for_event = Some(id);
                    }
                    if let Some(func) = tc_delta.function {
                        if let Some(name) = func.name {
                            entry.1 = name.clone();
                            name_for_event = Some(name);
                        }
                        if let Some(args) = func.arguments {
                            entry.2.push_str(&args);
                            args_for_event = Some(args);
                        }
                    }

                    yield InferenceEvent::ToolCallDelta {
                        request_id: request_id.clone(),
                        tool_index: tc_delta.index,
                        id: id_for_event,
                        name: name_for_event,
                        arguments_delta: args_for_event,
                    };
                }

                // Accumulate OpenRouter structured reasoning detail blocks.
                // These are echoed back verbatim on the next turn; we collect
                // them as raw JSON values to tolerate shape drift across
                // providers.
                if !delta.reasoning_details.is_empty() {
                    chunk_has_content = true;
                    reasoning_details_acc.extend(delta.reasoning_details);
                }
            }

            if chunk_has_content {
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

        // ── Build the final response ─────────────────────────────────
        let mut tool_calls: Vec<ToolCall> = tool_call_acc
            .into_values()
            .map(|(id, name, arguments)| ToolCall {
                id: std::sync::Arc::<str>::from(id),
                name,
                arguments: std::sync::Arc::<str>::from(arguments),
            })
            .collect();

        // Some tool-integrated-reasoning models (for example GLM-family
        // models served through OpenRouter) occasionally leak their native
        // XML tool-call markup into the assistant prose instead of the
        // structured `tool_calls` deltas. Recover those calls from the prose
        // only when the structured channel produced nothing at all, so
        // well-behaved responses are never touched.
        if tool_calls.is_empty() {
            let (prose, recovered) = crate::stream::tir_fallback::recover_tool_calls(&content_acc);
            content_acc = prose;
            tool_calls = recovered;
        }

        // Honor tool calls by overriding the stop reason if the model
        // forgot to set it (mirrors the shell's behavior).
        if !tool_calls.is_empty() {
            finish_reason = Some(StopReason::ToolCalls);
        }

        // Build the trailing Assistant + any reasoning sibling.
        let mut items: Vec<ConversationItem> = Vec::new();
        if first_choice_seen {
            if !reasoning_acc.is_empty() {
                items.push(ConversationItem::Reasoning(
                    xai_grok_inference_types::synthesized_reasoning_item(reasoning_acc),
                ));
            }
            items.push(ConversationItem::Assistant(AssistantItem {
                content: std::sync::Arc::<str>::from(content_acc),
                tool_calls,
                model_id: Some(model.clone()),
                model_fingerprint,
                // Chat Completions does not echo the applied reasoning effort.
                reasoning_effort: None,
                // OpenRouter structured reasoning detail blocks, echoed back
                // verbatim on the next turn for multi-turn reasoning fidelity.
                reasoning_details: reasoning_details_acc,
            provider_payload: None,
            }));
        } else {
            items.push(ConversationItem::assistant(""));
        }

        let stream_end = Instant::now();
        let metrics =
            InferenceLatencyStats::from_timestamps(stream_start, &chunk_timestamps, stream_end);

        // Detect an OpenRouter fallback: when the provider is OpenRouter and
        // the model the server actually served (`model`, captured from the
        // first chunk) differs from the model the session requested, OpenRouter
        // silently substituted a fallback from `openrouter_fallback_models`.
        // Other providers never produce this signal — `provider_identity` gates
        // it so a transient model-id mismatch on a first-party or custom
        // endpoint is never misreported as a fallback.
        let fallback_served_model = if provider_identity.is_openrouter()
            && first_chunk_seen
            && let Some(requested) = requested_model
            && !requested.is_empty()
            && model != requested
        {
            Some(model.clone())
        } else {
            None
        };
        if fallback_served_model.is_some() {
            tracing::info!(
                requested_model = requested_model.unwrap_or(""),
                served_model = %model,
                provider = "OpenRouter",
                "openrouter fallback served: model differs from requested"
            );
        }

        let response = ConversationResponse {
            items,
            stop_reason: finish_reason,
            usage,
            cost_usd_ticks,
            message_chunks_emitted: message_chunk_count,
            doom_loop_signals: Vec::new(),
            stop_message: None,
            fallback_served_model,
        };

        yield InferenceEvent::Completed {
            request_id: request_id.clone(),
            response: Box::new(response),
            metrics,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::stream;
    use std::pin::pin;
    use xai_grok_inference_types::{
        ChatChunkChoice, ChatChunkDelta, FinishReason, Role, ToolCallDelta as ChunkToolCallDelta,
        ToolCallFunctionDelta, Usage, rs,
    };

    fn rid() -> RequestId {
        RequestId::from("test-req")
    }

    fn make_chunk(deltas: Vec<ChatChunkDelta>) -> ChatCompletionChunk {
        ChatCompletionChunk {
            id: "chunk-1".into(),
            object: "chat.completion.chunk".into(),
            created: 0,
            model: "test-model".into(),
            choices: deltas
                .into_iter()
                .enumerate()
                .map(|(i, delta)| ChatChunkChoice {
                    index: i as u32,
                    delta,
                    finish_reason: None,
                })
                .collect(),
            usage: None,
            system_fingerprint: None,
        }
    }

    fn text_chunk(text: &str) -> ChatCompletionChunk {
        make_chunk(vec![ChatChunkDelta {
            role: Some(Role::Assistant),
            content: Some(text.to_string()),
            reasoning_content: None,
            tool_calls: vec![],
            tool_call_id: None,
            reasoning_details: Vec::new(),
        }])
    }

    fn final_chunk(reason: FinishReason) -> ChatCompletionChunk {
        let mut chunk = make_chunk(vec![ChatChunkDelta::default()]);
        chunk.choices[0].finish_reason = Some(reason);
        chunk
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
    async fn empty_stream_yields_started_then_completed() {
        let raw = stream::iter(Vec::<Result<ChatCompletionChunk, InferenceError>>::new()).boxed();
        let events = collect(stream_chat_completions(
            raw,
            None,
            rid(),
            Duration::from_secs(60),
            None,
            crate::config::ProviderIdentity::Custom,
        ))
        .await;

        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], InferenceEvent::StreamStarted { .. }));
        match &events[1] {
            InferenceEvent::Completed { response, .. } => {
                assert!(response.is_empty());
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn text_only_stream_emits_first_token_then_channel_tokens_then_completed() {
        let chunks: Vec<Result<ChatCompletionChunk, InferenceError>> = vec![
            Ok(text_chunk("Hello, ")),
            Ok(text_chunk("world!")),
            Ok(final_chunk(FinishReason::Stop)),
        ];
        let raw = stream::iter(chunks).boxed();
        let events = collect(stream_chat_completions(
            raw,
            None,
            rid(),
            Duration::from_secs(60),
            None,
            crate::config::ProviderIdentity::Custom,
        ))
        .await;

        // Expected sequence: StreamStarted, FirstToken, ChannelToken(Text)
        // x 2, Completed.
        assert!(matches!(events[0], InferenceEvent::StreamStarted { .. }));
        assert!(matches!(events[1], InferenceEvent::FirstToken { .. }));

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
        assert_eq!(text_tokens, vec!["Hello, ", "world!"]);

        match events.last().unwrap() {
            InferenceEvent::Completed { response, .. } => {
                let a = response.assistant().expect("assistant item present");
                assert_eq!(a.content.as_ref(), "Hello, world!");
                assert_eq!(response.stop_reason, Some(StopReason::Stop));
                assert_eq!(response.message_chunks_emitted, 2);
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn reasoning_chunk_emits_reasoning_channel_and_first_token_once() {
        let mut reasoning_chunk = make_chunk(vec![ChatChunkDelta {
            role: Some(Role::Assistant),
            content: None,
            reasoning_content: Some("thinking...".into()),
            tool_calls: vec![],
            tool_call_id: None,
            reasoning_details: Vec::new(),
        }]);
        reasoning_chunk.choices[0].finish_reason = None;

        let chunks: Vec<Result<ChatCompletionChunk, InferenceError>> = vec![
            Ok(reasoning_chunk),
            Ok(text_chunk("done")),
            Ok(final_chunk(FinishReason::Stop)),
        ];
        let raw = stream::iter(chunks).boxed();
        let events = collect(stream_chat_completions(
            raw,
            None,
            rid(),
            Duration::from_secs(60),
            None,
            crate::config::ProviderIdentity::Custom,
        ))
        .await;

        // FirstToken should appear exactly once.
        let first_token_count = events
            .iter()
            .filter(|e| matches!(e, InferenceEvent::FirstToken { .. }))
            .count();
        assert_eq!(first_token_count, 1);

        let mut saw_reasoning = false;
        let mut saw_text = false;
        for e in &events {
            if let InferenceEvent::ChannelToken { channel, text, .. } = e {
                match channel {
                    InferenceChannel::Reasoning => {
                        assert_eq!(text, "thinking...");
                        saw_reasoning = true;
                    }
                    InferenceChannel::Text => {
                        assert_eq!(text, "done");
                        saw_text = true;
                    }
                }
            }
        }
        assert!(saw_reasoning && saw_text);

        match events.last().unwrap() {
            InferenceEvent::Completed { response, .. } => {
                let r = response
                    .reasoning_items()
                    .next()
                    .expect("reasoning sibling preserved");
                let rs::SummaryPart::SummaryText(t) = &r.summary[0];
                assert_eq!(t.text, "thinking...");
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn tool_call_stream_emits_deltas_and_assembles_final_call() {
        // First chunk has id + name + part of arguments.
        let chunk1 = make_chunk(vec![ChatChunkDelta {
            role: None,
            content: None,
            reasoning_content: None,
            tool_calls: vec![ChunkToolCallDelta {
                index: 0,
                id: Some("call_abc".into()),
                kind: Some("function".into()),
                function: Some(ToolCallFunctionDelta {
                    name: Some("do_thing".into()),
                    arguments: Some("{\"x\":".into()),
                }),
            }],
            tool_call_id: None,
            reasoning_details: Vec::new(),
        }]);
        // Second chunk has only argument fragment.
        let chunk2 = make_chunk(vec![ChatChunkDelta {
            role: None,
            content: None,
            reasoning_content: None,
            tool_calls: vec![ChunkToolCallDelta {
                index: 0,
                id: None,
                kind: None,
                function: Some(ToolCallFunctionDelta {
                    name: None,
                    arguments: Some("1}".into()),
                }),
            }],
            tool_call_id: None,
            reasoning_details: Vec::new(),
        }]);

        let raw = stream::iter::<Vec<Result<ChatCompletionChunk, InferenceError>>>(vec![
            Ok(chunk1),
            Ok(chunk2),
        ])
        .boxed();
        let events = collect(stream_chat_completions(
            raw,
            None,
            rid(),
            Duration::from_secs(60),
            None,
            crate::config::ProviderIdentity::Custom,
        ))
        .await;

        let deltas: Vec<_> = events
            .iter()
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
            .collect();

        assert_eq!(deltas.len(), 2);
        assert_eq!(deltas[0].0, 0);
        assert_eq!(deltas[0].1.as_deref(), Some("call_abc"));
        assert_eq!(deltas[0].2.as_deref(), Some("do_thing"));
        assert_eq!(deltas[0].3.as_deref(), Some("{\"x\":"));
        assert_eq!(deltas[1].1, None);
        assert_eq!(deltas[1].2, None);
        assert_eq!(deltas[1].3.as_deref(), Some("1}"));

        match events.last().unwrap() {
            InferenceEvent::Completed { response, .. } => {
                let calls = response.tool_calls();
                assert_eq!(calls.len(), 1);
                assert_eq!(calls[0].id.as_ref(), "call_abc");
                assert_eq!(calls[0].name, "do_thing");
                assert_eq!(calls[0].arguments.as_ref(), "{\"x\":1}");
                // Tool calls force ToolCalls stop reason.
                assert_eq!(response.stop_reason, Some(StopReason::ToolCalls));
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn mid_stream_error_yields_failed_no_completed() {
        let chunks: Vec<Result<ChatCompletionChunk, InferenceError>> = vec![
            Ok(text_chunk("hi")),
            Err(InferenceError::EventStreamError("conn reset".into())),
        ];
        let raw = stream::iter(chunks).boxed();
        let events = collect(stream_chat_completions(
            raw,
            None,
            rid(),
            Duration::from_secs(60),
            None,
            crate::config::ProviderIdentity::Custom,
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
        // A stream that yields one chunk then hangs forever.
        let raw = stream::iter(vec![Ok(text_chunk("hello"))])
            .chain(stream::pending())
            .boxed();
        let events = collect(stream_chat_completions(
            raw,
            None,
            rid(),
            Duration::from_millis(100),
            None,
            crate::config::ProviderIdentity::Custom,
        ))
        .await;

        // Stream should emit StreamStarted, FirstToken, ChannelToken
        // then Failed(IdleTimeout) when the stall hits the deadline.
        match events.last().unwrap() {
            InferenceEvent::Failed { error, .. } => {
                assert_eq!(error.kind, crate::events::InferenceErrorKind::IdleTimeout);
            }
            other => panic!("expected Failed(IdleTimeout), got {other:?}"),
        }
    }

    #[tokio::test]
    async fn model_metadata_yielded_after_stream_started() {
        let raw = stream::iter(Vec::<Result<ChatCompletionChunk, InferenceError>>::new()).boxed();
        let metadata = ResponseModelMetadata {
            context_window: Some(8192),
            max_completion_tokens: Some(4096),
            models_etag: None,
        };
        let events = collect(stream_chat_completions(
            raw,
            Some(metadata.clone()),
            rid(),
            Duration::from_secs(60),
            None,
            crate::config::ProviderIdentity::Custom,
        ))
        .await;

        assert!(matches!(events[0], InferenceEvent::StreamStarted { .. }));
        match &events[1] {
            InferenceEvent::ModelMetadata { metadata: m, .. } => {
                assert_eq!(m.context_window, Some(8192));
                assert_eq!(m.max_completion_tokens, Some(4096));
            }
            other => panic!("expected ModelMetadata second, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn usage_is_extracted_from_chunk() {
        let mut chunk_with_usage = make_chunk(vec![ChatChunkDelta::default()]);
        chunk_with_usage.usage = Some(Usage {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
            prompt_tokens_details: None,
            completion_tokens_details: None,
            cost_in_usd_ticks: None,
            cost: None,
            cost_details: None,
            is_byok: None,
        });

        let chunks: Vec<Result<ChatCompletionChunk, InferenceError>> = vec![
            Ok(text_chunk("ok")),
            Ok(chunk_with_usage),
            Ok(final_chunk(FinishReason::Stop)),
        ];
        let raw = stream::iter(chunks).boxed();
        let events = collect(stream_chat_completions(
            raw,
            None,
            rid(),
            Duration::from_secs(60),
            None,
            crate::config::ProviderIdentity::Custom,
        ))
        .await;

        match events.last().unwrap() {
            InferenceEvent::Completed { response, .. } => {
                let u = response.usage.as_ref().expect("usage extracted");
                assert_eq!(u.prompt_tokens, 100);
                assert_eq!(u.completion_tokens, 50);
                assert_eq!(u.total_tokens, 150);
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    /// Server-reported cost lands on the response; the REST mapper's `0`
    /// backfill means "unreported" and must yield `None`.
    #[tokio::test]
    async fn cost_is_extracted_and_zero_is_unreported() {
        for (wire, expected) in [(Some(78), Some(78)), (Some(0), None), (None, None)] {
            let mut chunk_with_usage = make_chunk(vec![ChatChunkDelta::default()]);
            chunk_with_usage.usage = Some(Usage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
                prompt_tokens_details: None,
                completion_tokens_details: None,
                cost_in_usd_ticks: wire,
                cost: None,
                cost_details: None,
                is_byok: None,
            });
            let chunks: Vec<Result<ChatCompletionChunk, InferenceError>> = vec![
                Ok(text_chunk("ok")),
                Ok(chunk_with_usage),
                Ok(final_chunk(FinishReason::Stop)),
            ];
            let raw = stream::iter(chunks).boxed();
            let events = collect(stream_chat_completions(
                raw,
                None,
                rid(),
                Duration::from_secs(60),
                None,
                crate::config::ProviderIdentity::Custom,
            ))
            .await;
            match events.last().unwrap() {
                InferenceEvent::Completed { response, .. } => {
                    assert_eq!(response.cost_usd_ticks, expected, "wire {wire:?}");
                }
                other => panic!("expected Completed, got {other:?}"),
            }
        }
    }

    #[tokio::test]
    async fn later_missing_cost_does_not_clobber_earlier_ticks() {
        let mut first = make_chunk(vec![ChatChunkDelta::default()]);
        first.usage = Some(Usage {
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
            prompt_tokens_details: None,
            completion_tokens_details: None,
            cost_in_usd_ticks: Some(99),
            cost: None,
            cost_details: None,
            is_byok: None,
        });
        let mut second = make_chunk(vec![ChatChunkDelta::default()]);
        second.usage = Some(Usage {
            prompt_tokens: 12,
            completion_tokens: 6,
            total_tokens: 18,
            prompt_tokens_details: None,
            completion_tokens_details: None,
            cost_in_usd_ticks: Some(0),
            cost: None,
            cost_details: None,
            is_byok: None,
        });
        let chunks: Vec<Result<ChatCompletionChunk, InferenceError>> = vec![
            Ok(text_chunk("ok")),
            Ok(first),
            Ok(second),
            Ok(final_chunk(FinishReason::Stop)),
        ];
        let raw = stream::iter(chunks).boxed();
        let events = collect(stream_chat_completions(
            raw,
            None,
            rid(),
            Duration::from_secs(60),
            None,
            crate::config::ProviderIdentity::Custom,
        ))
        .await;
        match events.last().unwrap() {
            InferenceEvent::Completed { response, .. } => {
                assert_eq!(response.cost_usd_ticks, Some(99));
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    /// OpenRouter reports `usage.cost` (USD float) instead of `cost_in_usd_ticks`.
    /// When ticks are absent, the collector converts the USD float to the same
    /// tick scale (1 USD = 1e10 ticks) so cost display/telemetry work uniformly.
    #[tokio::test]
    async fn openrouter_cost_float_converted_to_ticks() {
        // $0.001 → 10_000_000 ticks
        let mut chunk_with_usage = make_chunk(vec![ChatChunkDelta::default()]);
        chunk_with_usage.usage = Some(Usage {
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
            prompt_tokens_details: None,
            completion_tokens_details: None,
            cost_in_usd_ticks: None,
            cost: Some(0.001),
            cost_details: None,
            is_byok: None,
        });
        let chunks: Vec<Result<ChatCompletionChunk, InferenceError>> = vec![
            Ok(text_chunk("ok")),
            Ok(chunk_with_usage),
            Ok(final_chunk(FinishReason::Stop)),
        ];
        let raw = stream::iter(chunks).boxed();
        let events = collect(stream_chat_completions(
            raw,
            None,
            rid(),
            Duration::from_secs(60),
            None,
            crate::config::ProviderIdentity::OpenRouter,
        ))
        .await;
        match events.last().unwrap() {
            InferenceEvent::Completed { response, .. } => {
                assert_eq!(response.cost_usd_ticks, Some(10_000_000));
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    /// When both `cost_in_usd_ticks` (xAI) and `cost` (OpenRouter) are present,
    /// the ticks value takes precedence (it's the provider-native field).
    #[tokio::test]
    async fn ticks_take_precedence_over_cost_float() {
        let mut chunk_with_usage = make_chunk(vec![ChatChunkDelta::default()]);
        chunk_with_usage.usage = Some(Usage {
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
            prompt_tokens_details: None,
            completion_tokens_details: None,
            cost_in_usd_ticks: Some(42),
            cost: Some(999.0),
            cost_details: None,
            is_byok: None,
        });
        let chunks: Vec<Result<ChatCompletionChunk, InferenceError>> = vec![
            Ok(text_chunk("ok")),
            Ok(chunk_with_usage),
            Ok(final_chunk(FinishReason::Stop)),
        ];
        let raw = stream::iter(chunks).boxed();
        let events = collect(stream_chat_completions(
            raw,
            None,
            rid(),
            Duration::from_secs(60),
            None,
            crate::config::ProviderIdentity::Custom,
        ))
        .await;
        match events.last().unwrap() {
            InferenceEvent::Completed { response, .. } => {
                assert_eq!(response.cost_usd_ticks, Some(42));
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    /// Helper: chunks whose `model` field names a fallback model.
    fn fallback_chunk(text: &str, served: &str) -> ChatCompletionChunk {
        let mut chunk = make_chunk(vec![ChatChunkDelta {
            role: Some(Role::Assistant),
            content: Some(text.to_string()),
            reasoning_content: None,
            tool_calls: vec![],
            tool_call_id: None,
            reasoning_details: Vec::new(),
        }]);
        chunk.model = served.to_string();
        chunk
    }

    /// OpenRouter + served model differs from requested → the completion
    /// carries `fallback_served_model` naming the served model.
    #[tokio::test]
    async fn openrouter_fallback_detected_when_served_model_differs() {
        let chunks: Vec<Result<ChatCompletionChunk, InferenceError>> = vec![
            Ok(fallback_chunk("hi", "openai/gpt-5-mini")),
            Ok(final_chunk(FinishReason::Stop)),
        ];
        let raw = stream::iter(chunks).boxed();
        let events = collect(stream_chat_completions(
            raw,
            None,
            rid(),
            Duration::from_secs(60),
            Some("anthropic/claude-opus-4"),
            ProviderIdentity::OpenRouter,
        ))
        .await;

        match events.last().unwrap() {
            InferenceEvent::Completed { response, .. } => {
                assert_eq!(
                    response.fallback_served_model.as_deref(),
                    Some("openai/gpt-5-mini"),
                );
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    /// OpenRouter + served model matches requested → no fallback signal.
    #[tokio::test]
    async fn openrouter_matched_model_produces_no_fallback_signal() {
        let chunks: Vec<Result<ChatCompletionChunk, InferenceError>> =
            vec![Ok(text_chunk("hi")), Ok(final_chunk(FinishReason::Stop))];
        let raw = stream::iter(chunks).boxed();
        let events = collect(stream_chat_completions(
            raw,
            None,
            rid(),
            Duration::from_secs(60),
            Some("test-model"),
            ProviderIdentity::OpenRouter,
        ))
        .await;

        match events.last().unwrap() {
            InferenceEvent::Completed { response, .. } => {
                assert!(
                    response.fallback_served_model.is_none(),
                    "matched model must not produce a fallback signal",
                );
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    /// Non-OpenRouter provider with a mismatched served model → no signal,
    /// even though the served model differs from the requested one.
    #[tokio::test]
    async fn non_openrouter_mismatch_never_produces_fallback_signal() {
        for identity in [
            ProviderIdentity::Xai,
            ProviderIdentity::OpenAi,
            ProviderIdentity::Custom,
        ] {
            let chunks: Vec<Result<ChatCompletionChunk, InferenceError>> = vec![
                Ok(fallback_chunk("hi", "other-model")),
                Ok(final_chunk(FinishReason::Stop)),
            ];
            let raw = stream::iter(chunks).boxed();
            let events = collect(stream_chat_completions(
                raw,
                None,
                rid(),
                Duration::from_secs(60),
                Some("test-model"),
                identity,
            ))
            .await;
            match events.last().unwrap() {
                InferenceEvent::Completed { response, .. } => {
                    assert!(
                        response.fallback_served_model.is_none(),
                        "{identity:?} must never produce a fallback signal",
                    );
                }
                other => panic!("expected Completed, got {other:?}"),
            }
        }
    }

    /// OpenRouter but no requested model supplied (`None`) → no signal,
    /// because there is no requested model to compare against.
    #[tokio::test]
    async fn openrouter_without_requested_model_produces_no_signal() {
        let chunks: Vec<Result<ChatCompletionChunk, InferenceError>> = vec![
            Ok(fallback_chunk("hi", "other-model")),
            Ok(final_chunk(FinishReason::Stop)),
        ];
        let raw = stream::iter(chunks).boxed();
        let events = collect(stream_chat_completions(
            raw,
            None,
            rid(),
            Duration::from_secs(60),
            None,
            ProviderIdentity::OpenRouter,
        ))
        .await;

        match events.last().unwrap() {
            InferenceEvent::Completed { response, .. } => {
                assert!(response.fallback_served_model.is_none());
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    /// OpenRouter returns `reasoning_details` (structured blocks) on streamed
    /// assistant deltas. The stream transform must accumulate them across
    /// chunks and store them verbatim on the trailing AssistantItem so they
    /// can be echoed back on the next turn.
    #[tokio::test]
    async fn reasoning_details_accumulated_and_stored_on_assistant_item() {
        let detail1 = serde_json::json!({"type": "reasoning.text", "text": "step 1"});
        let detail2 = serde_json::json!({"type": "reasoning.summary", "text": "summary"});
        let chunk1 = make_chunk(vec![ChatChunkDelta {
            role: Some(Role::Assistant),
            content: None,
            reasoning_content: None,
            tool_calls: vec![],
            tool_call_id: None,
            reasoning_details: vec![detail1.clone()],
        }]);
        let chunk2 = make_chunk(vec![ChatChunkDelta {
            role: Some(Role::Assistant),
            content: Some("answer".into()),
            reasoning_content: None,
            tool_calls: vec![],
            tool_call_id: None,
            reasoning_details: vec![detail2.clone()],
        }]);
        let raw = stream::iter::<Vec<Result<ChatCompletionChunk, InferenceError>>>(vec![
            Ok(chunk1),
            Ok(chunk2),
            Ok(final_chunk(FinishReason::Stop)),
        ])
        .boxed();
        let events = collect(stream_chat_completions(
            raw,
            None,
            rid(),
            Duration::from_secs(60),
            None,
            ProviderIdentity::OpenRouter,
        ))
        .await;

        match events.last().unwrap() {
            InferenceEvent::Completed { response, .. } => {
                let a = response.assistant().expect("assistant item present");
                assert_eq!(a.content.as_ref(), "answer");
                assert_eq!(a.reasoning_details.len(), 2);
                assert_eq!(a.reasoning_details[0], detail1);
                assert_eq!(a.reasoning_details[1], detail2);
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    /// Details captured on turn N must echo back unchanged when the
    /// conversation is converted to wire messages for turn N+1.
    #[tokio::test]
    async fn reasoning_details_echo_back_verbatim_via_conversation_to_chat_messages() {
        let detail = serde_json::json!({"type": "reasoning.encrypted", "data": "abc123"});
        let chunk1 = make_chunk(vec![ChatChunkDelta {
            role: Some(Role::Assistant),
            content: Some("hello".into()),
            reasoning_content: None,
            tool_calls: vec![],
            tool_call_id: None,
            reasoning_details: vec![detail.clone()],
        }]);
        let raw = stream::iter::<Vec<Result<ChatCompletionChunk, InferenceError>>>(vec![
            Ok(chunk1),
            Ok(final_chunk(FinishReason::Stop)),
        ])
        .boxed();
        let events = collect(stream_chat_completions(
            raw,
            None,
            rid(),
            Duration::from_secs(60),
            None,
            ProviderIdentity::OpenRouter,
        ))
        .await;

        let response = match events.last().unwrap() {
            InferenceEvent::Completed { response, .. } => response,
            other => panic!("expected Completed, got {other:?}"),
        };
        // Convert the response items back to wire messages (next-turn request body).
        let messages =
            xai_grok_inference_types::conversation_to_chat_messages(response.items.clone());
        let assistant_msg = messages
            .iter()
            .find(|m| m.role == Role::Assistant)
            .expect("assistant message present");
        assert_eq!(assistant_msg.reasoning_details.len(), 1);
        assert_eq!(assistant_msg.reasoning_details[0], detail);
    }

    /// Messages without `reasoning_details` must serialize exactly as before
    /// — no new `reasoning_details` key on the wire.
    #[test]
    fn assistant_item_without_reasoning_details_omits_key() {
        let item = ConversationItem::Assistant(AssistantItem {
            content: std::sync::Arc::<str>::from("hi"),
            tool_calls: Vec::new(),
            model_id: None,
            model_fingerprint: None,
            reasoning_effort: None,
            reasoning_details: Vec::new(),
            provider_payload: None,
        });
        let messages = xai_grok_inference_types::conversation_to_chat_messages(vec![item]);
        let json = serde_json::to_value(&messages[0]).unwrap();
        assert!(
            json.get("reasoning_details").is_none(),
            "an assistant item without reasoning_details must not emit the key"
        );
    }
}
