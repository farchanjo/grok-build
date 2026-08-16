//! Login, logout, account switching, and auth-code submission dispatchers.

use super::ctx::{restore_auth_return_view, show_welcome};
use super::queue::{maybe_drain_queue, note_peek_page_flip};
use super::router::dispatch;
use super::session::lifecycle::{clear_startup_actions, drain_startup_actions};
use crate::app::actions::{Action, Effect};
use crate::app::agent::AgentId;
use crate::app::agent_view::AgentView;
use crate::app::app_view::{ActiveView, AppView, AuthMode, AuthState};
use crate::scrollback::block::RenderBlock;
use crate::scrollback::blocks::SessionEvent;

// ---------------------------------------------------------------------------
// Auth dispatch
// ---------------------------------------------------------------------------

/// Internal xAI disconnect (from `/providers` LogoutXai). Not a global `/logout` route.
pub(super) fn dispatch_logout(_app: &mut AppView) -> Vec<Effect> {
    vec![Effect::Logout]
}

/// Ensure `login_method_id` is populated from stored auth methods.
/// On the eager-auth path (cached token), login_method_id is never set
/// because the user skipped the login screen.
///
/// Does **not** invent `grok.com` when no interactive method is advertised
/// (e.g. `preferred_method=api_key` with no key — empty `auth_methods`).
/// Callers already surface "No login method available" when this leaves
/// `login_method_id` unset.
pub(super) fn ensure_login_method(app: &mut AppView) {
    if app.login_method_id.is_some() {
        return;
    }
    let (label, method_id, start_mode) =
        crate::acp::find_interactive_login_method(&app.auth_methods);
    if let Some(id) = method_id {
        app.login_label = label;
        app.login_method_id = Some(id);
        app.auth_start_mode = match start_mode {
            crate::acp::AuthStartMode::Pending => AuthMode::Pending,
            crate::acp::AuthStartMode::Command => AuthMode::Command,
        };
    }
    // No interactive method: leave login_method_id unset (fail-closed).
}

/// Error when no interactive login method is available (empty auth_methods,
/// e.g. `preferred_method=api_key` with no credentials). Prefer the shell's
/// pin-unavailable copy when the list is empty.
fn no_login_method_error(app: &AppView) -> String {
    if app.auth_methods.is_empty() {
        xai_grok_shell::agent::auth_method::PREFERRED_API_KEY_UNAVAILABLE.to_string()
    } else {
        "No login method available".to_string()
    }
}

/// Abort any in-flight Authenticate/SwitchAccount task *and* its URL poll so a
/// new login cannot stack device-code mints or have a stale poll steal the
/// successor's URL (single-flight). No-op when not authenticating or when the
/// abort handles have not been installed yet.
fn abort_prior_auth(app: &mut AppView) {
    if let AuthState::Authenticating {
        handle,
        request_seq,
        ..
    } = &mut app.auth_state
        && let Some(h) = handle.take()
    {
        tracing::debug!(
            request_seq,
            "aborting prior in-flight auth task for single-flight"
        );
        h.abort();
    }
    if let Some((seq, h)) = app.auth_url_poll_handle.take() {
        tracing::debug!(
            request_seq = seq,
            "aborting prior auth URL poll for single-flight"
        );
        h.abort();
    }
}

/// Log out, then start a new login flow in a single sequential task.
pub(super) fn dispatch_switch_account(app: &mut AppView) -> Vec<Effect> {
    ensure_login_method(app);

    let Some(method_id) = app.login_method_id.clone() else {
        app.auth_state = AuthState::Pending {
            error: Some(no_login_method_error(app)),
        };
        return vec![];
    };

    abort_prior_auth(app);

    let request_seq = app.next_auth_request_seq;
    app.next_auth_request_seq += 1;
    app.auth_code_input.reset();
    app.auth_state = AuthState::Authenticating {
        request_seq,
        handle: None,
        auth_url: None,
        mode: app.auth_start_mode,
    };

    vec![
        Effect::SwitchAccount {
            request_seq,
            method_id,
            use_oauth: app.auth_use_oauth,
        },
        Effect::PollAuthUrl { request_seq },
    ]
}

