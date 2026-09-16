//! The background watcher: polls `until` after the inline phase, then reports completion.
//!
//! The watcher owns no child process. It runs one bounded attempt at a time through the
//! terminal backend (so the condition command inherits the session's shell state), sleeps on a
//! jittered backoff between attempts, and finishes on satisfaction, on the deadline, or on
//! cancel. Completion is reported with the same `TaskCompleted` notification a background
//! command uses, so the existing wake path, admission gate and suppression flags all apply.
//!
//! "Owns no child process" holds **between** attempts. A cancel that lands while an attempt is
//! in flight drops the attempt future but not the process the actor already spawned: that
//! attempt keeps running until its own budget (at most `attempt_timeout`) and keeps writing the
//! shared attempt log. The kill therefore reports `Killed` slightly before the last command
//! actually stops.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant, SystemTime};

use tokio::sync::mpsc;

use crate::computer::types::{
    ComputerError, TaskKind, TaskSnapshot, TerminalBackend, TerminalRunRequest,
};
use crate::notification::ToolNotificationHandle;

use super::types::WaitForError;

/// Signal name stamped on a watcher that ran out of deadline.
pub const TIMEOUT_SIGNAL: &str = "timeout";
/// Signal name stamped on a watcher the model or the user cancelled.
pub const CANCELLED_SIGNAL: &str = "cancelled";

/// Live watcher registry: `task_id` → cancel sender.
///
/// Held as a session resource so the tool (spawn/forget), the bridge (pager kill) and the
/// `kill_task` tool (model kill) can all reach it without widening the `TerminalBackend` trait.
/// Dropping the registry drops every sender, which is how a session teardown cancels outstanding
/// waits — the watcher's `recv()` resolves to `None` and it exits as cancelled. That only works
/// while the watcher holds the registry **weakly**; see [`spawn_watcher`].
///
/// Cancellation is reported as `explicitly_killed`, matching the monitor/background-task
/// convention: the kill result already told the model, so the wake is suppressed.
///
/// Known limitation: a subagent's registry is its own, so a watcher started inside a subagent is
/// reaped when that subagent exits instead of being reparented onto the parent the way a monitor
/// is. Reparenting would need the parent's registry to adopt the entry (owner swap plus a
/// notification-handle swap), which the terminal actor does for processes and this registry does
/// not yet do for waits.
#[derive(Default)]
pub struct WaitForRegistry {
    entries: Mutex<HashMap<String, mpsc::Sender<()>>>,
}

impl std::fmt::Debug for WaitForRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WaitForRegistry")
            .field("live", &self.len())
            .finish()
    }
}

impl WaitForRegistry {
    pub fn register(&self, task_id: &str, cancel: mpsc::Sender<()>) {
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(task_id.to_owned(), cancel);
    }

    pub fn forget(&self, task_id: &str) {
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(task_id);
    }

    /// Signal a live watcher. `false` when the id is unknown or already finished.
    ///
    /// A full channel counts as success: the channel has capacity 1, so a second
    /// cancel arriving before the watcher drains the first would otherwise
    /// report `NotFound` for a live watcher and make the pager drop its row.
    pub fn cancel(&self, task_id: &str) -> bool {
        let entry = self
            .entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(task_id)
            .cloned();
        match entry {
            Some(sender) => match sender.try_send(()) {
                Ok(()) | Err(mpsc::error::TrySendError::Full(())) => true,
                Err(mpsc::error::TrySendError::Closed(())) => false,
            },
            None => false,
        }
    }

