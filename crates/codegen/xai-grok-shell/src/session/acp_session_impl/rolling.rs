//! Rolling-compaction planning, sampling, and CAS application.

use super::*;
use crate::session::helpers::full_replace_compaction::ShellCompactionSampler;
use crate::session::rolling_compaction::{RollingCompactionJob, RollingCompactionResult};
use xai_grok_compaction::CompactionSampler;
use xai_grok_inference_types::SyntheticReason;

pub(super) fn rolling_fixed_prefix_count(
    conversation: &[ConversationItem],
    inherited_prefix_len: Option<usize>,
    inherited_prefix_released: bool,
) -> usize {
    if !inherited_prefix_released && let Some(prefix_len) = inherited_prefix_len {
        return prefix_len.min(conversation.len());
    }

    conversation
        .iter()
        .enumerate()
        .take_while(|(index, item)| match item {
            ConversationItem::System(_) => true,
            ConversationItem::User(user) => match user.synthetic_reason.as_ref() {
                Some(
                    SyntheticReason::CompactionMeta
                    | SyntheticReason::ProjectInstructions
                    | SyntheticReason::SystemReminder,
                ) => true,
                None if *index == 1 => item.text_content().trim_start().starts_with("<user_info>"),
                _ => false,
            },
            _ => false,
        })
        .count()
}

impl SessionActor {
    pub(super) async fn plan_rolling_compaction_job(
        &self,
    ) -> Result<Option<RollingCompactionJob>, String> {
        if self.execution_backend.get().is_external() {
            return Ok(None);
        }
        let policy = self.agent.borrow().compaction_policy().clone();
        if matches!(
            policy.strategy,
            xai_grok_agent::CompactionStrategy::FullReplace
        ) {
            return Ok(None);
        }

        let routes = self
            .prepare_compaction_routes()
            .await
            .map_err(|error| error.to_string())?;
        let compactor_window = routes
            .first()
            .map(|route| route.inference_config.context_window)
            .ok_or_else(|| "no compaction route configured".to_owned())?;
        let snapshot = self
            .chat_state_handle
            .snapshot()
            .await
            .ok_or_else(|| "chat-state actor unavailable".to_owned())?;
        if snapshot.conversation.len() < 3 {
            return Ok(None);
        }

        let fixed_prefix_count = rolling_fixed_prefix_count(
            &snapshot.conversation,
            self.startup_hints.inherited_prefix_len,
            self.compaction
                .prefix_released
                .load(std::sync::atomic::Ordering::Relaxed),
        );
        if fixed_prefix_count >= snapshot.conversation.len() {
            return Ok(None);
        }

        let fixed_prefix_tokens = xai_chat_state::estimate_conversation_tokens(
            &snapshot.conversation[..fixed_prefix_count],
        );
        const SUMMARY_OUTPUT_RESERVE_TOKENS: u64 = 32_768;
        const INSTRUCTION_RESERVE_TOKENS: u64 = 8_192;
        const TOKENIZER_SAFETY_MARGIN_TOKENS: u64 = 8_192;
        let budget =
            xai_grok_compaction::plan_rolling_budget(xai_grok_compaction::RollingBudgetInput {
                session_context_window: snapshot.inference_settings.context_window.get(),
                fixed_prefix_tokens,
                next_turn_output_reserve_tokens: SUMMARY_OUTPUT_RESERVE_TOKENS,
                request_safety_margin_tokens: TOKENIZER_SAFETY_MARGIN_TOKENS,
                band_count: policy.rolling_band_count as u64,
                compactor_context_window: compactor_window,
                summary_output_reserve_tokens: SUMMARY_OUTPUT_RESERVE_TOKENS,
                instruction_tokens: INSTRUCTION_RESERVE_TOKENS,
                tokenizer_safety_margin_tokens: TOKENIZER_SAFETY_MARGIN_TOKENS,
                tool_tax_tokens: 0,
            })
            .map_err(|error| format!("rolling budget failed: {error:?}"))?;

        let counter = xai_chat_state::actor::state::EstimatedItemTokenCounter;
        let protected_tail_count = xai_grok_compaction::plan_protected_tail_count(
            &snapshot.conversation,
            &counter,
            fixed_prefix_count,
            budget.nominal_band_target,
        )
        .map_err(|error| format!("rolling hot-tail planning failed: {error:?}"))?;
        let source = match xai_grok_compaction::plan_rolling_source(
            &snapshot.conversation,
            &counter,
            fixed_prefix_count,
            protected_tail_count,
            budget.nominal_band_target,
        ) {
            Ok(plan) => plan,
            Err(xai_grok_compaction::RollingSourceError::NoPlan) => return Ok(None),
            Err(error) => return Err(format!("rolling source planning failed: {error:?}")),
        };

        let source_items = snapshot.conversation[source.source_range.clone()].to_vec();
        let identity = xai_chat_state::CompactSourceIdentity::new(
            snapshot.structural_epoch,
            source.source_range.start,
            source.source_range.end,
            &source_items,
        )
        .map_err(|error| format!("rolling source fingerprint failed: {error}"))?;
        let original_user_info = snapshot
            .conversation
            .get(1)
            .map(ConversationItem::text_content)
            .filter(|text| !text.is_empty());

        Ok(Some(RollingCompactionJob {
            identity,
            source_items,
            compactor_input_capacity: budget.compactor_input_capacity,
            prompt_index: snapshot.prompt_index,
            original_user_info,
        }))
    }

