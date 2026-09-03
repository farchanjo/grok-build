use super::*;
use serde::Deserialize;

/// Handle `x.ai/models/update` — model list changed (etag-triggered refresh).
///
/// Updates are generation-gated: stale or equal-generation notifications are
/// rejected so catalog content and generation stay coherent on both
/// `app.models` and per-agent state. Each agent's current selection is
/// preserved by exact canonical catalog id when still present; removed
/// catalog keys fall back to the shell current (sibling keys are not
/// auto-selected).
pub(super) fn handle_models_update(notif: &acp::ExtNotification, app: &mut AppView) -> bool {
    if let Ok(model_state) = serde_json::from_str::<acp::SessionModelState>(notif.params.get()) {
        use crate::acp::model_state::ModelState;
        let new_models = ModelState::from(Some(model_state));
        let generation = new_models.catalog_generation;
        tracing::info!(
            count = new_models.available.len(),
            catalog_generation = generation,
            "models updated via x.ai/models/update"
        );

        let shell_fallback_current = new_models.current.clone();
        let generation_opt = (generation > 0).then_some(generation);

        // App-level catalog: versioned apply (never wholesale replace — a
        // delayed lower-generation update must not roll generation backward).
        let app_applied = app.models.update_catalog_versioned(
            new_models.available.clone(),
            shell_fallback_current.clone(),
            generation_opt,
        );
        if !app_applied {
            tracing::debug!(
                catalog_generation = generation,
                app_generation = app.models.catalog_generation,
                "models/update rejected as stale for app.models; skipping agent fan-out"
            );
            return true;
        }

        // After a successful app apply, prefer the active agent's exact
        // canonical id when it is still in the new catalog.
        if let ActiveView::Agent(id) = app.active_view
            && let Some(agent) = app.agents.get(&id)
            && let Some(ref agent_model) = agent.session.models.current
            && app.models.available.contains_key(agent_model)
        {
            app.models.current = Some(agent_model.clone());
        }

        for agent in app.agents.values_mut() {
            // Log when an update drops the agent's active model — this is the
            // moment the status bar visibly "switches model mid-conversation"
            // (the agent falls back to the shell's current model below).
            if let Some(ref current) = agent.session.models.current
                && !new_models.available.contains_key(current)
            {
                tracing::warn!(
                    current_model = %current.0,
                    fallback = ?shell_fallback_current.as_ref().map(|m| m.0.as_ref()),
                    available_count = new_models.available.len(),
                    catalog_generation = generation,
                    "models update removed this agent's current catalog key; falling back"
                );
            }
            let applied = agent.session.models.update_catalog_versioned(
                new_models.available.clone(),
                shell_fallback_current.clone(),
                generation_opt,
            );
            if applied {
                // Rebuild slash snapshot only from an applied catalog so open
                // pickers never mix pre/post-refresh rows from a rejected gen.
                agent.prompt.refresh_slash(&agent.session.models);
                let catalog = agent.session.models.clone();
                if let Some(crate::views::modal::ActiveModal::Providers { state }) =
                    agent.active_modal.as_mut()
                    && let Some(status) =
                        state.status_mut(&crate::views::providers_modal::ProviderKind::OpenAi)
                {
                    status.overlay_chatgpt_windows(|id| catalog.context_window_tokens_for(id));
                }
            }
        }
        // The shell emits this notification after config/model reload. Refresh
        // the pager-owned media snapshot after the agent loop so `/settings`
        // agrees with the policy already fanned out to active sessions. Tests
        // may not have a GROK_HOME-backed config, so keep the current snapshot
        // when loading fails.
        if let Some(media) = crate::app::event_loop::load_media_config_from_disk() {
            app.media_config = media;
        }
        true
    } else {
        tracing::warn!("Failed to parse x.ai/models/update");
        false
    }
}

/// Handle `x.ai/providers/update` — registry generation broadcast.
///
/// Version-tolerant: unknown optional fields are ignored. Clean `/providers`
/// list reloads the shell snapshot; a dirty open editor enters conflict mode
/// without clobbering local drafts.
pub(super) fn handle_providers_update(notif: &acp::ExtNotification, app: &mut AppView) -> bool {
    #[derive(Deserialize, Default)]
    struct ProvidersUpdate {
        #[serde(default)]
        schema_version: u32,
        #[serde(default)]
        generation: u64,
        #[serde(default)]
        changed_ids: Vec<String>,
        #[serde(default)]
        changed_fields: Vec<String>,
    }
    let update: ProvidersUpdate = serde_json::from_str(notif.params.get()).unwrap_or_default();
    tracing::info!(
        generation = update.generation,
        schema_version = update.schema_version,
        changed = ?update.changed_ids,
        "providers updated via x.ai/providers/update"
    );
    let mut effects = Vec::new();
    for (agent_id, agent) in app.agents.iter_mut() {
        let Some(crate::views::modal::ActiveModal::Providers { state }) =
            agent.active_modal.as_mut()
        else {
            continue;
        };
        if let Some(ed) = state.editor_mut() {
            let targets_this = update.changed_ids.is_empty()
                || update.changed_ids.iter().any(|id| id == &ed.detail.id);
            let gen_advanced = update.generation > ed.detail.generation.get();
            if targets_this && gen_advanced {
                if ed.is_dirty() {
                    // Dirty-only conflict: keep drafts intact.
                    ed.enter_conflict(
                        xai_grok_shell::provider_registry::management::dto::ProviderConflictInfo {
                            provider_id: ed.detail.id.clone(),
                            client_generation: ed.detail.generation,
                            live_generation:
                                xai_grok_shell::provider_registry::management::dto::RegistryGeneration(
                                    update.generation,
                                ),
                            changed_fields: update.changed_fields.clone(),
                            guidance: "Registry generation advanced. Reload to discard local edits, or Clone into a new id.".into(),
                        },
                    );
                } else {
                    // Clean editor: auto reload detail.
                    effects.push(crate::app::actions::Effect::ProviderOperation {
                        agent_id: *agent_id,
                        operation: crate::app::actions::ProviderOperation::LoadEditorDetail {
                            provider_id: ed.detail.id.clone(),
                        },
                        repair: None,
                    });
                }
            }
        } else {
            // Clean list: auto LoadListSnapshot.
            effects.push(crate::app::actions::Effect::ProviderOperation {
                agent_id: *agent_id,
                operation: crate::app::actions::ProviderOperation::LoadListSnapshot,
                repair: None,
            });
        }
        if update.generation > 0 {
            state.list_generation = update.generation;
        }
    }
    app.pending_effects.extend(effects);
    true
}

