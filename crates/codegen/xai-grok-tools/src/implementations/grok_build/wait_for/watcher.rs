//! The background watcher: polls `until` after the inline phase, then reports completion.
//!
//! The watcher owns no child process. It runs one bounded attempt at a time through the
//! terminal backend (so the condition command inherits the session's shell state), sleeps on a
//! jittered backoff between attempts, and finishes on satisfaction, on the deadline, or on
//! cancel. Completion is reported with the same `TaskCompleted` notification a background
//! command uses, so the existing wake path, admission gate and suppression flags all apply.
//!
//! "Owns no child process" holds **between** attempts. A cancel that lands while an attempt is
//! in flight drops the attempt future; the actor still owns the process, so the watcher asks the
//! backend to kill it by tool call id ([`TerminalBackend::kill_foreground_command_by_tool_call_id`])
//! before it reports `Killed`. The kill is best effort: a backend that does not track foreground
//! commands by tool call id leaves the attempt running until its own budget, which is what every
//! backend did before this call existed.

use std::collections::{HashMap, VecDeque};
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

/// Live watcher registry: `task_id` → entry.
///
/// Held as a session resource so the tool (spawn/forget), the bridge (pager kill) and the
/// `kill_task` tool (model kill) can all reach it without widening the `TerminalBackend` trait.
/// Dropping the registry drops every sender, which is how a session teardown cancels outstanding
/// waits — the watcher's `recv()` resolves to `None` and it exits as cancelled. That only works
/// while the watcher holds the registry **weakly**; see [`spawn_watcher`].
///
/// The registry also carries each watcher's latest [`TaskSnapshot`], because a watcher owns no
/// process and is therefore invisible to the terminal backend. That is what lets
/// `get_command_or_subagent_output("wait-…")` read a live watcher (last attempt so far) and its
/// final state after the wait ended, and what lets `wait_tasks` block on one.
///
/// Cancellation is reported as `explicitly_killed`, matching the monitor/background-task
/// convention: the kill result already told the model, so the wake is suppressed.
#[derive(Default)]
pub struct WaitForRegistry {
    entries: Mutex<HashMap<String, WaitEntry>>,
    /// Snapshots of watchers that already ended, kept so a read after the fact
    /// still resolves instead of reporting an unknown id. Bounded by [`FINISHED_CAP`].
    finished: Mutex<VecDeque<(String, TaskSnapshot)>>,
}

/// One live watcher: how to cancel it, and where it publishes its state.
struct WaitEntry {
    cancel: mpsc::Sender<()>,
    slot: WaitSlot,
}

/// Handles a watcher keeps to publish state and to signal its own completion.
///
/// Cloned from [`WaitForRegistry::register`]; the registry keeps a copy, so a read can observe the
/// same state the watcher writes.
#[derive(Clone)]
pub struct WaitSlot {
    latest: Arc<Mutex<Option<TaskSnapshot>>>,
    done: Arc<tokio::sync::Notify>,
}

impl WaitSlot {
    /// Publish the current state: live during the wait, final on the last call.
    pub fn publish(&self, snapshot: TaskSnapshot) {
        *self.latest.lock().unwrap_or_else(|e| e.into_inner()) = Some(snapshot);
    }

    /// Publish the final state and wake every reader blocked in
    /// [`WaitForRegistry::wait_completed`].
    pub fn finish(&self, snapshot: TaskSnapshot) {
        self.publish(snapshot);
        self.done.notify_waiters();
    }
}

/// How many finished watcher snapshots stay readable after the wait ends.
const FINISHED_CAP: usize = 64;

/// A live watcher's completion signal and its latest published state.
type ReaderHandles = (Arc<tokio::sync::Notify>, Arc<Mutex<Option<TaskSnapshot>>>);

impl std::fmt::Debug for WaitForRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WaitForRegistry")
            .field("live", &self.len())
            .finish()
    }
}