    pub(super) async fn run_rolling_compaction_job(
        &self,
        job: RollingCompactionJob,
    ) -> RollingCompactionResult {
        let result = self.sample_rolling_source(&job).await;
        RollingCompactionResult {
            identity: job.identity,
            summary: result,
            prompt_index: job.prompt_index,
            original_user_info: job.original_user_info,
        }
    }

    async fn sample_rolling_source(&self, job: &RollingCompactionJob) -> Result<String, String> {
        let routes = self
            .prepare_compaction_routes()
            .await
            .map_err(|error| error.to_string())?;
        let counter = xai_chat_state::actor::state::EstimatedItemTokenCounter;
        let chunks = xai_grok_compaction::plan_rolling_subchunks(
            &job.source_items,
            &counter,
            0..job.source_items.len(),
            job.compactor_input_capacity,
        )
        .map_err(|error| format!("rolling subchunk planning failed: {error:?}"))?;
        let wall_clock_budget_secs = self
            .agent
            .borrow()
            .compaction_policy()
            .wall_clock_budget_secs;
        let sampler = ShellCompactionSampler::new(
            false,
            None,
            Vec::new(),
            Vec::new(),
            routes,
            self.session_info.id.clone(),
            self.inference_idle_timeout,
            wall_clock_budget_secs,
            crate::util::config::CompactionToolChoice::None,
        )
        .with_supplied_prompt();
        let chunk_prompt = xai_grok_compaction::CompactionPrompt {
            system: "You compress coding-agent history without losing actionable state."
                .to_owned(),
            user: "Summarize this chronological history chunk for a successor assistant. Preserve decisions, unresolved work, exact identifiers, file paths, errors, and user intent. Return a dense standalone summary; do not call tools."
                .to_owned(),
        };
        let mut pending =
            std::collections::VecDeque::from_iter(chunks.into_iter().map(|chunk| chunk.range));
        let mut partial_summaries = Vec::new();
        while let Some(range) = pending.pop_front() {
            let prepared = xai_chat_state::compaction_utils::prepare_conversation_for_summarization(
                job.source_items[range.clone()].to_vec(),
            );
            match sampler
                .sample_compaction(&prepared, &chunk_prompt, self.inference_idle_timeout)
                .await
            {
                Ok(output) => {
                    Self::validate_rolling_summary(&output.response, "chunk")?;
                    partial_summaries.push(output.response);
                }
                Err(error) if xai_grok_compaction::is_context_length_error(&error.to_string()) => {
                    let split = xai_grok_compaction::plan_rolling_bisect(
                        &job.source_items,
                        &counter,
                        range,
                    )
                    .map_err(|split_error| {
                        format!(
                            "rolling compactor context overflow could not be subdivided: {split_error:?}"
                        )
                    })?;
                    // Push right first so the left half remains the next item and
                    // chronological output order is preserved.
                    pending.push_front(split.right.range);
                    pending.push_front(split.left.range);
                }
                Err(error) => return Err(error.to_string()),
            }
        }

        const MAX_MERGE_LEVELS: usize = 16;
        let merge_prompt = xai_grok_compaction::CompactionPrompt {
            system: "You merge chronological coding-agent summaries without losing actionable state."
                .to_owned(),
            user: "Merge these chronological partial summaries into one faithful standalone summary. Preserve order, decisions, user intent, exact identifiers, errors, and pending work; remove repetition."
                .to_owned(),
        };
        for _ in 0..MAX_MERGE_LEVELS {
            if partial_summaries.len() == 1 {
                return Ok(partial_summaries.remove(0));
            }
            let merge_items = partial_summaries
                .into_iter()
                .enumerate()
                .map(|(index, summary)| {
                    ConversationItem::compaction_summary(format!(
                        "Chronological partial summary {}:\n{}",
                        index + 1,
                        summary
                    ))
                })
                .collect::<Vec<_>>();
            let merge_chunks = xai_grok_compaction::plan_rolling_subchunks(
                &merge_items,
                &counter,
                0..merge_items.len(),
                job.compactor_input_capacity,
            )
            .map_err(|error| format!("rolling merge planning failed: {error:?}"))?;
            let mut pending = std::collections::VecDeque::from_iter(
                merge_chunks.into_iter().map(|chunk| chunk.range),
            );
            let mut merged = Vec::new();
            while let Some(range) = pending.pop_front() {
                match sampler
                    .sample_compaction(
                        &merge_items[range.clone()],
                        &merge_prompt,
                        self.inference_idle_timeout,
                    )
                    .await
                {
                    Ok(output) => {
                        Self::validate_rolling_summary(&output.response, "merge")?;
                        merged.push(output.response);
                    }
                    Err(error)
                        if xai_grok_compaction::is_context_length_error(&error.to_string()) =>
                    {
                        let split = xai_grok_compaction::plan_rolling_bisect(
                            &merge_items,
                            &counter,
                            range,
                        )
                        .map_err(|split_error| {
                            format!(
                                "rolling merge context overflow could not be subdivided: {split_error:?}"
                            )
                        })?;
                        pending.push_front(split.right.range);
                        pending.push_front(split.left.range);
                    }
                    Err(error) => return Err(error.to_string()),
                }
            }
            if merged.len() >= merge_items.len() {
                return Err("rolling merge made no progress after subdivision".to_owned());
            }
            partial_summaries = merged;
        }
        Err("rolling merge exceeded the maximum hierarchy depth".to_owned())
    }

