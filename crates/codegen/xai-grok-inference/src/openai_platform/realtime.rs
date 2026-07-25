//! Typed OpenAI Realtime WebSocket event envelopes.
//!
//! The pinned OpenAPI schema defines dozens of event payloads that evolve
//! additively. These envelopes preserve each full documented payload while
//! providing a closed typed discriminator for all pinned client and server
//! event kinds plus an explicit forward-compatible `Unknown` case.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Full JSON payload for a Realtime event.
pub type RealtimeEventPayload = Map<String, Value>;

macro_rules! realtime_events {
    (
        $(#[$meta:meta])*
        $name:ident {
            $($variant:ident => $wire:literal,)*
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq)]
        pub enum $name {
            $(
                $variant(RealtimeEventPayload),
            )*
            Unknown {
                event_type: String,
                payload: RealtimeEventPayload,
            },
        }

        impl $name {
            pub fn event_type(&self) -> &str {
                match self {
                    $(
                        Self::$variant(_) => $wire,
                    )*
                    Self::Unknown { event_type, .. } => event_type,
                }
            }

            pub fn payload(&self) -> &RealtimeEventPayload {
                match self {
                    $(
                        Self::$variant(payload) => payload,
                    )*
                    Self::Unknown { payload, .. } => payload,
                }
            }

            fn from_payload(mut payload: RealtimeEventPayload) -> Result<Self, String> {
                let event_type = payload
                    .remove("type")
                    .and_then(|value| value.as_str().map(str::to_owned))
                    .ok_or_else(|| "Realtime event is missing string field `type`".to_owned())?;
                Ok(match event_type.as_str() {
                    $(
                        $wire => Self::$variant(payload),
                    )*
                    _ => Self::Unknown {
                        event_type,
                        payload,
                    },
                })
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                let mut payload = self.payload().clone();
                payload.insert("type".to_owned(), Value::String(self.event_type().to_owned()));
                Value::Object(payload).serialize(serializer)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = Value::deserialize(deserializer)?;
                let Value::Object(payload) = value else {
                    return Err(serde::de::Error::custom("Realtime event must be a JSON object"));
                };
                Self::from_payload(payload).map_err(serde::de::Error::custom)
            }
        }
    };
}

realtime_events! {
    /// Client-to-server events in the pinned OpenAI Realtime schema.
    RealtimeClientEvent {
        ConversationItemCreate => "conversation.item.create",
        ConversationItemDelete => "conversation.item.delete",
        ConversationItemRetrieve => "conversation.item.retrieve",
        ConversationItemTruncate => "conversation.item.truncate",
        InputAudioBufferAppend => "input_audio_buffer.append",
        InputAudioBufferClear => "input_audio_buffer.clear",
        OutputAudioBufferClear => "output_audio_buffer.clear",
        InputAudioBufferCommit => "input_audio_buffer.commit",
        ResponseCancel => "response.cancel",
        ResponseCreate => "response.create",
        SessionUpdate => "session.update",
    }
}

realtime_events! {
    /// Server-to-client events in the pinned OpenAI Realtime schema.
    RealtimeServerEvent {
        ConversationCreated => "conversation.created",
        ConversationItemCreated => "conversation.item.created",
        ConversationItemDeleted => "conversation.item.deleted",
        ConversationItemInputAudioTranscriptionCompleted => "conversation.item.input_audio_transcription.completed",
        ConversationItemInputAudioTranscriptionDelta => "conversation.item.input_audio_transcription.delta",
        ConversationItemInputAudioTranscriptionFailed => "conversation.item.input_audio_transcription.failed",
        ConversationItemRetrieved => "conversation.item.retrieved",
        ConversationItemTruncated => "conversation.item.truncated",
        Error => "error",
        InputAudioBufferCleared => "input_audio_buffer.cleared",
        InputAudioBufferCommitted => "input_audio_buffer.committed",
        InputAudioBufferDtmfEventReceived => "input_audio_buffer.dtmf_event_received",
        InputAudioBufferSpeechStarted => "input_audio_buffer.speech_started",
        InputAudioBufferSpeechStopped => "input_audio_buffer.speech_stopped",
        RateLimitsUpdated => "rate_limits.updated",
        ResponseAudioDelta => "response.output_audio.delta",
        ResponseAudioDone => "response.output_audio.done",
        ResponseAudioTranscriptDelta => "response.output_audio_transcript.delta",
        ResponseAudioTranscriptDone => "response.output_audio_transcript.done",
        ResponseContentPartAdded => "response.content_part.added",
        ResponseContentPartDone => "response.content_part.done",
        ResponseCreated => "response.created",
        ResponseDone => "response.done",
        ResponseFunctionCallArgumentsDelta => "response.function_call_arguments.delta",
        ResponseFunctionCallArgumentsDone => "response.function_call_arguments.done",
        ResponseOutputItemAdded => "response.output_item.added",
        ResponseOutputItemDone => "response.output_item.done",
        ResponseTextDelta => "response.output_text.delta",
        ResponseTextDone => "response.output_text.done",
        SessionCreated => "session.created",
        SessionUpdated => "session.updated",
        OutputAudioBufferStarted => "output_audio_buffer.started",
        OutputAudioBufferStopped => "output_audio_buffer.stopped",
        OutputAudioBufferCleared => "output_audio_buffer.cleared",
        ConversationItemAdded => "conversation.item.added",
        ConversationItemDone => "conversation.item.done",
        InputAudioBufferTimeoutTriggered => "input_audio_buffer.timeout_triggered",
        ConversationItemInputAudioTranscriptionSegment => "conversation.item.input_audio_transcription.segment",
        McpListToolsInProgress => "mcp_list_tools.in_progress",
        McpListToolsCompleted => "mcp_list_tools.completed",
        McpListToolsFailed => "mcp_list_tools.failed",
        ResponseMcpCallArgumentsDelta => "response.mcp_call_arguments.delta",
        ResponseMcpCallArgumentsDone => "response.mcp_call_arguments.done",
        ResponseMcpCallInProgress => "response.mcp_call.in_progress",
        ResponseMcpCallCompleted => "response.mcp_call.completed",
        ResponseMcpCallFailed => "response.mcp_call.failed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_event_round_trips_with_payload() {
        let event: RealtimeServerEvent = serde_json::from_value(serde_json::json!({
            "type": "response.output_text.delta",
            "event_id": "event_1",
            "delta": "hello"
        }))
        .unwrap();
        assert!(matches!(event, RealtimeServerEvent::ResponseTextDelta(_)));
        assert_eq!(event.payload()["delta"], "hello");
        assert_eq!(serde_json::to_value(event).unwrap()["event_id"], "event_1");
    }

    #[test]
    fn additive_unknown_event_round_trips() {
        let event: RealtimeServerEvent = serde_json::from_value(serde_json::json!({
            "type": "response.future.delta",
            "delta": "new"
        }))
        .unwrap();
        assert!(matches!(event, RealtimeServerEvent::Unknown { .. }));
        let value = serde_json::to_value(event).unwrap();
        assert_eq!(value["type"], "response.future.delta");
        assert_eq!(value["delta"], "new");
    }
}