/// Handle `x.ai/retrieval/update` — retrieval graph generation broadcast (PR15).
///
/// Version-tolerant: unknown optional fields are ignored. Empty params tolerated.
/// Dirty modal preserves draft and enters conflict; clean open modal enqueues
/// `LoadSnapshot`. No raw config/content in the payload.
pub(super) fn handle_retrieval_update(notif: &acp::ExtNotification, app: &mut AppView) -> bool {
    #[derive(Deserialize, Default)]
    struct RetrievalUpdate {
        #[serde(default)]
        schema_version: u32,
        #[serde(default)]
        generation: u64,
        #[serde(default)]
        changed_fields: Vec<String>,
    }
    let update: RetrievalUpdate = serde_json::from_str(notif.params.get()).unwrap_or_default();
    tracing::info!(
        generation = update.generation,
        schema_version = update.schema_version,
        changed = ?update.changed_fields,
        "retrieval graph updated via x.ai/retrieval/update"
    );
    if update.generation == 0 {
        return true;
    }
    let mut effects = Vec::new();
    for (agent_id, agent) in app.agents.iter_mut() {
        let Some(crate::views::modal::ActiveModal::RetrievalSettings { state }) =
            agent.active_modal.as_mut()
        else {
            continue;
        };
        let live = xai_grok_shell::provider_registry::management::dto::RegistryGeneration(
            update.generation,
        );
        if state.on_remote_generation(live, &update.changed_fields) {
            effects.push(crate::app::actions::Effect::RetrievalOperation {
                agent_id: *agent_id,
                operation: crate::app::actions::RetrievalOperation::LoadSnapshot,
            });
        }
    }
    app.pending_effects.extend(effects);
    true
}

/// Handle `x.ai/prime/index/update` — version-tolerant job/status broadcast.
///
/// `notifySeq` is the delivery clock. Inventory `generation` is a blake3
/// identity hash, not a monotonic watermark: when `notifySeq > 0`, apply if
/// the sequence advanced and treat generation as equality/precondition
/// identity (`!=`, never `.max()`). Legacy shells (`notifySeq == 0`) still
/// reject `generation < last`. Same-generation job ticks apply when
/// `notifySeq` advances. Terminal jobs refresh compact counts via a
/// generation/fingerprint-preconditioned status fetch so `unchanged=true`
/// still merges vector_count/readiness. Unknown optional fields are ignored.
/// Search/filter/selection in `/skills` are preserved because this only
/// refreshes compact index state, not the inventory rows.
pub(super) fn handle_prime_index_update(notif: &acp::ExtNotification, app: &mut AppView) -> bool {
    use xai_grok_shell::session::prime::PrimeIndexUpdate;
    let update: PrimeIndexUpdate = serde_json::from_str(notif.params.get()).unwrap_or_default();
    if !prime_index_update_is_actionable(
        &update,
        app.prime_index_last_generation,
        app.prime_index_last_notify_seq,
    ) {
        return true;
    }
    if let Some(api) = update.api_version
        && api != 1
    {
        // Unknown major: still advance watermarks so we do not retry, but do
        // not apply job details.
        advance_prime_index_watermarks(app, &update);
        return true;
    }
    let generation_advanced = update.generation != app.prime_index_last_generation;
    advance_prime_index_watermarks(app, &update);
    let fetch_status = app.prime_index.status && (generation_advanced || update.job_is_terminal());
    let mut effects = Vec::new();
    for (agent_id, agent) in app.agents.iter_mut() {
        let session_id = agent.session.session_id.clone();
        let mut surface_open = false;
        if let Some(ref mut modal) = agent.extensions_modal {
            modal.apply_prime_index_update(&update);
            surface_open = true;
        }
        if let Some(ref mut agents_modal) = agent.agents_modal {
            agents_modal.apply_prime_index_update(&update);
            surface_open = true;
        }
        if let Some(crate::views::modal::ActiveModal::RetrievalSettings { state }) =
            agent.active_modal.as_mut()
        {
            state.apply_prime_index_update(&update);
            surface_open = true;
        }
        if fetch_status
            && surface_open
            && let Some(session_id) = session_id
        {
            effects.push(crate::app::actions::Effect::FetchPrimeIndexStatus {
                agent_id: *agent_id,
                session_id,
                expected_generation: Some(update.generation),
                expected_fingerprint: Some(update.fingerprint_short.clone()),
            });
        }
    }
    app.pending_effects.extend(effects);
    true
}

fn prime_index_update_is_actionable(
    update: &xai_grok_shell::session::prime::PrimeIndexUpdate,
    last_gen: u64,
    last_seq: u64,
) -> bool {
    let has_job = update.job.is_some() || update.changed_fields.iter().any(|f| f == "job");
    if update.notify_seq > 0 {
        // Order solely by notifySeq. Generation is an identity hash.
        return update.notify_seq > last_seq;
    }
    // Legacy shells omit notifySeq: generation is the only watermark.
    if update.generation == 0 {
        return has_job && last_seq == 0;
    }
    if update.generation < last_gen {
        return false;
    }
    if update.generation > last_gen {
        return true;
    }
    has_job
}

fn advance_prime_index_watermarks(
    app: &mut AppView,
    update: &xai_grok_shell::session::prime::PrimeIndexUpdate,
) {
    if update.notify_seq > 0 {
        // Identity, not a clock — a restage hash may be numerically smaller.
        app.prime_index_last_generation = update.generation;
        if update.notify_seq > app.prime_index_last_notify_seq {
            app.prime_index_last_notify_seq = update.notify_seq;
        }
    } else {
        app.prime_index_last_generation = app.prime_index_last_generation.max(update.generation);
    }
}

