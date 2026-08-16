//! Tests for login, logout, account switching, and auth-code dispatchers.

use super::*;

#[test]
fn cta_mcps_loaded_needs_auth_opens_modal_and_seeds() {
    use crate::app::agent_view::CtaPhase;
    use crate::views::extensions_modal::{ExtensionsTab, TabDataState};
    use crate::views::mcps_modal::{McpSectionId, McpServerDisplayStatus, section_key};
    let mut app = test_app_with_agent();
    app.team_id = Some("team-uuid".into());
    let id = AgentId(0);
    app.agents.get_mut(&id).unwrap().plugin_cta.phase = CtaPhase::AwaitingMcps {
        name: "figma".into(),
    };
    let servers = vec![
        cta_mcp_server("grok_com_managed", None, McpServerDisplayStatus::Ready),
        cta_mcp_server("local-srv", None, McpServerDisplayStatus::Ready),
        cta_mcp_server("other-srv", Some("slack"), McpServerDisplayStatus::Ready),
        cta_mcp_server(
            "figma-srv",
            Some("figma"),
            McpServerDisplayStatus::NeedsAuth,
        ),
    ];
    let effects = dispatch(
        Action::TaskComplete(TaskResult::PluginCtaMcpsLoaded {
            agent_id: id,
            plugin_name: "figma".into(),
            result: Ok(servers),
        }),
        &mut app,
    );
    // Handoff complete: CTA settles to Hidden.
    assert_eq!(app.agents[&id].plugin_cta.phase, CtaPhase::Hidden);
    // Modal opened to the MCP Servers tab.
    let modal = app.agents[&id]
        .extensions_modal
        .as_ref()
        .expect("extensions modal should be open");
    assert_eq!(modal.active_tab, ExtensionsTab::McpServers);
    // Session team id seeded so the Managed subtitle deep link matches Ctrl+O.
    assert_eq!(modal.session_team_id.as_deref(), Some("team-uuid"));
    // MCP tab seeded directly from the read we already have (no flash).
    match &modal.mcps_data {
        TabDataState::Loaded(servers) => assert_eq!(servers.len(), 4),
        other => panic!("expected mcps_data Loaded, got {other:?}"),
    }
    // Managed + Local + other plugins collapsed; only target expanded.
    let collapsed = &modal.mcps_collapsed_sections;
    assert!(collapsed.contains(&section_key(&McpSectionId::Managed)));
    assert!(collapsed.contains(&section_key(&McpSectionId::Local)));
    assert!(collapsed.contains(&section_key(&McpSectionId::Plugin("slack".into()))));
    assert!(!collapsed.contains(&section_key(&McpSectionId::Plugin("figma".into()))));
    assert!(modal.mcps_section_collapse_initialized);
    // Emits the SAME full tab fetch-set as a manual open so no tab is stuck
    // Loading, plus the candidate refresh.
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::FetchHooksList { .. }))
            .count(),
        1
    );
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::FetchPluginsList { .. }))
            .count(),
        1
    );
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::FetchMarketplaceList { .. }))
            .count(),
        1
    );
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::FetchMcpsList { .. }))
            .count(),
        1
    );
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::FetchSkillsList { .. }))
            .count(),
        1
    );
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::FetchPluginCtaCatalog { .. }))
    );
}

#[test]
fn cta_mcps_loaded_no_needs_auth_terminal_sets_installed() {
    use crate::app::agent_view::CtaPhase;
    use crate::views::mcps_modal::McpServerDisplayStatus;
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let cta = &mut app.agents.get_mut(&id).unwrap().plugin_cta;
        cta.phase = CtaPhase::AwaitingMcps {
            name: "figma".into(),
        };
        cta.expects_mcp = true;
    }
    // Plugin server present and Ready (terminal, no auth) -> settle now.
    let servers = vec![cta_mcp_server(
        "figma-srv",
        Some("figma"),
        McpServerDisplayStatus::Ready,
    )];
    let effects = dispatch(
        Action::TaskComplete(TaskResult::PluginCtaMcpsLoaded {
            agent_id: id,
            plugin_name: "figma".into(),
            result: Ok(servers),
        }),
        &mut app,
    );
    assert_eq!(
        app.agents[&id].plugin_cta.phase,
        CtaPhase::Installed {
            name: "figma".into()
        }
    );
    assert!(app.agents[&id].extensions_modal.is_none());
    // No modal repopulation; settle emits the auto-dismiss timer + candidate
    // refresh, and never re-probes.
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::FetchMcpsList { .. }))
    );
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::RetryPluginCtaMcps { .. }))
    );
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::DismissCtaInstalled { .. }))
    );
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::FetchPluginCtaCatalog { .. }))
    );
}

#[test]
fn cta_mcps_loaded_later_needs_auth_opens_handoff() {
    use crate::app::agent_view::CtaPhase;
    use crate::views::mcps_modal::McpServerDisplayStatus;
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let cta = &mut app.agents.get_mut(&id).unwrap().plugin_cta;
        cta.phase = CtaPhase::AwaitingMcps {
            name: "figma".into(),
        };
        cta.expects_mcp = true;
        // Several polls already elapsed before the server reached NeedsAuth.
        cta.mcp_attempt = 5;
    }
    let effects = dispatch(
        Action::TaskComplete(TaskResult::PluginCtaMcpsLoaded {
            agent_id: id,
            plugin_name: "figma".into(),
            result: Ok(vec![cta_mcp_server(
                "figma-srv",
                Some("figma"),
                McpServerDisplayStatus::NeedsAuth,
            )]),
        }),
        &mut app,
    );
    // NeedsAuth is terminal: hand off immediately even mid-poll.
    assert_eq!(app.agents[&id].plugin_cta.phase, CtaPhase::Hidden);
    assert!(app.agents[&id].extensions_modal.is_some());
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::RetryPluginCtaMcps { .. }))
    );
}

// ── agent-bound kinds (bash) ─────────

/// A bash command typed while a turn is RUNNING takes the
/// server-authoritative immediate path (Effect + optimistic echo, no local
/// queue entry).
#[test]
fn bash_while_running_is_server_authoritative() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    app.agents.get_mut(&id).unwrap().session.state = AgentState::TurnRunning;

    let effects = dispatch(Action::SendBashCommand("ls -la".into()), &mut app);
    let pid = match &effects[0] {
        Effect::SendBashCommand {
            command, prompt_id, ..
        } => {
            assert_eq!(command, "ls -la");
            prompt_id.clone()
        }
        other => panic!("expected immediate SendBashCommand, got {other:?}"),
    };
    // Not in the local queue.
    assert_eq!(app.agents[&id].session.queue_len(), 0);
    // Optimistic echo present with kind="bash".
    let q = app
        .shared_prompt_queue("test-session")
        .expect("echo present");
    assert_eq!(q.len(), 1);
    assert_eq!(q[0].id, pid);
    assert_eq!(q[0].kind, "bash");
    assert_eq!(q[0].text, "ls -la");
}

#[test]
fn auth_complete_triggers_bundle_status_fetch() {
    let mut app = test_app();
    app.auth_state = AuthState::Authenticating {
        request_seq: 1,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };

    let effects = dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: 1,
            meta: None,
            repair: None,
            credential_write_receipt: None,
        }),
        &mut app,
    );

    assert!(matches!(app.auth_state, AuthState::Done));
    // Pager only refreshes the on-disk catalog snapshot; the actual
    // bundle download now runs inside the shell post-auth.
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::FetchBundleStatus))
    );
}

#[test]
fn auth_complete_with_deferred_load_also_fetches_status() {
    let mut app = test_app();
    app.auth_state = AuthState::Authenticating {
        request_seq: 1,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };
    app.deferred_startup.session =
        Some(crate::app::session_startup::DeferredSessionStartup::Load {
            session_id: "test-session".into(),
            session_cwd: None,
            chat_kind: false,
        });

    let effects = dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: 1,
            meta: None,
            repair: None,
            credential_write_receipt: None,
        }),
        &mut app,
    );

    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::FetchBundleStatus))
    );
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::LoadSession { .. }))
    );
    assert!(app.deferred_startup.session.is_none());
}

/// `/login` from the welcome screen (startup / logged-out) must NOT
/// stash a return view — the normal login-then-load flow is preserved.
#[test]
fn login_from_welcome_does_not_stash_return_view() {
    let mut app = test_app();
    assert_eq!(app.active_view, ActiveView::Welcome);

    dispatch(Action::Login, &mut app);

    assert_eq!(app.active_view, ActiveView::Welcome);
    assert_eq!(app.auth_return_view, None);
}

