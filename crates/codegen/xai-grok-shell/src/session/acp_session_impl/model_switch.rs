use super::*;
use crate::remote::DEFAULT_CONTEXT_WINDOW;
use xai_chat_state::conversation_util::replace_or_insert_system_head;
impl SessionActor {
    pub(super) async fn handle_set_session_model(
        &self,
        selection_model_id: acp::ModelId,
        inference_config: xai_grok_inference::InferenceConfig,
        use_concise: bool,
        apply_prompt_override: bool,
        skip_prompt_rewrite: bool,
        auto_compact_threshold_percent: u8,
        execution_backend: crate::agent::execution_backend::ExecutionBackend,
    ) -> Result<acp::ModelId, acp::Error> {
        // Canonical selection is session-scoped; never take the upstream wire slug
        // from InferenceConfig.model as the selection id.
        let model_id = selection_model_id;
        let prepared_external_runtime = if execution_backend.is_native() {
            None
        } else if let Some(envelope) = self.external_runtime.borrow().clone() {
            let expected_kind = execution_backend.external_kind().ok_or_else(|| {
                acp::Error::invalid_params().data("external execution backend kind is missing")
            })?;
            if envelope.kind != expected_kind {
                return Err(acp::Error::invalid_params().data(format!(
                    "external runtime envelope kind '{}' does not match execution backend '{}'",
                    envelope.kind, expected_kind
                )));
            }
            envelope
                .validate()
                .map_err(|e| acp::Error::invalid_params().data(e.to_string()))?;
            Some(envelope)
        } else if let Some(kind) = execution_backend.external_kind() {
            let envelope = crate::agent::external_runtime::ExternalRuntimeEnvelope::for_kind(kind);
            envelope
                .validate()
                .map_err(|e| acp::Error::invalid_params().data(e.to_string()))?;
            Some(envelope)
        } else {
            None
        };
        // Resolve/assert the exact canonical route BEFORE any mutation of chat
        // state, credentials, compaction, selection, image budget, external
        // runtime, or sampler. Disabled, tombstoned, or otherwise unusable
        // routes fail closed and leave session state unchanged.
        let home = crate::util::grok_home::grok_home();
        let route = crate::session::route_context::resolve_for_models_manager_with_selection(
            &inference_config,
            &self.models_manager,
            model_id.0.as_ref(),
            Some(home.as_path()),
        )
        .map_err(|e| {
            acp::Error::invalid_params()
                .data(format!("provider route unusable for model switch: {e}"))
        })?;
        let prev_backend = self.execution_backend.get();
        // When leaving external mode, or switching to a different external kind,
        // shut down the retained runtime (bridge + temp resources + child).
        let backend_incompatible = match (
            prev_backend.external_kind(),
            execution_backend.external_kind(),
        ) {
            (Some(a), Some(b)) if a != b => true,
            (Some(_), None) => true,
            _ => false,
        };
        if backend_incompatible {
            self.shutdown_external_agent_runtime().await;
        }
        self.execution_backend.set(execution_backend);
        *self.external_runtime.borrow_mut() = prepared_external_runtime;
        let new_context_window = self.compaction.context_window_override.unwrap_or_else(|| {
            std::num::NonZeroU64::new(inference_config.context_window).unwrap_or_else(|| {
                std::num::NonZeroU64::new(DEFAULT_CONTEXT_WINDOW)
                    .expect("DEFAULT_CONTEXT_WINDOW is non-zero")
            })
        });
        let prev_threshold = self.compaction.threshold_percent.get();
        if prev_threshold != auto_compact_threshold_percent {
            tracing::info!(
                session_id = %self.session_info.id.0,
                new_model = %inference_config.model,
                old_threshold = prev_threshold,
                new_threshold = auto_compact_threshold_percent,
                "auto_compact_threshold_percent updated for model switch"
            );
        }
        self.compaction
            .threshold_percent
            .set(auto_compact_threshold_percent);
        self.supports_backend_search
            .set(inference_config.supports_backend_search);
        self.compactions_remaining
            .set(inference_config.compactions_remaining);
        self.compaction_at_tokens
            .set(inference_config.compaction_at_tokens);
        self.openrouter_fallback_models
            .replace(inference_config.openrouter_fallback_models.clone());
        self.openrouter_provider_preferences
            .replace(inference_config.openrouter_provider_preferences.clone());
        self.openrouter_plugins
            .replace(inference_config.openrouter_plugins.clone());
        self.openrouter_pacing
            .set(inference_config.openrouter_pacing);
        xai_grok_telemetry::unified_log::info(
            "backend_search: model switch",
            Some(self.session_info.id.0.as_ref()),
            Some(serde_json::json!({
                "new_model": &inference_config.model,
                "api_backend": format!("{:?}", inference_config.api_backend),
                "supports_backend_search": inference_config.supports_backend_search,
            })),
        );
        self.chat_state_handle
            .update_inference_settings_with_image_budget(
                xai_grok_inference_types::InferenceSettings {
                    base_url: inference_config.base_url.clone(),
                    model: inference_config.model.clone(),
                    max_completion_tokens: inference_config.max_completion_tokens,
                    temperature: inference_config.temperature,
                    top_p: inference_config.top_p,
                    api_backend: inference_config.api_backend.clone(),
                    extra_headers: inference_config.extra_headers.clone(),
                    context_window: new_context_window,
                    reasoning_effort: inference_config.reasoning_effort,
                    stream_tool_calls: Some(inference_config.stream_tool_calls),
                    supports_native_schema: inference_config.supports_native_schema,
                    supports_strict_tools: inference_config.supports_strict_tools,
                    supports_image_input: inference_config.supports_image_input,
                    supports_audio_input: inference_config.supports_audio_input,
                    supports_video_input: inference_config.supports_video_input,
                },
                image_budget_for_route(&inference_config, execution_backend),
            );
        let existing = self.chat_state_handle.get_credentials().await;
        let session_key = self
            .auth_manager
            .as_ref()
            .and_then(|am| am.current_or_expired().map(|a| a.key));
        self.chat_state_handle
            .update_credentials(xai_chat_state::Credentials {
                api_key: inference_config.api_key.clone(),
                auth_type: crate::agent::config::resolve_chat_state_auth_type(
                    inference_config.model.as_str(),
                    session_key.as_deref(),
                    existing.auth_type,
                ),
                alpha_test_key: existing.alpha_test_key,
                client_version: inference_config.client_version.clone(),
            });
        self.invalidate_model_auth_memo();
        self.signals_handle()
            .record_model_usage(&inference_config.model);
        if apply_prompt_override && !skip_prompt_rewrite {
            let mut conversation = self.chat_state_handle.get_conversation().await;
            for item in conversation.iter_mut() {
                if let ConversationItem::System(sys) = item {
                    if use_concise {
                        sys.content = std::sync::Arc::<str>::from(
                            xai_grok_agent::prompt::template::COMPACT_SYSTEM_PROMPT,
                        );
                    } else {
                        sys.content =
                            std::sync::Arc::<str>::from(self.agent.borrow().system_prompt());
                    }
                    break;
                }
            }
            self.chat_state_handle.replace_conversation(conversation);
        } else if !apply_prompt_override {
            tracing::info!(
                session_id = %self.session_info.id.0,
                model_id = %model_id.0,
                "handle_set_session_model: skipping prompt override (apply_prompt_override=false)"
            );
        } else {
            tracing::info!(
                session_id = %self.session_info.id.0,
                model_id = %model_id.0,
                "handle_set_session_model: skipping prompt rewrite (just rebuilt harness)"
            );
        }
        let agent_name = self.agent.borrow().definition().name.clone();
        let envelope = self.external_runtime.borrow().clone();
        // Envelope already validated while preparing `prepared_external_runtime`.
        // Canonical selection + route with sampler config (route already asserted).
        let reasoning_effort = inference_config.reasoning_effort;
        *self.selection_model_id.borrow_mut() = model_id.clone();
        *self.route_context.borrow_mut() = Some(route.clone());
        let provenance = crate::session::storage::model_route::provenance_from_route_context(
            &route,
            model_id.0.as_ref(),
            inference_config.model.as_str(),
        );
        self.sampler_handle.update_config_with_route_context(
            inference_config,
            xai_grok_inference::route_context::RouteContextUpdate::Replace(route),
        );
        let _ = self
            .notifications
            .persistence_tx
            .send(PersistenceMsg::CurrentModel {
                model_id: model_id.clone(),
                agent_name: Some(agent_name),
                reasoning_effort: Some(reasoning_effort),
                execution_backend: Some(execution_backend),
                external_runtime: Some(envelope),
                route_provenance: Some(provenance),
            });
        Ok(model_id)
    }
    /// Handle [`SessionCommand::RebuildAgentForDefinition`].
    ///
    /// Builds a fresh [`xai_grok_agent::Agent`] from the cached
    /// [`crate::session::agent_rebuild::AgentRebuildSpec`] + the supplied
    /// [`xai_grok_agent::AgentDefinition`], replaces `self.agent`,
    /// rewrites the system message in the conversation, persists the
    /// new prompt artifacts, and updates `active_agent_type`.
    ///
    /// Triggered from `MvpAgent::set_session_model` only when the new
    /// model's `agent_type` differs from the session's current
    /// `active_agent_type` AND `turn_count == 0` (no user message has
    /// been sent yet). Defense-in-depth: rejects if a turn is in flight.
    pub(super) async fn handle_rebuild_agent_for_definition(
        &self,
        definition: xai_grok_agent::AgentDefinition,
    ) -> Result<(), acp::Error> {
        {
            let state = self.state.lock().await;
            if state.running_task.is_some() {
                tracing::warn!(
                    session_id = %self.session_info.id.0,
                    new_agent_type = %definition.name,
                    "handle_rebuild_agent_for_definition: turn in flight, rejecting rebuild"
                );
                return Err(acp::Error::internal_error()
                    .data("rebuild_agent: turn in flight, refusing to rebuild harness"));
            }
        }
        let new_agent_name = definition.name.clone();
        tracing::info!(
            session_id = %self.session_info.id.0,
            new_agent_type = %new_agent_name,
            "handle_rebuild_agent_for_definition: rebuilding harness"
        );
        let new_agent = self
            .rebuild_spec
            .build_agent(definition)
            .await
            .map_err(|e| {
                tracing::error!(
                    session_id = %self.session_info.id.0,
                    new_agent_type = %new_agent_name,
                    error = %e,
                    "handle_rebuild_agent_for_definition: AgentBuilder::build failed"
                );
                acp::Error::internal_error().data(format!(
                    "rebuild_agent: build failed for agent_type={new_agent_name}: {e}"
                ))
            })?;
        let new_system_prompt = new_agent.system_prompt().to_string();
        let mut new_prompt_context = new_agent.prompt_context().clone();
        new_prompt_context.normalize_for_persistence();
        if let Some(handle) = self.compaction.prefire.take_handle() {
            handle.abort();
            let _ = handle.await;
            self.compaction.prefire.finish();
        }
        self.compaction.prefire.clear();
        *self.agent.borrow_mut() = new_agent;
        *self.active_agent_type.lock() = Some(new_agent_name.clone());
        self.emit_resolved_tool_overrides();
        self.queue_exit_reminder_on_approved_exit.store(
            self.is_cursor_harness(),
            std::sync::atomic::Ordering::Relaxed,
        );
        if let Err(e) = self.workspace_ops.bind_local_session(
            &self.session_id_string(),
            self.tool_context.cwd.as_path().to_path_buf(),
            self.tool_context.hunk_tracker_handle.clone(),
            self.agent.borrow().tool_bridge().toolset(),
            None,
        ) {
            tracing::warn!(error = %e, "failed to rebind local session toolset after agent rebuild");
        }
        {
            let bridge = self.agent.borrow().tool_bridge().clone();
            let snapshot = self.tool_metadata_snapshot.clone();
            let tool_index = crate::session::tool_index::Bm25ToolSearchIndex::new(snapshot);
            bridge
                .update_resource(xai_grok_tools::types::tool_index::ToolIndex(
                    std::sync::Arc::new(tool_index),
                ))
                .await;
            if let Some(client) = self.rebuild_spec.managed_gateway_tool_client.clone() {
                bridge.update_resource(client).await;
            }
            let plan_path = self.plan_mode.lock().plan_file_path().to_path_buf();
            bridge
                .update_resource(xai_grok_tools::types::resources::PlanFilePath(plan_path))
                .await;
            if let Some(display_cwd) = self.display_cwd.get() {
                bridge
                    .set_display_cwd(std::path::PathBuf::from(display_cwd))
                    .await;
            }
            bridge
                .update_resource(
                    xai_grok_tools::implementations::grok_build::workflow::WorkflowLaunchHandle(
                        self.workflow_launch_tx.clone(),
                    ),
                )
                .await;
            if !self.goal_runs_on_workflow_engine() {
                bridge
                    .update_resource(
                        xai_grok_tools::implementations::grok_build::update_goal::GoalUpdateHandle(
                            self.goal_update_tx.clone(),
                        ),
                    )
                    .await;
            }
            if let Some(reservations) = self.tool_context.task_completion_reservations.clone() {
                bridge.update_resource(reservations).await;
            }
            if let Some(gate) = self.tool_context.task_wake_suppressed.clone() {
                bridge.update_resource(gate).await;
            }
            self.inject_deny_read_globs().await;
        }
        {
            let notified = self.mcp_handshakes_done.notified();
            tokio::pin!(notified);
            let needs_wait = {
                let s = self.mcp_state.lock().await;
                !s.configs.is_empty() && !s.is_initialized()
            };
            if needs_wait {
                const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
                tokio::select! {
                    () = &mut notified => {}
                    () = tokio::time::sleep(TIMEOUT) => {
                        tracing::warn!(
                            session_id = %self.session_info.id.0,
                            "handle_rebuild_agent_for_definition: timed out waiting for MCP handshakes"
                        );
                    }
                }
            }
        }
        self.re_register_mcp_tools_on_rebuilt_bridge().await;
        if let Some(old_handle) = self.deferred_prefix.take() {
            old_handle.abort();
        }
        let new_user_prefix = self.build_user_message_prefix().await;
        {
            let mut conversation = self.chat_state_handle.get_conversation().await;
            let _ = replace_or_insert_system_head(&mut conversation, &new_system_prompt);
            let drop_startup_skill_reminder = false;
            Self::rewrite_zero_turn_prefix(
                &mut conversation,
                new_user_prefix,
                drop_startup_skill_reminder,
            );
            if !conversation_has_project_instructions(&conversation)
                && let Some(agents_md_reminder) = self.agent.borrow().agents_md_user_reminder()
            {
                let agents_md_at = conversation.len().min(2);
                conversation.insert(
                    agents_md_at,
                    ConversationItem::project_instructions(agents_md_reminder),
                );
            }
            self.inject_baseline_skill_reminder(&mut conversation).await;
            self.chat_state_handle.replace_conversation(conversation);
        }
        save_prompt_context(&self.session_info, &new_prompt_context);
        save_system_prompt(&self.session_info, &new_system_prompt);
        let snapshot = self.chat_state_handle.get_conversation().await;
        persist_chat_history_jsonl_sync(&self.session_info, &snapshot);
        self.mcp_reminder_dirty
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.send_available_commands_update().await;
        tracing::info!(
            session_id = %self.session_info.id.0,
            new_agent_type = %new_agent_name,
            "handle_rebuild_agent_for_definition: harness rebuild complete"
        );
        Ok(())
    }
    /// Apply a client-supplied `systemPromptOverride` on session attach without
    /// wiping user/assistant history: swap only the leading `System` message,
    /// atomically inside the `ChatStateActor` (see
    /// `ChatStateCommand::ReplaceSystemHead` for the serialization guarantees).
    /// `system_prompt.txt` (not owned by the persistence actor) is saved
    /// directly, even on a head no-op, so a previously-diverged secondary
    /// artifact self-heals. Skipped entirely on a verbatim mirror-fork
    /// (`preserve_inherited_system`).
    pub(super) async fn handle_replace_system_prompt(&self, system_prompt: String) {
        if self.startup_hints.preserve_inherited_system {
            tracing::debug!(
                session_id = %self.session_info.id.0,
                "handle_replace_system_prompt: skipped (preserve_inherited_system)"
            );
            return;
        }
        let Some(changed) = self
            .chat_state_handle
            .replace_system_head(&system_prompt)
            .await
        else {
            tracing::error!(
                session_id = %self.session_info.id.0,
                "handle_replace_system_prompt: chat-state actor unavailable; override not applied"
            );
            return;
        };
        save_system_prompt(&self.session_info, &system_prompt);
        if changed {
            tracing::info!(
                session_id = %self.session_info.id.0,
                prompt_len = system_prompt.len(),
                "handle_replace_system_prompt: client override applied"
            );
        } else {
            tracing::debug!(
                session_id = %self.session_info.id.0,
                "handle_replace_system_prompt: head already matches, no-op"
            );
        }
    }
}