/// Scan the trailing run of session-event / system blocks for a
/// credential-repair prompt ([`SessionEvent::ReAuthRequired`] or
/// [`SessionEvent::ProviderCredentialRequired`]). Used by the `PromptResponse`
/// handler to suppress the redundant "Turn failed" block after a 401 — the
/// repair prompt is pushed by the `RetryState` handler, which runs first.
pub(super) fn scrollback_has_recent_reauth_prompt(
    scrollback: &crate::scrollback::state::ScrollbackState,
) -> bool {
    scrollback_recent_credential_scope(scrollback).is_some()
        || scrollback_has_trailing_session_event(scrollback, |ev| {
            matches!(ev, SessionEvent::ReAuthRequired)
        })
}

/// Provider id + generation from the trailing credential-repair block, if any.
/// Used to key blocked-prompt stashes so sibling-provider reconnects cannot
/// resubmit. First-party `ReAuthRequired` maps to `("xai", 0)`.
pub(super) fn scrollback_recent_credential_scope(
    scrollback: &crate::scrollback::state::ScrollbackState,
) -> Option<(String, u64)> {
    use crate::scrollback::block::RenderBlock;
    for idx in (0..scrollback.len()).rev() {
        match scrollback.entry(idx).map(|e| &e.block) {
            Some(RenderBlock::SessionEvent(ev)) => match &ev.event {
                SessionEvent::ProviderCredentialRequired {
                    provider_id,
                    credential_generation,
                    ..
                } => {
                    return Some((provider_id.clone(), credential_generation.unwrap_or(0)));
                }
                SessionEvent::ReAuthRequired => return Some(("xai".to_string(), 0)),
                _ => {}
            },
            Some(RenderBlock::System(_)) => {}
            _ => break,
        }
    }
    None
}

fn scrollback_has_trailing_session_event(
    scrollback: &crate::scrollback::state::ScrollbackState,
    pred: impl Fn(&SessionEvent) -> bool,
) -> bool {
    use crate::scrollback::block::RenderBlock;
    for idx in (0..scrollback.len()).rev() {
        match scrollback.entry(idx).map(|e| &e.block) {
            Some(RenderBlock::SessionEvent(ev)) => {
                if pred(&ev.event) {
                    return true;
                }
            }
            Some(RenderBlock::System(_)) => {}
            _ => break,
        }
    }
    false
}

/// True if the trailing run of session/system blocks contains a terminal
/// context-overflow block ([`SessionEvent::ContextTooLarge`] or `CompactionFailed`).
/// Lets `PromptResponse` suppress the redundant `TurnFailed`, mirroring reauth.
pub(super) fn scrollback_has_recent_context_too_large(
    scrollback: &crate::scrollback::state::ScrollbackState,
) -> bool {
    use crate::scrollback::block::RenderBlock;
    for idx in (0..scrollback.len()).rev() {
        match scrollback.entry(idx).map(|e| &e.block) {
            Some(RenderBlock::SessionEvent(ev)) => {
                if matches!(
                    ev.event,
                    SessionEvent::ContextTooLarge | SessionEvent::CompactionFailed { .. }
                ) {
                    return true;
                }
            }
            // Tolerate interleaved system messages in the trailing run.
            Some(RenderBlock::System(_)) => {}
            // Stop at the first substantive block.
            _ => break,
        }
    }
    false
}

/// Strip the trailing run of auth-error blocks — the `ReAuthRequired`
/// prompt plus any stale `RetryFailed` / `TurnFailed` — from an agent's
/// scrollback. Called after a successful mid-session re-auth so the prompt
/// disappears once the user returns to the session. Mirrors the
/// credit-limit upsell's stale-block strip.
pub(super) fn strip_trailing_auth_error_blocks(agent: &mut AgentView) {
    use crate::scrollback::block::RenderBlock;
    let mut to_remove = Vec::new();
    for idx in (0..agent.scrollback.len()).rev() {
        match agent.scrollback.entry(idx).map(|e| &e.block) {
            Some(RenderBlock::SessionEvent(ev))
                if matches!(
                    &ev.event,
                    SessionEvent::ReAuthRequired
                        | SessionEvent::ProviderCredentialRequired { .. }
                        | SessionEvent::RetryFailed { .. }
                        | SessionEvent::TurnFailed { .. }
                ) =>
            {
                to_remove.push(idx);
            }
            // Skip over other trailing session-event / system blocks.
            Some(RenderBlock::SessionEvent(_) | RenderBlock::System(_)) => continue,
            // Stop at the first substantive block.
            _ => break,
        }
    }
    for idx in to_remove {
        agent.scrollback.remove_from(idx);
    }
}