    fn validate_rolling_summary(summary: &str, stage: &str) -> Result<(), String> {
        if summary.trim().is_empty() || xai_grok_compaction::is_degenerate_summary(summary) {
            return Err(format!("rolling {stage} returned an unusable summary"));
        }
        Ok(())
    }

    pub(super) async fn apply_rolling_compaction_result(
        &self,
        result: RollingCompactionResult,
    ) -> xai_chat_state::CasSpliceResult {
        let summary = match result.summary {
            Ok(summary) => summary,
            Err(error) => {
                self.send_xai_notification(
                    crate::extensions::notification::SessionUpdate::AutoCompactFailed {
                        error: error.clone(),
                    },
                )
                .await;
                tracing::warn!(%error, "rolling compaction job failed");
                return xai_chat_state::CasSpliceResult::PersistenceFailed;
            }
        };
        let tokens_before = self.chat_state_handle.get_estimated_total_tokens().await;

        let metadata = xai_chat_state::CompactionPersistenceMetadata {
            checkpoint_id: uuid::Uuid::new_v4().to_string(),
            prompt_index: result.prompt_index,
            auto_continue_prompt: None,
            original_user_info: result.original_user_info,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        let outcome = self
            .chat_state_handle
            .cas_splice_conversation_with_persistence(
                result.identity,
                vec![ConversationItem::compaction_summary(summary)],
                Some(metadata),
            )
            .await;
        match outcome {
            xai_chat_state::CasSpliceResult::Applied => {
                self.chat_state_handle
                    .record_compaction_at(result.prompt_index);
                self.compaction
                    .count
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let tokens_after = self.chat_state_handle.get_estimated_total_tokens().await;
                self.send_xai_notification(
                    crate::extensions::notification::SessionUpdate::AutoCompactCompleted {
                        tokens_before: Some(tokens_before),
                        tokens_after,
                        elapsed_ms: None,
                        summary_preview: None,
                    },
                )
                .await;
                tracing::info!("rolling compaction applied");
            }
            xai_chat_state::CasSpliceResult::Stale => {
                self.send_xai_notification(
                    crate::extensions::notification::SessionUpdate::AutoCompactCancelled {
                        reason: "Conversation changed before the rolling summary could be applied"
                            .to_string(),
                    },
                )
                .await;
                tracing::debug!("discarded stale rolling compaction result");
            }
            xai_chat_state::CasSpliceResult::InvalidRange => {
                self.send_xai_notification(
                    crate::extensions::notification::SessionUpdate::AutoCompactFailed {
                        error: "Rolling compaction returned an invalid source range".to_string(),
                    },
                )
                .await;
                tracing::error!("rolling compaction returned an invalid source range");
            }
            xai_chat_state::CasSpliceResult::PersistenceFailed => {
                self.send_xai_notification(
                    crate::extensions::notification::SessionUpdate::AutoCompactFailed {
                        error: "Rolling compaction could not be committed durably".to_string(),
                    },
                )
                .await;
                tracing::error!("rolling compaction persistence commit failed");
            }
            xai_chat_state::CasSpliceResult::PersistenceIndeterminate => {
                self.send_xai_notification(
                    crate::extensions::notification::SessionUpdate::AutoCompactFailed {
                        error: "Rolling compaction persistence is indeterminate; restart the session to recover"
                            .to_string(),
                    },
                )
                .await;
                tracing::error!("rolling compaction persistence state is indeterminate");
            }
        }
        outcome
    }
}

#[cfg(test)]
mod tests {
    use super::{SessionActor, rolling_fixed_prefix_count};
    use crate::agent::execution_backend::{ExecutionBackend, ExternalAgentKind};
    use crate::session::persistence::PersistenceMsg;
    use crate::session::rolling_compaction::RollingCompactionResult;
    use tokio::sync::mpsc;
    use xai_chat_state::types::CompactSourceIdentity;
    use xai_grok_inference_types::ConversationItem;