impl SessionActor {
    /// Re-stamp the `<tersify_style>` block in the system head from the stashed
    /// `/tersify` session override.
    ///
    /// Semantics:
    /// - `Some(level)` replaces the current block (or appends one when absent).
    /// - `None` clears the block: the persisted `[hints] tersify_*` governs the
    ///   next full rebuild (spawn), matching `/tersify off`'s documented behavior.
    ///
    /// No-op when the head has no block AND no override is set — the common case
    /// for every session that never touched `/tersify`, so this costs one mutex
    /// read and one string scan per turn.
    pub(super) async fn refresh_tersify_style_impl(&self) {
        let level = self
            .tersify_level_meta
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let Some(current) = crate::session::acp_session::load_system_prompt(&self.session_info)
        else {
            return;
        };
        let rebuilt = match level.as_deref() {
            Some("off") => Self::remove_tersify_style(&current),
            Some(level) => {
                let instruction = xai_grok_tersify::style::style_instruction(level);
                Self::upsert_tersify_style(&current, instruction)
            }
            None => current.clone(),
        };
        if rebuilt != current {
            self.handle_replace_system_prompt(rebuilt).await;
        }
    }

    /// Replace or insert the `<tersify_style>` block in a system prompt.
    #[must_use]
    fn upsert_tersify_style(prompt: &str, instruction: &str) -> String {
        let block = format!("<tersify_style>\n{instruction}\n</tersify_style>");
        match prompt.find("<tersify_style>") {
            Some(start) => {
                let Some(end_rel) = prompt[start..].find("</tersify_style>") else {
                    return format!("{prompt}\n\n{block}");
                };
                let end = start + end_rel + "</tersify_style>".len();
                format!("{}{}{}", &prompt[..start], block, &prompt[end..])
            }
            None => format!("{prompt}\n\n{block}"),
        }
    }