/// Mint a repair scope if an agent has a stashed failure for `provider_id`.
/// Binds a fresh never-reused token to the stash's frozen generation.
///
/// Re-resolve the durable binding generation after a repair completes.
/// Uses the app's auth home (process config home in production; optional
/// tempfile override in hermetic tests). Missing sources return `None`.
pub(in crate::app::dispatch) fn live_binding_gen_for_resume(
    auth_home: &std::path::Path,
    scope: &crate::app::agent::CredentialRepairScope,
) -> Option<u64> {
    xai_grok_shell::session::route_context::live_binding_generation_for_route(
        auth_home,
        &scope.provider_id,
        &scope.credential_route,
        scope.incarnation.as_deref(),
    )
}

/// Returns `(agent_id, scope)` so cancel/complete can target only that agent.
pub(in crate::app::dispatch) fn begin_credential_repair(
    app: &mut AppView,
    provider_id: &str,
) -> Option<(AgentId, crate::app::agent::CredentialRepairScope)> {
    use crate::app::agent::{CredentialRepairScope, mint_credential_repair_token};

    let (
        agent_id,
        generation,
        incarnation,
        registry_generation,
        failed_binding_generation,
        credential_route,
        correlation_token,
    ) = match app.active_view {
        ActiveView::Agent(id) => {
            let agent = app.agents.get(&id)?;
            let stashed = agent.reauth_stashed_prompt.as_ref()?;
            if stashed.provider_id != provider_id {
                return None;
            }
            if !stashed.repair_eligible() {
                return None;
            }
            (
                id,
                stashed.credential_generation,
                stashed.incarnation.clone(),
                stashed.registry_generation,
                stashed.binding_generation,
                stashed.credential_route.clone(),
                stashed.correlation_token.clone(),
            )
        }
        _ => app.agents.iter().find_map(|(id, agent)| {
            agent.reauth_stashed_prompt.as_ref().and_then(|s| {
                (s.provider_id == provider_id && s.repair_eligible()).then_some((
                    *id,
                    s.credential_generation,
                    s.incarnation.clone(),
                    s.registry_generation,
                    s.binding_generation,
                    s.credential_route.clone(),
                    s.correlation_token.clone(),
                ))
            })
        })?,
    };

    let token = mint_credential_repair_token(&mut app.next_credential_repair_token)?;
    let scope = CredentialRepairScope {
        token,
        provider_id: provider_id.to_owned(),
        credential_generation: generation,
        incarnation,
        registry_generation,
        failed_binding_generation,
        credential_route,
        correlation_token,
    };
    app.agents.get_mut(&agent_id)?.in_flight_repair = Some(scope.clone());
    Some((agent_id, scope))
}

/// Start an interactive xAI OAuth flow. Called only from `/providers`
/// (LoginXai) or equivalent provider-scoped paths — never a global slash
/// command.
///
/// When invoked mid-session (the active view is an agent/dashboard rather
/// than the welcome screen), the auth UI — including the external auth
/// provider's sign-in URL and status — is only rendered by the welcome
/// view. We therefore stash the caller's view in `auth_return_view` and
/// switch to `Welcome` so the flow is actually visible; the prior view is
/// restored once auth completes or is cancelled.
pub(super) fn dispatch_login(app: &mut AppView) -> Vec<Effect> {
    ensure_login_method(app);
    let Some(method_id) = app.login_method_id.clone() else {
        app.auth_state = AuthState::Pending {
            error: Some(no_login_method_error(app)),
        };
        return vec![];
    };

    // Bind repair token only when launched against a matching xAI stash.
    let repair_binding = begin_credential_repair(app, "xai");
    app.active_auth_repair = repair_binding.clone();
    let repair = repair_binding.map(|(_, scope)| scope);

    // Surface the auth UI when triggered from inside a session. `show_welcome`
    // resets ephemeral state here, covering the AuthComplete / cancel-login
    // fallbacks too (`auth_return_view` is only ever set here).
    if !matches!(app.active_view, ActiveView::Welcome) {
        app.auth_return_view = Some(app.active_view);
        show_welcome(app);
    }

    abort_prior_auth(app);

    let request_seq = app.next_auth_request_seq;
    app.next_auth_request_seq += 1;
    app.auth_code_input.reset();
    app.auth_state = AuthState::Authenticating {
        request_seq,
        handle: None,
        auth_url: None,
        mode: app.auth_start_mode,
    };

    vec![
        Effect::Authenticate {
            request_seq,
            method_id,
            use_oauth: app.auth_use_oauth,
            force_interactive: true,
            repair,
        },
        Effect::PollAuthUrl { request_seq },
    ]
}

