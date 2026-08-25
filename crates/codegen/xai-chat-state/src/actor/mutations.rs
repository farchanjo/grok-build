//! Mutation handlers for the ChatStateActor.

use xai_grok_inference_types::{
    ContentPart, ConversationItem, DanglingToolCallReason, dedup_duplicate_tool_results,
    repair_dangling_tool_calls,
};

use super::ChatStateActor;
use super::request_builder::HARD_CLEAR_PLACEHOLDER;
use crate::events::ChatStateEvent;
use crate::types::{ChatStateSnapshot, fingerprint_conversation_items};

/// Static string label for tracing on `ConversationItem` (avoids pulling
/// the `Role` enum into the format string).
fn item_kind_str(item: &ConversationItem) -> &'static str {
    match item {
        ConversationItem::System(_) => "system",
        ConversationItem::User(_) => "user",
        ConversationItem::Assistant(_) => "assistant",
        ConversationItem::ToolResult(_) => "tool_result",
        ConversationItem::BackendToolCall(_) => "backend_tool_call",
        ConversationItem::Reasoning(_) => "reasoning",
    }
}

impl ChatStateActor {
    /// Increment the structural epoch counter.
    ///
    /// Uses wrapping semantics to prevent overflow panics in long-running
    /// sessions. The epoch is used for CAS operations to detect stale state —
    /// wrapping is acceptable because the probability of wraparound in a
    /// practical session lifetime is astronomically low (u64::MAX operations
    /// at 1000 ops/sec would take ~584 million years).
    pub(super) fn bump_structural_epoch(&mut self) {
        self.state.structural_epoch = self.state.structural_epoch.wrapping_add(1);
    }

    /// Repair any dangling tool calls in the conversation and persist the fix.
    ///
    /// A "dangling" tool call is an assistant message with tool call IDs that
    /// lack matching `ToolResult` entries. This can happen when:
    /// - The user cancels (Ctrl+C) mid-tool-execution in a live session
    /// - The process crashes between pushing the assistant and tool results
    /// - The tokio task is aborted at an `.await` point
    ///
    /// This method also strips orphaned/displaced `ToolResult`s — results
    /// whose owning assistant tool call is gone or out of position. That is
    /// the shape behind "No tool call found for function call output with
    /// call_id …" 400s on strict Responses backends (ChatGPT Codex), which
    /// brick the session on every request until repaired.
    ///
    /// This method repairs the state in-place and persists the fix to disk.
    /// It is idempotent — calling it on a clean conversation is a cheap no-op
    /// (single forward scan, no allocations).
    ///
    /// Only call at write boundaries where the previous turn is definitively
    /// over (`ChatState::new()`, `push_user_message()`, `BuildConversationRequest`).
    /// Do NOT call from read handlers — background tasks run concurrently with
    /// tool execution and would misidentify in-flight calls as dangling.
    pub(super) fn ensure_conversation_integrity(&mut self) {
        self.ensure_conversation_integrity_with_reason(DanglingToolCallReason::UserCancelled);
    }

    /// Like [`Self::ensure_conversation_integrity`] but takes an explicit reason.
    pub(super) fn ensure_conversation_integrity_with_reason(
        &mut self,
        reason: DanglingToolCallReason,
    ) {
        // In-place integrity repair can add/remove items ahead of an active capture's
        // boundary, so snapshot + rebase the offset like the replace/restore paths.
        self.snapshot_turn_slice();
        let deduped = dedup_duplicate_tool_results(&mut self.state.conversation);
        if deduped > 0 {
            tracing::info!(
                deduped_count = deduped,
                "Removed duplicate tool results in conversation"
            );
        }
        // Orphaned/displaced tool results must be stripped before the dangling
        // pass so the two passes agree on which calls are answered (same order
        // as `compaction_utils::repair_history`). This self-heals sessions that
        // would otherwise 400 on every request until the out-of-band
        // `x.ai/session/repair` rail ran.
        let stripped =
            crate::compaction_utils::strip_displaced_tool_results(&mut self.state.conversation);
        if !stripped.is_empty() {
            tracing::warn!(
                stripped_tool_result_ids = ?stripped,
                "Stripped orphaned/displaced tool results in conversation"
            );
        }
        let repaired = repair_dangling_tool_calls(&mut self.state.conversation, reason);
        if repaired > 0 || deduped > 0 || !stripped.is_empty() {
            tracing::info!(
                repaired_count = repaired,
                "Repaired dangling tool calls in conversation"
            );
            self.persistence.replace_history(&self.state.conversation);
            self.bump_structural_epoch();
        }
        self.rebase_turn_capture_offset();
    }

