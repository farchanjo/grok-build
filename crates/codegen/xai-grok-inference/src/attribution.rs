//! 401 attribution callback hook for the inference client.
//!
//! Every 401 response site can optionally emit an attribution event so a
//! downstream observer can compare the bearer actually sent with its live
//! credential source. The full credential never crosses this crate boundary.

use std::sync::Arc;

pub use xai_grok_inference_types::bearer_fragment::BEARER_TAIL_CHARS;

/// A logical 401-emitting site inside the inference client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InferenceConsumer {
    /// OpenAI-compatible streaming Chat Completions API.
    ChatCompletionsStream,
    /// OpenAI-compatible non-streaming Chat Completions API.
    ChatCompletions,
    /// Responses API streaming.
    ResponsesStream,
    /// Responses API non-streaming.
    Responses,
    /// Anthropic Messages API streaming.
    MessagesStream,
    /// Anthropic Messages API non-streaming.
    Messages,
}

impl InferenceConsumer {
    /// Stable string identifier for this emit site.
    pub fn as_endpoint(self) -> &'static str {
        match self {
            Self::ChatCompletionsStream => "chat_completions_stream",
            Self::ChatCompletions => "chat_completions",
            Self::ResponsesStream => "responses_stream",
            Self::Responses => "responses",
            Self::MessagesStream => "messages_stream",
            Self::Messages => "messages",
        }
    }
}

/// Hook invoked by [`crate::InferenceClient`] at every 401 response site.
///
/// Implementations must be cheap and non-blocking because they run on the
/// user-visible error path. The `Debug` bound lets [`crate::InferenceConfig`]
/// retain its redacted `Debug` implementation.
pub trait Auth401AttributionCallback: Send + Sync + std::fmt::Debug {
    /// Record a 401 with the final [`BEARER_TAIL_CHARS`] characters of the
    /// bearer actually sent on the wire. `None` means no credential header was
    /// sent; it must not be interpreted as ownership of a generic 401.
    fn record_401(&self, consumer: InferenceConsumer, sent_bearer_tail: Option<&str>);
}

/// Shared, cheap-to-clone alias for the attribution callback.
pub type SharedAttributionCallback = Arc<dyn Auth401AttributionCallback>;
