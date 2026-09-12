#![cfg_attr(rustfmt, rustfmt::skip)]
//! Auto-wake turn binding.
//!
//! The shell runs background-task / subagent / notification completions as real
//! model turns through the actor. They have no `PromptResponse`, so nothing
//! called `start_turn` for them: left unmodelled the session stayed `Idle` and
//! the turn streamed in with no status line, no elapsed counter, no `[stop]`,
//! a Ctrl+C that only armed quit, and a local queue that drained straight into
//! the wake turn's window.
//!
//! These tests pin the binding that fixes that: the live entry, the durable
//! `TurnCompleted` exit, the non-clobber guard for a concurrent user turn, and
//! the replay guard that keeps a wake turn which already ended from being bound.

use super::*;

/// A live wake delta binds the turn: the session leaves Idle, adopts the wake
/// pid so its deltas pass the mismatch gate, and stamps a start anchor so the
/// elapsed counter is meaningful.
#[test]
fn live_wake_delta_binds_turn() {
    let mut app = make_app_with_agent("sess-wake");
    let _ = handle(
        make_viewer_chunk_with_turn_start("sess-wake", "task-completed-bg1", 5_000),
        &mut app,
    );

    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(
        matches!(agent.session.state, AgentState::TurnRunning),
        "a live wake delta must bind real turn state, got {:?}",
        agent.session.state
    );
    assert_eq!(
        agent.session.wake_turn_prompt_id.as_deref(),
        Some("task-completed-bg1")
    );
    assert_eq!(
        agent.session.current_prompt_id.as_deref(),
        Some("task-completed-bg1"),
        "the wake pid must be current so its own deltas are not dropped"
    );
    assert!(
        agent.turn_started_at.is_some(),
        "the elapsed counter needs an anchor"
    );
    assert!(
        !agent.session.state.is_idle(),
        "a bound wake turn must hold the local queue (server-busy)"
    );
}

/// The durable terminal is the exit: it returns the session to Idle and clears
/// the wake identity, so the prompt the server promotes next adopts cleanly.
#[test]
fn wake_terminal_unbinds_and_returns_to_idle() {
    let mut app = make_app_with_agent("sess-wake");
    let _ = handle(
        make_viewer_chunk_with_turn_start("sess-wake", "task-completed-bg1", 5_000),
        &mut app,
    );

    let affected = handle_ext_notification(
        &xai_wake_turn_completed_notif("sess-wake", "task-completed-bg1", None),
        &mut app,
    );
    assert!(affected, "the wake terminal must schedule a redraw");

    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(agent.session.state.is_idle(), "the wake turn must end idle");
    assert!(agent.session.wake_turn_prompt_id.is_none());
    assert!(
        agent.session.current_prompt_id.is_none(),
        "clearing the identity is what lets the next turn adopt"
    );
    assert!(agent.turn_started_at.is_none());
}

/// A wake terminal for a turn this client never bound — a user turn held the
/// session for the whole wake window — must not clobber the user turn.
#[test]
fn wake_terminal_leaves_running_user_turn_alone() {
    let mut app = make_app_with_agent("sess-wake");
    {
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        agent.session.start_turn(&mut agent.scrollback);
        agent.session.current_prompt_id = Some("pid-user".into());
        agent.turn_started_at = Some(std::time::Instant::now());
    }

    let _ = handle_ext_notification(
        &xai_wake_turn_completed_notif("sess-wake", "task-completed-bg1", None),
        &mut app,
    );

    let agent = app.agents.get(&AgentId(0)).unwrap();
    assert!(
        matches!(agent.session.state, AgentState::TurnRunning),
        "the user turn must still be running"
    );
    assert_eq!(
        agent.session.current_prompt_id.as_deref(),
        Some("pid-user"),
        "the user turn's identity must be untouched"
    );
    assert!(agent.session.wake_turn_prompt_id.is_none());
}

/// Binding is idempotent: the many deltas of one wake turn must not re-stamp
/// the anchor and make the elapsed counter jump backwards.
#[test]
fn repeated_wake_deltas_keep_the_original_anchor() {
    let mut app = make_app_with_agent("sess-wake");
    let _ = handle(
        make_viewer_chunk_with_turn_start("sess-wake", "task-completed-bg1", 5_000),
        &mut app,
    );
    let first = app.agents[&AgentId(0)].turn_started_at;
    let _ = handle(
        make_viewer_chunk_with_turn_start("sess-wake", "task-completed-bg1", 1_000),
        &mut app,
    );
    assert_eq!(
        app.agents[&AgentId(0)].turn_started_at, first,
        "the second delta must not re-stamp the turn start"
    );
}

/// A wake turn whose terminal already arrived in this load's replay has ended,
/// so it must not be bound (binding would wait for a terminal that already
/// fired).
#[test]
fn should_bind_wake_turn_rejects_terminal_in_replay() {
    let mut agent = make_agent(Some("sess-wake"));
    assert!(agent.should_bind_wake_turn("task-completed-bg1"));
    assert!(agent.should_bind_wake_turn("notifications-drain-1"));

    agent
        .replayed_terminal_prompts
        .insert("task-completed-bg1".into());
    assert!(
        !agent.should_bind_wake_turn("task-completed-bg1"),
        "a wake turn that ended in replay must not be bound"
    );
    assert!(agent.should_bind_wake_turn("notifications-drain-1"));

    // Non-wake pids take the sibling adoption path, not this one.
    assert!(!agent.should_bind_wake_turn("pid-user"));
}

/// The wake families are exactly the auto-wake ones — a scheduler fire or a
/// goal turn keeps its own shape.
#[test]
fn should_bind_wake_turn_covers_wake_families_only() {
    let agent = make_agent(Some("sess-wake"));
    for pid in [
        "task-completed-abc",
        "subagent-completed-abc",
        "workflow-completed-abc",
        "notifications-abc",
    ] {
        assert!(agent.should_bind_wake_turn(pid), "{pid} is a wake family");
    }
    for pid in ["scheduler-fired-abc", "goal-summary-abc", "plan-resume-abc"] {
        assert!(
            !agent.should_bind_wake_turn(pid),
            "{pid} keeps its own shape"
        );
    }
}