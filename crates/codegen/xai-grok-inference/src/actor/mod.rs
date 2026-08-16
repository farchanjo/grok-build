//! Sampler actor: owns global state, spawns per-request tasks.
//!
//! The actor task itself is single-threaded -- it processes one
//! command at a time -- but it spawns `tokio::spawn` per-request
//! tasks for the actual streaming work, so multiple requests can be
//! in flight concurrently.

pub(crate) mod pacing;
pub(crate) mod request_task;
pub(crate) mod state;

use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::commands::InferenceCommand;
use crate::config::{InferenceConfig, RetryPolicy};
use crate::events::InferenceEvent;
use crate::handle::InferenceHandle;
use state::{ActiveRequest, ActorState};

use crate::types::RequestId;

/// Sampler actor.
///
/// Construct via [`InferenceActor::spawn`]; the returned
/// [`InferenceHandle`] is the only supported way to interact with it.
pub struct InferenceActor {
    cmd_rx: mpsc::UnboundedReceiver<InferenceCommand>,
    event_tx: mpsc::UnboundedSender<InferenceEvent>,
    state: ActorState,
    /// Per-request tasks. The actor's run loop selects on
    /// `cmd_rx.recv()` and `tasks.join_next()`; when a task finishes
    /// it returns its `RequestId` so the actor can clean up
    /// `active_requests`.
    tasks: JoinSet<RequestId>,
}

impl InferenceActor {
    /// Spawn the actor on the current tokio runtime and return a
    /// handle. The actor stops when the returned handle (and all its
    /// clones) are dropped.
    pub fn spawn(
        config: InferenceConfig,
        retry_policy: RetryPolicy,
        event_tx: mpsc::UnboundedSender<InferenceEvent>,
    ) -> InferenceHandle {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let actor = Self {
            cmd_rx,
            event_tx,
            state: ActorState::new(config, retry_policy),
            tasks: JoinSet::new(),
        };
        tokio::spawn(actor.run());
        InferenceHandle::new(cmd_tx)
    }

    async fn run(mut self) {
        loop {
            tokio::select! {
                biased;
                // Prefer cleaning up finished tasks before processing
                // new commands -- prevents `active_requests` from
                // staying stale longer than necessary.
                Some(joined) = self.tasks.join_next(), if !self.tasks.is_empty() => {
                    match joined {
                        Ok(request_id) => {
                            // Task finished normally; remove from
                            // active set unless the user has already
                            // cancelled it (Cancel removes it too).
                            self.state.remove(&request_id);
                        }
                        Err(join_err) => {
                            tracing::warn!(
                                error = %join_err,
                                "request task panicked or was aborted"
                            );
                        }
                    }
                }
                cmd = self.cmd_rx.recv() => {
                    match cmd {
                        Some(cmd) => self.handle_command(cmd),
                        None => break, // all handles dropped
                    }
                }
            }
        }

        // Cancel any still-running tasks before exiting so they don't
        // leak. The cancellation token shutdown is best-effort.
        for (_, active) in self.state.active_requests.drain() {
            active.cancel_token.cancel();
        }
        self.tasks.shutdown().await;
    }

    fn handle_command(&mut self, cmd: InferenceCommand) {
        match cmd {
            InferenceCommand::Submit {
                request_id,
                request,
                config,
                completion_tx,
            } => {
                let cancel_token = CancellationToken::new();
                let active = ActiveRequest {
                    cancel_token: cancel_token.clone(),
                };
                if let Some(prev) = self.state.register(request_id.clone(), active) {
                    // Caller submitted a duplicate id; cancel the
                    // previous one so we don't leak its task.
                    prev.cancel_token.cancel();
                }
                let effective_config = config
                    .map(|b| *b)
                    .unwrap_or_else(|| self.state.config.clone());
                let event_tx = self.event_tx.clone();
                let retry_policy = self.state.retry_policy.clone();
                let inference_pacer = self.state.inference_pacer.clone();
                let request_inner = *request;
                self.tasks.spawn(request_task::run_request_task(
                    request_id,
                    request_inner,
                    effective_config,
                    retry_policy,
                    event_tx,
                    cancel_token,
                    completion_tx,
                    inference_pacer,
                ));
            }
            InferenceCommand::Cancel { request_id } => {
                self.state.cancel(&request_id);
            }
            InferenceCommand::UpdateConfig { config, route } => {
                self.state.apply_config_update(*config, route);
            }
            InferenceCommand::IsActive { request_id, reply } => {
                let _ = reply.send(self.state.active_requests.contains_key(&request_id));
            }
            InferenceCommand::ActiveCount { reply } => {
                let _ = reply.send(self.state.active_requests.len());
            }
        }
    }
}