/// Handle `x.ai/settings/update` — remote settings refreshed on `/new`.
pub(super) fn handle_settings_update(notif: &acp::ExtNotification, app: &mut AppView) -> bool {
    let Ok(update) = serde_json::from_str::<PagerSettingsUpdate>(notif.params.get()) else {
        tracing::warn!("Failed to parse x.ai/settings/update");
        return false;
    };

    if let Some(v) = update.auto_permission_mode_enabled {
        // Keep the pager's auto-permission-mode gate live with the remote settings
        // remote tier (the leader caches it agent-side; the pager process needs
        // its own copy). Refresh the startup snapshot so the Shift+Tab cycle and
        // the settings modal both reflect a remote-only enablement/kill-switch
        // without a restart.
        xai_grok_shell::util::config::cache_remote_auto_permission_mode_enabled(Some(v));
        app.auto_mode_gate = xai_grok_shell::util::config::auto_permission_mode_enabled_from_disk();
        // Mid-session kill switch: when the gate just went off, drop displayed
        // Auto to Ask + clear every agent's per-session flag (shared with the
        // startup reconcile), AND tell live sessions to leave Auto. Clearing only
        // the display would let the agent keep classifier-approving while the UI
        // shows "Ask" — the emergency-off must actually disable enforcement.
        if !app.auto_mode_gate {
            // Sessions to notify: agents that HAD Auto on (capture before the
            // downgrade clears the flag) and have a live session id.
            let leaving_auto: Vec<acp::SessionId> = app
                .agents
                .values()
                .filter(|a| a.session.is_auto())
                .filter_map(|a| a.session.session_id.clone())
                .collect();
            super::super::dispatch::downgrade_displayed_auto_if_gated(app);
            notify_sessions_leave_auto(app, &leaving_auto);
        }
        // Reveal/hide `/auto` on every slash surface in lockstep with the gate
        // (covers both a mid-session kill-switch and re-enablement).
        app.sync_permission_mode_slash_gate();
    }

    // `permission_mode` is presence-aware (omit / null / string). While the
    // soft default still owns the mode, a push re-arms `default_yolo` + UI for
    // the next `/new`; once the user claims a mode (Shift+Tab / settings /
    // `/mode`) the latch is cleared and pushes leave it alone.
    if let Some(remote_opt) = update.permission_mode.as_ref()
        && app.permission_mode_from_soft_default
    {
        // One config read at the I/O boundary; the applier is deterministic.
        let root = xai_grok_shell::config::load_effective_config().ok();
        apply_soft_default_permission_mode(
            app,
            root.as_ref().and_then(|r| r.get("ui")),
            remote_opt.as_deref(),
        );
    }

    if let Some(v) = update.show_resolved_model {
        app.show_resolved_model = v;
    }
    if let Some(v) = update.sharing_enabled {
        app.sharing_enabled = v;
        // Propagate to existing agents so slash-command registries stay
        // in sync (same fan-out pattern used when creating new agents).
        for agent in app.agents.values_mut() {
            agent.set_sharing_enabled(v);
        }
    }
    if let Some(v) = update.privacy_notice_rollout {
        app.privacy_notice_rollout = v;
    }
    if let Some(v) = update.privacy_banner_reshow_days {
        app.privacy_banner_reshow_days = Some(v);
    }
    // Tier before voice: same payload may set "API Key" and voice_mode_enabled=false.
    // Always recompute is_api_key_auth from the tier so a later Free/SuperGrok
    // stamp does not leave API-key bypass / a hidden billing surface stuck.
    if let Some(v) = update.subscription_tier_display {
        let was_api_key = app.is_api_key_auth;
        let is_key = super::super::app_view::is_api_key_label(&v);
        app.is_api_key_auth = is_key;
        app.usage_visible = !is_key && app.team_name.is_none();
        app.sync_billing_surface_to_agents();
        app.subscription_tier = Some(v);
        app.apply_tier_restrictions();
        // Leaving API Key → free/X Basic without a voice field: drop force-on.
        // Paid tiers keep voice; remote settings may send voice_mode_enabled later.
        if was_api_key
            && !is_key
            && update.voice_mode_enabled.is_none()
            && app
                .subscription_tier
                .as_deref()
                .is_some_and(xai_grok_shell::tier::is_restricted_tier_name)
        {
            app.voice_reset();
            app.voice_ui_active = false;
            app.apply_voice_mode_enabled(false);
        }
    }
    if let Some(remote_v) = update.voice_mode_enabled {
        let v = crate::app::resolve_voice_mode_live(Some(remote_v), app.is_api_key_auth);
        if !v {
            app.voice_reset();
            app.voice_ui_active = false;
        }
        app.apply_voice_mode_enabled(v);
    } else {
        app.ensure_voice_for_api_key();
    }
    // TODO: extract resolve_session_picker_grouped helper (duplicates event_loop.rs:143-160)
    // Respect env var > config > remote precedence (mirrors event_loop.rs startup).
    if let Some(remote_val) = update.session_picker_grouped {
        let resolved = std::env::var("GROK_SESSION_PICKER_GROUPED")
            .ok()
            .and_then(|v| match v.as_str() {
                "1" | "true" => Some(true),
                "0" | "false" => Some(false),
                _ => None,
            })
            .or_else(|| {
                xai_grok_shell::config::load_effective_config()
                    .ok()
                    .and_then(|cfg| cfg.get("cli")?.get("session_picker_grouped")?.as_bool())
            })
            .unwrap_or(remote_val);
        app.session_picker_grouped = resolved;
    }
    if let Some(v) = update.subscription_watch_interval_secs {
        app.subscription_watch_interval_secs = Some(v);
    }

    // Gate update logic:
    // - allow_access == Some(true): explicitly granted → lift the gate
    // - gate_message.is_some(): server sent a new message → impose/update
    // - Neither condition met: don't touch the gate. In particular,
    //   allow_access=Some(false) without a gate_message must NOT clear the
    //   gate (gate_from_settings returns None when gate_message is absent,
    //   which would incorrectly lift an existing gate).
    if update.allow_access == Some(true) {
        let effs = app.lift_gate();
        app.pending_effects.extend(effs);
    } else if let Some(msg) = update.gate_message.as_ref()
        && !msg.is_empty()
    {
        // (An empty gate_message would only clear the gate message text, NOT
        // access, so it intentionally does not touch the gate here.)
        let effs = app.impose_gate(xai_grok_shell::auth::GateInfo {
            message: msg.clone(),
            url: update.gate_url.clone(),
            label: update.gate_label.clone(),
        });
        app.pending_effects.extend(effs);
    }

    // Load config layers once for tips + group_tool_verbs +
    // collapsed_edit_blocks resolution. Loaded unconditionally: the UI flags
    // re-resolve on every update (see below), and updates are rare (post-auth
    // refresh, `/new`), so three small TOML reads are fine.
    let (requirements, user_config, managed_config) = (
        xai_grok_shell::config::load_merged_requirements(),
        xai_grok_shell::config::load_from_disk().ok(),
        xai_grok_shell::config::load_managed_config().ok(),
    );

    // Local layers may beat remote — re-resolve the full chain into the render
    // cache (mirrors the event_loop.rs startup resolve). Runs on None too: the
    // shell always publishes this field from its live remote tier, so None
    // means remote settings cleared it (or an older shell that cannot deliver the
    // remote tier at all) — either way resolving without a remote value is
    // correct, and it reverts a previously cached remote enable back to the
    // local/default (off) resolution instead of leaving Some(true) stuck
    // until restart.
    let remote = xai_grok_shell::util::config::RemoteSettings {
        group_tool_verbs: update.group_tool_verbs,
        ..Default::default()
    };
    let resolved = xai_grok_shell::util::config::resolve_group_tool_verbs(
        requirements.as_ref(),
        user_config.as_ref(),
        managed_config.as_ref(),
        Some(&remote),
    )
    .value;
    // On a real flip, re-fold every live transcript (mirrors dispatch's
    // set_group_tool_verbs_inner); unchanged values keep `/new` cheap.
    // Stale expansion ids describe the old grouping shape — drop them so the
    // re-fold can't reopen a verb slot expanded or mark a coincident dense
    // group expanded (see `clear_group_expansion`).
    if resolved != crate::appearance::cache::load_group_tool_verbs() {
        crate::appearance::cache::set_group_tool_verbs(resolved);
        for agent in app.agents.values_mut() {
            agent.scrollback.clear_group_expansion();
            agent.scrollback.invalidate_heights();
            for child in agent.subagent_views.values_mut() {
                child.scrollback.clear_group_expansion();
                child.scrollback.invalidate_heights();
            }
        }
    }

    // Same None-reverts contract as group_tool_verbs above: re-resolve the
    // full local chain with the pushed remote tier so a cleared remote settings
    // field falls back to local/default instead of staying latched.
    let remote = xai_grok_shell::util::config::RemoteSettings {
        collapsed_edit_blocks: update.collapsed_edit_blocks,
        ..Default::default()
    };
    let resolved = xai_grok_shell::util::config::resolve_collapsed_edit_blocks(
        requirements.as_ref(),
        user_config.as_ref(),
        managed_config.as_ref(),
        Some(&remote),
    )
    .value;
    // On a real flip, re-materialize on-default Edit rows + repaint suffixes
    // in every live transcript (mirrors dispatch's
    // set_collapsed_edit_blocks_inner); unchanged values keep `/new` cheap.
    let prev = crate::appearance::cache::load_collapsed_edit_blocks();
    if resolved != prev {
        crate::appearance::cache::set_collapsed_edit_blocks(resolved);
        for agent in app.agents.values_mut() {
            agent
                .scrollback
                .apply_collapsed_edit_blocks_flip(prev, resolved);
            for child in agent.subagent_views.values_mut() {
                child
                    .scrollback
                    .apply_collapsed_edit_blocks_flip(prev, resolved);
            }
        }
    }

    // Re-resolve tips from config layers + the updated remote tips.
    if let Some(remote_tips) = update.tips {
        use xai_grok_shell::util::config::resolve_tips;

        app.tips = resolve_tips(
            requirements.as_ref(),
            user_config.as_ref(),
            managed_config.as_ref(),
            Some(&remote_tips),
        );
        if !app.tips.is_empty() {
            let grok_home = xai_grok_tools::util::grok_home::grok_home();
            app.tip = xai_grok_shell::util::tips::pick_and_advance(&app.tips, &grok_home);
        } else {
            app.tip = None;
        }
    }

    tracing::info!("settings updated via x.ai/settings/update");
    true
}

