//! Layer-2 stream transforms: turn raw HTTP chunk streams into
//! [`InferenceEvent`](crate::events::InferenceEvent) streams.
//!
//! Each backend has its own transform because the raw chunk types
//! differ; backend dispatch happens in M4's
//! [`actor::request_task`](crate::actor::request_task), which knows
//! the API backend from `InferenceConfig.api_backend` and calls the
//! matching `InferenceClient::conversation_stream*` method before
//! handing the result to the corresponding transform here.
//!
//! ## Two ports + one collaborator
//!
//! Per the reviewed plan, [`ProviderAdapter`](crate::provider::ProviderAdapter)
//! (Port 1) is provider-shaped (identity + request + policy) while this
//! module owns Port 2 — the **wire codec**, which is protocol-shaped and
//! has one implementation per backend ([`ChatCompletionsCodec`],
//! [`ResponsesCodec`], [`MessagesCodec`]). Each codec owns the backend's SSE
//! semantics: distinguishing the terminal `[DONE]` sentinel from a
//! finish-delta chunk, the single typed `from_str` parse, stop-reason
//! normalization, and in-band error shapes. The
//! [`chat_completions`](chat_completions) codec additionally runs the
//! adapter's per-delta [`shape_delta`](crate::provider::ProviderAdapter::shape_delta)
//! hook and reads its [`reasoning_wire`](crate::provider::ProviderAdapter::reasoning_wire)
//! (echo/request-side only — never response decode).

use xai_grok_inference_types::InferenceError;

pub mod chat_completions;
pub mod collect;
pub mod messages;
pub mod responses;
pub mod structured_output;
pub mod tir_fallback;

pub use chat_completions::stream_chat_completions;
pub use collect::collect_response;
pub use messages::stream_messages;
pub use responses::stream_responses;
pub use structured_output::{ResponseFieldProjector, project_response_field};

pub use chat_completions::ChatCompletionsCodec;
pub use messages::MessagesCodec;
pub use responses::ResponsesCodec;

/// Port 2: protocol-shaped wire codec — one implementation per backend.
///
/// The codec is the single home for the backend's SSE semantics. It consumes
/// a raw `data:` frame and produces the typed wire chunk, so the client pump
/// never re-parses a chunk through [`serde_json::Value`]: the typed
/// [`decode_frame`](Self::decode_frame) is the one parse, and
/// [`is_terminal_data`](Self::is_terminal_data) distinguishes `[DONE]` from a
/// finish-delta chunk. Object-safe + `Sync` so a codec can be shared
/// alongside an [`Arc<dyn ProviderAdapter>`](crate::provider::ProviderAdapter).
pub trait WireCodec: Send + Sync + 'static {
    /// The typed wire frame this codec decodes.
    type Chunk: Send + 'static;

    /// Whether a raw SSE `data:` frame is the terminal `[DONE]` sentinel.
    ///
    /// Backends differ here: chat-completions and Responses terminate with a
    /// bare `[DONE]`, while the Anthropic Messages codec (owned by its own
    /// client) terminates on `message_stop`. The codec abstracts that so the
    /// pump never hard-codes `"</s>"`-style wire sentinels.
    fn is_terminal_data(&self, data: &str) -> bool {
        data == "[DONE]"
    }

    /// Decode a raw SSE `data:` frame into a typed chunk.
    ///
    /// Handles the backend's in-band error shapes (returned as [`Err`]) and
    /// returns [`Ok(None)`] for the terminal sentinel. This is the *single*
    /// typed parse for the backend — the pump must not re-parse the frame as
    /// [`serde_json::Value`].
    fn decode_frame(&self, data: &str) -> Result<Option<Self::Chunk>, InferenceError>;
}
