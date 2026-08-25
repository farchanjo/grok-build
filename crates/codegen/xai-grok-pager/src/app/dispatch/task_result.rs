//! Async task-result application: routes task results into state.
use super::auth::{
    ensure_login_method, handle_auth_complete, handle_auth_url_ready, handle_mcp_auth_trigger_done,
    handle_mcp_setup_submit_done,
};
use super::billing::{
    PAYWALL_AUTO_CHECK_TIMEOUT, apply_auto_topup, handle_billing_fetched,
    handle_check_subscription_complete, handle_credit_limit_recheck_complete,
    handle_gate_refreshed, handle_gate_verify_timeout,
};
use super::cta::{
    handle_cta_plugin_install_done, handle_cta_plugin_reload_done,
    handle_plugin_cta_catalog_loaded, handle_plugin_cta_debounce_expired,
    handle_plugin_cta_mcps_loaded,
};
use super::ctx::{find_agent_by_session_id, get_active_agent_mut};
use super::notes::{handle_btw_response, handle_memory_note_saved};
use super::prompt::{
    defer_to_open_reload_window, handle_compact_complete, handle_prompt_response,
    handle_suggestion_debounce_expired,
};
use super::rewind::{
    dispatch_rewind_success, handle_rewind_execute_failed, handle_rewind_points_loaded,
    handle_rewind_preview_complete, handle_rewind_preview_failed,
};
use super::router::{dispatch, dispatch_action_result};
use super::session::foreign::{
    handle_foreign_sessions_scanned, handle_session_list_failed, handle_session_list_loaded,
};
use super::session::fork::{
    handle_fork_session_failed, handle_fork_session_ready, handle_worktree_forked,
};
use super::session::lifecycle::{
    dispatch_exit_session, handle_session_created, handle_session_failed,
    handle_switch_model_complete, handle_worktree_session_created, handle_worktree_session_failed,
};
use super::session::load::{
    handle_card_detail_loaded, handle_deep_search_results, handle_session_load_failed,
    handle_session_loaded, handle_session_restore_failed, handle_session_restored,
    handle_session_search_debounce_expired, remove_session_from_pickers,
};
use super::settings::ui::apply_setting_rollback;
use super::status::{
    commit_session_usage_block, handle_coding_data_sharing_failed,
    handle_coding_data_sharing_updated, handle_context_info_complete, scrub_error_for_toast,
};
use super::transcript::{
    handle_hooks_list_loaded, handle_marketplace_list_loaded, handle_marketplace_updates_available,
    handle_mcp_toggle_done, handle_plugins_list_loaded, handle_skills_toggle_done,
};
use super::turn::handle_bg_task_killed;
use crate::app::actions::{
    ClipboardPasteCompletion, ClipboardPasteContext, ClipboardPasteFailure, ClipboardPasteTarget,
    DoctorFixTarget, DoctorPlanningOutcome, Effect, ProbedAttachment, SubagentKillOutcome,
    TaskResult,
};
use crate::app::agent::AgentId;
use crate::app::app_view::{ActiveView, AppView, AuthState};
use crate::scrollback::block::RenderBlock;
use agent_client_protocol as acp;
pub(super) fn unregister_session_effect(session_id: Option<acp::SessionId>) -> Vec<Effect> {
    session_id
        .map(|sid| Effect::UnregisterActiveSession { session_id: sid })
        .into_iter()
        .collect()
}
pub(super) fn unregister_all_active_sessions(app: &AppView) -> Vec<Effect> {
    app.agents
        .values()
        .filter_map(|a| {
            a.session
                .session_id
                .as_ref()
                .map(|sid| Effect::UnregisterActiveSession {
                    session_id: sid.clone(),
                })
        })
        .collect()
}
pub(super) const X11_PRIMARY_PASTE_HINT: &str = "Try Shift+Insert to paste selected text";
fn show_clipboard_toast(target: &ClipboardPasteTarget, message: &str, app: &mut AppView) {
    match target {
        ClipboardPasteTarget::AgentPrompt { agent_id, .. } => {
            if let Some(agent) = app.agents.get_mut(agent_id) {
                agent.show_toast(message);
            }
        }
        ClipboardPasteTarget::DashboardDispatch | ClipboardPasteTarget::DashboardPeek { .. } => {
            if let Some(dashboard) = app.dashboard.as_mut() {
                dashboard.error_toast = Some(message.to_owned());
            }
        }
    }
}
pub(super) fn maybe_show_x11_primary_paste_hint(
    eligible: bool,
    completion: ClipboardPasteCompletion,
    target: &ClipboardPasteTarget,
    app: &mut AppView,
) {
    if !eligible || completion != ClipboardPasteCompletion::FullMiss {
        return;
    }
    show_clipboard_toast(target, X11_PRIMARY_PASTE_HINT, app);
}
/// Whether a completed clipboard probe should fall through to the `grok wrap`
/// host-image request. A clean `FullMiss` always qualifies; a remote read
/// *error* (`AttachmentRead`) also qualifies because inside `grok wrap` the
/// authoritative pasteboard is the local host's, not the (absent) remote one, so
/// the error is recoverable over the wrap OSC path. Every other failure
/// (`TextRead`, `TargetInsertion`, `AlreadyReported`) is a real dead end and
/// must keep toasting. The request itself still self-gates on
/// `osc52_sink_active()`, so this is inert outside `grok wrap`.
pub(super) fn wrap_host_image_request_eligible(completion: ClipboardPasteCompletion) -> bool {
    matches!(
        completion,
        ClipboardPasteCompletion::FullMiss
            | ClipboardPasteCompletion::Failed(ClipboardPasteFailure::AttachmentRead)
    )
}
pub(super) fn show_clipboard_failure(
    target: &ClipboardPasteTarget,
    failure: ClipboardPasteFailure,
    app: &mut AppView,
) {
    let message = match failure {
        ClipboardPasteFailure::AlreadyReported => return,
        ClipboardPasteFailure::TextRead => "Couldn't read clipboard text",
        ClipboardPasteFailure::AttachmentRead => "Couldn't read clipboard contents",
        ClipboardPasteFailure::TargetInsertion => "Couldn't paste clipboard contents",
    };
    show_clipboard_toast(target, message, app);
}
fn apply_clipboard_paste_result(
    ctx: ClipboardPasteContext,
    image: ProbedAttachment,
    file_urls: Option<String>,
    app: &mut AppView,
) -> ClipboardPasteCompletion {
    match ctx.target.clone() {
        ClipboardPasteTarget::AgentPrompt { agent_id, .. } => app
            .agents
            .get_mut(&agent_id)
            .map_or(ClipboardPasteCompletion::Dropped, |agent| {
                agent.complete_clipboard_attachment_paste(ctx, image, file_urls)
            }),
        ClipboardPasteTarget::DashboardDispatch | ClipboardPasteTarget::DashboardPeek { .. } => app
            .dashboard
            .as_mut()
            .map_or(ClipboardPasteCompletion::Dropped, |dashboard| {
                dashboard.complete_clipboard_attachment_paste(ctx, image, file_urls)
            }),
    }
}
fn drain_clipboard_target(target: &ClipboardPasteTarget, app: &mut AppView) -> Vec<Effect> {
    match target {
        ClipboardPasteTarget::AgentPrompt { agent_id, .. } => {
            let is_active = app.active_view == ActiveView::Agent(*agent_id);
            let Some(agent) = app.agents.get_mut(agent_id) else {
                return vec![];
            };
            let resend = agent.take_deferred_send_after_paste();
            let action = if is_active {
                resend.and_then(|kind| agent.build_deferred_send_action(kind))
            } else {
                None
            };
            let mut effects = std::mem::take(&mut agent.pending_effects);
            if let Some(action) = action {
                effects.extend(dispatch(action, app));
            }
            effects
        }
        ClipboardPasteTarget::DashboardDispatch | ClipboardPasteTarget::DashboardPeek { .. } => {
            let Some(dashboard) = app.dashboard.as_mut() else {
                return vec![];
            };
            let resends = dashboard.take_deferred_sends_after_paste();
            let mut effects = std::mem::take(&mut dashboard.pending_effects);
            if matches!(app.active_view, ActiveView::AgentDashboard) {
                for action in resends {
                    effects.extend(dispatch(action, app));
                }
            }
            effects
        }
    }
}
pub(crate) fn current_doctor_target(
    app: &AppView,
    target: &DoctorFixTarget,
) -> Option<DoctorFixTarget> {
    let agent = app.agents.get(&target.agent_id)?;
    if agent.session.cwd != target.cwd {
        return None;
    }
    match (&target.session_id, &agent.session.session_id) {
        (Some(expected), Some(current))
            if expected == current
                && target.session_binding_epoch == agent.session_binding_epoch =>
        {
            Some(target.clone())
        }
        (None, Some(current))
            if agent.session_binding_epoch == target.session_binding_epoch.wrapping_add(1) =>
        {
            Some(DoctorFixTarget {
                session_id: Some(current.clone()),
                session_binding_epoch: agent.session_binding_epoch,
                ..target.clone()
            })
        }
        (None, None) if target.session_binding_epoch == agent.session_binding_epoch => {
            Some(target.clone())
        }
        _ => None,
    }
}
pub(crate) fn doctor_target_is_current(app: &AppView, target: &DoctorFixTarget) -> bool {
    app.agents.get(&target.agent_id).is_some_and(|agent| {
        agent.session.session_id == target.session_id
            && agent.session_binding_epoch == target.session_binding_epoch
            && agent.session.cwd == target.cwd
    })
}
pub(crate) fn deliver_doctor_message(app: &mut AppView, preferred: AgentId, message: String) {
    let destination = app
        .agents
        .contains_key(&preferred)
        .then_some(preferred)
        .or_else(|| match app.active_view {
            ActiveView::Agent(id) if app.agents.contains_key(&id) => Some(id),
            _ => app.agents.keys().next().copied(),
        });
    if let Some(destination) = destination
        && let Some(agent) = app.agents.get_mut(&destination)
    {
        agent.scrollback.push_block(RenderBlock::system(message));
        return;
    }
    app.startup_warnings.push(crate::startup::StartupWarning {
        severity: crate::startup::WarningSeverity::Info,
        message,
        action: None,
    });
}
/// Overlay catalog ChatGPT windows onto the open `/providers` OpenAI row
/// after a list snapshot or Refresh follow-up.
///
/// List snapshots do not carry subscription models. Refresh fills them and
/// applies persisted auto-compact thresholds; `apply_list_snapshot`
/// preserves email/models/thresholds, then this reapplies `app.models`
/// catalog windows so a reload cannot drop the listed context-window state.
fn overlay_chatgpt_catalog_on_providers(
    state: &mut crate::views::providers_modal::ProviderModalState,
    catalog: &crate::acp::model_state::ModelState,
) {
    if let Some(status) = state.status_mut(&crate::views::providers_modal::ProviderKind::OpenAi) {
        status.overlay_chatgpt_windows(|id| catalog.context_window_tokens_for(id));
    }
}