/// Compact-auth recovery: hold prompt across auto-compact 401, stash on
/// PromptResponse, resubmit on mid-session AuthComplete.
#[test]
fn e2e_compact_auth_failure_holds_prompt_and_resubmits_after_login() {
    use crate::app::acp_handler::apply_session_event_for_test;
    use crate::app::agent::{AgentState, InFlightPrompt};
    use crate::scrollback::EntryId;
    use crate::scrollback::block::RenderBlock;
    use xai_grok_shell::extensions::notification::{RetryState, SessionUpdate as XaiSessionUpdate};

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::TurnRunning;
        agent.turn_started_at = Some(std::time::Instant::now());
        agent.session.session_id = Some(acp::SessionId::new("sess-compact-auth-e2e"));
        agent.session.current_prompt_id = Some("prompt-1".into());
        agent.session.in_flight_prompt = Some(InFlightPrompt {
            text: "please continue after login".into(),
            images: Vec::new(),
            scrollback_entry: EntryId::new(1),
            combined_scrollback_entries: Vec::new(),
            chip_elements: Vec::new(),
        });

        apply_session_event_for_test(
            &XaiSessionUpdate::AutoCompactStarted {
                tokens_used: 180_000,
                context_window: 200_000,
                percentage: 90,
                reason: "threshold".into(),
            },
            &mut agent.session,
            &mut agent.scrollback,
        );
        assert!(
            agent.session.in_flight_prompt.is_none(),
            "cancel rewind must still be blocked mid-compact"
        );
        assert_eq!(
            agent
                .session
                .compact_held_prompt
                .as_ref()
                .map(|p| p.text.as_str()),
            Some("please continue after login"),
            "must hold the prompt text for reauth auto-resubmit"
        );

        apply_session_event_for_test(
            &XaiSessionUpdate::AutoCompactFailed {
                error: "authentication problem — reconnect xAI in /providers and retry.".into(),
            },
            &mut agent.session,
            &mut agent.scrollback,
        );
        assert!(agent.session.compact_held_prompt.is_some());

        apply_session_event_for_test(
            &XaiSessionUpdate::RetryState(RetryState::Failed {
                error_type: "provider_credential".into(),
                message: "xAI rejected its credentials.".into(),
                provider: Some(
                    xai_grok_shell::extensions::notification::ProviderCredentialFailure {
                        provider_id: "xai".into(),
                        provider_name: "xAI".into(),
                        credential_generation: 2,
                        ..Default::default()
                    },
                ),
            }),
            &mut agent.session,
            &mut agent.scrollback,
        );
        let has_reauth = (0..agent.scrollback.len()).any(|i| {
            matches!(
                agent.scrollback.entry(i).map(|e| &e.block),
                Some(RenderBlock::SessionEvent(ev))
                    if matches!(
                        ev.event,
                        SessionEvent::ProviderCredentialRequired { .. }
                            | SessionEvent::ReAuthRequired
                    )
            )
        });
        assert!(has_reauth, "RetryState auth must show credential repair");
    }

    dispatch(
        Action::TaskComplete(TaskResult::PromptResponse {
            agent_id: id,
            result: Err("Unauthorized (401)".to_string()),
            http_status: Some(401),
            prompt_id: Some("prompt-1".into()),
        }),
        &mut app,
    );
    assert_eq!(
        app.agents[&id]
            .reauth_stashed_prompt
            .as_ref()
            .map(|p| p.prompt.text.as_str()),
        Some("please continue after login"),
        "PromptResponse must stash the compact-held prompt for AuthComplete"
    );

    let login_effects = dispatch(Action::Login, &mut app);
    let seq = authenticating_seq(&app);
    let repair = login_effects.iter().find_map(|e| match e {
        Effect::Authenticate { repair, .. } => repair.clone(),
        _ => None,
    });
    assert!(
        repair.is_some(),
        "Login against xAI stash must mint a repair token: {login_effects:?}"
    );
    let effects = dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: seq,
            meta: None,
            repair,
                    credential_write_receipt: None,
        }),
        &mut app,
    );
    assert!(
        app.agents[&id].reauth_stashed_prompt.is_some(),
        "xAI AuthComplete without write receipt must leave stash"
    );
    assert!(
        !effects.iter().any(|e| matches!(
            e,
            Effect::SendPrompt { .. } | Effect::SendPromptBlocks { .. }
        )),
        "xAI session AuthComplete must not auto-resubmit without receipt: {effects:?}"
    );
}

/// Without compact_held, clearing in_flight on compact start leaves reauth empty.
#[test]
fn pre_fix_compact_start_without_hold_cannot_stash_for_reauth() {
    use crate::app::agent::AgentState;
    use crate::scrollback::block::RenderBlock;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::TurnRunning;
        agent.turn_started_at = Some(std::time::Instant::now());
        agent.session.session_id = Some(acp::SessionId::new("sess-pre-fix"));
        agent.session.current_prompt_id = Some("p1".into());
        agent.session.in_flight_prompt = None;
        agent.session.compact_held_prompt = None;
        agent
            .scrollback
            .push_block(RenderBlock::session_event(SessionEvent::ReAuthRequired));
    }
    dispatch(
        Action::TaskComplete(TaskResult::PromptResponse {
            agent_id: id,
            result: Err("Unauthorized (401)".to_string()),
            http_status: Some(401),
            prompt_id: Some("p1".into()),
        }),
        &mut app,
    );
    assert!(
        app.agents[&id].reauth_stashed_prompt.is_none(),
        "without compact_held / in_flight, reauth cannot stash — the pre-fix bug"
    );
}

/// A second auth-failed turn with no rewindable prompt
/// (`in_flight_prompt == None`) must not clobber the stash from an
/// earlier 401.
#[test]
fn second_auth_failure_does_not_clobber_reauth_stash() {
    use crate::scrollback::block::RenderBlock;
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.reauth_stashed_prompt = Some(crate::app::agent::ProviderScopedStashedPrompt {
            provider_id: "xai".into(),
            credential_generation: 0,
            incarnation: None,
            registry_generation: 0,
            binding_generation: 0,
            host_fallback: false,
            binding_complete: true,
            credential_route: "api_key".into(),
            route_authority: "authoritative".into(),
            correlation_token: String::new(),
            prompt: crate::app::agent::InFlightPrompt {
                text: "first prompt".into(),
                images: Vec::new(),
                scrollback_entry: crate::scrollback::EntryId::new(0),
                combined_scrollback_entries: Vec::new(),
                chip_elements: Vec::new(),
            },
        });
        agent
            .scrollback
            .push_block(RenderBlock::session_event(SessionEvent::ReAuthRequired));
        agent.session.state = AgentState::TurnRunning;
        agent.turn_started_at = Some(std::time::Instant::now());
        agent.session.in_flight_prompt = None;
    }

    dispatch(
        Action::TaskComplete(TaskResult::PromptResponse {
            agent_id: id,
            result: Err("Unauthorized (401)".to_string()),
            http_status: Some(401),
            prompt_id: None,
        }),
        &mut app,
    );

    assert_eq!(
        app.agents[&id]
            .reauth_stashed_prompt
            .as_ref()
            .map(|stashed| stashed.prompt.text.as_str()),
        Some("first prompt"),
        "a None in_flight_prompt must not wipe an earlier stash"
    );
}

/// Cancelling a mid-session re-auth drops the stashed prompt so it is
/// not silently resubmitted on a later, unrelated login.
#[test]
fn cancel_login_drops_reauth_stashed_prompt() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    app.agents.get_mut(&id).unwrap().reauth_stashed_prompt =
        Some(crate::app::agent::ProviderScopedStashedPrompt {
            provider_id: "xai".into(),
            credential_generation: 1,
            incarnation: None,
            registry_generation: 0,
            binding_generation: 0,
            host_fallback: false,
            binding_complete: true,
            credential_route: "api_key".into(),
            route_authority: "authoritative".into(),
            correlation_token: String::new(),
            prompt: crate::app::agent::InFlightPrompt {
                text: "stale".into(),
                images: Vec::new(),
                scrollback_entry: crate::scrollback::EntryId::new(0),
                combined_scrollback_entries: Vec::new(),
                chip_elements: Vec::new(),
            },
        });

    dispatch(Action::Login, &mut app);
    dispatch(Action::CancelLogin, &mut app);

    assert!(
        app.agents[&id].reauth_stashed_prompt.is_none(),
        "cancelling re-auth must drop the stashed prompt"
    );
}

