//! Compaction configuration and runtime state for the session actor.

use std::cell::Cell;
use std::cell::RefCell;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU8;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

/// One queued compaction log entry flowing through the async event channel.
#[derive(Debug)]
pub(crate) struct CompactionLogEntry {
    pub(crate) lvl: xai_grok_telemetry::unified_log::LogLevel,
    pub(crate) msg: String,
    pub(crate) sid: Option<String>,
    pub(crate) ctx: Option<serde_json::Value>,
}

/// Process-wide sender for compaction unified-log events.
///
/// Compaction emits progress events (prep stages, generation start/end,
/// completion) from the session actor thread. Writing them synchronously would
/// put file I/O on that thread and inside the compact critical path; instead
/// the sender pushes entries onto a tokio mpsc channel (lock-free send) and a
/// dedicated drain task owns the disk write. Before the worker is installed
/// (unit tests, early startup) events fall back to a direct synchronous write,
/// preserving the pre-channel behavior.
static COMPACTION_EVENT_SINK: OnceLock<tokio::sync::mpsc::UnboundedSender<CompactionLogEntry>> =
    OnceLock::new();

/// Spawn the drain task and install the process-wide compaction event channel.
///
/// Idempotent: only the first install wins; later sessions keep using the
/// original worker. Requires an active tokio runtime (session spawn context).
pub(crate) fn install_compaction_event_worker() {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<CompactionLogEntry>();
    if COMPACTION_EVENT_SINK.set(tx).is_err() {
        return;
    }
    tokio::spawn(async move {
        while let Some(entry) = rx.recv().await {
            xai_grok_telemetry::unified_log::emit(
                entry.lvl,
                &entry.msg,
                entry.sid.as_deref(),
                entry.ctx,
            );
        }
    });
}

/// Queue one compaction log entry through the async event channel.
///
/// Falls back to a direct synchronous write when the worker is not installed
/// or the channel is closed during teardown, so events are never silently
/// dropped in either direction.
pub(crate) fn emit_compaction_event(
    lvl: xai_grok_telemetry::unified_log::LogLevel,
    msg: &str,
    sid: Option<&str>,
    ctx: Option<serde_json::Value>,
) {
    let Some(tx) = COMPACTION_EVENT_SINK.get() else {
        xai_grok_telemetry::unified_log::emit(lvl, msg, sid, ctx);
        return;
    };
    let entry = CompactionLogEntry {
        lvl,
        msg: msg.to_owned(),
        sid: sid.map(str::to_owned),
        ctx,
    };
    if let Err(send_error) = tx.send(entry) {
        // Worker gone (teardown) — recover the entry and write synchronously
        // so the event still lands.
        let entry = send_error.0;
        xai_grok_telemetry::unified_log::emit(
            entry.lvl,
            &entry.msg,
            entry.sid.as_deref(),
            entry.ctx,
        );
    }
}

/// True only for trigger-bearing interactive cancels that are allowed to stop
/// compaction. Send-now, legacy/teardown `None`, and the internal persistence
/// fail-stop keep their existing semantics.
pub(crate) fn is_explicit_compaction_cancel_trigger(trigger: Option<&str>) -> bool {
    trigger.is_some_and(|trigger| {
        trigger != "send_now" && trigger != "compaction_persistence_indeterminate"
    })
}

/// Auto-compaction is gated whenever `auto_compact_suppressed` is not [`SUPPRESS_NONE`].
pub(crate) const SUPPRESS_NONE: u8 = 0;
/// Resolvable failure (`other`): suppressed for the current turn, then
/// cleared at the next turn start so compaction self-heals once the cause clears.
pub(crate) const SUPPRESS_TURN: u8 = 1;
/// Fatal failure (size/schema) retrying can never fix: survives turn boundaries,
/// cleared only when the context budget changes — a successful compaction, a
/// rewind (context shrank), or a model switch (a larger window may now fit).
pub(crate) const SUPPRESS_STICKY: u8 = 2;
/// Credit block: suppress until a model `200` (credits aren't client-observable).
/// Survives turns; context changes can't fix it. Token refresh must not clear this.
pub(crate) const SUPPRESS_UNTIL_SUCCESS: u8 = 3;
/// Auth-expired auto-compact: suppress until login/token refresh, not until 200
/// (waiting for a sample deadlocks when context is already over the window).
pub(crate) const SUPPRESS_AUTH: u8 = 4;

