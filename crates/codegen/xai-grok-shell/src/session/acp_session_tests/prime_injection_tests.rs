//! Composed production-path tests for PR19 skill-prime injection.
//!
//! These drive a real native `handle_prompt` (through the actual
//! `maybe_inject_prime_reminder` seam) with a prime-enabled retrieval
//! registry and an authoritative eligible-native-skills snapshot, and assert
//! the hidden `<skill_prime>` `SystemReminder` appears exactly once,
//! immediately before the real `User` item, only for explicit real `User`
//! origins — never for cron (SchedulerFired), sub-agent assignment, unknown,
//! external, or disabled/empty prime.

use super::support::*;
use super::*;

use crate::session::storage::updates_truncate_for_prompt;
use xai_grok_tools::implementations::skills::types::SkillInfo;

/// Prime-enabled actor with a real temp workspace containing a `SKILL.md` and
/// a seeded eligible-native-skill snapshot. When `hard_fail` is set, the prime
/// config uses `degrade_on_error = false` with a retrieval profile that does
/// not exist in the snapshot, so the semantic refinement fails hard and the
/// prime run returns a typed error (used to exercise the hard-failure index
/// gap). Returns the actor and test-local retrieval override guard.
async fn prime_enabled_actor(
    enabled: bool,
    skill_enabled: bool,
    hard_fail: bool,
) -> (
    SessionActor,
    tempfile::TempDir,
    crate::retrieval::TestRegistryOverride,
    std::sync::Arc<std::sync::Mutex<Vec<crate::session::storage::SessionUpdate>>>,
) {
    prime_enabled_actor_and_agents(enabled, skill_enabled, hard_fail, false).await
}

/// Same as [`prime_enabled_actor`] but also controls the agents selector.
async fn prime_enabled_actor_and_agents(
    enabled: bool,
    skill_enabled: bool,
    hard_fail: bool,
    agents_enabled: bool,
) -> (
    SessionActor,
    tempfile::TempDir,
    crate::retrieval::TestRegistryOverride,
    std::sync::Arc<std::sync::Mutex<Vec<crate::session::storage::SessionUpdate>>>,
) {
    let tmp = tempfile::TempDir::new().unwrap();
    let cwd = tmp.path().canonicalize().unwrap();
    let skill_dir = cwd.join("skills").join("deploy");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: deploy\ndescription: Deploy and release the application.\nmetadata:\n  grok:\n    when-to-use: use this when deploying or releasing\n---\nPRIME-SKILL-BODY deploy steps\n<script>alert(1)</script>\n",
    )
    .unwrap();
    let skill_path = skill_dir.join("SKILL.md");

    let (gateway_tx, _gateway_rx) = tokio::sync::mpsc::unbounded_channel();
    let (persistence_tx, mut persistence_rx) = tokio::sync::mpsc::unbounded_channel();
    let persisted_updates = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let persisted_updates_for_pump = persisted_updates.clone();
    // Resolve flush barriers so the persist-ack path completes deterministically
    // (mirrors the production persistence actor).
    tokio::task::spawn_local(async move {
        while let Some(msg) = persistence_rx.recv().await {
            match msg {
                PersistenceMsg::FlushAndAck { respond_to } => {
                    let _ = respond_to.send(());
                }
                PersistenceMsg::Update(update) => {
                    persisted_updates_for_pump.lock().unwrap().push(update);
                }
                _ => {}
            }
        }
    });
    let mut actor = create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await;
    // Repoint the session workspace at the temp dir (real filesystem).
    let abs = xai_grok_paths::AbsPathBuf::new(cwd.clone()).unwrap();
    actor.tool_context.cwd = abs;
    actor.session_info.cwd = cwd.to_string_lossy().to_string();

    // Seed the bridge's authoritative eligible-native-skill snapshot.
    let skill = SkillInfo {
        name: "deploy".into(),
        path: skill_path.to_string_lossy().to_string(),
        when_to_use: Some("use this when deploying or releasing".into()),
        body: Some("PRIME-SKILL-BODY deploy steps\n<script>alert(1)</script>\n".into()),
        ..SkillInfo::default()
    };
    {
        let bridge = actor.agent.borrow().tool_bridge().clone();
        bridge
            .seed_skill_discovery(
                Some(cwd.clone()),
                None,
                vec![skill],
                None,
                Some(256_000),
                None,
                xai_grok_tools::types::compat::CompatConfig::default(),
            )
            .await;
    }

    let reg = crate::retrieval::RetrievalRegistry::disabled(xai_grok_config::grok_home());
    let mut prime = xai_grok_config_types::PrimeConfig::default();
    prime.skills.enabled = skill_enabled;
    prime.agents.enabled = agents_enabled;
    if agents_enabled {
        // Let every callable candidate be selected for the accounting test
        // (builtins + any CLI-defined agent), so the recorded names are stable.
        prime.agents.max_results = 20;
    }
    if hard_fail {
        // Deterministic hard failure: `degrade_on_error = false` with a
        // retrieval profile that does not exist in the snapshot, so the
        // semantic refinement returns a hard error.
        prime.skills.degrade_on_error = false;
        prime.skills.retrieval_profile = Some("p-missing".into());
    }
    let snapshot = crate::retrieval::graph::RetrievalSnapshot {
        generation: 0,
        graph_generation: 0,
        provider_generation: 1,
        fingerprint: "pr19-test".into(),
        enabled,
        embedding_models: indexmap::IndexMap::new(),
        reranker_models: indexmap::IndexMap::new(),
        profiles: indexmap::IndexMap::new(),
        prime,
        memory_retrieval_profile: None,
        warnings: Vec::new(),
        source_graph: xai_grok_config_types::RetrievalGraphConfig::default(),
    };
    reg.force_publish(std::sync::Arc::new(snapshot));
    let registry_override = crate::retrieval::install_test_registry_override(reg);
    (actor, tmp, registry_override, persisted_updates)
}