/// Handle a completed async task result.
pub(super) fn dispatch_task_result(result: TaskResult, app: &mut AppView) -> Vec<Effect> {
    match result {
        TaskResult::SessionCreated {
            agent_id,
            session_id,
            models: new_models,
        } => handle_session_created(app, agent_id, session_id, new_models),
        TaskResult::SessionFailed { agent_id, error } => {
            handle_session_failed(app, agent_id, error)
        }
        TaskResult::WorktreeSessionCreated {
            agent_id,
            session_id,
            worktree_path,
            session_cwd,
            models: new_models,
        } => handle_worktree_session_created(
            app,
            agent_id,
            session_id,
            worktree_path,
            session_cwd,
            new_models,
        ),
        TaskResult::WorktreeForked {
            agent_id,
            session_id,
            worktree_path,
            session_cwd,
            code_restored,
            restore_summary,
            restore_degree,
        } => handle_worktree_forked(
            app,
            agent_id,
            session_id,
            worktree_path,
            session_cwd,
            code_restored,
            restore_summary,
            restore_degree,
        ),
        TaskResult::WorktreeSessionFailed { agent_id, error } => {
            handle_worktree_session_failed(app, agent_id, error)
        }
        TaskResult::ForkSessionReady {
            agent_id,
            new_session_id,
            cwd,
        } => handle_fork_session_ready(app, agent_id, new_session_id, cwd),
        TaskResult::ForkSessionFailed { agent_id, error } => {
            handle_fork_session_failed(app, agent_id, error)
        }
        TaskResult::BillingFetched {
            agent_id,
            balance,
            silent,
            subscription_tier,
            autotopup,
        } => handle_billing_fetched(app, agent_id, balance, silent, subscription_tier, autotopup),
        TaskResult::BillingError {
            agent_id,
            error,
            silent,
        } => {
            if !silent && let Some(agent) = app.agents.get_mut(&agent_id) {
                agent.scrollback.push_block(RenderBlock::System(
                    crate::scrollback::blocks::SystemMessageBlock::new(format!(
                        "Billing error: {error}"
                    )),
                ));
            }
            vec![]
        }
        TaskResult::AppBillingFetched { balance, autotopup } => {
            app.credit_balance = balance;
            apply_auto_topup(&mut app.auto_topup, &autotopup);
            vec![]
        }
        TaskResult::GateRefreshed { settings } => handle_gate_refreshed(app, settings),
        TaskResult::SessionLoaded {
            agent_id,
            session_id,
            models: new_models,
            code_restored,
            restore_summary,
            restore_degree,
            running_prompt_id,
        } => handle_session_loaded(
            app,
            agent_id,
            session_id,
            new_models,
            code_restored,
            restore_summary,
            restore_degree,
            running_prompt_id,
        ),
        TaskResult::SessionTitleFromDisk { agent_id, title } => {
            if let Some(agent) = app.agents.get_mut(&agent_id)
                && let Some((t, is_manual)) = title.filter(|(s, _)| !s.trim().is_empty())
            {
                if is_manual && agent.display_name.is_none() {
                    agent.display_name = Some(t.clone());
                }
                agent.generated_session_title = Some(t);
            }
            vec![]
        }
        TaskResult::SessionLoadFailed {
            agent_id,
            session_id,
            error,
        } => handle_session_load_failed(app, agent_id, session_id, error),
        TaskResult::SessionListLoaded {
            sessions,
            partial,
            scope,
            seq,
            query,
        } => handle_session_list_loaded(app, sessions, partial, scope, seq, query),
        TaskResult::ForeignSessionsScanned { entries, seq } => {
            handle_foreign_sessions_scanned(app, entries, seq)
        }
        TaskResult::ForeignResumeCwdCanonicalized {
            requested_cwd,
            canonical_cwd,
            launch_token,
        } => {
            let accepted_cwd = canonical_cwd.clone();
            if app.accept_foreign_resume_canonical_cwd(launch_token, &requested_cwd, canonical_cwd)
                && let Some(canonical_cwd) = accepted_cwd
            {
                vec![Effect::DetectForeignResumeHint {
                    canonical_cwd,
                    compat: app.foreign_session_compat,
                    grok_home: xai_grok_tools::util::grok_home::grok_home(),
                    launch_token,
                }]
            } else {
                vec![]
            }
        }
        TaskResult::ForeignResumeHintDetected {
            canonical_cwd,
            launch_token,
            hint,
        } => {
            app.apply_foreign_resume_detection(launch_token, &canonical_cwd, hint);
            vec![]
        }
        TaskResult::SessionListFailed { error, seq, query } => {
            handle_session_list_failed(app, error, seq, query)
        }
        TaskResult::SessionSearchDebounceExpired { query, seq } => {
            handle_session_search_debounce_expired(app, query, seq)
        }
        TaskResult::RosterLoaded { sessions } => {
            app.leader_roster = sessions;
            app.dashboard_sessions_loading = false;
            vec![]
        }
        TaskResult::RosterFailed { error } => {
            tracing::debug!(error = %error, "leader roster fetch failed");
            app.dashboard_sessions_loading = false;
            vec![]
        }
        TaskResult::DashboardSessionsLoaded { sessions } => {
            app.dashboard_local_sessions = sessions;
            app.dashboard_sessions_loading = false;
            vec![]
        }
        TaskResult::CardDetailLoaded {
            source,
            session_id,
            generation,
            detail,
        } => handle_card_detail_loaded(app, source, session_id, generation, detail),
        TaskResult::SessionRestored {
            agent_id,
            local_session_id,
        } => handle_session_restored(app, agent_id, local_session_id),
        TaskResult::SessionRestoreFailed { agent_id, error } => {
            handle_session_restore_failed(app, agent_id, error)
        }
        TaskResult::SessionRestoreProgress { agent_id, message } => {
            if let Some(agent) = app.agents.get_mut(&agent_id)
                && !defer_to_open_reload_window(agent, agent_id, "SessionRestoreProgress")
            {
                agent.scrollback.push_block(RenderBlock::system(message));
            }
            vec![]
        }
        TaskResult::PromptResponse {
            agent_id,
            result,
            http_status,
            prompt_id,
        } => handle_prompt_response(app, agent_id, result, http_status, prompt_id),
        TaskResult::SendPromptNowFailed {
            agent_id,
            session_id,
            prompt_id,
            error,
            blocks,
        } => {
            let sid = session_id.0.to_string();
            super::queue::retire_optimistic_echo(
                &mut app.optimistic_prompt_echoes,
                &mut app.shared_prompt_queues,
                &sid,
                &prompt_id,
            );
            if let Some(agent) = app.agents.get_mut(&agent_id) {
                agent.shared_queue.retain(|e| e.id != prompt_id);
                agent.note_queue_echo_retired(&prompt_id);
                if agent.expect_send_now_cancel.as_deref() == Some(prompt_id.as_str())
                    || agent.follow_without_jump_prompt_id.as_deref() == Some(prompt_id.as_str())
                {
                    agent.clear_send_now_expectation();
                }
                agent.retire_send_now_painted_block(&prompt_id);
                let text = blocks
                    .iter()
                    .find_map(|b| match b {
                        acp::ContentBlock::Text(t) => Some(t.text.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                let id = agent.session.next_queue_id;
                agent.session.next_queue_id += 1;
                agent
                    .session
                    .pending_prompts
                    .push_front(crate::app::agent::QueuedPrompt {
                        wire_blocks: Some(blocks),
                        ..crate::app::agent::QueuedPrompt::plain(
                            id,
                            &text,
                            crate::app::agent::QueueEntryKind::Prompt,
                        )
                    });
                agent.show_toast(&format!("Send now failed — requeued: {error}"));
            }
            vec![]
        }
        TaskResult::PreferredModelPersisted { result } => {
            if let Err(err) = result
                && let Some(agent) = get_active_agent_mut(app)
            {
                agent.scrollback.push_block(RenderBlock::system(format!(
                    "Couldn't save preferred model: {err} (still active for this session)"
                )));
            }
            vec![]
        }
        TaskResult::CancelComplete => {
            tracing::trace!("Cancel notification sent successfully");
            vec![]
        }
        TaskResult::KillSubagentComplete {
            session_id,
            subagent_id,
            outcome,
        } => {
            if let SubagentKillOutcome::NothingLive { status } = outcome {
                let status = status.as_deref().unwrap_or("cancelled");
                crate::app::acp_handler::finalize_killed_subagent(
                    app,
                    &session_id,
                    &subagent_id,
                    status,
                );
            }
            vec![]
        }
        TaskResult::CompactComplete { agent_id, result } => {
            handle_compact_complete(app, agent_id, result)
        }
        TaskResult::SwitchModelComplete {
            agent_id,
            model_id,
            effort,
            result,
            prev_model_id,
        } => handle_switch_model_complete(app, agent_id, model_id, effort, result, prev_model_id),
        TaskResult::BgTaskKilled {
            session_id,
            task_id,
            outcome,
        } => handle_bg_task_killed(app, session_id, task_id, outcome),
        TaskResult::BgTaskKillFailed {
            session_id,
            task_id,
            error,
        } => {
            tracing::warn!(task_id = %task_id, error = %error, "Failed to kill bg task");
            if let Some(agent) = find_agent_by_session_id(&mut app.agents, &session_id)
                && let Some(task) = agent.session.bg_tasks.get_mut(&task_id)
            {
                task.pending_kill = false;
                task.kill_requested_at = None;
            }
            vec![]
        }
        TaskResult::ChangelogFetched { markdown, entries } => {
            app.changelog_markdown = markdown;
            app.changelog_bullets =
                xai_grok_shell::util::changelog::bullets_from_entries(&entries, 3);
            vec![]
        }
        TaskResult::ClipboardAttachmentProbed {
            ctx,
            image,
            file_urls,
        } => {
            let is_clipboard_key = ctx.source.is_clipboard_key();
            let primary_hint_eligible = is_clipboard_key
                && !app.screen_mode.is_minimal()
                && crate::clipboard::x11_primary_guidance_available();
            let target = ctx.target.clone();
            let wrap_text = if is_clipboard_key {
                ctx.source.text().map(str::to_owned)
            } else {
                None
            };
            let completion = apply_clipboard_paste_result(ctx, image, file_urls, app);
            let wrap_request_emitted = wrap_host_image_request_eligible(completion)
                && is_clipboard_key
                && crate::wrap_clipboard_image::maybe_request_wrap_host_image(
                    None,
                    wrap_text.as_deref(),
                    None,
                );
            let effects = drain_clipboard_target(&target, app);
            maybe_show_x11_primary_paste_hint(
                primary_hint_eligible && !wrap_request_emitted,
                completion,
                &target,
                app,
            );
            if let ClipboardPasteCompletion::Failed(failure) = completion
                && !wrap_request_emitted
            {
                show_clipboard_failure(&target, failure, app);
            }
            effects
        }
        TaskResult::PromptImagePreviewPrepared => vec![],
        TaskResult::DoctorFixPlanned { target, result } => {
            let Some(target) = current_doctor_target(app, &target) else {
                deliver_doctor_message(
                    app,
                    target.agent_id,
                    "This fix was cancelled because the session changed. Run `/doctor fix` again."
                        .to_owned(),
                );
                return vec![];
            };
            match result {
                Ok(DoctorPlanningOutcome::Listing(listing)) => {
                    deliver_doctor_message(app, target.agent_id, listing);
                }
                Ok(DoctorPlanningOutcome::Plan(plan)) => {
                    super::prompt::open_doctor_fix_question(app, target, plan);
                }
                Ok(DoctorPlanningOutcome::RunLocally(command)) => {
                    deliver_doctor_message(
                        app,
                        target.agent_id,
                        format!(
                            "This fix configures your local computer, not this SSH session.\nOn your local computer, run: {command}"
                        ),
                    );
                }
                Err(error) => deliver_doctor_message(
                    app,
                    target.agent_id,
                    if error.starts_with("Could not prepare the fix:") {
                        error
                    } else {
                        format!("Could not prepare the fix: {error}")
                    },
                ),
            }
            vec![]
        }
        TaskResult::DoctorFixApplied {
            target,
            shell,
            result,
        } => {
            let message = match result {
                Ok(outcome) => {
                    let report_agent = doctor_target_is_current(app, &target)
                        .then_some(target.agent_id)
                        .or_else(|| match app.active_view {
                            ActiveView::Agent(id) if app.agents.contains_key(&id) => Some(id),
                            _ => app.agents.keys().next().copied(),
                        });
                    let Some(report_agent) = report_agent else {
                        let message = match outcome.status {
                            crate::diagnostics::FixStatus::Applied => {
                                format!(
                                    "Set up SSH wrapping in {}.",
                                    outcome.changed_path.display()
                                )
                            }
                            crate::diagnostics::FixStatus::AlreadyConfigured => {
                                format!(
                                    "SSH wrapping is already set up in {}.",
                                    outcome.changed_path.display()
                                )
                            }
                        };
                        deliver_doctor_message(app, target.agent_id, message);
                        return vec![];
                    };
                    let Some(mut report) =
                        super::prompt::collect_live_doctor_report(app, report_agent)
                    else {
                        unreachable!("report destination came from app.agents")
                    };
                    report = crate::diagnostics::configured_report(
                        report,
                        crate::diagnostics::managed_alias_configured(&outcome.changed_path, shell),
                    );
                    if report
                        .findings
                        .iter()
                        .any(|finding| finding.id == outcome.id)
                    {
                        format!(
                            "The change was applied, but Doctor still reports `{}`.",
                            outcome.id
                        )
                    } else {
                        let status = match outcome.status {
                            crate::diagnostics::FixStatus::Applied => {
                                format!(
                                    "Set up SSH wrapping in {}.",
                                    outcome.changed_path.display()
                                )
                            }
                            crate::diagnostics::FixStatus::AlreadyConfigured => {
                                format!(
                                    "SSH wrapping is already set up in {}.",
                                    outcome.changed_path.display()
                                )
                            }
                        };
                        let backup = outcome
                            .backup_path
                            .as_ref()
                            .map(|path| format!("\nBackup: {}", path.display()))
                            .unwrap_or_default();
                        format!(
                            "{status}{backup}\nStart a new shell to use the alias.\n\n{}",
                            crate::diagnostics::format_doctor(&report)
                        )
                    }
                }
                Err(error) if error.starts_with("Could not apply the fix:") => error,
                Err(error) => format!("Could not apply the fix: {error}"),
            };
            deliver_doctor_message(app, target.agent_id, message);
            vec![]
        }
        TaskResult::AnnouncementsHiddenPersisted { result } => {
            if let Err(e) = result {
                tracing::warn!("Failed to persist announcements hidden state: {}", e);
            }
            vec![]
        }
        TaskResult::PromptHistoryLoaded { agent_id, prompts } => {
            use xai_grok_tools::implementations::skills::skill::extract_skill_display_text;
            if let Some(agent) = app.agents.get_mut(&agent_id) {
                agent.session.prompt_history_loading = false;
                agent.session.prompt_history = prompts
                    .into_iter()
                    .map(|p| extract_skill_display_text(&p).unwrap_or(p))
                    .collect();
                if agent.prompt.history_search.is_active() {
                    let history = agent.combined_prompt_history();
                    agent.prompt.history_search.refresh_items(&history);
                    if !agent.prompt.history_search.is_browse() {
                        let query = agent.prompt.text().to_owned();
                        agent.prompt.history_search.update_query(&query);
                    }
                }
            }
            vec![]
        }
        TaskResult::AuthComplete {
            request_seq,
            meta,
            repair,
            credential_write_receipt,
        } => handle_auth_complete(app, request_seq, meta, repair, credential_write_receipt),
        TaskResult::AuthFailed { request_seq, error } => {
            if let AuthState::Authenticating {
                request_seq: current_seq,
                ..
            } = &app.auth_state
                && *current_seq == request_seq
            {
                app.auth_state = AuthState::Pending { error: Some(error) };
                app.auth_code_input.reset();
            }
            vec![]
        }
        TaskResult::AuthUrlReady {
            request_seq,
            auth_url,
            external,
            mode,
        } => handle_auth_url_ready(app, request_seq, auth_url, external, mode),
        TaskResult::AuthCodeSubmitted { .. } => vec![],
        TaskResult::AuthCancelComplete => vec![],
        TaskResult::McpsListLoaded { agent_id, result } => {
            use crate::views::extensions_modal::TabDataState;
            if let Some(agent) = app.agents.get_mut(&agent_id)
                && let Some(ref mut modal) = agent.extensions_modal
            {
                modal.pending_action = None;
                modal.pending_entry_index = None;
                modal.mcps_data = match result {
                    Ok(response) => TabDataState::Loaded(response),
                    Err(e) => TabDataState::Error(e),
                };
            }
            vec![]
        }
        TaskResult::McpAuthTriggerDone {
            agent_id,
            server_name,
            result,
        } => handle_mcp_auth_trigger_done(app, agent_id, server_name, result),
        TaskResult::McpSetupSubmitDone {
            agent_id,
            server_name,
            result,
        } => handle_mcp_setup_submit_done(app, agent_id, server_name, result),
        TaskResult::HooksListLoaded { agent_id, result } => {
            handle_hooks_list_loaded(app, agent_id, result)
        }
        TaskResult::PluginsListLoaded { agent_id, result } => {
            handle_plugins_list_loaded(app, agent_id, result)
        }
        TaskResult::HooksActionResult { agent_id, result }
        | TaskResult::PluginsActionResult { agent_id, result }
        | TaskResult::MarketplaceActionResult { agent_id, result } => {
            dispatch_action_result(app, agent_id, result)
        }
        TaskResult::CtaPluginInstallDone {
            agent_id,
            plugin_name,
            result,
        } => handle_cta_plugin_install_done(app, agent_id, plugin_name, result),
        TaskResult::CtaPluginReloadDone {
            agent_id,
            plugin_name,
            result,
        } => handle_cta_plugin_reload_done(app, agent_id, plugin_name, result),
        TaskResult::PluginCtaMcpsLoaded {
            agent_id,
            plugin_name,
            result,
        } => handle_plugin_cta_mcps_loaded(app, agent_id, plugin_name, result),
        TaskResult::CtaInstalledDismissTimeout {
            agent_id,
            plugin_name,
        } => {
            use crate::app::agent_view::CtaPhase;
            if let Some(agent) = app.agents.get_mut(&agent_id)
                && let CtaPhase::Installed { name } = &agent.plugin_cta.phase
                && *name == plugin_name
            {
                agent.plugin_cta.phase = CtaPhase::Hidden;
            }
            vec![]
        }
        TaskResult::McpToggleDone { agent_id, result } => {
            handle_mcp_toggle_done(app, agent_id, result)
        }
        TaskResult::MarketplaceUpdatesAvailable { agent_id, updates } => {
            handle_marketplace_updates_available(app, agent_id, updates)
        }
        TaskResult::MarketplaceListLoaded { agent_id, result } => {
            handle_marketplace_list_loaded(app, agent_id, result)
        }
        TaskResult::PluginCtaCatalogLoaded { agent_id, result } => {
            handle_plugin_cta_catalog_loaded(app, agent_id, result)
        }
        TaskResult::SkillsListLoaded { agent_id, result } => {
            use crate::views::extensions_modal::TabDataState;
            let mut effects = Vec::new();
            if let Some(agent) = app.agents.get_mut(&agent_id) {
                let in_flight = agent.skill_regress_in_flight.is_some();
                let session_id = agent.session.session_id.clone();
                let mut search_query = None;
                if let Some(ref mut modal) = agent.extensions_modal {
                    modal.skills_smart_rank = None;
                    modal.skills_data = match result {
                        Ok(skills) => TabDataState::Loaded(skills),
                        Err(e) => TabDataState::Error(e),
                    };
                    if in_flight || modal.pending_action.as_deref() == Some("regressing...") {
                        if modal.active_tab == crate::views::extensions_modal::ExtensionsTab::Skills
                        {
                            modal.pending_action = Some("regressing...".into());
                        }
                    } else {
                        modal.pending_action = None;
                        modal.pending_entry_index = None;
                    }
                    if matches!(modal.skills_data, TabDataState::Loaded(_)) {
                        search_query = modal.skills_smart_search_query().map(str::to_owned);
                    }
                }
                // Always bump when inventory is applied so a stale same-query
                // completion cannot match after reload, including off-tab
                // ListLoaded and a missing session_id. Issue at most one
                // fetch with that generation when Skills+Smart+query+session
                // are all live.
                if agent.extensions_modal.is_some() {
                    let r#gen = agent.bump_skills_smart_search_gen();
                    if let (Some(session_id), Some(query)) = (session_id, search_query) {
                        effects.push(Effect::FetchSkillsSearch {
                            agent_id,
                            session_id,
                            query,
                            r#gen,
                        });
                    }
                }
            }
            effects
        }
        TaskResult::SkillsSearchLoaded {
            agent_id,
            session_id,
            query,
            r#gen,
            result,
        } => {
            if let Some(agent) = app.agents.get_mut(&agent_id)
                && agent.session.session_id.as_ref() == Some(&session_id)
            {
                let current_gen = agent.skills_smart_search_gen;
                if let Some(ref mut modal) = agent.extensions_modal {
                    let live = modal.skills_smart_search_query().map(str::to_owned);
                    if r#gen != current_gen || live.as_deref() != Some(query.as_str()) {
                        // Stale generation, query change, mode change, empty
                        // query, or a later occupancy: do not install a rank,
                        // change selection, or trigger another side effect.
                    } else {
                        modal.skills_smart_rank = match result {
                            Ok((names, false)) => Some(names),
                            Ok((_, true)) | Err(_) => None,
                        };
                    }
                }
            }
            vec![]
        }
        TaskResult::WorkflowsListLoaded {
            agent_id,
            session_id,
            result,
        } => {
            use crate::views::extensions_modal::TabDataState;
            if let Some(agent) = app.agents.get_mut(&agent_id)
                && agent.session.session_id.as_ref() == Some(&session_id)
                && let Some(ref mut modal) = agent.extensions_modal
            {
                modal.workflows_data = match result {
                    Ok(workflows) => TabDataState::Loaded(workflows),
                    Err(e) => TabDataState::Error(e),
                };
            }
            vec![]
        }
        TaskResult::SkillsToggleDone { agent_id, result } => {
            handle_skills_toggle_done(app, agent_id, result)
        }
        TaskResult::SkillPublishDone { agent_id, result } => {
            use crate::views::extensions_modal::{ModalMessage, TabDataState};
            let mut effects = Vec::new();
            if let Some(agent) = app.agents.get_mut(&agent_id) {
                let session_id = agent.session.session_id.clone();
                if let Some(ref mut modal) = agent.extensions_modal {
                    modal.pending_action = None;
                    match result {
                        Ok(_) => {
                            modal.skills_data = TabDataState::Loading;
                            if let Some(session_id) = session_id {
                                effects.push(Effect::FetchSkillsList {
                                    agent_id,
                                    session_id,
                                });
                            }
                        }
                        Err(e) => {
                            modal.modal_message = Some(ModalMessage::Error(e));
                        }
                    }
                }
            }
            effects
        }
        TaskResult::SkillRegressDone { agent_id, result } => {
            use crate::views::extensions_modal::{ModalMessage, TabDataState};
            let mut effects = Vec::new();
            if let Some(agent) = app.agents.get_mut(&agent_id) {
                agent.skill_regress_in_flight = None;
                let session_id = agent.session.session_id.clone();
                if let Some(ref mut modal) = agent.extensions_modal {
                    modal.pending_action = None;
                    match result {
                        Ok(_) => {
                            modal.skills_data = TabDataState::Loading;
                            if let Some(session_id) = session_id {
                                effects.push(Effect::FetchSkillsList {
                                    agent_id,
                                    session_id,
                                });
                            }
                        }
                        Err(e) => {
                            modal.modal_message = Some(ModalMessage::Error(e));
                        }
                    }
                }
            }
            effects
        }
        TaskResult::PrimeIndexStatusLoaded { agent_id, result } => {
            apply_prime_index_status(app, agent_id, result);
            vec![]
        }
        TaskResult::PrimeIndexJobLoaded {
            agent_id,
            result,
            kind,
            collection,
        } => apply_prime_index_job(app, agent_id, result, &kind, &collection),
        TaskResult::SkillRegressCancelled { agent_id, result } => {
            use crate::views::extensions_modal::ModalMessage;
            if let Some(agent) = app.agents.get_mut(&agent_id)
                && let Some(ref mut modal) = agent.extensions_modal
            {
                if let Err(e) = result {
                    modal.modal_message = Some(ModalMessage::Error(e));
                }
            }
            vec![]
        }
        TaskResult::ShareSessionComplete {
            agent_id,
            share_url,
        } => {
            if let Some(agent) = app.agents.get_mut(&agent_id) {
                agent
                    .scrollback
                    .push_block(crate::scrollback::block::RenderBlock::system(format!(
                        "Session shared: {share_url}"
                    )));
            }
            vec![]
        }
        TaskResult::ShareSessionFailed { agent_id, error } => {
            if let Some(agent) = app.agents.get_mut(&agent_id) {
                agent
                    .scrollback
                    .push_block(crate::scrollback::block::RenderBlock::system(format!(
                        "Couldn't share session: {error}"
                    )));
            }
            vec![]
        }
        TaskResult::SessionAgentNameResolved {
            agent_id,
            agent_name,
        } => {
            if let Some(agent) = app.agents.get_mut(&agent_id) {
                agent.session_agent_name = agent_name.clone();
                if let Some(modal) = agent.agents_modal.as_mut() {
                    modal.active_agent = agent_name;
                }
            }
            vec![]
        }
        TaskResult::SessionInfoComplete {
            agent_id,
            info,
            text,
        } => {
            if let Some(agent) = app.agents.get_mut(&agent_id) {
                agent.session_agent_name = info.data.agent_name.clone();
                if let Some(modal) = agent.agents_modal.as_mut() {
                    modal.active_agent = info.data.agent_name.clone();
                }
                agent.apply_full_context_info(info.data.context);
                agent
                    .scrollback
                    .push_block(crate::scrollback::block::RenderBlock::system(text));
            }
            vec![]
        }
        TaskResult::SessionInfoFailed { agent_id, error } => {
            if let Some(agent) = app.agents.get_mut(&agent_id) {
                agent
                    .scrollback
                    .push_block(crate::scrollback::block::RenderBlock::system(format!(
                        "Couldn't load session info: {error}"
                    )));
            }
            vec![]
        }
        TaskResult::CodingDataSharingUpdated { agent_id, opted_in } => {
            handle_coding_data_sharing_updated(app, agent_id, opted_in)
        }
        TaskResult::CodingDataSharingFailed {
            agent_id,
            error,
            rollback_to_opted_in,
        } => handle_coding_data_sharing_failed(app, agent_id, error, rollback_to_opted_in),
        TaskResult::RenameSessionComplete { agent_id, title } => {
            if let Some(agent) = app.agents.get_mut(&agent_id) {
                let safe = crate::views::session_title::sanitize_display_text(&title);
                agent
                    .scrollback
                    .push_block(crate::scrollback::block::RenderBlock::system(format!(
                        "Session renamed to \"{safe}\""
                    )));
            }
            vec![]
        }
        TaskResult::RenameSessionFailed { agent_id, error } => {
            if let Some(agent) = app.agents.get_mut(&agent_id) {
                agent
                    .scrollback
                    .push_block(crate::scrollback::block::RenderBlock::system(format!(
                        "Couldn't rename session: {error}"
                    )));
            }
            vec![]
        }
        TaskResult::DeleteSessionComplete { source, session_id } => {
            remove_session_from_pickers(app, &source, &session_id);
            app.show_toast("Session deleted");
            vec![]
        }
        TaskResult::DeleteSessionFailed {
            source,
            session_id,
            error,
        } => {
            tracing::warn!(source, session_id = %session_id, error = %error, "session delete failed");
            app.show_toast(&format!("Couldn't delete session: {error}"));
            vec![]
        }
        TaskResult::ContextInfoComplete { agent_id, info } => {
            handle_context_info_complete(app, agent_id, info)
        }
        TaskResult::ContextInfoFailed { agent_id, error } => {
            if let Some(agent) = app.agents.get_mut(&agent_id) {
                agent
                    .scrollback
                    .push_block(crate::scrollback::block::RenderBlock::system(format!(
                        "Couldn't load context info: {error}"
                    )));
            }
            vec![]
        }
        TaskResult::SessionUsageComplete {
            agent_id,
            session_id,
            usage,
        } => commit_session_usage_block(
            app,
            agent_id,
            &session_id,
            crate::app::status_blocks::session_usage_block_text(&usage),
        ),
        TaskResult::SessionUsageFailed {
            agent_id,
            session_id,
            error,
        } => commit_session_usage_block(
            app,
            agent_id,
            &session_id,
            format!("Couldn't load session usage: {error}"),
        ),
        TaskResult::FeedbackComplete { .. } => vec![],
        TaskResult::FeedbackFailed { agent_id, error } => {
            if let Some(agent) = app.agents.get_mut(&agent_id) {
                agent
                    .scrollback
                    .push_block(crate::scrollback::block::RenderBlock::system(format!(
                        "Couldn't send feedback: {error}"
                    )));
            }
            vec![]
        }
        TaskResult::MemoryNoteSaved { agent_id, result } => {
            handle_memory_note_saved(app, agent_id, result)
        }
        TaskResult::MemoryNoteRewritten {
            agent_id,
            result,
            nonce,
        } => {
            if let Some(agent) = app.agents.get_mut(&agent_id)
                && let Ok(markdown) = result
                && let Some(crate::views::modal::ActiveModal::RememberNoteReview {
                    ref mut enhanced_content,
                    ref mut cached_lines,
                    rewrite_nonce,
                    ..
                }) = agent.active_modal
                && rewrite_nonce == nonce
            {
                *enhanced_content = Some(markdown);
                *cached_lines = None;
            }
            vec![]
        }
        TaskResult::BundleStatusReady {
            has_cache,
            version,
            personas,
            roles,
            agents,
            skills,
            persona_details,
            role_details,
        } => {
            app.bundle_state.has_cache = has_cache;
            app.bundle_state.version = version.unwrap_or_default();
            app.bundle_state.personas = personas;
            app.bundle_state.roles = roles;
            app.bundle_state.agents = agents;
            app.bundle_state.skills = skills;
            app.bundle_state.persona_details = persona_details;
            app.bundle_state.role_details = role_details;
            vec![]
        }
        TaskResult::BundleStatusFailed { error } => {
            tracing::warn!(error = %error, "bundle status fetch failed");
            vec![]
        }
        TaskResult::CatalogEntryReady {
            kind,
            name,
            content,
        } => {
            if let ActiveView::Agent(id) = app.active_view
                && let Some(agent) = app.agents.get_mut(&id)
            {
                let title = format!("{kind}: {name}");
                agent.block_viewer = Some(
                    crate::views::block_viewer::BlockViewerPane::for_plain_text(&title, &content),
                );
            }
            vec![]
        }
        TaskResult::CatalogEntryFailed { error } => {
            tracing::warn!(error = %error, "catalog entry fetch failed");
            if let ActiveView::Agent(id) = app.active_view
                && let Some(agent) = app.agents.get_mut(&id)
            {
                agent
                    .scrollback
                    .push_block(RenderBlock::system(format!("Couldn't load entry: {error}")));
            }
            vec![]
        }
        TaskResult::BtwResponse {
            agent_id,
            result,
            minimal_request_id,
        } => handle_btw_response(app, agent_id, result, minimal_request_id),
        TaskResult::InterjectQueued { .. } => vec![],
        TaskResult::RecapRequested {
            session_id,
            auto,
            error,
        } => {
            if let Some(error) = error {
                tracing::debug!(%error, "recap request failed");
                if !auto
                    && let Some(agent) = find_agent_by_session_id(&mut app.agents, &session_id.0)
                    && let Some(pending_id) = agent.pending_recap_entry.take()
                {
                    agent.scrollback.remove_entry(pending_id);
                    agent.show_toast(super::recap_unavailable_toast(
                        super::scrollback_has_user_messages(&agent.scrollback),
                    ));
                }
            }
            vec![]
        }
        TaskResult::InterjectFailed {
            agent_id,
            error,
            text,
            blocks,
        } => {
            if let Some(agent) = app.agents.get_mut(&agent_id) {
                let id = agent.session.next_queue_id;
                agent.session.next_queue_id += 1;
                agent
                    .session
                    .pending_prompts
                    .push_front(crate::app::agent::QueuedPrompt {
                        id,
                        text,
                        kind: crate::app::agent::QueueEntryKind::Prompt,
                        wire_blocks: blocks,
                        images: Vec::new(),
                        display_as_skill: false,
                        task_id: None,
                        human_schedule: None,
                        chip_elements: Vec::new(),
                        skill_token_ranges: Vec::new(),
                        combined_texts: Vec::new(),
                    });
                agent.show_toast(&format!("Interjection failed — requeued: {error}"));
            }
            vec![]
        }
        TaskResult::AvailableCommandsRefreshed { agent_id, commands } => {
            if !commands.is_empty()
                && let Some(agent) = app.agents.get_mut(&agent_id)
            {
                agent.session.available_commands = commands;
                agent.session.available_commands_generation += 1;
                super::super::acp_handler::refresh_workflow_run_capabilities(agent);
            }
            vec![]
        }
        TaskResult::AuthCopyFeedbackTimeout { generation } => {
            if generation == app.auth_clipboard_feedback_generation {
                app.auth_clipboard_delivery = None;
            }
            vec![]
        }
        TaskResult::PaywallCheckTick => {
            let timed_out = app
                .paywall_check_started
                .is_some_and(|t| t.elapsed() >= PAYWALL_AUTO_CHECK_TIMEOUT);
            if !app.has_access() && !timed_out {
                vec![
                    Effect::CheckSubscription { verify: None },
                    Effect::SchedulePaywallCheck,
                ]
            } else {
                vec![]
            }
        }
        TaskResult::CheckSubscriptionComplete { verify, meta } => {
            handle_check_subscription_complete(app, verify, meta)
        }
        TaskResult::GateVerifyTimeout { generation } => handle_gate_verify_timeout(app, generation),
        TaskResult::CreditLimitRecheckComplete { agent_id, meta } => {
            handle_credit_limit_recheck_complete(app, agent_id, meta)
        }
        TaskResult::LogoutComplete => {
            app.auth_state = AuthState::Pending { error: None };
            app.access_gate_shown_logged = false;
            app.announcement_cta_impressions_logged.clear();
            app.gate = None;
            app.pending_gate_verification = None;
            app.last_subscription_check_at = None;
            app.login_method_id = None;
            ensure_login_method(app);
            app.auth_clipboard_delivery = None;
            let effects = dispatch_exit_session(app);
            app.welcome_prompt_focused = false;
            effects
        }
        TaskResult::DeepSearchResults { results, seq } => {
            handle_deep_search_results(app, results, seq)
        }
        TaskResult::RewindPointsLoaded { agent_id, points } => {
            handle_rewind_points_loaded(app, agent_id, points)
        }
        TaskResult::RewindPointsFailed { agent_id, error } => {
            let Some(agent) = app.agents.get_mut(&agent_id) else {
                return vec![];
            };
            agent.rewind_state = None;
            app.show_toast(&format!("Undo failed: {error}"));
            vec![]
        }
        TaskResult::RewindPreviewComplete {
            agent_id,
            response,
            target_prompt_index,
            mode,
        } => handle_rewind_preview_complete(app, agent_id, response, target_prompt_index, mode),
        TaskResult::RewindPreviewFailed { agent_id, error } => {
            handle_rewind_preview_failed(app, agent_id, error)
        }
        TaskResult::RewindExecuteComplete { agent_id, response } => {
            dispatch_rewind_success(app, agent_id, response)
        }
        TaskResult::RewindExecuteFailed { agent_id, error } => {
            handle_rewind_execute_failed(app, agent_id, error)
        }
        TaskResult::SuggestionDebounceExpired {
            agent_id,
            generation,
        } => handle_suggestion_debounce_expired(app, agent_id, generation),
        TaskResult::PluginCtaDebounceExpired {
            agent_id,
            generation,
        } => handle_plugin_cta_debounce_expired(app, agent_id, generation),
        TaskResult::ShellSuggestionsLoaded {
            agent_id,
            response,
            request_text,
            request_cursor,
        } => {
            let Some(agent) = app.agents.get_mut(&agent_id) else {
                return vec![];
            };
            if agent.prompt_input_mode != crate::app::agent_view::PromptInputMode::Bash {
                return vec![];
            }
            let generation = response.generation;
            agent
                .prompt
                .suggestions
                .on_suggestions_loaded(response, &request_text, request_cursor);
            let text = agent.prompt.text().to_owned();
            agent.prompt.suggestions.set_last_request_text(&text);
            let mark = agent.pending_effects.len();
            if agent.prompt.suggestions.take_pending_tab(generation) {
                agent.shell_completion_tab();
            }
            agent.pending_effects.split_off(mark)
        }
        TaskResult::PromptSuggestionLoaded {
            agent_id,
            suggestion,
            generation,
        } => {
            if let Some(agent) = app.agents.get_mut(&agent_id) {
                agent
                    .prompt
                    .prompt_suggestion
                    .on_loaded(suggestion, generation);
                agent.refresh_prompt_suggestion_gate();
                agent.log_prompt_suggestion_shown_if_visible();
            }
            vec![]
        }
        TaskResult::SettingPersisted { key, value } => {
            tracing::trace!(target: "settings", ?key, ?value, "setting persisted");
            vec![]
        }
        TaskResult::SettingPersistFailed {
            key,
            rollback_value,
            error,
        } => {
            let rollback_effects = apply_setting_rollback(app, key, &rollback_value);
            tracing::warn!(target: "settings", ?key, ?rollback_value, %error, "setting persist failed; rolled back");
            let scrubbed = scrub_error_for_toast(&error);
            app.show_toast(&format!("\u{2717} Could not save {key}: {scrubbed}"));
            rollback_effects
        }
        TaskResult::SettingPersistFailedBestEffort { key, error } => {
            tracing::warn!(
                target: "settings",
                ?key, %error,
                "setting persist failed (best-effort); in-memory state stays at optimistic value",
            );
            let scrubbed = scrub_error_for_toast(&error);
            app.show_toast(&format!("\u{2717} Could not save {key}: {scrubbed}"));
            vec![]
        }
        TaskResult::ChatgptContextWindowSaved {
            agent_id,
            model_id,
            tokens,
            result,
        } => {
            match result {
                Ok(()) => {
                    if let Some(agent) = app.agents.get_mut(&agent_id) {
                        if let Some(crate::views::modal::ActiveModal::Providers { state }) =
                            agent.active_modal.as_mut()
                            && let Some(status) = state
                                .status_mut(&crate::views::providers_modal::ProviderKind::OpenAi)
                        {
                            status.apply_chatgpt_context_window(&model_id, tokens);
                        }
                        agent.scrollback.push_block(RenderBlock::system(
                            "ChatGPT context-window override saved. config.toml hot-reload applies it to subsequent turns and new sessions.",
                        ));
                        agent.show_toast("ChatGPT context-window override saved");
                    }
                }
                Err(error) => app.show_toast(&format!(
                    "Could not save ChatGPT context-window override: {error}"
                )),
            }
            vec![]
        }
        TaskResult::ChatgptAutoCompactThresholdSaved {
            agent_id,
            model_id,
            percent,
            result,
        } => {
            match result {
                Ok(()) => {
                    if let Some(agent) = app.agents.get_mut(&agent_id) {
                        if let Some(crate::views::modal::ActiveModal::Providers { state }) =
                            agent.active_modal.as_mut()
                            && let Some(status) = state
                                .status_mut(&crate::views::providers_modal::ProviderKind::OpenAi)
                        {
                            status.apply_chatgpt_auto_compact_threshold(&model_id, percent);
                        }
                        agent.scrollback.push_block(RenderBlock::system(
                            "ChatGPT auto-compact threshold saved. It applies from the next model switch and new sessions.",
                        ));
                        agent.show_toast("ChatGPT auto-compact threshold saved");
                    }
                }
                Err(error) => app.show_toast(&format!(
                    "Could not save ChatGPT auto-compact threshold: {error}"
                )),
            }
            vec![]
        }
        TaskResult::ProviderOperationComplete {
            agent_id,
            provider,
            status,
            claude_cli_status,
            repair,
            credential_write_receipt,
            management,
        } => {
            use super::auth::strip_trailing_auth_error_blocks;
            use super::queue::{maybe_drain_queue, note_peek_page_flip};
            use crate::app::actions::ProviderManagementResult;
            use crate::scrollback::block::RenderBlock;
            use crate::views::providers_modal::ProviderStatus;

            let fallback_error = match &status {
                ProviderStatus::Error(error) => Some(error.clone()),
                _ => None,
            };
            let connected = matches!(status, ProviderStatus::Connected { .. });
            let catalog_windows = app.models.clone();
            let mut follow_up: Vec<crate::app::actions::Effect> = Vec::new();
            let applied = app.agents.get_mut(&agent_id).is_some_and(|agent| {
                let Some(crate::views::modal::ActiveModal::Providers { state }) =
                    agent.active_modal.as_mut()
                else {
                    return false;
                };
                let mut status = status;
                status.overlay_chatgpt_windows(|id| catalog_windows.context_window_tokens_for(id));
                state.set_status(&provider, status);
                if let Some(cli_status) = claude_cli_status {
                    state.set_claude_cli_status(cli_status);
                }
                if let Some(mgmt) = management {
                    match mgmt {
                        ProviderManagementResult::List(snap) => {
                            state.apply_list_snapshot(&snap);
                            state.management_error = None;
                        }
                        ProviderManagementResult::Detail(detail) => {
                            // If editor already open for same id, reload in place (Issue 3).
                            if let Some(ed) = state.editor_mut() {
                                if ed.detail.id == detail.id {
                                    ed.reload_from_detail(detail);
                                } else {
                                    state.open_editor(detail);
                                }
                            } else {
                                state.open_editor(detail);
                            }
                        }
                        ProviderManagementResult::Mutation(result) => {
                            // Strict op-id correlation: exact match required when the
                            // result carries an operation id. No `None => accept` for
                            // historical/late results (PR13 Gate E).
                            let editor_matches = state.editor_mut().is_some_and(|ed| {
                                if ed.detail.id != result.id {
                                    return false;
                                }
                                mutation_operation_matches(
                                    result.operation_id.as_deref(),
                                    ed.pending_operation_id.as_deref(),
                                )
                            });
                            let list_matches = mutation_operation_matches(
                                result.operation_id.as_deref(),
                                state.pending_list_operation_id.as_deref(),
                            );
                            // Uncorrelated late/historical result: discard completely.
                            if !editor_matches && !list_matches {
                                // no-op
                            } else if result.ok {
                                state.list_generation = result.generation.get();
                                let partial = if result.partial_commit {
                                    " (reload required)"
                                } else {
                                    ""
                                };
                                state.management_message = Some(format!(
                                    "Saved `{}` (gen {}){partial}",
                                    result.id,
                                    result.generation.get()
                                ));
                                state.management_error = None;
                                if editor_matches {
                                    if let Some(ed) = state.editor_mut() {
                                        ed.conflict = None;
                                        ed.pending_operation_id = None;
                                    }
                                    follow_up.push(
                                        crate::app::actions::Effect::ProviderOperation {
                                            agent_id,
                                            operation: crate::app::actions::ProviderOperation::LoadEditorDetail {
                                                provider_id: result.id.clone(),
                                            },
                                            repair: None,
                                        },
                                    );
                                }
                                if list_matches {
                                    state.pending_list_operation_id = None;
                                }
                                follow_up.push(crate::app::actions::Effect::ProviderOperation {
                                    agent_id,
                                    operation:
                                        crate::app::actions::ProviderOperation::LoadListSnapshot,
                                    repair: None,
                                });
                            } else {
                                let msg = result
                                    .error
                                    .clone()
                                    .unwrap_or_else(|| "mutation failed".into());
                                let guidance = result.guidance.clone().unwrap_or_default();
                                let full = if guidance.is_empty() {
                                    msg
                                } else {
                                    format!("{msg} — {guidance}")
                                };
                                state.management_error = Some(full.clone());
                                if editor_matches {
                                    if let Some(ed) = state.editor_mut() {
                                        ed.error = Some(full);
                                        ed.pending_operation_id = None;
                                        if let Some(conflict) = result.conflict.clone() {
                                            if conflict.provider_id == ed.detail.id {
                                                ed.enter_conflict(conflict);
                                            }
                                        }
                                    }
                                }
                                if list_matches {
                                    state.pending_list_operation_id = None;
                                }
                            }
                        }
                        // Issue 5: ignore late results for wrong provider / older generation.
                        ProviderManagementResult::Status(snap) => {
                            if let Some(ed) = state.editor_mut() {
                                if management_result_is_fresh(
                                    &ed.detail.id,
                                    ed.detail.generation.get(),
                                    &snap.provider_id,
                                    snap.generation.get(),
                                ) {
                                    ed.status = Some(snap.clone());
                                    ed.message = Some(snap.label.clone());
                                    ed.error = snap.error.clone();
                                }
                            }
                        }
                        ProviderManagementResult::Catalog(snap) => {
                            if let Some(ed) = state.editor_mut() {
                                if management_result_is_fresh(
                                    &ed.detail.id,
                                    ed.detail.generation.get(),
                                    &snap.provider_id,
                                    snap.generation.get(),
                                ) {
                                    ed.catalog = Some(snap.clone());
                                    ed.message = Some("Catalog updated".into());
                                    ed.error = snap.error.clone();
                                }
                            }
                        }
                        ProviderManagementResult::Capabilities(snap) => {
                            if let Some(ed) = state.editor_mut() {
                                if management_result_is_fresh(
                                    &ed.detail.id,
                                    ed.detail.generation.get(),
                                    &snap.provider_id,
                                    snap.generation.get(),
                                ) {
                                    ed.capabilities = Some(snap.clone());
                                    ed.message = Some("Capabilities updated".into());
                                    ed.error = snap.error.clone();
                                }
                            }
                        }
                        ProviderManagementResult::Credits(snap) => {
                            if let Some(ed) = state.editor_mut() {
                                if management_result_is_fresh(
                                    &ed.detail.id,
                                    ed.detail.generation.get(),
                                    &snap.provider_id,
                                    snap.generation.get(),
                                ) {
                                    ed.credits = Some(snap.clone());
                                    ed.message = snap.summary.clone();
                                    ed.error = snap.error.clone();
                                }
                            }
                        }
                        ProviderManagementResult::References(snap) => {
                            if let Some(ed) = state.editor_mut() {
                                if management_result_is_fresh(
                                    &ed.detail.id,
                                    ed.detail.generation.get(),
                                    &snap.provider_id,
                                    snap.generation.get(),
                                ) {
                                    ed.references = Some(snap);
                                }
                            }
                        }
                        ProviderManagementResult::Error(err) => {
                            state.management_error = Some(err.clone());
                            if let Some(ed) = state.editor_mut() {
                                ed.error = Some(err);
                            }
                        }
                    }
                }
                overlay_chatgpt_catalog_on_providers(state, &catalog_windows);
                true
            });
            if !applied && let Some(error) = fallback_error {
                app.show_toast(&format!("Provider action failed: {error}"));
            }
            if !follow_up.is_empty() {
                return follow_up;
            }

            // Resume only when completion echoes the immutable repair scope
            // captured at op start. Refresh/Test without token, delayed prior
            // tokens, sibling providers, and duplicates never resubmit.
            let Some(repair_scope) = repair else {
                return vec![];
            };
            if !connected {
                return vec![];
            }
            let auth_home = app.auth_home();
            let mut retry_effects = Vec::new();
            let mut page_flips = Vec::new();
            for agent in app.agents.values_mut() {
                let Some(stashed) = agent.reauth_stashed_prompt.take() else {
                    if agent
                        .in_flight_repair
                        .as_ref()
                        .is_some_and(|f| f.token == repair_scope.token)
                    {
                        agent.in_flight_repair = None;
                    }
                    continue;
                };
                let live = super::auth::live_binding_gen_for_resume(&auth_home, &repair_scope);
                if repair_scope.allows_resume(agent.in_flight_repair.as_ref(), &stashed)
                    && repair_scope.validate_write_receipt(credential_write_receipt.as_ref(), live)
                {
                    agent.in_flight_repair = None;
                    strip_trailing_auth_error_blocks(agent);
                    agent.scrollback.push_block(RenderBlock::system(format!(
                        "Reconnected {}. Retrying\u{2026}",
                        provider.label()
                    )));
                    agent.session.enqueue_in_flight_prompt_front(stashed.prompt);
                    let drain = maybe_drain_queue(agent);
                    retry_effects.extend(drain.effects);
                    page_flips.push((agent.session.id, drain.page_flip_entry));
                } else {
                    agent.reauth_stashed_prompt = Some(stashed);
                }
            }
            for (id, page_flip_entry) in page_flips {
                note_peek_page_flip(app, id, page_flip_entry);
            }
            retry_effects
        }
        TaskResult::RetrievalOperationComplete { agent_id, result } => {
            use crate::app::actions::RetrievalManagementResult;
            use crate::views::modal::ActiveModal;

            let Some(agent) = app.agents.get_mut(&agent_id) else {
                return vec![];
            };
            let Some(ActiveModal::RetrievalSettings { state }) = agent.active_modal.as_mut() else {
                return vec![];
            };
            match result {
                RetrievalManagementResult::Snapshot(snap) => {
                    state.apply_snapshot(snap);
                }
                RetrievalManagementResult::Mutation(m) => {
                    let need_reload = m.ok && m.snapshot.is_none();
                    state.apply_mutation_result(m);
                    if need_reload {
                        return vec![crate::app::actions::Effect::RetrievalOperation {
                            agent_id,
                            operation: crate::app::actions::RetrievalOperation::LoadSnapshot,
                        }];
                    }
                }
                RetrievalManagementResult::Preview(p) => {
                    state.apply_preview(p);
                }
                RetrievalManagementResult::Error(e) => {
                    state.loading = false;
                    state.error = Some(e);
                }
            }
            vec![]
        }
    }
}

/// Accept async management snapshots only for the open editor provider and when
/// the result generation is not older than the editor's known generation.
fn management_result_is_fresh(
    editor_id: &str,
    editor_generation: u64,
    result_id: &str,
    result_generation: u64,
) -> bool {
    editor_id == result_id && result_generation >= editor_generation
}

/// Strict mutation operation correlation (PR13 Gate E).
///
/// Exact match of result op-id to pending draft epoch. No `None => accept`
/// for historical results: missing pending or missing result op discards.
pub(crate) fn mutation_operation_matches(
    result_operation_id: Option<&str>,
    pending_operation_id: Option<&str>,
) -> bool {
    match (result_operation_id, pending_operation_id) {
        (Some(op), Some(pending)) if pending == op => true,
        (Some(_), Some(_)) => false, // wrong incarnation / late op
        (Some(_), None) => false,    // pending cleared; discard historical
        (None, _) => false,          // modern mutations always carry op ids
    }
}

fn apply_prime_index_status(
    app: &mut crate::app::app_view::AppView,
    agent_id: crate::app::agent::AgentId,
    result: Result<xai_grok_shell::session::prime::PrimeIndexStatus, String>,
) {
    let Some(agent) = app.agents.get_mut(&agent_id) else {
        return;
    };
    match result {
        Ok(mut status) => {
            status.sanitize_secrets();
            let unchanged = status.unchanged;
            if let Some(ref mut modal) = agent.extensions_modal {
                if let Some(idx) = modal.selected_data_index()
                    && let crate::views::extensions_modal::TabDataState::Loaded(ref snap) =
                        modal.skills_data
                    && let Some(row) = snap.rows.get(idx)
                {
                    modal.skills_anchor_identity = Some(row.identity.clone());
                }
                // Always merge compact index state, including when
                // `unchanged=true` (generation matched but job/counts/readiness
                // still advance). Search/filter/selection live on the modal,
                // not on this DTO.
                if unchanged && let Some(existing) = modal.prime_index.as_mut() {
                    merge_live_prime_index_fields(existing, &status);
                } else {
                    modal.prime_index = Some(status.clone());
                }
                modal.prime_index_capable = true;
            }
            if let Some(ref mut agents_modal) = agent.agents_modal {
                agents_modal.prime_index = Some(status.clone());
                agents_modal.prime_index_capable = true;
            }
            if let Some(crate::views::modal::ActiveModal::RetrievalSettings { state }) =
                agent.active_modal.as_mut()
            {
                state.prime_index = Some(status);
            }
        }
        Err(e) if e == "unsupported" => {
            if let Some(ref mut modal) = agent.extensions_modal {
                modal.prime_index_capable = false;
            }
            if let Some(ref mut agents_modal) = agent.agents_modal {
                agents_modal.prime_index_capable = false;
            }
        }
        Err(e) => {
            if let Some(ref mut modal) = agent.extensions_modal {
                modal.modal_message = Some(crate::views::extensions_modal::ModalMessage::Error(e));
            }
        }
    }
}

fn merge_live_prime_index_fields(
    existing: &mut xai_grok_shell::session::prime::PrimeIndexStatus,
    incoming: &xai_grok_shell::session::prime::PrimeIndexStatus,
) {
    existing.job = incoming.job.clone();
    existing.skills = incoming.skills.clone();
    existing.agents = incoming.agents.clone();
    existing.configured_route = incoming.configured_route.clone();
    existing.capabilities = incoming.capabilities;
    if !incoming.fingerprint_short.is_empty() {
        existing.fingerprint_short = incoming.fingerprint_short.clone();
    }
}

fn prime_job_matches_displayed(
    displayed: Option<&xai_grok_shell::session::prime::PrimeIndexJobStatus>,
    incoming: &xai_grok_shell::session::prime::PrimeIndexJobStatus,
) -> bool {
    let Some(current) = displayed else {
        return true;
    };
    if current.job_id.is_empty() || incoming.job_id.is_empty() {
        return true;
    }
    let busy = matches!(current.state.as_str(), "running" | "cancelling");
    !(busy && current.job_id != incoming.job_id)
}

fn apply_prime_index_job(
    app: &mut crate::app::app_view::AppView,
    agent_id: crate::app::agent::AgentId,
    result: Result<xai_grok_shell::session::prime::PrimeIndexJobStatus, String>,
    inflight_kind: &str,
    inflight_collection: &str,
) -> Vec<crate::app::actions::Effect> {
    let mut fetch = None;
    {
        let Some(agent) = app.agents.get_mut(&agent_id) else {
            return vec![];
        };
        if let Some(ref mut modal) = agent.extensions_modal {
            modal.pending_action = None;
        }
        match result {
            Ok(mut job) => {
                job.sanitize_secrets();
                let confirm = xai_grok_shell::session::prime::prime_failure_is_confirm_required(
                    job.failure.as_deref(),
                );
                let route = prime_job_display_route(&job);
                if job.is_terminal() && !confirm {
                    fetch = Some((job.generation, job.fingerprint_short.clone()));
                }
                if let Some(ref mut modal) = agent.extensions_modal {
                    if let Some(ref mut status) = modal.prime_index
                        && prime_job_matches_displayed(status.job.as_ref(), &job)
                    {
                        status.job = Some(job.clone());
                        status.generation = job.generation;
                    }
                }
                if let Some(ref mut agents_modal) = agent.agents_modal
                    && let Some(ref mut status) = agents_modal.prime_index
                    && prime_job_matches_displayed(status.job.as_ref(), &job)
                {
                    status.job = Some(job.clone());
                }
                let skills_open = agent.extensions_modal.is_some();
                if let Some(crate::views::modal::ActiveModal::RetrievalSettings { state }) =
                    agent.active_modal.as_mut()
                {
                    if let Some(ref mut status) = state.prime_index
                        && prime_job_matches_displayed(status.job.as_ref(), &job)
                    {
                        status.job = Some(job.clone());
                    }
                    if confirm && !skills_open && !route.is_empty() {
                        prompt_retrieval_prime_confirm(
                            state,
                            &job.kind,
                            job.collection.clone(),
                            route,
                        );
                        return vec![];
                    }
                    if confirm && route.is_empty() {
                        let msg = crate::views::retrieval_settings_modal::PRIME_UNAVAILABLE_PROFILE
                            .to_string();
                        state.error = Some(msg.clone());
                        state.status = Some(msg);
                        if !skills_open {
                            return vec![];
                        }
                    }
                }
                if confirm && route.is_empty() {
                    if let Some(ref mut modal) = agent.extensions_modal {
                        modal.modal_message =
                            Some(crate::views::extensions_modal::ModalMessage::Error(
                                crate::views::retrieval_settings_modal::PRIME_UNAVAILABLE_PROFILE
                                    .to_string(),
                            ));
                    }
                } else if confirm && !route.is_empty() {
                    if let Some(ref mut modal) = agent.extensions_modal {
                        modal.modal_message = Some(
                            crate::views::extensions_modal::ModalMessage::Confirmation {
                                message: format!(
                                    "Use configured route `{route}`? This contacts the embedding profile."
                                ),
                                action:
                                    crate::views::extensions_modal::ConfirmationAction::PrimeIndex {
                                        kind: job.kind.clone(),
                                        collection: job.collection.clone(),
                                    },
                                pending_entry_index: None,
                            },
                        );
                    }
                } else if let Some(fail) = &job.failure
                    && let Some(ref mut modal) = agent.extensions_modal
                {
                    modal.modal_message =
                        Some(crate::views::extensions_modal::ModalMessage::Error(
                            compact_prime_job_error(fail),
                        ));
                }
            }
            Err(e) if e == "unsupported" => {
                if let Some(ref mut modal) = agent.extensions_modal {
                    modal.prime_index_capable = false;
                }
            }
            Err(e) => {
                let confirm =
                    xai_grok_shell::session::prime::prime_failure_is_confirm_required(Some(&e));
                let route = display_configured_route(confirm_required_route(&e)).to_owned();
                let skills_open = agent.extensions_modal.is_some();
                if confirm && route.is_empty() {
                    let msg = crate::views::retrieval_settings_modal::PRIME_UNAVAILABLE_PROFILE
                        .to_string();
                    if let Some(crate::views::modal::ActiveModal::RetrievalSettings { state }) =
                        agent.active_modal.as_mut()
                    {
                        state.error = Some(msg.clone());
                        state.status = Some(msg.clone());
                    }
                    if let Some(ref mut modal) = agent.extensions_modal {
                        modal.modal_message =
                            Some(crate::views::extensions_modal::ModalMessage::Error(msg));
                    }
                } else if confirm
                    && !route.is_empty()
                    && let Some((kind, collection)) =
                        inflight_prime_confirm_target(inflight_kind, inflight_collection)
                {
                    if !skills_open
                        && let Some(crate::views::modal::ActiveModal::RetrievalSettings { state }) =
                            agent.active_modal.as_mut()
                    {
                        prompt_retrieval_prime_confirm(state, &kind, collection, route);
                    } else if let Some(ref mut modal) = agent.extensions_modal {
                        modal.modal_message = Some(
                            crate::views::extensions_modal::ModalMessage::Confirmation {
                                message: format!(
                                    "Use configured route `{route}`? This contacts the embedding profile."
                                ),
                                action:
                                    crate::views::extensions_modal::ConfirmationAction::PrimeIndex {
                                        kind,
                                        collection,
                                    },
                                pending_entry_index: None,
                            },
                        );
                    }
                } else if let Some(ref mut modal) = agent.extensions_modal {
                    modal.modal_message =
                        Some(crate::views::extensions_modal::ModalMessage::Error(
                            compact_prime_job_error(&e),
                        ));
                } else if let Some(crate::views::modal::ActiveModal::RetrievalSettings { state }) =
                    agent.active_modal.as_mut()
                {
                    let compact = compact_prime_job_error(&e);
                    state.error = Some(compact.clone());
                    state.status = Some(compact);
                }
            }
        }
    }
    let mut effects = Vec::new();
    if let Some((generation, fingerprint)) = fetch
        && app.prime_index.status
        && let Some(agent) = app.agents.get(&agent_id)
    {
        let surface_open = agent.extensions_modal.is_some()
            || agent.agents_modal.is_some()
            || matches!(
                agent.active_modal,
                Some(crate::views::modal::ActiveModal::RetrievalSettings { .. })
            );
        if surface_open && let Some(session_id) = agent.session.session_id.clone() {
            effects.push(crate::app::actions::Effect::FetchPrimeIndexStatus {
                agent_id,
                session_id,
                expected_generation: Some(generation),
                expected_fingerprint: Some(fingerprint),
            });
        }
    }
    effects
}

fn inflight_prime_confirm_target(kind: &str, collection: &str) -> Option<(String, String)> {
    let kind = match kind {
        "rebuild" => "rebuild",
        "backfill" => "backfill",
        _ => return None,
    };
    let collection = match collection {
        "skills" | "agents" | "all" => collection,
        _ => return None,
    };
    Some((kind.to_owned(), collection.to_owned()))
}

fn prompt_retrieval_prime_confirm(
    state: &mut crate::views::retrieval_settings_modal::RetrievalSettingsState,
    kind: &str,
    collection: String,
    route: String,
) {
    state.edit = if kind == "rebuild" {
        crate::views::retrieval_settings_modal::RetrievalEditMode::ConfirmPrimeRebuild {
            collection,
            route,
        }
    } else {
        crate::views::retrieval_settings_modal::RetrievalEditMode::ConfirmPrimeBackfill {
            collection,
            route,
        }
    };
}

fn compact_prime_job_error(message: &str) -> String {
    crate::views::retrieval_settings_modal::compact_prime_job_error(message)
}

fn display_configured_route(raw: Option<&str>) -> String {
    crate::views::retrieval_settings_modal::display_configured_route(raw)
}

fn prime_job_display_route(job: &xai_grok_shell::session::prime::PrimeIndexJobStatus) -> String {
    let from_field = display_configured_route(job.configured_route.as_deref());
    if !from_field.is_empty() {
        return from_field;
    }
    display_configured_route(job.failure.as_deref().and_then(confirm_required_route))
}

fn confirm_required_route(message: &str) -> Option<&str> {
    xai_grok_shell::session::prime::confirm_required_display_route(message)
}

#[cfg(test)]
mod management_result_tests {
    use super::{
        apply_prime_index_job, apply_prime_index_status, confirm_required_route,
        inflight_prime_confirm_target, management_result_is_fresh, mutation_operation_matches,
    };

    fn apply_job(
        app: &mut crate::app::app_view::AppView,
        agent_id: crate::app::agent::AgentId,
        result: Result<xai_grok_shell::session::prime::PrimeIndexJobStatus, String>,
    ) -> Vec<crate::app::actions::Effect> {
        apply_prime_index_job(app, agent_id, result, "backfill", "skills")
    }

    #[test]
    fn rejects_wrong_provider_and_older_generation() {
        assert!(management_result_is_fresh("a", 3, "a", 3));
        assert!(management_result_is_fresh("a", 3, "a", 4));
        assert!(!management_result_is_fresh("a", 3, "b", 9));
        assert!(!management_result_is_fresh("a", 5, "a", 4));
    }

    #[test]
    fn mutation_op_correlation_exact_match_only() {
        // Production stamp sites (Enable/Disable/Add/Clone/Save in settings/ui)
        // set pending_*_operation_id before spawning the async mutation. Result
        // handling discards via this pure correlator — unit coverage for that
        // contract (not a full AppView effects harness).
        // Late same-id equal-gen completion with matching op → accept.
        assert!(mutation_operation_matches(Some("op-1"), Some("op-1")));
        // Wrong incarnation / different op id → discard.
        assert!(!mutation_operation_matches(Some("op-1"), Some("op-2")));
        // Pending cleared (historical result arrives late) → discard.
        assert!(!mutation_operation_matches(Some("op-1"), None));
        // No op-id on result → discard (no None => accept).
        assert!(!mutation_operation_matches(None, Some("op-1")));
        assert!(!mutation_operation_matches(None, None));
    }

    #[test]
    fn confirm_required_route_extracts_id_not_endpoint() {
        assert_eq!(
            confirm_required_route("confirm_required:main"),
            Some("main")
        );
        assert_eq!(
            confirm_required_route("couldn't run prime index: confirm_required:lab-emb"),
            Some("lab-emb")
        );
        assert_eq!(confirm_required_route("unavailable"), None);
        assert_eq!(
            confirm_required_route("confirm_required:http://127.0.0.1/v1"),
            None,
            "endpoints must not be shown as a configured route"
        );
        assert!(confirm_required_route("sk-live-secret").is_none());
        assert_eq!(
            confirm_required_route("confirm_required:file:///tmp/secret"),
            None
        );
        assert_eq!(
            confirm_required_route("confirm_required:main\nsk-live-secret"),
            None
        );
        assert_eq!(
            confirm_required_route(&format!("confirm_required:{}", "x".repeat(200))),
            None
        );
        assert_eq!(
            confirm_required_route("confirm_required"),
            None,
            "bare confirm_required has no display route"
        );
    }

    fn sample_status(
        unchanged: bool,
        done: u64,
        state: &str,
    ) -> xai_grok_shell::session::prime::PrimeIndexStatus {
        xai_grok_shell::session::prime::PrimeIndexStatus {
            api_version: 1,
            generation: 4,
            fingerprint_short: "abc123def456".into(),
            skills: xai_grok_shell::session::prime::PrimeIndexCollectionStatus {
                collection: "skills".into(),
                generation: 4,
                fingerprint_short: "abc123def456".into(),
                item_count: 3,
                vector_count: 1,
                missing_vectors: 2,
                readiness: "pending".into(),
                route_id: Some("main".into()),
                dimensions: None,
            },
            agents: xai_grok_shell::session::prime::PrimeIndexCollectionStatus {
                collection: "agents".into(),
                generation: 0,
                fingerprint_short: String::new(),
                item_count: 0,
                vector_count: 0,
                missing_vectors: 0,
                readiness: "ready".into(),
                route_id: None,
                dimensions: None,
            },
            job: Some(xai_grok_shell::session::prime::PrimeIndexJobStatus {
                api_version: 1,
                job_id: "j1".into(),
                kind: "backfill".into(),
                collection: "skills".into(),
                state: state.into(),
                generation: 4,
                fingerprint_short: "abc123def456".into(),
                done,
                total: 3,
                confirm_configured_profile: false,
                configured_route: Some("main".into()),
                failure: None,
            }),
            configured_route: Some("main".into()),
            capabilities: xai_grok_shell::session::prime::PrimeIndexCapabilities::SUPPORTED,
            unchanged,
        }
    }

    #[test]
    fn unchanged_status_still_merges_job_progress() {
        use crate::views::extensions_modal::{ExtensionsModalState, ExtensionsTab};
        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let agent_id = crate::app::agent::AgentId(0);
        if let Some(agent) = app.agents.get_mut(&agent_id) {
            let mut modal = ExtensionsModalState::new(ExtensionsTab::Skills);
            modal.picker_state.set_query("commit");
            modal.prime_index = Some(sample_status(false, 1, "running"));
            agent.extensions_modal = Some(modal);
        }
        apply_prime_index_status(&mut app, agent_id, Ok(sample_status(true, 2, "failed")));
        let agent = app.agents.get(&agent_id).expect("agent");
        let modal = agent.extensions_modal.as_ref().expect("skills");
        assert_eq!(modal.picker_state.query(), "commit");
        let job = modal
            .prime_index
            .as_ref()
            .and_then(|s| s.job.as_ref())
            .expect("job");
        assert_eq!(job.done, 2);
        assert_eq!(job.state, "failed");
        let footer = modal.prime_index_footer_line(true).expect("footer");
        assert!(
            footer.contains("failed") && footer.contains("2/3"),
            "{footer}"
        );
    }

    #[test]
    fn confirm_required_attaches_to_retrieval_when_skills_closed() {
        use crate::views::modal::ActiveModal;
        use crate::views::retrieval_settings_modal::{RetrievalEditMode, RetrievalSettingsState};
        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let agent_id = crate::app::agent::AgentId(0);
        if let Some(agent) = app.agents.get_mut(&agent_id) {
            agent.extensions_modal = None;
            let mut state = RetrievalSettingsState::new();
            state.prime_index = Some(sample_status(false, 0, "failed"));
            agent.active_modal = Some(ActiveModal::RetrievalSettings {
                state: Box::new(state),
            });
        }
        let job = xai_grok_shell::session::prime::PrimeIndexJobStatus {
            api_version: 1,
            job_id: String::new(),
            kind: "backfill".into(),
            collection: "skills".into(),
            state: "failed".into(),
            generation: 4,
            fingerprint_short: "abc123def456".into(),
            done: 0,
            total: 0,
            confirm_configured_profile: false,
            configured_route: Some("main".into()),
            failure: Some("confirm_required:main".into()),
        };
        apply_job(&mut app, agent_id, Ok(job));
        let agent = app.agents.get(&agent_id).expect("agent");
        let ActiveModal::RetrievalSettings { state } = agent.active_modal.as_ref().unwrap() else {
            panic!("retrieval settings");
        };
        match &state.edit {
            RetrievalEditMode::ConfirmPrimeBackfill { collection, route } => {
                assert_eq!(collection, "skills");
                assert_eq!(route, "main");
            }
            other => panic!("expected ConfirmPrimeBackfill, got {other:?}"),
        }
    }

    #[test]
    fn completed_same_generation_job_then_unchanged_status_shows_ready() {
        use crate::app::actions::Effect;
        use crate::views::extensions_modal::{ExtensionsModalState, ExtensionsTab};
        let mut app = crate::app::app_view::tests::test_app_with_agent();
        app.prime_index = xai_grok_shell::session::prime::PrimeIndexCapabilities::SUPPORTED;
        let agent_id = crate::app::agent::AgentId(0);
        if let Some(agent) = app.agents.get_mut(&agent_id) {
            let mut modal = ExtensionsModalState::new(ExtensionsTab::Skills);
            modal.prime_index = Some(sample_status(false, 1, "running"));
            agent.extensions_modal = Some(modal);
        }
        let completed = xai_grok_shell::session::prime::PrimeIndexJobStatus {
            api_version: 1,
            job_id: "j1".into(),
            kind: "backfill".into(),
            collection: "skills".into(),
            state: "completed".into(),
            generation: 4,
            fingerprint_short: "abc123def456".into(),
            done: 3,
            total: 3,
            confirm_configured_profile: false,
            configured_route: Some("main".into()),
            failure: None,
        };
        let effects = apply_job(&mut app, agent_id, Ok(completed));
        assert!(
            effects.iter().any(|e| matches!(
                e,
                Effect::FetchPrimeIndexStatus {
                    expected_generation: Some(4),
                    ..
                }
            )),
            "completed same-gen job must enqueue a status refresh, got {effects:?}"
        );
        let mut ready = sample_status(true, 3, "completed");
        ready.skills.vector_count = 3;
        ready.skills.missing_vectors = 0;
        ready.skills.readiness = "ready".into();
        apply_prime_index_status(&mut app, agent_id, Ok(ready));
        let agent = app.agents.get(&agent_id).expect("agent");
        let modal = agent.extensions_modal.as_ref().expect("skills");
        let status = modal.prime_index.as_ref().expect("status");
        assert_eq!(status.skills.vector_count, 3);
        assert_eq!(status.skills.readiness, "ready");
        let footer = modal.prime_index_footer_line(true).expect("footer");
        assert!(
            footer.contains("ready") && footer.contains("3/3"),
            "{footer}"
        );
        assert!(!footer.contains("completed"), "{footer}");
    }

    #[test]
    fn unavailable_job_error_surfaces_on_retrieval_when_skills_closed() {
        use crate::views::modal::ActiveModal;
        use crate::views::retrieval_settings_modal::{
            PRIME_UNAVAILABLE_PROFILE, RetrievalSettingsState,
        };
        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let agent_id = crate::app::agent::AgentId(0);
        if let Some(agent) = app.agents.get_mut(&agent_id) {
            agent.extensions_modal = None;
            agent.active_modal = Some(ActiveModal::RetrievalSettings {
                state: Box::new(RetrievalSettingsState::new()),
            });
        }
        let _ = apply_job(&mut app, agent_id, Err("unavailable".into()));
        let agent = app.agents.get(&agent_id).expect("agent");
        let ActiveModal::RetrievalSettings { state } = agent.active_modal.as_ref().unwrap() else {
            panic!("retrieval settings");
        };
        assert_eq!(state.error.as_deref(), Some(PRIME_UNAVAILABLE_PROFILE));
        assert_eq!(state.status.as_deref(), Some(PRIME_UNAVAILABLE_PROFILE));
        let _ = apply_job(&mut app, agent_id, Err("already_running".into()));
        let agent = app.agents.get(&agent_id).expect("agent");
        let ActiveModal::RetrievalSettings { state } = agent.active_modal.as_ref().unwrap() else {
            panic!("retrieval settings");
        };
        assert_eq!(state.error.as_deref(), Some("already_running"));
        assert_eq!(state.status.as_deref(), Some("already_running"));
    }

    fn unsanitary_confirm_job(raw: &str) -> xai_grok_shell::session::prime::PrimeIndexJobStatus {
        xai_grok_shell::session::prime::PrimeIndexJobStatus {
            api_version: 1,
            job_id: String::new(),
            kind: "backfill".into(),
            collection: "skills".into(),
            state: "failed".into(),
            generation: 4,
            fingerprint_short: "abc123def456".into(),
            done: 0,
            total: 0,
            confirm_configured_profile: false,
            configured_route: Some(raw.into()),
            failure: Some(format!("confirm_required:{raw}")),
        }
    }

    fn assert_no_raw_in_job(job: &xai_grok_shell::session::prime::PrimeIndexJobStatus, raw: &str) {
        let json = serde_json::to_string(job).expect("job json");
        for leak in [raw, raw.trim(), "127.0.0.1", "sk-live-secret", "file://"] {
            if leak.is_empty() {
                continue;
            }
            assert!(
                !json.contains(leak),
                "leaked {leak:?} from {raw:?} in job json {json}"
            );
        }
        if let Some(route) = &job.configured_route {
            assert!(
                !route.contains(raw.trim()) && !raw.contains('\n') || route != raw,
                "configured_route retained raw {raw:?}"
            );
        }
        if let Some(failure) = &job.failure {
            assert!(
                !failure.contains(raw.trim()),
                "failure retained raw {raw:?}: {failure}"
            );
            assert!(
                !failure.contains('\n') && !failure.contains('\u{0007}'),
                "failure retained control text: {failure}"
            );
        }
    }

    #[test]
    fn apply_prime_index_job_ok_omits_unsanitary_routes_from_overlay_and_state() {
        use crate::views::extensions_modal::{ExtensionsModalState, ExtensionsTab, ModalMessage};
        use crate::views::modal::ActiveModal;
        use crate::views::retrieval_settings_modal::{
            PRIME_UNAVAILABLE_PROFILE, RetrievalEditMode, RetrievalSettingsState,
        };

        let long = "x".repeat(200);
        let cases = [
            "http://127.0.0.1/v1",
            "sk-live-secret",
            "file:///tmp/secret",
            "main\nsk-live-secret",
            long.as_str(),
        ];
        for raw in cases {
            let mut app = crate::app::app_view::tests::test_app_with_agent();
            let agent_id = crate::app::agent::AgentId(0);
            if let Some(agent) = app.agents.get_mut(&agent_id) {
                let mut modal = ExtensionsModalState::new(ExtensionsTab::Skills);
                modal.prime_index = Some(sample_status(false, 0, "failed"));
                agent.extensions_modal = Some(modal);
                let mut state = RetrievalSettingsState::new();
                state.prime_index = Some(sample_status(false, 0, "failed"));
                agent.active_modal = Some(ActiveModal::RetrievalSettings {
                    state: Box::new(state),
                });
            }
            apply_job(&mut app, agent_id, Ok(unsanitary_confirm_job(raw)));
            let agent = app.agents.get(&agent_id).expect("agent");
            let modal = agent.extensions_modal.as_ref().expect("skills");
            match modal.modal_message.as_ref() {
                Some(ModalMessage::Error(msg)) => {
                    assert_eq!(msg, PRIME_UNAVAILABLE_PROFILE);
                    assert!(!msg.contains(raw.trim()), "{msg}");
                }
                Some(ModalMessage::Confirmation { message, .. }) => {
                    panic!(
                        "must not dispatch confirmation for unsanitary route {raw:?}: {message}"
                    );
                }
                None => panic!("expected unavailable overlay for {raw:?}"),
            }
            let job = modal
                .prime_index
                .as_ref()
                .and_then(|s| s.job.as_ref())
                .expect("stored job");
            assert_no_raw_in_job(job, raw);
            let ActiveModal::RetrievalSettings { state } = agent.active_modal.as_ref().unwrap()
            else {
                panic!("retrieval settings");
            };
            assert!(
                matches!(state.edit, RetrievalEditMode::Browse),
                "must not dispatch retrieval confirm for {raw:?}: {:?}",
                state.edit
            );
            let stored = state.prime_index.as_ref().and_then(|s| s.job.as_ref());
            if let Some(job) = stored {
                assert_no_raw_in_job(job, raw);
            }
            let diag = format!(
                "{:?} {:?} {:?}",
                modal.modal_message, modal.prime_index, state.prime_index
            );
            assert!(
                !diag.contains(raw.trim()),
                "diagnostics leaked {raw:?}: {diag}"
            );
            assert!(
                !diag.contains("127.0.0.1") || !raw.contains("127.0.0.1"),
                "{diag}"
            );
            assert!(
                !diag.contains("sk-live-secret") || !raw.contains("sk-"),
                "{diag}"
            );
        }
    }

    #[test]
    fn apply_prime_index_job_ok_safe_profile_id_still_confirms() {
        use crate::views::extensions_modal::{ExtensionsModalState, ExtensionsTab, ModalMessage};
        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let agent_id = crate::app::agent::AgentId(0);
        if let Some(agent) = app.agents.get_mut(&agent_id) {
            let mut modal = ExtensionsModalState::new(ExtensionsTab::Skills);
            modal.prime_index = Some(sample_status(false, 0, "failed"));
            agent.extensions_modal = Some(modal);
        }
        apply_job(&mut app, agent_id, Ok(unsanitary_confirm_job("main")));
        let agent = app.agents.get(&agent_id).expect("agent");
        let modal = agent.extensions_modal.as_ref().expect("skills");
        match modal.modal_message.as_ref() {
            Some(ModalMessage::Confirmation { message, .. }) => {
                assert!(message.contains("`main`"), "{message}");
                assert!(!message.contains("http"), "{message}");
            }
            other => panic!("expected confirmation for safe id, got {other:?}"),
        }
        let job = modal
            .prime_index
            .as_ref()
            .and_then(|s| s.job.as_ref())
            .expect("job");
        assert_eq!(job.configured_route.as_deref(), Some("main"));
        assert_eq!(job.failure.as_deref(), Some("confirm_required:main"));
    }

    #[test]
    fn apply_prime_index_job_err_prefixed_confirm_required_is_safe() {
        use crate::views::extensions_modal::{ExtensionsModalState, ExtensionsTab, ModalMessage};
        use crate::views::modal::ActiveModal;
        use crate::views::retrieval_settings_modal::{
            PRIME_UNAVAILABLE_PROFILE, RetrievalEditMode, RetrievalSettingsState,
        };

        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let agent_id = crate::app::agent::AgentId(0);
        if let Some(agent) = app.agents.get_mut(&agent_id) {
            let mut modal = ExtensionsModalState::new(ExtensionsTab::Skills);
            modal.prime_index = Some(sample_status(false, 0, "failed"));
            agent.extensions_modal = Some(modal);
        }
        apply_job(
            &mut app,
            agent_id,
            Err("couldn't run prime index: confirm_required:main".into()),
        );
        let agent = app.agents.get(&agent_id).expect("agent");
        match agent
            .extensions_modal
            .as_ref()
            .and_then(|m| m.modal_message.as_ref())
        {
            Some(ModalMessage::Confirmation {
                message,
                action:
                    crate::views::extensions_modal::ConfirmationAction::PrimeIndex { kind, collection },
                ..
            }) => {
                assert!(message.contains("`main`"), "{message}");
                assert!(!message.contains("couldn't run"), "{message}");
                assert_eq!(kind, "backfill");
                assert_eq!(collection, "skills");
            }
            other => panic!("prefixed confirm_required:main must confirm, got {other:?}"),
        }

        let mut app = crate::app::app_view::tests::test_app_with_agent();
        if let Some(agent) = app.agents.get_mut(&agent_id) {
            let mut modal = ExtensionsModalState::new(ExtensionsTab::Skills);
            modal.prime_index = Some(sample_status(false, 0, "failed"));
            agent.extensions_modal = Some(modal);
            agent.active_modal = Some(ActiveModal::RetrievalSettings {
                state: Box::new(RetrievalSettingsState::new()),
            });
        }
        let raw = "couldn't run prime index: confirm_required:http://127.0.0.1/v1";
        apply_job(&mut app, agent_id, Err(raw.into()));
        let agent = app.agents.get(&agent_id).expect("agent");
        match agent
            .extensions_modal
            .as_ref()
            .and_then(|m| m.modal_message.as_ref())
        {
            Some(ModalMessage::Error(msg)) => {
                assert_eq!(msg, PRIME_UNAVAILABLE_PROFILE);
                assert!(!msg.contains("127.0.0.1"), "{msg}");
                assert!(!msg.contains("couldn't run"), "{msg}");
            }
            other => panic!("prefixed unsanitary confirm must be unavailable, got {other:?}"),
        }
        let ActiveModal::RetrievalSettings { state } = agent.active_modal.as_ref().unwrap() else {
            panic!("retrieval settings");
        };
        assert!(matches!(state.edit, RetrievalEditMode::Browse));
        assert_eq!(state.error.as_deref(), Some(PRIME_UNAVAILABLE_PROFILE));
    }

    #[test]
    fn apply_prime_index_job_err_confirm_retries_inflight_kind_and_collection() {
        use crate::views::extensions_modal::{
            ConfirmationAction, ExtensionsModalState, ExtensionsTab, ModalMessage,
        };
        use crate::views::modal::ActiveModal;
        use crate::views::retrieval_settings_modal::{RetrievalEditMode, RetrievalSettingsState};

        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let agent_id = crate::app::agent::AgentId(0);
        if let Some(agent) = app.agents.get_mut(&agent_id) {
            let mut modal = ExtensionsModalState::new(ExtensionsTab::Skills);
            modal.prime_index = Some(sample_status(false, 0, "failed"));
            agent.extensions_modal = Some(modal);
        }
        apply_prime_index_job(
            &mut app,
            agent_id,
            Err("couldn't run prime index: confirm_required:main".into()),
            "rebuild",
            "agents",
        );
        let agent = app.agents.get(&agent_id).expect("agent");
        match agent
            .extensions_modal
            .as_ref()
            .and_then(|m| m.modal_message.as_ref())
        {
            Some(ModalMessage::Confirmation {
                action: ConfirmationAction::PrimeIndex { kind, collection },
                ..
            }) => {
                assert_eq!(kind, "rebuild");
                assert_eq!(collection, "agents");
            }
            other => panic!("rebuild/agents inflight must confirm that op, got {other:?}"),
        }

        let mut app = crate::app::app_view::tests::test_app_with_agent();
        if let Some(agent) = app.agents.get_mut(&agent_id) {
            agent.extensions_modal = None;
            let mut state = RetrievalSettingsState::new();
            state.prime_index = Some(sample_status(false, 0, "failed"));
            agent.active_modal = Some(ActiveModal::RetrievalSettings {
                state: Box::new(state),
            });
        }
        apply_prime_index_job(
            &mut app,
            agent_id,
            Err("couldn't run prime index: confirm_required:main".into()),
            "rebuild",
            "agents",
        );
        let agent = app.agents.get(&agent_id).expect("agent");
        let ActiveModal::RetrievalSettings { state } = agent.active_modal.as_ref().unwrap() else {
            panic!("retrieval settings");
        };
        match &state.edit {
            RetrievalEditMode::ConfirmPrimeRebuild { collection, route } => {
                assert_eq!(collection, "agents");
                assert_eq!(route, "main");
            }
            other => panic!("expected ConfirmPrimeRebuild for agents, got {other:?}"),
        }
    }

    #[test]
    fn apply_prime_index_job_err_confirm_without_inflight_does_not_guess_backfill() {
        use crate::views::extensions_modal::{ExtensionsModalState, ExtensionsTab, ModalMessage};
        use crate::views::modal::ActiveModal;
        use crate::views::retrieval_settings_modal::{RetrievalEditMode, RetrievalSettingsState};

        assert!(inflight_prime_confirm_target("cancel", "all").is_none());
        assert!(inflight_prime_confirm_target("", "").is_none());
        assert_eq!(
            inflight_prime_confirm_target("rebuild", "agents"),
            Some(("rebuild".into(), "agents".into()))
        );

        let mut app = crate::app::app_view::tests::test_app_with_agent();
        let agent_id = crate::app::agent::AgentId(0);
        if let Some(agent) = app.agents.get_mut(&agent_id) {
            let mut modal = ExtensionsModalState::new(ExtensionsTab::Skills);
            modal.prime_index = Some(sample_status(false, 0, "failed"));
            agent.extensions_modal = Some(modal);
        }
        apply_prime_index_job(
            &mut app,
            agent_id,
            Err("couldn't run prime index: confirm_required:main".into()),
            "cancel",
            "all",
        );
        let agent = app.agents.get(&agent_id).expect("agent");
        match agent
            .extensions_modal
            .as_ref()
            .and_then(|m| m.modal_message.as_ref())
        {
            Some(ModalMessage::Error(msg)) => {
                assert_eq!(msg, "confirm required");
                assert!(!msg.contains("couldn't run"), "{msg}");
                assert!(!msg.contains("main"), "{msg}");
            }
            other => panic!("bare cancel inflight must not confirm backfill/skills, got {other:?}"),
        }

        let mut app = crate::app::app_view::tests::test_app_with_agent();
        if let Some(agent) = app.agents.get_mut(&agent_id) {
            agent.extensions_modal = None;
            agent.active_modal = Some(ActiveModal::RetrievalSettings {
                state: Box::new(RetrievalSettingsState::new()),
            });
        }
        apply_prime_index_job(
            &mut app,
            agent_id,
            Err("couldn't run prime index: confirm_required:main".into()),
            "",
            "",
        );
        let agent = app.agents.get(&agent_id).expect("agent");
        let ActiveModal::RetrievalSettings { state } = agent.active_modal.as_ref().unwrap() else {
            panic!("retrieval settings");
        };
        assert!(matches!(state.edit, RetrievalEditMode::Browse));
        assert_eq!(state.error.as_deref(), Some("confirm required"));
    }
}