/// Re-arm the soft-defaulted launch mode from a pushed `permission_mode`
/// (TOML `[ui]` > remote > Ask), for the next `/new` only — live sessions are
/// untouched and nothing is persisted. `effective_ui` is injected so the
/// resolve is deterministic under test. Enforcement gating reuses the app's
/// startup snapshots (`yolo_policy_block`, `auto_mode_gate`); the agent's
/// permission manager re-clamps authoritatively at decision time.
pub(super) fn apply_soft_default_permission_mode(
    app: &mut AppView,
    effective_ui: Option<&toml::Value>,
    remote: Option<&str>,
) {
    let mode = xai_grok_shell::util::config::resolve_permission_mode(effective_ui, remote);
    app.default_yolo = mode.is_always_approve() && app.yolo_policy_block.is_none();
    let auto = mode.is_auto() && app.auto_mode_gate && !app.default_yolo;
    app.current_ui.permission_mode = Some(if auto {
        "auto".to_string()
    } else if app.default_yolo {
        "always-approve".to_string()
    } else {
        xai_grok_shell::util::config::resolved_display_permission_mode(effective_ui, remote)
            .to_string()
    });
}

/// Tell live sessions to leave Auto on the mid-session kill-switch: fire the
/// `x.ai/yolo_mode_changed` notification the agent maps to
/// `SetAutoMode { enabled: false }`, fire-and-forget over the shared ACP channel.
/// The notification is CLIENT-scoped (the agent applies it to every session of
/// the sending client), so one send covers all affected sessions. `yolo_mode` is
/// deliberately OMITTED — the agent skips the yolo branch when the key is absent,
/// so a sibling tab's always-approve is preserved; only auto is cleared.
pub(super) fn notify_sessions_leave_auto(app: &AppView, session_ids: &[acp::SessionId]) {
    if session_ids.is_empty() {
        return;
    }
    let params = serde_json::json!({
        "auto_mode": false,
        "permission_mode": "ask",
    });
    let notification = acp::ExtNotification::new(
        "x.ai/yolo_mode_changed",
        serde_json::value::to_raw_value(&params)
            .expect("serialize yolo_mode_changed params")
            .into(),
    );
    let (response_tx, _response_rx) = tokio::sync::oneshot::channel();
    let args = xai_acp_lib::AcpArgs {
        request: notification,
        response_tx,
    };
    let _ = app.acp_tx.send(args.into());
}