/// Soft-degrade actor: skills enabled with a retrieval profile that is absent
/// from the snapshot, under the default `degrade_on_error = true`, so the run
/// completes with a safe `profile_missing` degradation instead of failing.
async fn prime_enabled_actor_with_degraded_snapshot(
    agents_enabled: bool,
) -> (
    SessionActor,
    tempfile::TempDir,
    crate::retrieval::TestRegistryOverride,
    std::sync::Arc<std::sync::Mutex<Vec<crate::session::storage::SessionUpdate>>>,
) {
    let tmp = tempfile::TempDir::new().unwrap();
    let cwd = tmp.path().canonicalize().unwrap();
    let skill_dir = cwd.join("skills").join("deploy");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: deploy\ndescription: Deploy and release the application.\nmetadata:\n  grok:\n    when-to-use: use this when deploying or releasing\n---\nPRIME-SKILL-BODY deploy steps\n",
    )
    .unwrap();
    let skill_path = skill_dir.join("SKILL.md");

    let (gateway_tx, _gateway_rx) = tokio::sync::mpsc::unbounded_channel();
    let (persistence_tx, mut persistence_rx) = tokio::sync::mpsc::unbounded_channel();
    let persisted_updates = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let persisted_updates_for_pump = persisted_updates.clone();
    tokio::task::spawn_local(async move {
        while let Some(msg) = persistence_rx.recv().await {
            match msg {
                PersistenceMsg::FlushAndAck { respond_to } => {
                    let _ = respond_to.send(());
                }
                PersistenceMsg::Update(update) => {
                    persisted_updates_for_pump.lock().unwrap().push(update);
                }
                _ => {}
            }
        }
    });

    let mut actor = create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await;
    let abs = xai_grok_paths::AbsPathBuf::new(cwd.clone()).unwrap();
    actor.tool_context.cwd = abs;
    actor.session_info.cwd = cwd.to_string_lossy().to_string();
    let skill = SkillInfo {
        name: "deploy".into(),
        path: skill_path.to_string_lossy().to_string(),
        when_to_use: Some("use this when deploying or releasing".into()),
        body: Some("PRIME-SKILL-BODY deploy steps\n".into()),
        ..SkillInfo::default()
    };
    {
        let bridge = actor.agent.borrow().tool_bridge().clone();
        bridge
            .seed_skill_discovery(
                Some(cwd.clone()),
                None,
                vec![skill],
                None,
                Some(256_000),
                None,
                xai_grok_tools::types::compat::CompatConfig::default(),
            )
            .await;
    }

    let reg = crate::retrieval::RetrievalRegistry::disabled(xai_grok_config::grok_home());
    let mut prime = xai_grok_config_types::PrimeConfig::default();
    prime.skills.enabled = true;
    prime.skills.retrieval_profile = Some("p-missing".into());
    prime.agents.enabled = agents_enabled;
    if agents_enabled {
        prime.agents.max_results = 20;
    }
    let snapshot = crate::retrieval::graph::RetrievalSnapshot {
        generation: 3,
        graph_generation: 2,
        provider_generation: 1,
        fingerprint: "pr22-degraded".into(),
        enabled: true,
        embedding_models: indexmap::IndexMap::new(),
        reranker_models: indexmap::IndexMap::new(),
        profiles: indexmap::IndexMap::new(),
        prime,
        memory_retrieval_profile: None,
        warnings: Vec::new(),
        source_graph: xai_grok_config_types::RetrievalGraphConfig::default(),
    };
    reg.force_publish(std::sync::Arc::new(snapshot));
    let registry_override = crate::retrieval::install_test_registry_override(reg);
    (actor, tmp, registry_override, persisted_updates)
}

