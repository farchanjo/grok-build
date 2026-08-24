//! Tests for session-related modals (extensions, /new worktree question)
//! and session close helpers shared with the dashboard.

use super::*;

#[test]
fn open_extensions_modal_no_session_sets_flag_no_fetches() {
    use crate::views::extensions_modal::ExtensionsTab;
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    app.agents.get_mut(&id).unwrap().session.session_id = None;
    let effects = dispatch(
        Action::OpenExtensionsModal {
            tab: ExtensionsTab::Hooks,
            trigger: xai_grok_telemetry::events::ExtensionsModalTrigger::SlashCommand,
        },
        &mut app,
    );
    assert_eq!(count_extension_fetches(&effects), 0);
    assert!(app.agents[&id].pending_extensions_fetch);
    assert!(app.agents[&id].extensions_modal.is_some());
}

#[test]
fn open_extensions_modal_with_session_emits_fetches_no_flag() {
    use crate::views::extensions_modal::ExtensionsTab;
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let effects = dispatch(
        Action::OpenExtensionsModal {
            tab: ExtensionsTab::Hooks,
            trigger: xai_grok_telemetry::events::ExtensionsModalTrigger::SlashCommand,
        },
        &mut app,
    );
    assert_eq!(count_extension_fetches(&effects), 5);
    assert!(!app.agents[&id].pending_extensions_fetch);
}

#[test]
fn open_extensions_modal_with_session_resets_stale_flag() {
    use crate::views::extensions_modal::ExtensionsTab;
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    app.agents.get_mut(&id).unwrap().pending_extensions_fetch = true;
    let effects = dispatch(
        Action::OpenExtensionsModal {
            tab: ExtensionsTab::Hooks,
            trigger: xai_grok_telemetry::events::ExtensionsModalTrigger::SlashCommand,
        },
        &mut app,
    );
    assert_eq!(count_extension_fetches(&effects), 5);
    assert!(!app.agents[&id].pending_extensions_fetch);
}

#[test]
fn session_created_with_flag_but_modal_closed_clears_flag_no_fetches() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let a = app.agents.get_mut(&id).unwrap();
        a.session.session_id = None;
        a.pending_extensions_fetch = true;
        a.extensions_modal = None;
    }
    let effects = dispatch(
        Action::TaskComplete(TaskResult::SessionCreated {
            agent_id: id,
            session_id: acp::SessionId::new("s"),
            models: None,
        }),
        &mut app,
    );
    assert_eq!(count_extension_fetches(&effects), 0);
    assert!(!app.agents[&id].pending_extensions_fetch);
}

// ── /new dispatcher tests ─────────────────────────────────────────────

#[test]
fn dispatch_new_session_opens_question_modal_in_git_repo() {
    let mut app = new_session_test_app();
    app.new_session_worktree_mode = crate::app::app_view::WorktreeMode::Ask;
    let effects = dispatch(Action::NewSession, &mut app);
    assert!(effects.is_empty(), "no effects until modal answered");
    // No new agent yet (creation is deferred until modal answered).
    assert_eq!(app.agents.len(), 1);
    let qv = app.agents[&AgentId(0)]
        .question_view
        .as_ref()
        .expect("modal must be open");
    match qv.local_kind.as_ref().expect("local_kind must be set") {
        crate::views::question_view::LocalQuestionKind::NewSession => {}
        other => panic!("expected NewSession, got {other:?}"),
    }
    assert_eq!(
        qv.questions[0].options.len(),
        4,
        "modal must offer exactly 4 options (Yes/No/Always/Never)"
    );
    let labels: Vec<&str> = qv.questions[0]
        .options
        .iter()
        .map(|o| o.label.as_str())
        .collect();
    assert_eq!(
        labels,
        vec!["Yes", "No", "Always worktree", "Never worktree"]
    );
}

#[test]
fn dispatch_new_session_skips_modal_in_non_git_repo() {
    // current_branch stays None (no git repo) → no modal, straight
    // to dispatch_new_session_inner.
    let mut app = test_app_with_agent();
    let effects = dispatch(Action::NewSession, &mut app);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::CreateSession { .. })),
        "non-git path must emit CreateSession, got {effects:?}"
    );
    assert!(
        app.agents.values().all(|a| a.question_view.is_none()),
        "non-git path must not open the modal"
    );
}

// ── Session close (shared with dashboard) ─────────────────────────────