/// Cancelling a mid-session re-auth strips the stale `ReAuthRequired`
/// prompt from scrollback so a later `PromptResponse` cannot re-detect
/// it and re-stash the prompt for silent resubmission.
#[test]
fn cancel_login_strips_reauth_prompt_from_scrollback() {
    use crate::scrollback::block::RenderBlock;
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.reauth_stashed_prompt = Some(crate::app::agent::ProviderScopedStashedPrompt {
            provider_id: "xai".into(),
            credential_generation: 1,
            incarnation: None,
            registry_generation: 0,
            binding_generation: 0,
            host_fallback: false,
            binding_complete: true,
            credential_route: "api_key".into(),
            route_authority: "authoritative".into(),
            correlation_token: String::new(),
            prompt: crate::app::agent::InFlightPrompt {
                text: "stale".into(),
                images: Vec::new(),
                scrollback_entry: crate::scrollback::EntryId::new(0),
                combined_scrollback_entries: Vec::new(),
                chip_elements: Vec::new(),
            },
        });
        agent
            .scrollback
            .push_block(RenderBlock::session_event(SessionEvent::ReAuthRequired));
    }

    dispatch(Action::Login, &mut app);
    dispatch(Action::CancelLogin, &mut app);

    let sb = &app.agents[&id].scrollback;
    let has_reauth = (0..sb.len()).any(|i| {
        matches!(
            sb.entry(i).map(|e| &e.block),
            Some(RenderBlock::SessionEvent(ev)) if matches!(ev.event, SessionEvent::ReAuthRequired)
        )
    });
    assert!(
        !has_reauth,
        "cancelling re-auth must strip the stale re-auth prompt from scrollback"
    );
}

/// Empty `auth_methods` (preferred_method pin unavailable) must not invent
/// `grok.com` or start an OIDC flow the agent did not advertise.
#[test]
fn login_with_empty_auth_methods_fails_closed() {
    let mut app = test_app_with_agent();
    app.auth_methods.clear();
    app.login_method_id = None;

    let effects = dispatch(Action::Login, &mut app);

    assert!(
        effects.is_empty(),
        "must not start Authenticate without an advertised method"
    );
    assert_eq!(
        app.active_view,
        ActiveView::Agent(AgentId(0)),
        "must stay on the session view"
    );
    assert!(
        matches!(
            &app.auth_state,
            AuthState::Pending { error: Some(msg) }
                if msg.contains("preferred_method=api_key")
        ),
        "must surface pin-unavailable error, got {:?}",
        app.auth_state
    );
    assert!(app.login_method_id.is_none());
}

/// Puts the app in `Authenticating` with a live task's abort handle installed
/// (as the event loop would), returning the task's JoinHandle and the seq.
/// Callers assert the task actually gets aborted (`unwrap_err().is_cancelled()`),
/// not merely that the handle slot was cleared.
fn install_live_auth_task(
    app: &mut AppView,
    rt: &tokio::runtime::Runtime,
) -> (tokio::task::JoinHandle<()>, u64) {
    dispatch(Action::Login, app);
    let task = rt.spawn(std::future::pending::<()>());
    match &mut app.auth_state {
        AuthState::Authenticating {
            handle,
            request_seq,
            ..
        } => {
            *handle = Some(task.abort_handle());
            (task, *request_seq)
        }
        other => panic!("expected Authenticating after Login, got {other:?}"),
    }
}

fn test_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime")
}

/// A second `/login` while already authenticating must abort the prior auth
/// task and bump the seq (single-flight: no stacked device-code mints).
#[test]
fn login_while_authenticating_aborts_prior_task() {
    let rt = test_runtime();
    let mut app = test_app_with_agent();
    let (prior_task, first_seq) = install_live_auth_task(&mut app, &rt);

    let effects = dispatch(Action::Login, &mut app);

    rt.block_on(async {
        assert!(
            prior_task.await.unwrap_err().is_cancelled(),
            "prior auth task must be aborted"
        );
    });
    match &app.auth_state {
        AuthState::Authenticating { request_seq, .. } => {
            assert!(
                *request_seq > first_seq,
                "re-login must bump request_seq for single-flight"
            );
        }
        other => panic!("expected Authenticating after re-Login, got {other:?}"),
    }
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::Authenticate { .. })),
        "re-login must emit a new Authenticate"
    );
}

/// A stale `AuthComplete` (from an attempt whose abort lost the race because
/// the task had already finished) must not complete the new attempt: the
/// request-seq guard is the only protection here.
#[test]
fn stale_auth_complete_after_relogin_is_ignored() {
    let mut app = test_app_with_agent();
    dispatch(Action::Login, &mut app);
    let first_seq = match &app.auth_state {
        AuthState::Authenticating { request_seq, .. } => *request_seq,
        other => panic!("expected Authenticating after Login, got {other:?}"),
    };
    dispatch(Action::Login, &mut app); // re-login bumps to seq2

    dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: first_seq,
            meta: None,
            repair: None,
            credential_write_receipt: None,
        }),
        &mut app,
    );

    match &app.auth_state {
        AuthState::Authenticating { request_seq, .. } => {
            assert!(
                *request_seq > first_seq,
                "stale AuthComplete must leave the new attempt authenticating"
            );
        }
        other => panic!("stale AuthComplete must be ignored, got {other:?}"),
    }
}

/// Switch-account while authenticating goes through the same single-flight
/// abort as `/login` (sibling entry point).
#[test]
fn switch_account_while_authenticating_aborts_prior_task() {
    let rt = test_runtime();
    let mut app = test_app_with_agent();
    let (prior_task, first_seq) = install_live_auth_task(&mut app, &rt);

    dispatch(Action::SwitchAccount, &mut app);

    rt.block_on(async {
        assert!(
            prior_task.await.unwrap_err().is_cancelled(),
            "prior auth task must be aborted on switch-account"
        );
    });
    match &app.auth_state {
        AuthState::Authenticating { request_seq, .. } => {
            assert!(*request_seq > first_seq, "switch must bump request_seq");
        }
        other => panic!("expected Authenticating after SwitchAccount, got {other:?}"),
    }
}

/// Cancelling a mid-session login aborts the in-flight auth task (not just
/// restores the view) so a retry cannot race a still-polling prior mint.
#[test]
fn cancel_login_aborts_prior_task() {
    let rt = test_runtime();
    let mut app = test_app_with_agent();
    // Login from a session view stashes `auth_return_view`, making CancelLogin live.
    let (prior_task, _) = install_live_auth_task(&mut app, &rt);

    dispatch(Action::CancelLogin, &mut app);

    rt.block_on(async {
        assert!(
            prior_task.await.unwrap_err().is_cancelled(),
            "cancel must abort the in-flight auth task"
        );
    });
}

/// Cancelling a mid-session login returns to the session rather than
/// quitting the app, and clears the stashed view + auth state.
#[test]
fn cancel_login_restores_view() {
    let mut app = test_app_with_agent();
    dispatch(Action::Login, &mut app);
    assert_eq!(app.active_view, ActiveView::Welcome);
    let prior_seq = match &app.auth_state {
        AuthState::Authenticating { request_seq, .. } => *request_seq,
        other => panic!("expected Authenticating after Login, got {other:?}"),
    };

    let effects = dispatch(Action::CancelLogin, &mut app);

    assert!(
        matches!(
            effects.as_slice(),
            [Effect::CancelAuth { request_seq }] if *request_seq == prior_seq
        ),
        "cancel must tell the shell to stop the in-flight auth poll for this attempt"
    );
    assert_eq!(app.active_view, ActiveView::Agent(AgentId(0)));
    assert_eq!(app.auth_return_view, None);
    assert!(matches!(app.auth_state, AuthState::Done));
}

/// `CancelLogin` outside a mid-session login is a no-op (must not move
/// off the welcome screen or panic).
#[test]
fn cancel_login_noop_without_stashed_view() {
    let mut app = test_app();
    let effects = dispatch(Action::CancelLogin, &mut app);
    assert!(effects.is_empty());
    assert_eq!(app.active_view, ActiveView::Welcome);
    assert_eq!(app.auth_return_view, None);
}

