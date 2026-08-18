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

use xai_grok_tools::implementations::skills::types::SkillInfo;

/// Prime-enabled actor with a real temp workspace containing a `SKILL.md` and
/// a seeded eligible-native-skill snapshot. Returns the actor and the temp dir
/// guard (kept alive for the test).
async fn prime_enabled_actor(
    enabled: bool,
    skill_enabled: bool,
) -> (SessionActor, tempfile::TempDir) {
    let tmp = tempfile::TempDir::new().unwrap();
    let cwd = tmp.path().canonicalize().unwrap();
    std::fs::create_dir_all(cwd.join("skills")).unwrap();
    std::fs::write(
        cwd.join("skills").join("SKILL.md"),
        "PRIME-SKILL-BODY deploy steps\n<script>alert(1)</script>\n",
    )
    .unwrap();
    let skill_path = cwd.join("skills").join("SKILL.md");

    let (gateway_tx, _gateway_rx) = tokio::sync::mpsc::unbounded_channel();
    let (persistence_tx, mut persistence_rx) = tokio::sync::mpsc::unbounded_channel();
    // Resolve flush barriers so the persist-ack path completes deterministically
    // (mirrors the production persistence actor).
    tokio::task::spawn_local(async move {
        while let Some(msg) = persistence_rx.recv().await {
            if let PersistenceMsg::FlushAndAck { respond_to } = msg {
                let _ = respond_to.send(());
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

    // Install a prime-enabled retrieval registry for the test home.
    let home = xai_grok_config::grok_home();
    let reg = crate::retrieval::RetrievalRegistry::disabled(home.clone());
    let mut prime = xai_grok_config_types::PrimeConfig::default();
    prime.skills.enabled = skill_enabled;
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
    crate::retrieval::install_registry_for_home(home.clone(), reg);
    (actor, tmp)
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
async fn real_user_turn_injects_hidden_skill_prime_reminder_before_user() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, _guard) = prime_enabled_actor(true, true).await;
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
            prompt_task.abort();
        })
        .await;
}

/// Cron (`/loop`) turns arrive as the typed `SchedulerFired` origin (via the
/// ACP prompt-origin meta tag): they must NOT prime and must shape as
/// `ConversationItem::scheduler_fired`, not a plain user turn.
#[tokio::test(flavor = "current_thread")]
async fn scheduler_fired_cron_never_primes_and_shapes_scheduler_fired() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, _guard) = prime_enabled_actor(true, true).await;
            let actor = std::sync::Arc::new(actor);
            // The typed origin the shell reader produces for the pager's
            // `_meta.promptOrigin: "scheduler_fired"` stamp.
            let cron_origin =
                crate::session::PromptOrigin::from_prompt_origin_meta(Some("scheduler_fired"));
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
async fn subagent_assignment_never_primes_but_later_user_turn_does() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, _guard) = prime_enabled_actor(true, true).await;
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
async fn unknown_origin_never_primes() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, _guard) = prime_enabled_actor(true, true).await;
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

/// Disabled prime omits the reminder entirely (user item still flows).
#[tokio::test(flavor = "current_thread")]
async fn disabled_prime_omits_reminder() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, _guard) = prime_enabled_actor(true, false).await;
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
async fn pre_cancelled_turn_omits_prime_but_keeps_user_item() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, _guard) = prime_enabled_actor(true, true).await;
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
async fn scheduler_fired_queued_without_queue_row_or_history() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (actor, _guard) = prime_enabled_actor(true, true).await;
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