impl WaitForRegistry {
    /// Register a watcher and hand back its publish handles.
    pub fn register(&self, task_id: &str, cancel: mpsc::Sender<()>) -> WaitSlot {
        let slot = WaitSlot {
            latest: Arc::new(Mutex::new(None)),
            done: Arc::new(tokio::sync::Notify::new()),
        };
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                task_id.to_owned(),
                WaitEntry {
                    cancel,
                    slot: slot.clone(),
                },
            );
        slot
    }

    /// Move a finished watcher out of the live set, keeping its last snapshot readable.
    ///
    /// Called by the watcher itself, after it published the final snapshot.
    pub fn forget(&self, task_id: &str) {
        let entry = self
            .entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(task_id);
        let Some(entry) = entry else { return };
        let snapshot = entry
            .slot
            .latest
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let Some(snapshot) = snapshot else { return };
        let mut finished = self.finished.lock().unwrap_or_else(|e| e.into_inner());
        finished.push_back((task_id.to_owned(), snapshot));
        while finished.len() > FINISHED_CAP {
            finished.pop_front();
        }
    }

    /// The watcher's latest state, live or already finished. `None` when the id is unknown.
    pub fn snapshot(&self, task_id: &str) -> Option<TaskSnapshot> {
        let live = self
            .entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(task_id)
            .and_then(|entry| {
                entry
                    .slot
                    .latest
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone()
            });
        if live.is_some() {
            return live;
        }
        self.finished
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .rev()
            .find(|(id, _)| id == task_id)
            .map(|(_, snapshot)| snapshot.clone())
    }

    /// Wait until the watcher finishes, or `timeout` elapses.
    ///
    /// Returns the final snapshot when the wait ended, the live one on timeout, and `None` only
    /// for an unknown id. A reader must treat the result as it would a terminal snapshot: check
    /// `completed` before assuming the wait is over.
    pub async fn wait_completed(&self, task_id: &str, timeout: Duration) -> Option<TaskSnapshot> {
        let deadline = Instant::now() + timeout;
        loop {
            let Some((done, latest)) = self.reader_handles(task_id) else {
                return self.snapshot(task_id);
            };
            // Register as a waiter *before* re-reading, so a `finish()` landing in
            // between cannot be missed.
            let notified = done.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();

            if let Some(snapshot) = latest.lock().unwrap_or_else(|e| e.into_inner()).clone()
                && snapshot.completed
            {
                return Some(snapshot);
            }

            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return self.snapshot(task_id);
            }
            tokio::select! {
                _ = &mut notified => {}
                _ = tokio::time::sleep(remaining) => return self.snapshot(task_id),
            }
        }
    }

    /// The notify + state handles of a live watcher.
    fn reader_handles(&self, task_id: &str) -> Option<ReaderHandles> {
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(task_id)
            .map(|entry| (entry.slot.done.clone(), entry.slot.latest.clone()))
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
            .map(|entry| entry.cancel.clone());
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
    let slot = registry.register(&task_id, cancel_tx);

    let registry_for_task = Arc::downgrade(&registry);
    let task_id_for_closure = task_id.clone();
    let slot_for_task = slot.clone();
    tokio::spawn(async move {
        let (exit, last_output) = run_watcher(
            &backend,
            &cwd,
            &spec,
            &notification_handle,
            &slot_for_task,
            cancel_rx,
        )
        .await;
        let snapshot = completion_snapshot(&spec, exit, last_output);
        // Publish the final state before forgetting, so a read racing the exit
        // finds it either live or in the finished ring, never nowhere.
        slot.finish(snapshot.clone());
        if let Some(registry) = registry_for_task.upgrade() {
            registry.forget(&task_id_for_closure);
        }
        notification_handle.send_task_complete(snapshot);
    });

    task_id
}