/// Drive `handle_prompt` synchronously through the persist-ack barrier (fired
/// after the user pair is pushed/persisted, before inference) and return the
/// actor, the spawned task, and the resolved conversation snapshot.
async fn drive_prompt(
    actor: std::sync::Arc<SessionActor>,
    prompt_id: &str,
    origin: crate::session::PromptOrigin,
    text: &str,
) -> (
    tokio::task::JoinHandle<crate::session::PromptTurnResult>,
    Vec<xai_grok_inference_types::ConversationItem>,
) {
    let (ack_tx, ack_rx) = tokio::sync::oneshot::channel();
    let prompt_blocks = vec![acp::ContentBlock::Text(acp::TextContent::new(
        text.to_string(),
    ))];
    let actor_for_prompt = actor.clone();
    let prompt_id = prompt_id.to_string();
    let prompt_task = tokio::task::spawn_local(async move {
        actor_for_prompt
            .handle_prompt(
                &prompt_id,
                origin,
                prompt_blocks,
                PromptMode::Agent,
                None,
                None,
                None,
                None,
                true,
                None,
                Some(ack_tx),
                None,
            )
            .await
    });
    assert!(
        ack_rx.await.is_ok(),
        "persist ack must resolve after the user pair is durably accepted"
    );
    let conv = actor.chat_state_handle.get_conversation().await;
    (prompt_task, conv)
}

fn reminder_items<'a>(
    conv: &'a [xai_grok_inference_types::ConversationItem],
) -> Vec<&'a xai_grok_inference_types::ConversationItem> {
    conv.iter()
        .filter(|item| {
            matches!(
                item,
                xai_grok_inference_types::ConversationItem::User(u)
                    if u.synthetic_reason
                        == Some(xai_grok_inference_types::SyntheticReason::SystemReminder)
            )
        })
        .collect()
}

/// Composed production path: a real native `User` turn with prime enabled and
/// an authoritative eligible skill injects exactly one hidden `<skill_prime>`
/// `SystemReminder` immediately before the real `User` item, before inference,
/// with no echo duplication (exactly [reminder, user] in the conversation).
#[tokio::test(flavor = "current_thread")]
#[serial_test::serial(prime_registry)]
async fn real_user_turn_injects_hidden_skill_prime_reminder_before_user() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, _workspace, _registry, _persisted_updates) =
                prime_enabled_actor(true, true, false).await;
            let actor = std::sync::Arc::new(actor);
            let (prompt_task, conv) = drive_prompt(
                actor.clone(),
                "pr19-user",
                crate::session::PromptOrigin::User,
                "deploy the release now",
            )
            .await;

            assert_eq!(
                conv.len(),
                2,
                "exactly [reminder, user], no echo duplication"
            );
            let reminders = reminder_items(&conv);
            assert_eq!(reminders.len(), 1, "exactly one hidden prime reminder");
            let reminder_text = reminders[0].text_content();
            assert!(
                reminder_text.contains("<skill_prime>"),
                "reminder must carry the PR18 rendered block"
            );
            assert!(
                reminder_text.contains("deploy"),
                "reminder must carry the selected skill body"
            );
            assert!(
                !reminder_text.contains("</skill_prime><script>"),
                "PR18 escaping must survive insertion (no breakout)"
            );
            let second = &conv[1];
            assert!(
                matches!(
                    second,
                    xai_grok_inference_types::ConversationItem::User(u)
                        if u.synthetic_reason.is_none()
                            && second.text_content().contains("deploy the release now")
                ),
                "the real user item must directly follow the reminder"
            );
            let reminder_index = match reminders[0] {
                xai_grok_inference_types::ConversationItem::User(user) => user.prompt_index,
                _ => None,
            };
            let user_index = match second {
                xai_grok_inference_types::ConversationItem::User(user) => user.prompt_index,
                _ => None,
            };
            assert_eq!(reminder_index, Some(0));
            assert_eq!(reminder_index, user_index);
            prompt_task.abort();
        })
        .await;
}

/// Cron (`/loop`) turns arrive as the typed `SchedulerFired` origin (via the
/// ACP prompt-origin meta tag): they must NOT prime and must shape as
/// `ConversationItem::scheduler_fired`, not a plain user turn.
#[tokio::test(flavor = "current_thread")]
#[serial_test::serial(prime_registry)]
async fn scheduler_fired_cron_never_primes_and_shapes_scheduler_fired() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, _workspace, _registry, _persisted_updates) =
                prime_enabled_actor(true, true, false).await;
            let actor = std::sync::Arc::new(actor);
            // The typed origin the shell reader produces for the pager's
            // `_meta.promptOrigin: "scheduler_fired"` stamp.
            let cron_origin = crate::session::PromptOrigin::from_prompt_origin_meta(Some(
                crate::session::PROMPT_ORIGIN_SCHEDULER_FIRED,
            ));
            assert_eq!(cron_origin, crate::session::PromptOrigin::SchedulerFired);
            let (prompt_task, conv) = drive_prompt(
                actor.clone(),
                "scheduler-fired-1",
                cron_origin,
                "scheduler tick",
            )
            .await;

            assert!(
                reminder_items(&conv).is_empty(),
                "a cron turn must never prime"
            );
            assert_eq!(conv.len(), 1, "only the scheduler user item is pushed");
            assert!(
                matches!(
                    &conv[0],
                    xai_grok_inference_types::ConversationItem::User(u)
                        if u.synthetic_reason
                            == Some(xai_grok_inference_types::SyntheticReason::SchedulerFired)
                ),
                "cron must shape as ConversationItem::scheduler_fired"
            );
            prompt_task.abort();
        })
        .await;
}

