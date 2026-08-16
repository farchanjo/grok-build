//! Public handle for talking to the sampler actor.

use tokio::sync::{mpsc, oneshot};

use xai_grok_inference_types::{ConversationRequest, ConversationResponse, InferenceError};

use crate::commands::InferenceCommand;
use crate::config::InferenceConfig;
use crate::metrics::InferenceLatencyStats;
use crate::types::RequestId;

/// Cheaply-cloneable handle to the sampler actor.
///
/// Internally just an `mpsc::UnboundedSender<InferenceCommand>`. All
/// methods are non-blocking (fire-and-forget) except for the
/// `*_async` queries which return a future awaiting an
/// `oneshot::Receiver`.
#[derive(Clone)]
pub struct InferenceHandle {
    cmd_tx: mpsc::UnboundedSender<InferenceCommand>,
}

impl InferenceHandle {
    /// Construct a handle from a command sender. `pub(crate)` because
    /// only [`InferenceActor::spawn`](crate::actor::InferenceActor::spawn)
    /// produces one of these.
    pub(crate) fn new(cmd_tx: mpsc::UnboundedSender<InferenceCommand>) -> Self {
        Self { cmd_tx }
    }

    /// Create a no-op handle that discards all commands.
    ///
    /// Useful for tests and callers that need a `InferenceHandle` field
    /// before the actor is wired up. Mirrors
    /// [`HunkTrackerHandle::noop`](https://docs.rs/xai-hunk-tracker).
    pub fn noop() -> Self {
        let (cmd_tx, _cmd_rx) = mpsc::unbounded_channel();
        // Receiver is dropped immediately; sends will fail but every
        // send-site uses `let _ = ...` so that is fine.
        Self { cmd_tx }
    }

    /// Submit a sampling request. Fire-and-forget -- results arrive
    /// via the shared event channel.
    pub fn submit(&self, request_id: RequestId, request: ConversationRequest) {
        let _ = self.cmd_tx.send(InferenceCommand::Submit {
            request_id,
            request: Box::new(request),
            config: None,
            completion_tx: None,
        });
    }

    /// Submit a sampling request with an explicit per-request config
    /// override (e.g., a different model than the actor's default).
    pub fn submit_with_config(
        &self,
        request_id: RequestId,
        request: ConversationRequest,
        config: InferenceConfig,
    ) {
        let _ = self.cmd_tx.send(InferenceCommand::Submit {
            request_id,
            request: Box::new(request),
            config: Some(Box::new(config)),
            completion_tx: None,
        });
    }

    /// Cancel an in-flight request. No-op if the request id is
    /// unknown (already finished or never submitted).
    pub fn cancel(&self, request_id: RequestId) {
        let _ = self.cmd_tx.send(InferenceCommand::Cancel { request_id });
    }

    /// Update the default sampling config (e.g., after model switch
    /// or auth refresh). The next request submitted without an
    /// override will use it. Clears any explicit route context.
    pub fn update_config(&self, config: InferenceConfig) {
        self.update_config_with_route_context(
            config,
            crate::route_context::RouteContextUpdate::DeriveLegacy,
        );
    }

    /// Atomically update config and route context together so a stale
    /// explicit account context cannot survive a bare config refresh.
    pub fn update_config_with_route_context(
        &self,
        config: InferenceConfig,
        route: crate::route_context::RouteContextUpdate,
    ) {
        let _ = self.cmd_tx.send(InferenceCommand::UpdateConfig {
            config: Box::new(config),
            route,
        });
    }

    /// Query whether a request is still in flight. Returns `false`
    /// for unknown / finished / cancelled ids.
    pub async fn is_active(&self, request_id: RequestId) -> bool {
        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = self.cmd_tx.send(InferenceCommand::IsActive {
            request_id,
            reply: reply_tx,
        });
        reply_rx.await.unwrap_or(false)
    }

    /// Query the number of in-flight requests. Returns 0 if the
    /// actor has been shut down.
    pub async fn active_count(&self) -> usize {
        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = self
            .cmd_tx
            .send(InferenceCommand::ActiveCount { reply: reply_tx });
        reply_rx.await.unwrap_or(0)
    }

    /// Submit a request and await its completion. Events still flow
    /// to the shared channel for live UI updates -- this method just
    /// additionally awaits the per-request completion oneshot so the
    /// caller gets a clean `Result` without filtering events.
    ///
    /// Used by sequential callers like compaction / summary /
    /// `/btw` side questions.
    pub async fn submit_and_collect(
        &self,
        request_id: RequestId,
        request: ConversationRequest,
    ) -> Result<(ConversationResponse, InferenceLatencyStats), InferenceError> {
        // RAII guard: when this future is dropped (cancel, panic, or normal return),
        // tell the sampler actor to cancel the in-flight request_id. No-op if the
        // actor already finished and removed it from its active set.
        struct CancelOnDrop {
            cmd_tx: mpsc::UnboundedSender<InferenceCommand>,
            request_id: RequestId,
        }
        impl Drop for CancelOnDrop {
            fn drop(&mut self) {
                // fire-and-forget the send.
                let _ = self.cmd_tx.send(InferenceCommand::Cancel {
                    request_id: self.request_id.clone(),
                });
            }
        }

        let (completion_tx, completion_rx) = oneshot::channel();
        let cancel_id = request_id.clone();

        // Only arm the guard if Submit actually reached the actor.
        let _guard = self
            .cmd_tx
            .send(InferenceCommand::Submit {
                request_id,
                request: Box::new(request),
                config: None,
                completion_tx: Some(completion_tx),
            })
            .ok()
            .map(|_| CancelOnDrop {
                cmd_tx: self.cmd_tx.clone(),
                request_id: cancel_id,
            });
        completion_rx.await.unwrap_or_else(|_| {
            Err(InferenceError::Auth(
                "sampler actor dropped before completion".to_string(),
            ))
        })
    }
}