async fn run_watcher(
    backend: &Weak<dyn TerminalBackend>,
    cwd: &std::path::Path,
    spec: &WatcherSpec,
    notification_handle: &ToolNotificationHandle,
    slot: &WaitSlot,
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
            _ = cancel_rx.recv() => {
                // The future above is dropped, but the actor owns the process it
                // spawned. Kill it here so a cancel stops the attempt now instead
                // of at its budget; the tool call id is the only id we hold.
                let _ = backend
                    .kill_foreground_command_by_tool_call_id(&spec.tool_call_id)
                    .await;
                return (WatcherExit::Cancelled, last_output);
            }
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
                // Publish the attempt so a read of a live watcher shows what the
                // last probe saw instead of an empty row.
                slot.publish(live_snapshot(spec, last_output.clone(), outcome.exit_code));
                if outcome.satisfied() {
                    return (WatcherExit::Satisfied, last_output);
                }
            }
            Err(error) => {
                last_output = excerpt(&error.to_string());
                slot.publish(live_snapshot(spec, last_output.clone(), None));
            }
        }

        attempt += 1;
        // Never let the backoff eat the whole remaining window. `min(remaining)` alone lets a
        // capped 30s delay sleep straight to the deadline, after which the loop's top-of-iteration
        // deadline check returns `TimedOut` *without another attempt* — so a condition satisfied in
        // that tail window is missed entirely. Reserving a tail keeps the deadline ending on a try.
        let remaining = spec.deadline.saturating_duration_since(Instant::now());
        let sleep_for = next_sleep(delay, spec.retry_jitter_permille, attempt, remaining);
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

/// Budget reserved before the deadline so the final attempt still fits inside it.
const FINAL_ATTEMPT_TAIL: Duration = Duration::from_millis(250);

/// Sleep before the next attempt: jittered backoff, capped so the deadline ends on a try rather
/// than on a sleep. Zero means "attempt now".
pub(crate) fn next_sleep(
    delay: Duration,
    permille: u32,
    attempt: u64,
    remaining: Duration,
) -> Duration {
    jittered(delay, permille, attempt).min(remaining.saturating_sub(FINAL_ATTEMPT_TAIL))
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
    let span = delay.as_nanos() * (permille as u128) / 1000;
    if span == 0 {
        return delay;
    }
    let roll = splitmix64(salt) as u128 % (2 * span + 1);
    let jittered = delay.as_nanos().saturating_add(roll).saturating_sub(span);
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
        end_time: Some(SystemTime::now()),
        exit_code: exit.exit_code(),
        signal: exit.signal().map(str::to_owned),
        completed: true,
        explicitly_killed: exit == WatcherExit::Cancelled,
        output,
        ..snapshot_base(spec)
    }
}

/// The state of a watcher that is still polling: no end time, no signal, the last
/// attempt's output. What a read of a live watcher id shows.
fn live_snapshot(spec: &WatcherSpec, output: String, exit_code: Option<i32>) -> TaskSnapshot {
    TaskSnapshot {
        exit_code,
        output,
        ..snapshot_base(spec)
    }
}