    /// Repair dangling tool calls after a harness-initiated halt.
    pub(super) fn repair_dangling_after_harness_halt(&mut self, class: &'static str) {
        self.ensure_conversation_integrity_with_reason(DanglingToolCallReason::HarnessHalted {
            class,
        });
    }

    /// Out-of-band history repair (`x.ai/session/repair`): run
    /// [`crate::compaction_utils::repair_history`] and persist changes via
    /// [`Self::replace_conversation`]. Unlike
    /// [`Self::ensure_conversation_integrity`], this also removes orphaned
    /// `ToolResult`s — the shape that bricks a session with provider 400s.
    /// `dry_run` only reports.
    pub(super) fn repair_history(
        &mut self,
        dry_run: bool,
    ) -> crate::compaction_utils::HistoryRepairReport {
        if dry_run {
            let mut copy = self.state.conversation.clone();
            return crate::compaction_utils::repair_history(&mut copy);
        }
        let mut items = std::mem::take(&mut self.state.conversation);
        let report = crate::compaction_utils::repair_history(&mut items);
        if report.changed() {
            tracing::warn!(
                duplicates_removed = report.duplicates_removed,
                stripped_tool_result_ids = ?report.stripped_tool_result_ids,
                synthetic_results_inserted = report.synthetic_results_inserted,
                "History repair modified the conversation"
            );
            // Full replace: persists atomically and re-bases token estimates.
            self.replace_conversation(items, false);
        } else {
            // Nothing changed — put the conversation back untouched.
            self.state.conversation = items;
        }
        report
    }

    /// Strip matching image URLs in place and request a backup-gated disk
    /// rewrite. Token totals and turn-capture offsets remain valid because no
    /// conversation item is inserted or removed. The structural epoch is
    /// bumped on a real mutation so stale rolling-compaction CAS identities
    /// cannot apply against the pre-strip content.
    pub(super) fn strip_conversation_images(
        &mut self,
        urls: &[std::sync::Arc<str>],
    ) -> Option<(usize, tokio::sync::oneshot::Receiver<std::io::Result<()>>)> {
        let stripped =
            xai_grok_inference_types::strip_images_by_url(&mut self.state.conversation, urls);
        if stripped == 0 {
            return None;
        }
        tracing::warn!(
            stripped,
            "stripped server-rejected image(s) from stored conversation"
        );
        self.bump_structural_epoch();
        let acknowledgement = self
            .persistence
            .replace_history_for_strip_and_ack(&self.state.conversation);
        Some((stripped, acknowledgement))
    }

