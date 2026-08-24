//! Internal actor protocol.
//!
//! `InferenceCommand` is `pub(crate)` because it is the wire between
//! [`InferenceHandle`](crate::handle::InferenceHandle) and the actor task,
//! not a public type. External callers always go through `InferenceHandle`.

use tokio::sync::oneshot;

use xai_grok_inference_types::{ConversationRequest, ConversationResponse, InferenceError};

use crate::config::InferenceConfig;
use crate::metrics::InferenceLatencyStats;
use crate::types::RequestId;

/// Commands sent from a [`InferenceHandle`](crate::handle::InferenceHandle)
/// to the actor task.
///
/// Large payloads (`ConversationRequest`, `InferenceConfig`) are boxed so
/// every command stays cheap to copy through the mpsc channel.
pub(crate) enum InferenceCommand {
    /// Submit a new sampling request. Fire-and-forget — results come via
    /// events. When `completion_tx` is set the per-request task also
    /// signals that channel for `submit_and_collect` callers.
    Submit {
        request_id: RequestId,
        request: Box<ConversationRequest>,
        config: Option<Box<InferenceConfig>>,
        completion_tx: Option<
            oneshot::Sender<Result<(ConversationResponse, InferenceLatencyStats), InferenceError>>,
        >,
    },

    /// Cancel an in-flight request.
    Cancel { request_id: RequestId },

    /// Update the default sampling config (model switch, auth refresh).
    UpdateConfig {
        config: Box<InferenceConfig>,
        /// Optional route-context update applied atomically with config.
        route: crate::route_context::RouteContextUpdate,
    },

    /// Query: is a specific request still in flight?
    IsActive {
        request_id: RequestId,
        reply: oneshot::Sender<bool>,
    },

    /// Query: how many requests are in flight?
    ActiveCount { reply: oneshot::Sender<usize> },
}