/// Handle `x.ai/sessions/changed` — the leader broadcasts roster
/// upserts/removals to all clients (FleetView dashboard).
pub(super) fn handle_sessions_changed(notif: &acp::ExtNotification, app: &mut AppView) -> bool {
    let Ok(changed) = serde_json::from_str::<crate::app::roster::RosterChanged>(notif.params.get())
    else {
        tracing::warn!("Failed to parse x.ai/sessions/changed");
        return false;
    };
    let mut affected = false;
    for entry in changed.upserted {
        app.upsert_roster_entry(entry);
        affected = true;
    }
    for sid in changed.removed {
        app.remove_roster_entry(&sid);
        affected = true;
    }
    affected
}

pub(super) fn handle_announcements_update(notif: &acp::ExtNotification, app: &mut AppView) -> bool {
    let Ok(parsed) =
        serde_json::from_str::<xai_grok_announcements::AnnouncementsRefreshed>(notif.params.get())
    else {
        return false;
    };

    if parsed.r#gen <= app.announcements_last_gen {
        return false;
    }

    // Re-merge config layers like startup (and the pre-unification settings
    // branch) did: the push carries the remote list only, and a wholesale
    // replace would drop requirements/user/managed announcements and let the
    // prune erase their persisted hide keys. Same disk reads the settings
    // branch performed; pushes are rare.
    let requirements = xai_grok_shell::config::load_merged_requirements();
    let user_config = xai_grok_shell::config::load_from_disk().ok();
    let managed_config = xai_grok_shell::config::load_managed_config().ok();
    apply_announcements_update(
        app,
        parsed.r#gen,
        &parsed.announcements,
        requirements.as_ref(),
        user_config.as_ref(),
        managed_config.as_ref(),
    );
    true
}

/// Apply half of [`handle_announcements_update`], with config layers injected
/// so the merge/prune behavior is unit-testable without disk state.
/// `resolve_announcements` honors `GROK_ANNOUNCEMENTS_OVERRIDE` first, so a
/// backend push can't reintroduce announcements when the override is set.
pub(super) fn apply_announcements_update(
    app: &mut AppView,
    next_gen: u64,
    remote: &[xai_grok_announcements::RemoteAnnouncement],
    requirements: Option<&toml::Value>,
    user_config: Option<&toml::Value>,
    managed_config: Option<&toml::Value>,
) {
    let merged = xai_grok_shell::util::config::resolve_announcements(
        requirements,
        user_config,
        managed_config,
        Some(remote),
    );
    let announcements = xai_grok_announcements::filter_expired(merged);

    app.announcement = match app.announcement.as_ref() {
        Some(current) => announcements
            .iter()
            .find(|a| *a == current)
            .cloned()
            .or_else(|| pick_random_announcement(&announcements)),
        None => pick_random_announcement(&announcements),
    };
    app.active_announcements = announcements;
    app.announcements_last_gen = next_gen;
    // Opportunistic per-ID prune on a real update (never per frame) so the hidden set cannot grow unboundedly.
    if xai_grok_announcements::prune_hidden_announcement_ids(
        &mut app.hidden_announcement_ids,
        &app.active_announcements,
    ) {
        app.pending_effects
            .push(Effect::PersistAnnouncementsHidden {
                hidden_ids: app.hidden_announcement_ids.clone(),
            });
    }
    app.sync_session_announcement_slash_gate();
}

pub(super) fn pick_random_announcement(
    announcements: &[xai_grok_announcements::RemoteAnnouncement],
) -> Option<xai_grok_announcements::RemoteAnnouncement> {
    if announcements.is_empty() {
        return None;
    }
    use rand::Rng;
    let idx = rand::rng().random_range(0..announcements.len());
    announcements.get(idx).cloned()
}

/// Deserialization type for the `x.ai/settings/update` notification payload.
///
/// This is intentionally a separate struct from `SettingsUpdateNotification` in
/// `xai-grok-shell/src/agent/mvp_agent.rs`. The shell side derives `Serialize`
/// and owns the canonical field set from `RemoteSettings`; this pager side
/// derives `Deserialize` and selectively consumes only the fields relevant to
/// the TUI. Keeping them separate avoids coupling the pager to shell internals
/// and lets each side evolve independently (e.g. adding a shell-only field
/// doesn't require a pager change). All fields are `Option` with
/// `#[serde(default)]` so that partial updates and forward-compatible additions
/// are handled gracefully.
///
/// **Keep in sync** with field names/types in `SettingsUpdateNotification` at
/// `xai-grok-shell/src/agent/mvp_agent.rs` when adding fields that both sides
/// need.
#[derive(serde::Deserialize)]
pub(super) struct PagerSettingsUpdate {
    #[serde(default)]
    show_resolved_model: Option<bool>,
    #[serde(default)]
    sharing_enabled: Option<bool>,
    #[serde(default)]
    privacy_notice_rollout: Option<bool>,
    #[serde(default)]
    privacy_banner_reshow_days: Option<u64>,
    #[serde(default)]
    voice_mode_enabled: Option<bool>,
    #[serde(default)]
    session_picker_grouped: Option<bool>,
    #[serde(default)]
    tips: Option<Vec<String>>,
    // `announcements` is deliberately NOT consumed here: every shell writer of
    // remote_settings also emits gen-ordered `x.ai/announcements/update`
    // (emit_announcements_if_changed), and a gen-less apply on this path could
    // clobber a newer push. Single ingest path: handle_announcements_update.
    #[serde(default)]
    gate_message: Option<String>,
    #[serde(default)]
    gate_url: Option<String>,
    #[serde(default)]
    gate_label: Option<String>,
    #[serde(default)]
    allow_access: Option<bool>,
    #[serde(default)]
    subscription_tier_display: Option<String>,
    #[serde(default)]
    auto_permission_mode_enabled: Option<bool>,
    /// Soft-default permission mode. Presence-aware: omit = no update,
    /// `null` = recompute with remote=None, string = that soft-default.
    /// Omission happens with older shells that predate the field (they can
    /// never clear a mode they don't know about) — that version skew is why
    /// this is tri-state instead of a plain `Option`.
    #[serde(default, deserialize_with = "deserialize_presence_aware_string")]
    permission_mode: Option<Option<String>>,
    #[serde(default)]
    group_tool_verbs: Option<bool>,
    #[serde(default)]
    collapsed_edit_blocks: Option<bool>,
    #[serde(default)]
    subscription_watch_interval_secs: Option<u64>,
}

/// Presence-aware string: omit → `None` (`#[serde(default)]`), null →
/// `Some(None)`, string → `Some(Some(_))`.
fn deserialize_presence_aware_string<'de, D>(
    deserializer: D,
) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::<String>::deserialize(deserializer)?))
}