/// Cancel an in-session xAI OAuth detour and restore the caller's view.
/// Only meaningful when `auth_return_view` is set (provider-scoped re-auth).
/// Aborts the in-flight auth task and tells the shell to cancel its
/// device/loopback flow so a retry does not race a still-polling prior mint.
///
/// Touches **only** the agent bound in [`AppView::active_auth_repair`]. Sibling
/// OpenAI/OpenRouter/xAI stashes and CTAs are left byte-for-byte.
pub(super) fn dispatch_cancel_login(app: &mut AppView) -> Vec<Effect> {
    let Some(return_view) = app.auth_return_view.take() else {
        return vec![];
    };
    // Capture the attempt's request_seq before abort clears Authenticating so
    // the shell cancel is scoped to this attempt only (a delayed RPC must not
    // cancel a fast re-login).
    let cancel_seq = match &app.auth_state {
        AuthState::Authenticating { request_seq, .. } => Some(*request_seq),
        _ => None,
    };
    abort_prior_auth(app);
    app.next_auth_request_seq += 1;
    app.auth_state = AuthState::Done;
    app.auth_show_raw_url = false;
    app.auth_code_input.reset();
    restore_auth_return_view(app, return_view);

    // Exact bound repair only: clear its in-flight token so a delayed
    // AuthComplete cannot resume. Drop the matching stash and strip CTA only
    // when the stash is for this same repair scope. Sibling stashes/CTAs stay.
    if let Some((agent_id, scope)) = app.active_auth_repair.take() {
        if let Some(agent) = app.agents.get_mut(&agent_id) {
            let bound = agent.in_flight_repair.as_ref().is_some_and(|f| f == &scope);
            if bound {
                agent.in_flight_repair = None;
                let drop_stash = agent.reauth_stashed_prompt.as_ref().is_some_and(|s| {
                    s.matches_repair(&scope.provider_id, scope.credential_generation)
                });
                if drop_stash {
                    agent.reauth_stashed_prompt = None;
                    strip_trailing_auth_error_blocks(agent);
                }
            }
        }
    }

    // Ask the shell to cancel its in-flight interactive auth (device poll /
    // loopback wait). Fire-and-forget: UI state is already restored.
    match cancel_seq {
        Some(request_seq) => vec![Effect::CancelAuth { request_seq }],
        None => vec![],
    }
}

/// User submitted a manually-pasted auth token in loopback mode.
pub(super) fn dispatch_submit_auth_code(app: &mut AppView, code: String) -> Vec<Effect> {
    let request_seq = match &app.auth_state {
        AuthState::Authenticating { request_seq, .. } => *request_seq,
        _ => return vec![],
    };

    vec![Effect::SubmitAuthCode { request_seq, code }]
}

// TaskResult handlers.

