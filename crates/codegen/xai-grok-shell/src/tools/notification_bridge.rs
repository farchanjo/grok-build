//! Notification bridge: translates `xai-grok-tools` `ToolNotification` events
//! into `xai-grok-shell`'s native systems (ACP gateway, hunk tracker, file state tracker).
use crate::session::commands::SessionCommand;
use crate::session::commands::{NotificationPriority, NotificationSource};
use crate::session::persistence::{DurableAppendError, PersistenceHandle, PersistenceMsg};
use agent_client_protocol::{self as acp, Client as _};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex as TokioMutex, mpsc};
use xai_acp_lib::AcpAgentGatewaySender as GatewaySender;
use xai_grok_tools::notification::types::{
    AssetJobEvent, ToolNotification, ToolNotificationHandle,
};
// `TokenBucket` lives in `xai-file-utils` and is re-exported by the monitor
// module; importing through the monitor keeps this path stable whether or not
// the shared limiter module is present in a given checkout.
use xai_grok_tools::implementations::grok_build::monitor::rate_limiter::TokenBucket;
use xai_grok_tools::types::output::{BashOutput, ToolOutput};
use xai_grok_workspace::session::file_state::FileStateTracker;
use xai_hunk_tracker::HunkTrackerHandle;
const TASK_WAKE_ADMISSION_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(250);

/// How many `x.ai/asset_job_event` frames may pass before the coalescer starts
/// dropping progress frames.
const ASSET_JOB_COALESCE_CAPACITY: u32 = 10;
/// Refill interval for the asset-job coalescer's token bucket.
const ASSET_JOB_COALESCE_REFILL_MS: u64 = 500;

/// Per-session mutable state for the notification bridge.
///
/// Bundled so a new coalescer does not grow `handle_notification`'s arity on
/// every feature; the bridge owns exactly one instance per session.
#[derive(Default)]
struct BridgeState {
    /// Byte offsets per tool_call_id for incremental bash output.
    offsets: HashMap<String, usize>,
    /// Progress coalescer for asset transfer jobs.
    asset_jobs: AssetJobCoalescer,
}

/// Shell-side throttle for `x.ai/asset_job_event`.
///
/// Coalescing happens **here, before the ACP hop**, so a fast upload cannot
/// flood the channel; the pager renders only what it receives. Each job gets
/// its own [`TokenBucket`] (500 ms refill, capacity 10) — one bucket per job
/// so a burst on one transfer cannot starve another.
///
/// Terminal frames always pass and drop their bucket: a throttled completion
/// would otherwise leave the pager row spinning forever.
#[derive(Default)]
struct AssetJobCoalescer {
    buckets: HashMap<String, TokenBucket>,
}

impl AssetJobCoalescer {
    /// Whether this frame should reach the client.
    fn admit(&mut self, event: &AssetJobEvent) -> bool {
        if event.is_terminal() {
            self.forget(&event.job_id);
            return true;
        }
        let bucket = self.buckets.entry(event.job_id.clone()).or_insert_with(|| {
            TokenBucket::new(ASSET_JOB_COALESCE_CAPACITY, ASSET_JOB_COALESCE_REFILL_MS)
        });
        bucket.try_consume()
    }

    /// Forget the bucket for `job_id` (used by tests and on terminal frames
    /// that arrive out of band).
    fn forget(&mut self, job_id: &str) {
        self.buckets.remove(job_id);
    }
}

/// Outcome of the actor-side admission handshake for a synthetic
/// task-completion wake. Drives the fail-safe so a wake that never becomes a
/// turn still reaches the model through the pending-notification drain.
enum WakeAdmissionOutcome {
    /// Actor accepted; the queued prompt will run (or its dead-reply path will
    /// park the fallback as a pending notification).
    Accepted,
    /// Actor explicitly refused (gate/suppressed); it already parked the wake
    /// fallback as a pending notification.
    Refused,
    /// Actor never answered within the admission budget. The queued command
    /// may still be processed later — the actor parks the fallback when it
    /// finds the admission reply already gone — so the bridge must not
    /// double-inject.
    NoResponse,
    /// The wake prompt never reached the session actor channel. The bridge
    /// injects the completion itself as a pending notification.
    SendFailed,
}

impl WakeAdmissionOutcome {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Refused => "refused",
            Self::NoResponse => "no_response",
            Self::SendFailed => "send_failed",
        }
    }

    fn will_wake(&self) -> bool {
        matches!(self, Self::Accepted)
    }
}

/// How a completed background task is classified for wake wording and for the
/// deferred fallback prompt id.
///
/// The three kinds share the wake path, the admission gate and the suppression
/// flags; only the model-facing wording and the prompt-id prefix differ, so the
/// classification is resolved once at the top of the `TaskCompleted` arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompletedTaskKind {
    Monitor,
    /// `wait_for` watcher — reports satisfaction or deadline expiry.
    Wait,
    Bash,
}

impl CompletedTaskKind {
    fn of(snapshot: &xai_grok_tools::computer::types::TaskSnapshot) -> Self {
        use xai_grok_tools::computer::types::TaskKind;
        match snapshot.kind {
            TaskKind::Monitor => Self::Monitor,
            TaskKind::Wait => Self::Wait,
            TaskKind::Bash => Self::Bash,
        }
    }

    fn is_monitor(self) -> bool {
        matches!(self, Self::Monitor)
    }

    /// Fallback / deferred-inject prompt id, prefixed per kind so a deferred
    /// wait completion can never dedupe against a same-task-id bash one.
    fn fallback_prompt_id(self, task_id: &str) -> String {
        match self {
            Self::Monitor => format!("monitor-completed-{task_id}"),
            Self::Wait => format!("wait-completed-{task_id}"),
            Self::Bash => format!("bash-completed-{task_id}"),
        }
    }

    /// Model-facing wake body.
    fn body(
        self,
        task: &xai_grok_tools::computer::types::TaskSnapshot,
        tool_name: Option<&str>,
        read_name: Option<&str>,
    ) -> String {
        use xai_grok_tools::reminders::task_completion::{
            format_bash_completion, format_monitor_completion, format_wait_completion,
        };
        match self {
            Self::Monitor => format_monitor_completion(task, tool_name),
            Self::Wait => format_wait_completion(task, tool_name),
            Self::Bash => format_bash_completion(task, tool_name, read_name),
        }
    }
}
/// Configuration for the notification bridge.
pub struct NotificationBridgeConfig {
    /// ACP gateway for sending streaming updates to TUI
    pub gateway: GatewaySender,
    /// ACP session ID
    pub session_id: acp::SessionId,
    /// Hunk tracker for recording agent writes
    pub hunk_tracker_handle: HunkTrackerHandle,
    /// File state tracker for rewind functionality
    pub file_state_tracker: Arc<FileStateTracker>,
    /// Current prompt index (shared with session state)
    pub prompt_index: Arc<TokioMutex<usize>>,
    /// Working directory for path relativization
    pub cwd: PathBuf,
    /// Shared gate: when false, suppress gateway forwarding.
    /// Events are still processed for hunk tracking and file state.
    pub gateway_enabled: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Persistence handle for FIFO ordinary writes and durable tombstone barriers.
    pub persistence: PersistenceHandle,
    /// When true, send incremental `output_delta` instead of full `output`
    /// in bash streaming updates. The client must opt in via the
    /// `x.ai/incrementalBashOutput` capability.
    pub incremental_bash_output: bool,
    /// Plan mode tracker shared with the session actor.
    /// Used to transition state on `PlanModeEntered` / `PlanModeExited`
    /// tool notifications.
    pub plan_mode: Arc<parking_lot::Mutex<crate::session::plan_mode::PlanModeTracker>>,
    /// Session-level prompt mode shared with the session actor.
    /// Updated on `PlanModeEntered` / `PlanModeExited` and `session/set_mode`
    /// so the next turn starts in the correct mode.
    pub current_prompt_mode: Arc<parking_lot::Mutex<crate::session::plan_mode::PromptMode>>,
    /// Turn-level prompt mode. Set at turn start, then updated only by
    /// agent tool calls (`EnterPlanMode` / `ExitPlanMode`). NOT affected
    /// by `session/set_mode`. Read at turn end for `end_prompt_mode`.
    pub turn_prompt_mode: Arc<parking_lot::Mutex<crate::session::plan_mode::PromptMode>>,
    /// Session command channel for monitor events and task-completed injections.
    pub session_cmd_tx: mpsc::UnboundedSender<SessionCommand>,
    pub task_completion_reservations:
        xai_grok_tools::reminders::task_completion::TaskCompletionReservations,
    pub task_wake_suppressed: xai_grok_tools::reminders::task_completion::TaskWakeSuppressed,
    /// Channel for requesting trace uploads for synthetic auto-wake turns.
    /// Wrapped in `Arc<Mutex<..>>` because the coordinator creates the channel
    /// after the notification bridge is spawned — the bridge reads the latest
    /// value on each notification.
    pub(crate) synthetic_trace_tx: Arc<
        std::sync::Mutex<
            Option<
                tokio::sync::mpsc::UnboundedSender<crate::upload::turn::SyntheticTurnTraceRequest>,
            >,
        >,
    >,
    /// Resolved name of the `BackgroundTaskAction` tool. Written exactly
    /// once after the agent's toolset is finalized; read many times
    /// thereafter from the notification bridge and the session actor's
    /// between-turn drain. `None` means no such tool is registered in this
    /// toolset, which is a valid resolved state.
    pub task_output_tool_name: Arc<std::sync::OnceLock<Option<String>>>,
    /// Resolved name of the `Read` tool, used by `format_bash_completion`'s
    /// disk-pointer footer so the model can recover full bash output from
    /// `task.output_file` even when no polling tool is available. Same
    /// write-once-read-many lifecycle as `task_output_tool_name`.
    pub read_tool_name: Arc<std::sync::OnceLock<Option<String>>>,
    /// When `false`, bash task completions fall back to the idle-gated
    /// `InjectNotification` path instead of immediate synthetic prompts.
    pub auto_wake_enabled: bool,
    /// When `true`, an approved `PlanModeExited` also arms the tracker's
    /// next-turn exit reminder. Grok-build leaves this `false` — its
    /// exit-plan tool result already informs the model, and a deferred
    /// reminder would arrive stale. Shared with the session actor (the
    /// `gateway_enabled` pattern) and refreshed on zero-turn rebuilds so the
    /// bridge always agrees with the live session gate.
    pub queue_exit_reminder_on_approved_exit: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// When `true`, suppress the bash auto-wake synthetic prompt. Shared `Arc`
    /// written at one chokepoint — see
    /// `SessionActor::set_goal_loop_active_resource` for the rationale.
    pub goal_loop_active: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
/// Snapshot a shared `OnceLock` tool-name slot as a borrowed `&str`.
/// Returns `None` if the slot is still unset (toolset not yet finalized)
/// or if the resolved value is `None` (no such tool registered in this
/// toolset.
pub(crate) fn resolved_tool_name(slot: &std::sync::OnceLock<Option<String>>) -> Option<&str> {
    slot.get().and_then(|v| v.as_deref())
}
/// Stamp a bridge-emitted notification's meta before it forks into
/// persistence + broadcast — see `util::event_id::ensure_event_id_meta`.
fn stamp_event_id(config: &NotificationBridgeConfig, meta: &mut Option<acp::Meta>) {
    stamp_event_id_for(&config.session_id, meta);
}

/// Same, for a frame addressed to `session_id`.
///
/// A routed frame carries the *owner's* `sessionId`, so its `eventId` prefix
/// must match that session: the id is `"{sessionId}-{counter}"` and clients
/// parse the suffix only, but a frame whose prefix disagrees with its own
/// `sessionId` reads as a foreign event in the transcript.
fn stamp_event_id_for(session_id: &acp::SessionId, meta: &mut Option<acp::Meta>) {
    crate::util::event_id::ensure_event_id_meta(&session_id.0, meta);
}

/// Session a notification's frame must be addressed to.
///
/// A subagent inherits the parent's scheduler, so a schedule it created is
/// announced on the parent's bridge. The owner stamp routes the card to the
/// creating session while it is alive; a dead owner falls back to this bridge.
fn frame_session_for(owner: Option<&str>, config: &NotificationBridgeConfig) -> acp::SessionId {
    route_frame(owner, config).0
}

/// Resolve the owner of a frame once, for both its session id and its
/// persistence channel.
fn route_frame(
    owner: Option<&str>,
    config: &NotificationBridgeConfig,
) -> (
    acp::SessionId,
    Option<crate::session::delivery::SessionDeliveryTarget>,
) {
    let routed = match crate::session::delivery::route(owner, config.session_id.0.as_ref()) {
        crate::session::delivery::Delivery::Routed(target) => Some(target),
        crate::session::delivery::Delivery::Local => None,
    };
    let frame_session_id = routed.as_ref().map_or_else(
        || config.session_id.clone(),
        |target| acp::SessionId::new(target.session_id.clone()),
    );
    (frame_session_id, routed)
}

/// Persistence channel a frame belongs to: the owner's when routed, this
/// bridge's otherwise.
///
/// Without this a frame addressed to the owner was written to the holder's
/// transcript — a foreign `sessionId` in the holder's file, and nothing in the
/// owner's after it reloads.
fn frame_persistence<'a>(
    routed: &'a Option<crate::session::delivery::SessionDeliveryTarget>,
    config: &'a NotificationBridgeConfig,
) -> &'a tokio::sync::mpsc::UnboundedSender<PersistenceMsg> {
    routed
        .as_ref()
        .map_or(&config.persistence.tx, |target| &target.persistence_tx)
}

fn stamp_scheduler_meta(
    session_id: &acp::SessionId,
    meta: &mut Option<acp::Meta>,
    generation: &str,
    revision: u64,
) {
    stamp_event_id_for(session_id, meta);
    let meta = meta.get_or_insert_with(acp::Meta::new);
    meta.insert("x.ai/schedulerGeneration".to_owned(), generation.into());
    meta.insert("x.ai/schedulerRevision".to_owned(), revision.into());
}
/// Hand a completion to the session that owns the task, when that is not the
/// session this bridge belongs to.
///
/// A shared terminal backend (a subagent reusing the parent's) or a reparented
/// task can deliver a completion on a handle whose session is not the owner.
/// The owner is authoritative: the completion is queued on its command channel
/// as a deferred notification and its task id is reserved there so the owner's
/// own reminder does not surface the same completion twice.
///
/// Returns the target when the delivery was queued — the caller then skips the
/// local auto-wake/inject paths. `None` means the owner is gone (channel
/// closed), so the caller falls back to delivering locally.
fn deliver_completion_to_owner(
    config: &NotificationBridgeConfig,
    target: &crate::session::delivery::SessionDeliveryTarget,
    task_snapshot: &xai_grok_tools::computer::types::TaskSnapshot,
    completion_kind: CompletedTaskKind,
    is_monitor: bool,
    task_id: &str,
) -> Option<crate::session::delivery::SessionDeliveryTarget> {
    let tool_name = resolved_tool_name(&config.task_output_tool_name);
    let read_name = resolved_tool_name(&config.read_tool_name);
    let message = xai_grok_tools::reminders::wrap_reminder(&completion_kind.body(
        task_snapshot,
        tool_name,
        read_name,
    ));
    let source = if is_monitor {
        NotificationSource::MonitorCompleted {
            task_id: task_id.to_string(),
        }
    } else {
        NotificationSource::BashTaskCompleted {
            task_id: task_id.to_string(),
        }
    };
    let sent = target.cmd_tx.send(SessionCommand::InjectNotification {
        prompt_id: completion_kind.fallback_prompt_id(task_id),
        prompt_blocks: vec![acp::ContentBlock::Text(acp::TextContent::new(message))],
        priority: NotificationPriority::Later,
        source,
    });
    if sent.is_err() {
        return None;
    }
    target
        .task_completion_reservations
        .reserve(task_id.to_string());
    tracing::info!(
        task_id = %task_id,
        owner = %target.session_id,
        bridge_session = %config.session_id.0,
        is_monitor,
        "task completion routed to its owning session"
    );
    Some(target.clone())
}