/// A child sub-agent's initial prompt is parent-authored (`SubagentAssignment`):
/// it must never prime. A later explicit real `User` turn in the child still
/// primes (child prime is not disabled globally).
#[tokio::test(flavor = "current_thread")]
#[serial_test::serial(prime_registry)]
async fn subagent_assignment_never_primes_but_later_user_turn_does() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, _workspace, _registry, _persisted_updates) =
                prime_enabled_actor(true, true, false).await;
            let actor = std::sync::Arc::new(actor);
            let (prompt_task, conv) = drive_prompt(
                actor.clone(),
                "subagent-assignment-1",
                crate::session::PromptOrigin::SubagentAssignment,
                "do the task",
            )
            .await;
            assert!(
                reminder_items(&conv).is_empty(),
                "a parent-authored assignment must never prime"
            );
            assert_eq!(conv.len(), 1, "only the assignment user item is pushed");
            prompt_task.abort();

            // A later explicit real user turn primes (own cache/workspace).
            let (prompt_task2, conv2) = drive_prompt(
                actor.clone(),
                "subagent-user-2",
                crate::session::PromptOrigin::User,
                "deploy the release now",
            )
            .await;
            assert_eq!(
                reminder_items(&conv2).len(),
                1,
                "a later explicit user turn in the child may prime"
            );
            prompt_task2.abort();
        })
        .await;
}

/// Unknown/legacy origins (fail-closed) never prime.
#[tokio::test(flavor = "current_thread")]
#[serial_test::serial(prime_registry)]
async fn unknown_origin_never_primes() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, _workspace, _registry, _persisted_updates) =
                prime_enabled_actor(true, true, false).await;
            let actor = std::sync::Arc::new(actor);
            let (prompt_task, conv) = drive_prompt(
                actor.clone(),
                "legacy-1",
                crate::session::PromptOrigin::Unknown,
                "legacy text",
            )
            .await;
            assert!(
                reminder_items(&conv).is_empty(),
                "an Unknown origin must never prime"
            );
            assert_eq!(conv.len(), 1);
            prompt_task.abort();
        })
        .await;
}

/// An external backend never primes an otherwise eligible user turn.
#[tokio::test(flavor = "current_thread")]
#[serial_test::serial(prime_registry)]
async fn external_backend_user_turn_omits_prime_reminder() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, _workspace, _registry, _persisted_updates) =
                prime_enabled_actor(true, true, false).await;
            let actor = std::sync::Arc::new(actor);
            actor.execution_backend.set(
                crate::agent::execution_backend::ExecutionBackend::ExternalAgent(
                    crate::agent::execution_backend::ExternalAgentKind::ClaudeCli,
                ),
            );
            let result = actor
                .maybe_inject_prime_reminder(
                    &crate::session::PromptOrigin::User,
                    "deploy the release now",
                    &tokio_util::sync::CancellationToken::new(),
                )
                .await
                .unwrap();
            assert!(
                result.reminder.is_none(),
                "external execution must never prime"
            );
            assert!(
                matches!(result.accounting, PrimeAccounting::Unchanged),
                "an external backend must not overwrite the last real-turn outcome"
            );
        })
        .await;
}

/// Disabled prime omits the reminder entirely (user item still flows).
#[tokio::test(flavor = "current_thread")]
#[serial_test::serial(prime_registry)]
async fn disabled_prime_omits_reminder() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, _workspace, _registry, _persisted_updates) =
                prime_enabled_actor(true, false, false).await;
            let actor = std::sync::Arc::new(actor);
            let (prompt_task, conv) = drive_prompt(
                actor.clone(),
                "pr19-disabled",
                crate::session::PromptOrigin::User,
                "deploy the release now",
            )
            .await;
            assert!(
                reminder_items(&conv).is_empty(),
                "disabled prime must omit the reminder"
            );
            assert_eq!(conv.len(), 1, "user item still flows");
            prompt_task.abort();
        })
        .await;
}