pub(super) fn handle_auth_complete(
    app: &mut AppView,
    request_seq: u64,
    meta: Option<serde_json::Value>,
    repair: Option<crate::app::agent::CredentialRepairScope>,
    credential_write_receipt: Option<crate::app::agent::CredentialWriteReceipt>,
) -> Vec<Effect> {
    if let AuthState::Authenticating {
        request_seq: current_seq,
        ..
    } = &app.auth_state
        && *current_seq == request_seq
    {
        if let Some(meta_val) = meta.as_ref()
            && let Ok(auth_meta) =
                serde_json::from_value::<xai_grok_shell::auth::AuthMeta>(meta_val.clone())
        {
            app.apply_auth_meta(&auth_meta);
        }

        app.auth_state = AuthState::Done;
        app.auth_show_raw_url = false;
        app.welcome_prompt_focused = !app.is_access_blocked();
        app.auth_code_input.reset();

        // Mid-session re-auth (provider-scoped OAuth): restore the view the
        // user was on instead of running the startup load-session flow.
        if let Some(return_view) = app.auth_return_view.take() {
            restore_auth_return_view(app, return_view);
            clear_startup_actions(app);

            let mut retry_effects = Vec::new();
            let mut page_flips = Vec::new();
            let auth_home = app.auth_home();

            // Only the agent bound to the immutable completion scope may be
            // stripped/resumed. Unbound (repair=None) or mismatched completions
            // leave every provider stash and CTA untouched.
            let bound = app.active_auth_repair.take();
            if let Some(repair_scope) = repair.as_ref() {
                // Prefer explicit active_auth_repair agent; fall back to any
                // agent whose in_flight_repair matches the completion scope.
                let target_id = bound
                    .as_ref()
                    .filter(|(_, s)| s == repair_scope)
                    .map(|(id, _)| *id)
                    .or_else(|| {
                        app.agents.iter().find_map(|(id, agent)| {
                            agent
                                .in_flight_repair
                                .as_ref()
                                .filter(|f| *f == repair_scope)
                                .map(|_| *id)
                        })
                    });

                if let Some(agent_id) = target_id
                    && let Some(agent) = app.agents.get_mut(&agent_id)
                {
                    if let Some(stashed) = agent.reauth_stashed_prompt.take() {
                        let live = live_binding_gen_for_resume(&auth_home, repair_scope);
                        if repair_scope.allows_resume(agent.in_flight_repair.as_ref(), &stashed)
                            && repair_scope
                                .validate_write_receipt(credential_write_receipt.as_ref(), live)
                        {
                            agent.in_flight_repair = None;
                            // Strip only this agent's credential CTA after exact match.
                            strip_trailing_auth_error_blocks(agent);
                            agent.scrollback.push_block(RenderBlock::system(
                                "Reconnected xAI. Retrying\u{2026}".to_string(),
                            ));
                            agent.session.enqueue_in_flight_prompt_front(stashed.prompt);
                            let drain = maybe_drain_queue(agent);
                            retry_effects.extend(drain.effects);
                            page_flips.push((agent.session.id, drain.page_flip_entry));
                        } else {
                            agent.reauth_stashed_prompt = Some(stashed);
                        }
                    } else if agent
                        .in_flight_repair
                        .as_ref()
                        .is_some_and(|f| f.token == repair_scope.token)
                    {
                        // Clear matching in-flight without stripping CTAs.
                        agent.in_flight_repair = None;
                    }
                }
            }
            // else: unbound AuthComplete — no strip, no stash touch.

            for (id, page_flip_entry) in page_flips {
                note_peek_page_flip(app, id, page_flip_entry);
            }
            let mut effects = dispatch(Action::RequestBundleStatus, app);
            if app.usage_visible {
                effects.push(Effect::FetchAppBilling);
            }
            effects.extend(retry_effects);
            return effects;
        }

        // status only; shell auto-syncs post-auth
        let mut effects = dispatch(Action::RequestBundleStatus, app);

        // Start auto-checking subscription if gated.
        // Check immediately (don't wait 5s) then schedule the timer.
        if !app.has_access() {
            app.paywall_check_started = Some(std::time::Instant::now());
            effects.push(Effect::CheckSubscription { verify: None });
            effects.push(Effect::SchedulePaywallCheck);
        }
        // Fetch billing so the welcome screen can show a credit warning.
        if app.usage_visible {
            effects.push(Effect::FetchAppBilling);
        }
        // Fetch changelog (mirrors startup path for interactive login).
        effects.push(Effect::FetchChangelog);

        // ZDR-blocked users stay on the welcome screen — discard any
        // deferred startup (they cannot start a session).
        if app.is_zdr_blocked() {
            clear_startup_actions(app);
            return effects;
        }

        // Replay deferred session startup once BOTH gates are open. Auth
        // is now Done, so `session_startup_allowed()` here means "is trust
        // also resolved?" -- if trust is still Pending its question renders
        // next and its answer drains instead. Same predicate the trust
        // handlers use, so the deferred startup runs exactly once after
        // whichever gate resolves last.
        if app.session_startup_allowed() {
            let mut drained = drain_startup_actions(app);
            // Auth was the last startup gate (or trust was already resolved).
            // If no deferred intent opened a session / dashboard, land on the
            // grouped session picker so the user can resume a previous session.
            drained.extend(super::ctx::maybe_open_welcome_session_picker(app));
            effects.extend(drained);
        }
        return effects;
    }
    vec![]
}