    /// Make memory match the disk-authoritative switch for one generation.
    pub(super) fn converge_working_directory_switch(
        &mut self,
        generation: u64,
        authoritative: ConversationItem,
    ) {
        if let Some(index) = self
            .state
            .conversation
            .iter()
            .position(|item| item.working_directory_switch_generation() == Some(generation))
        {
            let existing = &self.state.conversation[index];
            let unchanged = match (
                serde_json::to_vec(existing),
                serde_json::to_vec(&authoritative),
            ) {
                (Ok(existing), Ok(authoritative)) => existing == authoritative,
                _ => false,
            };
            if unchanged {
                return;
            }

            let old_tokens = super::state::estimate_item_tokens(existing);
            let new_tokens = super::state::estimate_item_tokens(&authoritative);
            self.state.estimated_tokens_since_model = if new_tokens >= old_tokens {
                self.state
                    .estimated_tokens_since_model
                    .saturating_add(new_tokens - old_tokens)
            } else {
                self.state
                    .estimated_tokens_since_model
                    .saturating_sub(old_tokens - new_tokens)
            };
            self.state.conversation[index] = authoritative;
        } else {
            self.state.estimated_tokens_since_model +=
                super::state::estimate_item_tokens(&authoritative);
            self.state.conversation.push(authoritative);
        }
        self.bump_structural_epoch();
    }

    /// Push any conversation item (user, assistant, or tool result) and persist it.
    pub(super) fn push_message(&mut self, item: ConversationItem) {
        let count_in_delta = !matches!(item, ConversationItem::Assistant(_));
        if count_in_delta {
            let estimated_tokens = super::state::estimate_item_tokens(&item);
            self.state.estimated_tokens_since_model += estimated_tokens;
            tracing::debug!(
                item_kind = item_kind_str(&item),
                estimated_tokens_delta = estimated_tokens,
                estimated_total = self.state.total_tokens + self.state.estimated_tokens_since_model,
                model_reported_total = self.state.total_tokens,
                "ChatState: push_message updated estimated_tokens_since_model"
            );
        }
        self.persistence.persist_message(&item);
        self.state.conversation.push(item);
        self.bump_structural_epoch();
    }

    /// Push a user message, ensuring conversation integrity first.
    ///
    /// When the user cancels a turn while the model was executing parallel
    /// tool calls, the conversation may have dangling tool call IDs. This
    /// method repairs them before appending the new message so the on-disk
    /// and in-memory state stay consistent.
    ///
    /// Also runs [`prune_retained_conversation`] to eagerly hard-clear very
    /// old tool results from the in-memory state, bounding long-session
    /// retained memory without waiting for the context-window threshold.
    pub(super) fn push_user_message(&mut self, item: ConversationItem) {
        self.push_user_message_with_repair_reason(item, DanglingToolCallReason::UserCancelled);
    }

    /// Like [`Self::push_user_message`] but takes an explicit repair reason.
    pub(super) fn push_user_message_with_repair_reason(
        &mut self,
        item: ConversationItem,
        reason: DanglingToolCallReason,
    ) {
        self.ensure_conversation_integrity_with_reason(reason);
        let estimated_tokens = super::state::estimate_item_tokens(&item);
        self.state.estimated_tokens_since_model += estimated_tokens;
        tracing::debug!(
            item_kind = item_kind_str(&item),
            estimated_tokens_delta = estimated_tokens,
            estimated_total = self.state.total_tokens + self.state.estimated_tokens_since_model,
            model_reported_total = self.state.total_tokens,
            "ChatState: push_user_message updated estimated_tokens_since_model"
        );
        self.persistence.persist_message(&item);
        self.state.conversation.push(item);
        self.bump_structural_epoch();
        self.prune_retained_conversation();
    }

    /// Push a consecutive batch of user-role items (e.g. a hidden prime
    /// `<system_reminder>` immediately before the real user message) in order,
    /// as ONE durable append. Integrity repair, epoch bump, and pruning run
    /// once for the batch, so the pair can never be interleaved or partially
    /// persisted in memory or on disk.
    pub(super) fn push_message_batch(&mut self, items: Vec<ConversationItem>) {
        if items.is_empty() {
            return;
        }
        self.ensure_conversation_integrity_with_reason(DanglingToolCallReason::UserCancelled);
        let mut estimated_tokens = 0u64;
        for item in &items {
            estimated_tokens += super::state::estimate_item_tokens(item);
        }
        self.state.estimated_tokens_since_model += estimated_tokens;
        tracing::debug!(
            batch_len = items.len(),
            estimated_tokens_delta = estimated_tokens,
            "ChatState: push_message_batch updated estimated_tokens_since_model"
        );
        self.persistence.persist_message_batch(&items);
        self.state.conversation.extend(items);
        self.bump_structural_epoch();
        self.prune_retained_conversation();
    }