/// Pre-cancelled turn: prime cancels and discards partial work; the real user
/// item lifecycle is unaffected (no reminder, user item pushed normally).
#[tokio::test(flavor = "current_thread")]
#[serial_test::serial(prime_registry)]
async fn pre_cancelled_turn_omits_prime_but_keeps_user_item() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, _workspace, _registry, _persisted_updates) =
                prime_enabled_actor(true, true, false).await;
            let actor = std::sync::Arc::new(actor);
            // Cancel the turn before the prompt runs: prime sees the shared
            // turn-cancel token and returns cancelled (no reminder); the user
            // item is still pushed.
            actor.turn_cancel.borrow().clone().cancel();
            let (prompt_task, conv) = drive_prompt(
                actor.clone(),
                "pr19-pre-cancel",
                crate::session::PromptOrigin::User,
                "deploy the release now",
            )
            .await;
            assert!(
                reminder_items(&conv).is_empty(),
                "a pre-cancelled prime must discard partial content"
            );
            assert_eq!(conv.len(), 1, "the real user item lifecycle is unaffected");
            prompt_task.abort();
        })
        .await;
}

/// Queue-level cron property: a `SchedulerFired` prompt is queued WITHOUT a
/// shared-queue row and (being synthetic) never enters normal real-user prompt
/// history.
#[tokio::test(flavor = "current_thread")]
#[serial_test::serial(prime_registry)]
async fn scheduler_fired_queued_without_queue_row_or_history() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, _workspace, _registry, _persisted_updates) =
                prime_enabled_actor(true, true, false).await;
            let (respond_to, _rx) = tokio::sync::oneshot::channel();
            let _ = actor
                .queue_input(
                    vec![acp::ContentBlock::Text(acp::TextContent::new("tick"))],
                    "scheduler-fired-1".into(),
                    crate::session::PromptOrigin::SchedulerFired,
                    PromptMode::Agent,
                    None,
                    None,
                    None,
                    None,
                    true,
                    None,
                    false,
                    None,
                    None,
                    respond_to,
                    None,
                    None,
                )
                .await;
            let state = actor.state.lock().await;
            let item = state.pending_inputs.front().unwrap();
            assert_eq!(item.origin, crate::session::PromptOrigin::SchedulerFired);
            assert!(
                item.queue_meta.is_none(),
                "a cron turn must never appear as a shared-queue row"
            );
            assert!(
                item.origin.is_synthetic(),
                "a cron turn must be synthetic (excluded from real-user prompt history)"
            );
        })
        .await;
}