/// Token-bound repair race safety for ProviderOperationComplete.
#[test]
fn provider_operation_complete_token_race_safety() {
    use crate::app::agent::{CredentialRepairScope, CredentialRepairToken};
    use crate::views::providers_modal::{ProviderKind, ProviderStatus};

    fn scope(token: u64, provider_id: &str, generation: u64) -> CredentialRepairScope {
        CredentialRepairScope {
            token: CredentialRepairToken(token),
            provider_id: provider_id.into(),
            credential_generation: generation,
            incarnation: None,
            registry_generation: 0,
            failed_binding_generation: 0,
            credential_route: "api_key".into(),
            correlation_token: String::new(),
        }
    }
    fn connected(detail: &str) -> ProviderStatus {
        ProviderStatus::Connected {
            detail: Some(detail.into()),
        }
    }
    fn stash(
        agent: &mut crate::app::agent_view::AgentView,
        provider_id: &str,
        generation: u64,
        text: &str,
    ) {
        agent.reauth_stashed_prompt = Some(crate::app::agent::ProviderScopedStashedPrompt {
            provider_id: provider_id.into(),
            credential_generation: generation,
            incarnation: None,
            registry_generation: 0,
            binding_generation: 0,
            host_fallback: false,
            binding_complete: true,
            credential_route: "api_key".into(),
            route_authority: "authoritative".into(),
            correlation_token: String::new(),
            prompt: crate::app::agent::InFlightPrompt {
                text: text.into(),
                images: Vec::new(),
                scrollback_entry: crate::scrollback::EntryId::new(0),
                combined_scrollback_entries: Vec::new(),
                chip_elements: Vec::new(),
            },
        });
    }

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.session_id = Some(acp::SessionId::new("sess-token-race"));
        agent.session.state = crate::app::agent::AgentState::Idle;
        stash(agent, "openrouter", 1, "prompt-gen1");
        // Op A started for gen 1.
        agent.in_flight_repair = Some(scope(10, "openrouter", 1));
    }

    // Newer failure gen 2 replaces stash; op B starts with token 20.
    {
        let agent = app.agents.get_mut(&id).unwrap();
        stash(agent, "openrouter", 2, "prompt-gen2");
        agent.in_flight_repair = Some(scope(20, "openrouter", 2));
    }

    // Delayed completion A (token 10, gen 1) must NOT resume gen-2 stash.
    let effects = dispatch(
        Action::TaskComplete(TaskResult::ProviderOperationComplete {
            agent_id: id,
            provider: ProviderKind::OpenRouter,
            status: connected("a"),
            claude_cli_status: None,
            repair: Some(scope(10, "openrouter", 1)),
            credential_write_receipt: None,
        }),
        &mut app,
    );
    assert_eq!(
        app.agents[&id]
            .reauth_stashed_prompt
            .as_ref()
            .map(|s| s.prompt.text.as_str()),
        Some("prompt-gen2"),
        "delayed prior completion must not release newer stash"
    );
    assert!(
        !effects.iter().any(|e| matches!(
            e,
            Effect::SendPrompt { .. } | Effect::SendPromptBlocks { .. }
        )),
        "delayed A must not resubmit: {effects:?}"
    );

    // Sibling provider completion with its own token cannot resume OpenRouter.
    let sibling = scope(99, "openai", 2);
    app.agents.get_mut(&id).unwrap().in_flight_repair = Some(scope(20, "openrouter", 2));
    let effects = dispatch(
        Action::TaskComplete(TaskResult::ProviderOperationComplete {
            agent_id: id,
            provider: ProviderKind::OpenAi,
            status: connected("openai"),
            claude_cli_status: None,
            repair: Some(sibling),
            credential_write_receipt: None,
        }),
        &mut app,
    );
    assert!(app.agents[&id].reauth_stashed_prompt.is_some());
    assert!(!effects.iter().any(|e| matches!(
        e,
        Effect::SendPrompt { .. } | Effect::SendPromptBlocks { .. }
    )));

    // Test/Refresh without repair token never resumes.
    let effects = dispatch(
        Action::TaskComplete(TaskResult::ProviderOperationComplete {
            agent_id: id,
            provider: ProviderKind::OpenRouter,
            status: connected("refresh"),
            claude_cli_status: None,
            repair: None,
            credential_write_receipt: None,
        }),
        &mut app,
    );
    assert!(app.agents[&id].reauth_stashed_prompt.is_some());
    assert!(
        !effects.iter().any(|e| matches!(
            e,
            Effect::SendPrompt { .. } | Effect::SendPromptBlocks { .. }
        )),
        "unbound Test/Refresh must not resubmit"
    );

    // Exact completion B without live source fails closed.
    app.agents.get_mut(&id).unwrap().in_flight_repair = Some(scope(20, "openrouter", 2));
    let b = scope(20, "openrouter", 2);
    let effects = dispatch(
        Action::TaskComplete(TaskResult::ProviderOperationComplete {
            agent_id: id,
            provider: ProviderKind::OpenRouter,
            status: connected("b"),
            claude_cli_status: None,
            repair: Some(b.clone()),
            credential_write_receipt: Some(b.write_receipt(1)),
        }),
        &mut app,
    );
    assert!(
        app.agents[&id].reauth_stashed_prompt.is_some(),
        "receipt without live source must fail closed"
    );
    assert!(
        !effects.iter().any(|e| matches!(
            e,
            Effect::SendPrompt { .. } | Effect::SendPromptBlocks { .. }
        )),
        "unresolved exact source must not resume: {effects:?}"
    );

    // Duplicate B must not resubmit.
    let effects = dispatch(
        Action::TaskComplete(TaskResult::ProviderOperationComplete {
            agent_id: id,
            provider: ProviderKind::OpenRouter,
            status: connected("b-dup"),
            claude_cli_status: None,
            repair: Some(b),
            credential_write_receipt: None,
        }),
        &mut app,
    );
    assert!(
        !effects.iter().any(|e| matches!(
            e,
            Effect::SendPrompt { .. } | Effect::SendPromptBlocks { .. }
        )),
        "duplicate B must not resubmit: {effects:?}"
    );
}

/// OpenAI ChatGPT repair cannot resume xAI/OpenRouter; exact OpenAI works once.
#[test]
fn openai_repair_token_does_not_resume_other_providers() {
    use crate::app::agent::{CredentialRepairScope, CredentialRepairToken};
    use crate::views::providers_modal::{ProviderKind, ProviderStatus};

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let openai_scope = CredentialRepairScope {
        token: CredentialRepairToken(7),
        provider_id: "openai".into(),
        credential_generation: 3,
            incarnation: None,
            registry_generation: 0,
            failed_binding_generation: 0,
            credential_route: "api_key".into(),
            correlation_token: String::new(),
    };
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.session_id = Some(acp::SessionId::new("sess-openai"));
        agent.session.state = crate::app::agent::AgentState::Idle;
        agent.reauth_stashed_prompt = Some(crate::app::agent::ProviderScopedStashedPrompt {
            provider_id: "openrouter".into(),
            credential_generation: 3,
            incarnation: None,
            registry_generation: 0,
            binding_generation: 0,
            host_fallback: false,
            binding_complete: true,
            credential_route: "api_key".into(),
            route_authority: "authoritative".into(),
            correlation_token: String::new(),
            prompt: crate::app::agent::InFlightPrompt {
                text: "or".into(),
                images: Vec::new(),
                scrollback_entry: crate::scrollback::EntryId::new(0),
                combined_scrollback_entries: Vec::new(),
                chip_elements: Vec::new(),
            },
        });
        agent.in_flight_repair = Some(openai_scope.clone());
    }
    let effects = dispatch(
        Action::TaskComplete(TaskResult::ProviderOperationComplete {
            agent_id: id,
            provider: ProviderKind::OpenAi,
            status: ProviderStatus::Connected {
                detail: Some("ok".into()),
            },
            claude_cli_status: None,
            repair: Some(openai_scope.clone()),
            credential_write_receipt: None,
        }),
        &mut app,
    );
    assert!(
        app.agents[&id].reauth_stashed_prompt.is_some(),
        "OpenAI completion must not resume OpenRouter stash"
    );
    assert!(!effects.iter().any(|e| matches!(
        e,
        Effect::SendPrompt { .. } | Effect::SendPromptBlocks { .. }
    )));

    // Exact OpenAI stash + token resumes once.
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.reauth_stashed_prompt = Some(crate::app::agent::ProviderScopedStashedPrompt {
            provider_id: "openai".into(),
            credential_generation: 3,
            incarnation: None,
            registry_generation: 0,
            binding_generation: 0,
            host_fallback: false,
            binding_complete: true,
            credential_route: "api_key".into(),
            route_authority: "authoritative".into(),
            correlation_token: String::new(),
            prompt: crate::app::agent::InFlightPrompt {
                text: "openai prompt".into(),
                images: Vec::new(),
                scrollback_entry: crate::scrollback::EntryId::new(0),
                combined_scrollback_entries: Vec::new(),
                chip_elements: Vec::new(),
            },
        });
        agent.in_flight_repair = Some(openai_scope.clone());
    }
    let effects = dispatch(
        Action::TaskComplete(TaskResult::ProviderOperationComplete {
            agent_id: id,
            provider: ProviderKind::OpenAi,
            status: ProviderStatus::Connected {
                detail: Some("ok".into()),
            },
            claude_cli_status: None,
            repair: Some(openai_scope),
            credential_write_receipt: None,
        }),
        &mut app,
    );
    assert!(
        app.agents[&id].reauth_stashed_prompt.is_some(),
        "OpenAI receipt without live source must fail closed"
    );
    assert!(
        !effects.iter().any(|e| matches!(
            e,
            Effect::SendPrompt { .. } | Effect::SendPromptBlocks { .. }
        )),
        "no live source must not resume: {effects:?}"
    );
}