    /// Remove the block entirely; whitespace-normalized tail.
    #[must_use]
    fn remove_tersify_style(prompt: &str) -> String {
        let Some(start) = prompt.find("<tersify_style>") else {
            return prompt.to_string();
        };
        let Some(end_rel) = prompt[start..].find("</tersify_style>") else {
            return prompt.to_string();
        };
        let end = start + end_rel + "</tersify_style>".len();
        let mut out = String::with_capacity(prompt.len());
        out.push_str(prompt[..start].trim_end());
        out.push('\n');
        out.push_str(prompt[end..].trim_start());
        out.trim_end().to_string()
    }

    /// Mid-session pinned-tools update (`x.ai/pinned_tools_changed`).
    ///
    /// Resolves the pinned names against the live toolset catalog and
    /// re-stamps the `<pinned_tools>` block in the system head. No-op when
    /// the rebuilt head matches the live one (idempotent duplicate
    /// notifications).
    pub(super) async fn handle_set_pinned_tools(&self, tool_names: &[String]) {
        let Some(current) = crate::session::acp_session::load_system_prompt(&self.session_info)
        else {
            return;
        };
        let definitions = self.agent.borrow().tool_bridge().tool_definitions().await;
        let rebuilt = crate::session::pinned_tools::refresh_pinned_tools_block(
            &current,
            &definitions,
            tool_names,
        );
        if rebuilt != current {
            tracing::info!(
                pinned_count = tool_names.len(),
                "Re-stamping <pinned_tools> system block"
            );
            self.handle_replace_system_prompt(rebuilt).await;
        }
    }
}