/// N2: a hard prime failure (`degrade_on_error=false`) must NEVER reuse an
/// already-broadcast prompt index. The index stays MONOTONIC (the failed turn
/// consumes a documented gap), so the next real turn on the same session gets
/// a distinct, higher index and a coherent terminal-failure -> next-turn
/// sequence on the replay/updates rail.
#[tokio::test(flavor = "current_thread")]
#[serial_test::serial(prime_registry)]
async fn hard_prime_failure_leaves_monotonic_index_gap_then_success_primes_distinct_index() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, _workspace, _registry, persisted_updates) =
                prime_enabled_actor(true, true, true).await;
            let actor = std::sync::Arc::new(actor);

            // Turn 1: hard prime failure. The user pair is never emitted or
            // persisted, so await the task result directly.
            let (ack_tx, _ack_rx) = tokio::sync::oneshot::channel();
            let prompt_blocks = vec![acp::ContentBlock::Text(acp::TextContent::new(
                "deploy the release now",
            ))];
            let actor_for_prompt = actor.clone();
            let prompt_task = tokio::task::spawn_local(async move {
                actor_for_prompt
                    .handle_prompt(
                        "pr19-hard-fail",
                        crate::session::PromptOrigin::User,
                        prompt_blocks,
                        PromptMode::Agent,
                        None,
                        None,
                        None,
                        None,
                        true,
                        None,
                        Some(ack_tx),
                        None,
                    )
                    .await
            });
            let result = prompt_task.await.unwrap();
            assert!(
                result.is_err(),
                "hard prime failure must return a typed error before user insertion"
            );
            // The index remains monotonic although no user item was inserted.
            assert_eq!(
                actor.chat_state_handle.get_prompt_index().await,
                1,
                "failed hard-prime turn must leave an index gap without a replayed echo"
            );
            let conv_after_failure = actor.chat_state_handle.get_conversation().await;
            assert!(
                conv_after_failure.is_empty(),
                "a hard prime failure must not insert any user item"
            );
            tokio::task::yield_now().await;
            let updates = persisted_updates.lock().unwrap().clone();
            assert!(
                updates.iter().all(|update| !matches!(
                    update,
                    crate::session::storage::SessionUpdate::Acp(notification)
                        if matches!(notification.update, acp::SessionUpdate::UserMessageChunk(_))
                )),
                "a hard prime failure must not enqueue a durable UserMessageChunk"
            );
            let real_user = |text: &str, prompt_index: usize| {
                let mut meta = acp::Meta::new();
                meta.insert("promptIndex".into(), serde_json::json!(prompt_index));
                crate::session::storage::SessionUpdate::Acp(Box::new(
                    acp::SessionNotification::new(
                        acp::SessionId::new("prime-rail"),
                        acp::SessionUpdate::UserMessageChunk(
                            acp::ContentChunk::new(acp::ContentBlock::Text(acp::TextContent::new(
                                text,
                            )))
                            .meta(Some(meta)),
                        ),
                    ),
                ))
            };
            let agent_boundary = |text: &str| {
                crate::session::storage::SessionUpdate::Acp(Box::new(
                    acp::SessionNotification::new(
                        acp::SessionId::new("prime-rail"),
                        acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
                            acp::ContentBlock::Text(acp::TextContent::new(text)),
                        )),
                    ),
                ))
            };
            let prior = real_user("prior real turn", 0);
            let next = real_user("next real turn", 2);
            let mut no_phantom = vec![
                prior.clone(),
                agent_boundary("prior response"),
                next.clone(),
            ];
            no_phantom.extend(updates.clone());
            let mut with_phantom = vec![
                prior,
                agent_boundary("prior response"),
                real_user("phantom failed turn", 0),
                agent_boundary("phantom response"),
                next,
            ];
            with_phantom.extend(updates);
            assert_eq!(
                updates_truncate_for_prompt(&no_phantom, 1),
                no_phantom.len()
            );
            assert!(
                updates_truncate_for_prompt(&with_phantom, 1) < with_phantom.len(),
                "a persisted phantom must add a truncate-visible user run"
            );

            let write_updates =
                |dir: &std::path::Path,
                 name: &str,
                 rail: &[crate::session::storage::SessionUpdate]| {
                    let path = dir.join(name);
                    let mut jsonl = Vec::new();
                    for update in rail {
                        let envelope =
                            crate::session::storage::SessionUpdateEnvelope::from_update(update)
                                .unwrap();
                        let mut line = serde_json::to_vec(&envelope).unwrap();
                        line.push(b'\n');
                        jsonl.extend(line);
                    }
                    std::fs::write(&path, jsonl).unwrap();
                    path
                };
            let replay_dir = tempfile::TempDir::new().unwrap();
            let no_phantom_path = write_updates(replay_dir.path(), "no-phantom.jsonl", &no_phantom);
            let phantom_path =
                write_updates(replay_dir.path(), "with-phantom.jsonl", &with_phantom);
            let no_phantom_replay = crate::session::helpers::replay::replay_to_prompt(
                &no_phantom_path,
                replay_dir.path(),
                1,
            )
            .unwrap();
            let phantom_replay = crate::session::helpers::replay::replay_to_prompt(
                &phantom_path,
                replay_dir.path(),
                1,
            )
            .unwrap();
            assert_eq!(
                no_phantom_replay
                    .conversation
                    .iter()
                    .map(xai_grok_inference_types::ConversationItem::text_content)
                    .collect::<Vec<_>>(),
                vec!["prior real turn", "prior response"]
            );
            assert!(
                phantom_replay
                    .conversation
                    .iter()
                    .any(|item| item.text_content().contains("phantom failed turn")),
                "a persisted phantom must survive replay at target index 1"
            );

            // Turn 2 on the same session: reinstall a healthy registry and
            // drive a real user turn with the next distinct index.
            let reg = crate::retrieval::RetrievalRegistry::disabled(xai_grok_config::grok_home());
            let mut prime = xai_grok_config_types::PrimeConfig::default();
            prime.skills.enabled = true;
            let snapshot = crate::retrieval::graph::RetrievalSnapshot {
                generation: 1,
                graph_generation: 0,
                provider_generation: 1,
                fingerprint: "pr19-test-2".into(),
                enabled: true,
                embedding_models: indexmap::IndexMap::new(),
                reranker_models: indexmap::IndexMap::new(),
                profiles: indexmap::IndexMap::new(),
                prime,
                memory_retrieval_profile: None,
                warnings: Vec::new(),
                source_graph: xai_grok_config_types::RetrievalGraphConfig::default(),
            };
            reg.force_publish(std::sync::Arc::new(snapshot));
            let _healthy_registry = crate::retrieval::install_test_registry_override(reg);

            let (prompt_task2, conv2) = drive_prompt(
                actor.clone(),
                "pr19-hard-success",
                crate::session::PromptOrigin::User,
                "deploy the release now",
            )
            .await;
            assert_eq!(
                reminder_items(&conv2).len(),
                1,
                "the next real user turn must prime"
            );
            let user_item_index = conv2.iter().find_map(|item| match item {
                xai_grok_inference_types::ConversationItem::User(u)
                    if u.synthetic_reason.is_none() =>
                {
                    u.prompt_index
                }
                _ => None,
            });
            assert_eq!(
                user_item_index,
                Some(1),
                "the next real turn must use the next distinct index"
            );
            assert_eq!(
                actor.chat_state_handle.get_prompt_index().await,
                2,
                "indices stay monotonic across failure + success"
            );
            prompt_task2.abort();
        })
        .await;
}