    /// Eagerly hard-clear tool results from very old turns in the retained
    /// in-memory conversation, freeing the actual string bytes.
    ///
    /// Unlike the API-copy pruning in `build_conversation_request` (which runs
    /// on a *clone* only when context > 50% full), this operates on
    /// `self.state.conversation` directly and runs after every user turn.
    ///
    /// # What this does
    ///
    /// Only **hard-clears** are applied (no soft-trim).  Soft-trimming is a
    /// context-management operation that changes what the model sees;
    /// hard-clearing is a memory-management operation that replaces content
    /// that is so old the model should not need it again.  The threshold is
    /// controlled by `PruningConfig::hard_clear_age_turns`.
    ///
    /// # Retained-memory measurement
    ///
    /// When any clearing occurs, a `tracing::debug!` event reports:
    /// - `hard_cleared` — number of tool results cleared
    /// - `bytes_freed` — approximate bytes recovered (sum of content lengths)
    /// - `conversation_len` — total item count after the pass
    ///
    /// # Synthetic User items and turn-age accuracy
    ///
    /// The shell can inject synthetic `User` items mid-turn (e.g. system
    /// corrective warnings) without calling `increment_prompt_index`.  These
    /// do not represent real user turns.  The backward scan here counts every
    /// `User` item as a turn boundary, so synthetic items would normally cause
    /// old tool results to appear older than they really are.
    ///
    /// This is compensated by raising the effective clearing threshold by the
    /// number of synthetic User items (`total_user_items - prompt_index`).
    /// The result: a tool result is never cleared before `hard_clear_age_turns`
    /// REAL turns have elapsed, even in sessions with many synthetic messages.
    ///
    /// # Replay / rewind correctness
    ///
    /// `updates.jsonl` is **never touched**, so cross-compaction
    /// `replay_to_prompt` is unaffected.  The pruned `chat_history.jsonl`
    /// on disk mirrors the in-memory state — both lose old bulk content but
    /// `updates.jsonl` retains the original data for replay.
    pub(super) fn prune_retained_conversation(&mut self) -> usize {
        if !self.pruning_config.enabled {
            return 0;
        }
        // Fast exit: not enough turns have elapsed for any hard-clear to apply.
        if self.state.prompt_index < self.pruning_config.hard_clear_age_turns {
            return 0;
        }

        // Compute how many synthetic User items exist (system reminders, etc.).
        // Synthetic User items are NOT real user turns — they are injected by the
        // shell mid-turn and do not increment `prompt_index`.  The naive backward
        // scan counts every User item as a turn boundary, so synthetic items make
        // old tool results appear older than they really are and can cause
        // premature hard-clears.
        //
        // Fix: raise the effective clearing threshold by the number of synthetic
        // User items.  This guarantees a tool result is never cleared before
        // `hard_clear_age_turns` REAL turns have elapsed, regardless of how many
        // synthetic messages the session contains.
        let total_user_items = self
            .state
            .conversation
            .iter()
            .filter(|i| matches!(i, ConversationItem::User(_)))
            .count();
        let synthetic_count = total_user_items.saturating_sub(self.state.prompt_index);
        let effective_threshold = self
            .pruning_config
            .hard_clear_age_turns
            .saturating_add(synthetic_count);

        let before_bytes = self.conversation_content_bytes();
        let mut cleared = 0usize;
        let mut turn_from_end: usize = 0;
        let mut seen_first_user = false;

        for i in (0..self.state.conversation.len()).rev() {
            if matches!(&self.state.conversation[i], ConversationItem::User(_)) {
                if seen_first_user {
                    turn_from_end += 1;
                }
                seen_first_user = true;
                continue;
            }

            let ConversationItem::ToolResult(tr) = &mut self.state.conversation[i] else {
                continue;
            };

            if turn_from_end < effective_threshold {
                continue;
            }

            if tr.content.as_ref() != HARD_CLEAR_PLACEHOLDER {
                tr.content = std::sync::Arc::<str>::from(HARD_CLEAR_PLACEHOLDER);
                cleared += 1;
            }
        }

        if cleared > 0 {
            let after_bytes = self.conversation_content_bytes();
            tracing::debug!(
                hard_cleared = cleared,
                bytes_freed = before_bytes.saturating_sub(after_bytes),
                conversation_len = self.state.conversation.len(),
                "ChatState: in-memory tool-result prune"
            );
            self.persistence.replace_history(&self.state.conversation);
            // Bump epoch for structural change (in-place content modification)
            self.bump_structural_epoch();
        }

        cleared
    }