fn snapshot_base(spec: &WatcherSpec) -> TaskSnapshot {
    TaskSnapshot {
        task_id: spec.task_id.clone(),
        command: spec.command.clone(),
        display_command: Some(format!("[wait] {}", spec.command)),
        cwd: spec.cwd.display().to_string(),
        start_time: spec.started_at,
        end_time: None,
        output: String::new(),
        output_file: spec.output_file.clone(),
        truncated: false,
        exit_code: None,
        signal: None,
        completed: false,
        kind: TaskKind::Wait,
        block_waited: false,
        explicitly_killed: false,
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
    use crate::computer::types::TerminalRunResult;
    use crate::notification::types::ToolNotification;
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// One scripted attempt result.
    #[derive(Clone, Copy)]
    enum Script {
        /// Return this exit code immediately.
        Exit(i32),
        /// Stay in flight for this long, then exit 0.
        Hang(Duration),
    }

    /// Backend that plays a scripted sequence of attempts, repeating the last
    /// entry once the script runs out, and records the ids it was asked to kill.
    #[derive(Default)]
    struct ScriptedBackend {
        script: Mutex<VecDeque<Script>>,
        attempts: AtomicU32,
        killed: Mutex<Vec<String>>,
        /// Fires when an attempt starts, so a test can cancel mid-attempt.
        started: tokio::sync::Notify,
    }

    impl ScriptedBackend {
        fn new(script: Vec<Script>) -> Arc<Self> {
            Arc::new(Self {
                script: Mutex::new(script.into()),
                ..Default::default()
            })
        }

        fn attempts(&self) -> u32 {
            self.attempts.load(Ordering::SeqCst)
        }

        fn killed(&self) -> Vec<String> {
            self.killed.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl TerminalBackend for ScriptedBackend {
        async fn run(
            &self,
            request: TerminalRunRequest,
        ) -> Result<TerminalRunResult, ComputerError> {
            self.attempts.fetch_add(1, Ordering::SeqCst);
            self.started.notify_one();
            let script = {
                let mut queue = self.script.lock().unwrap();
                if queue.len() > 1 {
                    queue.pop_front().unwrap()
                } else {
                    *queue.front().unwrap_or(&Script::Exit(1))
                }
            };
            let exit_code = match script {
                Script::Exit(code) => code,
                Script::Hang(hold) => {
                    tokio::time::sleep(hold).await;
                    0
                }
            };
            Ok(TerminalRunResult {
                combined_output: format!("attempt {}", self.attempts()),
                exit_code: Some(exit_code),
                truncated: false,
                signal: None,
                timed_out: false,
                output_file: request.output_file,
                total_bytes: 0,
                pid: None,
            })
        }

        async fn run_background(
            &self,
            _request: TerminalRunRequest,
        ) -> Result<crate::computer::types::BackgroundHandle, ComputerError> {
            unimplemented!()
        }

        async fn get_task(&self, _task_id: &str) -> Option<TaskSnapshot> {
            None
        }

        async fn kill_task(&self, _task_id: &str) -> crate::computer::types::KillOutcome {
            crate::computer::types::KillOutcome::NotFound
        }

        async fn kill_foreground_command_by_tool_call_id(&self, tool_call_id: &str) -> bool {
            self.killed.lock().unwrap().push(tool_call_id.to_owned());
            true
        }

        async fn wait_for_completion(
            &self,
            _task_id: &str,
            _timeout: Option<Duration>,
        ) -> Option<TaskSnapshot> {
            None
        }

        async fn list_tasks(&self) -> Vec<TaskSnapshot> {
            Vec::new()
        }
    }

    fn spec(task_id: &str, deadline_in: Duration, retry: Duration) -> WatcherSpec {
        WatcherSpec {
            task_id: task_id.to_owned(),
            command: "test -f /tmp/ready".to_owned(),
            cwd: PathBuf::from("/tmp"),
            deadline: Instant::now() + deadline_in,
            retry_initial: retry,
            retry_max: retry,
            retry_multiplier: 1,
            retry_jitter_permille: 0,
            attempt_timeout: Duration::from_secs(5),
            output_file: PathBuf::from("/tmp/wait-test.log"),
            tool_call_id: format!("call-{task_id}"),
            owner_session_id: None,
            started_at: SystemTime::now(),
        }
    }

    /// Spawn a watcher against a scripted backend, returning the backend, the
    /// registry and the completion receiver.
    fn harness(
        script: Vec<Script>,
        spec: WatcherSpec,
    ) -> (
        Arc<ScriptedBackend>,
        Arc<WaitForRegistry>,
        mpsc::UnboundedReceiver<ToolNotification>,
    ) {
        let backend = ScriptedBackend::new(script);
        let registry = Arc::new(WaitForRegistry::default());
        let (handle, rx) = ToolNotificationHandle::channel();
        spawn_watcher(
            Arc::downgrade(&(backend.clone() as Arc<dyn TerminalBackend>)),
            registry.clone(),
            PathBuf::from("/tmp"),
            spec,
            handle,
        );
        (backend, registry, rx)
    }

    /// The watcher must retry a failing condition and stop on the first success,
    /// reporting the attempt count through its published snapshot.
    #[tokio::test]
    async fn loop_retries_until_the_condition_holds() {
        let spec = spec(
            "wait-retry",
            Duration::from_secs(10),
            Duration::from_millis(1),
        );
        let task_id = spec.task_id.clone();
        let (backend, registry, mut rx) = harness(
            vec![Script::Exit(1), Script::Exit(1), Script::Exit(0)],
            spec,
        );

        let snapshot = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("the watcher reports completion")
            .expect("a notification");
        let ToolNotification::TaskCompleted(snapshot) = snapshot else {
            panic!("expected TaskCompleted, got {snapshot:?}");
        };

        assert_eq!(snapshot.task_id, task_id);
        assert!(snapshot.completed);
        assert_eq!(snapshot.exit_code, Some(0));
        assert!(snapshot.signal.is_none(), "satisfied carries no signal");
        assert_eq!(backend.attempts(), 3, "two failures then the success");
        assert!(
            registry.is_empty(),
            "a finished watcher leaves the live set"
        );
        assert!(
            registry.snapshot(&task_id).is_some_and(|s| s.completed),
            "the final snapshot stays readable after the wait ends"
        );
    }

    /// A satisfied wait is not a failure: the snapshot must not look killed.
    #[tokio::test]
    async fn satisfied_snapshot_is_not_marked_killed() {
        let spec = spec("wait-ok", Duration::from_secs(10), Duration::from_millis(1));
        let (_backend, _registry, mut rx) = harness(vec![Script::Exit(0)], spec);
        let Some(ToolNotification::TaskCompleted(snapshot)) = rx.recv().await else {
            panic!("expected a completion");
        };
        assert!(!snapshot.explicitly_killed);
        assert!(snapshot.end_time.is_some());
    }

    /// The deadline ends the loop, and it ends *on an attempt*: the reserved tail
    /// keeps the backoff from sleeping straight past it.
    #[tokio::test]
    async fn loop_times_out_on_the_deadline_with_a_last_attempt() {
        let spec = spec(
            "wait-timeout",
            Duration::from_millis(400),
            Duration::from_millis(50),
        );
        let (backend, registry, mut rx) = harness(vec![Script::Exit(1)], spec);

        let Some(ToolNotification::TaskCompleted(snapshot)) = rx.recv().await else {
            panic!("expected a completion");
        };
        assert_eq!(snapshot.signal.as_deref(), Some(TIMEOUT_SIGNAL));
        assert!(snapshot.completed);
        assert!(
            backend.attempts() >= 2,
            "the loop retried before giving up: {} attempts",
            backend.attempts()
        );
        assert!(
            registry.snapshot("wait-timeout").is_some(),
            "a timed-out watcher is still readable"
        );
    }

    /// A cancel that lands while an attempt is in flight drops the attempt future
    /// but must also stop the process the backend already owns.
    #[tokio::test]
    async fn cancel_kills_the_in_flight_attempt() {
        let spec = spec(
            "wait-cancel",
            Duration::from_secs(60),
            Duration::from_millis(1),
        );
        let tool_call_id = spec.tool_call_id.clone();
        // The single attempt hangs far past the cancel.
        let (backend, registry, mut rx) =
            harness(vec![Script::Hang(Duration::from_secs(30))], spec);
        // Cancel only once the attempt is actually in flight, so the kill has a
        // process to reach.
        backend.started.notified().await;
        assert!(registry.cancel("wait-cancel"));

        let started = Instant::now();
        let Some(ToolNotification::TaskCompleted(snapshot)) = rx.recv().await else {
            panic!("expected a completion");
        };
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the cancel must not wait out the attempt: {:?}",
            started.elapsed()
        );
        assert_eq!(snapshot.signal.as_deref(), Some(CANCELLED_SIGNAL));
        assert!(snapshot.explicitly_killed);
        assert_eq!(backend.killed(), vec![tool_call_id]);
    }

    /// `wait_completed` resolves on the completion signal, not by polling.
    #[tokio::test]
    async fn wait_completed_resolves_when_the_watcher_finishes() {
        let spec = spec(
            "wait-read",
            Duration::from_secs(10),
            Duration::from_millis(5),
        );
        let registry = Arc::new(WaitForRegistry::default());
        let backend = ScriptedBackend::new(vec![Script::Exit(1), Script::Exit(0)]);
        let (handle, mut rx) = ToolNotificationHandle::channel();
        spawn_watcher(
            Arc::downgrade(&(backend.clone() as Arc<dyn TerminalBackend>)),
            registry.clone(),
            PathBuf::from("/tmp"),
            spec,
            handle,
        );

        let snapshot = registry
            .wait_completed("wait-read", Duration::from_secs(5))
            .await
            .expect("a readable watcher");
        assert!(snapshot.completed);
        assert_eq!(snapshot.exit_code, Some(0));
        assert!(rx.try_recv().is_ok(), "the wake still goes out");
    }

    /// An unknown id is `None` rather than a wait for the full timeout.
    #[tokio::test]
    async fn wait_completed_on_an_unknown_id_returns_immediately() {
        let registry = WaitForRegistry::default();
        let started = Instant::now();
        assert!(
            registry
                .wait_completed("wait-nope", Duration::from_secs(30))
                .await
                .is_none()
        );
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn finished_snapshots_are_capped() {
        let registry = WaitForRegistry::default();
        for i in 0..FINISHED_CAP + 8 {
            let id = format!("wait-{i}");
            let (_tx, _rx) = mpsc::channel(1);
            let slot = registry.register(&id, _tx);
            slot.finish(snapshot_stub(&id));
            registry.forget(&id);
        }
        assert!(registry.is_empty(), "all of them finished");
        assert!(registry.snapshot("wait-0").is_none(), "the oldest fell off");
        assert!(
            registry
                .snapshot(&format!("wait-{}", FINISHED_CAP + 7))
                .is_some()
        );
    }

    fn snapshot_stub(task_id: &str) -> TaskSnapshot {
        TaskSnapshot {
            task_id: task_id.to_owned(),
            command: "test -f /tmp/ready".to_owned(),
            display_command: None,
            cwd: "/tmp".to_owned(),
            start_time: SystemTime::now(),
            end_time: Some(SystemTime::now()),
            output: String::new(),
            output_file: PathBuf::from("/tmp/wait-test.log"),
            truncated: false,
            exit_code: Some(0),
            signal: None,
            completed: true,
            kind: TaskKind::Wait,
            block_waited: false,
            explicitly_killed: false,
            owner_session_id: None,
        }
    }

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

    /// The backoff must never consume the whole remaining window: the loop's
    /// top-of-iteration deadline check returns `TimedOut` without another attempt, so a
    /// condition satisfied in the tail would be missed. A capped 30s delay against a 20s
    /// window is exactly the live case that exposed this.
    #[test]
    fn next_sleep_reserves_a_final_attempt() {
        let capped = Duration::from_secs(30);

        // Plenty of window: the backoff is used as-is.
        assert_eq!(
            next_sleep(Duration::from_secs(1), 0, 1, Duration::from_secs(60)),
            Duration::from_secs(1)
        );

        // Window shorter than the delay: sleep only up to the reserved tail.
        assert_eq!(
            next_sleep(capped, 0, 5, Duration::from_secs(20)),
            Duration::from_secs(20) - FINAL_ATTEMPT_TAIL
        );

        // Window inside the tail: attempt immediately instead of sleeping past the deadline.
        assert_eq!(
            next_sleep(capped, 0, 5, FINAL_ATTEMPT_TAIL / 2),
            Duration::ZERO
        );
        assert_eq!(next_sleep(capped, 0, 5, FINAL_ATTEMPT_TAIL), Duration::ZERO);

        // Never past the deadline, never negative.
        for millis in [0u64, 1, 100, 249, 250, 251, 1_000, 29_999, 30_000] {
            let remaining = Duration::from_millis(millis);
            let sleep = next_sleep(capped, 100, 3, remaining);
            assert!(sleep <= remaining, "{sleep:?} exceeds {remaining:?}");
        }
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