#[test]
fn close_inactive_agent_drops_it() {
    let mut app = three_agent_app();
    let effects = dispatch_sessions_confirm_close(&mut app, AgentId(2));
    assert!(
        effects
            .iter()
            .all(|e| matches!(e, Effect::UnregisterActiveSession { .. }))
    );
    assert!(!app.agents.contains_key(&AgentId(2)));
    assert_eq!(app.agents.len(), 2);
}

#[test]
fn close_agent_releases_retained_memory() {
    use crate::memory_release::test_support;
    test_support::install_counting_hook();

    let mut app = three_agent_app();

    // Dropping a real AgentView (scrollback + caches + child views) → purge.
    let before = test_support::calls();
    dispatch_sessions_confirm_close(&mut app, AgentId(2));
    assert!(!app.agents.contains_key(&AgentId(2)));
    assert_eq!(
        test_support::calls(),
        before + 1,
        "dropping the closed AgentView must purge retained pages"
    );

    // Closing an unknown agent drops nothing → no purge.
    let before = test_support::calls();
    dispatch_sessions_confirm_close(&mut app, AgentId(999));
    assert_eq!(
        test_support::calls(),
        before,
        "a no-op close must not purge"
    );
}

#[test]
fn close_clears_forked_from_on_surviving_children() {
    let mut app = three_agent_app();
    set_forked_from(&mut app, AgentId(2), AgentId(1));
    dispatch_sessions_confirm_close(&mut app, AgentId(1));
    assert!(
        app.agents[&AgentId(2)].session.forked_from.is_none(),
        "stale forked_from pointer must be cleared after parent close"
    );
}

#[test]
fn close_only_agent_is_refused_with_toast() {
    let mut app = test_app_with_agent();
    let agents_before = app.agents.len();
    dispatch_sessions_confirm_close(&mut app, AgentId(0));
    assert_eq!(
        app.agents.len(),
        agents_before,
        "the only agent must NOT be closed"
    );
}

#[test]
fn close_unknown_agent_is_silent_noop() {
    let mut app = three_agent_app();
    let agents_before = app.agents.len();
    dispatch_sessions_confirm_close(&mut app, AgentId(999));
    assert_eq!(app.agents.len(), agents_before);
}

#[test]
fn close_only_agent_short_circuits_before_reaching_welcome_fallback() {
    let mut app = test_app_with_agent();
    assert!(matches!(app.active_view, ActiveView::Agent(id) if id == AgentId(0)));
    dispatch_sessions_confirm_close(&mut app, AgentId(0));
    assert!(matches!(app.active_view, ActiveView::Agent(id) if id == AgentId(0)));
    assert!(app.agents.contains_key(&AgentId(0)));
}

#[test]
fn close_does_not_disturb_unrelated_forked_from_pointers() {
    let mut app = three_agent_app();
    set_forked_from(&mut app, AgentId(1), AgentId(0));
    set_forked_from(&mut app, AgentId(2), AgentId(0));
    dispatch_sessions_confirm_close(&mut app, AgentId(1));
    assert_eq!(
        app.agents[&AgentId(2)].session.forked_from,
        Some(AgentId(0)),
        "unrelated forked_from must NOT be cleared"
    );
}

#[test]
fn extensions_modal_in_non_project_dir_creates_session() {
    let mut app = project_picker_app();
    dispatch(Action::NewSession, &mut app);
    let id = AgentId(0);

    let effects = dispatch(
        Action::OpenExtensionsModal {
            tab: crate::views::extensions_modal::ExtensionsTab::McpServers,
            trigger: xai_grok_telemetry::events::ExtensionsModalTrigger::SlashCommand,
        },
        &mut app,
    );

    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::CreateSession { .. })),
        "session-less modal open must create the deferred session"
    );
    assert!(app.agents[&id].pending_extensions_fetch);
}

fn count_marketplace_fetches(effects: &[Effect]) -> usize {
    effects
        .iter()
        .filter(|e| matches!(e, Effect::FetchMarketplaceList { .. }))
        .count()
}

fn success_outcome() -> xai_hooks_plugins_types::ActionOutcome {
    xai_hooks_plugins_types::ActionOutcome {
        status: xai_hooks_plugins_types::OutcomeStatus::Success,
        message: "ok".into(),
        requires_reload: false,
        requires_restart: false,
    }
}

fn empty_marketplace_response() -> xai_hooks_plugins_types::MarketplaceListResponse {
    xai_hooks_plugins_types::MarketplaceListResponse { sources: vec![] }
}