    pub fn len(&self) -> usize {
        self.entries.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// What one attempt produced.
#[derive(Debug, Clone)]
pub struct AttemptOutcome {
    pub exit_code: Option<i32>,
    pub output: String,
    pub output_file: PathBuf,
}

impl AttemptOutcome {
    pub fn satisfied(&self) -> bool {
        self.exit_code == Some(0)
    }
}

/// Run one bounded attempt. `auto_background_on_timeout: false` makes the actor kill a hung
/// attempt at `budget` instead of backgrounding it, so the caller keeps control of the loop.
pub(crate) async fn run_attempt(
    backend: &Arc<dyn TerminalBackend>,
    cwd: &std::path::Path,
    command: &str,
    budget: Duration,
    output_file: PathBuf,
    notification_handle: &ToolNotificationHandle,
    tool_call_id: &str,
    owner_session_id: Option<String>,
) -> Result<AttemptOutcome, ComputerError> {
    let result = backend
        .run(TerminalRunRequest {
            command: command.to_owned(),
            working_directory: cwd.to_path_buf(),
            env: HashMap::new(),
            timeout: budget,
            output_byte_limit: 64 * 1024,
            output_file,
            notification_handle: notification_handle.clone(),
            tool_call_id: tool_call_id.to_owned(),
            display_command: Some(command.to_owned()),
            auto_background_on_timeout: false,
            foreground_block_budget: None,
            kind: TaskKind::Wait,
            owner_session_id,
        })
        .await?;
    Ok(AttemptOutcome {
        exit_code: result.exit_code,
        output: result.combined_output,
        output_file: result.output_file,
    })
}

/// Everything the watcher needs that cannot cross a spawn boundary cheaply.
pub struct WatcherSpec {
    pub task_id: String,
    pub command: String,
    pub cwd: PathBuf,
    pub deadline: Instant,
    pub retry_initial: Duration,
    pub retry_max: Duration,
    pub retry_multiplier: u32,
    pub retry_jitter_permille: u32,
    pub attempt_timeout: Duration,
    pub output_file: PathBuf,
    pub tool_call_id: String,
    pub owner_session_id: Option<String>,
    pub started_at: SystemTime,
}

/// How the watcher finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatcherExit {
    Satisfied,
    TimedOut,
    Cancelled,
}

impl WatcherExit {
    fn signal(self) -> Option<&'static str> {
        match self {
            Self::Satisfied => None,
            Self::TimedOut => Some(TIMEOUT_SIGNAL),
            Self::Cancelled => Some(CANCELLED_SIGNAL),
        }
    }

    fn exit_code(self) -> Option<i32> {
        match self {
            Self::Satisfied => Some(0),
            Self::TimedOut | Self::Cancelled => None,
        }
    }
}

/// Spawn the watcher and return its task id. The backend is held weakly, mirroring
/// `run_monitor_pipeline`, so a long wait cannot pin the terminal actor.
///
/// The registry is held **weakly** too. A strong handle would keep the registry (and therefore
/// the cancel senders inside it) alive for the watcher's whole life, so a session teardown that
/// drops the registry could never close the channel and `cancel_rx.recv()` would never resolve
/// to `None`. Weak here means: registry dropped → senders dropped → the watcher exits cancelled.
#[allow(clippy::too_many_arguments)]
pub fn spawn_watcher(
    backend: Weak<dyn TerminalBackend>,
    registry: Arc<WaitForRegistry>,
    cwd: PathBuf,
    spec: WatcherSpec,
    notification_handle: ToolNotificationHandle,
) -> String {
    let task_id = spec.task_id.clone();
    let (cancel_tx, cancel_rx) = mpsc::channel::<()>(1);
    registry.register(&task_id, cancel_tx);

    let registry_for_task = Arc::downgrade(&registry);
    let task_id_for_closure = task_id.clone();
    tokio::spawn(async move {
        let (exit, last_output) =
            run_watcher(&backend, &cwd, &spec, &notification_handle, cancel_rx).await;
        if let Some(registry) = registry_for_task.upgrade() {
            registry.forget(&task_id_for_closure);
        }
        let snapshot = completion_snapshot(&spec, exit, last_output);
        notification_handle.send_task_complete(snapshot);
    });

    task_id
}

async fn run_watcher(
    backend: &Weak<dyn TerminalBackend>,
    cwd: &std::path::Path,
    spec: &WatcherSpec,
    notification_handle: &ToolNotificationHandle,
    mut cancel_rx: mpsc::Receiver<()>,
) -> (WatcherExit, String) {
    let mut delay = spec.retry_initial;
    let mut attempt: u64 = 0;
    let mut last_output = String::new();

    loop {
        let remaining = spec.deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return (WatcherExit::TimedOut, last_output);
        }

        let Some(backend) = backend.upgrade() else {
            return (WatcherExit::Cancelled, last_output);
        };
        let budget = remaining.min(spec.attempt_timeout);
        let attempt_result = tokio::select! {
            biased;
            _ = cancel_rx.recv() => return (WatcherExit::Cancelled, last_output),
            result = run_attempt(
                &backend,
                cwd,
                &spec.command,
                budget,
                spec.output_file.clone(),
                notification_handle,
                &spec.tool_call_id,
                spec.owner_session_id.clone(),
            ) => result,
        };
        drop(backend);

        match attempt_result {
            Ok(outcome) => {
                last_output = excerpt(&outcome.output);
                if outcome.satisfied() {
                    return (WatcherExit::Satisfied, last_output);
                }
            }
            Err(error) => last_output = excerpt(&error.to_string()),
        }

        attempt += 1;
        let sleep_for = jittered(delay, spec.retry_jitter_permille, attempt)
            .min(spec.deadline.saturating_duration_since(Instant::now()));
        if sleep_for.is_zero() {
            continue;
        }
        tokio::select! {
            biased;
            _ = cancel_rx.recv() => return (WatcherExit::Cancelled, last_output),
            _ = tokio::time::sleep(sleep_for) => {}
        }
        delay = (delay * spec.retry_multiplier).min(spec.retry_max);
    }
}