/// Model slug and context window from the previous turn.
#[derive(Clone, Debug)]
pub struct PreviousModelInfo {
    pub model_slug: String,
    pub context_window: u64,
}

/// Cached result of an **async** (background / prefire) pass-1 sample for
/// two-pass compaction. Held on the session actor between the background
/// pass-1 and the synchronous pass-2 apply at compaction time.
#[derive(Clone, Debug)]
pub struct AsyncCompactionCache {
    /// The successor-usable NOTE₁ text (extracted `<summary>` or full pass-1 output).
    pub note1: String,
    /// Number of leading conversation items pass-1 summarized (the prefix
    /// boundary in the LIVE conversation as of pass-1 time). The pass-2 tail is
    /// `conversation[prefix_len..]`.
    pub prefix_len: usize,
    /// Fingerprint of `conversation[..prefix_len]` at pass-1 time. Pass-2 only
    /// applies NOTE₁ when the current conversation still has this exact prefix.
    pub fingerprint: u64,
    /// Model slug pass-1 ran under; invalidated on model switch.
    pub model_slug: String,
    /// Wall time pass-1 took (ms) — latency that ran off the critical path
    /// when prefire finished before compact (not counted in telemetry TTFT unless
    /// the user waited on an in-flight pass-1).
    pub pass1_latency_ms: u64,
}

/// Cancel gate for in-flight compact, prefire, and rolling operations.
///
/// Each top-level operation owns a fresh token. The gate retains every active
/// token so one explicit user stop cancels overlapping operations together,
/// while a new operation started after that stop does not inherit an old
/// cancelled token that is still unwinding.
#[derive(Clone, Default)]
pub struct CompactCancelGate {
    inner: Arc<Mutex<CompactCancelState>>,
}

#[derive(Default)]
struct CompactCancelState {
    active: Vec<(u64, tokio_util::sync::CancellationToken)>,
    next_scope_id: u64,
    cancel_commands_pending: usize,
    cancel_sequence: u64,
}

/// Removes one active cancellation token when its operation ends.
pub struct CompactCancelScope {
    gate: CompactCancelGate,
    scope_id: u64,
}

impl Drop for CompactCancelScope {
    fn drop(&mut self) {
        self.gate.end(self.scope_id);
    }
}

/// Clears an out-of-band promotion barrier unless ownership was transferred to
/// the queued actor command.
pub(crate) struct PendingCompactCancelCommand {
    gate: CompactCancelGate,
    committed: bool,
}

impl PendingCompactCancelCommand {
    /// Transfer responsibility for clearing the barrier to the actor command.
    pub(crate) fn commit(mut self) {
        self.committed = true;
    }
}

impl Drop for PendingCompactCancelCommand {
    fn drop(&mut self) {
        if !self.committed {
            self.gate.clear_cancel_command_pending();
        }
    }
}

impl CompactCancelGate {
    /// Start an independent compaction operation and atomically capture the
    /// cancellation sequence at which its token became visible.
    pub(crate) fn enter_with_sequence(
        &self,
    ) -> (u64, tokio_util::sync::CancellationToken, CompactCancelScope) {
        let (cancel_sequence, scope_id, token) = {
            let mut state = self.inner.lock().expect("compaction cancel gate poisoned");
            state.next_scope_id = state.next_scope_id.wrapping_add(1);
            let scope_id = state.next_scope_id;
            let token = tokio_util::sync::CancellationToken::new();
            state.active.push((scope_id, token.clone()));
            (state.cancel_sequence, scope_id, token)
        };
        (
            cancel_sequence,
            token,
            CompactCancelScope {
                gate: self.clone(),
                scope_id,
            },
        )
    }

    /// Start an independent compaction operation.
    pub fn enter(&self) -> (tokio_util::sync::CancellationToken, CompactCancelScope) {
        let (_, token, scope) = self.enter_with_sequence();
        (token, scope)
    }

