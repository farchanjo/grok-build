//! Rolling-compaction planning, sampling, and CAS application.
//!
//! The rolling sampler enriches the job snapshot exactly once per job
//! through the canonical media preflight
//! ([`SessionActor::prepare_compaction_source`], plan §14.3) and reuses the
//! pairing-safe enriched source for chunk planning, bisection, merge
//! preparation, and every route fallback. The job identity stays
//! fingerprinted on the raw items for CAS staleness detection.

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

    /// One media-enrichment preflight per rolling job (plan §14.3).
    ///
    /// The raw job snapshot is the only input; the canonical preflight seam
    /// ([`SessionActor::prepare_compaction_source`]) honors the
    /// `GROK_DISABLE_MEDIA_COMPACTION_ENRICH` kill switch and the resolved
    /// `compaction_enrichment` config flag with the same strict/best-effort
    /// policy as full-replace. Best-effort failures return the raw snapshot
    /// (placeholder path — the sampler's final sanitizer keeps its current
    /// behavior); strict failures fail the rolling job. The returned items
    /// are the pairing-safe enriched source reused for chunk planning,
    /// bisection, merge preparation, and every route fallback.
    async fn prepare_rolling_source(
        &self,
        job: &RollingCompactionJob,
    ) -> Result<Vec<ConversationItem>, String> {
        let prepared = self
            .prepare_compaction_source(&job.source_items)
            .await
            .map_err(|error| format!("rolling media preflight failed: {error}"))?;
        Ok(prepared.enriched)
    }

    async fn sample_rolling_source(&self, job: &RollingCompactionJob) -> Result<String, String> {
        let routes = self
            .prepare_compaction_routes()
            .await
            .map_err(|error| error.to_string())?;
        // One media-enrichment preflight per rolling job (plan §14.3): the
        // raw job snapshot is enriched exactly once, and the enriched source
        // is reused for chunk planning, bisection, merge preparation, and
        // every route fallback. Live history is never re-fetched and never
        // mutated; the CAS identity stays fingerprinted on the raw items
        // captured in the job.
        let enriched_source = self.prepare_rolling_source(job).await?;
        let counter = xai_chat_state::actor::state::EstimatedItemTokenCounter;
        let chunks = xai_grok_compaction::plan_rolling_subchunks(
            &enriched_source,
            &counter,
            0..enriched_source.len(),
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
            xai_chat_state::compaction_utils::conversation_contains_images(&job.source_items),
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
                enriched_source[range.clone()].to_vec(),
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
                        &enriched_source,
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
    use crate::agent::config::CompactionPreflightPolicy;
    use crate::agent::execution_backend::{ExecutionBackend, ExternalAgentKind};
    use crate::session::media::{
        CompactionAnalyzer, CompactionEnrichmentMode, fingerprint_snapshot,
        run_compaction_preflight,
    };
    use crate::session::persistence::PersistenceMsg;
    use crate::session::rolling_compaction::{RollingCompactionJob, RollingCompactionResult};
    use async_trait::async_trait;
    use base64::Engine as _;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::mpsc;
    use xai_chat_state::compaction_utils::{
        conversation_contains_images, prepare_conversation_for_summarization,
    };
    use xai_chat_state::types::CompactSourceIdentity;
    use xai_grok_compaction::{plan_rolling_bisect, plan_rolling_subchunks};
    use xai_grok_inference_types::{ContentPart, ConversationItem, ToolCall};
    use xai_grok_tools::media::backend::{
        MediaProvenance, MediaSemantics, MediaUnderstandingError, MediaUnderstandingRequest,
        MediaUnderstandingResult,
    };
    use xai_grok_tools::media::domain::{MediaCategory, MediaCategoryStrategy, MediaSource};

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

    // ── Rolling media-enrichment wiring (plan §14.3) ─────────────────────
    //
    // The rolling sampler must enrich the stable job snapshot exactly once
    // via the canonical preflight and reuse that enriched source for chunk
    // planning, bisection, merge preparation, and every route fallback.

    struct StubAnalyzer {
        calls: AtomicUsize,
        fail: bool,
    }

    #[async_trait]
    impl CompactionAnalyzer for StubAnalyzer {
        async fn analyze_for_compaction(
            &self,
            request: MediaUnderstandingRequest,
        ) -> Result<MediaUnderstandingResult, MediaUnderstandingError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                return Err(MediaUnderstandingError::AllRoutesExhausted(
                    "stub failure".to_string(),
                ));
            }
            let mut results = Vec::with_capacity(request.media.len());
            for source in &request.media {
                let digest = match source {
                    MediaSource::ArtifactRef { blob_hash } => blob_hash.clone(),
                    _ => String::new(),
                };
                results.push(MediaSemantics {
                    source: source.clone(),
                    category: MediaCategory::Image,
                    text: format!("semantics for {digest}"),
                    provenance: MediaProvenance {
                        provider: "stub".to_string(),
                        model: "stub-model".to_string(),
                        strategy: MediaCategoryStrategy::Native,
                    },
                });
            }
            Ok(MediaUnderstandingResult {
                results,
                attempts: vec![],
            })
        }
    }

    fn analyzer(fail: bool) -> StubAnalyzer {
        StubAnalyzer {
            calls: AtomicUsize::new(0),
            fail,
        }
    }

    fn data_url(seed: u8) -> String {
        let bytes = [seed; 64];
        format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        )
    }

    fn user_with_image(text: &str, seed: u8) -> ConversationItem {
        let mut user = ConversationItem::user(text);
        user.add_image(data_url(seed));
        user
    }

    fn tool_result_with_image(id: &str, seed: u8) -> ConversationItem {
        ConversationItem::tool_result_with_images(
            id,
            "tool text",
            vec![ContentPart::Image {
                url: std::sync::Arc::<str>::from(data_url(seed)),
            }],
        )
    }

    fn item_has_image_parts(item: &ConversationItem) -> bool {
        match item {
            ConversationItem::User(user) => user
                .content
                .iter()
                .any(|part| matches!(part, ContentPart::Image { .. })),
            ConversationItem::ToolResult(result) => !result.images.is_empty(),
            _ => false,
        }
    }

    /// The rolling job snapshot is enriched exactly once per job, and the
    /// enriched source is reused for chunk planning, bisection, merge
    /// preparation, and sampler-input preparation with zero additional
    /// backend calls.
    #[tokio::test]
    async fn rolling_job_preflight_enriches_once_and_reuses_source_for_chunk_bisect_merge() {
        let tmp = tempfile::tempdir().unwrap();
        let analyzer = analyzer(false);

        // The rolling job snapshot: a system item, two user items and a tool
        // result that all share the SAME image bytes, and a valid
        // assistant/tool-result pair.
        let source_items = vec![
            ConversationItem::system("sys"),
            user_with_image("look at this", 1),
            ConversationItem::assistant_tool_calls(vec![ToolCall {
                id: std::sync::Arc::<str>::from("tc-1"),
                name: "bash".to_string(),
                arguments: std::sync::Arc::<str>::from("{}"),
            }]),
            tool_result_with_image("tc-1", 1),
            user_with_image("same image again", 1),
        ];
        let job = RollingCompactionJob {
            identity: CompactSourceIdentity {
                expected_epoch: 0,
                source_start: 0,
                source_end: source_items.len(),
                source_fingerprint: [0; 32],
            },
            source_items: source_items.clone(),
            compactor_input_capacity: 100_000,
            prompt_index: 0,
            original_user_info: None,
        };

        // Exactly ONE canonical preflight for the whole job; the same image
        // bytes across user and tool-result items are deduplicated into a
        // single backend call.
        let prepared = run_compaction_preflight(
            &analyzer,
            tmp.path(),
            &job.source_items,
            CompactionEnrichmentMode::Enabled {
                policy: CompactionPreflightPolicy::BestEffort,
            },
        )
        .await
        .unwrap();
        assert_eq!(
            analyzer.calls.load(Ordering::SeqCst),
            1,
            "one preflight per job: duplicate image bytes across items share one backend call"
        );
        // The fingerprint stays on the RAW snapshot so CAS staleness
        // detection is unaffected by enrichment.
        assert_eq!(
            prepared.snapshot_fingerprint,
            fingerprint_snapshot(&job.source_items)
        );

        let enriched = prepared.enriched;
        assert_eq!(
            enriched.len(),
            source_items.len(),
            "pairing-safe enrichment preserves item count"
        );
        // Tool-call/result pairing is preserved.
        match &enriched[3] {
            ConversationItem::ToolResult(result) => {
                assert_eq!(result.tool_call_id, "tc-1");
                assert!(result.images.is_empty(), "tool result images cleared");
                assert!(result.content.starts_with("tool text"));
                assert!(
                    result.content.contains("<media_semantics"),
                    "semantic envelope rides in the tool result content"
                );
            }
            other => panic!("expected tool result, got {other:?}"),
        }
        assert!(
            !conversation_contains_images(&enriched),
            "the enriched rolling source is text-only"
        );
        // The caller's raw job snapshot is never mutated.
        assert!(
            conversation_contains_images(&job.source_items),
            "the preflight only transforms its own copy of the job snapshot"
        );

        // The rolling sampler's input pipeline reuses the ONE enriched
        // source: chunk planning, bisection, sampler-input preparation, and
        // merge preparation add ZERO backend calls.
        let counter = xai_chat_state::actor::state::EstimatedItemTokenCounter;
        let chunks = plan_rolling_subchunks(
            &enriched,
            &counter,
            0..enriched.len(),
            job.compactor_input_capacity,
        )
        .unwrap();
        assert!(!chunks.is_empty());
        let mut any_envelope = false;
        for chunk in &chunks {
            let prepared =
                prepare_conversation_for_summarization(enriched[chunk.range.clone()].to_vec());
            any_envelope |= prepared
                .iter()
                .any(|item| item.text_content().contains("<media_semantics"));
        }
        assert!(
            any_envelope,
            "media semantics reach the text-only rolling sampler"
        );

        let bisect = plan_rolling_bisect(&enriched, &counter, 0..enriched.len()).unwrap();
        assert!(
            bisect.left.range.start < bisect.right.range.end,
            "bisection stays valid on the enriched source"
        );

        // Merge preparation derives text-only summary items from the
        // enriched lineage and plans fine.
        let merge_items = chunks
            .iter()
            .enumerate()
            .map(|(index, chunk)| {
                ConversationItem::compaction_summary(format!(
                    "Chronological partial summary {}:\n{}",
                    index + 1,
                    enriched[chunk.range.clone()]
                        .first()
                        .map(|item| item.text_content())
                        .unwrap_or_default()
                ))
            })
            .collect::<Vec<_>>();
        let merge_chunks = plan_rolling_subchunks(
            &merge_items,
            &counter,
            0..merge_items.len(),
            job.compactor_input_capacity,
        )
        .unwrap();
        assert!(!merge_chunks.is_empty());

        assert_eq!(
            analyzer.calls.load(Ordering::SeqCst),
            1,
            "enriched-source reuse across chunking/bisection/merge adds no backend calls"
        );
    }

    /// Failure policy through the canonical preflight the rolling path
    /// calls: strict fails the job when semantics cannot be produced;
    /// best-effort and the disabled (kill-switch) path keep the raw
    /// placeholder behavior.
    #[tokio::test]
    async fn rolling_source_strict_fails_job_best_effort_falls_back() {
        let tmp = tempfile::tempdir().unwrap();
        let failing = analyzer(true);
        let raw = vec![user_with_image("u", 1)];

        // strict: required media semantics cannot be produced -> job fails.
        let error = run_compaction_preflight(
            &failing,
            tmp.path(),
            &raw,
            CompactionEnrichmentMode::Enabled {
                policy: CompactionPreflightPolicy::Strict,
            },
        )
        .await
        .unwrap_err();
        assert!(
            error.to_string().contains("preflight"),
            "strict surfaces a preflight error, got {error}"
        );

        // best_effort: the same backend failure keeps the placeholder path.
        let prepared = run_compaction_preflight(
            &failing,
            tmp.path(),
            &raw,
            CompactionEnrichmentMode::Enabled {
                policy: CompactionPreflightPolicy::BestEffort,
            },
        )
        .await
        .unwrap();
        assert_eq!(
            serde_json::to_value(&prepared.enriched).unwrap(),
            serde_json::to_value(&raw).unwrap(),
            "best_effort falls back to the raw snapshot"
        );
        assert!(
            item_has_image_parts(&prepared.enriched[0]),
            "best_effort keeps image parts for the final sanitizer"
        );

        // disabled: raw flow-through, exactly like the kill-switch path.
        let disabled = run_compaction_preflight(
            &failing,
            tmp.path(),
            &raw,
            CompactionEnrichmentMode::Disabled,
        )
        .await
        .unwrap();
        assert_eq!(
            serde_json::to_value(&disabled.enriched).unwrap(),
            serde_json::to_value(&raw).unwrap()
        );
    }

    /// The actor-level rolling seam routes through the canonical preflight:
    /// with no media context (enrichment unavailable), the raw job snapshot
    /// flows through unchanged and the sampler's final sanitizer remains the
    /// safety net.
    #[tokio::test(flavor = "current_thread")]
    async fn prepare_rolling_source_preserves_placeholder_path_when_enrichment_unavailable() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (gateway_tx, _) = mpsc::unbounded_channel::<xai_acp_lib::AcpClientMessage>();
                let (persistence_tx, _) = mpsc::unbounded_channel::<PersistenceMsg>();
                let actor =
                    create_test_actor_for_rolling(0, 256_000, gateway_tx, persistence_tx).await;

                // The test actor has no media context: enrichment is
                // unavailable and the canonical seam returns the raw job
                // snapshot (placeholder path).
                let raw = vec![
                    ConversationItem::system("sys"),
                    user_with_image("u1", 1),
                    ConversationItem::assistant("a"),
                    user_with_image("u2", 2),
                ];
                let job = RollingCompactionJob {
                    identity: CompactSourceIdentity {
                        expected_epoch: 0,
                        source_start: 0,
                        source_end: raw.len(),
                        source_fingerprint: [0; 32],
                    },
                    source_items: raw.clone(),
                    compactor_input_capacity: 100_000,
                    prompt_index: 0,
                    original_user_info: None,
                };

                let enriched = actor.prepare_rolling_source(&job).await.unwrap();
                assert_eq!(
                    serde_json::to_value(&enriched).unwrap(),
                    serde_json::to_value(&raw).unwrap(),
                    "no enrichment: the raw snapshot flows through unchanged"
                );
                assert!(
                    conversation_contains_images(&enriched),
                    "image parts are preserved for the final sanitizer"
                );
                assert!(
                    conversation_contains_images(&job.source_items),
                    "the job snapshot itself is never mutated"
                );

                // Even on the placeholder path the sampler's input
                // preparation strips images: the sanitizer stays the final
                // safety net.
                let counter = xai_chat_state::actor::state::EstimatedItemTokenCounter;
                let chunks = plan_rolling_subchunks(
                    &enriched,
                    &counter,
                    0..enriched.len(),
                    job.compactor_input_capacity,
                )
                .unwrap();
                for chunk in chunks {
                    let prepared =
                        prepare_conversation_for_summarization(enriched[chunk.range].to_vec());
                    assert!(
                        !conversation_contains_images(&prepared),
                        "the final sanitizer keeps sampler input text-only on the placeholder path"
                    );
                }
            })
            .await;
    }
}