    async fn create_test_actor_for_rolling(
        total_tokens: u64,
        context_window: u64,
        gateway_tx: mpsc::UnboundedSender<xai_acp_lib::AcpClientMessage>,
        persistence_tx: mpsc::UnboundedSender<PersistenceMsg>,
    ) -> SessionActor {
        super::super::support::create_test_actor(
            total_tokens,
            context_window,
            85,
            gateway_tx,
            persistence_tx,
        )
        .await
    }

    async fn next_xai_update(
        receiver: &mut mpsc::UnboundedReceiver<PersistenceMsg>,
    ) -> crate::extensions::notification::SessionUpdate {
        loop {
            let message = receiver.recv().await.expect("persistence channel closed");
            if let PersistenceMsg::Update(crate::session::storage::SessionUpdate::Xai(
                notification,
            )) = message
            {
                return notification.update;
            }
        }
    }

    #[test]
    fn fixed_prefix_keeps_runtime_prefix_but_not_summary_spine() {
        let conversation = vec![
            ConversationItem::system("system"),
            ConversationItem::user("<user_info>OS: macos</user_info>"),
            ConversationItem::project_instructions("project rules"),
            ConversationItem::system_reminder("startup reminder"),
            ConversationItem::compaction_summary("oldest summary"),
            ConversationItem::user("new work"),
        ];
        assert_eq!(rolling_fixed_prefix_count(&conversation, None, false), 4);
    }