/// A successful real native turn records a `primed` outcome with the final
/// selected skill names and the post-truncation injected budgets, and the
/// snapshot generations actually used by the run.
#[tokio::test(flavor = "current_thread")]
#[serial_test::serial(prime_registry)]
async fn real_user_turn_records_primed_outcome_with_skill_names_and_injected_budgets() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, _workspace, _registry, _persisted_updates) =
                prime_enabled_actor(true, true, false).await;
            let actor = std::sync::Arc::new(actor);
            let (prompt_task, _conv) = drive_prompt(
                actor.clone(),
                "pr22-accounting-1",
                crate::session::PromptOrigin::User,
                "deploy the release now",
            )
            .await;
            let outcome = actor
                .last_prime_outcome
                .borrow()
                .clone()
                .expect("a real user turn must record an outcome");
            assert_eq!(outcome.status, PrimeOutcomeStatus::Primed);
            assert!(
                outcome.primed_skill_names.contains(&"deploy".to_string()),
                "skill names from the final selection budget: {:?}",
                outcome.primed_skill_names
            );
            assert!(outcome.recommended_agent_names.is_empty());
            assert!(
                outcome.injected_chars > 0,
                "injected chars must be final rendered"
            );
            assert!(
                outcome.injected_tokens > 0,
                "injected tokens must be final rendered"
            );
            // `force_publish` assigns the first published test snapshot
            // generation 1; accounting must report that loaded value rather
            // than the pre-publication value in the fixture.
            assert_eq!(outcome.retrieval_snapshot_generation, Some(1));
            assert_eq!(outcome.provider_generation, Some(1));
            assert!(outcome.degradation.is_empty());
            prompt_task.abort();
        })
        .await;
}

/// Eligible + native + registry with BOTH selectors disabled records a
/// truthful `disabled` outcome (empty names, zero budgets), not a remap.
#[tokio::test(flavor = "current_thread")]
#[serial_test::serial(prime_registry)]
async fn real_user_turn_skills_disabled_agents_disabled_records_disabled() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, _workspace, _registry, _persisted_updates) =
                prime_enabled_actor(true, false, false).await;
            let actor = std::sync::Arc::new(actor);
            let (prompt_task, _conv) = drive_prompt(
                actor.clone(),
                "pr22-disabled-accounting",
                crate::session::PromptOrigin::User,
                "deploy the release now",
            )
            .await;
            let outcome = actor
                .last_prime_outcome
                .borrow()
                .clone()
                .expect("a disabled-but-eligible turn must still record");
            assert_eq!(outcome.status, PrimeOutcomeStatus::Disabled);
            assert!(outcome.primed_skill_names.is_empty());
            assert!(outcome.recommended_agent_names.is_empty());
            assert_eq!(outcome.injected_chars, 0);
            assert_eq!(outcome.injected_tokens, 0);
            prompt_task.abort();
        })
        .await;
}

/// Synthetic origins (scheduler/subagent assignment/unknown) must NOT
/// overwrite the last real-turn outcome.
#[tokio::test(flavor = "current_thread")]
#[serial_test::serial(prime_registry)]
async fn synthetic_origins_do_not_overwrite_last_prime_outcome() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, _workspace, _registry, _persisted_updates) =
                prime_enabled_actor(true, true, false).await;
            let actor = std::sync::Arc::new(actor);
            let (prompt_task, _conv) = drive_prompt(
                actor.clone(),
                "pr22-base-record",
                crate::session::PromptOrigin::User,
                "deploy the release now",
            )
            .await;
            let recorded = actor
                .last_prime_outcome
                .borrow()
                .clone()
                .expect("baseline real turn must record");
            assert_eq!(recorded.status, PrimeOutcomeStatus::Primed);
            let skill_names = recorded.primed_skill_names.clone();
            prompt_task.abort();

            for (id, origin) in [
                ("pr22-cron", crate::session::PromptOrigin::SchedulerFired),
                (
                    "pr22-subagent",
                    crate::session::PromptOrigin::SubagentAssignment,
                ),
                ("pr22-unknown", crate::session::PromptOrigin::Unknown),
            ] {
                // A fresh turn-cancel so a previous abort does not leak.
                let (prompt_task2, _conv2) = drive_prompt(actor.clone(), id, origin, "tick").await;
                let after = actor
                    .last_prime_outcome
                    .borrow()
                    .clone()
                    .expect("prior outcome must survive");
                assert_eq!(after.status, PrimeOutcomeStatus::Primed);
                assert_eq!(after.primed_skill_names, skill_names.clone());
                prompt_task2.abort();
            }
        })
        .await;
}