    fn end(&self, scope_id: u64) {
        let mut state = self.inner.lock().expect("compaction cancel gate poisoned");
        let old_len = state.active.len();
        state.active.retain(|(id, _)| *id != scope_id);
        debug_assert_eq!(state.active.len() + 1, old_len, "unknown compaction scope");
    }

    /// Cancel every compaction operation currently in flight.
    pub fn request_cancel(&self) {
        let tokens = {
            let state = self.inner.lock().expect("compaction cancel gate poisoned");
            state
                .active
                .iter()
                .map(|(_, token)| token.clone())
                .collect::<Vec<_>>()
        };
        for token in tokens {
            token.cancel();
        }
    }

    /// Mark that an out-of-band user cancellation will queue normal actor
    /// teardown. Rolling-result handling pauses prompt promotion until the
    /// corresponding actor command clears the barrier.
    pub(crate) fn begin_cancel_command(&self) -> PendingCompactCancelCommand {
        let mut state = self.inner.lock().expect("compaction cancel gate poisoned");
        state.cancel_commands_pending = state.cancel_commands_pending.saturating_add(1);
        state.cancel_sequence = state.cancel_sequence.wrapping_add(1);
        PendingCompactCancelCommand {
            gate: self.clone(),
            committed: false,
        }
    }

    pub(crate) fn clear_cancel_command_pending(&self) {
        let mut state = self.inner.lock().expect("compaction cancel gate poisoned");
        state.cancel_commands_pending = state.cancel_commands_pending.saturating_sub(1);
    }

    /// Monotonic user-cancel sequence used to reject a result produced before
    /// a cancel even if its sampling scope has already drained.
    pub fn cancel_sequence(&self) -> u64 {
        self.inner
            .lock()
            .expect("compaction cancel gate poisoned")
            .cancel_sequence
    }

    pub fn cancel_command_pending(&self) -> bool {
        self.inner
            .lock()
            .expect("compaction cancel gate poisoned")
            .cancel_commands_pending
            > 0
    }

    /// Linearize synchronous work against out-of-band cancellation. The
    /// closure must not await; holding this short mutex section prevents a
    /// pending marker from landing between the final sequence check and the
    /// protected action (prompt spawn or durable-CAS enqueue).
    pub(crate) fn run_if_sequence_current<T>(
        &self,
        expected_sequence: u64,
        action: impl FnOnce() -> T,
    ) -> Option<T> {
        let state = self.inner.lock().expect("compaction cancel gate poisoned");
        if state.cancel_commands_pending > 0 || state.cancel_sequence != expected_sequence {
            return None;
        }
        Some(action())
    }

    pub fn is_cancelled(&self) -> bool {
        self.inner
            .lock()
            .expect("compaction cancel gate poisoned")
            .active
            .iter()
            .any(|(_, token)| token.is_cancelled())
    }
}

/// Prefire two-pass state. `Default` so it drops into existing `CompactionConfig`
/// struct literals with a single `prefire: PrefireState::default()` field.
///
/// `SessionActor` is `!Send` and single-threaded; the `AtomicBool` is only used
/// for its ergonomic `compare_exchange` (no cross-thread sharing), and the
/// `RefCell`s need no locking (the `JoinHandle` is from `spawn_local`, so it is
/// local to this LocalSet and never crosses threads).
#[derive(Default)]
pub struct PrefireState {
    /// Set while a background pass-1 sample is running, so the per-turn trigger
    /// never spawns a second concurrent job.
    in_flight: AtomicBool,
    /// Cached async pass-1 result, ready for pass-2 apply (or `None`).
    cache: RefCell<Option<AsyncCompactionCache>>,
    /// Handle to the in-flight background pass-1 task. Pass-2 awaits this when
    /// compaction fires before prefire finished, so a still-running pass-1 is
    /// used rather than discarded for a full single-pass.
    handle: RefCell<Option<tokio::task::JoinHandle<()>>>,
}

impl PrefireState {
    /// Try to claim the single in-flight slot. Returns `true` iff this caller
    /// won the race and should spawn pass-1 (the caller must later call
    /// [`Self::finish`]).
    pub fn try_begin(&self) -> bool {
        self.in_flight
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
    }