#[cfg(test)]
mod presence_aware_dto_tests {
    use super::*;

    #[derive(Deserialize)]
    struct Probe {
        #[serde(default, deserialize_with = "deserialize_presence_aware_string")]
        permission_mode: Option<Option<String>>,
    }

    #[test]
    fn permission_mode_dto_distinguishes_omit_from_null() {
        let omit: Probe = serde_json::from_value(serde_json::json!({
            "show_resolved_model": true,
        }))
        .unwrap();
        assert_eq!(omit.permission_mode, None, "omit must be None (no update)");

        let null_v: Probe = serde_json::from_value(serde_json::json!({
            "permission_mode": null,
        }))
        .unwrap();
        assert_eq!(
            null_v.permission_mode,
            Some(None),
            "explicit null must be Some(None)"
        );

        let some_v: Probe = serde_json::from_value(serde_json::json!({
            "permission_mode": "always-approve",
        }))
        .unwrap();
        assert_eq!(
            some_v.permission_mode,
            Some(Some("always-approve".into())),
            "string must be Some(Some(_))"
        );
    }
}

#[cfg(test)]
mod providers_update_handler_tests {
    use super::handle_providers_update;
    use crate::app::agent::AgentId;
    use crate::app::app_view::AppView;
    use crate::views::modal::ActiveModal;
    use crate::views::providers_modal::{ProviderModalMode, ProviderModalState};
    use agent_client_protocol as acp;
    use xai_grok_shell::provider_registry::management::dto::{
        CredentialPresence, ProviderDetailDto, RegistryGeneration,
    };

    fn minimal_detail(id: &str, generation: u64) -> ProviderDetailDto {
        ProviderDetailDto {
            id: id.into(),
            display_name: Some("Lab".into()),
            kind: "openai_compatible".into(),
            enabled: true,
            is_built_in: false,
            is_configured: true,
            is_editable: true,
            base_url: Some("http://127.0.0.1:9/v1".into()),
            admin_base_url: None,
            default_backend: None,
            auth_scheme: None,
            env_key: None,
            admin_env_key: None,
            catalog_enabled: false,
            capability_mode: None,
            catalog_ttl_secs: None,
            request_timeout_secs: None,
            pool_max_idle: None,
            pool_idle_timeout_secs: None,
            pool_connect_timeout_secs: None,
            organization: None,
            project: None,
            api_surface: None,
            credential_route: None,
            api_backend: None,
            auth_provider: None,
            extra_headers: Default::default(),
            capabilities: Default::default(),
            openrouter_fallback_models: vec![],
            openrouter_data_collection: None,
            openrouter_require_parameters: None,
            openrouter_allow_fallbacks: None,
            openrouter_zdr: None,
            openrouter_order: vec![],
            openrouter_only: vec![],
            openrouter_ignore: vec![],
            openrouter_quantizations: vec![],
            openrouter_sort: None,
            openrouter_pacing: false,
            max_completion_tokens: None,
            openrouter_plugin_ids: vec![],
            credentials: CredentialPresence::default(),
            generation: RegistryGeneration(generation),
            warnings: vec![],
            unsupported_edit_reason: None,
            incarnation: None,
            tombstone_blocks_readd: false,
        }
    }

    fn notif(generation: u64, changed_ids: &[&str]) -> acp::ExtNotification {
        let params = serde_json::json!({
            "schema_version": 1,
            "generation": generation,
            "changed_ids": changed_ids,
            "changed_fields": ["enabled"],
        });
        let raw = serde_json::value::to_raw_value(&params).unwrap();
        acp::ExtNotification::new("x.ai/providers/update", raw.into())
    }

    #[test]
    fn dirty_editor_conflicts_clean_list_auto_loads() {
        use crate::acp::model_state::ModelState;
        use crate::app::agent::{AgentSession, AgentState};
        use crate::app::agent_view::AgentView;
        use crate::scrollback::state::ScrollbackState;
        use std::path::PathBuf;

        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = AppView::new(tx.clone(), ModelState::default(), Vec::new());
        let agent_id = AgentId(0);
        let session = AgentSession {
            id: agent_id,
            acp_tx: tx,
            session_id: Some(acp::SessionId::new("sess-providers")),
            models: ModelState::default(),
            state: AgentState::Idle,
            tracker: crate::acp::tracker::AcpUpdateTracker::new(),
            cwd: PathBuf::from("/tmp"),
            is_worktree: false,
            forked_from: None,
            pending_prompts: std::collections::VecDeque::new(),
            next_queue_id: 0,
            yolo_mode: false,
            auto_mode: false,
            prompt_history: Vec::new(),
            prompt_history_loading: false,
            loading_replay: false,
            restore_degree: None,
            rate_limited: false,
            model_incompatible: false,
            credit_limit_blocked: false,
            free_usage_blocked: false,
            available_commands: Vec::new(),
            available_commands_generation: 0,
            available_tools: None,
            tool_catalog: None,
            model_switch_pending: false,
            user_model_preference: None,
            deferred_model_switch: None,
            bg_tasks: std::collections::BTreeMap::new(),
            bg_tool_call_to_task: std::collections::HashMap::new(),
            scheduled_tasks: std::collections::HashMap::new(),
            in_flight_prompt: None,
            compact_held_prompt: None,
            current_prompt_id: None,
            created_via_new: false,
        };
        let mut agent = AgentView::new(session, ScrollbackState::new());
        agent.active_modal = Some(ActiveModal::Providers {
            state: Box::new(ProviderModalState::new()),
        });
        app.agents.insert(agent_id, agent);
        crate::app::dispatch::switch_to_agent(
            &mut app,
            agent_id,
            crate::app::dispatch::SwitchCause::New,
        );
        app.pending_effects.clear();
        assert!(handle_providers_update(&notif(5, &["lab"]), &mut app));
        assert!(
            app.pending_effects.iter().any(|e| matches!(
                e,
                crate::app::actions::Effect::ProviderOperation {
                    operation: crate::app::actions::ProviderOperation::LoadListSnapshot,
                    ..
                }
            )),
            "clean list must auto LoadListSnapshot"
        );

        // Dirty editor → conflict, drafts preserved.
        {
            let agent = app.agents.get_mut(&agent_id).unwrap();
            let mut state = ProviderModalState::new();
            state.open_editor(minimal_detail("lab", 1));
            if let Some(ed) = state.editor_mut() {
                ed.clone_id_draft = "dirty-draft".into();
                assert!(ed.is_dirty());
            }
            agent.active_modal = Some(ActiveModal::Providers {
                state: Box::new(state),
            });
        }
        app.pending_effects.clear();
        assert!(handle_providers_update(&notif(9, &["lab"]), &mut app));
        let agent = app.agents.get(&agent_id).unwrap();
        let ActiveModal::Providers { state } = agent.active_modal.as_ref().unwrap() else {
            panic!("providers modal");
        };
        let ProviderModalMode::Editor(ed) = &state.mode else {
            panic!("editor mode");
        };
        assert!(ed.conflict.is_some(), "dirty editor must enter conflict");
        assert_eq!(ed.clone_id_draft, "dirty-draft", "drafts must stay intact");
    }
}