#[test]
fn marketplace_fetch_coalesces_while_inflight() {
    use crate::views::extensions_modal::ExtensionsTab;
    let mut app = test_app_with_agent();
    let id = AgentId(0);

    let effects = dispatch(
        Action::OpenExtensionsModal {
            tab: ExtensionsTab::Marketplace,
            trigger: xai_grok_telemetry::events::ExtensionsModalTrigger::SlashCommand,
        },
        &mut app,
    );
    assert_eq!(count_marketplace_fetches(&effects), 1);

    // A successful action while the open-fetch is still in flight must not
    // stack a second scan; it queues one refetch instead.
    let effects = dispatch(
        Action::TaskComplete(TaskResult::PluginsActionResult {
            agent_id: id,
            result: Ok(success_outcome()),
        }),
        &mut app,
    );
    assert_eq!(count_marketplace_fetches(&effects), 0);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::FetchHooksList { .. })),
        "non-marketplace refetches still fire"
    );

    // When the in-flight fetch lands, the queued refetch fires exactly once.
    let effects = dispatch(
        Action::TaskComplete(TaskResult::MarketplaceListLoaded {
            agent_id: id,
            result: Ok(empty_marketplace_response()),
        }),
        &mut app,
    );
    assert_eq!(count_marketplace_fetches(&effects), 1);

    // And the queue drains: the refetch landing issues nothing further.
    let effects = dispatch(
        Action::TaskComplete(TaskResult::MarketplaceListLoaded {
            agent_id: id,
            result: Ok(empty_marketplace_response()),
        }),
        &mut app,
    );
    assert_eq!(count_marketplace_fetches(&effects), 0);
}

#[test]
fn open_skills_modal_does_not_fetch_marketplace() {
    use crate::views::extensions_modal::ExtensionsTab;
    let mut app = test_app_with_agent();
    let effects = dispatch(
        Action::OpenExtensionsModal {
            tab: ExtensionsTab::Skills,
            trigger: xai_grok_telemetry::events::ExtensionsModalTrigger::SlashCommand,
        },
        &mut app,
    );
    assert_eq!(count_marketplace_fetches(&effects), 0);
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::FetchSkillsList { .. }))
            .count(),
        1
    );
    assert!(
        !effects.iter().any(|e| matches!(
            e,
            Effect::FetchHooksList { .. } | Effect::FetchMcpsList { .. }
        )),
        "opening /skills must not fan out other extension fetches, got {effects:?}"
    );
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::FetchSkillsSearch { .. })),
        "opening /skills in Local mode must not search, got {effects:?}"
    );
}

#[test]
fn search_skills_smart_dispatches_prime_search_with_query() {
    use crate::views::extensions_modal::{ExtensionsModalState, ExtensionsTab, SkillSearchMode};
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let mut modal = ExtensionsModalState::new(ExtensionsTab::Skills);
        modal.skills_search_mode = SkillSearchMode::Smart;
        modal.picker_state.set_query("commit");
        app.agents.get_mut(&id).unwrap().extensions_modal = Some(modal);
    }
    let effects = dispatch(Action::SearchSkillsSmart, &mut app);
    match effects.as_slice() {
        [
            Effect::FetchSkillsSearch {
                agent_id,
                query,
                r#gen,
                ..
            },
        ] => {
            assert_eq!(*agent_id, id);
            assert_eq!(query, "commit");
            assert_eq!(*r#gen, 1);
        }
        other => panic!("expected FetchSkillsSearch, got {other:?}"),
    }
    assert_eq!(app.agents[&id].skills_smart_search_gen, 1);
}

#[test]
fn search_skills_smart_clears_stale_rank_before_refetch() {
    use crate::views::extensions_modal::{ExtensionsModalState, ExtensionsTab, SkillSearchMode};
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let mut modal = ExtensionsModalState::new(ExtensionsTab::Skills);
        modal.skills_search_mode = SkillSearchMode::Smart;
        modal.picker_state.set_query("z");
        modal.skills_smart_rank = Some(vec!["commit".into(), "alpha".into()]);
        app.agents.get_mut(&id).unwrap().extensions_modal = Some(modal);
    }
    let effects = dispatch(Action::SearchSkillsSmart, &mut app);
    assert!(
        app.agents[&id]
            .extensions_modal
            .as_ref()
            .unwrap()
            .skills_smart_rank
            .is_none(),
        "SearchSkillsSmart must drop the previous query's rank before refetch"
    );
    match effects.as_slice() {
        [
            Effect::FetchSkillsSearch {
                agent_id,
                query,
                r#gen,
                ..
            },
        ] => {
            assert_eq!(*agent_id, id);
            assert_eq!(query, "z");
            assert_eq!(*r#gen, 1);
        }
        other => panic!("expected FetchSkillsSearch, got {other:?}"),
    }
    assert_eq!(app.agents[&id].skills_smart_search_gen, 1);
}