/// A cancelled prime leaves the previous real-turn outcome unchanged.
#[tokio::test(flavor = "current_thread")]
#[serial_test::serial(prime_registry)]
async fn cancelled_prime_leaves_previous_outcome_unchanged() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, _workspace, _registry, _persisted_updates) =
                prime_enabled_actor(true, true, false).await;
            let actor = std::sync::Arc::new(actor);
            let (prompt_task, _conv) = drive_prompt(
                actor.clone(),
                "pr22-before-cancel",
                crate::session::PromptOrigin::User,
                "deploy the release now",
            )
            .await;
            let before = actor
                .last_prime_outcome
                .borrow()
                .clone()
                .expect("baseline turn must record");
            prompt_task.abort();

            actor.turn_cancel.borrow().clone().cancel();
            let (prompt_task2, _conv2) = drive_prompt(
                actor.clone(),
                "pr22-cancelled",
                crate::session::PromptOrigin::User,
                "deploy the release now",
            )
            .await;
            assert!(
                actor.last_prime_outcome.borrow().is_some(),
                "the cancelled turn must not erase prior accounting"
            );
            assert_eq!(
                actor.last_prime_outcome.borrow().clone().unwrap().status,
                before.status,
                "cancelled prime must preserve the prior real-turn outcome"
            );
            prompt_task2.abort();
        })
        .await;
}

/// With agents enabled, final callable advisory names are recorded but no
/// `<agent_recommendations>` block enters the conversation.
#[tokio::test(flavor = "current_thread")]
#[serial_test::serial(prime_registry)]
async fn agents_enabled_records_names_without_inserting_agent_reminder() {
    use xai_grok_agent::config::AgentDefinition;
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (mut actor, _workspace, _registry, _persisted_updates) =
                prime_enabled_actor_and_agents(true, false, false, true).await;
            // Expose a callable CLI-defined agent through the same capture lane
            // the session turn path uses.
            let mut def = AgentDefinition::default_grok_build();
            def.name = "steve".into();
            actor.rebuild_spec =
                crate::session::agent_rebuild::test_rebuild_spec_enabled_subagents(vec![def]);
            let actor = std::sync::Arc::new(actor);
            let (prompt_task, conv) = drive_prompt(
                actor.clone(),
                "pr22-agents",
                crate::session::PromptOrigin::User,
                "deploy the release now",
            )
            .await;
            let outcome = actor
                .last_prime_outcome
                .borrow()
                .clone()
                .expect("agents-enabled turn must record");
            assert_eq!(outcome.status, PrimeOutcomeStatus::Primed);
            assert!(
                outcome
                    .recommended_agent_names
                    .contains(&"steve".to_string()),
                "agent names from the final callable advisory set: {:?}",
                outcome.recommended_agent_names
            );
            assert!(outcome.primed_skill_names.is_empty());
            assert!(
                reminder_items(&conv).is_empty(),
                "agents are advisory-only: no agent reminder may be inserted"
            );
            assert_eq!(conv.len(), 1, "only the real user item is pushed");
            prompt_task.abort();
        })
        .await;
}

/// A soft (degrade_on_error = true) semantic miss records only the safe
/// degradation label, never an error string or profile detail.
#[tokio::test(flavor = "current_thread")]
#[serial_test::serial(prime_registry)]
async fn degraded_semantic_records_safe_degradation_labels_only() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, _workspace, _registry, _persisted_updates) =
                prime_enabled_actor_with_degraded_snapshot(false).await;
            let actor = std::sync::Arc::new(actor);
            let (prompt_task, _conv) = drive_prompt(
                actor.clone(),
                "pr22-degraded",
                crate::session::PromptOrigin::User,
                "deploy the release now",
            )
            .await;
            let outcome = actor
                .last_prime_outcome
                .borrow()
                .clone()
                .expect("degraded turn must record");
            assert_eq!(outcome.status, PrimeOutcomeStatus::Degraded);
            assert!(
                outcome
                    .degradation
                    .iter()
                    .any(|d| d.as_str() == "profile_missing"),
                "safe label only: {:?}",
                outcome.degradation
            );
            assert!(
                outcome.injected_chars > 0,
                "soft-degrade still renders the selection"
            );
            prompt_task.abort();
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial(prime_registry)]
async fn degraded_skill_selection_still_records_callable_agent_recommendations() {
    use xai_grok_agent::config::AgentDefinition;

    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (mut actor, _workspace, _registry, _persisted_updates) =
                prime_enabled_actor_with_degraded_snapshot(true).await;
            let mut definition = AgentDefinition::default_grok_build();
            definition.name = "steve".into();
            actor.rebuild_spec =
                crate::session::agent_rebuild::test_rebuild_spec_enabled_subagents(vec![
                    definition,
                ]);
            let actor = std::sync::Arc::new(actor);
            let (prompt_task, _conv) = drive_prompt(
                actor.clone(),
                "pr22-degraded-skills-agents",
                crate::session::PromptOrigin::User,
                "deploy the release now",
            )
            .await;
            let outcome = actor
                .last_prime_outcome
                .borrow()
                .clone()
                .expect("degraded turn must record");
            assert_eq!(outcome.status, PrimeOutcomeStatus::Degraded);
            assert!(
                outcome
                    .recommended_agent_names
                    .contains(&"steve".to_string()),
                "skill degradation must not suppress callable agent advice: {:?}",
                outcome.recommended_agent_names
            );
            prompt_task.abort();
        })
        .await;
}