#[cfg(test)]
mod prime_index_update_handler_tests {
    use super::handle_prime_index_update;
    use crate::app::app_view::AppView;
    use agent_client_protocol as acp;

    fn notif(params: serde_json::Value) -> acp::ExtNotification {
        let raw = serde_json::value::to_raw_value(&params).unwrap();
        acp::ExtNotification::new("x.ai/prime/index/update", raw.into())
    }

    #[test]
    fn stale_and_zero_generation_are_rejected() {
        use crate::acp::model_state::ModelState;
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = AppView::new(tx, ModelState::default(), Vec::new());
        app.prime_index_last_generation = 4;
        assert!(handle_prime_index_update(
            &notif(serde_json::json!({})),
            &mut app
        ));
        assert_eq!(app.prime_index_last_generation, 4);
        assert!(handle_prime_index_update(
            &notif(serde_json::json!({"generation": 3, "apiVersion": 1})),
            &mut app
        ));
        assert_eq!(app.prime_index_last_generation, 4);
        assert!(handle_prime_index_update(
            &notif(serde_json::json!({"generation": 5, "apiVersion": 1})),
            &mut app
        ));
        assert_eq!(app.prime_index_last_generation, 5);
    }

    #[test]
    fn unknown_api_version_advances_watermark_without_job() {
        use crate::acp::model_state::ModelState;
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = AppView::new(tx, ModelState::default(), Vec::new());
        assert!(handle_prime_index_update(
            &notif(serde_json::json!({"generation": 2, "apiVersion": 9})),
            &mut app
        ));
        assert_eq!(app.prime_index_last_generation, 2);
    }

    fn job_update(generation: u64, notify_seq: u64, done: u64, state: &str) -> serde_json::Value {
        serde_json::json!({
            "schemaVersion": 1,
            "apiVersion": 1,
            "generation": generation,
            "notifySeq": notify_seq,
            "fingerprintShort": "abc123def456",
            "changedFields": ["job"],
            "job": {
                "apiVersion": 1,
                "jobId": "j1",
                "kind": "backfill",
                "collection": "skills",
                "state": state,
                "generation": generation,
                "fingerprintShort": "abc123def456",
                "done": done,
                "total": 3,
                "confirmConfiguredProfile": false
            }
        })
    }

    #[test]
    fn same_generation_job_progress_updates_footer() {
        use crate::views::extensions_modal::{ExtensionsModalState, ExtensionsTab};
        let mut app = crate::app::app_view::tests::test_app_with_agent();
        app.prime_index = xai_grok_shell::session::prime::PrimeIndexCapabilities::SUPPORTED;
        let agent_id = crate::app::agent::AgentId(0);
        if let Some(agent) = app.agents.get_mut(&agent_id) {
            let mut modal = ExtensionsModalState::new(ExtensionsTab::Skills);
            modal.prime_index_capable = true;
            modal.picker_state.set_query("commit");
            modal.prime_index = Some(xai_grok_shell::session::prime::PrimeIndexStatus {
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
                    route_id: None,
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
                job: None,
                configured_route: None,
                capabilities: xai_grok_shell::session::prime::PrimeIndexCapabilities::SUPPORTED,
                unchanged: false,
            });
            agent.extensions_modal = Some(modal);
        }
        assert!(handle_prime_index_update(
            &notif(job_update(4, 1, 1, "running")),
            &mut app
        ));
        assert_eq!(app.prime_index_last_generation, 4);
        assert_eq!(app.prime_index_last_notify_seq, 1);
        assert!(handle_prime_index_update(
            &notif(job_update(4, 2, 2, "failed")),
            &mut app
        ));
        assert_eq!(app.prime_index_last_notify_seq, 2);
        let agent = app.agents.get(&agent_id).expect("agent");
        let modal = agent.extensions_modal.as_ref().expect("skills modal");
        assert_eq!(modal.picker_state.query(), "commit");
        let footer = modal.prime_index_footer_line(true).expect("compact footer");
        assert!(footer.contains("failed"), "{footer}");
        assert!(footer.contains("2/3"), "{footer}");
        assert!(
            footer.len() < 80,
            "compact footer must fit a narrow terminal: {footer}"
        );
    }

    #[test]
    fn generation_zero_job_update_is_delivered() {
        use crate::views::extensions_modal::{ExtensionsModalState, ExtensionsTab};
        let mut app = crate::app::app_view::tests::test_app_with_agent();
        app.prime_index = xai_grok_shell::session::prime::PrimeIndexCapabilities::SUPPORTED;
        let agent_id = crate::app::agent::AgentId(0);
        if let Some(agent) = app.agents.get_mut(&agent_id) {
            agent.extensions_modal = Some(ExtensionsModalState::new(ExtensionsTab::Skills));
        }
        assert!(handle_prime_index_update(
            &notif(job_update(0, 1, 1, "running")),
            &mut app
        ));
        let job = app
            .agents
            .get(&agent_id)
            .and_then(|a| a.extensions_modal.as_ref())
            .and_then(|m| m.prime_index.as_ref())
            .and_then(|s| s.job.as_ref());
        assert_eq!(job.map(|j| j.done), Some(1));
    }