    /// Approximate byte footprint of all string content in the conversation.
    ///
    /// Used for before/after measurement logging when pruning runs.
    /// Sums the byte lengths of all string fields; does not allocate.
    fn conversation_content_bytes(&self) -> usize {
        self.state
            .conversation
            .iter()
            .map(|item| match item {
                ConversationItem::System(s) => s.content.len(),
                ConversationItem::User(u) => u
                    .content
                    .iter()
                    .map(|p| match p {
                        ContentPart::Text { text } => text.len(),
                        ContentPart::Image { url } => url.len(),
                    })
                    .sum::<usize>(),
                ConversationItem::Assistant(a) => a.content.len(),
                ConversationItem::ToolResult(tr) => tr.content.len(),
                ConversationItem::BackendToolCall(b) => b.text_summary().len(),
                ConversationItem::Reasoning(r) => {
                    xai_grok_inference_types::reasoning_item_text(r).len()
                        + r.encrypted_content.as_deref().map(str::len).unwrap_or(0)
                }
            })
            .sum()
    }

    /// Record accumulated token usage and emit an event.
    pub(super) fn record_token_usage(&mut self, total_tokens: u64) {
        self.state.estimated_tokens_since_model = 0;
        self.state.estimate_at_last_response =
            super::state::estimate_conversation_tokens(&self.state.conversation);
        self.state.total_tokens = total_tokens;
        self.send_event(ChatStateEvent::TokensUpdated { total_tokens });
    }

    /// Stash the per-turn `TokenUsage` from the most recent model response.
    /// No event is emitted — this slot is read on demand at `PromptResponse`
    /// construction time, not pushed to subscribers.
    pub(super) fn record_last_turn_usage(&mut self, usage: xai_grok_inference_types::TokenUsage) {
        self.state.last_turn_usage = Some(usage);
    }

    pub(super) fn record_model_call_usage(
        &mut self,
        model_id: Option<String>,
        usage: &xai_grok_inference_types::TokenUsage,
        api_duration_ms: Option<u64>,
        cost_usd_ticks: Option<i64>,
    ) {
        let model_key = match model_id.as_deref() {
            Some(id) if !id.is_empty() => id,
            _ => self.state.inference_settings.model.as_str(),
        }
        .to_owned();
        self.state
            .prompt_usage
            .get_or_insert_default()
            .record_main_loop_call(&model_key, usage, api_duration_ms, cost_usd_ticks);
        self.state.session_usage.record_main_loop_call(
            &model_key,
            usage,
            api_duration_ms,
            cost_usd_ticks,
        );
    }