    /// Release the in-flight slot (call exactly once after a `try_begin` win).
    pub fn finish(&self) {
        self.in_flight.store(false, Ordering::Release);
    }

    pub fn is_in_flight(&self) -> bool {
        self.in_flight.load(Ordering::Acquire)
    }

    /// Stash the spawned pass-1 task handle so pass-2 can await it if it is
    /// still running when compaction fires.
    pub fn set_handle(&self, handle: tokio::task::JoinHandle<()>) {
        self.handle.replace(Some(handle));
    }

    /// Take the pass-1 task handle, if any, so the caller can await completion
    /// before reading the cache. Leaves `None`.
    pub fn take_handle(&self) -> Option<tokio::task::JoinHandle<()>> {
        self.handle.borrow_mut().take()
    }

    pub fn store(&self, cache: AsyncCompactionCache) {
        self.cache.replace(Some(cache));
    }

    /// Take the cache, leaving `None`.
    pub fn take(&self) -> Option<AsyncCompactionCache> {
        self.cache.borrow_mut().take()
    }

    /// Drop any cached async pass-1 result (invalidation: model switch, rewind,
    /// apply, edits).
    pub fn clear(&self) {
        self.cache.replace(None);
    }

    pub fn has_cache(&self) -> bool {
        self.cache.borrow().is_some()
    }
}

pub struct CompactionConfig {
    /// Context window usage percentage (0-100) at which auto-compact triggers.
    ///
    /// `Cell` so the value can be re-resolved at model-switch time without
    /// holding `&mut self` on the actor. `SessionActor` is `!Send`, so
    /// `Cell` is sufficient (no atomic ordering needed).
    pub threshold_percent: Cell<u8>,
    /// Debug: when set, next auto-compact check triggers unconditionally.
    pub force_compact: Arc<AtomicBool>,
    /// Auto-compaction suppression state (`SUPPRESS_*`) after a deterministic
    /// failure; the gates early-return unless `SUPPRESS_NONE`. Manual `/compact` ignores it.
    pub auto_compact_suppressed: AtomicU8,
    /// Locks the context window when `GROK_DEBUG_CONTEXT_WINDOW` is set.
    pub context_window_override: Option<std::num::NonZeroU64>,
    pub count: AtomicU64,
    /// Set at turn end; consumed at next turn start for model-switch compaction.
    /// `Cell` because `SessionActor` is `!Send`.
    pub previous_model: Cell<Option<PreviousModelInfo>>,
    /// The resolved mode; `Segments` carries its detail level inline.
    pub compaction_mode: xai_chat_state::CompactionMode,
    /// When `true`, feed the summarizer the verbatim conversation instead of the lossy rewrite (the retry loop may still fall back).
    pub verbatim_input: bool,
    pub tool_choice: crate::util::config::CompactionToolChoice,
    /// Prefire two-pass state (background NOTE₁ cache + in-flight guard).
    /// `Default` (empty cache, not in-flight).
    pub prefire: PrefireState,
    /// Sticky once a forked session releases its inherited prefix under compaction pressure (see `run_compact_inner`), so it stops re-pinning it.
    pub prefix_released: AtomicBool,
    /// Explicit user cancellation for the current compaction generation.
    pub cancel: CompactCancelGate,
    /// True from admission of a rolling job until its result is applied or
    /// discarded. Prompt promotion pauses while this is set, making the CAS
    /// application an idle safe point rather than racing an in-flight sample.
    pub rolling_in_flight: AtomicBool,
}

#[cfg(test)]
mod prefire_state_tests {
    use super::*;

    fn dummy_cache() -> AsyncCompactionCache {
        AsyncCompactionCache {
            note1: "NOTE1".to_string(),
            prefix_len: 3,
            fingerprint: 42,
            model_slug: "grok".to_string(),
            pass1_latency_ms: 5,
        }
    }