pub(super) fn handle_auth_url_ready(
    app: &mut AppView,
    request_seq: u64,
    auth_url: Option<String>,
    external: bool,
    mode: Option<String>,
) -> Vec<Effect> {
    if let AuthState::Authenticating {
        request_seq: current_seq,
        auth_url: current_url,
        mode: current_mode,
        ..
    } = &mut app.auth_state
        && *current_seq == request_seq
    {
        *current_url = auth_url;
        // Prefer `mode`; fall back to `external` for older agents. An
        // old-agent device login lands on Loopback (harmless paste box;
        // the background poll still completes).
        *current_mode = match mode.as_deref() {
            Some("device") => AuthMode::Device,
            Some("command") => AuthMode::Command,
            Some("loopback") => AuthMode::Loopback,
            _ if external => AuthMode::Command,
            _ => AuthMode::Loopback,
        };
    }
    vec![]
}

pub(super) fn handle_mcp_auth_trigger_done(
    app: &mut AppView,
    agent_id: AgentId,
    server_name: String,
    result: Result<crate::app::actions::McpAuthTriggerOutcome, String>,
) -> Vec<Effect> {
    let Some(agent) = app.agents.get_mut(&agent_id) else {
        return vec![];
    };
    if let Some(ref mut modal) = agent.extensions_modal {
        modal.pending_action = None;
        modal.pending_entry_index = None;
        match result {
            Ok(crate::app::actions::McpAuthTriggerOutcome::Authenticated) => {}
            Ok(crate::app::actions::McpAuthTriggerOutcome::SetupRequired(setup)) => {
                let setup_values = match &modal.mcps_data {
                    crate::views::extensions_modal::TabDataState::Loaded(servers) => servers
                        .iter()
                        .find(|server| server.name == server_name)
                        .map(|server| server.setup_values.clone())
                        .unwrap_or_default(),
                    _ => std::collections::HashMap::new(),
                };
                if let Some(form) = crate::views::extensions_modal::McpSetupFormState::from_setup(
                    server_name.clone(),
                    setup,
                    setup_values,
                ) {
                    modal.mcp_setup = Some(form);
                } else {
                    modal.modal_message =
                        Some(crate::views::extensions_modal::ModalMessage::Error(
                            format!("{server_name}: setup schema is not supported in this UI"),
                        ));
                }
                return vec![];
            }
            Err(e) => {
                let msg = if e.starts_with("To authenticate") {
                    format!("{server_name}: {e}")
                } else if e.contains(&server_name) {
                    format!("Auth failed: {e}")
                } else {
                    format!("{server_name} auth failed: {e}")
                };
                modal.modal_message =
                    Some(crate::views::extensions_modal::ModalMessage::Error(msg));
                if let Some(session_id) = agent.session.session_id.clone() {
                    return vec![Effect::FetchMcpsList {
                        agent_id,
                        session_id,
                        cache: false,
                    }];
                }
                return vec![];
            }
        }
    }
    // No toast on success: the row transition from the FetchMcpsList
    // refresh below is the confirmation.
    let Some(session_id) = agent.session.session_id.clone() else {
        return vec![];
    };
    vec![Effect::FetchMcpsList {
        agent_id,
        session_id,
        cache: false,
    }]
}

pub(super) fn handle_mcp_setup_submit_done(
    app: &mut AppView,
    agent_id: AgentId,
    server_name: String,
    result: Result<(), String>,
) -> Vec<Effect> {
    let Some(agent) = app.agents.get_mut(&agent_id) else {
        return vec![];
    };
    if let Some(ref mut modal) = agent.extensions_modal {
        if let Err(e) = result {
            modal.pending_action = None;
            modal.pending_entry_index = None;
            modal.modal_message = Some(crate::views::extensions_modal::ModalMessage::Error(
                format!("{server_name} setup failed: {e}"),
            ));
            return vec![];
        }
        modal.pending_action = Some(format!("Authenticating {server_name}..."));
        modal.pending_entry_index = None;
    }
    let Some(session_id) = agent.session.session_id.clone() else {
        if let Some(ref mut modal) = agent.extensions_modal {
            modal.pending_action = None;
            modal.modal_message = Some(crate::views::extensions_modal::ModalMessage::Error(
                format!("{server_name}: no active session for authentication"),
            ));
        }
        return vec![];
    };
    vec![Effect::McpAuthTrigger {
        agent_id,
        session_id,
        server_name,
    }]
}