/// Startup / unbound AuthComplete never resubmits a stash.
#[test]
fn unbound_auth_complete_does_not_resubmit_stash() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.reauth_stashed_prompt = Some(crate::app::agent::ProviderScopedStashedPrompt {
            provider_id: "xai".into(),
            credential_generation: 1,
            incarnation: None,
            registry_generation: 0,
            binding_generation: 0,
            host_fallback: false,
            binding_complete: true,
            credential_route: "api_key".into(),
            route_authority: "authoritative".into(),
            correlation_token: String::new(),
            prompt: crate::app::agent::InFlightPrompt {
                text: "keep".into(),
                images: Vec::new(),
                scrollback_entry: crate::scrollback::EntryId::new(0),
                combined_scrollback_entries: Vec::new(),
                chip_elements: Vec::new(),
            },
        });
    }
    // Mid-session path needs auth_return_view so handle reaches resume logic.
    app.auth_return_view = Some(ActiveView::Agent(id));
    app.auth_state = AuthState::Authenticating {
        request_seq: 1,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };
    let effects = dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: 1,
            meta: None,
            repair: None,
            credential_write_receipt: None, // startup / unbound
        }),
        &mut app,
    );
    assert_eq!(
        app.agents[&id]
            .reauth_stashed_prompt
            .as_ref()
            .map(|s| s.prompt.text.as_str()),
        Some("keep"),
        "unbound AuthComplete must not resubmit"
    );
    assert!(!effects.iter().any(|e| matches!(
        e,
        Effect::SendPrompt { .. } | Effect::SendPromptBlocks { .. }
    )));
}

/// xAI repair AuthComplete with exact token resumes once; sibling agent
/// OpenRouter stash/CTA/in-flight stay byte-for-byte.
#[test]
fn auth_complete_xai_repair_token_resumes_once() {
    use crate::app::agent::{CredentialRepairScope, CredentialRepairToken};
    use crate::scrollback::block::RenderBlock;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let sibling_id = AgentId(1);
    let sibling_session = make_test_agent_session(&app, sibling_id, "sess-or-sibling");
    app.agents.insert(
        sibling_id,
        AgentView::new(sibling_session, ScrollbackState::new()),
    );
    app.next_agent_id = 2;

    let scope = CredentialRepairScope {
        token: CredentialRepairToken(5),
        provider_id: "xai".into(),
        credential_generation: 5,
            incarnation: None,
            registry_generation: 0,
            failed_binding_generation: 0,
            credential_route: "api_key".into(),
            correlation_token: String::new(),
    };
    let or_scope = CredentialRepairScope {
        token: CredentialRepairToken(50),
        provider_id: "openrouter".into(),
        credential_generation: 7,
            incarnation: None,
            registry_generation: 0,
            failed_binding_generation: 0,
            credential_route: "api_key".into(),
            correlation_token: String::new(),
    };
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.session_id = Some(acp::SessionId::new("sess-xai-token"));
        agent.session.state = crate::app::agent::AgentState::Idle;
        agent.reauth_stashed_prompt = Some(crate::app::agent::ProviderScopedStashedPrompt {
            provider_id: "xai".into(),
            credential_generation: 5,
            incarnation: None,
            registry_generation: 0,
            binding_generation: 0,
            host_fallback: false,
            binding_complete: true,
            credential_route: "api_key".into(),
            route_authority: "authoritative".into(),
            correlation_token: String::new(),
            prompt: crate::app::agent::InFlightPrompt {
                text: "xai prompt".into(),
                images: Vec::new(),
                scrollback_entry: crate::scrollback::EntryId::new(0),
                combined_scrollback_entries: Vec::new(),
                chip_elements: Vec::new(),
            },
        });
        agent.in_flight_repair = Some(scope.clone());
        agent
            .scrollback
            .push_block(RenderBlock::session_event(SessionEvent::ReAuthRequired));
    }
    {
        let agent = app.agents.get_mut(&sibling_id).unwrap();
        agent.reauth_stashed_prompt = Some(crate::app::agent::ProviderScopedStashedPrompt {
            provider_id: "openrouter".into(),
            credential_generation: 7,
            incarnation: None,
            registry_generation: 0,
            binding_generation: 0,
            host_fallback: false,
            binding_complete: true,
            credential_route: "api_key".into(),
            route_authority: "authoritative".into(),
            correlation_token: String::new(),
            prompt: crate::app::agent::InFlightPrompt {
                text: "sibling-openrouter".into(),
                images: Vec::new(),
                scrollback_entry: crate::scrollback::EntryId::new(0),
                combined_scrollback_entries: Vec::new(),
                chip_elements: Vec::new(),
            },
        });
        agent.in_flight_repair = Some(or_scope.clone());
        agent.scrollback.push_block(RenderBlock::session_event(
            SessionEvent::ProviderCredentialRequired {
                provider_id: "openrouter".into(),
                provider_name: "OpenRouter".into(),
                failed_model_id: None,
                credential_kind: Some("api_key".into()),
                credential_generation: Some(7),
            },
        ));
    }
    app.active_auth_repair = Some((id, scope.clone()));
    app.auth_return_view = Some(ActiveView::Agent(id));
    app.auth_state = AuthState::Authenticating {
        request_seq: 3,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };
    let effects = dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: 3,
            meta: None,
            repair: Some(scope.clone()),
            credential_write_receipt: None,
        }),
        &mut app,
    );
    assert!(
        app.agents[&id].reauth_stashed_prompt.is_some(),
        "xAI AuthComplete without write receipt must not consume stash"
    );
    // in_flight may remain until explicit cancel or successful receipt resume

    // Fail-closed complete leaves CTA; sibling still intact.
    let xai_cta = (0..app.agents[&id].scrollback.len()).any(|i| {
        matches!(
            app.agents[&id].scrollback.entry(i).map(|e| &e.block),
            Some(RenderBlock::SessionEvent(ev))
                if matches!(ev.event, SessionEvent::ReAuthRequired)
        )
    });
    assert!(xai_cta, "fail-closed xAI complete must leave CTA");
    assert!(
        !effects.iter().any(|e| matches!(
            e,
            Effect::SendPrompt { .. } | Effect::SendPromptBlocks { .. }
        )),
        "xAI session AuthComplete without receipt must not resume: {effects:?}"
    );

    let sibling = &app.agents[&sibling_id];
    assert_eq!(
        sibling
            .reauth_stashed_prompt
            .as_ref()
            .map(|s| s.prompt.text.as_str()),
        Some("sibling-openrouter")
    );
    assert_eq!(sibling.in_flight_repair.as_ref(), Some(&or_scope));
    let sibling_cta = (0..sibling.scrollback.len()).any(|i| {
        matches!(
            sibling.scrollback.entry(i).map(|e| &e.block),
            Some(RenderBlock::SessionEvent(ev))
                if matches!(
                    &ev.event,
                    SessionEvent::ProviderCredentialRequired { provider_id, .. }
                        if provider_id == "openrouter"
                )
        )
    });
    assert!(
        sibling_cta,
        "sibling OpenRouter CTA must stay after xAI complete"
    );

    // Duplicate with same token after clear: no second resume.
    app.auth_return_view = Some(ActiveView::Agent(id));
    app.auth_state = AuthState::Authenticating {
        request_seq: 4,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };
    let effects = dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: 4,
            meta: None,
            repair: Some(scope),
            credential_write_receipt: None,
        }),
        &mut app,
    );
    assert!(
        !effects.iter().any(|e| matches!(
            e,
            Effect::SendPrompt { .. } | Effect::SendPromptBlocks { .. }
        )),
        "duplicate xAI repair completion must not resubmit"
    );
    // Sibling still untouched after duplicate complete.
    assert_eq!(
        app.agents[&sibling_id]
            .reauth_stashed_prompt
            .as_ref()
            .map(|s| s.prompt.text.as_str()),
        Some("sibling-openrouter")
    );
}

