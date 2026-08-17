//! Buffered consumer for [`InferenceEvent`] streams.
//!
//! Drains a Layer-2 event stream into the final
//! `(ConversationResponse, InferenceLatencyStats)` pair. Used by
//! callers that don't need streaming UI updates (e.g., compaction,
//! `/btw`, dream-model calls).

use futures_util::StreamExt;
use futures_util::stream::Stream;

use xai_grok_inference_types::ConversationResponse;

use crate::events::{InferenceErrorInfo, InferenceErrorKind, InferenceEvent};
use crate::metrics::InferenceLatencyStats;

/// Drain a [`InferenceEvent`] stream, returning the final response.
///
/// Returns `Ok((response, metrics))` on the first
/// [`InferenceEvent::Completed`] and `Err(error)` on the first
/// [`InferenceEvent::Failed`]. Intermediate events (deltas, retries,
/// metadata) are silently consumed -- this function is for callers
/// that only need the final result.
///
/// If the stream ends without yielding either terminal event,
/// returns an `Err` of kind [`InferenceErrorKind::Api`] indicating
/// truncation. The Layer-2 transforms guarantee a terminal event in
/// every successful return path, so this only fires for streams that
/// are dropped mid-flight (e.g., the producer panicked or the
/// underlying `tokio::spawn` was cancelled).
pub async fn collect_response(
    stream: impl Stream<Item = InferenceEvent>,
) -> Result<(ConversationResponse, InferenceLatencyStats), InferenceErrorInfo> {
    tokio::pin!(stream);

    while let Some(event) = stream.next().await {
        match event {
            InferenceEvent::Completed {
                response, metrics, ..
            } => return Ok((*response, metrics)),
            InferenceEvent::Failed { error, .. } => return Err(error),
            // Drop intermediate events; this is a buffered collector.
            _ => {}
        }
    }

    Err(InferenceErrorInfo {
        kind: InferenceErrorKind::Api,
        status_code: None,
        message: "stream ended without Completed or Failed".to_string(),
        is_retryable: false,
        retry_after_secs: None,
        model_metadata: None,
        diagnostics: None,
        error_code: None,
        empty_response_context: None,
        doom_loop_triggers: None,
        doom_loop_aborted_at_chunk: None,
        credential: xai_grok_inference_types::SentCredential::Unknown,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::stream;
    use xai_grok_inference_types::{ConversationItem, InferenceError, StopReason};

    use crate::events::InferenceChannel;
    use crate::stream::stream_chat_completions;
    use crate::types::RequestId;
    use std::time::Duration;
    use xai_grok_inference_types::{
        ChatChunkChoice, ChatChunkDelta, ChatCompletionChunk, FinishReason, Role,
    };

    fn rid() -> RequestId {
        RequestId::from("collect-test")
    }

    fn text_chunk(text: &str) -> ChatCompletionChunk {
        ChatCompletionChunk {
            id: "chunk".into(),
            object: "chat.completion.chunk".into(),
            created: 0,
            model: "test-model".into(),
            choices: vec![ChatChunkChoice {
                index: 0,
                delta: ChatChunkDelta {
                    role: Some(Role::Assistant),
                    content: Some(text.to_string()),
                    reasoning_content: None,
                    tool_calls: vec![],
                    tool_call_id: None,
                    reasoning_details: Vec::new(),
                },
                finish_reason: None,
            }],
            usage: None,
            system_fingerprint: None,
        }
    }

    fn final_chunk() -> ChatCompletionChunk {
        ChatCompletionChunk {
            id: "chunk".into(),
            object: "chat.completion.chunk".into(),
            created: 0,
            model: "test-model".into(),
            choices: vec![ChatChunkChoice {
                index: 0,
                delta: ChatChunkDelta::default(),
                finish_reason: Some(FinishReason::Stop),
            }],
            usage: None,
            system_fingerprint: None,
        }
    }

    #[tokio::test]
    async fn happy_path_returns_response_and_metrics() {
        let chunks: Vec<Result<ChatCompletionChunk, InferenceError>> =
            vec![Ok(text_chunk("hello")), Ok(final_chunk())];
        let raw = stream::iter(chunks).boxed();
        let events = stream_chat_completions(
            raw,
            None,
            rid(),
            Duration::from_secs(60),
            None,
            crate::config::ProviderIdentity::Custom,
        );

        let (response, _metrics) = collect_response(events)
            .await
            .expect("happy path returns Ok");
        let a = response.assistant().expect("assistant item present");
        assert_eq!(a.content.as_ref(), "hello");
        assert_eq!(response.stop_reason, Some(StopReason::Stop));
    }

    #[tokio::test]
    async fn failure_path_returns_error() {
        let chunks: Vec<Result<ChatCompletionChunk, InferenceError>> = vec![
            Ok(text_chunk("partial")),
            Err(InferenceError::EventStreamError("boom".into())),
        ];
        let raw = stream::iter(chunks).boxed();
        let events = stream_chat_completions(
            raw,
            None,
            rid(),
            Duration::from_secs(60),
            None,
            crate::config::ProviderIdentity::Custom,
        );

        let err = collect_response(events).await.expect_err("error returned");
        assert!(err.message.contains("boom"));
    }

    #[tokio::test]
    async fn truncated_stream_returns_error() {
        let truncated = stream::iter(vec![InferenceEvent::StreamStarted {
            request_id: rid(),
            timestamp_ms: 0,
        }]);
        let err = collect_response(truncated)
            .await
            .expect_err("truncated stream returns Err");
        assert_eq!(err.kind, InferenceErrorKind::Api);
        assert!(err.message.contains("stream ended without"));
    }

    #[tokio::test]
    async fn intermediate_events_are_dropped() {
        let token = InferenceEvent::ChannelToken {
            request_id: rid(),
            channel: InferenceChannel::Text,
            text: "hi".into(),
            chunk_index: 1,
        };
        let completed = InferenceEvent::Completed {
            request_id: rid(),
            response: Box::new(ConversationResponse {
                items: vec![ConversationItem::assistant("hi")],
                stop_reason: Some(StopReason::Stop),
                usage: None,
                cost_usd_ticks: None,
                message_chunks_emitted: 1,
                doom_loop_signals: Vec::new(),
                stop_message: None,
                fallback_served_model: None,
            }),
            metrics: InferenceLatencyStats::default(),
        };
        let s = stream::iter(vec![token, completed]);
        let (response, _) = collect_response(s).await.expect("ok");
        let a = response.assistant().expect("assistant item present");
        assert_eq!(a.content.as_ref(), "hi");
    }

    /// Non-streaming-style consumption (the buffered collector used by
    /// `conversation_collect`) carries the OpenRouter fallback signal
    /// through to the final `ConversationResponse`.
    #[tokio::test]
    async fn openrouter_fallback_carries_through_collect_response() {
        let mut chunk = text_chunk("hi");
        chunk.model = "openai/gpt-5-mini".into();
        let chunks: Vec<Result<ChatCompletionChunk, InferenceError>> =
            vec![Ok(chunk), Ok(final_chunk())];
        let raw = stream::iter(chunks).boxed();
        let events = stream_chat_completions(
            raw,
            None,
            rid(),
            Duration::from_secs(60),
            Some("anthropic/claude-opus-4"),
            crate::config::ProviderIdentity::OpenRouter,
        );
        let (response, _metrics) = collect_response(events)
            .await
            .expect("happy path returns Ok");
        assert_eq!(
            response.fallback_served_model.as_deref(),
            Some("openai/gpt-5-mini"),
        );
    }
}