fn durable_append_landed(result: Result<(), DurableAppendError>) -> Result<(), String> {
    match result {
        Ok(()) => Ok(()),
        Err(DurableAppendError::Committed(error)) => {
            tracing::warn!(%error, "Scheduler tombstone committed with bookkeeping failure");
            Ok(())
        }
        Err(DurableAppendError::NotCommitted(error)) => {
            Err(format!("scheduler tombstone was not committed: {error}"))
        }
        Err(DurableAppendError::AcknowledgementLost(error)) => Err(format!(
            "scheduler tombstone commit status is unknown: {error}"
        )),
    }
}
async fn handle_scheduled_task_removed(
    config: &NotificationBridgeConfig,
    removed: xai_grok_tools::notification::ScheduledTaskRemoved,
    acknowledgement: Option<tokio::sync::oneshot::Sender<Result<(), String>>>,
) -> Result<(), String> {
    tracing::info!(task_id = %removed.task_id, "Scheduled task removed");
    let result: Result<Box<serde_json::value::RawValue>, String> = async {
        let (frame_session_id, routed) = route_frame(removed.owner_session_id.as_deref(), config);
        let mut meta = None;
        stamp_scheduler_meta(
            &frame_session_id,
            &mut meta,
            &removed.generation,
            removed.revision,
        );
        let notification = crate::extensions::notification::SessionNotification {
            session_id: frame_session_id,
            update: crate::extensions::notification::SessionUpdate::ScheduledTaskDeleted {
                task_id: removed.task_id,
            },
            meta: meta.map(serde_json::Value::Object),
        };
        let params = serde_json::to_value(&notification)
            .and_then(|value| serde_json::value::to_raw_value(&value))
            .map_err(|error| format!("failed to serialize scheduled task deletion: {error}"))?;
        let update = crate::session::storage::SessionUpdate::Xai(Box::new(notification));
        if acknowledgement.is_some() {
            // The durable append targets this bridge's session file, which owns
            // the scheduler state; the live write goes to the frame's owner.
            durable_append_landed(config.persistence.append_update_durably(update).await)?;
        } else {
            frame_persistence(&routed, config)
                .send(PersistenceMsg::Update(update))
                .map_err(|_| "session persistence stopped".to_owned())?;
        }
        Ok(params)
    }
    .await;
    match result {
        Ok(params) => {
            if let Some(acknowledgement) = acknowledgement {
                let _ = acknowledgement.send(Ok(()));
            }
            config
                .gateway
                .forward_fire_and_forget(acp::ExtNotification::new(
                    "x.ai/scheduled_task_deleted",
                    params.into(),
                ));
            Ok(())
        }
        Err(error) => {
            if let Some(acknowledgement) = acknowledgement {
                let _ = acknowledgement.send(Err(error.clone()));
            }
            Err(error)
        }
    }
}
/// Create a `ToolNotificationHandle` and spawn a bridge task that
/// translates notifications into shell-native systems.
pub fn spawn_notification_bridge(config: NotificationBridgeConfig) -> ToolNotificationHandle {
    let (handle, mut rx) = ToolNotificationHandle::acknowledged_channel();
    tokio::task::spawn_local(async move {
        let mut state = BridgeState::default();
        while let Some(delivery) = rx.recv().await {
            let acknowledgement = delivery.acknowledgement;
            match delivery.notification {
                ToolNotification::ScheduledTaskRemoved(removed) => {
                    if let Err(error) =
                        handle_scheduled_task_removed(&config, removed, acknowledgement).await
                    {
                        tracing::warn!(%error, "Failed to handle scheduled task removal");
                    }
                }
                notification => {
                    handle_notification(&config, notification, &mut state).await;
                    if let Some(acknowledgement) = acknowledgement {
                        let _ = acknowledgement.send(Ok(()));
                    }
                }
            }
        }
        tracing::debug!("Notification bridge task exiting (sender dropped)");
    });
    handle
}
/// Emit a `CurrentModeUpdate` for the given [`SessionMode`] — persisted to
/// `updates.jsonl` so session replay re-applies the mode, and forwarded to
/// the gateway so the pager updates live.
async fn emit_current_mode_update(
    config: &NotificationBridgeConfig,
    mode: xai_grok_tools::types::SessionMode,
) {
    let mut notification = acp::SessionNotification::new(
        config.session_id.clone(),
        acp::SessionUpdate::CurrentModeUpdate(acp::CurrentModeUpdate::new(
            acp::SessionModeId::new(mode.as_id()),
        )),
    );
    stamp_event_id(config, &mut notification.meta);
    let _ = config.persistence.tx.send(PersistenceMsg::Update(
        crate::session::storage::SessionUpdate::Acp(Box::new(notification.clone())),
    ));
    config.gateway.forward_fire_and_forget(notification);
}
/// Handle a single notification by forwarding it to the appropriate shell system.
async fn handle_notification(
    config: &NotificationBridgeConfig,
    notification: ToolNotification,
    state: &mut BridgeState,
) {
    let offsets = &mut state.offsets;
    match notification {
        ToolNotification::BashOutputChunk(chunk) => {
            let (output, output_delta) = if config.incremental_bash_output {
                let prev_offset = offsets.get(&chunk.base.tool_call_id).copied().unwrap_or(0);
                let full = &chunk.base.output;
                let delta = if prev_offset <= full.len() {
                    full[prev_offset..].to_vec()
                } else {
                    full.clone()
                };
                offsets.insert(chunk.base.tool_call_id.clone(), full.len());
                (Vec::new(), Some(delta))
            } else {
                (chunk.base.output.clone(), None)
            };
            let bash_output = ToolOutput::Bash(BashOutput {
                output_for_prompt: BashOutput::make_output_for_prompt(&String::from_utf8_lossy(
                    &chunk.base.output,
                )),
                output,
                exit_code: 0,
                command: chunk.base.command.clone(),
                truncated: chunk.base.truncated,
                signal: None,
                timed_out: false,
                description: None,
                current_dir: chunk.base.cwd.to_string_lossy().to_string(),
                output_file: String::new(),
                total_bytes: chunk.base.total_bytes,
                output_delta,
                was_bare_echo: false,
            });
            let update = acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate::new(
                acp::ToolCallId::new(chunk.base.tool_call_id.clone()),
                acp::ToolCallUpdateFields::new()
                    .status(Some(acp::ToolCallStatus::InProgress))
                    .content(Some(vec![acp::ToolCallContent::from(
                        acp::ContentBlock::Text(acp::TextContent::new(
                            String::from_utf8_lossy(&chunk.base.output).into_owned(),
                        )),
                    )]))
                    .raw_output(serde_json::to_value(&bash_output).ok()),
            ));
            let mut notification = acp::SessionNotification::new(config.session_id.clone(), update);
            stamp_event_id(config, &mut notification.meta);
            let _ = config.persistence.tx.send(PersistenceMsg::Update(
                crate::session::storage::SessionUpdate::Acp(Box::new(notification.clone())),
            ));
            if config
                .gateway_enabled
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                let _ = config.gateway.session_notification(notification).await;
            }
        }
        ToolNotification::BashExecutionComplete(complete) => {
            offsets.remove(&complete.base.tool_call_id);
            tracing::debug!(
                tool_call_id = %complete.base.tool_call_id,
                exit_code = ?complete.exit_code,
                "Bash execution complete notification received"
            );
        }
        ToolNotification::BashExecutionTimeout(timeout) => {
            tracing::debug!(
                tool_call_id = %timeout.base.tool_call_id,
                elapsed = ?timeout.elapsed,
                "Bash execution timeout notification received"
            );
        }
        ToolNotification::BashExecutionFailed(failed) => {
            tracing::warn!(
                tool_call_id = %failed.tool_call_id,
                error = %failed.error,
                "Bash execution failed notification received"
            );
        }
        ToolNotification::BashExecutionBackgrounded(bg) => {
            tracing::debug!(
                tool_call_id = %bg.base.tool_call_id,
                task_id = %bg.task_id,
                command = %bg.base.command,
                output_file = %bg.output_file.display(),
                "Bash execution backgrounded notification received — forwarding to TUI"
            );
            let mut notification = crate::extensions::notification::SessionNotification {
                session_id: config.session_id.clone(),
                update: crate::extensions::notification::SessionUpdate::TaskBackgrounded {
                    tool_call_id: bg.base.tool_call_id.clone(),
                    task_id: bg.task_id.clone(),
                    command: bg.base.command.clone(),
                    cwd: bg.base.cwd.to_string_lossy().to_string(),
                    output_file: bg.output_file.to_string_lossy().to_string(),
                    monitor_description: bg.monitor_description.clone(),
                    description: bg.description.clone(),
                },
                meta: None,
            };
            {
                let mut meta_map = None;
                stamp_event_id(config, &mut meta_map);
                notification.meta = meta_map.map(serde_json::Value::Object);
            }
            let _ = config.persistence.tx.send(PersistenceMsg::Update(
                crate::session::storage::SessionUpdate::Xai(Box::new(notification.clone())),
            ));
            let params = serde_json::to_value(&notification)
                .and_then(|v| serde_json::value::to_raw_value(&v))
                .ok();
            if let Some(params) = params {
                let ext_notification =
                    acp::ExtNotification::new("x.ai/task_backgrounded", params.into());
                config.gateway.forward_fire_and_forget(ext_notification);
            }
        }
        ToolNotification::FileWritten(written) => {
            let prompt_index = *config.prompt_index.lock().await;
            config.hunk_tracker_handle.record_agent_write(
                written.absolute_path.clone(),
                written.content.clone(),
                prompt_index,
                written.previous_content.clone(),
            );
            if written.previous_content.is_some() || written.is_new_file {
                config
                    .file_state_tracker
                    .add_before_snapshot_for_prompt(
                        prompt_index,
                        &written.absolute_path,
                        &config.cwd,
                        written.previous_content,
                    )
                    .await;
            }
            tracing::debug!(
                path = %written.absolute_path.display(),
                is_new_file = written.is_new_file,
                "FileWritten notification forwarded to hunk tracker"
            );
        }
        ToolNotification::TaskCompleted(task_snapshot) => {
            let completion_kind = CompletedTaskKind::of(&task_snapshot);
            let is_monitor = completion_kind.is_monitor();
            let task_id = task_snapshot.task_id.clone();
            let goal_loop_active = config
                .goal_loop_active
                .load(std::sync::atomic::Ordering::Relaxed);
            let mut will_wake = false;
            // Owner routing: when the completion arrives on a handle that
            // belongs to another session (shared backend, reparented task),
            // deliver it to the owning session instead of this one.
            let routed = match crate::session::delivery::route(
                task_snapshot.owner_session_id.as_deref(),
                config.session_id.0.as_ref(),
            ) {
                crate::session::delivery::Delivery::Routed(target) => deliver_completion_to_owner(
                    config,
                    &target,
                    &task_snapshot,
                    completion_kind,
                    is_monitor,
                    &task_id,
                ),
                crate::session::delivery::Delivery::Local => None,
            };
            if routed.is_some() {
                // Queued on the owner's channel; its drain owns the wording.
            } else if task_snapshot.block_waited || task_snapshot.explicitly_killed {
            } else if goal_loop_active {
                tracing::info!(
                    task_id = %task_id,
                    is_monitor,
                    "auto-wake: suppressed completion (goal loop active)"
                );
            } else if config.auto_wake_enabled {
                config.task_completion_reservations.reserve(task_id.clone());
                let tool_name = resolved_tool_name(&config.task_output_tool_name);
                let read_name = resolved_tool_name(&config.read_tool_name);
                let body = completion_kind.body(&task_snapshot, tool_name, read_name);
                let message = xai_grok_tools::reminders::wrap_reminder(&body);
                let prompt_id = format!("task-completed-{task_id}");
                let prompt_blocks = vec![acp::ContentBlock::Text(acp::TextContent::new(message))];
                let synthetic_trace_tx = config
                    .synthetic_trace_tx
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone();
                let (respond_to, completion_rx) = tokio::sync::oneshot::channel();
                let (admission_tx, admission_rx) = tokio::sync::oneshot::channel();
                tracing::info!(
                    task_id = %task_id,
                    prompt_id = %prompt_id,
                    is_monitor,
                    "auto-wake: requesting synthetic prompt admission for completed background task"
                );
                let enqueued = config
                    .session_cmd_tx
                    .send(SessionCommand::Prompt {
                        prompt_id: prompt_id.clone(),
                        origin: crate::session::PromptOrigin::TaskCompleted {
                            task_id: task_id.clone(),
                        },
                        prompt_blocks,
                        prompt_mode: crate::session::plan_mode::PromptMode::Agent,
                        artifact_upload_ctx: None,
                        client_identifier: None,
                        screen_mode: None,
                        tersify_level: None,
                        verbatim: true,
                        traceparent: xai_file_utils::trace_context::current_traceparent(),
                        json_schema: None,
                        send_now: false,
                        tool_overrides_update: None,
                        admission: Some(crate::session::commands::TaskWakeAdmission {
                            respond_to: admission_tx,
                            fallback: crate::session::commands::TaskWakeFallback {
                                prompt_id: completion_kind.fallback_prompt_id(&task_id),
                                prompt_blocks: vec![acp::ContentBlock::Text(
                                    acp::TextContent::new(body.clone()),
                                )],
                                source: if is_monitor {
                                    NotificationSource::MonitorCompleted {
                                        task_id: task_id.clone(),
                                    }
                                } else {
                                    NotificationSource::BashTaskCompleted {
                                        task_id: task_id.clone(),
                                    }
                                },
                            },
                        }),
                        respond_to,
                        persist_ack: None,
                        parsed_prompt_tx: None,
                    })
                    .is_ok();
                let outcome = if !enqueued {
                    WakeAdmissionOutcome::SendFailed
                } else {
                    match tokio::time::timeout(TASK_WAKE_ADMISSION_TIMEOUT, admission_rx).await {
                        Ok(Ok(true)) => WakeAdmissionOutcome::Accepted,
                        Ok(Ok(false)) => WakeAdmissionOutcome::Refused,
                        Ok(Err(_)) | Err(_) => WakeAdmissionOutcome::NoResponse,
                    }
                };
                will_wake = outcome.will_wake();
                if matches!(outcome, WakeAdmissionOutcome::SendFailed) {
                    // The wake prompt never reached the session actor. Park the
                    // completion as a pending notification (same shape as the
                    // auto-wake-disabled branch) so the idle / next-turn drain
                    // still surfaces it instead of losing it silently.
                    config.task_completion_reservations.release(&task_id);
                    let source = if is_monitor {
                        NotificationSource::MonitorCompleted {
                            task_id: task_id.clone(),
                        }
                    } else {
                        NotificationSource::BashTaskCompleted {
                            task_id: task_id.clone(),
                        }
                    };
                    let _ = config
                        .session_cmd_tx
                        .send(SessionCommand::InjectNotification {
                            prompt_id: completion_kind.fallback_prompt_id(&task_id),
                            prompt_blocks: vec![acp::ContentBlock::Text(acp::TextContent::new(
                                body.clone(),
                            ))],
                            priority: NotificationPriority::Later,
                            source,
                        });
                }
                xai_grok_telemetry::unified_log::info(
                    "shell.task_wake.bridge_admission",
                    Some(config.session_id.0.as_ref()),
                    Some(serde_json::json!({
                        "task_id": &task_id,
                        "monitor": is_monitor,
                        "enqueued": enqueued,
                        "admitted": will_wake,
                        "admission_outcome": outcome.as_str(),
                        "gate": config.task_wake_suppressed.get(),
                    })),
                );
                if will_wake {
                    if is_monitor {
                        let _ =
                            config
                                .session_cmd_tx
                                .send(SessionCommand::DropMonitorNotifications {
                                    task_id: task_id.clone(),
                                });
                    }
                    if let Some(trace_tx) = synthetic_trace_tx {
                        let (before_copy_tx, before_session_copy_rx) =
                            tokio::sync::oneshot::channel();
                        let copy_requested = config
                            .session_cmd_tx
                            .send(SessionCommand::CopyFile {
                                respond_to: before_copy_tx,
                            })
                            .is_ok();
                        if copy_requested {
                            tracing::info!(
                                task_id = %task_id,
                                "auto-wake: sending synthetic turn trace request"
                            );
                            let _ = trace_tx.send(crate::upload::turn::SyntheticTurnTraceRequest {
                                session_id: config.session_id.clone(),
                                prompt_id,
                                completion_rx,
                                before_session_copy_rx,
                            });
                        } else {
                            tracing::debug!(
                                task_id = %task_id,
                                "auto-wake: session snapshot request failed, skipping trace request"
                            );
                        }
                    } else {
                        tracing::debug!(
                            task_id = %task_id,
                            "auto-wake: no synthetic trace consumer, skipping trace request"
                        );
                    }
                }
            } else {
                let tool_name = resolved_tool_name(&config.task_output_tool_name);
                let read_name = resolved_tool_name(&config.read_tool_name);
                let message = completion_kind.body(&task_snapshot, tool_name, read_name);
                let source = if is_monitor {
                    NotificationSource::MonitorCompleted {
                        task_id: task_id.clone(),
                    }
                } else {
                    NotificationSource::BashTaskCompleted {
                        task_id: task_id.clone(),
                    }
                };
                let _ = config
                    .session_cmd_tx
                    .send(SessionCommand::InjectNotification {
                        prompt_id: completion_kind.fallback_prompt_id(&task_id),
                        prompt_blocks: vec![acp::ContentBlock::Text(acp::TextContent::new(
                            message,
                        ))],
                        priority: NotificationPriority::Later,
                        source,
                    });
            }
            let mut notification = crate::extensions::notification::SessionNotification {
                // A routed completion belongs to the owner's transcript, so
                // the frontend frame carries the owner's session id.
                session_id: routed.as_ref().map_or_else(
                    || config.session_id.clone(),
                    |t| acp::SessionId::new(t.session_id.clone()),
                ),
                update: crate::extensions::notification::SessionUpdate::TaskCompleted {
                    task_snapshot,
                    will_wake,
                },
                meta: None,
            };
            {
                let mut meta_map = None;
                // Routed frames belong to the owner: the event id prefix must
                // name the frame's session, not the bridge's.
                stamp_event_id_for(&notification.session_id, &mut meta_map);
                notification.meta = meta_map.map(serde_json::Value::Object);
            }
            // A routed frame belongs to the owner's transcript and the owner's
            // hook registry, not to the bridge's session.
            let frame_persistence = routed
                .as_ref()
                .map_or(&config.persistence.tx, |target| &target.persistence_tx);
            let _ = frame_persistence.send(PersistenceMsg::Update(
                crate::session::storage::SessionUpdate::Xai(Box::new(notification.clone())),
            ));
            let params = serde_json::to_value(&notification)
                .and_then(|v| serde_json::value::to_raw_value(&v))
                .ok();
            if let Some(params) = params {
                let notification: acp::ExtNotification =
                    acp::ExtNotification::new("x.ai/task_completed", params.into());
                config.gateway.forward_fire_and_forget(notification);
            }
            let hook_tx = routed
                .as_ref()
                .map_or(&config.session_cmd_tx, |target| &target.cmd_tx);
            let _ = hook_tx.send(SessionCommand::DispatchNotificationHook {
                notification_type: "task_complete".into(),
                message: Some(format!("Background task completed: {task_id}")),
                title: None,
                level: Some("info".into()),
            });
        }
        ToolNotification::PlanModeEntered(entered) => {
            let activated = config.plan_mode.lock().activate_from_tool();
            if activated {
                *config.current_prompt_mode.lock() = crate::session::plan_mode::PromptMode::Plan;
                *config.turn_prompt_mode.lock() = crate::session::plan_mode::PromptMode::Plan;
                let snapshot = config.plan_mode.lock().snapshot();
                let _ = config
                    .persistence
                    .tx
                    .send(PersistenceMsg::PlanModeState(snapshot));
                emit_current_mode_update(config, xai_grok_tools::types::SessionMode::Plan).await;
            }
            tracing::info!(
                tool_call_id = %entered.tool_call_id,
                activated,
                "Plan mode entered via EnterPlanMode tool"
            );
        }
        ToolNotification::PlanModeExited(exited) => {
            let deactivated = {
                let mut tracker = config.plan_mode.lock();
                let deactivated = tracker.deactivate_approved();
                if deactivated
                    && config
                        .queue_exit_reminder_on_approved_exit
                        .load(std::sync::atomic::Ordering::Relaxed)
                {
                    tracker.queue_exit_reminder();
                }
                deactivated
            };
            if deactivated {
                *config.current_prompt_mode.lock() = crate::session::plan_mode::PromptMode::Agent;
                *config.turn_prompt_mode.lock() = crate::session::plan_mode::PromptMode::Agent;
                let snapshot = config.plan_mode.lock().snapshot();
                let _ = config
                    .persistence
                    .tx
                    .send(PersistenceMsg::PlanModeState(snapshot));
                emit_current_mode_update(config, xai_grok_tools::types::SessionMode::Default).await;
            }
            tracing::info!(
                tool_call_id = %exited.tool_call_id,
                deactivated,
                has_plan = exited.plan_content.is_some(),
                "Plan mode exited via ExitPlanMode tool"
            );
        }
        ToolNotification::UserQuestionAsked(asked) => {
            tracing::info!(
                tool_call_id = %asked.tool_call_id,
                "User question asked"
            );
        }
        ToolNotification::LspServerStarting(s) => {
            tracing::debug!(server = %s.server_name, command = %s.command, "LSP server starting");
        }
        ToolNotification::LspServerReady(s) => {
            tracing::info!(server = %s.server_name, "LSP server ready");
        }
        ToolNotification::LspServerCrashed(s) => {
            tracing::warn!(server = %s.server_name, "LSP server crashed");
        }
        ToolNotification::LspServerRetrying(s) => {
            tracing::warn!(
                server = %s.server_name,
                attempt = s.attempt,
                max_restarts = s.max_restarts,
                backoff_ms = s.backoff_ms,
                "LSP server retrying"
            );
        }
        ToolNotification::LspServerFailed(s) => {
            tracing::error!(server = %s.server_name, error = %s.error, "LSP server failed");
        }
        ToolNotification::ScheduledTaskFired(fired) => {
            tracing::info!(
                task_id = %fired.task_id,
                schedule = %fired.human_schedule,
                subagent_id = fired.subagent_id.as_deref().unwrap_or(""),
                "Scheduled task fired"
            );
            // A subagent inherits the parent's scheduler, so a schedule it
            // created fires on the parent's bridge. The owner stamp routes
            // the fire (and its injected prompt) to the creating session
            // while it is alive; a dead owner falls back to this bridge.
            let routed = match crate::session::delivery::route(
                fired.owner_session_id.as_deref(),
                config.session_id.0.as_ref(),
            ) {
                crate::session::delivery::Delivery::Routed(target) => Some(target),
                crate::session::delivery::Delivery::Local => None,
            };
            let frame_session_id = routed.as_ref().map_or_else(
                || config.session_id.clone(),
                |t| acp::SessionId::new(t.session_id.clone()),
            );
            if let Some(target) = &routed {
                tracing::info!(
                    task_id = %fired.task_id,
                    owner = %target.session_id,
                    bridge_session = %config.session_id.0,
                    "scheduled fire routed to its owning session"
                );
            }
            if fired.subagent_id.is_none() {
                let inject_payload = serde_json::json!({
                    "sessionId": frame_session_id,
                    "taskId": &fired.task_id,
                    "prompt": &fired.prompt,
                    "humanSchedule": &fired.human_schedule,
                    "nextFireAt": &fired.next_fire_at,
                });
                if let Ok(params) = serde_json::value::to_raw_value(&inject_payload) {
                    config
                        .gateway
                        .forward_fire_and_forget(acp::ExtNotification::new(
                            "x.ai/scheduled_task_inject_prompt",
                            params.into(),
                        ));
                }
            }
            let mut meta = None;
            // `frame_session_id` is the owner's when the fire was routed; the
            // event id must agree with the frame it belongs to.
            stamp_scheduler_meta(
                &frame_session_id,
                &mut meta,
                &fired.generation,
                fired.revision,
            );
            let fired_notif = crate::extensions::notification::SessionNotification {
                session_id: frame_session_id,
                update: crate::extensions::notification::SessionUpdate::ScheduledTaskFired {
                    task_id: fired.task_id,
                    prompt: fired.prompt,
                    human_schedule: fired.human_schedule,
                    next_fire_at: fired.next_fire_at,
                    subagent_id: fired.subagent_id,
                },
                meta: meta.map(serde_json::Value::Object),
            };
            if let Ok(params) =
                serde_json::to_value(&fired_notif).and_then(|v| serde_json::value::to_raw_value(&v))
            {
                config
                    .gateway
                    .forward_fire_and_forget(acp::ExtNotification::new(
                        "x.ai/scheduled_task_fired",
                        params.into(),
                    ));
            }
        }
        ToolNotification::MonitorEvent(event) => {
            let my_session = config.session_id.0.as_ref();
            // A monitor runs on the backend of whoever started it, so the event
            // normally reaches the owner's bridge. When it lands here instead
            // (a reparent race, a handle captured before an adoption) route it
            // rather than dropping it: the owner still has the task and its
            // conversation is where the event belongs.
            let routed = match crate::session::delivery::route(
                event.owner_session_id.as_deref(),
                my_session,
            ) {
                crate::session::delivery::Delivery::Routed(target) => Some(target),
                crate::session::delivery::Delivery::Local => None,
            };
            let frame_session_id = routed.as_ref().map_or_else(
                || config.session_id.clone(),
                |t| acp::SessionId::new(t.session_id.clone()),
            );
            if let Some(target) = &routed {
                tracing::debug!(
                    task_id = %event.task_id,
                    description = %event.description,
                    monitor_owner = %target.session_id,
                    bridge_session = %my_session,
                    "monitor event routed to its owning session"
                );
            }
            tracing::debug!(
                task_id = %event.task_id,
                description = %event.description,
                "Monitor event received, injecting into session"
            );
            let notification = crate::extensions::notification::SessionNotification {
                session_id: frame_session_id,
                update: crate::extensions::notification::SessionUpdate::MonitorEvent {
                    task_id: event.task_id.clone(),
                    description: event.description.clone(),
                    event_text: event.raw_text.clone(),
                },
                meta: None,
            };
            let params = serde_json::to_value(&notification)
                .and_then(|v| serde_json::value::to_raw_value(&v))
                .ok();
            if let Some(params) = params {
                config
                    .gateway
                    .forward_fire_and_forget(acp::ExtNotification::new(
                        "x.ai/monitor_event",
                        params.into(),
                    ));
            }
            // The owner's own bookkeeping decides whether the model still needs
            // the inject: its reservation is set when the task auto-woke.
            let (reservations, cmd_tx) = match &routed {
                Some(target) => (
                    target.task_completion_reservations.clone(),
                    target.cmd_tx.clone(),
                ),
                None => (
                    config.task_completion_reservations.clone(),
                    config.session_cmd_tx.clone(),
                ),
            };
            if reservations.contains(&event.task_id) {
                tracing::debug!(
                    task_id = %event.task_id,
                    "skipping model inject for monitor event: task already auto-woke via TaskCompleted"
                );
                return;
            }
            let prompt_id = format!("monitor-{}-{}", event.task_id, uuid::Uuid::now_v7());
            let prompt_blocks = vec![acp::ContentBlock::Text(acp::TextContent::new(
                event.event_text,
            ))];
            let _ = cmd_tx.send(SessionCommand::InjectNotification {
                prompt_id,
                prompt_blocks,
                priority: NotificationPriority::Next,
                source: NotificationSource::MonitorEvent {
                    task_id: event.task_id.clone(),
                },
            });
        }
        ToolNotification::ScheduledTaskRemoved(removed) => {
            if let Err(error) = handle_scheduled_task_removed(config, removed, None).await {
                tracing::warn!(%error, "Failed to handle scheduled task removal");
            }
        }
        ToolNotification::AssetJobEvent(event) => {
            // Coalesce progress before the ACP hop: the pager renders only what
            // reaches it, so a fast transfer must not flood the channel.
            if !state.asset_jobs.admit(&event) {
                tracing::trace!(
                    job_id = %event.job_id,
                    state = %event.state,
                    "asset job progress coalesced"
                );
                return;
            }
            let notification = crate::extensions::notification::SessionNotification {
                session_id: config.session_id.clone(),
                update: crate::extensions::notification::SessionUpdate::AssetJobEvent {
                    job_id: event.job_id.clone(),
                    kind: event.kind,
                    key: event.key,
                    backend: event.backend,
                    state: event.state,
                    bytes_transferred: event.bytes_transferred,
                    bytes_total: event.bytes_total,
                    error: event.error,
                },
                meta: None,
            };
            if let Ok(params) = serde_json::to_value(&notification)
                .and_then(|v| serde_json::value::to_raw_value(&v))
            {
                config
                    .gateway
                    .forward_fire_and_forget(acp::ExtNotification::new(
                        "x.ai/asset_job_event",
                        params.into(),
                    ));
            }
        }
        ToolNotification::ScheduledTaskCreated(created) => {
            tracing::info!(task_id = %created.task_id, "Scheduled task created");
            let (frame_session_id, routed) =
                route_frame(created.owner_session_id.as_deref(), config);
            let mut meta = None;
            stamp_scheduler_meta(
                &frame_session_id,
                &mut meta,
                &created.generation,
                created.revision,
            );
            let notification = crate::extensions::notification::SessionNotification {
                session_id: frame_session_id,
                update: crate::extensions::notification::SessionUpdate::ScheduledTaskCreated {
                    task_id: created.task_id,
                    prompt: created.prompt,
                    human_schedule: created.human_schedule,
                    next_fire_at: created.next_fire_at,
                },
                meta: meta.map(serde_json::Value::Object),
            };
            let _ = frame_persistence(&routed, config).send(PersistenceMsg::Update(
                crate::session::storage::SessionUpdate::Xai(Box::new(notification.clone())),
            ));
            if let Ok(params) = serde_json::to_value(&notification)
                .and_then(|v| serde_json::value::to_raw_value(&v))
            {
                config
                    .gateway
                    .forward_fire_and_forget(acp::ExtNotification::new(
                        "x.ai/scheduled_task_created",
                        params.into(),
                    ));
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use xai_grok_tools::computer::types::TaskKind;
    use xai_grok_tools::types::TaskSnapshot;
    /// Drive the admission handshake inline so receiver assertions observe the
    /// bridge's command order without racing a detached proxy task.
    async fn handle_notification_with_admission(
        config: &NotificationBridgeConfig,
        notification: ToolNotification,
        state: &mut BridgeState,
        cmd_rx: &mut mpsc::UnboundedReceiver<SessionCommand>,
        accepted: bool,
    ) {
        let notification = handle_notification(config, notification, state);
        tokio::pin!(notification);
        let mut command = tokio::select! {
            _ = &mut notification => panic!("notification completed before requesting admission"),
            command = cmd_rx.recv() => command.expect("expected task-wake prompt"),
        };
        let SessionCommand::Prompt { admission, .. } = &mut command else {
            panic!("expected task-wake prompt");
        };
        admission
            .take()
            .expect("expected task-wake admission request")
            .respond_to
            .send(accepted)
            .expect("notification must still be awaiting admission");
        config
            .session_cmd_tx
            .send(command)
            .expect("test command receiver must remain open");
        notification.await;
    }
    fn make_test_config() -> (
        NotificationBridgeConfig,
        mpsc::UnboundedReceiver<SessionCommand>,
    ) {
        let (config, _gateway_rx, _persistence_rx, session_cmd_rx) = make_test_config_full();
        (config, session_cmd_rx)
    }
    #[allow(clippy::type_complexity)]
    fn make_test_config_full() -> (
        NotificationBridgeConfig,
        mpsc::UnboundedReceiver<xai_acp_lib::AcpClientMessage>,
        mpsc::UnboundedReceiver<PersistenceMsg>,
        mpsc::UnboundedReceiver<SessionCommand>,
    ) {
        make_test_config_full_raw()
    }
    #[allow(clippy::type_complexity)]
    fn make_test_config_full_raw() -> (
        NotificationBridgeConfig,
        mpsc::UnboundedReceiver<xai_acp_lib::AcpClientMessage>,
        mpsc::UnboundedReceiver<PersistenceMsg>,
        mpsc::UnboundedReceiver<SessionCommand>,
    ) {
        let (gateway_tx, gateway_rx) = mpsc::unbounded_channel();
        let gateway = xai_acp_lib::AcpAgentGatewaySender::new(gateway_tx);
        let (session_cmd_tx, session_cmd_rx) = mpsc::unbounded_channel();
        let (persistence_tx, persistence_rx) = mpsc::unbounded_channel();
        let config = NotificationBridgeConfig {
            gateway,
            session_id: acp::SessionId::new("test-session"),
            hunk_tracker_handle: HunkTrackerHandle::noop(),
            file_state_tracker: Arc::new(FileStateTracker::new()),
            prompt_index: Arc::new(TokioMutex::new(0)),
            cwd: PathBuf::from("/tmp"),
            gateway_enabled: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            persistence: PersistenceHandle::from_sender_for_test(persistence_tx),
            incremental_bash_output: false,
            plan_mode: Arc::new(parking_lot::Mutex::new(
                crate::session::plan_mode::PlanModeTracker::new(PathBuf::from("/tmp/test-session")),
            )),
            current_prompt_mode: Arc::new(parking_lot::Mutex::new(
                crate::session::plan_mode::PromptMode::Agent,
            )),
            turn_prompt_mode: Arc::new(parking_lot::Mutex::new(
                crate::session::plan_mode::PromptMode::Agent,
            )),
            session_cmd_tx,
            task_completion_reservations:
                xai_grok_tools::reminders::task_completion::TaskCompletionReservations::default(),
            task_wake_suppressed:
                xai_grok_tools::reminders::task_completion::TaskWakeSuppressed::default(),
            synthetic_trace_tx: Arc::new(std::sync::Mutex::new(None)),
            task_output_tool_name: Arc::new(std::sync::OnceLock::new()),
            read_tool_name: Arc::new(std::sync::OnceLock::new()),
            auto_wake_enabled: true,
            queue_exit_reminder_on_approved_exit: Arc::new(std::sync::atomic::AtomicBool::new(
                false,
            )),
            goal_loop_active: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };
        (config, gateway_rx, persistence_rx, session_cmd_rx)
    }
    fn make_task_snapshot(task_id: &str, kind: TaskKind) -> TaskSnapshot {
        TaskSnapshot {
            task_id: task_id.into(),
            command: "echo test".into(),
            display_command: None,
            cwd: String::new(),
            start_time: std::time::SystemTime::now(),
            end_time: Some(std::time::SystemTime::now()),
            output: String::new(),
            output_file: PathBuf::new(),
            truncated: false,
            exit_code: Some(0),
            signal: None,
            completed: true,
            kind,
            block_waited: false,
            explicitly_killed: false,
            owner_session_id: None,
        }
    }
    /// A completion delivered on this bridge's handle but owned by another,
    /// still-live session (shared terminal backend / reparented task) must be
    /// queued on the owner's channel — not auto-woken here.
    #[tokio::test]
    async fn task_completed_owned_by_another_session_routes_to_it() {
        let (config, mut cmd_rx) = make_test_config();
        let (owner_tx, mut owner_rx) = mpsc::unbounded_channel();
        let (owner_persistence_tx, _owner_persistence_rx) = mpsc::unbounded_channel();
        crate::session::delivery::register(crate::session::delivery::SessionDeliveryTarget {
            session_id: "child-session".to_string(),
            cmd_tx: owner_tx,
            persistence_tx: owner_persistence_tx,
            mcp_state: std::sync::Weak::new(),
            push_stats: Arc::new(parking_lot::Mutex::new(Default::default())),
            subscription_registry: Arc::new(parking_lot::Mutex::new(Default::default())),
            task_completion_reservations:
                xai_grok_tools::reminders::task_completion::TaskCompletionReservations::default(),
        });

        let mut snapshot = make_task_snapshot("bg-child", TaskKind::Bash);
        snapshot.owner_session_id = Some("child-session".to_string());
        let mut state = BridgeState::default();
        handle_notification(
            &config,
            ToolNotification::TaskCompleted(snapshot),
            &mut state,
        )
        .await;

        let command = owner_rx
            .try_recv()
            .expect("the owner must receive the completion");
        match command {
            SessionCommand::InjectNotification {
                prompt_id, source, ..
            } => {
                assert_eq!(prompt_id, "bash-completed-bg-child");
                assert!(matches!(
                    source,
                    NotificationSource::BashTaskCompleted { .. }
                ));
            }
            _ => panic!("expected InjectNotification"),
        }
        match cmd_rx.try_recv() {
            Ok(SessionCommand::Prompt { .. }) => panic!("holder got a wake Prompt"),
            Ok(SessionCommand::InjectNotification { .. }) => {
                panic!("holder got an InjectNotification")
            }
            Ok(_) => panic!("holder got another command"),
            Err(_) => {}
        }
        let target = crate::session::delivery::resolve("child-session").expect("registered");
        assert!(
            target.task_completion_reservations.contains("bg-child"),
            "the owner's reservation must be set so its reminder does not duplicate"
        );
        crate::session::delivery::unregister("child-session");
    }

    /// A routed frame must be addressed to its owner in *both* places that name
    /// a session: `sessionId` and the `_meta.eventId` prefix. Clients parse the
    /// counter suffix only, but a frame whose prefix disagrees with its own
    /// `sessionId` reads as a foreign event in the owner's transcript.
    #[tokio::test]
    async fn routed_completion_frame_carries_the_owner_identity() {
        let (config, mut gateway_rx, _persistence_rx, _cmd_rx) = make_test_config_full();
        let (owner_tx, mut owner_rx) = mpsc::unbounded_channel();
        let (owner_persistence_tx, _owner_persistence_rx) = mpsc::unbounded_channel();
        crate::session::delivery::register(crate::session::delivery::SessionDeliveryTarget {
            session_id: "child-session".to_string(),
            cmd_tx: owner_tx,
            persistence_tx: owner_persistence_tx,
            mcp_state: std::sync::Weak::new(),
            push_stats: Arc::new(parking_lot::Mutex::new(Default::default())),
            subscription_registry: Arc::new(parking_lot::Mutex::new(Default::default())),
            task_completion_reservations:
                xai_grok_tools::reminders::task_completion::TaskCompletionReservations::default(),
        });

        let mut snapshot = make_task_snapshot("bg-owned", TaskKind::Bash);
        snapshot.owner_session_id = Some("child-session".to_string());
        let mut state = BridgeState::default();
        handle_notification(
            &config,
            ToolNotification::TaskCompleted(snapshot),
            &mut state,
        )
        .await;
        assert!(
            owner_rx.try_recv().is_ok(),
            "the owner must receive the routed completion"
        );

        let frame = loop {
            match gateway_rx.try_recv() {
                Ok(xai_acp_lib::AcpClientMessage::ExtNotification(args)) => {
                    if args.request.method.as_ref() == "x.ai/task_completed" {
                        let json: serde_json::Value =
                            serde_json::from_str(args.request.params.get()).expect("params JSON");
                        break json;
                    }
                }
                Ok(_) => {}
                Err(_) => panic!("routed completion was never forwarded"),
            }
        };
        assert_eq!(
            frame.get("sessionId").and_then(|v| v.as_str()),
            Some("child-session"),
            "the frame belongs to the owner's session"
        );
        let event_id = frame
            .get("_meta")
            .and_then(|m| m.get("eventId"))
            .and_then(|v| v.as_str())
            .expect("frame carries an event id");
        assert!(
            event_id.starts_with("child-session-"),
            "the event id prefix must name the frame's own session, got {event_id}"
        );
        crate::session::delivery::unregister("child-session");
    }

    /// A schedule a subagent created is announced on the subagent, not on the
    /// parent whose scheduler holds it: the card names the owner in both places
    /// that identify a frame.
    #[tokio::test]
    async fn scheduled_created_card_is_routed_to_its_owner() {
        let (config, mut gateway_rx, _persistence_rx, _cmd_rx) = make_test_config_full();
        let (owner_tx, _owner_rx) = mpsc::unbounded_channel();
        let (owner_persistence_tx, _owner_persistence_rx) = mpsc::unbounded_channel();
        crate::session::delivery::register(crate::session::delivery::SessionDeliveryTarget {
            session_id: "child-session".to_string(),
            cmd_tx: owner_tx,
            persistence_tx: owner_persistence_tx,
            mcp_state: std::sync::Weak::new(),
            push_stats: Arc::new(parking_lot::Mutex::new(Default::default())),
            subscription_registry: Arc::new(parking_lot::Mutex::new(Default::default())),
            task_completion_reservations: Default::default(),
        });

        let created = xai_grok_tools::notification::types::ScheduledTaskCreated {
            task_id: "sched-1".to_string(),
            owner_session_id: Some("child-session".to_string()),
            prompt: "watch ci".to_string(),
            human_schedule: "every 5 minutes".to_string(),
            next_fire_at: None,
            generation: "gen-1".to_string(),
            revision: 1,
        };
        let mut state = BridgeState::default();
        handle_notification(
            &config,
            ToolNotification::ScheduledTaskCreated(created),
            &mut state,
        )
        .await;

        let frame = loop {
            match gateway_rx.try_recv() {
                Ok(xai_acp_lib::AcpClientMessage::ExtNotification(args)) => {
                    if args.request.method.as_ref() == "x.ai/scheduled_task_created" {
                        let json: serde_json::Value =
                            serde_json::from_str(args.request.params.get()).expect("params JSON");
                        break json;
                    }
                }
                Ok(_) => {}
                Err(_) => panic!("created card was never forwarded"),
            }
        };
        assert_eq!(
            frame.get("sessionId").and_then(|v| v.as_str()),
            Some("child-session"),
            "the card belongs to the creating session"
        );
        let event_id = frame
            .get("_meta")
            .and_then(|m| m.get("eventId"))
            .and_then(|v| v.as_str())
            .expect("frame carries an event id");
        assert!(
            event_id.starts_with("child-session-"),
            "the event id prefix must name the frame's own session, got {event_id}"
        );
        crate::session::delivery::unregister("child-session");
    }

    /// A dead owner falls back to the bridge's own session, like the fire.
    #[tokio::test]
    async fn scheduled_created_card_without_owner_stays_local() {
        let (config, mut gateway_rx, _persistence_rx, _cmd_rx) = make_test_config_full();
        let created = xai_grok_tools::notification::types::ScheduledTaskCreated {
            task_id: "sched-2".to_string(),
            owner_session_id: Some("gone-session".to_string()),
            prompt: "watch ci".to_string(),
            human_schedule: "every 5 minutes".to_string(),
            next_fire_at: None,
            generation: "gen-2".to_string(),
            revision: 2,
        };
        let mut state = BridgeState::default();
        handle_notification(
            &config,
            ToolNotification::ScheduledTaskCreated(created),
            &mut state,
        )
        .await;

        let frame = loop {
            match gateway_rx.try_recv() {
                Ok(xai_acp_lib::AcpClientMessage::ExtNotification(args)) => {
                    if args.request.method.as_ref() == "x.ai/scheduled_task_created" {
                        let json: serde_json::Value =
                            serde_json::from_str(args.request.params.get()).expect("params JSON");
                        break json;
                    }
                }
                Ok(_) => {}
                Err(_) => panic!("created card was never forwarded"),
            }
        };
        assert_eq!(
            frame.get("sessionId").and_then(|v| v.as_str()),
            Some("test-session"),
            "an owner that is not a live session falls back to this bridge"
        );
    }

    /// A monitor event that lands on a bridge which is not the owner is routed to
    /// the owner instead of being dropped: the owner's conversation is where the
    /// event belongs, and its reservations decide whether the model still needs the
    /// inject.
    #[tokio::test]
    async fn monitor_event_is_routed_to_its_owner() {
        let (config, mut gateway_rx, _persistence_rx, mut bridge_cmd_rx) = make_test_config_full();
        let (owner_tx, mut owner_rx) = mpsc::unbounded_channel();
        let (owner_persistence_tx, _owner_persistence_rx) = mpsc::unbounded_channel();
        crate::session::delivery::register(crate::session::delivery::SessionDeliveryTarget {
            session_id: "child-session".to_string(),
            cmd_tx: owner_tx,
            persistence_tx: owner_persistence_tx,
            mcp_state: std::sync::Weak::new(),
            push_stats: Arc::new(parking_lot::Mutex::new(Default::default())),
            subscription_registry: Arc::new(parking_lot::Mutex::new(Default::default())),
            task_completion_reservations: Default::default(),
        });

        let event = xai_grok_tools::notification::types::MonitorEvent {
            task_id: "mon-1".to_string(),
            description: "errors in deploy.log".to_string(),
            event_text: "<monitor-event>boom</monitor-event>".to_string(),
            raw_text: "boom".to_string(),
            owner_session_id: Some("child-session".to_string()),
        };
        let mut state = BridgeState::default();
        handle_notification(&config, ToolNotification::MonitorEvent(event), &mut state).await;

        let frame = loop {
            match gateway_rx.try_recv() {
                Ok(xai_acp_lib::AcpClientMessage::ExtNotification(args)) => {
                    if args.request.method.as_ref() == "x.ai/monitor_event" {
                        let json: serde_json::Value =
                            serde_json::from_str(args.request.params.get()).expect("params JSON");
                        break json;
                    }
                }
                Ok(_) => {}
                Err(_) => panic!("monitor event was never forwarded"),
            }
        };
        assert_eq!(
            frame.get("sessionId").and_then(|v| v.as_str()),
            Some("child-session"),
            "the monitor frame belongs to the owning session"
        );
        assert!(
            matches!(
                owner_rx.try_recv(),
                Ok(SessionCommand::InjectNotification { .. })
            ),
            "the model inject must land in the owner's queue"
        );
        assert!(
            !matches!(
                bridge_cmd_rx.try_recv(),
                Ok(SessionCommand::InjectNotification { .. })
            ),
            "the bridge's own session must not also inject it"
        );
        crate::session::delivery::unregister("child-session");
    }

    /// A routed card is persisted to the *owner's* transcript, not the holder's:
    /// the frame names the owner, and the owner is where it must survive a reload.
    #[tokio::test]
    async fn routed_card_persists_to_the_owner() {
        let (config, mut gateway_rx, mut bridge_persistence_rx, _cmd_rx) = make_test_config_full();
        let (owner_tx, _owner_rx) = mpsc::unbounded_channel();
        let (owner_persistence_tx, mut owner_persistence_rx) = mpsc::unbounded_channel();
        crate::session::delivery::register(crate::session::delivery::SessionDeliveryTarget {
            session_id: "child-session".to_string(),
            cmd_tx: owner_tx,
            persistence_tx: owner_persistence_tx,
            mcp_state: std::sync::Weak::new(),
            push_stats: Arc::new(parking_lot::Mutex::new(Default::default())),
            subscription_registry: Arc::new(parking_lot::Mutex::new(Default::default())),
            task_completion_reservations: Default::default(),
        });

        let created = xai_grok_tools::notification::types::ScheduledTaskCreated {
            task_id: "sched-persist".to_string(),
            owner_session_id: Some("child-session".to_string()),
            prompt: "watch ci".to_string(),
            human_schedule: "every 5 minutes".to_string(),
            next_fire_at: None,
            generation: "gen-1".to_string(),
            revision: 1,
        };
        let mut state = BridgeState::default();
        handle_notification(
            &config,
            ToolNotification::ScheduledTaskCreated(created),
            &mut state,
        )
        .await;

        // Drain the gateway so the frame is definitely built.
        while gateway_rx.try_recv().is_ok() {}
        assert!(
            owner_persistence_rx.try_recv().is_ok(),
            "the owner's transcript must receive the card"
        );
        assert!(
            bridge_persistence_rx.try_recv().is_err(),
            "the holder's transcript must not receive a frame addressed to the owner"
        );
        crate::session::delivery::unregister("child-session");
    }

    /// With no owner stamp the completion stays local (legacy behavior).
    #[tokio::test]
    async fn task_completed_without_owner_stays_local() {
        let (config, mut cmd_rx) = make_test_config();
        config
            .task_output_tool_name
            .set(Some("get_command_or_subagent_output".to_string()))
            .expect("slot is fresh in this test fixture");
        let snapshot = make_task_snapshot("bg-local", TaskKind::Bash);
        let mut state = BridgeState::default();
        handle_notification_with_admission(
            &config,
            ToolNotification::TaskCompleted(snapshot),
            &mut state,
            &mut cmd_rx,
            true,
        )
        .await;
        let command = cmd_rx.try_recv().expect("expected local Prompt");
        assert!(matches!(command, SessionCommand::Prompt { .. }));
    }

    #[tokio::test]
    async fn bash_task_completed_injects_bash_task_completed_source() {
        let (config, mut cmd_rx) = make_test_config();
        config
            .task_output_tool_name
            .set(Some("get_command_or_subagent_output".to_string()))
            .expect("slot is fresh in this test fixture");
        let snapshot = make_task_snapshot("bg-123", TaskKind::Bash);
        let notification = ToolNotification::TaskCompleted(snapshot);
        let mut state = BridgeState::default();
        handle_notification_with_admission(&config, notification, &mut state, &mut cmd_rx, true)
            .await;
        let command = cmd_rx.try_recv().expect("expected Prompt");
        match command {
            SessionCommand::Prompt {
                prompt_id,
                prompt_blocks,
                verbatim,
                ..
            } => {
                assert!(prompt_id.starts_with("task-completed-"));
                assert!(verbatim);
                let text = match &prompt_blocks[0] {
                    acp::ContentBlock::Text(t) => &t.text,
                    _ => panic!("expected text block"),
                };
                assert!(text.contains("bg-123"));
                assert!(text.contains("exit code: 0"));
                assert!(text.contains(r#"get_command_or_subagent_output("bg-123")"#));
                assert!(!text.contains(r#"get_task_output("bg-123")"#));
            }
            _ => panic!("expected Prompt"),
        }
        let cmd3 = cmd_rx
            .try_recv()
            .expect("expected DispatchNotificationHook for task_complete");
        match cmd3 {
            SessionCommand::DispatchNotificationHook {
                notification_type,
                message,
                ..
            } => {
                assert_eq!(notification_type, "task_complete");
                assert_eq!(
                    message.as_deref(),
                    Some("Background task completed: bg-123")
                );
            }
            _ => panic!("expected DispatchNotificationHook"),
        }
    }
    /// Gap 1: while a goal loop is active, a completed background bash task
    /// must NOT fire the synthetic auto-wake prompt — an async "task completed"
    /// wake mid-goal derails a weak model. It must also NOT be marked
    /// reserved (so surface 2's `TaskCompletionReminder` is free to
    /// drain it). The pager's `x.ai/task_completed` notification still fires.
    #[tokio::test]
    async fn bash_task_completed_suppresses_auto_wake_during_goal_loop() {
        let (config, mut gateway_rx, _persistence_rx, mut cmd_rx) = make_test_config_full();
        config
            .task_output_tool_name
            .set(Some("get_command_or_subagent_output".to_string()))
            .expect("slot is fresh in this test fixture");
        config
            .goal_loop_active
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let snapshot = make_task_snapshot("bg-goal", TaskKind::Bash);
        let mut state = BridgeState::default();
        handle_notification(
            &config,
            ToolNotification::TaskCompleted(snapshot),
            &mut state,
        )
        .await;
        match cmd_rx
            .try_recv()
            .expect("expected DispatchNotificationHook for task_complete")
        {
            SessionCommand::DispatchNotificationHook {
                notification_type, ..
            } => {
                assert_eq!(notification_type, "task_complete")
            }
            _ => panic!("unexpected session command"),
        }
        assert!(
            cmd_rx.try_recv().is_err(),
            "goal-loop-active bash completion must not inject auto-wake commands"
        );
        assert!(
            config.task_completion_reservations.snapshot().is_empty(),
            "goal-loop-active completion must not be marked reserved"
        );
        let mut found_ext = false;
        while let Ok(msg) = gateway_rx.try_recv() {
            if let xai_acp_lib::AcpClientMessage::ExtNotification(args) = msg
                && args.request.method.as_ref() == "x.ai/task_completed"
            {
                found_ext = true;
            }
        }
        assert!(
            found_ext,
            "x.ai/task_completed ExtNotification must still be sent for UI"
        );
    }
    /// Gap 1 (preserve non-goal behavior): with the goal loop inactive — the
    /// default for a normal session — a completed bash task DOES fire the
    /// synthetic auto-wake prompt AND is marked reserved so surface
    /// 2 suppresses the duplicate reminder.
    #[tokio::test]
    async fn bash_task_completed_auto_wakes_and_reserves_without_goal_loop() {
        let (config, mut cmd_rx) = make_test_config();
        config
            .task_output_tool_name
            .set(Some("get_command_or_subagent_output".to_string()))
            .expect("slot is fresh in this test fixture");
        let snapshot = make_task_snapshot("bg-normal", TaskKind::Bash);
        let mut state = BridgeState::default();
        handle_notification_with_admission(
            &config,
            ToolNotification::TaskCompleted(snapshot),
            &mut state,
            &mut cmd_rx,
            true,
        )
        .await;
        assert!(matches!(
            cmd_rx.try_recv(),
            Ok(SessionCommand::Prompt { .. })
        ));
        assert!(matches!(
            cmd_rx.try_recv(),
            Ok(SessionCommand::DispatchNotificationHook { .. })
        ));
        assert_eq!(
            config.task_completion_reservations.snapshot(),
            vec!["bg-normal".to_string()],
        );
    }
    fn task_completed_will_wake(
        gateway_rx: &mut mpsc::UnboundedReceiver<xai_acp_lib::AcpClientMessage>,
    ) -> Option<bool> {
        while let Ok(msg) = gateway_rx.try_recv() {
            if let xai_acp_lib::AcpClientMessage::ExtNotification(args) = msg
                && args.request.method.as_ref() == "x.ai/task_completed"
            {
                let v: serde_json::Value = serde_json::from_str(args.request.params.get()).ok()?;
                return v["update"]["will_wake"].as_bool();
            }
        }
        None
    }
    /// The completion notification carries the wake verdict — the pager keys
    /// its between-turns status line on it (skip when a wake response
    /// follows, emit when nothing else will mark the moment).
    #[tokio::test]
    async fn task_completed_notification_stamps_will_wake() {
        let (config, mut gateway_rx, _persistence_rx, mut cmd_rx) = make_test_config_full();
        config
            .task_output_tool_name
            .set(Some("get_command_or_subagent_output".to_string()))
            .expect("slot is fresh in this test fixture");
        let (trace_tx, mut trace_rx) = mpsc::unbounded_channel();
        *config
            .synthetic_trace_tx
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(trace_tx);
        let mut state = BridgeState::default();
        handle_notification_with_admission(
            &config,
            ToolNotification::TaskCompleted(make_task_snapshot("bg-wake", TaskKind::Bash)),
            &mut state,
            &mut cmd_rx,
            true,
        )
        .await;
        assert!(matches!(
            cmd_rx.recv().await,
            Some(SessionCommand::Prompt { .. })
        ));
        match cmd_rx.recv().await {
            Some(SessionCommand::CopyFile { respond_to }) => drop(respond_to),
            _ => panic!("trace copy must follow accepted prompt admission"),
        }
        assert_eq!(
            task_completed_will_wake(&mut gateway_rx),
            Some(true),
            "an auto-woken completion must stamp will_wake: true"
        );
        assert!(
            trace_rx.try_recv().is_ok(),
            "accepted admission must request a synthetic-turn trace"
        );
        let (config, mut gateway_rx, mut persistence_rx, mut cmd_rx) = make_test_config_full();
        let (trace_tx, mut trace_rx) = mpsc::unbounded_channel();
        *config
            .synthetic_trace_tx
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(trace_tx);
        let mut state = BridgeState::default();
        handle_notification_with_admission(
            &config,
            ToolNotification::TaskCompleted(make_task_snapshot("bg-declined", TaskKind::Bash)),
            &mut state,
            &mut cmd_rx,
            false,
        )
        .await;
        assert_eq!(
            task_completed_will_wake(&mut gateway_rx),
            Some(false),
            "an actor-declined completion must stamp will_wake: false"
        );
        assert!(
            config.task_completion_reservations.contains("bg-declined"),
            "the actor owns reservation release after queuing the deferred fallback"
        );
        assert!(
            trace_rx.try_recv().is_err(),
            "declined admission must not request a synthetic-turn trace"
        );
        assert!(matches!(
            cmd_rx.try_recv(),
            Ok(SessionCommand::Prompt { .. })
        ));
        assert!(matches!(
            cmd_rx.try_recv(),
            Ok(SessionCommand::DispatchNotificationHook { .. })
        ));
        let mut persisted = false;
        while let Ok(message) = persistence_rx.try_recv() {
            if let PersistenceMsg::Update(crate::session::storage::SessionUpdate::Xai(update)) =
                message
                && matches!(
                    &update.update,
                    crate::extensions::notification::SessionUpdate::TaskCompleted { .. }
                )
            {
                persisted = true;
            }
        }
        assert!(
            persisted,
            "declined admission must still persist x.ai/task_completed"
        );
    }
    #[tokio::test(start_paused = true)]
    async fn stalled_admission_is_bounded_and_task_completion_still_emits() {
        let (config, mut gateway_rx, mut persistence_rx, mut cmd_rx) = make_test_config_full_raw();
        config
            .task_output_tool_name
            .set(Some("get_command_or_subagent_output".to_string()))
            .expect("slot is fresh in this test fixture");
        let mut state = BridgeState::default();
        let notification = handle_notification(
            &config,
            ToolNotification::TaskCompleted(make_task_snapshot("bg-stalled", TaskKind::Bash)),
            &mut state,
        );
        tokio::pin!(notification);
        tokio::select! {
            _ = &mut notification => panic!("admission should still be waiting"),
            command = cmd_rx.recv() => assert!(matches!(command, Some(SessionCommand::Prompt { .. }))),
        }
        tokio::time::advance(TASK_WAKE_ADMISSION_TIMEOUT + std::time::Duration::from_millis(1))
            .await;
        tokio::task::yield_now().await;
        notification.await;
        assert_eq!(task_completed_will_wake(&mut gateway_rx), Some(false));
        assert!(
            config.task_completion_reservations.contains("bg-stalled"),
            "a timed-out admission may still be handled and deferred by the actor"
        );
        let mut persisted_completion = false;
        while let Ok(message) = persistence_rx.try_recv() {
            if let PersistenceMsg::Update(crate::session::storage::SessionUpdate::Xai(update)) =
                message
                && matches!(
                    &update.update,
                    crate::extensions::notification::SessionUpdate::TaskCompleted { .. }
                )
            {
                persisted_completion = true;
            }
        }
        assert!(persisted_completion);
    }
    #[tokio::test(start_paused = true)]
    async fn timed_out_monitor_admission_queues_one_fallback_and_late_actor_drops_prompt() {
        let (config, mut gateway_rx, _persistence_rx, mut cmd_rx) = make_test_config_full_raw();
        config
            .task_output_tool_name
            .set(Some("get_command_or_subagent_output".to_string()))
            .expect("slot is fresh in this test fixture");
        let mut state = BridgeState::default();
        let notification = handle_notification(
            &config,
            ToolNotification::TaskCompleted(make_task_snapshot("mon-timeout", TaskKind::Monitor)),
            &mut state,
        );
        tokio::pin!(notification);
        let prompt = tokio::select! {
            _ = &mut notification => panic!("admission should still be waiting"),
            command = cmd_rx.recv() => command.expect("prompt command"),
        };
        tokio::time::advance(TASK_WAKE_ADMISSION_TIMEOUT + std::time::Duration::from_millis(1))
            .await;
        tokio::task::yield_now().await;
        notification.await;
        let SessionCommand::Prompt {
            admission: Some(admission),
            respond_to,
            ..
        } = prompt
        else {
            panic!("expected task wake prompt");
        };
        assert!(matches!(
            admission.fallback.source,
            NotificationSource::MonitorCompleted { ref task_id } if task_id == "mon-timeout"
        ));
        assert!(admission.respond_to.send(true).is_err());
        let _ = respond_to.send(Ok(crate::session::commands::PromptTurnOk {
            stop_reason: acp::StopReason::Cancelled,
            total_tokens: 0,
            turn_snapshot: None,
            completion_kind: crate::session::commands::PromptCompletionKind::RemovedFromQueue,
            structured_output: None,
            usage: None,
            tool_overrides: None,
        }));
        assert!(matches!(
            cmd_rx.try_recv(),
            Ok(SessionCommand::DispatchNotificationHook { .. })
        ));
        assert!(cmd_rx.try_recv().is_err());
        assert_eq!(task_completed_will_wake(&mut gateway_rx), Some(false));
        assert!(
            config.task_completion_reservations.contains("mon-timeout"),
            "the late actor fallback retains the reservation until user delivery"
        );
    }
    #[tokio::test]
    async fn task_completed_stamps_will_wake_false_when_session_channel_closed() {
        let (config, mut gateway_rx, _persistence_rx, cmd_rx) = make_test_config_full_raw();
        config
            .task_output_tool_name
            .set(Some("get_command_or_subagent_output".to_string()))
            .expect("slot is fresh in this test fixture");
        drop(cmd_rx);
        config
            .task_completion_reservations
            .reserve("bg-dead".into());
        let mut state = BridgeState::default();
        handle_notification(
            &config,
            ToolNotification::TaskCompleted(make_task_snapshot("bg-dead", TaskKind::Bash)),
            &mut state,
        )
        .await;
        assert_eq!(
            task_completed_will_wake(&mut gateway_rx),
            Some(false),
            "a completion whose wake prompt could not be enqueued must stamp will_wake: false"
        );
        assert!(config.task_completion_reservations.contains("bg-dead"));
        config.task_completion_reservations.release("bg-dead");
        assert!(!config.task_completion_reservations.contains("bg-dead"));
    }
    /// Gap 1 (adjacent branch): the goal-loop arm sits BEFORE the
    /// `auto_wake_enabled == false` `InjectNotification` fallback, so an
    /// auto-wake-DISABLED completion mid-goal must also be suppressed — it must
    /// NOT fall through to the idle-gated `InjectNotification`. Guards against a
    /// future reorder that would leak a mid-goal notification.
    #[tokio::test]
    async fn bash_task_completed_auto_wake_disabled_still_suppressed_during_goal_loop() {
        let (mut config, mut cmd_rx) = make_test_config();
        config.auto_wake_enabled = false;
        config
            .goal_loop_active
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let snapshot = make_task_snapshot("bg-disabled-goal", TaskKind::Bash);
        let mut state = BridgeState::default();
        handle_notification(
            &config,
            ToolNotification::TaskCompleted(snapshot),
            &mut state,
        )
        .await;
        match cmd_rx
            .try_recv()
            .expect("expected DispatchNotificationHook for task_complete")
        {
            SessionCommand::DispatchNotificationHook {
                notification_type, ..
            } => {
                assert_eq!(notification_type, "task_complete")
            }
            _ => panic!("unexpected session command"),
        }
        assert!(
            cmd_rx.try_recv().is_err(),
            "goal-loop-active completion must not InjectNotification with auto-wake disabled"
        );
        assert!(config.task_completion_reservations.snapshot().is_empty());
    }
    /// Natural monitor exit (including exit code 0) must immediate-auto-wake
    /// the same way bash does — not only via the idle-gated MonitorEvent path.
    /// Also drops queued MonitorEvents so a second NotificationDrain turn is
    /// not started for the same completion.
    #[tokio::test]
    async fn monitor_task_completed_auto_wakes_with_monitor_ended_message() {
        let (config, mut cmd_rx) = make_test_config();
        config
            .task_output_tool_name
            .set(Some("get_command_or_subagent_output".to_string()))
            .expect("slot is fresh in this test fixture");
        let mut snapshot = make_task_snapshot("mon-456", TaskKind::Monitor);
        snapshot.display_command = Some("[monitor] watch deploy".into());
        snapshot.command = "tail -f deploy.log".into();
        snapshot.exit_code = Some(0);
        let mut state = BridgeState::default();
        handle_notification_with_admission(
            &config,
            ToolNotification::TaskCompleted(snapshot),
            &mut state,
            &mut cmd_rx,
            true,
        )
        .await;
        let cmd = cmd_rx.try_recv().expect("expected Prompt auto-wake");
        match cmd {
            SessionCommand::Prompt {
                prompt_id,
                prompt_blocks,
                verbatim,
                ..
            } => {
                assert_eq!(prompt_id, "task-completed-mon-456");
                assert!(verbatim);
                let text = match &prompt_blocks[0] {
                    acp::ContentBlock::Text(t) => t.text.as_str(),
                    _ => panic!("expected text block"),
                };
                assert!(
                    text.contains("[monitor ended: exited (code 0)]"),
                    "auto-wake must carry the terminal ended wording: {text}"
                );
                assert!(
                    text.contains("watch deploy"),
                    "auto-wake should include the monitor description: {text}"
                );
                assert!(
                    text.contains("get_command_or_subagent_output(\"mon-456\")"),
                    "auto-wake should point at the poll tool: {text}"
                );
            }
            _ => panic!("expected Prompt auto-wake for natural monitor exit"),
        }
        match cmd_rx
            .try_recv()
            .expect("accepted monitor wake must drop pipeline notifications")
        {
            SessionCommand::DropMonitorNotifications { task_id } => {
                assert_eq!(task_id, "mon-456");
            }
            _ => panic!("expected DropMonitorNotifications after accepted Prompt"),
        }
        assert!(matches!(
            cmd_rx.try_recv(),
            Ok(SessionCommand::DispatchNotificationHook { .. })
        ));
        assert_eq!(
            config.task_completion_reservations.snapshot(),
            vec!["mon-456".to_string()],
        );
    }
    #[tokio::test]
    async fn declined_quiet_monitor_wake_queues_canonical_deferred_completion() {
        let (config, _gateway_rx, mut persistence_rx, mut cmd_rx) = make_test_config_full();
        config
            .task_output_tool_name
            .set(Some("get_command_or_subagent_output".to_string()))
            .expect("slot is fresh in this test fixture");
        let mut state = BridgeState::default();
        handle_notification_with_admission(
            &config,
            ToolNotification::TaskCompleted(make_task_snapshot("mon-declined", TaskKind::Monitor)),
            &mut state,
            &mut cmd_rx,
            false,
        )
        .await;
        assert!(matches!(
            cmd_rx.try_recv(),
            Ok(SessionCommand::Prompt { .. })
        ));
        assert!(matches!(
            cmd_rx.try_recv(),
            Ok(SessionCommand::DispatchNotificationHook { .. })
        ));
        assert!(cmd_rx.try_recv().is_err());
        let mut persisted_completion = false;
        while let Ok(message) = persistence_rx.try_recv() {
            if let PersistenceMsg::Update(crate::session::storage::SessionUpdate::Xai(update)) =
                message
                && matches!(
                    &update.update,
                    crate::extensions::notification::SessionUpdate::TaskCompleted { .. }
                )
            {
                persisted_completion = true;
            }
        }
        assert!(persisted_completion);
        assert!(
            config.task_completion_reservations.contains("mon-declined"),
            "the actor owns reservation release after queuing the deferred fallback"
        );
    }
    /// After TaskCompleted auto-wake reserves the task, late pipeline
    /// MonitorEvents must not inject another model-facing notification.
    #[tokio::test]
    async fn monitor_event_skipped_after_task_completed_auto_wake() {
        let (config, mut cmd_rx) = make_test_config();
        config
            .task_completion_reservations
            .reserve("mon-done".into());
        let mut state = BridgeState::default();
        handle_notification(
            &config,
            ToolNotification::MonitorEvent(xai_grok_tools::notification::types::MonitorEvent {
                task_id: "mon-done".into(),
                description: "short exit".into(),
                event_text: "<monitor-event>done</monitor-event>".into(),
                raw_text: "done".into(),
                owner_session_id: Some("test-session".into()),
            }),
            &mut state,
        )
        .await;
        assert!(
            cmd_rx.try_recv().is_err(),
            "post-auto-wake MonitorEvent must not InjectNotification"
        );
    }
    /// Explicit kill of a monitor still skips auto-wake — the model already
    /// got the kill_task tool result.
    #[tokio::test]
    async fn monitor_explicitly_killed_skips_auto_wake() {
        let (config, mut cmd_rx) = make_test_config();
        let mut snapshot = make_task_snapshot("mon-killed", TaskKind::Monitor);
        snapshot.explicitly_killed = true;
        let mut state = BridgeState::default();
        handle_notification(
            &config,
            ToolNotification::TaskCompleted(snapshot),
            &mut state,
        )
        .await;
        match cmd_rx
            .try_recv()
            .expect("expected DispatchNotificationHook for task_complete")
        {
            SessionCommand::DispatchNotificationHook {
                notification_type, ..
            } => {
                assert_eq!(notification_type, "task_complete")
            }
            _ => panic!("unexpected session command"),
        }
        assert!(
            cmd_rx.try_recv().is_err(),
            "explicitly-killed monitor must not auto-wake"
        );
        assert!(config.task_completion_reservations.snapshot().is_empty());
    }
    /// Goal-loop suppression applies to monitor completions too.
    #[tokio::test]
    async fn monitor_task_completed_suppressed_during_goal_loop() {
        let (config, mut cmd_rx) = make_test_config();
        config
            .goal_loop_active
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let snapshot = make_task_snapshot("mon-goal", TaskKind::Monitor);
        let mut state = BridgeState::default();
        handle_notification(
            &config,
            ToolNotification::TaskCompleted(snapshot),
            &mut state,
        )
        .await;
        match cmd_rx
            .try_recv()
            .expect("expected DispatchNotificationHook for task_complete")
        {
            SessionCommand::DispatchNotificationHook {
                notification_type, ..
            } => {
                assert_eq!(notification_type, "task_complete")
            }
            _ => panic!("unexpected session command"),
        }
        assert!(
            cmd_rx.try_recv().is_err(),
            "goal-loop-active monitor completion must not auto-wake"
        );
        assert!(config.task_completion_reservations.snapshot().is_empty());
    }
    #[tokio::test]
    async fn scheduled_task_created_is_persisted() {
        let (config, _gateway_rx, mut persistence_rx, _cmd_rx) = make_test_config_full();
        let notification = ToolNotification::ScheduledTaskCreated(
            xai_grok_tools::notification::types::ScheduledTaskCreated {
                task_id: "loop-1".into(),
                owner_session_id: None,
                prompt: "check deploy".into(),
                human_schedule: "every 5 minutes".into(),
                next_fire_at: Some("2026-01-01T00:00:00Z".into()),
                generation: "generation-a".into(),
                revision: 1,
            },
        );
        let mut state = BridgeState::default();
        handle_notification(&config, notification, &mut BridgeState::default()).await;
        let msg = persistence_rx
            .try_recv()
            .expect("scheduled_task_created must be persisted");
        match msg {
            PersistenceMsg::Update(crate::session::storage::SessionUpdate::Xai(notif)) => {
                assert!(matches!(
                    &notif.update,
                    crate::extensions::notification::SessionUpdate::ScheduledTaskCreated { .. }
                ));
                let meta = notif.meta.as_ref().expect("scheduler metadata");
                assert_eq!(meta["x.ai/schedulerGeneration"], "generation-a");
                assert_eq!(meta["x.ai/schedulerRevision"], 1);
                assert!(
                    notif
                        .meta
                        .as_ref()
                        .and_then(|m| m.get("eventId"))
                        .and_then(|v| v.as_str())
                        .is_some_and(|id| id.starts_with("test-session-")),
                    "persisted xAI bridge lines must carry an eventId"
                );
            }
            _ => panic!("expected PersistenceMsg::Update(Xai(ScheduledTaskCreated))"),
        }
    }
    /// Persisted⇒stamped contract at the bridge's highest-frequency emitter:
    /// the persisted bash-output line carries an `eventId`, and the live
    /// broadcast carries the SAME id (the meta is minted before the
    /// persist/broadcast fork — divergent ids would re-deliver the line on a
    /// cursor reconnect).
    #[tokio::test]
    async fn bash_output_chunk_persists_and_broadcasts_one_event_id() {
        let (config, mut gateway_rx, mut persistence_rx, _cmd_rx) = make_test_config_full();
        let notification = ToolNotification::BashOutputChunk(
            xai_grok_tools::notification::types::BashOutputChunk {
                base: xai_grok_tools::notification::types::BashNotificationBase {
                    tool_call_id: "call-1".into(),
                    command: "echo hi".into(),
                    output: b"hi\n".to_vec(),
                    total_bytes: 3,
                    truncated: false,
                    cwd: PathBuf::from("/tmp"),
                },
            },
        );
        let mut state = BridgeState::default();
        handle_notification(&config, notification, &mut BridgeState::default()).await;
        let persisted_id = match persistence_rx.try_recv().expect("chunk must be persisted") {
            PersistenceMsg::Update(crate::session::storage::SessionUpdate::Acp(notif)) => notif
                .meta
                .as_ref()
                .and_then(|m| m.get("eventId"))
                .and_then(|v| v.as_str())
                .expect("persisted ACP bridge lines must carry an eventId")
                .to_string(),
            other => panic!("expected PersistenceMsg::Update(Acp(..)), got {other:?}"),
        };
        let broadcast_id = match gateway_rx.try_recv().expect("chunk must be broadcast") {
            xai_acp_lib::AcpClientMessage::SessionNotification(args) => args
                .request
                .meta
                .as_ref()
                .and_then(|m| m.get("eventId"))
                .and_then(|v| v.as_str())
                .expect("broadcast must carry the eventId")
                .to_string(),
            other => panic!("expected SessionNotification, got {other:?}"),
        };
        assert_eq!(persisted_id, broadcast_id);
    }
    #[tokio::test]
    async fn scheduled_task_removed_is_persisted() {
        let (config, _gateway_rx, mut persistence_rx, _cmd_rx) = make_test_config_full();
        let removed = xai_grok_tools::notification::ScheduledTaskRemoved {
            task_id: "loop-1".into(),
            owner_session_id: None,
            generation: "generation-a".into(),
            revision: 2,
        };
        handle_scheduled_task_removed(&config, removed, None)
            .await
            .unwrap();
        let msg = persistence_rx
            .try_recv()
            .expect("scheduled_task_removed must be persisted");
        match msg {
            PersistenceMsg::Update(crate::session::storage::SessionUpdate::Xai(notif)) => {
                assert!(matches!(
                    &notif.update,
                    crate::extensions::notification::SessionUpdate::ScheduledTaskDeleted { .. }
                ));
                assert!(
                    xai_persisted_event_id(&notif).is_some(),
                    "the persisted deletion line must be stamped"
                );
                let meta = notif.meta.as_ref().expect("scheduler metadata");
                assert_eq!(meta["x.ai/schedulerGeneration"], "generation-a");
                assert_eq!(meta["x.ai/schedulerRevision"], 2);
            }
            _ => panic!("expected PersistenceMsg::Update(Xai(ScheduledTaskDeleted))"),
        }
    }
    #[tokio::test]
    async fn acknowledged_scheduler_removal_appends_before_ack_and_broadcast() {
        let (config, mut gateway_rx, mut persistence_rx, _cmd_rx) = make_test_config_full();
        let removed = xai_grok_tools::notification::ScheduledTaskRemoved {
            task_id: "loop-ack".into(),
            owner_session_id: None,
            generation: "generation-a".into(),
            revision: 17,
        };
        let (acknowledgement, mut receipt) = tokio::sync::oneshot::channel::<Result<(), String>>();
        let persistence = async {
            let PersistenceMsg::AppendUpdateDurablyAndAck {
                update: crate::session::storage::SessionUpdate::Xai(notification),
                respond_to,
            } = persistence_rx.recv().await.expect("durable append")
            else {
                panic!("expected durable scheduler tombstone");
            };
            assert_eq!(notification.meta.unwrap()["x.ai/schedulerRevision"], 17);
            assert!(gateway_rx.try_recv().is_err());
            assert!(matches!(
                receipt.try_recv(),
                Err(tokio::sync::oneshot::error::TryRecvError::Empty)
            ));
            respond_to.send(Ok(())).unwrap();
            receipt.await.unwrap().unwrap();
        };
        let (result, ()) = tokio::join!(
            handle_scheduled_task_removed(&config, removed, Some(acknowledgement)),
            persistence,
        );
        result.unwrap();
        assert!(matches!(
            gateway_rx.try_recv(),
            Ok(xai_acp_lib::AcpClientMessage::ExtNotification(_))
        ));
    }
    fn xai_persisted_event_id(
        notif: &crate::extensions::notification::SessionNotification,
    ) -> Option<String> {
        notif
            .meta
            .as_ref()
            .and_then(|m| m.get("eventId"))
            .and_then(|v| v.as_str())
            .map(str::to_string)
    }
    /// Per-site stamp pins for the bridge emitters not covered by the
    /// representative chokepoint tests: deleting any one `stamp_event_id`
    /// call must fail a test (an id-less persisted line silently disables
    /// incremental reconnect for the session).
    #[tokio::test]
    async fn task_backgrounded_persisted_line_is_stamped() {
        let (config, _gateway_rx, mut persistence_rx, _cmd_rx) = make_test_config_full();
        let notification = ToolNotification::BashExecutionBackgrounded(
            xai_grok_tools::notification::types::BashExecutionBackgrounded {
                base: xai_grok_tools::notification::types::BashNotificationBase {
                    tool_call_id: "call-bg".into(),
                    command: "sleep 100".into(),
                    output: Vec::new(),
                    total_bytes: 0,
                    truncated: false,
                    cwd: PathBuf::from("/tmp"),
                },
                output_file: PathBuf::from("/tmp/out.log"),
                task_id: "task-bg".into(),
                monitor_description: None,
                description: None,
            },
        );
        let mut state = BridgeState::default();
        handle_notification(&config, notification, &mut BridgeState::default()).await;
        match persistence_rx.try_recv().expect("must persist") {
            PersistenceMsg::Update(crate::session::storage::SessionUpdate::Xai(notif)) => {
                assert!(xai_persisted_event_id(&notif).is_some());
            }
            _ => panic!("expected Xai update"),
        }
    }
    #[tokio::test]
    async fn task_completed_persisted_line_is_stamped() {
        let (config, _gateway_rx, mut persistence_rx, _cmd_rx) = make_test_config_full();
        let snapshot = make_task_snapshot("mon-1", TaskKind::Monitor);
        let mut state = BridgeState::default();
        handle_notification(
            &config,
            ToolNotification::TaskCompleted(snapshot),
            &mut state,
        )
        .await;
        match persistence_rx.try_recv().expect("must persist") {
            PersistenceMsg::Update(crate::session::storage::SessionUpdate::Xai(notif)) => {
                assert!(xai_persisted_event_id(&notif).is_some());
            }
            _ => panic!("expected Xai update"),
        }
    }
    #[tokio::test]
    async fn current_mode_update_persisted_line_is_stamped() {
        let (config, _gateway_rx, mut persistence_rx, _cmd_rx) = make_test_config_full();
        emit_current_mode_update(&config, xai_grok_tools::types::SessionMode::Plan).await;
        match persistence_rx.try_recv().expect("must persist") {
            PersistenceMsg::Update(crate::session::storage::SessionUpdate::Acp(notif)) => {
                assert!(matches!(
                    notif.update,
                    acp::SessionUpdate::CurrentModeUpdate(_)
                ));
                assert!(
                    notif
                        .meta
                        .as_ref()
                        .and_then(|m| m.get("eventId"))
                        .and_then(|v| v.as_str())
                        .is_some(),
                    "the persisted mode line must be stamped"
                );
            }
            _ => panic!("expected Acp update"),
        }
    }
    #[test]
    fn durable_append_mapping_respects_commit_disposition() {
        assert!(
            durable_append_landed(Err(DurableAppendError::Committed(std::io::Error::other(
                "summary failed"
            ),)))
            .is_ok()
        );
        for failure in [
            DurableAppendError::NotCommitted(std::io::Error::other("append failed")),
            DurableAppendError::AcknowledgementLost(std::io::Error::other("lost")),
        ] {
            assert!(durable_append_landed(Err(failure)).is_err());
        }
    }
    #[tokio::test]
    async fn scheduled_task_fired_is_not_persisted() {
        let (config, mut gateway_rx, mut persistence_rx, _cmd_rx) = make_test_config_full();
        let notification = ToolNotification::ScheduledTaskFired(
            xai_grok_tools::notification::types::ScheduledTaskFired {
                owner_session_id: None,
                task_id: "loop-1".into(),
                prompt: "check deploy".into(),
                human_schedule: "every 5 minutes".into(),
                next_fire_at: Some("2026-01-01T00:00:00Z".into()),
                subagent_id: Some("subagent-1".into()),
                generation: "generation-a".into(),
                revision: 3,
            },
        );
        let mut state = BridgeState::default();
        handle_notification(&config, notification, &mut BridgeState::default()).await;
        assert!(
            persistence_rx.try_recv().is_err(),
            "scheduled_task_fired must NOT be persisted (recurring \u{2192} unbounded log growth)"
        );
        let fired = gateway_rx
            .try_recv()
            .expect("scheduled fire must be broadcast");
        let xai_acp_lib::AcpClientMessage::ExtNotification(fired) = fired else {
            panic!("expected scheduler fire notification");
        };
        let value: serde_json::Value = serde_json::from_str(fired.request.params.get()).unwrap();
        assert_eq!(value["_meta"]["x.ai/schedulerGeneration"], "generation-a");
        assert_eq!(value["_meta"]["x.ai/schedulerRevision"], 3);
    }
    fn make_monitor_event_notification(task_id: &str, owner: Option<&str>) -> ToolNotification {
        ToolNotification::MonitorEvent(xai_grok_tools::notification::types::MonitorEvent {
            task_id: task_id.into(),
            description: "errors in deploy.log".into(),
            event_text: format!("<monitor-event task_id=\"{task_id}\">boom</monitor-event>"),
            raw_text: "boom".into(),
            owner_session_id: owner.map(str::to_string),
        })
    }
    #[tokio::test]
    async fn cross_session_monitor_event_is_routed_to_its_owner() {
        let (config, mut gateway_rx, _persistence_rx, mut cmd_rx) = make_test_config_full();
        // Routing requires a *registered* owner: an unknown owner is delivered
        // locally on purpose, so a dead session cannot swallow its events.
        let (owner_tx, mut owner_rx) = mpsc::unbounded_channel();
        let (owner_persistence_tx, _owner_persistence_rx) = mpsc::unbounded_channel();
        crate::session::delivery::register(crate::session::delivery::SessionDeliveryTarget {
            session_id: "other-session".to_string(),
            cmd_tx: owner_tx,
            persistence_tx: owner_persistence_tx,
            mcp_state: std::sync::Weak::new(),
            push_stats: Arc::new(parking_lot::Mutex::new(HashMap::new())),
            subscription_registry: Arc::new(parking_lot::Mutex::new(HashMap::new())),
            task_completion_reservations: Default::default(),
        });
        let notification = make_monitor_event_notification("mon-foreign", Some("other-session"));
        handle_notification(&config, notification, &mut BridgeState::default()).await;
        assert!(
            cmd_rx.try_recv().is_err(),
            "a foreign owner's event must not be injected into this session"
        );
        assert!(
            matches!(
                owner_rx.try_recv().expect("owner must receive the inject"),
                SessionCommand::InjectNotification { .. }
            ),
            "the event must reach its owner"
        );
        // The pager still hears about it: the frame is addressed to the owner,
        // so the transport holder forwards rather than swallows it.
        assert!(matches!(
            gateway_rx.try_recv(),
            Ok(xai_acp_lib::AcpClientMessage::ExtNotification(_))
        ));
        crate::session::delivery::unregister("other-session");
    }
    #[tokio::test]
    async fn same_session_monitor_event_is_injected() {
        let (config, mut cmd_rx) = make_test_config();
        let notification = make_monitor_event_notification("mon-own", Some("test-session"));
        let mut state = BridgeState::default();
        handle_notification(&config, notification, &mut BridgeState::default()).await;
        match cmd_rx
            .try_recv()
            .expect("own-session monitor event must be injected")
        {
            SessionCommand::InjectNotification { source, .. } => match source {
                NotificationSource::MonitorEvent { task_id } => {
                    assert_eq!(task_id, "mon-own")
                }
                _ => panic!("expected MonitorEvent notification source"),
            },
            _ => panic!("expected InjectNotification"),
        }
    }
    #[tokio::test]
    async fn legacy_monitor_event_without_owner_is_injected() {
        let (config, mut cmd_rx) = make_test_config();
        let notification = make_monitor_event_notification("mon-legacy", None);
        let mut state = BridgeState::default();
        handle_notification(&config, notification, &mut BridgeState::default()).await;
        assert!(
            matches!(
                cmd_rx
                    .try_recv()
                    .expect("legacy (no-owner) monitor event must be injected"),
                SessionCommand::InjectNotification {
                    source: NotificationSource::MonitorEvent { .. },
                    ..
                }
            ),
            "legacy monitor event should be injected as a MonitorEvent notification"
        );
    }
    #[tokio::test]
    async fn block_waited_task_skips_auto_wake_prompt() {
        let (config, mut gateway_rx, _persistence_rx, mut cmd_rx) = make_test_config_full();
        let mut snapshot = make_task_snapshot("bg-waited", TaskKind::Bash);
        snapshot.block_waited = true;
        let notification = ToolNotification::TaskCompleted(snapshot);
        let mut state = BridgeState::default();
        handle_notification(&config, notification, &mut BridgeState::default()).await;
        match cmd_rx
            .try_recv()
            .expect("expected DispatchNotificationHook for task_complete")
        {
            SessionCommand::DispatchNotificationHook {
                notification_type, ..
            } => {
                assert_eq!(notification_type, "task_complete")
            }
            _ => panic!("unexpected session command"),
        }
        assert!(
            cmd_rx.try_recv().is_err(),
            "block_waited completion should not send Prompt or InjectNotification"
        );
        let mut found_ext = false;
        while let Ok(msg) = gateway_rx.try_recv() {
            if let xai_acp_lib::AcpClientMessage::ExtNotification(args) = msg
                && args.request.method.as_ref() == "x.ai/task_completed"
            {
                found_ext = true;
            }
        }
        assert!(
            found_ext,
            "x.ai/task_completed ExtNotification must still be sent for UI"
        );
    }
    #[tokio::test]
    async fn explicitly_killed_task_skips_auto_wake_prompt() {
        let (config, mut gateway_rx, _persistence_rx, mut cmd_rx) = make_test_config_full();
        let mut snapshot = make_task_snapshot("bg-killed", TaskKind::Bash);
        snapshot.explicitly_killed = true;
        let notification = ToolNotification::TaskCompleted(snapshot);
        let mut state = BridgeState::default();
        handle_notification(&config, notification, &mut BridgeState::default()).await;
        match cmd_rx
            .try_recv()
            .expect("expected DispatchNotificationHook for task_complete")
        {
            SessionCommand::DispatchNotificationHook {
                notification_type, ..
            } => {
                assert_eq!(notification_type, "task_complete")
            }
            _ => panic!("unexpected session command"),
        }
        assert!(
            cmd_rx.try_recv().is_err(),
            "explicitly_killed completion should not send Prompt or InjectNotification"
        );
        let mut found_ext = false;
        while let Ok(msg) = gateway_rx.try_recv() {
            if let xai_acp_lib::AcpClientMessage::ExtNotification(args) = msg
                && args.request.method.as_ref() == "x.ai/task_completed"
            {
                found_ext = true;
            }
        }
        assert!(
            found_ext,
            "x.ai/task_completed ExtNotification must still be sent for UI"
        );
    }
    #[tokio::test]
    async fn bash_task_completed_falls_back_when_auto_wake_disabled() {
        let (mut config, mut cmd_rx) = make_test_config();
        config.auto_wake_enabled = false;
        config
            .task_output_tool_name
            .set(Some("get_command_or_subagent_output".to_string()))
            .expect("slot is fresh in this test fixture");
        let snapshot = make_task_snapshot("bg-disabled", TaskKind::Bash);
        let notification = ToolNotification::TaskCompleted(snapshot);
        let mut state = BridgeState::default();
        handle_notification(&config, notification, &mut BridgeState::default()).await;
        let cmd = cmd_rx.try_recv().expect("expected InjectNotification");
        match cmd {
            SessionCommand::InjectNotification {
                prompt_id,
                prompt_blocks,
                priority,
                source,
                ..
            } => {
                assert!(prompt_id.starts_with("bash-completed-"));
                assert_eq!(priority, NotificationPriority::Later);
                assert!(matches!(
                    source,
                    NotificationSource::BashTaskCompleted { ref task_id } if task_id == "bg-disabled"
                ));
                let text = match &prompt_blocks[0] {
                    acp::ContentBlock::Text(t) => &t.text,
                    _ => panic!("expected text block"),
                };
                assert!(text.contains(r#"get_command_or_subagent_output("bg-disabled")"#));
                assert!(!text.contains(r#"get_task_output("bg-disabled")"#));
                assert!(!text.contains("response:"));
            }
            _ => panic!("expected InjectNotification"),
        }
        let hook_cmd = cmd_rx
            .try_recv()
            .expect("expected DispatchNotificationHook for task_complete");
        match hook_cmd {
            SessionCommand::DispatchNotificationHook {
                notification_type,
                message,
                ..
            } => {
                assert_eq!(notification_type, "task_complete");
                assert_eq!(
                    message.as_deref(),
                    Some("Background task completed: bg-disabled")
                );
            }
            _ => panic!("expected DispatchNotificationHook"),
        }
    }
    /// A completed `wait_for` watcher wakes with wait wording and its own
    /// `wait-completed-` fallback id, never the bash one.
    #[tokio::test]
    async fn wait_task_completed_uses_wait_wording_and_prompt_id() {
        let (mut config, mut cmd_rx) = make_test_config();
        config.auto_wake_enabled = false;
        let mut snapshot = make_task_snapshot("wait-abc", TaskKind::Wait);
        snapshot.command = "curl -sf localhost:3000".into();
        snapshot.exit_code = None;
        snapshot.signal = Some("timeout".into());
        let notification = ToolNotification::TaskCompleted(snapshot);
        let mut state = BridgeState::default();
        handle_notification(&config, notification, &mut state).await;
        let cmd = cmd_rx.try_recv().expect("expected InjectNotification");
        match cmd {
            SessionCommand::InjectNotification {
                prompt_id,
                prompt_blocks,
                ..
            } => {
                assert!(prompt_id.starts_with("wait-completed-"), "{prompt_id}");
                assert!(prompt_id.ends_with("wait-abc"), "{prompt_id}");
                let text = match &prompt_blocks[0] {
                    acp::ContentBlock::Text(t) => &t.text,
                    _ => panic!("expected text block"),
                };
                assert!(text.contains("expired"), "deadline wording: {text}");
                assert!(!text.contains("satisfied"), "{text}");
                assert!(text.contains("curl -sf localhost:3000"), "{text}");
            }
            _ => panic!("expected InjectNotification"),
        }
    }
    /// A satisfied wait (exit code 0, no signal) reads as a success.
    #[tokio::test]
    async fn satisfied_wait_task_completed_says_satisfied() {
        let (mut config, mut cmd_rx) = make_test_config();
        config.auto_wake_enabled = false;
        let mut snapshot = make_task_snapshot("wait-ok", TaskKind::Wait);
        snapshot.exit_code = Some(0);
        snapshot.signal = None;
        let notification = ToolNotification::TaskCompleted(snapshot);
        let mut state = BridgeState::default();
        handle_notification(&config, notification, &mut state).await;
        match cmd_rx.try_recv().expect("expected InjectNotification") {
            SessionCommand::InjectNotification { prompt_blocks, .. } => {
                let text = match &prompt_blocks[0] {
                    acp::ContentBlock::Text(t) => &t.text,
                    _ => panic!("expected text block"),
                };
                assert!(text.contains("satisfied"), "{text}");
                assert!(!text.contains("expired"), "{text}");
            }
            _ => panic!("expected InjectNotification"),
        }
    }
    #[tokio::test]
    async fn bash_completion_uses_single_task_id_clone() {
        let (config, mut cmd_rx) = make_test_config();
        let snapshot = make_task_snapshot("unique-id-789", TaskKind::Bash);
        let notification = ToolNotification::TaskCompleted(snapshot);
        let mut state = BridgeState::default();
        handle_notification_with_admission(&config, notification, &mut state, &mut cmd_rx, true)
            .await;
        let cmd = cmd_rx.try_recv().unwrap();
        if let SessionCommand::Prompt { prompt_id, .. } = cmd {
            assert_eq!(prompt_id, "task-completed-unique-id-789");
        } else {
            panic!("expected Prompt");
        }
    }
    fn extract_current_mode_id(notification: &acp::SessionNotification) -> Option<&str> {
        match &notification.update {
            acp::SessionUpdate::CurrentModeUpdate(cmu) => Some(cmu.current_mode_id.0.as_ref()),
            _ => None,
        }
    }
    /// Regression: `PlanModeExited` must emit `CurrentModeUpdate("default")`
    /// onto both the gateway and the persistence stream. Without this,
    /// agent-driven plan approvals leave the TUI stuck in plan mode.
    #[tokio::test]
    async fn plan_mode_exited_emits_current_mode_update_default() {
        let (config, mut gateway_rx, mut persistence_rx, _cmd_rx) = make_test_config_full();
        {
            let mut tracker = config.plan_mode.lock();
            assert!(tracker.activate_from_tool());
        }
        *config.current_prompt_mode.lock() = crate::session::plan_mode::PromptMode::Plan;
        *config.turn_prompt_mode.lock() = crate::session::plan_mode::PromptMode::Plan;
        let notification =
            ToolNotification::PlanModeExited(xai_grok_tools::notification::types::PlanModeExited {
                tool_call_id: "tc-exit-1".into(),
                plan_content: Some("- step 1".into()),
                plan_file_path: "/tmp/test-session/plan.md".into(),
            });
        let mut state = BridgeState::default();
        handle_notification(&config, notification, &mut BridgeState::default()).await;
        let mut gateway_modes = Vec::new();
        while let Ok(msg) = gateway_rx.try_recv() {
            if let xai_acp_lib::AcpClientMessage::SessionNotification(args) = msg
                && let Some(id) = extract_current_mode_id(&args.request)
            {
                gateway_modes.push(id.to_string());
            }
        }
        assert_eq!(
            gateway_modes,
            vec!["default".to_string()],
            "PlanModeExited should emit exactly one CurrentModeUpdate(default) to the gateway"
        );
        let mut persisted_modes = Vec::new();
        while let Ok(msg) = persistence_rx.try_recv() {
            if let PersistenceMsg::Update(crate::session::storage::SessionUpdate::Acp(notif)) = msg
                && let Some(id) = extract_current_mode_id(&notif)
            {
                persisted_modes.push(id.to_string());
            }
        }
        assert_eq!(
            persisted_modes,
            vec!["default".to_string()],
            "PlanModeExited should persist exactly one CurrentModeUpdate(default)"
        );
        assert!(matches!(
            *config.current_prompt_mode.lock(),
            crate::session::plan_mode::PromptMode::Agent
        ));
    }
    /// Default (grok) polarity: the exit_plan_mode tool result is the model's
    /// only exit signal, so an approved `PlanModeExited` must NOT arm the
    /// deferred exit reminder — in memory or in the persisted snapshot.
    /// Sibling of `plan_mode_exited_arms_exit_reminder_when_gated`.
    #[tokio::test]
    async fn plan_mode_exited_does_not_arm_exit_reminder_by_default() {
        let (config, _gateway_rx, mut persistence_rx, _cmd_rx) = make_test_config_full();
        {
            let mut tracker = config.plan_mode.lock();
            assert!(tracker.activate_from_tool());
        }
        let notification =
            ToolNotification::PlanModeExited(xai_grok_tools::notification::types::PlanModeExited {
                tool_call_id: "tc-exit-grok".into(),
                plan_content: Some("- step 1".into()),
                plan_file_path: "/tmp/test-session/plan.md".into(),
            });
        let mut state = BridgeState::default();
        handle_notification(&config, notification, &mut BridgeState::default()).await;
        assert!(
            !config.plan_mode.lock().has_pending_exit_reminder(),
            "approved exit must not arm the deferred exit reminder"
        );
        let mut persisted_plan_snapshots = Vec::new();
        while let Ok(msg) = persistence_rx.try_recv() {
            if let PersistenceMsg::PlanModeState(snapshot) = msg {
                persisted_plan_snapshots.push(snapshot);
            }
        }
        assert!(
            !persisted_plan_snapshots.is_empty()
                && persisted_plan_snapshots
                    .iter()
                    .all(|s| !s.pending_exit_reminder),
            "persisted plan-mode snapshot must not carry the exit reminder"
        );
    }
    /// Gated counterpart: when `queue_exit_reminder_on_approved_exit` is
    /// set, an approved `PlanModeExited` must arm the next-turn exit
    /// reminder and persist it.
    #[tokio::test]
    async fn plan_mode_exited_arms_exit_reminder_when_gated() {
        let (config, _gateway_rx, mut persistence_rx, _cmd_rx) = make_test_config_full();
        config
            .queue_exit_reminder_on_approved_exit
            .store(true, std::sync::atomic::Ordering::Relaxed);
        {
            let mut tracker = config.plan_mode.lock();
            assert!(tracker.activate_from_tool());
        }
        let notification =
            ToolNotification::PlanModeExited(xai_grok_tools::notification::types::PlanModeExited {
                tool_call_id: "tc-exit-gated".into(),
                plan_content: Some("- step 1".into()),
                plan_file_path: "/tmp/test-session/plan.md".into(),
            });
        let mut state = BridgeState::default();
        handle_notification(&config, notification, &mut BridgeState::default()).await;
        assert!(
            config.plan_mode.lock().has_pending_exit_reminder(),
            "gated approved exit must arm the next-turn exit reminder"
        );
        let mut persisted_plan_snapshots = Vec::new();
        while let Ok(msg) = persistence_rx.try_recv() {
            if let PersistenceMsg::PlanModeState(snapshot) = msg {
                persisted_plan_snapshots.push(snapshot);
            }
        }
        assert!(
            !persisted_plan_snapshots.is_empty()
                && persisted_plan_snapshots
                    .iter()
                    .all(|s| s.pending_exit_reminder),
            "persisted plan-mode snapshot must carry the armed exit reminder"
        );
    }
    /// Symmetric to the exit test: `PlanModeEntered` emits
    /// `CurrentModeUpdate("plan")`.
    #[tokio::test]
    async fn plan_mode_entered_emits_current_mode_update_plan() {
        let (config, mut gateway_rx, mut persistence_rx, _cmd_rx) = make_test_config_full();
        let notification = ToolNotification::PlanModeEntered(
            xai_grok_tools::notification::types::PlanModeEntered {
                tool_call_id: "tc-enter-1".into(),
            },
        );
        let mut state = BridgeState::default();
        handle_notification(&config, notification, &mut BridgeState::default()).await;
        let mut gateway_modes = Vec::new();
        while let Ok(msg) = gateway_rx.try_recv() {
            if let xai_acp_lib::AcpClientMessage::SessionNotification(args) = msg
                && let Some(id) = extract_current_mode_id(&args.request)
            {
                gateway_modes.push(id.to_string());
            }
        }
        assert_eq!(gateway_modes, vec!["plan".to_string()]);
        let mut persisted_modes = Vec::new();
        while let Ok(msg) = persistence_rx.try_recv() {
            if let PersistenceMsg::Update(crate::session::storage::SessionUpdate::Acp(notif)) = msg
                && let Some(id) = extract_current_mode_id(&notif)
            {
                persisted_modes.push(id.to_string());
            }
        }
        assert_eq!(persisted_modes, vec!["plan".to_string()]);
    }
    /// Build a completed-bash `TaskSnapshot` whose `output` is large enough
    /// to trip the inline-completion truncation cap, with a concrete
    /// `output_file` path so the disk-pointer footer is exercised end-to-end.
    fn make_large_bash_snapshot(task_id: &str, output_file: PathBuf) -> TaskSnapshot {
        TaskSnapshot {
            task_id: task_id.into(),
            command: "yes hello | head -c 20000".into(),
            display_command: None,
            cwd: String::new(),
            start_time: std::time::SystemTime::now(),
            end_time: Some(std::time::SystemTime::now()),
            output: "h".repeat(20_000),
            output_file,
            truncated: true,
            exit_code: Some(0),
            signal: None,
            completed: true,
            kind: TaskKind::Bash,
            block_waited: false,
            explicitly_killed: false,
            owner_session_id: None,
        }
    }
    /// Extract the auto-wake prompt text emitted on the session command channel.
    fn auto_wake_prompt_text(cmd_rx: &mut mpsc::UnboundedReceiver<SessionCommand>) -> String {
        let cmd = cmd_rx.try_recv().expect("expected Prompt");
        match cmd {
            SessionCommand::Prompt { prompt_blocks, .. } => match &prompt_blocks[0] {
                acp::ContentBlock::Text(t) => t.text.clone(),
                _ => panic!("expected text block"),
            },
            _ => panic!("expected Prompt"),
        }
    }
    /// Extract the InjectNotification prompt text emitted on the session
    /// command channel (auto-wake-disabled fallback path).
    fn inject_notification_prompt_text(
        cmd_rx: &mut mpsc::UnboundedReceiver<SessionCommand>,
    ) -> String {
        let cmd = cmd_rx.try_recv().expect("expected InjectNotification");
        match cmd {
            SessionCommand::InjectNotification { prompt_blocks, .. } => match &prompt_blocks[0] {
                acp::ContentBlock::Text(t) => t.text.clone(),
                _ => panic!("expected text block"),
            },
            _ => panic!("expected InjectNotification"),
        }
    }
    /// Bash completion with a large output and no polling tool (compat-harness
    /// toolset) renders the truncation marker AND the disk-pointer footer
    /// pointing the model at `output_file` via the resolved Read tool name.
    /// Covers BOTH the auto-wake branch and the auto-wake-disabled fallback
    /// so the truncation + footer behaviour stays consistent across both
    /// completion-injection paths.
    #[tokio::test]
    async fn bash_completion_renders_disk_pointer_footer_in_both_branches() {
        let output_file = PathBuf::from("/tmp/bg-disk-pointer.log");
        let (config_auto, mut cmd_rx_auto) = make_test_config();
        config_auto
            .read_tool_name
            .set(Some("read_file".to_string()))
            .expect("fresh slot");
        let snapshot = make_large_bash_snapshot("bg-disk-1", output_file.clone());
        let mut state = BridgeState::default();
        handle_notification_with_admission(
            &config_auto,
            ToolNotification::TaskCompleted(snapshot),
            &mut state,
            &mut cmd_rx_auto,
            true,
        )
        .await;
        let prompt = auto_wake_prompt_text(&mut cmd_rx_auto);
        assert!(
            prompt.contains("[Output truncated"),
            "auto-wake: expected truncation marker, got: {prompt}"
        );
        let expected_footer = format!(
            "Use read_file on {} for full content",
            output_file.display()
        );
        assert!(
            prompt.contains(&expected_footer),
            "auto-wake: expected disk-pointer footer `{expected_footer}`, got: {prompt}"
        );
        assert!(
            prompt.contains("bg-disk-1"),
            "auto-wake: prompt must reference task id"
        );
        let (mut config_no_wake, mut cmd_rx_no_wake) = make_test_config();
        config_no_wake.auto_wake_enabled = false;
        config_no_wake
            .read_tool_name
            .set(Some("read_file".to_string()))
            .expect("fresh slot");
        let snapshot = make_large_bash_snapshot("bg-disk-2", output_file.clone());
        let mut state = BridgeState::default();
        handle_notification(
            &config_no_wake,
            ToolNotification::TaskCompleted(snapshot),
            &mut state,
        )
        .await;
        let prompt = inject_notification_prompt_text(&mut cmd_rx_no_wake);
        assert!(
            prompt.contains("[Output truncated"),
            "auto-wake-disabled: expected truncation marker, got: {prompt}"
        );
        let expected_footer = format!(
            "Use read_file on {} for full content",
            output_file.display()
        );
        assert!(
            prompt.contains(&expected_footer),
            "auto-wake-disabled: expected disk-pointer footer `{expected_footer}`, got: {prompt}"
        );
        assert!(
            prompt.contains("bg-disk-2"),
            "auto-wake-disabled: prompt must reference task id"
        );
    }

    /// When the wake prompt cannot reach the session actor (channel closed),
    /// the bridge must release the reminder-suppression reservation so the
    /// per-tool-call completion reminder can still surface the completion,
    /// and must attempt the pending-notification fallback injection.
    #[tokio::test]
    async fn send_failed_wake_releases_reservation() {
        let (config, _gateway_rx, _persistence_rx, cmd_rx) = make_test_config_full();
        config
            .task_output_tool_name
            .set(Some("get_command_or_subagent_output".to_string()))
            .expect("slot is fresh in this test fixture");
        // Close the session command channel: the wake prompt cannot reach the
        // session actor at all. The bridge reserves the completion itself; a
        // SendFailed outcome must release that reservation.
        drop(cmd_rx);

        let snapshot = make_task_snapshot("bg-send-failed", TaskKind::Bash);
        let mut state = BridgeState::default();
        handle_notification(
            &config,
            ToolNotification::TaskCompleted(snapshot),
            &mut state,
        )
        .await;

        assert!(
            !config
                .task_completion_reservations
                .contains("bg-send-failed"),
            "a send-failed wake must release its reminder-suppression reservation"
        );
    }

    // -- Asset transfer jobs ------------------------------------------------

    fn make_asset_job_event(job_id: &str, state: &str, bytes: u64) -> AssetJobEvent {
        AssetJobEvent {
            job_id: job_id.to_string(),
            kind: "upload".to_string(),
            key: "uploads/photo.png".to_string(),
            backend: "s3".to_string(),
            state: state.to_string(),
            bytes_transferred: bytes,
            bytes_total: Some(10_000),
            error: None,
        }
    }

    /// Every `x.ai/asset_job_event` the bridge put on the wire, decoded.
    fn forwarded_asset_job_events(
        gateway_rx: &mut mpsc::UnboundedReceiver<xai_acp_lib::AcpClientMessage>,
    ) -> Vec<serde_json::Value> {
        let mut out = Vec::new();
        while let Ok(msg) = gateway_rx.try_recv() {
            if let xai_acp_lib::AcpClientMessage::ExtNotification(args) = msg
                && args.request.method.as_ref() == "x.ai/asset_job_event"
            {
                if let Ok(value) = serde_json::from_str(args.request.params.get()) {
                    out.push(value);
                }
            }
        }
        out
    }

    /// The pager renders only what reaches it, so a burst of progress frames
    /// must collapse before the ACP hop — otherwise a fast upload floods the
    /// channel and the TUI with one frame per chunk.
    #[tokio::test]
    async fn asset_job_progress_is_coalesced_before_the_acp_hop() {
        let (config, mut gateway_rx, _persistence_rx, _cmd_rx) = make_test_config_full();
        let mut state = BridgeState::default();

        for step in 1..=100u64 {
            handle_notification(
                &config,
                ToolNotification::AssetJobEvent(make_asset_job_event(
                    "job-1",
                    "running",
                    step * 10,
                )),
                &mut state,
            )
            .await;
        }

        let forwarded = forwarded_asset_job_events(&mut gateway_rx);
        assert!(
            forwarded.len() < 100,
            "100 progress frames must coalesce, got {}",
            forwarded.len()
        );
        assert!(
            forwarded.len() <= ASSET_JOB_COALESCE_CAPACITY as usize,
            "the token bucket must cap the burst at its capacity, got {}",
            forwarded.len()
        );
        assert!(!forwarded.is_empty(), "the first frame must always pass");

        let first = &forwarded[0];
        assert_eq!(first["sessionId"].as_str(), Some("test-session"));
        // The variant is `snake_case`-tagged and its fields keep snake_case
        // names — the payload the pager's `handle_asset_job_event` parses.
        assert_eq!(first["update"]["job_id"].as_str(), Some("job-1"));
        assert_eq!(first["update"]["kind"].as_str(), Some("upload"));
        assert_eq!(first["update"]["backend"].as_str(), Some("s3"));
        assert_eq!(first["update"]["state"].as_str(), Some("running"));
    }

    /// Coalescing must never swallow a terminal frame: a dropped completion
    /// leaves the pager row spinning forever.
    #[tokio::test]
    async fn asset_job_terminal_frame_always_reaches_the_client() {
        let (config, mut gateway_rx, _persistence_rx, _cmd_rx) = make_test_config_full();
        let mut state = BridgeState::default();

        for step in 1..=100u64 {
            handle_notification(
                &config,
                ToolNotification::AssetJobEvent(make_asset_job_event(
                    "job-1",
                    "running",
                    step * 10,
                )),
                &mut state,
            )
            .await;
        }
        // Emitted while the bucket is still empty.
        handle_notification(
            &config,
            ToolNotification::AssetJobEvent(make_asset_job_event("job-1", "completed", 10_000)),
            &mut state,
        )
        .await;

        let forwarded = forwarded_asset_job_events(&mut gateway_rx);
        let last = forwarded.last().expect("terminal frame must be forwarded");
        assert_eq!(last["update"]["state"].as_str(), Some("completed"));
        assert_eq!(last["update"]["bytes_transferred"].as_u64(), Some(10_000));
    }

    /// One bucket per job: a burst on one transfer must not starve another.
    #[tokio::test]
    async fn asset_job_coalescing_is_per_job() {
        let (config, mut gateway_rx, _persistence_rx, _cmd_rx) = make_test_config_full();
        let mut state = BridgeState::default();

        for step in 1..=50u64 {
            handle_notification(
                &config,
                ToolNotification::AssetJobEvent(make_asset_job_event(
                    "job-1",
                    "running",
                    step * 10,
                )),
                &mut state,
            )
            .await;
        }
        handle_notification(
            &config,
            ToolNotification::AssetJobEvent(make_asset_job_event("job-2", "running", 7)),
            &mut state,
        )
        .await;

        let forwarded = forwarded_asset_job_events(&mut gateway_rx);
        assert!(
            forwarded
                .iter()
                .any(|v| v["update"]["job_id"].as_str() == Some("job-2")),
            "job-2's first frame must pass despite job-1's burst"
        );
    }

    /// The coalescer is a plain data structure: assert the admit/forget
    /// contract without the async plumbing.
    #[test]
    fn asset_job_coalescer_admits_capacity_then_drops_and_frees_on_terminal() {
        let mut coalescer = AssetJobCoalescer::default();
        let admitted = (1..=100u64)
            .filter(|n| coalescer.admit(&make_asset_job_event("job-1", "running", *n)))
            .count();
        assert_eq!(admitted, ASSET_JOB_COALESCE_CAPACITY as usize);

        assert!(
            coalescer.admit(&make_asset_job_event("job-1", "failed", 100)),
            "a terminal frame bypasses the bucket"
        );
        assert!(
            coalescer.buckets.is_empty(),
            "a terminal frame frees the job's bucket"
        );

        coalescer.admit(&make_asset_job_event("job-1", "running", 1));
        coalescer.forget("job-1");
        assert!(coalescer.buckets.is_empty());
    }
}