    #[test]
    fn fixed_prefix_honors_unreleased_inherited_boundary() {
        let conversation = vec![
            ConversationItem::system("system"),
            ConversationItem::user("parent turn"),
            ConversationItem::assistant("parent answer"),
            ConversationItem::user("child turn"),
        ];
        assert_eq!(rolling_fixed_prefix_count(&conversation, Some(3), false), 3);
        assert_eq!(rolling_fixed_prefix_count(&conversation, Some(3), true), 1);
    }

    /// Test that `apply_rolling_compaction_result` sends the
    /// `AutoCompactCancelled` notification when CAS returns `Stale`.
    /// This verifies that stale rolling results are properly communicated
    /// to the client and the session continues cleanly.
    #[tokio::test(flavor = "current_thread")]
    async fn apply_rolling_compaction_result_sends_cancelled_on_stale_cas() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (gateway_tx, _gateway_rx) =
                    mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
                let (persistence_tx, mut persistence_rx) =
                    mpsc::unbounded_channel::<PersistenceMsg>();
                let actor =
                    create_test_actor_for_rolling(0, 256_000, gateway_tx, persistence_tx).await;

                actor
                    .chat_state_handle
                    .push_user_message(ConversationItem::system("system"));
                actor
                    .chat_state_handle
                    .push_user_message(ConversationItem::user("old turn"));
                let conversation = actor.chat_state_handle.get_conversation().await;

                // The range is valid, but the epoch deliberately cannot match.
                let identity = CompactSourceIdentity::new(
                    actor.chat_state_handle.get_structural_epoch().await + 1,
                    0,
                    conversation.len(),
                    &conversation,
                )
                .unwrap();

                let result = RollingCompactionResult {
                    identity,
                    summary: Ok("test summary".to_string()),
                    prompt_index: 0,
                    original_user_info: None,
                };

                actor.apply_rolling_compaction_result(result).await;