    #[test]
    fn malicious_job_update_is_sanitized_and_old_schema_is_version_safe() {
        use crate::views::extensions_modal::{ExtensionsModalState, ExtensionsTab};
        let mut app = crate::app::app_view::tests::test_app_with_agent();
        app.prime_index = xai_grok_shell::session::prime::PrimeIndexCapabilities::SUPPORTED;
        let agent_id = crate::app::agent::AgentId(0);
        if let Some(agent) = app.agents.get_mut(&agent_id) {
            let mut modal = ExtensionsModalState::new(ExtensionsTab::Skills);
            modal.prime_index_capable = true;
            modal.picker_state.set_query("commit");
            agent.extensions_modal = Some(modal);
        }
        let raw = "http://127.0.0.1/v1";
        assert!(handle_prime_index_update(
            &notif(serde_json::json!({
                "schemaVersion": 1,
                "apiVersion": 1,
                "generation": 4,
                "notifySeq": 1,
                "fingerprintShort": "abc123def456",
                "changedFields": ["job"],
                "job": {
                    "apiVersion": 1,
                    "jobId": "j1",
                    "kind": "backfill",
                    "collection": "skills",
                    "state": "failed",
                    "generation": 4,
                    "fingerprintShort": "abc123def456",
                    "done": 0,
                    "total": 3,
                    "confirmConfiguredProfile": false,
                    "configuredRoute": raw,
                    "failure": format!("confirm_required:{raw}")
                }
            })),
            &mut app
        ));
        let agent = app.agents.get(&agent_id).expect("agent");
        let modal = agent.extensions_modal.as_ref().expect("skills");
        assert_eq!(modal.picker_state.query(), "commit");
        let job = modal.prime_index.as_ref().and_then(|s| s.job.as_ref());
        let json = serde_json::to_string(&job).expect("job json");
        assert!(!json.contains("127.0.0.1"), "{json}");
        assert!(!json.contains("http://"), "{json}");
        let footer = modal.prime_index_footer_line(true).expect("footer");
        assert!(
            footer.contains("unavailable")
                || footer.contains("failed")
                || footer.contains("confirm"),
            "{footer}"
        );
        assert!(!footer.contains("127.0.0.1"), "{footer}");
        app.prime_index_last_generation = 9;
        app.prime_index_last_notify_seq = 0;
        assert!(handle_prime_index_update(
            &notif(serde_json::json!({"generation": 3, "apiVersion": 1})),
            &mut app
        ));
        assert_eq!(app.prime_index_last_generation, 9);
        assert!(handle_prime_index_update(
            &notif(serde_json::json!({"generation": 11, "apiVersion": 1})),
            &mut app
        ));
        assert_eq!(app.prime_index_last_generation, 11);
    }

    #[test]
    fn decreasing_generation_with_advancing_notify_seq_is_applied() {
        use crate::views::extensions_modal::{ExtensionsModalState, ExtensionsTab};
        let mut app = crate::app::app_view::tests::test_app_with_agent();
        app.prime_index = xai_grok_shell::session::prime::PrimeIndexCapabilities::SUPPORTED;
        let prior = 1u64 << 62;
        app.prime_index_last_generation = prior;
        app.prime_index_last_notify_seq = 1;
        let agent_id = crate::app::agent::AgentId(0);
        if let Some(agent) = app.agents.get_mut(&agent_id) {
            let mut modal = ExtensionsModalState::new(ExtensionsTab::Skills);
            modal.prime_index_capable = true;
            modal.prime_index = Some(xai_grok_shell::session::prime::PrimeIndexStatus {
                api_version: 1,
                generation: prior,
                fingerprint_short: "oldfpoldfp12".into(),
                skills: xai_grok_shell::session::prime::PrimeIndexCollectionStatus {
                    collection: "skills".into(),
                    generation: prior,
                    fingerprint_short: "oldfpoldfp12".into(),
                    item_count: 3,
                    vector_count: 1,
                    missing_vectors: 2,
                    readiness: "pending".into(),
                    route_id: None,
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
                job: None,
                configured_route: None,
                capabilities: xai_grok_shell::session::prime::PrimeIndexCapabilities::SUPPORTED,
                unchanged: false,
            });
            agent.extensions_modal = Some(modal);
        }
        assert!(handle_prime_index_update(
            &notif(job_update(123, 2, 2, "running")),
            &mut app
        ));
        assert_eq!(app.prime_index_last_generation, 123);
        assert_eq!(app.prime_index_last_notify_seq, 2);
        let job = app
            .agents
            .get(&agent_id)
            .and_then(|a| a.extensions_modal.as_ref())
            .and_then(|m| m.prime_index.as_ref())
            .and_then(|s| s.job.as_ref());
        assert_eq!(job.map(|j| j.done), Some(2));
        assert!(handle_prime_index_update(
            &notif(job_update(prior, 2, 3, "running")),
            &mut app
        ));
        let job = app
            .agents
            .get(&agent_id)
            .and_then(|a| a.extensions_modal.as_ref())
            .and_then(|m| m.prime_index.as_ref())
            .and_then(|s| s.job.as_ref());
        assert_eq!(
            job.map(|j| j.done),
            Some(2),
            "equal notifySeq must be rejected even when generation is larger"
        );
    }

    #[test]
    fn completed_same_generation_job_enqueues_status_fetch() {
        use crate::app::actions::Effect;
        use crate::views::extensions_modal::{ExtensionsModalState, ExtensionsTab};
        let mut app = crate::app::app_view::tests::test_app_with_agent();
        app.prime_index = xai_grok_shell::session::prime::PrimeIndexCapabilities::SUPPORTED;
        app.prime_index_last_generation = 4;
        app.prime_index_last_notify_seq = 1;
        let agent_id = crate::app::agent::AgentId(0);
        if let Some(agent) = app.agents.get_mut(&agent_id) {
            let mut modal = ExtensionsModalState::new(ExtensionsTab::Skills);
            modal.prime_index_capable = true;
            modal.prime_index = Some(xai_grok_shell::session::prime::PrimeIndexStatus {
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
                    route_id: None,
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
                job: None,
                configured_route: None,
                capabilities: xai_grok_shell::session::prime::PrimeIndexCapabilities::SUPPORTED,
                unchanged: false,
            });
            agent.extensions_modal = Some(modal);
        }
        assert!(handle_prime_index_update(
            &notif(job_update(4, 2, 3, "completed")),
            &mut app
        ));
        assert!(
            app.pending_effects.iter().any(|e| matches!(
                e,
                Effect::FetchPrimeIndexStatus {
                    expected_generation: Some(4),
                    ..
                }
            )),
            "terminal same-gen jobs must refresh compact counts, got {:?}",
            app.pending_effects
        );
    }
}