    pub(super) fn record_subagent_usage(
        &mut self,
        by_model: &[(String, crate::usage::UsageTotals)],
        attribute_to_prompt: bool,
        incomplete: bool,
    ) {
        if by_model.is_empty() && !incomplete {
            return;
        }
        if attribute_to_prompt {
            self.state
                .prompt_usage
                .get_or_insert_default()
                .record_subagent(by_model, incomplete);
        }
        // The session ledger always folds, even when the usage is not
        // attributable to the open prompt (its pin may belong to an earlier
        // prompt). Reporting that gap is the coordinator's sticky flag's job —
        // never mark a different live prompt's ledger.
        self.state
            .session_usage
            .record_subagent(by_model, incomplete);
    }

    pub(super) fn mark_usage_incomplete(&mut self, prompt: bool, session: bool) {
        if prompt {
            self.state
                .prompt_usage
                .get_or_insert_default()
                .mark_incomplete();
        }
        if session {
            self.state.session_usage.mark_incomplete();
        }
    }

    pub(super) fn increment_prompt_index(&mut self) {
        self.state.prompt_usage = None;
        self.state.prompt_index += 1;
        self.send_event(ChatStateEvent::PromptIndexChanged {
            new_index: self.state.prompt_index,
        });
    }

    /// Replace the entire conversation, persist, re-estimate `total_tokens`,
    /// and emit reset + token-update events.
    ///
    /// Compaction replaces carry the provider-side overhead forward as a
    /// *ratio* (`base_estimate × provider_total ÷ estimate_at_last_response`,
    /// capped at the pre-compaction total; `base_estimate` when that estimate is
    /// 0) so the reseed neither springs back nor over-counts (see
    /// `COMPACTION.md`).
    ///
    /// Increments the structural epoch to invalidate any stale snapshots.
    pub(super) fn replace_conversation(
        &mut self,
        items: Vec<ConversationItem>,
        is_compaction: bool,
    ) {
        self.apply_conversation_replacement(items, is_compaction, true);
    }

    fn apply_conversation_replacement(
        &mut self,
        items: Vec<ConversationItem>,
        is_compaction: bool,
        persist_history: bool,
    ) {
        self.snapshot_turn_slice();
        if is_compaction && let Some(cap) = &mut self.state.turn_capture {
            cap.compaction_occurred = true;
        }
        let pre_replace_total = self.state.total_tokens;
        // `harness_trace_buffer` / `harness_trace_turns` intentionally untouched:
        // the planner/verifier subagents ran, so their sealed trace turns survive
        // a conversation replace (same intent as the `TruncateToPromptIndex` arm).
        if persist_history {
            self.persistence.replace_history(&items);
        }
        let base_estimate = super::state::estimate_conversation_tokens(&items);
        let mut estimated_tokens =
            if is_compaction && pre_replace_total > 0 && self.state.estimate_at_last_response > 0 {
                let ratio = pre_replace_total as f64 / self.state.estimate_at_last_response as f64;
                (base_estimate as f64 * ratio).round() as u64
            } else {
                base_estimate
            };
        // Compaction must never appear to increase usage.
        if is_compaction && pre_replace_total > 0 {
            estimated_tokens = estimated_tokens.min(pre_replace_total);
        }
        self.state.conversation = items;
        self.state.estimated_tokens_since_model = 0;
        self.state.total_tokens = estimated_tokens;
        self.state.estimate_at_last_response =
            super::state::estimate_conversation_tokens(&self.state.conversation);
        self.rebase_turn_capture_offset();
        // Bump epoch for structural change
        self.bump_structural_epoch();
        self.send_event(ChatStateEvent::ConversationReset {
            new_len: self.state.conversation.len(),
        });
        self.send_event(ChatStateEvent::TokensUpdated {
            total_tokens: estimated_tokens,
        });
    }