                let update = next_xai_update(&mut persistence_rx).await;
                let crate::extensions::notification::SessionUpdate::AutoCompactCancelled {
                    reason,
                } = update
                else {
                    panic!("expected AutoCompactCancelled, got {update:?}");
                };
                assert!(
                    reason.contains("Conversation changed"),
                    "reason should mention conversation changed: {reason}"
                );
            })
            .await;
    }

    /// Test that `apply_rolling_compaction_result` sends `AutoCompactFailed`
    /// notification when the rolling job fails (summary is Err).
    /// This is the result failure path that needs to communicate failure to the client.
    #[tokio::test(flavor = "current_thread")]
    async fn apply_rolling_compaction_result_sends_failed_on_summary_error() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (gateway_tx, _gateway_rx) =
                    mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
                let (persistence_tx, mut persistence_rx) =
                    mpsc::unbounded_channel::<PersistenceMsg>();
                let actor =
                    create_test_actor_for_rolling(0, 256_000, gateway_tx, persistence_tx).await;

                // Build a CAS identity (won't matter since summary is Err)
                let identity = CompactSourceIdentity {
                    expected_epoch: 0,
                    source_start: 0,
                    source_end: 2,
                    source_fingerprint: [0; 32],
                };

                let result = RollingCompactionResult {
                    identity,
                    summary: Err("sampling failed: context overflow".to_string()),
                    prompt_index: 0,
                    original_user_info: None,
                };

                actor.apply_rolling_compaction_result(result).await;

                let update = next_xai_update(&mut persistence_rx).await;
                let crate::extensions::notification::SessionUpdate::AutoCompactFailed { error } =
                    update
                else {
                    panic!("expected AutoCompactFailed, got {update:?}");
                };
                assert!(
                    error.contains("sampling failed"),
                    "error should contain the failure reason: {error}"
                );
            })
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn plan_rolling_returns_none_for_external_backend() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (gateway_tx, _) = mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
                let (persistence_tx, _) = mpsc::unbounded_channel::<PersistenceMsg>();
                let actor =
                    create_test_actor_for_rolling(0, 256_000, gateway_tx, persistence_tx).await;
                actor.execution_backend.set(ExecutionBackend::ExternalAgent(
                    ExternalAgentKind::ClaudeCli,
                ));

                let result = actor.plan_rolling_compaction_job().await;
                assert!(result.is_ok(), "external planning should not error");
                assert!(
                    result.unwrap().is_none(),
                    "host rolling compaction must not run for external-agent context"
                );
            })
            .await;
    }

    /// Test that `plan_rolling_compaction_job` returns `None` when
    /// the strategy is `FullReplace`, allowing the Auto semantics
    /// to fall back to full replacement.
    #[tokio::test(flavor = "current_thread")]
    async fn plan_rolling_returns_none_for_full_replace_strategy() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (gateway_tx, _) = mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
                let (persistence_tx, _) = mpsc::unbounded_channel::<PersistenceMsg>();
                let actor =
                    create_test_actor_for_rolling(0, 256_000, gateway_tx, persistence_tx).await;

                // Set strategy to FullReplace
                actor
                    .agent
                    .borrow_mut()
                    .set_compaction_policy(xai_grok_agent::CompactionPolicy {
                        strategy: xai_grok_agent::CompactionStrategy::FullReplace,
                        ..Default::default()
                    });

                let result = actor.plan_rolling_compaction_job().await;
                assert!(
                    result.is_ok(),
                    "plan_rolling should not error for FullReplace"
                );
                assert!(
                    result.unwrap().is_none(),
                    "plan_rolling should return None for FullReplace strategy"
                );
            })
            .await;
    }

    /// Test that `plan_rolling_compaction_job` returns `None` when
    /// conversation has fewer than 3 items.
    #[tokio::test(flavor = "current_thread")]
    async fn plan_rolling_returns_none_for_small_conversation() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (gateway_tx, _) = mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
                let (persistence_tx, _) = mpsc::unbounded_channel::<PersistenceMsg>();
                let actor =
                    create_test_actor_for_rolling(0, 256_000, gateway_tx, persistence_tx).await;

                // Conversation is empty by default - should return None
                let result = actor.plan_rolling_compaction_job().await;
                assert!(
                    result.is_ok(),
                    "plan_rolling should not error for small conversation"
                );
                assert!(
                    result.unwrap().is_none(),
                    "plan_rolling should return None for conversation with < 3 items"
                );
            })
            .await;
    }

    /// Test that `plan_rolling_compaction_job` returns `None` when
    /// the conversation is too small after fixed prefix (all items are
    /// part of the fixed prefix that should be preserved).
    #[tokio::test(flavor = "current_thread")]
    async fn plan_rolling_returns_none_when_all_items_are_fixed_prefix() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (gateway_tx, _) = mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
                let (persistence_tx, _) = mpsc::unbounded_channel::<PersistenceMsg>();
                let mut actor =
                    create_test_actor_for_rolling(0, 256_000, gateway_tx, persistence_tx).await;

                // Set inherited prefix length to match conversation size
                actor.startup_hints.inherited_prefix_len = Some(5);
                actor
                    .compaction
                    .prefix_released
                    .store(false, std::sync::atomic::Ordering::Relaxed);

                // Build a conversation with exactly 5 items
                actor
                    .chat_state_handle
                    .push_user_message(ConversationItem::system("system"));
                actor
                    .chat_state_handle
                    .push_user_message(ConversationItem::user("q1"));
                actor
                    .chat_state_handle
                    .push_user_message(ConversationItem::assistant("a1"));
                actor
                    .chat_state_handle
                    .push_user_message(ConversationItem::user("q2"));
                actor
                    .chat_state_handle
                    .push_user_message(ConversationItem::assistant("a2"));

                let result = actor.plan_rolling_compaction_job().await;
                assert!(result.is_ok(), "plan_rolling should not error");
                assert!(
                    result.unwrap().is_none(),
                    "plan_rolling should return None when fixed prefix covers all items"
                );
            })
            .await;
    }

    /// Test that `apply_rolling_compaction_result` sends `AutoCompactCompleted`
    /// notification on successful CAS application.
    #[tokio::test(flavor = "current_thread")]
    async fn apply_rolling_compaction_result_sends_completed_on_success() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (gateway_tx, _gateway_rx) =
                    mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
                let (persistence_tx, mut persistence_rx) =
                    mpsc::unbounded_channel::<PersistenceMsg>();
                let actor =
                    create_test_actor_for_rolling(0, 256_000, gateway_tx, persistence_tx).await;

                actor
                    .chat_state_handle
                    .push_user_message(ConversationItem::system("system"));
                actor
                    .chat_state_handle
                    .push_user_message(ConversationItem::user("old question"));
                actor
                    .chat_state_handle
                    .push_user_message(ConversationItem::assistant("old answer"));
                actor
                    .chat_state_handle
                    .push_user_message(ConversationItem::user("hot tail"));

                // Replace only the cold middle range, preserving both the
                // fixed system prefix and newest raw tail.
                let epoch = actor.chat_state_handle.get_structural_epoch().await;
                let conversation = actor.chat_state_handle.get_conversation().await;
                let identity =
                    CompactSourceIdentity::new(epoch, 1, 3, &conversation[1..3]).unwrap();

                let result = RollingCompactionResult {
                    identity,
                    summary: Ok("compacted summary".to_string()),
                    prompt_index: 0,
                    original_user_info: None,
                };

                actor.apply_rolling_compaction_result(result).await;

                let update = next_xai_update(&mut persistence_rx).await;
                let crate::extensions::notification::SessionUpdate::AutoCompactCompleted {
                    tokens_before,
                    tokens_after,
                    ..
                } = update
                else {
                    panic!("expected AutoCompactCompleted, got {update:?}");
                };
                assert!(tokens_before.is_some(), "tokens_before should be Some");
                assert!(tokens_after > 0, "tokens_after should be positive");
                assert_eq!(
                    actor
                        .chat_state_handle
                        .get_conversation()
                        .await
                        .iter()
                        .map(ConversationItem::text_content)
                        .collect::<Vec<_>>(),
                    vec!["system", "compacted summary", "hot tail"]
                );
                assert_eq!(
                    actor
                        .compaction
                        .count
                        .load(std::sync::atomic::Ordering::Relaxed),
                    1
                );
            })
            .await;
    }

    /// Test that `plan_rolling_compaction_job` returns `None` when
    /// all conversation items are part of the fixed prefix that should
    /// be preserved (inherited prefix not released). This verifies the
    /// edge case where rolling compaction cannot proceed.
    #[tokio::test(flavor = "current_thread")]
    async fn plan_rolling_returns_none_when_fixed_prefix_equals_conversation_len() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (gateway_tx, _) = mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
                let (persistence_tx, _) = mpsc::unbounded_channel::<PersistenceMsg>();
                let mut actor =
                    create_test_actor_for_rolling(0, 256_000, gateway_tx, persistence_tx).await;

                // Set inherited prefix length to match conversation size
                actor.startup_hints.inherited_prefix_len = Some(3);
                actor
                    .compaction
                    .prefix_released
                    .store(false, std::sync::atomic::Ordering::Relaxed);

                // Build a conversation with exactly 3 items
                actor
                    .chat_state_handle
                    .push_user_message(ConversationItem::system("system"));
                actor
                    .chat_state_handle
                    .push_user_message(ConversationItem::user("q1"));
                actor
                    .chat_state_handle
                    .push_user_message(ConversationItem::assistant("a1"));

                let result = actor.plan_rolling_compaction_job().await;
                assert!(result.is_ok(), "plan_rolling should not error");
                assert!(
                    result.unwrap().is_none(),
                    "plan_rolling should return None when fixed prefix equals conversation len"
                );
            })
            .await;
    }

    /// Test that `plan_rolling_compaction_job` returns `Some(job)` when
    /// conversation has enough items and strategy is not FullReplace.
    /// This verifies the happy path for rolling compaction planning.
    #[tokio::test(flavor = "current_thread")]
    async fn plan_rolling_returns_job_for_rolling_strategy() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (gateway_tx, _) = mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
                let (persistence_tx, _) = mpsc::unbounded_channel::<PersistenceMsg>();
                let actor =
                    create_test_actor_for_rolling(0, 256_000, gateway_tx, persistence_tx).await;

                actor
                    .agent
                    .borrow_mut()
                    .set_compaction_policy(xai_grok_agent::CompactionPolicy {
                        strategy: xai_grok_agent::CompactionStrategy::Rolling,
                        ..Default::default()
                    });

                // Four ~30K-token mutable items leave one protected hot band
                // and at least one older whole item eligible for compaction.
                let large = "x".repeat(120_000);
                actor
                    .chat_state_handle
                    .push_user_message(ConversationItem::system("system"));
                actor
                    .chat_state_handle
                    .push_user_message(ConversationItem::user(large.clone()));
                actor
                    .chat_state_handle
                    .push_user_message(ConversationItem::assistant(large.clone()));
                actor
                    .chat_state_handle
                    .push_user_message(ConversationItem::user(large.clone()));
                actor
                    .chat_state_handle
                    .push_user_message(ConversationItem::assistant(large));

                let result = actor.plan_rolling_compaction_job().await;
                assert!(
                    result.is_ok(),
                    "plan_rolling should not error for Rolling strategy"
                );
                let job = result.unwrap();
                assert!(
                    job.is_some(),
                    "plan_rolling should return Some for Rolling strategy with sufficient conversation"
                );

                // Verify the job has the expected identity
                let job = job.unwrap();
                assert!(job.identity.source_start == 1, "source_start should skip system message");
                assert!(
                    job.source_items.len() >= 1,
                    "should have source items to compact"
                );
            })
            .await;
    }

    /// Test that `apply_rolling_compaction_result` sends `AutoCompactFailed`
    /// notification when CAS returns `InvalidRange`.
    #[tokio::test(flavor = "current_thread")]
    async fn apply_rolling_compaction_result_sends_failed_on_invalid_range() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (gateway_tx, _gateway_rx) =
                    mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
                let (persistence_tx, mut persistence_rx) =
                    mpsc::unbounded_channel::<PersistenceMsg>();
                let actor =
                    create_test_actor_for_rolling(0, 256_000, gateway_tx, persistence_tx).await;

                // Build an invalid CAS identity (end > start)
                let identity = CompactSourceIdentity {
                    expected_epoch: 0,
                    source_start: 5,
                    source_end: 3, // Invalid: start > end
                    source_fingerprint: [0; 32],
                };

                let result = RollingCompactionResult {
                    identity,
                    summary: Ok("test summary".to_string()),
                    prompt_index: 0,
                    original_user_info: None,
                };

                actor.apply_rolling_compaction_result(result).await;

                let update = next_xai_update(&mut persistence_rx).await;
                let crate::extensions::notification::SessionUpdate::AutoCompactFailed { error } =
                    update
                else {
                    panic!("expected AutoCompactFailed, got {update:?}");
                };
                assert!(
                    error.contains("invalid source range"),
                    "error should mention invalid source range: {error}"
                );
            })
            .await;
    }
}