#[test]
fn auth_complete_extracts_show_resolved_model_from_meta() {
    let mut app = test_app();
    app.auth_state = AuthState::Authenticating {
        request_seq: 1,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };
    assert!(app.show_resolved_model);

    dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: 1,
            meta: Some(serde_json::json!({ "show_resolved_model": false })),
            repair: None,
            credential_write_receipt: None,
        }),
        &mut app,
    );

    assert!(!app.show_resolved_model);
}

#[test]
fn auth_complete_preserves_show_resolved_model_when_absent() {
    let mut app = test_app();
    app.show_resolved_model = false;
    app.auth_state = AuthState::Authenticating {
        request_seq: 1,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };

    dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: 1,
            meta: Some(serde_json::to_value(xai_grok_shell::auth::AuthMeta::default()).unwrap()),
            repair: None,
            credential_write_receipt: None,
        }),
        &mut app,
    );

    assert!(!app.show_resolved_model);
}

/// Cancelling xAI OAuth repair must preserve sibling OpenRouter stash/CTA
/// (same agent with non-matching stash, and a second agent with its own
/// OpenRouter in-flight + stash + CTA).
#[test]
fn cancel_xai_repair_preserves_openrouter_stash_and_cta() {
    use crate::app::agent::{CredentialRepairScope, CredentialRepairToken};
    use crate::scrollback::block::RenderBlock;

    let mut app = test_app_with_agent();
    let xai_id = AgentId(0);
    let or_id = AgentId(1);
    let or_session = make_test_agent_session(&app, or_id, "or-sibling");
    app.agents
        .insert(or_id, AgentView::new(or_session, ScrollbackState::new()));
    app.next_agent_id = 2;

    let xai_scope = CredentialRepairScope {
        token: CredentialRepairToken(11),
        provider_id: "xai".into(),
        credential_generation: 1,
            incarnation: None,
            registry_generation: 0,
            failed_binding_generation: 0,
            credential_route: "api_key".into(),
            correlation_token: String::new(),
    };
    let or_scope = CredentialRepairScope {
        token: CredentialRepairToken(12),
        provider_id: "openrouter".into(),
        credential_generation: 9,
            incarnation: None,
            registry_generation: 0,
            failed_binding_generation: 0,
            credential_route: "api_key".into(),
            correlation_token: String::new(),
    };
    let or_stash_bytes = "openrouter keep";
    {
        // Agent under xAI repair: OpenRouter stash (not the repair target).
        let agent = app.agents.get_mut(&xai_id).unwrap();
        agent.reauth_stashed_prompt = Some(crate::app::agent::ProviderScopedStashedPrompt {
            provider_id: "openrouter".into(),
            credential_generation: 9,
            incarnation: None,
            registry_generation: 0,
            binding_generation: 0,
            host_fallback: false,
            binding_complete: true,
            credential_route: "api_key".into(),
            route_authority: "authoritative".into(),
            correlation_token: String::new(),
            prompt: crate::app::agent::InFlightPrompt {
                text: or_stash_bytes.into(),
                images: Vec::new(),
                scrollback_entry: crate::scrollback::EntryId::new(0),
                combined_scrollback_entries: Vec::new(),
                chip_elements: Vec::new(),
            },
        });
        agent.scrollback.push_block(RenderBlock::session_event(
            SessionEvent::ProviderCredentialRequired {
                provider_id: "openrouter".into(),
                provider_name: "OpenRouter".into(),
                failed_model_id: None,
                credential_kind: Some("api_key".into()),
                credential_generation: Some(9),
            },
        ));
        agent.in_flight_repair = Some(xai_scope.clone());
    }
    {
        // Sibling agent: independent OpenRouter repair state.
        let agent = app.agents.get_mut(&or_id).unwrap();
        agent.reauth_stashed_prompt = Some(crate::app::agent::ProviderScopedStashedPrompt {
            provider_id: "openrouter".into(),
            credential_generation: 9,
            incarnation: None,
            registry_generation: 0,
            binding_generation: 0,
            host_fallback: false,
            binding_complete: true,
            credential_route: "api_key".into(),
            route_authority: "authoritative".into(),
            correlation_token: String::new(),
            prompt: crate::app::agent::InFlightPrompt {
                text: "sibling-or".into(),
                images: Vec::new(),
                scrollback_entry: crate::scrollback::EntryId::new(0),
                combined_scrollback_entries: Vec::new(),
                chip_elements: Vec::new(),
            },
        });
        agent.in_flight_repair = Some(or_scope.clone());
        agent.scrollback.push_block(RenderBlock::session_event(
            SessionEvent::ProviderCredentialRequired {
                provider_id: "openrouter".into(),
                provider_name: "OpenRouter".into(),
                failed_model_id: None,
                credential_kind: Some("api_key".into()),
                credential_generation: Some(9),
            },
        ));
    }
    app.active_auth_repair = Some((xai_id, xai_scope));
    app.auth_return_view = Some(ActiveView::Agent(xai_id));
    app.auth_state = AuthState::Authenticating {
        request_seq: 1,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };

    dispatch(Action::CancelLogin, &mut app);

    let agent = &app.agents[&xai_id];
    assert_eq!(
        agent
            .reauth_stashed_prompt
            .as_ref()
            .map(|s| s.prompt.text.as_str()),
        Some(or_stash_bytes),
        "non-matching OpenRouter stash on cancel target must stay byte-for-byte"
    );
    let has_or_cta = (0..agent.scrollback.len()).any(|i| {
        matches!(
            agent.scrollback.entry(i).map(|e| &e.block),
            Some(RenderBlock::SessionEvent(ev))
                if matches!(
                    &ev.event,
                    SessionEvent::ProviderCredentialRequired { provider_id, .. }
                        if provider_id == "openrouter"
                )
        )
    });
    assert!(
        has_or_cta,
        "OpenRouter CTA must remain after cancelling xAI repair"
    );
    assert!(
        agent.in_flight_repair.is_none(),
        "cancel clears only the bound in-flight"
    );
    assert!(app.active_auth_repair.is_none());

    let sibling = &app.agents[&or_id];
    assert_eq!(
        sibling
            .reauth_stashed_prompt
            .as_ref()
            .map(|s| s.prompt.text.as_str()),
        Some("sibling-or")
    );
    assert_eq!(sibling.in_flight_repair.as_ref(), Some(&or_scope));
    let sibling_cta = (0..sibling.scrollback.len()).any(|i| {
        matches!(
            sibling.scrollback.entry(i).map(|e| &e.block),
            Some(RenderBlock::SessionEvent(ev))
                if matches!(
                    &ev.event,
                    SessionEvent::ProviderCredentialRequired { provider_id, .. }
                        if provider_id == "openrouter"
                )
        )
    });
    assert!(sibling_cta, "sibling OpenRouter CTA must be untouched");
}