    /// Atomically swap the leading `System` message with `prompt` (or insert one
    /// if absent), persisting when changed. Runs inside the actor's command loop
    /// so it serializes with turn pushes — no lost-update race on a mid-turn
    /// reconnect. Returns whether the conversation changed.
    ///
    /// The conversation is cloned (items are `Arc`-backed, so the clone is
    /// shallow) rather than `mem::take`n: `replace_conversation` snapshots the
    /// in-flight turn-capture tail from `state.conversation` before swapping,
    /// so the state must stay intact until then.
    pub(super) fn replace_system_head(&mut self, prompt: &str) -> bool {
        if let Some(ConversationItem::System(sys)) = self.state.conversation.first()
            && crate::conversation_util::canonical_system_prompt_eq(sys.content.as_ref(), prompt)
        {
            return false;
        }
        let mut conversation = self.state.conversation.clone();
        let changed =
            crate::conversation_util::replace_or_insert_system_head(&mut conversation, prompt);
        debug_assert!(changed, "head mismatch must produce a change");
        self.replace_conversation(conversation, false);
        changed
    }

    /// Restore all state fields from a snapshot.
    ///
    /// Increments the structural epoch to invalidate any stale snapshots that
    /// were taken before this restore.
    pub(super) fn restore_snapshot(&mut self, snap: ChatStateSnapshot) {
        self.snapshot_turn_slice();
        // Harness trace buffers are transient (not part of the snapshot) and
        // intentionally survive a restore — see `replace_conversation`.
        self.state.conversation = snap.conversation;
        self.rebase_turn_capture_offset();
        self.state.inference_settings = snap.inference_settings;
        self.state.prompt_index = snap.prompt_index;
        self.state.total_tokens = snap.total_tokens;
        self.state.estimated_tokens_since_model = 0;
        self.state.estimate_at_last_response = if snap.estimate_at_last_response > 0 {
            snap.estimate_at_last_response
        } else {
            super::state::estimate_conversation_tokens(&self.state.conversation)
        };
        self.state.agent_edited_paths = snap.agent_edited_paths;
        self.state.prompt_texts = snap.prompt_texts;
        self.state.stream_start_ms = snap.stream_start_ms;
        self.state.turn_start_ms = snap.turn_start_ms;
        self.state.last_compaction_prompt_index = snap.last_compaction_prompt_index;
        self.state.credentials = snap.credentials;
        // Drop abandoned prompt billing; session ledger is lifetime.
        self.state.prompt_usage = None;
        // Bump epoch for structural change (restore replaces the entire conversation)
        self.bump_structural_epoch();
    }

    /// If turn capture is active, append the current turn's tail items into
    /// `pre_replacement_messages` before an in-place mutation shifts or drops them.
    pub(super) fn snapshot_turn_slice(&mut self) {
        if let Some(cap) = &mut self.state.turn_capture {
            cap.pre_replacement_messages
                .extend_from_slice(Self::turn_tail(
                    &self.state.conversation,
                    cap.turn_start_offset,
                ));
        }
    }

    /// Re-base an active turn capture's start offset to the current conversation
    /// length after an in-place mutation, keeping the tail slice valid.
    pub(super) fn rebase_turn_capture_offset(&mut self) {
        if let Some(cap) = &mut self.state.turn_capture {
            cap.turn_start_offset = self.state.conversation.len();
        }
    }

    /// Fail-safe `conversation[offset..]` for turn capture: a capture accounting
    /// slip must never abort the user's session (a raw index here SIGABRT-crashed
    /// a live CLI), so an out-of-range offset yields an empty slice — loud in dev
    /// via `debug_assert!`, with a prod breadcrumb via `error!`.
    pub(super) fn turn_tail(
        conversation: &[ConversationItem],
        offset: usize,
    ) -> &[ConversationItem] {
        debug_assert!(
            offset <= conversation.len(),
            "turn_start_offset {offset} > len {}",
            conversation.len()
        );
        conversation.get(offset..).unwrap_or_else(|| {
            tracing::error!(
                offset,
                len = conversation.len(),
                "turn-capture offset past conversation end; trace tail dropped"
            );
            &[]
        })
    }