/// Cap for the last-attempt excerpt carried on the completion snapshot.
const SNAPSHOT_OUTPUT_CHARS: usize = 2_048;

fn excerpt(text: &str) -> String {
    let trimmed = text.trim_end();
    if trimmed.chars().count() <= SNAPSHOT_OUTPUT_CHARS {
        return trimmed.to_owned();
    }
    let head: String = trimmed.chars().take(SNAPSHOT_OUTPUT_CHARS).collect();
    format!("{head}\n… [truncated]")
}

/// Apply ±`permille`/1000 jitter deterministically, so tests do not need an RNG.
pub(crate) fn jittered(delay: Duration, permille: u32, salt: u64) -> Duration {
    if permille == 0 || delay.is_zero() {
        return delay;
    }
    let span = (delay.as_nanos() as u128) * (permille as u128) / 1000;
    if span == 0 {
        return delay;
    }
    let roll = splitmix64(salt) as u128 % (2 * span + 1);
    let jittered = (delay.as_nanos() as u128)
        .saturating_add(roll)
        .saturating_sub(span);
    Duration::from_nanos(jittered.min(u64::MAX as u128) as u64)
}

fn splitmix64(seed: u64) -> u64 {
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn completion_snapshot(spec: &WatcherSpec, exit: WatcherExit, output: String) -> TaskSnapshot {
    TaskSnapshot {
        task_id: spec.task_id.clone(),
        command: spec.command.clone(),
        display_command: Some(format!("[wait] {}", spec.command)),
        cwd: spec.cwd.display().to_string(),
        start_time: spec.started_at,
        end_time: Some(SystemTime::now()),
        output,
        output_file: spec.output_file.clone(),
        truncated: false,
        exit_code: exit.exit_code(),
        signal: exit.signal().map(str::to_owned),
        completed: true,
        kind: TaskKind::Wait,
        block_waited: false,
        explicitly_killed: exit == WatcherExit::Cancelled,
        owner_session_id: spec.owner_session_id.clone(),
    }
}

/// Build the watcher id. Stable prefix keeps it greppable in logs and the tasks pane.
pub fn watcher_task_id(call_id: &str) -> String {
    format!("wait-{call_id}")
}

impl From<WaitForError> for xai_tool_runtime::ToolError {
    fn from(error: WaitForError) -> Self {
        xai_tool_runtime::ToolError::invalid_arguments(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_cancel_reports_liveness() {
        let registry = WaitForRegistry::default();
        let (tx, mut rx) = mpsc::channel(1);
        registry.register("wait-1", tx);
        assert_eq!(registry.len(), 1);
        assert!(registry.cancel("wait-1"));
        assert_eq!(rx.try_recv(), Ok(()));
        assert!(!registry.cancel("missing"));
        registry.forget("wait-1");
        assert!(registry.is_empty());
    }

    #[test]
    fn jitter_stays_within_the_band_and_is_deterministic() {
        let base = Duration::from_secs(10);
        for salt in 0..64u64 {
            let value = jittered(base, 100, salt);
            assert!(value >= Duration::from_secs(9), "{value:?} below band");
            assert!(value <= Duration::from_secs(11), "{value:?} above band");
            assert_eq!(value, jittered(base, 100, salt));
        }
    }

    #[test]
    fn jitter_disabled_and_zero_delay_are_identity() {
        assert_eq!(
            jittered(Duration::from_secs(5), 0, 7),
            Duration::from_secs(5)
        );
        assert_eq!(jittered(Duration::ZERO, 100, 7), Duration::ZERO);
    }

    #[test]
    fn exit_codes_and_signals_match_the_outcome() {
        assert_eq!(WatcherExit::Satisfied.exit_code(), Some(0));
        assert_eq!(WatcherExit::Satisfied.signal(), None);
        assert_eq!(WatcherExit::TimedOut.signal(), Some(TIMEOUT_SIGNAL));
        assert_eq!(WatcherExit::Cancelled.signal(), Some(CANCELLED_SIGNAL));
    }

    #[test]
    fn attempt_is_satisfied_only_on_zero() {
        let mut outcome = AttemptOutcome {
            exit_code: Some(0),
            output: String::new(),
            output_file: PathBuf::new(),
        };
        assert!(outcome.satisfied());
        outcome.exit_code = Some(1);
        assert!(!outcome.satisfied());
        outcome.exit_code = None;
        assert!(!outcome.satisfied());
    }
}