#[test]
fn search_skills_smart_clears_rank_when_local_or_empty() {
    use crate::views::extensions_modal::{ExtensionsModalState, ExtensionsTab, SkillSearchMode};
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let mut modal = ExtensionsModalState::new(ExtensionsTab::Skills);
        modal.skills_search_mode = SkillSearchMode::Local;
        modal.skills_smart_rank = Some(vec!["stale".into()]);
        modal.picker_state.set_query("commit");
        app.agents.get_mut(&id).unwrap().extensions_modal = Some(modal);
    }
    let effects = dispatch(Action::SearchSkillsSmart, &mut app);
    assert!(
        effects.is_empty(),
        "Local mode must not search, got {effects:?}"
    );
    assert!(
        app.agents[&id]
            .extensions_modal
            .as_ref()
            .unwrap()
            .skills_smart_rank
            .is_none()
    );
    assert_eq!(
        app.agents[&id].skills_smart_search_gen, 0,
        "Local mode must not bump the search generation when no fetch is issued"
    );
}

#[test]
fn search_skills_smart_twice_bumps_generation() {
    use crate::views::extensions_modal::{ExtensionsModalState, ExtensionsTab, SkillSearchMode};
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let mut modal = ExtensionsModalState::new(ExtensionsTab::Skills);
        modal.skills_search_mode = SkillSearchMode::Smart;
        modal.picker_state.set_query("commit");
        app.agents.get_mut(&id).unwrap().extensions_modal = Some(modal);
    }
    let first = dispatch(Action::SearchSkillsSmart, &mut app);
    let second = dispatch(Action::SearchSkillsSmart, &mut app);
    match (first.as_slice(), second.as_slice()) {
        (
            [
                Effect::FetchSkillsSearch {
                    r#gen: first_gen, ..
                },
            ],
            [
                Effect::FetchSkillsSearch {
                    r#gen: second_gen,
                    query,
                    ..
                },
            ],
        ) => {
            assert_eq!(query, "commit");
            assert_eq!(*first_gen, 1);
            assert_eq!(*second_gen, 2);
        }
        other => panic!("expected two FetchSkillsSearch effects, got {other:?}"),
    }
    assert_eq!(app.agents[&id].skills_smart_search_gen, 2);
}

#[test]
fn reopen_skills_modal_restores_in_flight_regress_badge_without_starting_run() {
    use crate::views::extensions_modal::ExtensionsTab;
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    app.agents.get_mut(&id).unwrap().skill_regress_in_flight = Some("deploy-prod".into());
    let effects = dispatch(
        Action::OpenExtensionsModal {
            tab: ExtensionsTab::Skills,
            trigger: xai_grok_telemetry::events::ExtensionsModalTrigger::SlashCommand,
        },
        &mut app,
    );
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::RunSkillRegress { .. })),
        "reopening /skills must not start regression, got {effects:?}"
    );
    assert_eq!(
        app.agents[&id]
            .extensions_modal
            .as_ref()
            .and_then(|modal| modal.pending_action.as_deref()),
        Some("regressing...")
    );
}

#[test]
fn open_create_skill_wizard_does_not_fetch_marketplace() {
    let mut app = test_app_with_agent();
    let effects = dispatch(
        Action::OpenCreateSkillWizard {
            trigger: xai_grok_telemetry::events::ExtensionsModalTrigger::SlashCommand,
        },
        &mut app,
    );
    assert_eq!(count_marketplace_fetches(&effects), 0);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::FetchSkillsList { .. }))
    );
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::FetchMarketplaceList { .. }))
    );
}

#[test]
fn marketplace_fetch_fires_immediately_when_idle() {
    use crate::views::extensions_modal::ExtensionsTab;
    let mut app = test_app_with_agent();
    let id = AgentId(0);

    dispatch(
        Action::OpenExtensionsModal {
            tab: ExtensionsTab::Marketplace,
            trigger: xai_grok_telemetry::events::ExtensionsModalTrigger::SlashCommand,
        },
        &mut app,
    );
    dispatch(
        Action::TaskComplete(TaskResult::MarketplaceListLoaded {
            agent_id: id,
            result: Ok(empty_marketplace_response()),
        }),
        &mut app,
    );

    // Nothing in flight: an action-triggered refetch goes out immediately.
    let effects = dispatch(
        Action::TaskComplete(TaskResult::PluginsActionResult {
            agent_id: id,
            result: Ok(success_outcome()),
        }),
        &mut app,
    );
    assert_eq!(count_marketplace_fetches(&effects), 1);
}
