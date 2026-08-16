#![cfg_attr(rustfmt, rustfmt::skip)]
#![allow(unused_imports)]
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use agent_client_protocol as acp;
use tokio::sync::{Notify, mpsc, oneshot};
use tokio_util::sync::CancellationToken;
use crate::extensions::notification::{SessionNotification, SessionUpdate};
use crate::session::{
    self, SessionCommand, SessionHandle, SessionThread,
    commands::{PromptCompletionKind, PromptTurnResult as SubagentPromptTurnResult},
    fs_watch::FsWatchCapabilities, info::Info as SessionInfo,
};
use crate::terminal::AsyncTerminalRunner;
use crate::tools::ToolContext;
use crate::upload::trace::{
    GCS_SCHEMA_VERSION, PromptMetadata, SubagentSpawnedRef, TurnResultMetadata,
    local_sandbox_telemetry, upload_metadata, upload_session_state,
    upload_subagent_metadata, upload_turn_result,
};
use crate::upload::turn::{PromptTraceContext, complete_prompt_trace};
use xai_acp_lib::AcpAgentGatewaySender as GatewaySender;
use xai_grok_tools::implementations::grok_build::task::types::*;
use xai_grok_workspace::file_system::AsyncFileSystem;
use xai_hunk_tracker::HunkTrackerHandle;
use super::*;
impl SubagentCoordinator {
    pub fn new() -> Self {
        Self {
            pending: HashMap::new(),
            active: HashMap::new(),
            completed: HashMap::new(),
            completion_notify: Arc::new(Notify::new()),
            pending_completions: Vec::new(),
            is_turn_active: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            synthetic_trace_tx: None,
            running_gauge: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            block_wait_slots: HashMap::new(),
            subagent_usage_not_applied_prompts: std::collections::HashSet::new(),
            loop_owned: HashMap::new(),
        }
    }
    pub fn mark_subagent_usage_not_applied(&mut self, prompt_id: &str) {
        self.subagent_usage_not_applied_prompts.insert(prompt_id.to_string());
    }
    pub fn subagent_usage_not_applied(&self, prompt_id: &str) -> bool {
        self.subagent_usage_not_applied_prompts.contains(prompt_id)
    }
    pub fn clear_subagent_usage_not_applied(&mut self, prompt_id: &str) {
        self.subagent_usage_not_applied_prompts.remove(prompt_id);
    }
    pub fn parent_prompt_id_for(&self, subagent_id: &str) -> Option<String> {
        self.active
            .get(subagent_id)
            .and_then(|t| t.parent_prompt_id.clone())
            .or_else(|| {
                self.pending.get(subagent_id).and_then(|p| p.parent_prompt_id.clone())
            })
    }
    /// Rebind the running-subagent gauge, copying the current count so a
    /// late rebind cannot under-report.
    pub fn set_running_gauge(&mut self, gauge: Arc<std::sync::atomic::AtomicUsize>) {
        gauge
            .store(
                self.pending.len() + self.active.len(),
                std::sync::atomic::Ordering::Relaxed,
            );
        self.running_gauge = gauge;
    }
    /// Recompute the gauge from `pending` + `active` after every mutation of
    /// either map — recomputing (rather than incrementing) prevents drift.
    fn sync_running_gauge(&self) {
        self.running_gauge
            .store(
                self.pending.len() + self.active.len(),
                std::sync::atomic::Ordering::Relaxed,
            );
    }
    pub fn completion_notify(&self) -> Arc<Notify> {
        Arc::clone(&self.completion_notify)
    }
    /// Returns a shared handle to the turn-active flag.
    pub fn turn_active_flag(&self) -> Arc<std::sync::atomic::AtomicBool> {
        Arc::clone(&self.is_turn_active)
    }
    /// Whether the model's turn is currently active.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "used from tests only; remove expect when wired in production"
        )
    )]
    pub fn is_turn_active(&self) -> bool {
        self.is_turn_active.load(std::sync::atomic::Ordering::Relaxed)
    }
    /// Pending + active turn-blocking subagent IDs for `prompt_id`.
    /// Background children are excluded: they outlive the turn by design, so
    /// the freeze drain must not wait on them (their spend reaches the session
    /// ledger when they finish; the prompt report flags them via
    /// `background_live`).
    pub fn outstanding_for_prompt(&self, prompt_id: &str) -> Vec<String> {
        let mut ids: Vec<String> = self
            .pending
            .values()
            .filter(|p| {
                p.parent_prompt_id.as_deref() == Some(prompt_id) && !p.run_in_background
            })
            .map(|p| p.subagent_id.clone())
            .chain(
                self
                    .active
                    .values()
                    .filter(|t| {
                        t.parent_prompt_id.as_deref() == Some(prompt_id)
                            && !t.run_in_background
                    })
                    .map(|t| t.subagent_id.clone()),
            )
            .collect();
        ids.sort();
        ids
    }
    /// True while any background child of `prompt_id` is pending or active.
    /// Their spend is missing from the prompt report (it lands on the session
    /// ledger at completion), so the report is incomplete — without waiting.
    pub fn background_live_for_prompt(&self, prompt_id: &str) -> bool {
        self
            .pending
            .values()
            .any(|p| {
                p.parent_prompt_id.as_deref() == Some(prompt_id) && p.run_in_background
            })
            || self
                .active
                .values()
                .any(|t| {
                    t.parent_prompt_id.as_deref() == Some(prompt_id)
                        && t.run_in_background
                })
    }
    /// Record that a foreground child was auto-backgrounded (await budget
    /// expired): it no longer blocks the turn, so the freeze drain must stop
    /// waiting on it.
    pub fn mark_backgrounded(&mut self, subagent_id: &str) {
        if let Some(t) = self.active.values_mut().find(|t| t.subagent_id == subagent_id)
        {
            t.run_in_background = true;
        }
        if let Some(p) = self.pending.values_mut().find(|p| p.subagent_id == subagent_id)
        {
            p.run_in_background = true;
        }
    }
    pub fn outstanding_reply_for_prompt(
        &self,
        prompt_id: &str,
    ) -> xai_grok_tools::implementations::grok_build::task::types::SubagentOutstandingReply {
        xai_grok_tools::implementations::grok_build::task::types::SubagentOutstandingReply {
            live_ids: self.outstanding_for_prompt(prompt_id),
            background_live: self.background_live_for_prompt(prompt_id),
            subagent_usage_not_applied: self.subagent_usage_not_applied(prompt_id),
        }
    }
    pub fn drain_pending_completions_for(
        &mut self,
        session_id: &str,
    ) -> Vec<SubagentCompletionSummary> {
        if session_id.is_empty() {
            return std::mem::take(&mut self.pending_completions);
        }
        let (mine, others) = std::mem::take(&mut self.pending_completions)
            .into_iter()
            .partition(|c| {
                c.owner_session_id.is_empty() || c.owner_session_id == session_id
            });
        self.pending_completions = others;
        mine
    }
    pub fn discard_pending_completions_for(&mut self, session_id: &str) {
        if session_id.is_empty() {
            return;
        }
        self.pending_completions.retain(|c| c.owner_session_id != session_id);
    }
    fn enforce_pending_completions_cap(&mut self) {
        const MAX_PENDING_COMPLETIONS: usize = 256;
        if self.pending_completions.len() > MAX_PENDING_COMPLETIONS {
            let excess = self.pending_completions.len() - MAX_PENDING_COMPLETIONS;
            self.pending_completions.drain(..excess);
        }
    }
    /// Collect references to subagents spawned for a specific parent prompt.
    /// Returns only the children whose `parent_prompt_id` matches, so the
    /// parent turn's `turn_result.json` accurately reflects what was spawned
    /// during that turn — not the entire coordinator lifetime.
    pub fn spawned_refs_for_prompt(&self, prompt_id: &str) -> Vec<SubagentSpawnedRef> {
        let mut refs: Vec<_> = self
            .active
            .values()
            .filter(|t| t.parent_prompt_id.as_deref() == Some(prompt_id))
            .map(|t| SubagentSpawnedRef {
                subagent_id: t.subagent_id.clone(),
                child_session_id: t.child_session_id.0.to_string(),
                subagent_type: t.subagent_type.clone(),
                description: t.description.clone(),
                persona: t.persona.clone(),
                resumed_from: t.resumed_from.clone(),
            })
            .chain(
                self
                    .completed
                    .values()
                    .filter(|c| c.parent_prompt_id.as_deref() == Some(prompt_id))
                    .map(|c| SubagentSpawnedRef {
                        subagent_id: c.subagent_id.clone(),
                        child_session_id: c.child_session_id.clone(),
                        subagent_type: c.subagent_type.clone(),
                        description: c.description.clone(),
                        persona: c.persona.clone(),
                        resumed_from: c.resumed_from.clone(),
                    }),
            )
            .collect();
        refs.sort_by(|a, b| a.subagent_id.cmp(&b.subagent_id));
        refs
    }
    /// Register a subagent as pending (initializing). Call this early,
    /// before any blocking work like worktree creation, so that
    /// `get_task_output` can report the subagent as initializing instead
    /// of "not found".
    pub fn insert_pending(&mut self, entry: PendingSubagent) -> bool {
        if self
            .pending
            .get(&entry.subagent_id)
            .is_some_and(|current| current.assigned_meta_owner.is_some())
            || self
                .active
                .get(&entry.subagent_id)
                .is_some_and(|current| current.assigned_meta_owner.is_some())
            || self
                .completed
                .get(&entry.subagent_id)
                .is_some_and(|current| current.assigned_meta_owner.is_some())
        {
            return false;
        }
        self.pending.insert(entry.subagent_id.clone(), entry);
        self.sync_running_gauge();
        true
    }
    pub(crate) fn install_pending_assigned_owner(
        &mut self,
        id: &str,
        owner: identity_store::AssignedMetaOwner,
    ) -> bool {
        let Some(pending) = self.pending.get_mut(id) else {
            return false;
        };
        if pending.assigned_meta_owner.is_some() {
            return false;
        }
        pending.assigned_meta_owner = Some(owner);
        true
    }
    fn assigned_owner_matches(
        actual: Option<&identity_store::AssignedMetaOwner>,
        expected: Option<&identity_store::AssignedMetaOwner>,
    ) -> bool {
        match (actual, expected) {
            (None, None) => true,
            (Some(actual), Some(expected)) => actual.matches(expected),
            _ => false,
        }
    }
    /// Remove a pending subagent without recording a failure.
    /// Used by cancel flows where the subagent was intentionally stopped.
    #[cfg(test)]
    pub fn remove_pending(&mut self, id: &str) {
        self.remove_pending_owned(id, None);
    }
    pub(crate) fn remove_pending_owned(
        &mut self,
        id: &str,
        expected_owner: Option<&identity_store::AssignedMetaOwner>,
    ) -> bool {
        if !self.pending.get(id).is_some_and(|pending| {
            Self::assigned_owner_matches(pending.assigned_meta_owner.as_ref(), expected_owner)
        }) {
            return false;
        }
        self.pending.remove(id);
        self.sync_running_gauge();
        true
    }
    /// Move a pending subagent directly to `completed` so it stays queryable via
    /// `get_task_output`. `cancelled` stamps `"cancelled"` vs `"failed"`.
    fn move_pending_to_terminal(
        &mut self,
        id: &str,
        error: &str,
        cancelled: bool,
        expected_owner: Option<&identity_store::AssignedMetaOwner>,
    ) {
        if !self.pending.get(id).is_some_and(|pending| {
            Self::assigned_owner_matches(pending.assigned_meta_owner.as_ref(), expected_owner)
        }) {
            return;
        }
        let Some(pending) = self.pending.remove(id) else {
            return;
        };
        self.record_failure_completion(FailureCompletion {
            subagent_id: pending.subagent_id,
            subagent_type: pending.subagent_type,
            description: pending.description,
            parent_prompt_id: pending.parent_prompt_id,
            parent_session_id: pending.parent_session_id,
            owner: pending.owner,
            assigned_meta_owner: pending.assigned_meta_owner,
            persona: pending.persona,
            started_at: pending.started_at,
            error,
            surface_completion: pending.surface_completion,
            cancelled,
        });
    }
    /// Move a pending subagent to `completed` as a failure so it stays queryable
    /// via `get_task_output`.
    pub fn move_pending_to_failed(&mut self, id: &str, error: &str) {
        self.move_pending_to_failed_owned(id, error, None);
    }
    pub fn move_pending_to_failed_owned(
        &mut self,
        id: &str,
        error: &str,
        expected_owner: Option<&identity_store::AssignedMetaOwner>,
    ) {
        self.move_pending_to_terminal(id, error, false, expected_owner);
    }
    /// Like [`Self::move_pending_to_failed`] but stamps `"cancelled"` — a pending
    /// subagent killed while initializing.
    pub fn move_pending_to_cancelled(&mut self, id: &str, error: &str) {
        self.move_pending_to_cancelled_owned(id, error, None);
    }
    pub fn move_pending_to_cancelled_owned(
        &mut self,
        id: &str,
        error: &str,
        expected_owner: Option<&identity_store::AssignedMetaOwner>,
    ) {
        self.move_pending_to_terminal(id, error, true, expected_owner);
    }
    /// Complete a provider-backed agent that does not own an in-process
    /// [`SubagentTracker`] (currently the Codex app-server bridge).
    ///
    /// It remains in `pending` while the external agent runs so the existing
    /// query/cancellation paths work without a second coordinator.
    pub fn complete_pending_external(
        &mut self,
        id: &str,
        result: SubagentResult,
        child_session_id: String,
        child_cwd: String,
        worktree_path: Option<PathBuf>,
        effective_model_id: String,
        persisted_output_dir: Option<PathBuf>,
    ) {
        let Some(pending) = self.pending.remove(id) else {
            return;
        };
        self.loop_owned.remove(id);
        self.sync_running_gauge();
        let success = result.success && !result.cancelled;
        if pending.surface_completion {
            self.pending_completions.push(SubagentCompletionSummary {
                subagent_id: id.to_string(),
                owner_session_id: pending.parent_session_id.clone(),
                subagent_type: pending.subagent_type.clone(),
                description: pending.description.clone(),
                success,
                duration_ms: result.duration_ms,
                tool_calls: result.tool_calls,
                turns: result.turns,
                output: result.output.clone(),
            });
            self.enforce_pending_completions_cap();
        }
        self.completed.insert(
            id.to_string(),
            CompletedSubagent {
                subagent_id: id.to_string(),
                parent_session_id: pending.parent_session_id,
                parent_prompt_id: pending.parent_prompt_id,
                owner: pending.owner,
                assigned_meta_owner: pending.assigned_meta_owner,
                child_session_id,
                description: pending.description,
                subagent_type: pending.subagent_type,
                persona: pending.persona,
                started_at: pending.started_at,
                completed_at: std::time::Instant::now(),
                result,
                resumed_from: None,
                child_cwd,
                worktree_path,
                snapshot_ref: None,
                effective_model_id,
                block_waited: false,
                explicitly_killed: false,
                completion_output_cap: None,
                persisted_output_dir,
            },
        );
        self.enforce_completed_cap();
        self.completion_notify.notify_waiters();
    }
    /// Record a synthetic failure for a subagent that never reached `pending`.
    pub fn record_pre_spawn_failure(
        &mut self,
        subagent_id: String,
        subagent_type: String,
        description: String,
        parent_prompt_id: Option<String>,
        parent_session_id: String,
        owner: SubagentOwner,
        error: &str,
        surface_completion: bool,
    ) {
        self.record_failure_completion(FailureCompletion {
            subagent_id,
            subagent_type,
            description,
            parent_prompt_id,
            parent_session_id,
            owner,
            assigned_meta_owner: None,
            persona: None,
            started_at: std::time::Instant::now(),
            error,
            surface_completion,
            cancelled: false,
        });
    }
    /// Insert a synthetic failed entry, push a completion summary, notify waiters.
    /// Clears any stale pending entry for the same id.
    fn record_failure_completion(&mut self, c: FailureCompletion<'_>) {
        self.pending.remove(&c.subagent_id);
        self.loop_owned.remove(&c.subagent_id);
        self.sync_running_gauge();
        let FailureCompletion {
            subagent_id,
            subagent_type,
            description,
            parent_prompt_id,
            parent_session_id,
            owner,
            assigned_meta_owner,
            persona,
            started_at,
            error,
            surface_completion,
            cancelled,
        } = c;
        let result = SubagentResult {
            success: false,
            cancelled,
            error: Some(error.to_string()),
            subagent_id: subagent_id.clone(),
            ..Default::default()
        };
        let summary_output = result.output.clone();
        let owner_session_id = parent_session_id.clone();
        self.completed
            .insert(
                subagent_id.clone(),
                CompletedSubagent {
                    subagent_id: subagent_id.clone(),
                    parent_session_id,
                    parent_prompt_id,
                    owner,
                    assigned_meta_owner,
                    child_session_id: String::new(),
                    description: description.clone(),
                    subagent_type: subagent_type.clone(),
                    persona,
                    started_at,
                    completed_at: std::time::Instant::now(),
                    result,
                    resumed_from: None,
                    child_cwd: String::new(),
                    worktree_path: None,
                    snapshot_ref: None,
                    effective_model_id: String::new(),
                    block_waited: false,
                    explicitly_killed: false,
                    completion_output_cap: None,
                    persisted_output_dir: None,
                },
            );
        self.enforce_completed_cap();
        if surface_completion {
            self.pending_completions
                .push(SubagentCompletionSummary {
                    subagent_id,
                    owner_session_id,
                    subagent_type,
                    description,
                    success: false,
                    duration_ms: 0,
                    tool_calls: 0,
                    turns: 0,
                    output: summary_output,
                });
            self.enforce_pending_completions_cap();
        }
        self.completion_notify.notify_waiters();
    }
    pub fn insert(&mut self, tracker: SubagentTracker) {
        let expected_owner = tracker.assigned_meta_owner.clone();
        self.insert_owned(tracker, expected_owner.as_ref());
    }
    pub(crate) fn insert_owned(
        &mut self,
        tracker: SubagentTracker,
        expected_owner: Option<&identity_store::AssignedMetaOwner>,
    ) {
        let id = tracker.subagent_id.clone();
        let can_promote = match self.pending.get(&id) {
            Some(pending) => Self::assigned_owner_matches(
                pending.assigned_meta_owner.as_ref(),
                expected_owner,
            ) && Self::assigned_owner_matches(
                tracker.assigned_meta_owner.as_ref(),
                expected_owner,
            ),
            None => expected_owner.is_none() && tracker.assigned_meta_owner.is_none(),
        };
        if !can_promote {
            return;
        }
        self.pending.remove(&id);
        self.active.insert(id, tracker);
        self.sync_running_gauge();
    }
    /// Move a finished subagent from `active` to `completed`.
    /// Returns the tracker if it was active.
    pub fn move_to_completed(
        &mut self,
        id: &str,
        description: String,
        subagent_type: String,
        result: SubagentResult,
        persisted_output_dir: Option<PathBuf>,
    ) -> Option<SubagentTracker> {
        self.move_to_completed_owned(
            id,
            None,
            description,
            subagent_type,
            result,
            persisted_output_dir,
        )
    }
    pub fn move_to_completed_owned(
        &mut self,
        id: &str,
        expected_owner: Option<&identity_store::AssignedMetaOwner>,
        description: String,
        subagent_type: String,
        result: SubagentResult,
        persisted_output_dir: Option<PathBuf>,
    ) -> Option<SubagentTracker> {
        if self.active.get(id).is_some_and(|tracker| {
            !Self::assigned_owner_matches(
                tracker.assigned_meta_owner.as_ref(),
                expected_owner,
            )
        }) {
            return None;
        }
        let tracker = self.active.remove(id);
        self.loop_owned.remove(id);
        self.sync_running_gauge();
        let started_at = tracker
            .as_ref()
            .map(|t| t.started_at)
            .unwrap_or_else(std::time::Instant::now);
        let parent_session_id = tracker
            .as_ref()
            .map(|t| t.parent_session_id.clone())
            .unwrap_or_default();
        let child_session_id = tracker
            .as_ref()
            .map(|t| t.child_session_id.0.to_string())
            .unwrap_or_default();
        let parent_prompt_id = tracker.as_ref().and_then(|t| t.parent_prompt_id.clone());
        let owner = tracker.as_ref().map(|t| t.owner.clone()).unwrap_or_default();
        let assigned_meta_owner = tracker
            .as_ref()
            .and_then(|t| t.assigned_meta_owner.clone());
        let persona = tracker.as_ref().and_then(|t| t.persona.clone());
        let child_cwd = tracker
            .as_ref()
            .map(|t| t.child_cwd.clone())
            .unwrap_or_default();
        let worktree_path = tracker.as_ref().and_then(|t| t.worktree_path.clone());
        let resumed_from = tracker.as_ref().and_then(|t| t.resumed_from.clone());
        let effective_model_id = tracker
            .as_ref()
            .map(|t| t.effective_model_id.clone())
            .unwrap_or_default();
        let block_waited = tracker.as_ref().is_some_and(|t| t.block_waited);
        let explicitly_killed = tracker.as_ref().is_some_and(|t| t.explicitly_killed);
        let surface_completion = tracker.as_ref().is_none_or(|t| t.surface_completion);
        let completion_output_cap = tracker
            .as_ref()
            .and_then(|t| t.completion_output_cap);
        let mut completed = CompletedSubagent {
            subagent_id: id.to_string(),
            parent_session_id,
            parent_prompt_id,
            owner,
            assigned_meta_owner,
            child_session_id,
            description,
            subagent_type,
            persona,
            started_at,
            completed_at: std::time::Instant::now(),
            result,
            resumed_from,
            child_cwd,
            worktree_path,
            snapshot_ref: None,
            effective_model_id,
            block_waited,
            explicitly_killed,
            completion_output_cap,
            persisted_output_dir,
        };
        let success = completed.result.success && !completed.result.cancelled;
        {
            let preview = crate::util::truncate(&completed.result.output, 200);
            let level_fn = if success {
                xai_grok_telemetry::unified_log::info
            } else {
                xai_grok_telemetry::unified_log::error
            };
            level_fn(
                if success { "subagent completed" } else { "subagent failed" },
                None,
                Some(
                    serde_json::json!({
                    "subagent_id": &completed.subagent_id,
                    "subagent_type": &completed.subagent_type,
                    "effective_model": &completed.effective_model_id,
                    "success": success,
                    "cancelled": completed.result.cancelled,
                    "duration_ms": completed.result.duration_ms,
                    "turns": completed.result.turns,
                    "tool_calls": completed.result.tool_calls,
                    "output_preview": preview,
                    "error": &completed.result.error,
                }),
                ),
            );
        }
        if surface_completion {
            self.pending_completions
                .push(SubagentCompletionSummary {
                    subagent_id: id.to_string(),
                    owner_session_id: completed.parent_session_id.clone(),
                    subagent_type: completed.subagent_type.clone(),
                    description: completed.description.clone(),
                    success,
                    duration_ms: completed.result.duration_ms,
                    tool_calls: completed.result.tool_calls,
                    turns: completed.result.turns,
                    output: super::cap_completion_output(
                        &completed.result.output,
                        completed.completion_output_cap,
                    ),
                });
            self.enforce_pending_completions_cap();
        }
        if completed.persisted_output_dir.is_some() {
            completed.result.output = Arc::from("");
        }
        self.completed.insert(id.to_string(), completed);
        self.enforce_completed_cap();
        self.completion_notify.notify_waiters();
        tracker
    }
    /// Record the durable worktree snapshot ref on a completed subagent so
    /// in-memory `resume_from` resolution can rehydrate the disposed worktree.
    /// No-op if the entry was already evicted (the on-disk meta.json still has it).
    pub fn set_completed_snapshot_ref(&mut self, id: &str, snapshot_ref: String) {
        self.set_completed_snapshot_ref_owned(id, None, snapshot_ref);
    }
    pub fn set_completed_snapshot_ref_owned(
        &mut self,
        id: &str,
        expected_owner: Option<&identity_store::AssignedMetaOwner>,
        snapshot_ref: String,
    ) {
        if let Some(completed) = self.completed.get_mut(id)
            && Self::assigned_owner_matches(
                completed.assigned_meta_owner.as_ref(),
                expected_owner,
            )
        {
            completed.snapshot_ref = Some(snapshot_ref);
        }
    }
    /// Cancel all active subagents that were launched by a specific parent turn,
    /// including `run_in_background: true` subagents.
    pub fn cancel_by_parent_prompt_id(&mut self, parent_prompt_id: &str) {
        for tracker in self.active.values() {
            if tracker.parent_prompt_id.as_deref() == Some(parent_prompt_id) {
                Self::cancel_tracker(tracker);
            }
        }
        for pending in self.pending.values() {
            if pending.parent_prompt_id.as_deref() == Some(parent_prompt_id) {
                pending.cancel_token.cancel();
            }
        }
    }
    pub fn cancel_workflow_children(&mut self, run_id: &str) -> usize {
        for tracker in self.active.values() {
            if tracker.owner.workflow_run_id() == Some(run_id) {
                Self::cancel_tracker(tracker);
            }
        }
        for pending in self.pending.values() {
            if pending.owner.workflow_run_id() == Some(run_id) {
                pending.cancel_token.cancel();
            }
        }
        self.outstanding_for_workflow(run_id)
    }
    pub fn outstanding_for_workflow(&self, run_id: &str) -> usize {
        self
            .pending
            .values()
            .filter(|entry| entry.owner.workflow_run_id() == Some(run_id))
            .count()
            + self
                .active
                .values()
                .filter(|entry| entry.owner.workflow_run_id() == Some(run_id))
                .count()
    }
    /// Attempt to cancel a subagent. Returns a typed outcome covering all cases:
    /// - Active → cancel it, return Cancelled
    /// - Pending (initializing) → fire its spawn token, return Cancelled
    /// - Already finished → return AlreadyFinished with terminal status
    /// - Unknown ID → return NotFound
    pub fn cancel_with_outcome(&mut self, subagent_id: &str) -> SubagentCancelOutcome {
        if let Some(tracker) = self.active.get(subagent_id) {
            Self::cancel_tracker(tracker);
            return SubagentCancelOutcome::Cancelled;
        }
        if let Some(pending) = self.pending.get(subagent_id) {
            pending.cancel_token.cancel();
            return SubagentCancelOutcome::Cancelled;
        }
        if let Some(entry) = self.completed.get(subagent_id) {
            return SubagentCancelOutcome::AlreadyFinished {
                status: entry.result.status().to_string(),
            };
        }
        SubagentCancelOutcome::NotFound
    }
    /// Internal: send Cancel + Shutdown to a tracked subagent.
    fn cancel_tracker(tracker: &SubagentTracker) {
        tracker.cancel_token.cancel();
        let _ = tracker
            .child_handle
            .cmd_tx
            .send(SessionCommand::Cancel {
                cancel_subagents: true,
                kill_background_tasks: true,
                rewind_if_pristine: false,
                trigger: None,
            });
        let _ = tracker.child_handle.cmd_tx.send(SessionCommand::Shutdown);
    }
}