    /// Atomically apply a rolling-compaction replacement when the source still
    /// matches the epoch, range, and fingerprint captured by the worker.
    ///
    /// Only the selected range is changed. In particular, this path does not run
    /// whole-history repair or sanitization, because doing so would mutate items
    /// outside the CAS identity. Atomic-group validation belongs to the rolling
    /// planner before the job is launched.
    pub(super) async fn cas_splice_conversation(
        &mut self,
        identity: crate::types::CompactSourceIdentity,
        items: Vec<ConversationItem>,
        persistence: Option<crate::persistence::CompactionPersistenceMetadata>,
    ) -> crate::commands::CasSpliceResult {
        if identity.source_start >= identity.source_end
            || identity.source_end > self.state.conversation.len()
        {
            return crate::commands::CasSpliceResult::InvalidRange;
        }

        if self.state.structural_epoch != identity.expected_epoch {
            tracing::debug!(
                expected_epoch = identity.expected_epoch,
                current_epoch = self.state.structural_epoch,
                "CAS splice rejected: epoch mismatch"
            );
            return crate::commands::CasSpliceResult::Stale;
        }

        let source_items = &self.state.conversation[identity.source_start..identity.source_end];
        let actual_fingerprint = match fingerprint_conversation_items(source_items) {
            Ok(fingerprint) => fingerprint,
            Err(error) => {
                tracing::error!(?error, "CAS splice rejected: source fingerprint failed");
                return crate::commands::CasSpliceResult::Stale;
            }
        };
        if actual_fingerprint != identity.source_fingerprint {
            tracing::debug!(
                expected_fingerprint = %hex::encode(identity.source_fingerprint),
                actual_fingerprint = %hex::encode(actual_fingerprint),
                "CAS splice rejected: fingerprint mismatch"
            );
            return crate::commands::CasSpliceResult::Stale;
        }

        let mut new_conversation = Vec::with_capacity(
            identity.source_start
                + items.len()
                + (self.state.conversation.len() - identity.source_end),
        );
        new_conversation.extend_from_slice(&self.state.conversation[..identity.source_start]);
        new_conversation.extend(items);
        new_conversation.extend_from_slice(&self.state.conversation[identity.source_end..]);

        if let Some(metadata) = persistence {
            let request = crate::persistence::CompactionPersistenceRequest {
                metadata,
                previous_history: self.state.conversation.clone(),
                compacted_history: new_conversation.clone(),
            };
            match self.persistence.commit_compaction_and_ack(request).await {
                Ok(Ok(())) => {}
                Ok(Err(crate::persistence::CompactionPersistenceError::Committed(error))) => {
                    tracing::warn!(%error, "CAS splice marker committed with a later persistence error");
                }
                Ok(Err(crate::persistence::CompactionPersistenceError::Indeterminate(error))) => {
                    tracing::error!(%error, "CAS splice persistence state is indeterminate");
                    // Fail-stop before the actor can consume another queued mutation.
                    // The shell shares this token with its session loop and tears the
                    // session down while startup recovery reconciles the journal.
                    self.cancellation_token.cancel();
                    return crate::commands::CasSpliceResult::PersistenceIndeterminate;
                }
                Ok(Err(error)) => {
                    tracing::error!(%error, "CAS splice persistence commit failed");
                    return crate::commands::CasSpliceResult::PersistenceFailed;
                }
                Err(_) => {
                    tracing::error!("CAS splice persistence acknowledgement dropped");
                    // Once persistence accepted the transaction, a dropped reply
                    // cannot prove whether the marker committed. Treat it exactly
                    // like every other indeterminate outcome.
                    self.cancellation_token.cancel();
                    return crate::commands::CasSpliceResult::PersistenceIndeterminate;
                }
            }
            self.apply_conversation_replacement(new_conversation, true, false);
        } else {
            // Reuse the established compaction replacement path so token
            // overhead, turn capture, persistence, epoch updates, and events
            // stay consistent for legacy/test callers.
            self.replace_conversation(new_conversation, true);
        }
        crate::commands::CasSpliceResult::Applied
    }
}