/// Cancel exact xAI token: delayed matching AuthComplete cannot resume;
/// sibling OpenRouter stash/in-flight remain untouched through cancel + late complete.
#[test]
fn cancel_xai_token_blocks_delayed_auth_complete_resume() {
    use crate::app::agent::{CredentialRepairScope, CredentialRepairToken};

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let sibling_id = AgentId(1);
    let sibling_session = make_test_agent_session(&app, sibling_id, "sess-or-cancel");
    app.agents.insert(
        sibling_id,
        AgentView::new(sibling_session, ScrollbackState::new()),
    );
    app.next_agent_id = 2;

    let scope = CredentialRepairScope {
        token: CredentialRepairToken(22),
        provider_id: "xai".into(),
        credential_generation: 4,
            incarnation: None,
            registry_generation: 0,
            failed_binding_generation: 0,
            credential_route: "api_key".into(),
            correlation_token: String::new(),
    };
    let or_scope = CredentialRepairScope {
        token: CredentialRepairToken(23),
        provider_id: "openrouter".into(),
        credential_generation: 2,
            incarnation: None,
            registry_generation: 0,
            failed_binding_generation: 0,
            credential_route: "api_key".into(),
            correlation_token: String::new(),
    };
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.session_id = Some(acp::SessionId::new("sess-cancel-delay"));
        agent.session.state = crate::app::agent::AgentState::Idle;
        agent.reauth_stashed_prompt = Some(crate::app::agent::ProviderScopedStashedPrompt {
            provider_id: "xai".into(),
            credential_generation: 4,
            incarnation: None,
            registry_generation: 0,
            binding_generation: 0,
            host_fallback: false,
            binding_complete: true,
            credential_route: "api_key".into(),
            route_authority: "authoritative".into(),
            correlation_token: String::new(),
            prompt: crate::app::agent::InFlightPrompt {
                text: "xai-stashed".into(),
                images: Vec::new(),
                scrollback_entry: crate::scrollback::EntryId::new(0),
                combined_scrollback_entries: Vec::new(),
                chip_elements: Vec::new(),
            },
        });
        agent.in_flight_repair = Some(scope.clone());
    }
    {
        let agent = app.agents.get_mut(&sibling_id).unwrap();
        agent.reauth_stashed_prompt = Some(crate::app::agent::ProviderScopedStashedPrompt {
            provider_id: "openrouter".into(),
            credential_generation: 2,
            incarnation: None,
            registry_generation: 0,
            binding_generation: 0,
            host_fallback: false,
            binding_complete: true,
            credential_route: "api_key".into(),
            route_authority: "authoritative".into(),
            correlation_token: String::new(),
            prompt: crate::app::agent::InFlightPrompt {
                text: "or-sibling-keep".into(),
                images: Vec::new(),
                scrollback_entry: crate::scrollback::EntryId::new(0),
                combined_scrollback_entries: Vec::new(),
                chip_elements: Vec::new(),
            },
        });
        agent.in_flight_repair = Some(or_scope.clone());
    }
    app.active_auth_repair = Some((id, scope.clone()));
    app.auth_return_view = Some(ActiveView::Agent(id));
    app.auth_state = AuthState::Authenticating {
        request_seq: 5,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };

    dispatch(Action::CancelLogin, &mut app);
    // Matching xAI stash deliberately dropped on cancel so delayed complete cannot resume.
    assert!(app.agents[&id].reauth_stashed_prompt.is_none());
    assert!(app.agents[&id].in_flight_repair.is_none());
    assert_eq!(
        app.agents[&sibling_id]
            .reauth_stashed_prompt
            .as_ref()
            .map(|s| s.prompt.text.as_str()),
        Some("or-sibling-keep")
    );
    assert_eq!(
        app.agents[&sibling_id].in_flight_repair.as_ref(),
        Some(&or_scope)
    );

    app.auth_return_view = Some(ActiveView::Agent(id));
    app.auth_state = AuthState::Authenticating {
        request_seq: 6,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };
    app.agents.get_mut(&id).unwrap().reauth_stashed_prompt =
        Some(crate::app::agent::ProviderScopedStashedPrompt {
            provider_id: "xai".into(),
            credential_generation: 4,
            incarnation: None,
            registry_generation: 0,
            binding_generation: 0,
            host_fallback: false,
            binding_complete: true,
            credential_route: "api_key".into(),
            route_authority: "authoritative".into(),
            correlation_token: String::new(),
            prompt: crate::app::agent::InFlightPrompt {
                text: "should-not-resume".into(),
                images: Vec::new(),
                scrollback_entry: crate::scrollback::EntryId::new(0),
                combined_scrollback_entries: Vec::new(),
                chip_elements: Vec::new(),
            },
        });
    let effects = dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: 6,
            meta: None,
            repair: Some(scope),
            credential_write_receipt: None,
        }),
        &mut app,
    );
    assert_eq!(
        app.agents[&id]
            .reauth_stashed_prompt
            .as_ref()
            .map(|s| s.prompt.text.as_str()),
        Some("should-not-resume"),
        "delayed cancelled-token complete must not resume"
    );
    assert!(!effects.iter().any(|e| matches!(
        e,
        Effect::SendPrompt { .. } | Effect::SendPromptBlocks { .. }
    )));
    assert_eq!(
        app.agents[&sibling_id]
            .reauth_stashed_prompt
            .as_ref()
            .map(|s| s.prompt.text.as_str()),
        Some("or-sibling-keep"),
        "sibling OpenRouter stash must survive delayed cancelled complete"
    );
    assert_eq!(
        app.agents[&sibling_id].in_flight_repair.as_ref(),
        Some(&or_scope)
    );
}

/// Mismatched xAI completion preserves stashes and CTAs.
#[test]
fn mismatched_auth_complete_preserves_sibling_stashes_and_ctas() {
    use crate::app::agent::{CredentialRepairScope, CredentialRepairToken};
    use crate::scrollback::block::RenderBlock;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let flight = CredentialRepairScope {
        token: CredentialRepairToken(30),
        provider_id: "xai".into(),
        credential_generation: 1,
            incarnation: None,
            registry_generation: 0,
            failed_binding_generation: 0,
            credential_route: "api_key".into(),
            correlation_token: String::new(),
    };
    let wrong = CredentialRepairScope {
        token: CredentialRepairToken(31),
        provider_id: "xai".into(),
        credential_generation: 1,
            incarnation: None,
            registry_generation: 0,
            failed_binding_generation: 0,
            credential_route: "api_key".into(),
            correlation_token: String::new(),
    };
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.reauth_stashed_prompt = Some(crate::app::agent::ProviderScopedStashedPrompt {
            provider_id: "xai".into(),
            credential_generation: 1,
            incarnation: None,
            registry_generation: 0,
            binding_generation: 0,
            host_fallback: false,
            binding_complete: true,
            credential_route: "api_key".into(),
            route_authority: "authoritative".into(),
            correlation_token: String::new(),
            prompt: crate::app::agent::InFlightPrompt {
                text: "xai".into(),
                images: Vec::new(),
                scrollback_entry: crate::scrollback::EntryId::new(0),
                combined_scrollback_entries: Vec::new(),
                chip_elements: Vec::new(),
            },
        });
        agent.in_flight_repair = Some(flight.clone());
        agent
            .scrollback
            .push_block(RenderBlock::session_event(SessionEvent::ReAuthRequired));
    }
    app.active_auth_repair = Some((id, flight));
    app.auth_return_view = Some(ActiveView::Agent(id));
    app.auth_state = AuthState::Authenticating {
        request_seq: 1,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };

    dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: 1,
            meta: None,
            repair: Some(wrong),
            credential_write_receipt: None,
        }),
        &mut app,
    );

    let agent = &app.agents[&id];
    assert!(agent.reauth_stashed_prompt.is_some());
    let has_reauth = (0..agent.scrollback.len()).any(|i| {
        matches!(
            agent.scrollback.entry(i).map(|e| &e.block),
            Some(RenderBlock::SessionEvent(ev))
                if matches!(ev.event, SessionEvent::ReAuthRequired)
        )
    });
    assert!(has_reauth, "mismatched complete must not strip CTA");
}

/// Unbound AuthComplete preserves all provider stashes and CTAs.
#[test]
fn unbound_auth_complete_preserves_all_stashes_and_ctas() {
    use crate::scrollback::block::RenderBlock;

    let mut app = test_app_with_agent();
    let id = AgentId(0);
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.reauth_stashed_prompt = Some(crate::app::agent::ProviderScopedStashedPrompt {
            provider_id: "openrouter".into(),
            credential_generation: 2,
            incarnation: None,
            registry_generation: 0,
            binding_generation: 0,
            host_fallback: false,
            binding_complete: true,
            credential_route: "api_key".into(),
            route_authority: "authoritative".into(),
            correlation_token: String::new(),
            prompt: crate::app::agent::InFlightPrompt {
                text: "keep-or".into(),
                images: Vec::new(),
                scrollback_entry: crate::scrollback::EntryId::new(0),
                combined_scrollback_entries: Vec::new(),
                chip_elements: Vec::new(),
            },
        });
        agent.scrollback.push_block(RenderBlock::session_event(
            SessionEvent::ProviderCredentialRequired {
                provider_id: "openrouter".into(),
                provider_name: "OpenRouter".into(),
                failed_model_id: None,
                credential_kind: Some("api_key".into()),
                credential_generation: Some(2),
            },
        ));
    }
    app.auth_return_view = Some(ActiveView::Agent(id));
    app.auth_state = AuthState::Authenticating {
        request_seq: 1,
        handle: None,
        auth_url: None,
        mode: AuthMode::Pending,
    };

    dispatch(
        Action::TaskComplete(TaskResult::AuthComplete {
            request_seq: 1,
            meta: None,
            repair: None,
            credential_write_receipt: None,
        }),
        &mut app,
    );

    let agent = &app.agents[&id];
    assert_eq!(
        agent
            .reauth_stashed_prompt
            .as_ref()
            .map(|s| s.prompt.text.as_str()),
        Some("keep-or")
    );
    let has_or_cta = (0..agent.scrollback.len()).any(|i| {
        matches!(
            agent.scrollback.entry(i).map(|e| &e.block),
            Some(RenderBlock::SessionEvent(ev))
                if matches!(
                    &ev.event,
                    SessionEvent::ProviderCredentialRequired { provider_id, .. }
                        if provider_id == "openrouter"
                )
        )
    });
    assert!(
        has_or_cta,
        "unbound AuthComplete must preserve OpenRouter CTA"
    );
}