    /// Pass-2 must be able to await a still-running pass-1 and then read its
    /// cache — i.e. an in-flight prefire is waited for, not discarded for a full
    /// single-pass.
    #[tokio::test]
    async fn take_handle_awaits_in_flight_pass1_then_cache_is_available() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let state = std::rc::Rc::new(PrefireState::default());
                let worker = std::rc::Rc::clone(&state);
                // Background pass-1 that stores its cache only after yielding,
                // so the cache is absent at the moment pass-2 starts.
                let handle = tokio::task::spawn_local(async move {
                    tokio::task::yield_now().await;
                    worker.store(dummy_cache());
                    worker.finish();
                });
                state.set_handle(handle);

                assert!(!state.has_cache(), "cache absent before pass-1 completes");

                if let Some(h) = state.take_handle() {
                    let _ = h.await;
                }

                assert!(state.has_cache(), "cache present after awaiting pass-1");
                assert_eq!(state.take().unwrap().note1, "NOTE1");
                assert!(state.take_handle().is_none(), "handle consumed once taken");
            })
            .await;
    }

    /// No prefire spawned → no handle to await (pass-2 falls straight through to
    /// the single-pass path via the `take()?` that follows in the caller).
    #[tokio::test]
    async fn take_handle_is_none_without_a_spawned_pass1() {
        let state = PrefireState::default();
        assert!(state.take_handle().is_none());
        assert!(state.take().is_none());
    }
}

#[cfg(test)]
mod compact_cancel_gate_tests {
    use super::*;

    #[test]
    fn only_interactive_trigger_bearing_cancels_stop_compaction() {
        assert!(is_explicit_compaction_cancel_trigger(Some("ctrl_c")));
        assert!(is_explicit_compaction_cancel_trigger(Some("esc")));
        assert!(is_explicit_compaction_cancel_trigger(Some("dashboard")));
        assert!(!is_explicit_compaction_cancel_trigger(Some("send_now")));
        assert!(!is_explicit_compaction_cancel_trigger(Some(
            "compaction_persistence_indeterminate"
        )));
        assert!(!is_explicit_compaction_cancel_trigger(None));
    }

    #[test]
    fn request_cancel_trips_shared_token() {
        let gate = CompactCancelGate::default();
        let (token, _scope) = gate.enter();
        assert!(!token.is_cancelled());
        gate.request_cancel();
        assert!(token.is_cancelled());
        assert!(gate.is_cancelled());
    }

    #[test]
    fn pending_cancel_command_is_shared_and_clearable() {
        let gate = CompactCancelGate::default();
        let clone = gate.clone();
        let before = gate.cancel_sequence();
        let first = clone.begin_cancel_command();
        let second = gate.begin_cancel_command();
        assert!(gate.cancel_command_pending());
        assert_eq!(gate.cancel_sequence(), before.wrapping_add(2));
        let unsent = gate.begin_cancel_command();
        assert_eq!(gate.cancel_sequence(), before.wrapping_add(3));
        drop(unsent);
        assert!(gate.cancel_command_pending());
        first.commit();
        gate.clear_cancel_command_pending();
        assert!(clone.cancel_command_pending());
        second.commit();
        clone.clear_cancel_command_pending();
        assert!(!clone.cancel_command_pending());
        assert_eq!(clone.cancel_sequence(), before.wrapping_add(3));
    }

    #[test]
    fn request_cancel_is_noop_when_idle() {
        let gate = CompactCancelGate::default();
        gate.request_cancel();
        let (token, _scope) = gate.enter();
        assert!(!token.is_cancelled());
        assert!(!gate.is_cancelled());
    }

    #[test]
    fn overlapping_operations_cancel_together_but_new_operation_starts_fresh() {
        let gate = CompactCancelGate::default();
        let (first_token, first_scope) = gate.enter();
        let (second_token, second_scope) = gate.enter();
        gate.request_cancel();
        assert!(first_token.is_cancelled());
        assert!(second_token.is_cancelled());

        let (next_token, next_scope) = gate.enter();
        assert!(!next_token.is_cancelled());
        assert!(gate.is_cancelled(), "old cancelled scopes are still active");

        drop(first_scope);
        drop(second_scope);
        assert!(!gate.is_cancelled());
        drop(next_scope);
    }
}