/// Store-bound API-key repair resume under an injectable tempfile auth home.
#[test]
fn store_bound_api_key_repair_resumes_once_with_temp_home() {
    use crate::app::agent::{CredentialRepairScope, CredentialRepairToken};
    use crate::views::providers_modal::{ProviderKind, ProviderStatus};
    use xai_grok_shell::auth::{OPENROUTER_API_KEY_SCOPE, store_provider_api_key};

    let dir = tempfile::tempdir().expect("temp auth home");
    let home = dir.path().to_path_buf();

    let failed_gen =
        store_provider_api_key(&home, OPENROUTER_API_KEY_SCOPE, "or-key-failed-AAAAAAAA")
            .expect("seed key");
    assert_eq!(failed_gen, 1);

    let post_gen =
        store_provider_api_key(&home, OPENROUTER_API_KEY_SCOPE, "or-key-repaired-BBBBBBBB")
            .expect("repair store");
    assert_eq!(post_gen, 2);
    assert!(post_gen > failed_gen);

    let mut app = test_app_with_agent();
    app.auth_home_override = Some(home.clone());
    let id = AgentId(0);

    let scope = CredentialRepairScope {
        token: CredentialRepairToken(42),
        provider_id: "openrouter".into(),
        credential_generation: 9,
        incarnation: None,
        registry_generation: 0,
        failed_binding_generation: failed_gen,
        credential_route: "api_key".into(),
        correlation_token: "9".into(),
    };
    {
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.session_id = Some(acp::SessionId::new("sess-store-bound"));
        agent.session.state = crate::app::agent::AgentState::Idle;
        agent.reauth_stashed_prompt = Some(crate::app::agent::ProviderScopedStashedPrompt {
            provider_id: "openrouter".into(),
            credential_generation: 9,
            incarnation: None,
            registry_generation: 0,
            binding_generation: failed_gen,
            host_fallback: false,
            binding_complete: true,
            credential_route: "api_key".into(),
            route_authority: "authoritative".into(),
            correlation_token: "9".into(),
            prompt: crate::app::agent::InFlightPrompt {
                text: "store-bound prompt".into(),
                images: Vec::new(),
                scrollback_entry: crate::scrollback::EntryId::new(0),
                combined_scrollback_entries: Vec::new(),
                chip_elements: Vec::new(),
            },
        });
        agent.in_flight_repair = Some(scope.clone());
    }

    // Missing receipt fails closed.
    let effects = dispatch(
        Action::TaskComplete(TaskResult::ProviderOperationComplete {
            agent_id: id,
            provider: ProviderKind::OpenRouter,
            status: ProviderStatus::Connected {
                detail: Some("connected".into()),
            },
            claude_cli_status: None,
            repair: Some(scope.clone()),
            credential_write_receipt: None,
        }),
        &mut app,
    );
    assert!(app.agents[&id].reauth_stashed_prompt.is_some());
    assert!(!effects
        .iter()
        .any(|e| matches!(e, Effect::SendPrompt { .. } | Effect::SendPromptBlocks { .. })));

    // Sibling provider receipt cannot resume.
    {
        let mut sibling = scope.clone();
        sibling.provider_id = "openai".into();
        app.agents.get_mut(&id).unwrap().in_flight_repair = Some(scope.clone());
        let effects = dispatch(
            Action::TaskComplete(TaskResult::ProviderOperationComplete {
                agent_id: id,
                provider: ProviderKind::OpenAi,
                status: ProviderStatus::Connected {
                    detail: Some("sibling".into()),
                },
                claude_cli_status: None,
                repair: Some(sibling.clone()),
                credential_write_receipt: Some(sibling.write_receipt(post_gen)),
            }),
            &mut app,
        );
        assert!(app.agents[&id].reauth_stashed_prompt.is_some());
        assert!(!effects
            .iter()
            .any(|e| matches!(e, Effect::SendPrompt { .. } | Effect::SendPromptBlocks { .. })));
    }

    // Stale incarnation / registry cannot resume.
    {
        let mut stale = scope.clone();
        stale.incarnation = Some("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa".into());
        stale.registry_generation = 99;
        app.agents.get_mut(&id).unwrap().in_flight_repair = Some(scope.clone());
        let effects = dispatch(
            Action::TaskComplete(TaskResult::ProviderOperationComplete {
                agent_id: id,
                provider: ProviderKind::OpenRouter,
                status: ProviderStatus::Connected {
                    detail: Some("stale".into()),
                },
                claude_cli_status: None,
                repair: Some(stale.clone()),
                credential_write_receipt: Some(stale.write_receipt(post_gen)),
            }),
            &mut app,
        );
        assert!(app.agents[&id].reauth_stashed_prompt.is_some());
        assert!(!effects
            .iter()
            .any(|e| matches!(e, Effect::SendPrompt { .. } | Effect::SendPromptBlocks { .. })));
    }

    // External rotation below the store-bound post fails closed (live != post).
    {
        let _ = store_provider_api_key(&home, OPENROUTER_API_KEY_SCOPE, "or-key-external-CCCCCCCC")
            .expect("external rotate");
        // post_gen was 2; live is now 3 — receipt claiming post=2 is rejected.
        app.agents.get_mut(&id).unwrap().in_flight_repair = Some(scope.clone());
        let effects = dispatch(
            Action::TaskComplete(TaskResult::ProviderOperationComplete {
                agent_id: id,
                provider: ProviderKind::OpenRouter,
                status: ProviderStatus::Connected {
                    detail: Some("stale live".into()),
                },
                claude_cli_status: None,
                repair: Some(scope.clone()),
                credential_write_receipt: Some(scope.write_receipt(post_gen)),
            }),
            &mut app,
        );
        assert!(
            app.agents[&id].reauth_stashed_prompt.is_some(),
            "external rotation must fail closed when live != post"
        );
        assert!(!effects
            .iter()
            .any(|e| matches!(e, Effect::SendPrompt { .. } | Effect::SendPromptBlocks { .. })));
        // Restore exact post generation for the success path.
        let restored =
            store_provider_api_key(&home, OPENROUTER_API_KEY_SCOPE, "or-key-repaired-BBBBBBBB")
                .expect("restore");
        // Generations are monotonic — capture the live post for a fresh receipt.
        let live_post = restored;
        assert!(live_post > failed_gen);

        // Exact store-bound receipt + live re-resolve resumes once.
        app.agents.get_mut(&id).unwrap().in_flight_repair = Some(scope.clone());
        let mut receipt = scope.write_receipt(live_post);
        // The receipt must carry the actual store-returned post generation.
        receipt.post_generation = live_post;
        let effects = dispatch(
            Action::TaskComplete(TaskResult::ProviderOperationComplete {
                agent_id: id,
                provider: ProviderKind::OpenRouter,
                status: ProviderStatus::Connected {
                    detail: Some("connected".into()),
                },
                claude_cli_status: None,
                repair: Some(scope.clone()),
                credential_write_receipt: Some(receipt),
            }),
            &mut app,
        );
        assert!(
            app.agents[&id].reauth_stashed_prompt.is_none(),
            "store-bound receipt must consume stash once"
        );
        let send_count = effects
            .iter()
            .filter(|e| matches!(e, Effect::SendPrompt { .. } | Effect::SendPromptBlocks { .. }))
            .count();
        assert_eq!(
            send_count, 1,
            "store-bound repair must send prompt exactly once: {effects:?}"
        );

        // Duplicate is a no-op.
        let effects = dispatch(
            Action::TaskComplete(TaskResult::ProviderOperationComplete {
                agent_id: id,
                provider: ProviderKind::OpenRouter,
                status: ProviderStatus::Connected {
                    detail: Some("dup".into()),
                },
                claude_cli_status: None,
                repair: Some(scope.clone()),
                credential_write_receipt: Some(scope.write_receipt(live_post)),
            }),
            &mut app,
        );
        assert!(!effects
            .iter()
            .any(|e| matches!(e, Effect::SendPrompt { .. } | Effect::SendPromptBlocks { .. })));
    }
}
