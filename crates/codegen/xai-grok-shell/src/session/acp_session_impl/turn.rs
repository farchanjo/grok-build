//! Turn-execution concern for `SessionActor` (`handle_prompt`, turn-end,
//! sampling loop).
use super::*;

impl SessionActor {
    /// Single effective capability mode key for external runtimes.
    ///
    /// Precedence (security): plan / read-only **always wins** over yolo.
    /// `plan + yolo=true` → `read_only` (never always_approve). When not in
    /// plan, yolo → `always_approve` (broad allowlist, still brokered). Auto
    /// and default → `all`.
    ///
    /// This key is used both to configure the runtime and as the retained-runtime
    /// compatibility key (mode changes force recreate + shutdown).
    pub(crate) fn external_effective_mode_key(&self) -> String {
        use crate::session::plan_mode::PlanModeState;
        let plan_active = self.plan_mode.lock().state() != PlanModeState::Inactive
            || *self.current_prompt_mode.lock() == PromptMode::Plan;
        // Plan always wins over yolo / always-approve.
        if plan_active {
            return "read_only".to_owned();
        }
        if self.permissions.is_yolo_mode() {
            return "always_approve".to_owned();
        }
        if self.permissions.is_auto_mode() {
            return "all".to_owned();
        }
        match *self.current_prompt_mode.lock() {
            PromptMode::Ask => "read_only".to_owned(),
            PromptMode::Plan => "read_only".to_owned(),
            // Default/agent: brokered all mode.
            PromptMode::Agent => "all".to_owned(),
        }
    }

    /// Back-compat alias used by older tests.
    pub(crate) fn external_host_mode_label(&self) -> String {
        self.external_effective_mode_key()
    }

    /// Obtain or create the session-scoped external runtime (PermissionHandle +
    /// effective capability mode). Reuses one Arc across turns when kind and
    /// effective mode are unchanged.
    pub(crate) async fn ensure_external_agent_runtime(
        &self,
        kind: crate::agent::execution_backend::ExternalAgentKind,
    ) -> Result<
        std::sync::Arc<dyn crate::agent::external_runtime::ExternalAgentRuntime>,
        crate::agent::external_runtime::ExternalRuntimeError,
    > {
        use crate::agent::external_runtime::{
            ExternalRuntimeSessionContext, RetainedExternalAgentRuntime, default_registry,
        };

        let effective_mode = self.external_effective_mode_key();
        {
            let guard = self.external_agent_runtime.borrow();
            if let Some(retained) = guard.as_ref() {
                if retained.kind == kind && retained.effective_mode == effective_mode {
                    return Ok(retained.runtime.clone());
                }
            }
        }
        // Kind or effective capability mode changed: shut down prior instance.
        self.shutdown_external_agent_runtime().await;

        let ctx =
            ExternalRuntimeSessionContext::new(self.permissions.clone(), effective_mode.clone());
        let runtime = default_registry()
            .create_for_session(kind, &ctx)
            .ok_or_else(|| {
                crate::agent::external_runtime::ExternalRuntimeError::unavailable(kind)
            })?;
        *self.external_agent_runtime.borrow_mut() = Some(RetainedExternalAgentRuntime::new(
            kind,
            effective_mode,
            runtime.clone(),
        ));
        Ok(runtime)
    }

    /// Shut down and drop the retained external runtime (bridge, temp dirs,
    /// persistent child). Safe to call when none is retained.
    pub(crate) async fn shutdown_external_agent_runtime(&self) {
        let retained = self.external_agent_runtime.borrow_mut().take();
        if let Some(retained) = retained {
            let envelope = self.external_runtime.borrow().clone().unwrap_or_else(|| {
                crate::agent::external_runtime::ExternalRuntimeEnvelope::for_kind(retained.kind)
            });
            if let Err(e) = retained.runtime.shutdown(&envelope).await {
                tracing::warn!(
                    session_id = %self.session_info.id.0,
                    kind = %retained.kind,
                    error = %e,
                    "external agent runtime shutdown returned error"
                );
            }
        }
    }

    /// Fail closed when the session's execution backend is an external agent
    /// that is not available (gates closed, missing binary, probe failure).
    /// Safe to call before any turn mutation (`increment_turn`, history append).
    pub(crate) async fn preflight_external_execution_backend(
        &self,
    ) -> Result<(), crate::agent::external_runtime::ExternalRuntimeError> {
        let backend = self.execution_backend.get();
        if backend.is_native() {
            return Ok(());
        }
        let kind = backend.external_kind().ok_or_else(|| {
            crate::agent::external_runtime::ExternalRuntimeError::new(
                crate::agent::external_runtime::ExternalRuntimeErrorKind::InvalidRequest,
                "external execution backend selected without a kind",
                None,
            )
        })?;
        // Session-aware factory: attach PermissionHandle + capability mode.
        // Preflight and turn share the retained Arc.
        let runtime = self.ensure_external_agent_runtime(kind).await?;
        // Successful probe is required before turn establishment. Probe
        // failure fails closed with no session mutation of turn_count/history.
        runtime.probe().await?;
        Ok(())
    }

    /// Run one external-agent turn (Claude CLI, …), mapping normalized events
    /// into ACP SessionUpdates. Does **not** enter InferenceActor / Grok tool
    /// loop / compaction / memory / goals / workflow machinery.
    ///
    /// Reuses the session-scoped runtime Arc. Successful assistant text is
    /// persisted as a text-only ConversationItem (Claude tools are display-only).
    pub(crate) async fn run_external_agent_turn(
        self: &std::sync::Arc<Self>,
        prompt_id: &str,
        prompt_text: &str,
    ) -> crate::session::commands::PromptTurnResult {
        use crate::agent::external_runtime::{
            ExternalRuntimeTurnEvent, ExternalStartRequest, ExternalTurnRequest,
        };
        use crate::session::commands::ok_end_turn;

        let backend = self.execution_backend.get();
        let kind = backend.external_kind().ok_or_else(|| {
            crate::agent::external_runtime::ExternalRuntimeError::new(
                crate::agent::external_runtime::ExternalRuntimeErrorKind::InvalidRequest,
                "external execution backend selected without a kind",
                None,
            )
            .into_acp_error()
        })?;

        let runtime = self
            .ensure_external_agent_runtime(kind)
            .await
            .map_err(|e| e.into_acp_error())?;

        // The host goal harness never applies to an external backend, which owns
        // its own loop. Merely having it available must not block the turn — only
        // an actually running goal does.
        match external_turn_goal_action(
            self.goal_harness_enabled(),
            self.goal_tracker.lock().status(),
        ) {
            ExternalTurnGoalAction::Proceed => {}
            ExternalTurnGoalAction::DisableHarness => {
                self.disable_goal_harness_for_external_turn();
            }
            ExternalTurnGoalAction::Refuse => {
                return Err(
                    crate::agent::external_runtime::ExternalRuntimeError::new(
                        crate::agent::external_runtime::ExternalRuntimeErrorKind::InvalidRequest,
                        "A running /goal cannot drive a Claude Agent CLI session. Pause the goal, or start /new with a native model.",
                        Some(kind),
                    )
                    .into_acp_error(),
                );
            }
        }

        let cwd = self.tool_context.cwd.as_str().to_owned();
        // Wire slug for the external CLI (provider expects upstream id).
        // Session-private canonical selection is used only for persistence.
        let selected_model = self
            .chat_state_handle
            .get_inference_settings()
            .await
            .map(|c| c.model)
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| self.selection_model_id.borrow().0.to_string());
        let effort = self
            .chat_state_handle
            .get_inference_settings()
            .await
            .and_then(|c| c.reasoning_effort.map(|e| e.as_str().to_owned()));

        let mut envelope = self.external_runtime.borrow().clone();
        let stored_valid_envelope = envelope.clone().and_then(|env| env.validated().ok());
        if envelope.is_none() {
            let started = runtime
                .start(ExternalStartRequest {
                    cwd: cwd.clone(),
                    worktree_identity: None,
                    selected_model: Some(selected_model.clone()),
                    reasoning_effort: effort.clone(),
                    token_budget: None,
                })
                .await
                .map_err(|e| e.into_acp_error())?;
            envelope = Some(started);
        } else if let Some(ref env) = envelope {
            // Resume path validates pointer when present.
            if env.session_pointer.as_ref().is_some_and(|s| !s.is_empty()) {
                let resumed = runtime.resume(env).await.map_err(|e| e.into_acp_error())?;
                envelope = Some(resumed);
            }
        }
        let envelope = envelope.ok_or_else(|| {
            crate::agent::external_runtime::ExternalRuntimeError::unavailable(kind).into_acp_error()
        })?;
        let prior_valid_envelope =
            stored_valid_envelope.or_else(|| envelope.clone().validated().ok());

        // Wire cancel: if the session turn_cancel fires, cancel the *retained* runtime.
        let cancel_token = self.turn_cancel.borrow().clone();
        let runtime_for_cancel = runtime.clone();
        let env_for_cancel = envelope.clone();
        let cancel_watch = tokio::spawn(async move {
            cancel_token.cancelled().await;
            let _ = runtime_for_cancel.cancel(&env_for_cancel).await;
        });

        let turn_result = runtime
            .turn(
                &envelope,
                ExternalTurnRequest {
                    prompt: prompt_text.to_owned(),
                    selected_model: Some(selected_model.clone()),
                    reasoning_effort: effort,
                    token_budget: None,
                },
            )
            .await;

        cancel_watch.abort();

        let outcome = match turn_result {
            Ok(o) => o,
            Err(e) => {
                // Persist text-only partial assistant content exactly once on
                // *any* failure that carried TextDelta events (cancel or not).
                // Never include ToolCall/Status/Error display text; never Grok tools.
                // Process APIs buffer NDJSON (no live stream), so emit UI TextDelta
                // once here for all failure kinds without double-emitting.
                let partial_text = Self::collect_external_text_deltas(&e.partial_events);
                if !partial_text.is_empty() {
                    for event in &e.partial_events {
                        if let ExternalRuntimeTurnEvent::TextDelta { text } = event {
                            if text.is_empty() {
                                continue;
                            }
                            self.send_update(
                                acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
                                    acp::ContentBlock::Text(acp::TextContent::new(text.clone())),
                                )),
                                None,
                            )
                            .await;
                        }
                    }
                    self.chat_state_handle
                        .push_assistant_response(ConversationItem::assistant(partial_text));
                }

                // Best-effort envelope on any failure that carries a partial pointer.
                if let Some(partial) = e.partial_envelope.clone() {
                    if let Ok(validated) = partial.clone().validated() {
                        *self.external_runtime.borrow_mut() = Some(validated.clone());
                        // Canonical selection — not the upstream wire slug.
                        let model_id = self.selection_model_id.borrow().clone();
                        let agent_name = self.agent.borrow().definition().name.clone();
                        let _ = self.notifications.persistence_tx.send(
                            crate::session::persistence::PersistenceMsg::CurrentModel {
                                model_id,
                                agent_name: Some(agent_name),
                                reasoning_effort: None,
                                execution_backend: Some(backend),
                                external_runtime: Some(Some(validated)),
                                route_provenance: None,
                            },
                        );
                    }
                }

                if e.kind == crate::agent::external_runtime::ExternalRuntimeErrorKind::Cancelled {
                    // Status display only for cancel path (not chat-state).
                    for event in &e.partial_events {
                        if let ExternalRuntimeTurnEvent::Status { message } = event {
                            self.send_update(
                                acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
                                    acp::ContentBlock::Text(acp::TextContent::new(format!(
                                        "[{message}]"
                                    ))),
                                )),
                                None,
                            )
                            .await;
                        }
                    }
                    return Ok(crate::session::commands::PromptTurnOk {
                        stop_reason: acp::StopReason::Cancelled,
                        total_tokens: 0,
                        turn_snapshot: None,
                        completion_kind:
                            crate::session::commands::PromptCompletionKind::Cancelled {
                                category: None,
                                context: None,
                            },
                        structured_output: None,
                        usage: None,
                        tool_overrides: None,
                    });
                }
                return Err(e.into_acp_error());
            }
        };

        // Map normalized events → ACP SessionUpdates (no second protocol).
        // Collect assistant text only (never Claude tool events as Grok tools).
        let mut assistant_text = String::new();
        for event in &outcome.events {
            match event {
                ExternalRuntimeTurnEvent::TextDelta { text } => {
                    if text.is_empty() {
                        continue;
                    }
                    assistant_text.push_str(text);
                    self.send_update(
                        acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
                            acp::ContentBlock::Text(acp::TextContent::new(text.clone())),
                        )),
                        None,
                    )
                    .await;
                }
                ExternalRuntimeTurnEvent::ToolCall { name, summary } => {
                    // Display/audit only — never dispatch to Grok tool executor
                    // and never record as Grok tool_call ConversationItems.
                    let msg = match summary {
                        Some(s) => format!("[Claude tool: {name} ({s})]"),
                        None => format!("[Claude tool: {name}]"),
                    };
                    self.send_update(
                        acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
                            acp::ContentBlock::Text(acp::TextContent::new(msg)),
                        )),
                        None,
                    )
                    .await;
                }
                ExternalRuntimeTurnEvent::Status { message } => {
                    self.send_update(
                        acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
                            acp::ContentBlock::Text(acp::TextContent::new(format!("[{message}]"))),
                        )),
                        None,
                    )
                    .await;
                }
                ExternalRuntimeTurnEvent::Error { message } => {
                    self.send_update(
                        acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
                            acp::ContentBlock::Text(acp::TextContent::new(format!(
                                "[error: {message}]"
                            ))),
                        )),
                        None,
                    )
                    .await;
                }
            }
        }

        // Persist successful external assistant text as a normalized text-only
        // ConversationItem for replay/export/rewind (no tool_calls).
        if !assistant_text.is_empty() {
            self.chat_state_handle
                .push_assistant_response(ConversationItem::assistant(assistant_text));
        }

        // Persist redacted envelope only (no raw NDJSON). A runtime outcome
        // that fails validation can never replace the durable pointer.
        let envelope_to_store = match outcome.envelope.clone().validated() {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    session_id = %self.session_info.id.0,
                    error = %e,
                    "external envelope failed validation; retaining prior valid envelope"
                );
                prior_valid_envelope.ok_or_else(|| {
                    crate::agent::external_runtime::ExternalRuntimeError::new(
                        crate::agent::external_runtime::ExternalRuntimeErrorKind::InvalidRequest,
                        format!("external runtime returned an invalid envelope: {e}"),
                        Some(kind),
                    )
                    .into_acp_error()
                })?
            }
        };
        *self.external_runtime.borrow_mut() = Some(envelope_to_store.clone());
        // Canonical selection — not the upstream wire slug in selected_model.
        let model_id = self.selection_model_id.borrow().clone();
        let agent_name = self.agent.borrow().definition().name.clone();
        let _ = self.notifications.persistence_tx.send(
            crate::session::persistence::PersistenceMsg::CurrentModel {
                model_id,
                agent_name: Some(agent_name),
                reasoning_effort: None,
                execution_backend: Some(backend),
                external_runtime: Some(Some(envelope_to_store)),
                route_provenance: None,
            },
        );

        let tokens = outcome
            .usage
            .as_ref()
            .and_then(|u| {
                u.total_tokens
                    .or_else(|| match (u.input_tokens, u.output_tokens) {
                        (Some(i), Some(o)) => Some(i.saturating_add(o)),
                        (Some(i), None) => Some(i),
                        (None, Some(o)) => Some(o),
                        _ => None,
                    })
            })
            .unwrap_or(0);

        tracing::info!(
            session_id = %self.session_info.id.0,
            prompt_id = %prompt_id,
            kind = %kind,
            tokens,
            "external agent turn completed"
        );

        ok_end_turn(tokens, None)
    }

    /// Collect TextDelta only from external events (exactly once for chat-state).
    /// Excludes ToolCall / Status / Error display strings.
    pub(crate) fn collect_external_text_deltas(
        events: &[crate::agent::external_runtime::ExternalRuntimeTurnEvent],
    ) -> String {
        use crate::agent::external_runtime::ExternalRuntimeTurnEvent;
        let mut out = String::new();
        for event in events {
            if let ExternalRuntimeTurnEvent::TextDelta { text } = event {
                if !text.is_empty() {
                    out.push_str(text);
                }
            }
        }
        out
    }
}

/// Synthetic tool the model calls to return its schema-constrained final answer
/// on backends that cannot constrain output natively (custom Messages, or
/// Messages without model-level native-schema capability), and for the
/// dual-language envelope on every backend. Native `response_format` /
/// `output_config.format` cannot represent tool calls, so the language
/// envelope is advertised here instead of on the request. Intercepted in
/// the loop, never executed as a real tool.
const STRUCTURED_OUTPUT_TOOL: &str = "StructuredOutput";
/// Max times the model may re-call `StructuredOutput` with non-conforming args
/// before the turn ends with the last validation error.
const STRUCTURED_OUTPUT_MAX_RETRIES: u32 = 3;
/// What a `StructuredOutput` tool call means for the turn (see
/// `handle_structured_output_tool_call`).
enum StructuredOutputStep {
    /// Accepted, or retries exhausted: the carried result is the final output.
    Complete(Result<serde_json::Value, String>),
    /// Non-conforming args; a corrective tool_result was pushed — re-sample.
    Retry,
    /// No sole StructuredOutput call (absent, or co-emitted with real tools that
    /// should run this round).
    Proceed,
}
/// Parse `raw` as JSON and validate it against a `validator` compiled once per
/// turn. Returns the value on success, or a human-readable error (surfaced to
/// the model on retry and to the client as `structuredOutputError`). A `validator`
/// of `Err` means the user's schema itself was invalid.
fn validate_structured_output(
    validator: &Result<jsonschema::Validator, String>,
    raw: &str,
) -> Result<serde_json::Value, String> {
    let validator = validator.as_ref().map_err(Clone::clone)?;
    let value: serde_json::Value = serde_json::from_str(raw.trim())
        .map_err(|e| format!("model output was not valid JSON: {e}"))?;
    match validator.validate(&value) {
        Ok(()) => Ok(value),
        Err(e) => Err(format!("output does not match the required schema: {e}")),
    }
}

/// Decode the language-envelope `response` string. `None` if the field is
/// missing or not a string.
fn extract_language_response(value: &serde_json::Value) -> Option<String> {
    value.get("response")?.as_str().map(str::to_owned)
}

/// Decide how a turn applies a JSON schema.
///
/// User-supplied schemas keep the native `response_format` /
/// `output_config.format` path when the backend supports it. The dual-language
/// envelope never uses native schema: that object cannot represent tool calls,
/// and Chat Completions `json_schema` makes models skip tools. The envelope is
/// advertised as the [`STRUCTURED_OUTPUT_TOOL`] instead, while
/// `<language_policy>` remains on the prompt and as a per-turn reminder.
fn structured_output_modes(
    schema_ok: bool,
    native_backend: bool,
    language_envelope_active: bool,
) -> (bool, bool) {
    let structured_output_native = schema_ok && native_backend && !language_envelope_active;
    let structured_output_tool = schema_ok && (!native_backend || language_envelope_active);
    (structured_output_native, structured_output_tool)
}

/// True when `text` is a language-envelope JSON object. Used to keep raw
/// envelope bytes off `AgentMessageChunk` / headless stdout.
fn is_language_envelope_json(text: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(text.trim())
        .ok()
        .is_some_and(|value| {
            value.get("response").is_some() && value.get("conversation_language").is_some()
        })
}

/// Decode `response` from envelope JSON, or `None` if `text` is not an envelope.
fn decode_language_envelope_text(text: &str) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(text.trim()).ok()?;
    extract_language_response(&value)
}

/// Rewrite an assistant item's content to the decoded `response` so chat_state
/// never stores the raw envelope. Clears Messages replay payloads — they still
/// hold envelope bytes and must not be replayed after a rewrite.
fn rewrite_assistant_content(item: ConversationItem, response: &str) -> ConversationItem {
    match item {
        ConversationItem::Assistant(mut assistant) => {
            assistant.content = std::sync::Arc::<str>::from(response);
            assistant.clear_provider_payload();
            ConversationItem::Assistant(assistant)
        }
        other => other,
    }
}

/// Map assistant items through `rewrite`, leaving non-assistant items intact.
fn rewrite_assistant_items(
    items: Vec<xai_grok_inference_types::ConversationItem>,
    response: &str,
) -> Vec<xai_grok_inference_types::ConversationItem> {
    items
        .into_iter()
        .map(|item| {
            if matches!(
                item,
                xai_grok_inference_types::ConversationItem::Assistant(_)
            ) {
                rewrite_assistant_content(item, response)
            } else {
                item
            }
        })
        .collect()
}

impl SessionActor {
    /// Dual-language policy for this turn. Session conversation language
    /// overrides config; artifact comes from the merged `[language]` config
    /// (locked values already win at merge time). Inactive when conversation
    /// is unset.
    fn language_policy_for_turn(&self) -> Option<crate::agent::config::ResolvedLanguagePolicy> {
        let cfg = crate::config::load_effective_config()
            .ok()
            .and_then(|raw| crate::agent::config::Config::new_from_toml_cfg(&raw).ok())
            .map(|c| c.language)
            .unwrap_or_default();
        let session = self.conversation_language.borrow();
        cfg.resolved(session.as_deref())
    }
}

/// Result of the turn-end usage drain (and cancel's no-drain snapshot).
///
/// **Ledger marks** only when [`Self::fail_closed`]. Sticky and background
/// live are **report-level only** (tokens still land on the session ledger).
pub(super) struct UsageDrainOutcome {
    /// Query failure, FG still live after timeout/cancel. Marks both
    /// the prompt and session bills incomplete. (True apply-miss stains
    /// ledgers at fold time via `mark_apply_miss_incomplete`, not here.)
    pub(super) fail_closed: bool,
    /// A background child is still running: only this prompt's report is
    /// incomplete; its spend reaches the session ledger at completion.
    pub(super) background_live: bool,
    /// Pin-scoped sticky (session-only attribution or apply-miss report).
    /// Report incomplete only — does not stain ledgers by itself.
    pub(super) sticky_report: bool,
}
impl UsageDrainOutcome {
    /// Wire / attach incomplete: fail-closed ∪ background ∪ sticky.
    pub(super) fn report_incomplete(&self) -> bool {
        self.fail_closed || self.background_live || self.sticky_report
    }
    /// Map an outstanding reply without a multi-second drain (cancel path).
    /// Same policy as freeze's terminal outcome: FG live → fail-closed;
    /// sticky and background → report only.
    pub(super) fn from_outstanding_reply(
        reply: Option<
            &xai_grok_tools::implementations::grok_build::task::types::SubagentOutstandingReply,
        >,
    ) -> Self {
        match reply {
            None => Self {
                fail_closed: true,
                background_live: false,
                sticky_report: false,
            },
            Some(r) => Self {
                fail_closed: !r.live_ids.is_empty(),
                background_live: r.background_live,
                sticky_report: r.subagent_usage_not_applied,
            },
        }
    }
}
/// Accumulates a turn's per-call token usage and tool-call presence across the
/// agentic loop's model calls, recording running totals on the turn span. Kept
/// out of the loop body so telemetry bookkeeping doesn't obscure control flow.
#[derive(Default)]
struct TurnSpanTotals {
    input_tokens: i64,
    output_tokens: i64,
    cache_read_tokens: i64,
    has_tool_call: bool,
}
impl TurnSpanTotals {
    /// Fold one model response into the totals (tokens sum — each call is billed
    /// its full prompt; has_tool_call OR-s — the final call has none) and update
    /// the span. `stop_reason` is last-wins (the terminal reason), not summed.
    fn record(&mut self, span: &tracing::Span, response: &ConversationResponse) {
        if let Some(u) = response.usage.as_ref() {
            self.input_tokens += i64::from(u.prompt_tokens);
            self.output_tokens += i64::from(u.completion_tokens);
            self.cache_read_tokens += i64::from(u.cached_prompt_tokens);
            span.record("input_tokens", self.input_tokens);
            span.record("output_tokens", self.output_tokens);
            span.record("cache_read_tokens", self.cache_read_tokens);
        }
        if let Some(sr) = response.stop_reason {
            span.record("stop_reason", sr.as_str());
        }
        self.has_tool_call |= !response.tool_calls().is_empty();
        span.record("response.has_tool_call", self.has_tool_call);
    }
}
/// How the turn's per-block user-message echo is published to clients /
/// `updates.jsonl`.
///
/// Every turn consumes a `prompt_index`, and rewind / fork truncation
/// (`replay_to_prompt`, `updates_truncate_for_prompt`) recover turn
/// boundaries by counting persisted `UserMessageChunk` runs — so every mode
/// persists the echo. Turns whose content must not render as a user prompt
/// (notification drain) are hidden by the *pager* via the
/// `hideFromScrollback` chunk meta, not by omitting the persisted line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UserEchoMode {
    /// Live + persist (real user / cron / skill turns).
    Broadcast,
    /// Persist without live broadcast. Interject-fallback: panes already
    /// rendered the text, so a live echo would duplicate it. Notification
    /// drain: model-only content (the UI surfaces it via side channels:
    /// monitor gutter, task pane) that no pane should render live.
    PersistOnly,
}
fn user_echo_mode(origin: &super::super::PromptOrigin) -> UserEchoMode {
    match origin {
        // Interjection fallback and notification drain are model-only / already
        // rendered side-channel content — never broadcast as a user prompt.
        super::super::PromptOrigin::Interjection
        | super::super::PromptOrigin::NotificationDrain => UserEchoMode::PersistOnly,
        super::super::PromptOrigin::User
        | super::super::PromptOrigin::SubagentAssignment
        | super::super::PromptOrigin::TaskCompleted { .. }
        | super::super::PromptOrigin::SubagentCompleted { .. }
        | super::super::PromptOrigin::WorkflowCompleted { .. }
        | super::super::PromptOrigin::GoalSummary
        | super::super::PromptOrigin::SchedulerFired
        | super::super::PromptOrigin::PlanResume
        | super::super::PromptOrigin::Unknown => UserEchoMode::Broadcast,
    }
}
impl SessionActor {
    /// Run the image-normalization pipeline (re-encode caps, min-side and
    /// integrity checks) and surface its outcomes: compression / re-encode
    /// fallback / dropped notices are appended to `text_out` (TEXT only —
    /// image data never enters a string) and mirrored as
    /// `ImageCompressed`/`ImageDropped` notifications. Returns the surviving
    /// images. Single owner of the notice/notify wiring, shared by the
    /// prompt path and the interjection drain.
    pub(crate) async fn normalize_images_with_notices(
        &self,
        text_out: &mut String,
        images: Vec<acp::ImageContent>,
        is_cursor: bool,
    ) -> Vec<acp::ImageContent> {
        let mut norm_result =
            crate::session::image_normalize::normalize_images(images, is_cursor).await;
        let user_images = std::mem::take(&mut norm_result.images);
        use crate::extensions::notification::ImageCompressedEntry;
        if !norm_result.compressed.is_empty() {
            text_out.push_str(&crate::session::image_normalize::render_compression_notice(
                &norm_result.compressed,
                is_cursor,
            ));
            let message = norm_result
                .compressed
                .iter()
                .map(|c| c.display())
                .collect::<Vec<_>>()
                .join("; ");
            let images = norm_result
                .compressed
                .iter()
                .map(ImageCompressedEntry::from)
                .collect();
            self.send_xai_notification(XaiSessionUpdate::ImageCompressed { images, message })
                .await;
        }
        if !norm_result.re_encode_fallbacks.is_empty() {
            text_out.push_str(
                &crate::session::image_normalize::render_re_encode_fallback_notice(
                    &norm_result.re_encode_fallbacks,
                    is_cursor,
                ),
            );
            self.send_xai_notification(XaiSessionUpdate::ImageCompressed {
                images: vec![],
                message: norm_result.re_encode_fallbacks.join(" "),
            })
            .await;
        }
        if let Some((notice, notes)) = crate::session::image_normalize::dropped_to_envelope(
            std::mem::take(&mut norm_result.dropped),
            is_cursor,
        ) {
            text_out.push_str(&notice);
            self.send_xai_notification(XaiSessionUpdate::ImageDropped { notes })
                .await;
        }
        user_images
    }
    pub(super) fn persist_host_turn_user_echo(
        &self,
        text: &str,
        origin: &super::super::PromptOrigin,
    ) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        let mut chunk_meta = serde_json::Map::new();
        chunk_meta.insert(
            crate::session::storage::HOST_TURN_META_KEY.into(),
            serde_json::json!(true),
        );
        if origin.hide_user_echo_from_scrollback() {
            chunk_meta.insert("hideFromScrollback".into(), serde_json::json!(true));
        }
        let update = acp::SessionUpdate::UserMessageChunk(
            acp::ContentChunk::new(acp::ContentBlock::Text(acp::TextContent::new(
                text.to_string(),
            )))
            .meta(Some(chunk_meta)),
        );
        let notification_meta = self.build_notification_meta();
        let notification = acp::SessionNotification::new(self.session_info.id.clone(), update)
            .meta(notification_meta.as_object().cloned());
        let _ = self
            .notifications
            .persistence_tx
            .send(PersistenceMsg::Update(
                crate::session::storage::SessionUpdate::Acp(Box::new(notification)),
            ));
    }
    /// Per-session PR18 workspace inventory, built off the async executor and
    /// cached via [`InventoryCache`]. `/clear` and tool-touched paths
    /// invalidate it; children build their own (no parent body/cache reuse).
    async fn prime_inventory(&self) -> crate::session::prime::inventory::WorkspaceInventory {
        let cache = self.prime_cache.clone();
        let root = self.tool_context.cwd.to_path_buf();
        let limits = crate::session::prime::inventory::InventoryLimits::default();
        tokio::task::spawn_blocking(move || cache.get_or_build(&root, limits))
            .await
            .unwrap_or_default()
    }
    /// Trusted containment roots for the prime skill-body load: the session
    /// cwd, the git root when present, and the Grok home. The home is included
    /// so bundled and user-scope native skills — which always live under
    /// `$GROK_HOME`, never under the workspace — stay primable.
    /// `validated_roots` still drops any root that is not an ancestor of an
    /// eligible skill from the authoritative snapshot, so an unrelated root
    /// (or an unrelated home) authorizes nothing.
    fn prime_trusted_roots(
        cwd: &std::path::Path,
        git_root: Option<&std::path::Path>,
        grok_home: &std::path::Path,
    ) -> Vec<std::path::PathBuf> {
        let mut roots = vec![cwd.to_path_buf()];
        if let Some(root) = git_root {
            roots.push(root.to_path_buf());
        }
        roots.push(grok_home.to_path_buf());
        roots
    }
    /// PR19: run PR18 skill prime for an explicit real [`PromptOrigin::User`]
    /// turn on a native backend, at execution time (after all slash/media
    /// normalization is complete), returning a hidden `ConversationItem` to
    /// insert immediately before the real user item. Never primes for external
    /// backends, synthetic/unknown origins, subagents without an explicit user
    /// origin, or when disabled/degraded/cancelled.
    ///
    /// Fail-closed: on a hard prime error with `degrade_on_error = false` the
    /// typed error is returned BEFORE any user insertion / inference. The
    /// rendered block is PR18's already-escaped output, used literally (no
    /// entity re-decode / re-encode). Prompt and prime bodies never reach
    /// debug/telemetry.
    pub(crate) async fn maybe_inject_prime_reminder(
        self: &Arc<Self>,
        origin: &crate::session::PromptOrigin,
        user_query: &str,
        turn_cancel: &tokio_util::sync::CancellationToken,
    ) -> Result<PrimeInjectResult, acp::Error> {
        // ── Prime gate: ONLY an explicit real `User` + native backend. ─────
        let unchanged = || PrimeInjectResult {
            reminder: None,
            accounting: PrimeAccounting::Unchanged,
        };
        if !origin.prime_eligible() {
            return Ok(unchanged());
        }
        if self.execution_backend.get().is_external() {
            return Ok(unchanged());
        }
        // Per-home retrieval registry supplies the prime config and the
        // semantic service (child sessions share the registry but build their
        // own inventory from their own workspace below).
        let Some(registry) =
            crate::retrieval::registry_for_prime_home(xai_grok_config::grok_home())
        else {
            return Ok(unchanged());
        };
        // Turn-time truth: this is the snapshot the selectors run against, and
        // (on `Record`) the snapshot the accounting reports — not a later reload.
        let snapshot = registry.load();
        let skills_cfg = &snapshot.prime.skills;
        let agents_cfg = &snapshot.prime.agents;
        let skills_enabled = skills_cfg.enabled;
        let agents_enabled = agents_cfg.enabled;

        // Eligible + native + registry with both selectors disabled: record a
        // truthful `disabled` outcome with the loaded snapshot's generations.
        if !skills_enabled && !agents_enabled {
            return Ok(PrimeInjectResult {
                reminder: None,
                accounting: PrimeAccounting::Record(LastPrimeOutcome {
                    retrieval_snapshot_generation: Some(snapshot.generation),
                    graph_generation: Some(snapshot.graph_generation),
                    provider_generation: Some(snapshot.provider_generation),
                    status: PrimeOutcomeStatus::Disabled,
                    selection_mode: Some("disabled".into()),
                    readiness: Some("ready".into()),
                    ..LastPrimeOutcome::default()
                }),
            });
        }

        // Select skills from the raw user query so assembled context stays local.
        let prompt = user_query.to_string();
        let context_window = self
            .chat_state_handle
            .get_inference_settings()
            .await
            .map(|s| s.context_window.get());

        let mut primed_skill_names: Vec<String> = Vec::new();
        let mut recommended_agent_names: Vec<String> = Vec::new();
        let mut skill_injected_chars: u64 = 0;
        let mut skill_injected_tokens: u64 = 0;
        let mut degradation_kinds: Vec<crate::retrieval::DegradationKind> = Vec::new();
        let mut prime_skill_reminder: Option<ConversationItem> = None;
        let cwd: std::path::PathBuf = self.tool_context.cwd.to_path_buf();
        let grok_home = xai_grok_config::grok_home();

        // ── Skills selector (may produce the injected reminder) ─────────────
        if skills_enabled {
            // Authoritative eligible native skill snapshot + fresh revalidation
            // source (closes the rank-vs-load TOCTOU).
            let bridge = self.tool_bridge_handle();
            let eligible = bridge.eligible_native_skills().await;
            let refresh_bridge = Arc::clone(&bridge);
            let refresh = move || {
                let b = Arc::clone(&refresh_bridge);
                async move { b.eligible_native_skills().await }
            };

            // Workspace + trusted containment roots (canonicalized upstream).
            // The Grok home is included so bundled and user-scope native
            // skills — which always live under `$GROK_HOME`, never under the
            // workspace — stay primable. `validated_roots` still drops any
            // root that is not an ancestor of an eligible skill from the
            // authoritative snapshot, so an unrelated home adds nothing.
            let git_root = match xai_grok_workspace::session::git::discover_git_root(&cwd) {
                xai_grok_workspace::session::git::GitDiscoveryResult::Found(root) => Some(root),
                _ => None,
            };
            let mut trusted_roots =
                Self::prime_trusted_roots(&cwd, git_root.as_deref(), &grok_home);

            let semantic_profile = skills_cfg.retrieval_profile.clone();
            let explicit_skill = self.active_skill.lock().clone();
            let owned_inventory = self.prime_inventory().await;
            let semantic_service = registry.service();

            let input = crate::session::prime::PrimeInput {
                eligible_skills: &eligible,
                refresh_skills: &refresh,
                workspace_root: &cwd,
                trusted_roots: &trusted_roots,
                prompt: &prompt,
                explicit_skill: explicit_skill.as_deref(),
                config: skills_cfg.clone(),
                context_window,
                semantic_profile: semantic_profile.as_deref(),
                semantic_service: if semantic_profile.is_some() && snapshot.enabled {
                    Some(&semantic_service)
                } else {
                    None
                },
                inventory: Some(&owned_inventory),
                grok_home: Some(&grok_home),
                snapshot_generation: Some(snapshot.generation),
            };

            match crate::session::prime::run_prime_selection(&input, turn_cancel.clone()).await {
                Ok(sel) if sel.cancelled => return Ok(unchanged()),
                Ok(sel) => {
                    tracing::debug!(
                        session_id = %self.session_info.id.0,
                        selected_names = ?sel.budget_state.selected_names,
                        rendered_chars = sel.rendered.as_ref().map(|r| r.chars),
                        cancelled = sel.cancelled,
                        "skill prime run complete (secret-free; bodies omitted)"
                    );
                    primed_skill_names = sel.budget_state.selected_names.clone();
                    degradation_kinds.extend(sel.degradation_kinds());
                    if let Some(rendered) = sel.rendered
                        && !rendered.text.is_empty()
                    {
                        // PR18 rendered block, used literally (already single-pass
                        // escaped). Auto/empty/degraded omit the reminder.
                        prime_skill_reminder =
                            Some(xai_grok_inference_types::ConversationItem::system_reminder(
                                rendered.text,
                            ));
                        skill_injected_chars = rendered.chars as u64;
                        skill_injected_tokens = rendered.tokens_est as u64;
                    }
                }
                Err(crate::session::prime::PrimeError::SemanticRetrievalFailed) => {
                    if !skills_cfg.degrade_on_error {
                        return Err(acp::Error::internal_error().data(
                            "skill prime failed and degrade_on_error is disabled; \
                             refusing to continue without priming",
                        ));
                    }
                    degradation_kinds.push(crate::retrieval::DegradationKind::SemanticUnavailable);
                }
            }
        }

        // ── Agents selector (names-only accounting; never inserted) ─────────
        if agents_enabled {
            let authority = self.session_callable_agent_authority(Some(snapshot.generation));
            let plugin_handle = self.plugin_registry_handle.clone();
            let spec = std::sync::Arc::clone(&self.rebuild_spec);
            let cwd_for_refresh = std::path::PathBuf::from(&self.session_info.cwd);
            let parent_depth = self.tool_context.subagent_depth;
            let (current_agent, allowed_subagent_types) = {
                let agent_borrow = self.agent.borrow();
                let def = agent_borrow.definition();
                (def.name.clone(), def.allowed_subagent_types.clone())
            };
            let generation = snapshot.generation;
            let fallback_plugins = spec.plugin_registry.clone();
            let refresh = move || {
                let plugins = plugin_handle
                    .as_ref()
                    .and_then(|handle| handle.snapshot())
                    .or_else(|| fallback_plugins.clone());
                let spec = std::sync::Arc::clone(&spec);
                let cwd_for_refresh = cwd_for_refresh.clone();
                let current_agent = current_agent.clone();
                let allowed_subagent_types = allowed_subagent_types.clone();
                async move {
                    crate::session::prime::agents::capture_callable_agent_authority(
                        crate::session::prime::agents::CaptureCallableArgs {
                            parent_cwd: &cwd_for_refresh,
                            parent_depth,
                            current_agent: Some(current_agent),
                            plugin_registry: plugins,
                            subagent_toggle: &spec.subagent_toggle,
                            cli_agents: &spec.cli_agents,
                            allowed_subagent_types,
                            global_subagents_enabled: spec.subagents_enabled,
                            generation: Some(generation),
                        },
                    )
                }
            };
            let semantic_service = if agents_cfg.retrieval_profile.is_some() && snapshot.enabled {
                Some(registry.service())
            } else {
                None
            };
            let input = crate::session::prime::agents::AgentInput {
                agents: &authority.agents,
                refresh: &refresh,
                selected_skills: &primed_skill_names,
                prompt: &prompt,
                explicit_agent: None,
                config: agents_cfg.clone(),
                context_window,
                semantic_profile: agents_cfg.retrieval_profile.as_deref(),
                // The owned service stays alive for the whole run; the input
                // borrows it.
                semantic_service: semantic_service.as_ref(),
                grok_home: Some(&grok_home),
                workspace_root: &cwd,
            };
            let selection = match crate::session::prime::agents::run_prime_agent_selection(
                &input,
                turn_cancel.clone(),
            )
            .await
            {
                Ok(sel) => Some(sel),
                Err(crate::session::prime::PrimeError::SemanticRetrievalFailed) => {
                    if !agents_cfg.degrade_on_error {
                        // Fail-closed before user insertion, even if skills
                        // already produced a reminder; no partial accounting.
                        return Err(acp::Error::internal_error().data(
                            "agent prime failed; degrade_on_error is disabled; \
                                 refusing to continue without priming",
                        ));
                    }
                    degradation_kinds.push(crate::retrieval::DegradationKind::SemanticUnavailable);
                    None
                }
            };
            if let Some(selection) = selection {
                if selection.cancelled {
                    return Ok(unchanged());
                }
                tracing::debug!(
                    session_id = %self.session_info.id.0,
                    selected_names = ?selection.budget_state.selected_names,
                    cancelled = selection.cancelled,
                    "agent prime run complete (names-only; advisory; never inserted)"
                );
                recommended_agent_names = selection.budget_state.selected_names.clone();
                degradation_kinds.extend(selection.degradation_kinds());
            }
        }

        // ── Assemble the single `Record` outcome (secret-free). ─────────────
        let profile_used = if skills_enabled {
            skills_cfg.retrieval_profile.clone()
        } else {
            agents_cfg.retrieval_profile.clone()
        };
        let mut degradation: Vec<PrimeDegradationLabel> = Vec::new();
        for kind in degradation_kinds {
            let label = PrimeDegradationLabel::from_kind(kind);
            if !degradation.contains(&label) {
                degradation.push(label);
            }
        }
        let status = if degradation.is_empty() {
            PrimeOutcomeStatus::Primed
        } else {
            PrimeOutcomeStatus::Degraded
        };
        let selection_mode = if profile_used.is_some()
            && !degradation
                .iter()
                .any(|d| matches!(d, PrimeDegradationLabel::SemanticUnavailable))
        {
            Some("semantic".to_string())
        } else {
            Some("local".to_string())
        };
        let readiness = if degradation.iter().any(|d| {
            matches!(
                d,
                PrimeDegradationLabel::SemanticUnavailable
                    | PrimeDegradationLabel::ServiceDisabled
                    | PrimeDegradationLabel::ProfileMissing
            )
        }) {
            Some("unavailable".to_string())
        } else {
            Some("ready".to_string())
        };
        // Caps are reported only when the skills selector actually ran; the
        // agents selector is never injected, so it contributes no cap.
        let (max_total_chars, max_tokens) = if skills_enabled {
            (
                Some(skills_cfg.max_total_chars as u64),
                Some(skills_cfg.max_tokens as u64),
            )
        } else {
            (None, None)
        };

        Ok(PrimeInjectResult {
            reminder: prime_skill_reminder,
            accounting: PrimeAccounting::Record(LastPrimeOutcome {
                retrieval_profile: profile_used,
                retrieval_snapshot_generation: Some(snapshot.generation),
                graph_generation: Some(snapshot.graph_generation),
                provider_generation: Some(snapshot.provider_generation),
                primed_skill_names,
                recommended_agent_names,
                injected_chars: skill_injected_chars,
                injected_tokens: skill_injected_tokens,
                max_total_chars,
                max_tokens,
                degradation,
                status,
                selection_mode,
                readiness,
            }),
        })
    }
    #[tracing::instrument(
        name = "session.handle_prompt",
        skip_all,
        fields(
            session_id = %self.session_info.id.0,
            prompt_id = %prompt_id,
            prompt_length = tracing::field::Empty,
            command_name = tracing::field::Empty,
            command_source = tracing::field::Empty,
        )
    )]
    pub(super) async fn handle_prompt(
        self: &Arc<Self>,
        prompt_id: &str,
        origin: super::super::PromptOrigin,
        prompt_blocks: Vec<acp::ContentBlock>,
        prompt_mode: PromptMode,
        trace_gcs_config: Option<crate::session::repo_changes::TraceExportConfig>,
        artifact_tracker: Option<crate::upload::manifest::ArtifactTracker>,
        prompt_client_identifier: Option<String>,
        prompt_screen_mode: Option<String>,
        verbatim: bool,
        json_schema: Option<serde_json::Value>,
        persist_ack: Option<oneshot::Sender<()>>,
        parsed_prompt_tx: Option<oneshot::Sender<ParsedPromptInfo>>,
    ) -> PromptTurnResult {
        let handle_prompt_start = std::time::Instant::now();
        let prompt_length: usize = prompt_blocks
            .iter()
            .map(|b| match b {
                acp::ContentBlock::Text(t) => t.text.len(),
                _ => 0,
            })
            .sum();
        tracing::Span::current().record("prompt_length", prompt_length as i64);
        *self.active_skill.lock() = None;
        // Mid-session tersify level switch: re-stamp the style block in the
        // system head so the coming turn sees the new level. Cheap: string
        // swap + one head replace; skipped when nothing changed.
        self.refresh_tersify_style_impl().await;
        xai_grok_telemetry::unified_log::info(
            "shell.handle_prompt.start",
            Some(self.session_info.id.0.as_ref()),
            Some(serde_json::json!({
                "prompt_id": prompt_id,
                "block_count": prompt_blocks.len(),
            })),
        );
        // External-runtime preflight MUST run before turn_count increment and
        // durable user history append. Unavailable CLI (PR5 stub) fails closed
        // with InvalidRequest so the session remains switchable to native.
        if let Err(err) = self.preflight_external_execution_backend().await {
            tracing::warn!(
                session_id = %self.session_info.id.0,
                prompt_id = %prompt_id,
                error = %err,
                "handle_prompt: external execution preflight failed (no turn mutation)"
            );
            return Err(err.into_acp_error());
        }
        if let Some(completion_id) = origin.completion_id() {
            self.mark_completions_reported(&[completion_id]).await;
            if let Some(reservations) = &self.tool_context.task_completion_reservations {
                reservations.release(completion_id);
            }
        }
        if origin.is_client_user_prompt() {
            self.cancel_pending_recap_for_new_prompt();
        }
        *self.turn_start_prompt_mode.lock() = prompt_mode;
        *self.turn_prompt_mode.lock() = prompt_mode;
        self.signals_handle().increment_turn();
        self.reconcile_plan_mode_with_prompt(prompt_mode);
        let _turn_active_guard =
            TurnActiveGuard::activate(self.tool_context.is_turn_active.as_ref());
        let _session_turn_active_guard = TurnActiveGuard::activate(Some(&self.session_turn_active));
        let turn_start_input =
            xai_agent_lifecycle::TurnStartInput::new(!origin.is_client_user_prompt());
        for contributor in self.extension_registry.turn_lifecycle_contributors() {
            contributor.on_turn_start(&turn_start_input).await;
        }
        if let Ok(mut pending) = self.rewind_pending_prompt.lock()
            && let Some(prev_text) = pending.take()
        {
            let new_text = prompt_blocks.iter().fold(String::new(), |mut acc, b| {
                if let acp::ContentBlock::Text(t) = b {
                    acc.push_str(&t.text);
                }
                acc
            });
            if new_text.trim() == prev_text.trim() {
                self.signals_handle().record_regeneration();
            } else {
                self.signals_handle().record_edit_and_retry();
            }
        }
        if let Some(bash_command) = Self::extract_bash_command(&prompt_blocks) {
            return self
                .handle_direct_bash_command(prompt_id, bash_command, &prompt_blocks)
                .await;
        }
        let slash_skills = self
            .agent
            .borrow()
            .tool_bridge()
            .clone()
            .slash_skills()
            .await;
        let skill_rewrite = if crate::session::is_cursor_user_template(
            &self.agent.borrow().definition().user_message_template,
        ) {
            slash_commands::SkillSlashRewrite::Passthrough
        } else {
            slash_commands::SkillSlashRewrite::RewriteToRun
        };
        let availability = self.command_availability().await;
        let mut pending_skill_information: Option<String> = None;
        let (workflow_registry, named_workflows) = self.named_workflow_snapshot();
        let original_prompt_text = prompt_blocks.iter().fold(String::new(), |mut acc, b| {
            if let acp::ContentBlock::Text(t) = b {
                acc.push_str(&t.text);
            }
            acc
        });
        let prompt_blocks = match slash_commands::resolve(
            prompt_blocks,
            &slash_skills,
            availability,
            skill_rewrite,
            &named_workflows,
        ) {
            Ok(blocks) => blocks,
            Err(SlashCommandOutcome::Builtin(action)) => {
                let text_block =
                    |text: String| acp::ContentBlock::Text(acp::TextContent::new(text));
                let slash_used = xai_grok_telemetry::events::SlashCommandUsed {
                    command: action.command_name().to_string(),
                    args_provided: action.args_provided(),
                };
                {
                    let span = tracing::Span::current();
                    span.record("command_name", action.command_name());
                    span.record("command_source", "builtin");
                }
                match action {
                    BuiltinAction::GoalSet { .. }
                    | BuiltinAction::GoalResume
                    | BuiltinAction::GoalStatus
                    | BuiltinAction::GoalPause
                    | BuiltinAction::GoalClear
                    | BuiltinAction::DeepResearch { .. }
                    | BuiltinAction::WorkflowLaunch { .. }
                    | BuiltinAction::WorkflowManage { .. }
                    | BuiltinAction::Compact { .. }
                    | BuiltinAction::Dream
                    | BuiltinAction::FlushMemory
                        if self.execution_backend.get().is_external() =>
                    {
                        // Reject before any goal/workflow/memory/dream mutation.
                        xai_grok_telemetry::session_ctx::log_event(slash_used);
                        self.persist_host_turn_user_echo(&original_prompt_text, &origin);
                        let msg = format!(
                            "{} is not supported on Claude Agent (CLI, Experimental) sessions. \
                             Start /new with a native model.",
                            action.command_name()
                        );
                        self.send_host_turn_slash_command_output(&msg).await;
                        return ok_end_turn(0, None);
                    }
                    BuiltinAction::GoalSet {
                        objective,
                        token_budget,
                    } => {
                        xai_grok_telemetry::session_ctx::log_event(slash_used);
                        let reminder = self.setup_goal(&objective, token_budget).await;
                        vec![text_block(reminder)]
                    }
                    BuiltinAction::GoalResume => {
                        xai_grok_telemetry::session_ctx::log_event(slash_used);
                        match self.resume_goal().await {
                            GoalResumeOutcome::Inference { reminder, user_msg } => {
                                self.send_slash_command_output(&user_msg).await;
                                vec![text_block(reminder)]
                            }
                            GoalResumeOutcome::Message(msg) => {
                                self.persist_host_turn_user_echo(&original_prompt_text, &origin);
                                self.send_host_turn_slash_command_output(&msg).await;
                                return ok_end_turn(0, None);
                            }
                        }
                    }
                    BuiltinAction::WorkflowLaunch { name, input } => {
                        self.persist_host_turn_user_echo(&original_prompt_text, &origin);
                        let msg = self
                            .launch_named_workflow(&workflow_registry, &name, &input)
                            .await;
                        self.send_host_turn_slash_command_output(&msg).await;
                        return ok_end_turn(0, None);
                    }
                    _ => {
                        self.persist_host_turn_user_echo(&original_prompt_text, &origin);
                        return self.execute_builtin_slash_command(action).await;
                    }
                }
            }
            Err(SlashCommandOutcome::InvokeSkill {
                blocks: original_blocks,
                skills: parsed_skills,
            }) => {
                if let Some(first) = parsed_skills.first() {
                    *self.active_skill.lock() = Some(first.name.clone());
                    let span = tracing::Span::current();
                    span.record("command_name", first.name.as_str());
                    span.record(
                        "command_source",
                        if first.plugin_name.is_some() {
                            "plugin"
                        } else {
                            "skill"
                        },
                    );
                }
                for sk in &parsed_skills {
                    xai_grok_telemetry::session_ctx::log_event(
                        xai_grok_telemetry::events::SlashCommandUsed {
                            command: sk.name.clone(),
                            args_provided: !sk.args.is_empty(),
                        },
                    );
                    xai_grok_telemetry::session_ctx::log_event(
                        xai_grok_telemetry::events::SkillDispatched {
                            skill_name: sk.name.clone(),
                            plugin_source: sk.plugin_name.clone(),
                        },
                    );
                    let skill_source = if sk.plugin_name.is_some() {
                        "plugin"
                    } else {
                        crate::session::telemetry::skill_source_label(
                            &sk.skill_path,
                            self.session_info.cwd.as_str(),
                        )
                    };
                    tracing::info_span!(
                        "skill.activated",
                        skill_name = %sk.name,
                        invocation_trigger = "slash_command",
                        skill_source = skill_source,
                    )
                    .in_scope(|| {});
                    if let Some(ref pname) = sk.plugin_name {
                        xai_grok_telemetry::session_ctx::log_event(
                            xai_grok_telemetry::events::PluginUsed {
                                plugin_id: pname.clone(),
                                plugin_name: pname.clone(),
                                skill_name: Some(sk.name.clone()),
                                hook_event: None,
                                success: true,
                            },
                        );
                        tracing::info_span!(
                            "plugin.used",
                            plugin_name = %pname,
                            skill_name = %sk.name,
                        )
                        .in_scope(|| {});
                    }
                }
                pending_skill_information = slash_commands::build_skill_information_for_refs(
                    &parsed_skills,
                    &slash_skills,
                    &self.session_id_string(),
                )
                .await;
                original_blocks
            }
        };
        self.events.begin_turn();
        let model_id = self.current_model_id().await;
        let turn_number = self.chat_state_handle.get_prompt_index().await as u64;
        self.current_turn_number.set(turn_number);
        let yolo_mode = self.permissions.is_yolo_mode();
        let msg_count = self.chat_state_handle.get_conversation_len().await;
        let redirect_kind = if origin.prime_eligible() {
            self.events.take_prior_redirect_kind()
        } else {
            None
        };
        self.emit_event(crate::session::events::Event::TurnStarted {
            session_id: self.session_id_string(),
            turn_number,
            model_id: model_id.clone(),
            yolo_mode,
            conversation_message_count: msg_count,
            session_relationship: crate::session::events::SessionRelationship::Primary,
            schema_version: crate::session::events::EVENT_SCHEMA_VERSION.into(),
            redirect_kind,
        });
        self.observability_bridge
            .emit(
                xai_tool_protocol::session_event::SessionEvent::TurnStarted {
                    turn_number,
                    model_id: model_id.clone(),
                    yolo_mode,
                },
            )
            .await;
        self.send_before_turn_event(xai_tool_protocol::turn_hook::BeforeTurnPayload {
            turn_number: self.chat_state_handle.get_prompt_index().await as u64,
            model_id: model_id.clone(),
            yolo_mode: self.permissions.is_yolo_mode(),
            conversation_message_count: msg_count,
            session_relationship: xai_tool_protocol::turn_hook::DEFAULT_SESSION_RELATIONSHIP
                .to_string(),
            schema_version: crate::session::events::EVENT_SCHEMA_VERSION.to_string(),
        })
        .await;
        let turn_idx = self.chat_state_handle.get_prompt_index().await as u64;
        xai_grok_telemetry::session_ctx::log_session_event(crate::agent::session_metrics::Turn {
            session_id: self.session_info.id.0.to_string(),
            turn_number: turn_idx,
        });
        let current_prompt_index = self.chat_state_handle.get_prompt_index().await;
        xai_grok_telemetry::session_ctx::begin_prompt_id();
        let mut chunk_meta = serde_json::Map::new();
        chunk_meta.insert("modelId".into(), serde_json::json!(model_id));
        chunk_meta.insert(
            "promptIndex".into(),
            serde_json::json!(current_prompt_index),
        );
        if origin.hide_user_echo_from_scrollback() {
            chunk_meta.insert("hideFromScrollback".into(), serde_json::json!(true));
        }
        let user_chunk_meta = Some(chunk_meta);
        self.chat_state_handle.increment_prompt_index();
        let text = prompt_blocks.iter().fold(String::new(), |mut acc, b| {
            if let acp::ContentBlock::Text(t) = b {
                acc.push_str(&t.text);
            }
            acc
        });
        let trimmed = text.trim().to_string();
        if !trimmed.is_empty() {
            self.chat_state_handle.cache_prompt_text(trimmed);
        }
        *self.tool_context.prompt_index.lock().await = current_prompt_index;
        self.file_state_tracker
            .begin_prompt(current_prompt_index)
            .await;
        let echo_mode = user_echo_mode(&origin);
        let crate::session::prompt_parser::ParsedPrompt {
            mut context,
            query,
            skill_information: skill_info,
            images: mut raw_images,
            audios: raw_audios,
            is_cursor,
        } = match parse_prompt_with_skills(
            &prompt_blocks,
            self.tool_context.cwd.to_path_buf(),
            &self.session_info,
            verbatim,
            self.is_cursor_harness(),
            pending_skill_information.take().unwrap_or_default(),
        )
        .await
        {
            Ok(v) => v,
            Err(err) => {
                tracing::warn!("Invalid prompt: {}", err.message);
                return Err(err);
            }
        };
        let recovered = crate::session::placeholder_images::recover_orphan_placeholders(
            &query,
            &mut raw_images,
            std::path::Path::new(&self.session_info.cwd),
        );
        if recovered > 0 {
            tracing::info!(
                session_id = %self.session_info.id,
                recovered,
                "server-side placeholder fallback: loaded orphan image(s) from disk",
            );
        }
        let query = crate::session::placeholder_images::strip_paths_from_image_placeholders(query);
        let user_images = self
            .normalize_images_with_notices(&mut context, raw_images, is_cursor)
            .await;
        let (query, extra_images) = if !self.is_cursor_harness() {
            let extraction = xai_grok_tools::util::base64_images::extract_base64_images(query);
            if extraction.images.is_empty() {
                (extraction.text, Vec::new())
            } else {
                let cleaned_text = extraction.text;
                let count = extraction.images.len();
                tracing::info!(
                    session_id = %self.session_info.id,
                    count,
                    "base64 images extracted from user query",
                );
                let acp_imgs: Vec<agent_client_protocol::ImageContent> = extraction
                    .images
                    .into_iter()
                    .map(|img| agent_client_protocol::ImageContent::new(img.data, img.mime_type))
                    .collect();
                let nr = crate::session::image_normalize::normalize_images(acp_imgs, false).await;
                if !nr.re_encode_fallbacks.is_empty() {
                    tracing::warn!(
                        session_id = %self.session_info.id,
                        notes = %nr.re_encode_fallbacks.join(" "),
                        "Extracted user query image kept original after re-encode failure",
                    );
                }
                (cleaned_text, nr.images)
            }
        } else {
            (query, Vec::new())
        };
        let prime_query = query.clone();
        let assembled = crate::session::prompt_parser::ParsedPrompt::assemble_parts_with_skills(
            &context,
            &query,
            &skill_info,
            is_cursor,
        );
        let pre_truncation_text = assembled.clone();
        let (user_message, truncated_local_path) = if verbatim {
            (assembled, None)
        } else {
            self.maybe_truncate_large_prompt_with_skills(
                context,
                query,
                skill_info,
                is_cursor,
                current_prompt_index,
            )
            .await
        };
        let was_truncated = truncated_local_path.is_some();
        if let Some(tx) = parsed_prompt_tx {
            let _ = tx.send(ParsedPromptInfo {
                text: user_message.clone(),
                full_text: if was_truncated {
                    Some(pre_truncation_text)
                } else {
                    None
                },
                local_path: truncated_local_path,
            });
        }
        let _ = self
            .notifications
            .persistence_tx
            .send(PersistenceMsg::ContentChunk(PersistenceContentChunk::new(
                prompt_blocks.to_vec(),
            )));
        let model_id = self
            .chat_state_handle
            .get_inference_settings()
            .await
            .map(|c| c.model)
            .unwrap_or_default();
        if self.telemetry_enabled || xai_grok_telemetry::external::is_active() {
            let effective_client_identifier =
                prompt_client_identifier.or_else(|| self.client_identifier.clone());
            let ev = xai_grok_telemetry::events::PromptSubmitted {
                prompt_length: user_message.len(),
                model_id,
                client_identifier: effective_client_identifier,
                screen_mode: prompt_screen_mode,
                prompt_text: None,
            };
            xai_grok_telemetry::session_ctx::log_event_dual(self.telemetry_enabled, ev);
        }
        self.maybe_inject_mcp_reminder().await;
        self.maybe_inject_mcp_connecting_reminder().await;
        self.maybe_inject_date_rollover_reminder().await;
        self.inject_plan_mode_reminders().await;
        self.inject_resumed_tasks_reminder();
        if origin.prime_eligible() {
            if let Some(gate) = &self.tool_context.task_wake_suppressed {
                gate.set(false);
            }
            xai_grok_telemetry::unified_log::info(
                "shell.task_wake.gate_cleared",
                Some(self.session_info.id.0.as_ref()),
                Some(serde_json::json!({ "reason": "handle_prompt_user_start" })),
            );
            self.consume_deferred_completions_for_user_turn().await;
        }
        self.drain_between_turn_completions().await;
        self.inject_workflow_status_reminder().await;
        let active_supports_images = self
            .chat_state_handle
            .get_inference_settings()
            .await
            .and_then(|settings| settings.supports_image_input);
        let describe_user_images = self.is_cursor_harness() || active_supports_images != Some(true);
        let media = self.media_config.borrow().clone();
        if describe_user_images && !user_images.is_empty() {
            crate::session::media_pipeline::auxiliary_media_allowed(
                media.mode,
                crate::session::image_describe::ImageDescribeSource::UserAttachment,
            )
            .map_err(|error| acp::Error::invalid_request().data(error.to_string()))?;
        }
        // Normalize ACP audio immediately: confined session asset + text
        // envelope. Never persist an Audio conversation variant.
        let user_message = if raw_audios.is_empty() {
            user_message
        } else {
            let session_dir =
                crate::session::persistence::session_dir(&crate::session::info::Info {
                    id: self.session_info.id.clone(),
                    cwd: self.session_info.cwd.clone(),
                });
            let media = media.clone();
            // Gate B MediaStt: exact xAI session route before any STT bearer use.
            let stt = match self
                .resolve_media_stt_transcriber(media.audio_model.as_deref())
                .await
            {
                Ok((_route, tx)) => Some(tx),
                Err(error) => {
                    tracing::debug!(%error, "user-audio STT route closed");
                    None
                }
            };
            let mut envelopes = Vec::with_capacity(raw_audios.len());
            for audio in &raw_audios {
                envelopes.push(
                    crate::session::media_pipeline::normalize_acp_audio_to_envelope(
                        &session_dir,
                        &audio.data,
                        &audio.mime_type,
                        &media,
                        &self.media_descriptor_store,
                        crate::session::image_describe::ImageDescribeSource::UserAttachment,
                        &xai_grok_tools::util::ffmpeg::SystemProcessRunner,
                        stt.as_deref(),
                    )
                    .await,
                );
            }
            if envelopes.is_empty() {
                user_message
            } else {
                format!("{user_message}\n\n{}", envelopes.join("\n\n"))
            }
        };
        let user_message = if user_images.is_empty() {
            user_message
        } else if describe_user_images {
            self.transcribe_user_images(user_message, &user_images)
                .await?
        } else {
            let session_dir =
                crate::session::persistence::session_dir(&crate::session::info::Info {
                    id: self.session_info.id.clone(),
                    cwd: self.session_info.cwd.clone(),
                });
            crate::session::image_describe::persist_and_prepend_image_files(
                &session_dir,
                &user_images,
                &user_message,
            )
            .map_err(|e| {
                acp::Error::internal_error()
                    .data(format!("failed to save user images to assets dir: {e}"))
            })?
        };
        let attached_image_refs = if describe_user_images {
            Vec::new()
        } else {
            crate::session::placeholder_images::attached_image_references(&user_images)
        };
        self.tool_bridge_handle()
            .update_resource(xai_grok_tools::types::resources::AttachedImages(
                attached_image_refs,
            ))
            .await;
        let prompt_text_for_hook = user_message.clone();
        {
            if trace_gcs_config.is_some() {
                self.chat_state_handle.begin_turn_capture();
            }
            // The interrupt reminder applies to a real user turn (User) or a
            // promoted interjection (the user typed it mid-turn). Legacy
            // Unknown and parent-authored assignments are ambiguous, so they
            // are omitted here (is_user_typed is exhaustive).
            if origin.is_user_typed() {
                self.maybe_inject_interrupt_reminder().await;
            }
            let mut user_chat = match &origin {
                super::super::PromptOrigin::TaskCompleted { .. } => {
                    ConversationItem::task_completed(user_message)
                }
                super::super::PromptOrigin::SubagentCompleted { .. } => {
                    ConversationItem::subagent_completed(user_message)
                }
                super::super::PromptOrigin::WorkflowCompleted { .. } => {
                    ConversationItem::notification_drain(user_message)
                }
                super::super::PromptOrigin::NotificationDrain => {
                    ConversationItem::notification_drain(user_message)
                }
                super::super::PromptOrigin::GoalSummary => {
                    ConversationItem::goal_summary(user_message)
                }
                super::super::PromptOrigin::SchedulerFired => {
                    ConversationItem::scheduler_fired(user_message)
                }
                super::super::PromptOrigin::PlanResume
                | super::super::PromptOrigin::Interjection
                | super::super::PromptOrigin::SubagentAssignment
                | super::super::PromptOrigin::Unknown => ConversationItem::user(user_message),
                super::super::PromptOrigin::User => {
                    let mut item = ConversationItem::user(user_message);
                    if let Some(interrupt) = self
                        .events
                        .take_prior_interrupt_category()
                        .and_then(crate::session::events::prior_turn_interrupt_from_cancellation)
                    {
                        item.set_prior_turn_interrupt(interrupt);
                    }
                    item
                }
            };
            user_chat.set_prompt_index(current_prompt_index);
            if !describe_user_images {
                for image in &user_images {
                    user_chat.add_image(pick_user_image_url(image));
                }
                for image in &extra_images {
                    user_chat.add_image(format!("data:{};base64,{}", image.mime_type, image.data));
                }
            }
            // The prime run shares the turn's cancellation token, so a
            // cancel/abort stops semantic work and discards partial prime
            // without breaking the real user item lifecycle.
            let turn_cancel = self.turn_cancel.borrow().clone();
            let PrimeInjectResult {
                reminder: prime_reminder,
                accounting,
            } = match self
                .maybe_inject_prime_reminder(&origin, &prime_query, &turn_cancel)
                .await
            {
                Ok(result) => result,
                Err(error) => {
                    let message = error.to_string();
                    let input = xai_agent_lifecycle::TurnErrorInput { message: &message };
                    for contributor in self.extension_registry.turn_lifecycle_contributors() {
                        contributor.on_turn_error(&input).await;
                    }
                    return Err(error);
                }
            };
            // Hold finalized accounting until the user/reminder pair has been
            // accepted (and, when requested, crossed the persistence barrier).
            // Synthetic/external/cancelled/ineligible turns keep `None` and do
            // not overwrite the last eligible real-turn snapshot.
            let finalized_prime_outcome = match accounting {
                PrimeAccounting::Record(outcome) => Some(outcome),
                PrimeAccounting::Unchanged => None,
            };
            let mut items: Vec<ConversationItem> = prime_reminder.into_iter().collect();
            for item in &mut items {
                item.set_prompt_index(current_prompt_index);
            }
            items.push(user_chat);
            for block in &prompt_blocks {
                let update = acp::SessionUpdate::UserMessageChunk(
                    acp::ContentChunk::new(block.clone()).meta(user_chunk_meta.clone()),
                );
                let notification_meta = self.build_notification_meta();
                let notification =
                    acp::SessionNotification::new(self.session_info.id.clone(), update)
                        .meta(notification_meta.as_object().cloned());
                if echo_mode == UserEchoMode::PersistOnly {
                    let _ = self
                        .notifications
                        .persistence_tx
                        .send(PersistenceMsg::Update(
                            crate::session::storage::SessionUpdate::Acp(Box::new(notification)),
                        ));
                } else {
                    self.emit_notification_direct(notification).await;
                }
            }
            if let Some(ack) = persist_ack {
                if self
                    .chat_state_handle
                    .push_message_batch_and_ack(items)
                    .await
                    .is_some()
                {
                    let (flush_tx, flush_rx) = oneshot::channel();
                    if self
                        .notifications
                        .persistence_tx
                        .send(PersistenceMsg::FlushAndAck {
                            respond_to: flush_tx,
                        })
                        .is_ok()
                        && flush_rx.await.is_ok()
                    {
                        let _ = ack.send(());
                    } else {
                        // Surface the failure: the user pair was not durably
                        // accepted, so do not proceed to inference.
                        return Err(acp::Error::internal_error().data(
                            "persist flush barrier failed after prompt insertion; \
                             user message pair was not durably accepted",
                        ));
                    }
                } else {
                    return Err(acp::Error::internal_error()
                        .data("chat-state actor unavailable; user message pair was not accepted"));
                }
            } else {
                self.chat_state_handle.push_message_batch(items);
            }
            if let Some(outcome) = finalized_prime_outcome {
                *self.last_prime_outcome.borrow_mut() = Some(outcome);
            }
        }
        self.dispatch_hook(
            xai_grok_hooks::event::HookEventName::UserPromptSubmit,
            xai_grok_hooks::event::HookPayload::UserPromptSubmit {
                prompt: Some(prompt_text_for_hook.clone()),
            },
            Some(prompt_id),
            None,
        )
        .await;
        // External agent path: session-scoped runtime (persistent multi-turn
        // when capability-supported), no InferenceActor / Grok tool loop /
        // compaction / memory / goals / workflow machinery.
        if self.execution_backend.get().is_external() {
            let prompt_for_external = {
                // Prefer the hook text (post-parse); fall back to concatenated blocks.
                if !prompt_text_for_hook.is_empty() {
                    prompt_text_for_hook
                } else {
                    prompt_blocks.iter().fold(String::new(), |mut acc, b| {
                        if let acp::ContentBlock::Text(t) = b {
                            acc.push_str(&t.text);
                        }
                        acc
                    })
                }
            };
            return self
                .run_external_agent_turn(prompt_id, &prompt_for_external)
                .await;
        }
        let turn_scope_guard =
            TurnSubagentScopeGuard::new(self.current_prompt_id.clone(), prompt_id.to_string());
        let turn_model_id = self.current_model_id().await;
        let doom_event_model = turn_model_id.clone();
        let turn_timer = std::time::Instant::now();
        let result = {
            let mut round_trace = trace_gcs_config;
            let mut round_artifact = artifact_tracker;
            let mut stop_continuations_this_turn: u32 = 0;
            loop {
                if self.goal_harness_enabled() {
                    let goal_loop_active = self.goal_tracker.lock().status()
                        == Some(crate::session::goal_tracker::GoalStatus::Active);
                    self.set_goal_loop_active_resource(goal_loop_active).await;
                }
                let round = self
                    .process_conversation_turn_with_recovery(
                        prompt_id,
                        round_trace.take(),
                        round_artifact.take(),
                        json_schema.clone(),
                    )
                    .await;
                if !matches!(round, Ok(TurnOutcome::Completed { .. })) {
                    break round;
                }
                if matches!(
                    round,
                    Ok(TurnOutcome::Completed {
                        refusal: Some(_),
                        ..
                    })
                ) {
                    self.auto_pause_goal_if_active_with_message(
                        crate::session::goal_tracker::GoalPauseReason::Infra,
                        "The model provider refused this goal round. Use /goal resume to retry."
                            .to_string(),
                    )
                    .await;
                    break round;
                }
                let goal_active = laziness_injection_active(
                    self.goal_harness_enabled(),
                    self.goal_tracker.lock().status(),
                );
                if goal_active {
                    let decision = if self.goal_runs_on_workflow_engine() {
                        self.run_goal_round_end().await
                    } else {
                        self.run_goal_round_end_legacy().await
                    };
                    if let GoalRoundDecision::Continue(directive) = decision {
                        self.inject_goal_continuation_message(directive).await;
                        continue;
                    }
                }
                match self
                    .run_stop_gate(prompt_id, stop_continuations_this_turn)
                    .await
                {
                    StopGateDecision::AllowStop => break round,
                    StopGateDecision::KeepWorking { feedback } => {
                        stop_continuations_this_turn += 1;
                        self.chat_state_handle
                            .push_user_message(ConversationItem::stop_hook_feedback(feedback));
                    }
                }
            }
        };
        let turn_duration_ms = turn_timer.elapsed().as_millis() as u64;
        let handle_prompt_elapsed_ms = handle_prompt_start.elapsed().as_millis() as u64;
        xai_grok_telemetry::unified_log::info(
            "shell.handle_prompt.done",
            Some(self.session_info.id.0.as_ref()),
            Some(serde_json::json!({
                "prompt_id": prompt_id,
                "total_elapsed_ms": handle_prompt_elapsed_ms,
                "turn_elapsed_ms": turn_duration_ms,
                "pre_turn_ms": handle_prompt_elapsed_ms.saturating_sub(turn_duration_ms),
                "ok": result.is_ok(),
            })),
        );
        let turn_tool_count = self.events.tool_count_this_turn();
        let bridge_outcome = turn_result_to_hook_outcome(&result);
        self.observability_bridge
            .emit(xai_tool_protocol::session_event::SessionEvent::TurnEnded {
                turn_number: current_prompt_index as u64,
                outcome: bridge_outcome,
                duration_ms: turn_duration_ms,
                tool_call_count: turn_tool_count,
                model_id: turn_model_id.clone(),
            })
            .await;
        match &result {
            Ok(TurnOutcome::Completed { refusal, .. }) => {
                self.emit_turn_ended(
                    crate::session::events::TurnOutcomeLabel::Completed,
                    None,
                    None,
                );
                if let Some(explanation) = refusal {
                    let details = (!explanation.is_empty()).then(|| explanation.clone());
                    self.report_turn_end(
                        prompt_id,
                        TurnEnd::Failed {
                            error: xai_grok_hooks::event::StopFailureKind::InvalidRequest,
                            error_details: details.clone(),
                            last_assistant_message: details,
                        },
                    );
                }
                self.send_after_turn_event(xai_tool_protocol::turn_hook::AfterTurnPayload {
                    turn_number: current_prompt_index as u64,
                    outcome: xai_tool_protocol::turn_hook::TurnHookOutcome::Completed,
                    duration_ms: turn_duration_ms,
                    tool_call_count: turn_tool_count,
                    model_id: turn_model_id.clone(),
                    written_repo_paths: Vec::new(),
                    cancellation_category: None,
                    cancellation_context: None,
                })
                .await;
                xai_grok_telemetry::session_ctx::log_event(
                    xai_grok_telemetry::events::TurnCompleted {
                        outcome: xai_grok_telemetry::events::Outcome::Completed,
                        duration_ms: turn_duration_ms,
                        tool_call_count: turn_tool_count,
                        model_id: turn_model_id,
                        cancellation_category: None,
                        error_category: None,
                    },
                );
            }
            Ok(TurnOutcome::Cancelled { category, context }) => {
                self.emit_turn_ended(
                    crate::session::events::TurnOutcomeLabel::Cancelled,
                    *category,
                    context.clone(),
                );
                if let Some(cause) = category {
                    self.events.set_prior_interrupt_category(*cause);
                }
                self.send_after_turn_event(xai_tool_protocol::turn_hook::AfterTurnPayload {
                    turn_number: current_prompt_index as u64,
                    outcome: xai_tool_protocol::turn_hook::TurnHookOutcome::Cancelled,
                    duration_ms: turn_duration_ms,
                    tool_call_count: turn_tool_count,
                    model_id: turn_model_id.clone(),
                    written_repo_paths: Vec::new(),
                    cancellation_category: cancellation_category_to_wire_string(*category),
                    cancellation_context: context.clone(),
                })
                .await;
                xai_grok_telemetry::session_ctx::log_event(
                    xai_grok_telemetry::events::TurnCompleted {
                        outcome: xai_grok_telemetry::events::Outcome::Cancelled,
                        duration_ms: turn_duration_ms,
                        tool_call_count: turn_tool_count,
                        model_id: turn_model_id,
                        cancellation_category: category.map(|c| format!("{c:?}")),
                        error_category: None,
                    },
                );
            }
            Ok(TurnOutcome::MaxTurnsReached { limit }) => {
                tracing::info!(limit, "turn ended: max_turns reached");
                self.emit_turn_ended(
                    crate::session::events::TurnOutcomeLabel::Cancelled,
                    None,
                    Some(serde_json::json!({
                        "reason": "max_turns_reached",
                        "limit": limit,
                    })),
                );
                self.send_after_turn_event(xai_tool_protocol::turn_hook::AfterTurnPayload {
                    turn_number: current_prompt_index as u64,
                    outcome: xai_tool_protocol::turn_hook::TurnHookOutcome::Cancelled,
                    duration_ms: turn_duration_ms,
                    tool_call_count: turn_tool_count,
                    model_id: turn_model_id.clone(),
                    written_repo_paths: Vec::new(),
                    cancellation_category: None,
                    cancellation_context: Some(serde_json::json!({
                        "reason": "max_turns_reached",
                        "limit": limit,
                    })),
                })
                .await;
                xai_grok_telemetry::session_ctx::log_event(
                    xai_grok_telemetry::events::TurnCompleted {
                        outcome: xai_grok_telemetry::events::Outcome::Cancelled,
                        duration_ms: turn_duration_ms,
                        tool_call_count: turn_tool_count,
                        model_id: turn_model_id,
                        cancellation_category: Some("max_turns_reached".to_string()),
                        error_category: None,
                    },
                );
            }
            Err(err) => {
                self.emit_turn_ended(crate::session::events::TurnOutcomeLabel::Error, None, None);
                self.send_after_turn_event(xai_tool_protocol::turn_hook::AfterTurnPayload {
                    turn_number: current_prompt_index as u64,
                    outcome: xai_tool_protocol::turn_hook::TurnHookOutcome::Error,
                    duration_ms: turn_duration_ms,
                    tool_call_count: turn_tool_count,
                    model_id: turn_model_id.clone(),
                    written_repo_paths: Vec::new(),
                    cancellation_category: None,
                    cancellation_context: None,
                })
                .await;
                let error_category = Self::classify_turn_error(err);
                xai_grok_telemetry::session_ctx::log_session_event(
                    xai_grok_telemetry::events::ApiError {
                        error_category: error_category.clone(),
                        model_id: turn_model_id.clone(),
                        status_code: None,
                        duration_ms: Some(turn_duration_ms),
                        // Phase 8: additive provider diagnostics. The turn
                        // error path does not currently surface OpenRouter
                        // diagnostics from the `InferenceError` here; left as
                        // None (the field is additive, so xAI errors are
                        // unaffected). A follow-up can thread diagnostics
                        // through if the error carries them.
                        provider_name: None,
                        generation_id: None,
                    },
                );
                xai_grok_telemetry::session_ctx::log_event(
                    xai_grok_telemetry::events::TurnCompleted {
                        outcome: xai_grok_telemetry::events::Outcome::Error,
                        duration_ms: turn_duration_ms,
                        tool_call_count: turn_tool_count,
                        model_id: turn_model_id,
                        cancellation_category: None,
                        error_category: Some(error_category),
                    },
                );
                self.report_turn_end(
                    prompt_id,
                    TurnEnd::Failed {
                        error: Self::stop_failure_error_type(err),
                        error_details: Self::turn_error_detail(err),
                        last_assistant_message: Some(Self::format_turn_error_message(err)),
                    },
                );
            }
        }
        xai_grok_telemetry::session_ctx::log_session_event(
            crate::agent::session_metrics::TurnCompletedLifecycle {
                session_id: self.session_info.id.0.to_string(),
                turn_number: current_prompt_index as u64,
            },
        );
        let doom_tally = std::mem::take(&mut *self.doom_loop_turn_tally.lock());
        if doom_tally.fired() {
            xai_grok_telemetry::session_ctx::log_session_event(
                crate::agent::session_metrics::DoomLoopRecovery {
                    session_id: self.session_info.id.0.to_string(),
                    turn_number: current_prompt_index as u64,
                    attempts: doom_tally.attempts,
                    accepted_after_budget: doom_tally.accepted_after_budget,
                    top_trigger: doom_tally.top_trigger,
                    model: doom_event_model,
                },
            );
        }
        match &result {
            Ok(TurnOutcome::Completed { .. }) => {
                for contributor in self.extension_registry.turn_lifecycle_contributors() {
                    contributor
                        .on_turn_done(&xai_agent_lifecycle::TurnDoneInput)
                        .await;
                }
            }
            Ok(TurnOutcome::Cancelled { .. }) | Ok(TurnOutcome::MaxTurnsReached { .. }) => {
                let input = xai_agent_lifecycle::TurnAbortInput::new(
                    xai_agent_lifecycle::TurnAbortReason::Interrupted,
                );
                for contributor in self.extension_registry.turn_lifecycle_contributors() {
                    contributor.on_turn_abort(&input).await;
                }
            }
            Err(err) => {
                let message = err.to_string();
                let input = xai_agent_lifecycle::TurnErrorInput { message: &message };
                for contributor in self.extension_registry.turn_lifecycle_contributors() {
                    contributor.on_turn_error(&input).await;
                }
            }
        }
        if matches!(
            result,
            Ok(TurnOutcome::Cancelled { .. }) | Ok(TurnOutcome::MaxTurnsReached { .. })
        ) {
            self.cancel_running_turn_subagents();
        }
        self.flush_to_disk().await;
        self.file_state_tracker
            .end_prompt(&self.tool_context.fs, current_prompt_index)
            .await;
        if let Some(mut rewind_point) = self
            .file_state_tracker
            .get_rewind_point(current_prompt_index)
            .await
        {
            rewind_point.normalize_to_relative(self.tool_context.cwd.as_ref());
            let _ = self
                .notifications
                .persistence_tx
                .send(PersistenceMsg::RewindPoint(rewind_point));
        }
        match result {
            Ok(outcome) => {
                let usage = self.freeze_prompt_usage(prompt_id).await;
                drop(turn_scope_guard);
                self.chat_state_handle.flush();
                let total_tokens = self.chat_state_handle.get_total_tokens().await;
                let (stop_reason, mut snapshot, completion_kind, structured_output) = match outcome
                {
                    TurnOutcome::Completed {
                        snapshot,
                        structured_output,
                        refusal,
                        ..
                    } => (
                        if refusal.is_some() {
                            acp::StopReason::Refusal
                        } else {
                            acp::StopReason::EndTurn
                        },
                        *snapshot,
                        PromptCompletionKind::Completed,
                        structured_output,
                    ),
                    TurnOutcome::Cancelled { category, context } => {
                        let cancellation_ctx = context.and_then(|v| serde_json::from_value(v).ok());
                        (
                            acp::StopReason::Cancelled,
                            None,
                            PromptCompletionKind::Cancelled {
                                category,
                                context: cancellation_ctx,
                            },
                            None,
                        )
                    }
                    TurnOutcome::MaxTurnsReached { limit } => (
                        acp::StopReason::Cancelled,
                        None,
                        PromptCompletionKind::MaxTurnsReached { limit },
                        None,
                    ),
                };
                if let Some(snapshot) = snapshot.as_mut() {
                    self.apply_prompt_modes_to_snapshot(snapshot);
                }
                Ok(crate::session::commands::PromptTurnOk {
                    stop_reason,
                    total_tokens,
                    turn_snapshot: snapshot,
                    completion_kind,
                    structured_output,
                    usage,
                    tool_overrides: None,
                })
            }
            Err(e) => {
                let usage = self.freeze_prompt_usage(prompt_id).await;
                drop(turn_scope_guard);
                Err(crate::inference::error::attach_prompt_usage(e, usage))
            }
        }
    }
    /// Wait for turn-blocking subagents (up to 120s on the turn task),
    /// snapshot, clear sticky. Background children never gate the drain: the
    /// prompt report is marked incomplete immediately and their spend reaches
    /// the session ledger when they finish.
    /// Cancel intentionally skips this multi-second drain (actor-loop safety).
    pub(super) async fn freeze_prompt_usage(
        &self,
        prompt_id: &str,
    ) -> Option<crate::extensions::notification::PromptUsage> {
        const DRAIN: std::time::Duration = std::time::Duration::from_secs(120);
        self.freeze_prompt_usage_bounded(prompt_id, DRAIN).await
    }
    /// [`freeze_prompt_usage`] with an explicit drain bound, for tests.
    pub(super) async fn freeze_prompt_usage_bounded(
        &self,
        prompt_id: &str,
        max_wait: std::time::Duration,
    ) -> Option<crate::extensions::notification::PromptUsage> {
        let drain = self
            .drain_subagent_usage_for_prompt_bounded(prompt_id, max_wait)
            .await;
        self.finalize_usage_from_outcome(prompt_id, drain).await
    }
    /// Waits for turn-blocking folds only.
    /// `fail_closed` on timeout or query failure; sticky and `background_live`
    /// are report-level only (no ledger mark). Must run on the turn task (not
    /// the session actor loop) so folds can land.
    pub(super) async fn drain_subagent_usage_for_prompt_bounded(
        &self,
        prompt_id: &str,
        max_wait: std::time::Duration,
    ) -> UsageDrainOutcome {
        const POLL: std::time::Duration = std::time::Duration::from_millis(50);
        let deadline = std::time::Instant::now() + max_wait;
        loop {
            let reply = self.outstanding_reply_for_prompt(prompt_id).await;
            match reply.as_ref() {
                None => {
                    tracing::warn!(
                        prompt_id,
                        "outstanding subagent query failed; treating usage as incomplete"
                    );
                    return UsageDrainOutcome {
                        fail_closed: true,
                        background_live: false,
                        sticky_report: false,
                    };
                }
                Some(r) if r.live_ids.is_empty() => {
                    return UsageDrainOutcome {
                        fail_closed: false,
                        background_live: r.background_live,
                        sticky_report: r.subagent_usage_not_applied,
                    };
                }
                Some(r) => {
                    if std::time::Instant::now() >= deadline {
                        tracing::warn!(
                            prompt_id,
                            count = r.live_ids.len(),
                            max_wait_ms = max_wait.as_millis() as u64,
                            "subagent usage drain timed out; usage may under-count"
                        );
                        return UsageDrainOutcome {
                            fail_closed: true,
                            background_live: r.background_live,
                            sticky_report: r.subagent_usage_not_applied,
                        };
                    }
                }
            }
            tokio::time::sleep(POLL).await;
        }
    }
    pub(super) async fn snapshot_prompt_usage(
        &self,
    ) -> Option<crate::extensions::notification::PromptUsage> {
        self.snapshot_prompt_usage_marked(false).await
    }
    pub(super) async fn snapshot_prompt_usage_marked(
        &self,
        incomplete: bool,
    ) -> Option<crate::extensions::notification::PromptUsage> {
        let actor_background_spend = self
            .unattributed_background_usage
            .swap(false, std::sync::atomic::Ordering::Relaxed);
        let shared_background_spend = self
            .tool_context
            .unattributed_background_usage
            .swap(false, std::sync::atomic::Ordering::Relaxed);
        let incomplete = incomplete || actor_background_spend || shared_background_spend;
        match self.chat_state_handle.try_get_prompt_usage().await {
            Ok(ledger) => {
                let incomplete = incomplete || ledger.as_ref().is_some_and(|l| l.incomplete);
                crate::extensions::notification::PromptUsage::project_from_ledger(
                    ledger.as_ref(),
                    incomplete,
                )
            }
            Err(()) => {
                crate::extensions::notification::PromptUsage::project_from_ledger(None, true)
            }
        }
    }
    /// When freeze did not attach: incomplete if billed or may under-count; else omit.
    pub(super) async fn error_path_usage_fallback(
        &self,
        prompt_id: &str,
    ) -> Option<crate::extensions::notification::PromptUsage> {
        let may_undercount = Self::usage_incomplete_from_reply(
            self.outstanding_reply_for_prompt(prompt_id).await.as_ref(),
        );
        match self.chat_state_handle.try_get_prompt_usage().await {
            Ok(ledger) => crate::extensions::notification::PromptUsage::for_error_path(
                ledger.as_ref(),
                may_undercount,
            ),
            Err(()) => crate::extensions::notification::PromptUsage::for_error_path(None, true),
        }
    }
    /// Sticky incomplete for `prompt_id`, or the live pin when `None`.
    /// Returns true only if the coordinator acked the mark.
    pub(super) async fn mark_subagent_usage_not_applied(&self, prompt_id: Option<&str>) -> bool {
        let resolved = prompt_id
            .map(str::to_owned)
            .or_else(|| self.current_prompt_id.lock().ok().and_then(|g| g.clone()));
        let Some(pid) = resolved else {
            self.unattributed_background_usage
                .store(true, std::sync::atomic::Ordering::Relaxed);
            self.tool_context
                .unattributed_background_usage
                .store(true, std::sync::atomic::Ordering::Relaxed);
            return false;
        };
        let Some(tx) = &self.tool_context.subagent_event_tx else {
            return false;
        };
        use xai_grok_tools::implementations::grok_build::task::types::{
            SubagentEvent, SubagentMarkUsageNotAppliedRequest,
        };
        let (respond_to, ack) = tokio::sync::oneshot::channel();
        if tx
            .send(SubagentEvent::MarkUsageNotApplied(
                SubagentMarkUsageNotAppliedRequest {
                    prompt_id: pid,
                    respond_to,
                },
            ))
            .is_err()
        {
            return false;
        }
        ack.await.is_ok()
    }
    /// Drain this session's buffered mid-turn monitor events
    /// (`drain_owned` — leader mode shares the buffer) into ONE hidden
    /// synthetic user message, tagged `SyntheticReason::SystemReminder` so
    /// compaction/fork/pruning skip it. Deliberately a bare
    /// `push_user_message`, NOT `inject_synthetic_user_message`: the latter
    /// persists a `UserMessageChunk` to `updates.jsonl`, which resume
    /// replays — the raw XML would render as a user prompt. Clients see
    /// monitor events only via the structured `x.ai/monitor_event` channel.
    pub(crate) async fn inject_pending_monitor_events(&self) {
        let Some(buffer) = &self.tool_context.monitor_event_buffer else {
            return;
        };
        let mine = xai_grok_tools::implementations::grok_build::task::types::drain_owned(
            buffer,
            Some(self.session_info.id.0.as_ref()),
        );
        if mine.is_empty() {
            return;
        }
        let Some(body) = xai_grok_tools::reminders::task_completion::format_monitor_events(
            &mine,
            Some(&self.tool_context.task_output_tool_name),
        ) else {
            return;
        };
        let wrapped = xai_grok_tools::reminders::wrap_reminder(&body);
        self.chat_state_handle
            .push_user_message(ConversationItem::system_reminder(wrapped));
        tracing::info!(
            session_id = %self.session_info.id.0,
            count = mine.len(),
            "injected mid-turn monitor events as hidden synthetic user message"
        );
    }
    /// Per-turn hook called from the event-loop completion handler
    /// after every turn finishes. Two terminal branches when the
    /// goal is `Active` (`goal_active_now == true`):
    ///
    /// 1. **Success.** Reset `goal_continuation_streak` to 0, then call
    ///    `maybe_queue_goal_continuation`. That helper verifies any
    ///    pending completion via its turn-end drain, queues the
    ///    continuation reminder if the goal is still `Active`, and runs
    ///    the stop-detector to select the nudge flavor (generic vs.
    ///    bail-specific) and emit `Event::GoalPrematureStopDetected`.
    /// 2. **Non-success.** Increment `goal_continuation_streak`. At
    ///    [`GOAL_CONTINUATION_BACKOFF_THRESHOLD`] consecutive hits,
    ///    reset the streak and auto-pause with
    ///    `GoalPauseReason::BackOff`. No continuation is queued on this path: an
    ///    infra-error / cancelled turn rarely carries a deliberate
    ///    turn-final message, and stop-detection lives on the success
    ///    path inside `maybe_queue_goal_continuation`.
    ///
    /// When the goal is not `Active` (`goal_active_now == false` —
    /// the doom-loop / infra-error branches in the event loop ran
    /// before this method and already transitioned the goal out of
    /// Active), both branches are skipped: neither streak moves and the
    /// existing pause cause is preserved.
    pub(crate) async fn handle_turn_end(&self, turn_succeeded: bool) {
        let goal_active_now = laziness_injection_active(
            self.goal_harness_enabled(),
            self.goal_tracker.lock().status(),
        );
        if turn_succeeded && goal_active_now {
            self.goal_continuation_streak
                .store(0, std::sync::atomic::Ordering::Relaxed);
            self.maybe_queue_goal_continuation().await;
            return;
        }
        if !turn_succeeded && goal_active_now {
            let current_tokens = self.chat_state_handle.get_total_tokens().await as i64;
            if self.enforce_goal_token_budget(current_tokens).await {
                return;
            }
            let streak = self
                .goal_continuation_streak
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                + 1;
            if streak >= GOAL_CONTINUATION_BACKOFF_THRESHOLD {
                self.goal_continuation_streak
                    .store(0, std::sync::atomic::Ordering::Relaxed);
                self.auto_pause_goal_if_active(
                    crate::session::goal_tracker::GoalPauseReason::BackOff,
                )
                .await;
                self.send_slash_command_output(&format!(
                    "Goal auto-paused after {GOAL_CONTINUATION_BACKOFF_THRESHOLD} consecutive \
                     non-completing turns. The model is not making progress. \
                     Use /goal resume to retry or /goal clear to abandon."
                ))
                .await;
            }
        }
    }
    /// Wraps `process_conversation_turn` with auto-recovery for agents that opt in.
    ///
    /// Agents with a `completion_requirement` in their definition require the model
    /// to call a specific tool before finishing. If a prompt turn ends without that
    /// tool having been called, this method injects the recovery prompt and re-runs
    /// the turn with exponential backoff.
    ///
    /// Agents without `completion_requirement` bypass this entirely.
    #[tracing::instrument(
        name = "session.process_conversation_turn_with_recovery",
        skip_all,
        err,
        fields(req_id = %req_id, session_id = %self.session_info.id.0)
    )]
    pub(super) async fn process_conversation_turn_with_recovery(
        self: &Arc<Self>,
        req_id: &str,
        trace_gcs_config: Option<crate::session::repo_changes::TraceExportConfig>,
        artifact_tracker: Option<crate::upload::manifest::ArtifactTracker>,
        json_schema: Option<serde_json::Value>,
    ) -> Result<TurnOutcome, acp::Error> {
        let _ = self.compaction.auto_compact_suppressed.compare_exchange(
            crate::session::compaction_config::SUPPRESS_TURN,
            crate::session::compaction_config::SUPPRESS_NONE,
            std::sync::atomic::Ordering::Relaxed,
            std::sync::atomic::Ordering::Relaxed,
        );
        let agent_ref = self.agent.borrow();
        let completion_req = match agent_ref.completion_requirement() {
            Some(req) => req,
            None => {
                return self
                    .process_conversation_turn(
                        req_id,
                        trace_gcs_config,
                        artifact_tracker.as_ref(),
                        json_schema,
                    )
                    .await;
            }
        };
        let recovery = match &completion_req.recovery {
            Some(r) => r.clone(),
            None => {
                return self
                    .process_conversation_turn(
                        req_id,
                        trace_gcs_config,
                        artifact_tracker.as_ref(),
                        json_schema,
                    )
                    .await;
            }
        };
        let required_tool = completion_req.tool.clone();
        let recovery_prompt = completion_req.reminder.clone();
        let mut result = self
            .process_conversation_turn(
                req_id,
                trace_gcs_config.clone(),
                artifact_tracker.as_ref(),
                json_schema.clone(),
            )
            .await;
        if matches!(result, Ok(TurnOutcome::MaxTurnsReached { .. })) {
            return result;
        }
        if let Ok(TurnOutcome::Completed {
            ref tools_called, ..
        }) = result
            && tools_called.iter().any(|name| name == &required_tool)
        {
            tracing::info!(
                "Completion requirement satisfied (tool '{}' called) for session {}",
                required_tool,
                self.session_info.id.0,
            );
            return result;
        }
        let mut attempt = 0u32;
        loop {
            attempt += 1;
            let error_desc = match &result {
                Ok(_) => "Agent finished without completing required task".into(),
                Err(e) => format!("{e:?}"),
            };
            if attempt > recovery.max_retries {
                tracing::error!(
                    "Auto-recovery exhausted after {attempt} attempts for session {}: {error_desc}",
                    self.session_info.id.0,
                );
                self.send_xai_notification(XaiSessionUpdate::AutoRecoveryExhausted {
                    attempts: attempt,
                    error: error_desc,
                })
                .await;
                return result;
            }
            let delay_ms = std::cmp::min(
                recovery.base_delay_ms * 2u64.pow(attempt.saturating_sub(1)),
                recovery.max_delay_ms,
            );
            let delay = std::time::Duration::from_millis(delay_ms);
            tracing::warn!(
                "Auto-recovery attempt {}/{} for session {}: {error_desc}. Retrying in {}ms",
                attempt,
                recovery.max_retries,
                self.session_info.id.0,
                delay.as_millis(),
            );
            self.send_xai_notification(XaiSessionUpdate::AutoRecoveryStarted {
                attempt,
                max_retries: recovery.max_retries,
                error: error_desc,
                delay_ms: delay.as_millis() as u64,
            })
            .await;
            sleep(delay).await;
            let recovery_message = ConversationItem::auto_recovery(recovery_prompt.clone());
            self.chat_state_handle.push_user_message(recovery_message);
            result = self
                .process_conversation_turn(
                    req_id,
                    trace_gcs_config.clone(),
                    artifact_tracker.as_ref(),
                    None,
                )
                .await;
            if matches!(result, Ok(TurnOutcome::MaxTurnsReached { .. })) {
                return result;
            }
            if let Ok(TurnOutcome::Completed {
                ref tools_called, ..
            }) = result
                && tools_called.iter().any(|name| name == &required_tool)
            {
                tracing::info!(
                    "Completion requirement satisfied after {} recovery attempt(s) \
                     (tool '{}' called) for session {}",
                    attempt,
                    required_tool,
                    self.session_info.id.0,
                );
                return result;
            }
        }
    }
    /// Compute the first-turn memory reminder, if one should be injected.
    ///
    /// A block persisted by an earlier session segment (a prior `--resume`
    /// process, or a turn before a compaction) is reused verbatim — see
    /// [`conversation_has_memory_context`] for why re-searching is harmful.
    ///
    /// [`conversation_has_memory_context`]: crate::session::helpers::memory_context::conversation_has_memory_context
    pub(crate) async fn first_turn_memory_reminder(&self) -> Option<String> {
        if self
            .memory
            .context_injected
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return None;
        }
        self.memory
            .context_injected
            .store(true, std::sync::atomic::Ordering::Relaxed);
        if !self.memory.initial_injection_config.enabled {
            tracing::info!(
                target: xai_grok_telemetry::memory_log::TARGET,
                "MEMORY_INJECT: first-turn injection disabled by config"
            );
            return None;
        }
        let (Some(storage), Some(params)) =
            (self.memory.storage(), self.memory.backend_params.as_ref())
        else {
            return None;
        };
        let conversation = self.chat_state_handle.get_conversation().await;
        if crate::session::helpers::memory_context::conversation_has_memory_context(&conversation) {
            tracing::info!(
                target: xai_grok_telemetry::memory_log::TARGET,
                "MEMORY_INJECT: existing memory-context block present in system message -- skipping re-injection to preserve prompt cache"
            );
            return None;
        }
        use xai_grok_tools::types::memory_backend::MemoryBackend as _;
        let (injection_params, configured_min_score) =
            build_initial_injection_backend_params(params, &self.memory.initial_injection_config);
        let backend = crate::session::memory::MemoryBackendImpl::from_session_params(
            storage,
            &injection_params,
        );
        let raw_query =
            crate::session::helpers::session_compact::extract_last_real_user_query(&conversation)
                .unwrap_or_default();
        let was_greeting = raw_query.is_empty()
            || raw_query.len() < 20
            || crate::session::helpers::memory_context::is_greeting(&raw_query);
        let query = if was_greeting {
            "project conventions preferences architecture".to_string()
        } else {
            raw_query
        };
        let inject_start = std::time::Instant::now();
        let inject_results = backend.search(&query, 6, configured_min_score).await.ok();
        let result_count = inject_results.as_ref().map_or(0, |r| r.len());
        let top_score = inject_results
            .as_ref()
            .and_then(|r| r.first())
            .map_or(0.0, |r| r.score);
        let total_snippet_chars: usize = inject_results
            .as_ref()
            .map_or(0, |r| r.iter().map(|s| s.snippet.len()).sum());
        tracing::info!(
            target: xai_grok_telemetry::memory_log::TARGET,
            configured_min_score,
            "MEMORY_INJECT_SEARCH: results={result_count}"
        );
        xai_grok_telemetry::session_ctx::log_event(
            xai_grok_telemetry::memory_telemetry::MemoryInjection {
                session_id: self.session_info.id.to_string(),
                was_greeting_fallback: was_greeting,
                result_count,
                total_snippet_chars,
                top_score,
                configured_min_score,
                injection_duration_ms: inject_start.elapsed().as_millis() as u64,
            },
        );
        inject_results.and_then(|results| {
            crate::session::helpers::memory_context::format_memory_reminder(&results)
        })
    }
    /// Inspect `tool_calls` for a `StructuredOutput` call and decide the turn's
    /// next step, pushing the call's `tool_result` (correction / retry error /
    /// terminal) as a side effect. Validates the args against `validator` and
    /// bumps `retries` on a non-conforming retry.
    async fn handle_structured_output_tool_call(
        &self,
        tool_calls: &mut Vec<xai_grok_inference_types::conversation::ToolCall>,
        validator: &Result<jsonschema::Validator, String>,
        retries: &mut u32,
    ) -> StructuredOutputStep {
        let Some(pos) = tool_calls
            .iter()
            .position(|tc| tc.name == STRUCTURED_OUTPUT_TOOL)
        else {
            return StructuredOutputStep::Proceed;
        };
        if tool_calls.len() > 1 {
            for tc in tool_calls
                .iter()
                .filter(|tc| tc.name == STRUCTURED_OUTPUT_TOOL)
            {
                self.chat_state_handle
                    .push_tool_result(ConversationItem::tool_result(
                        tc.id.as_ref().to_owned(),
                        "Call StructuredOutput alone, exactly once, after all other tools finish.",
                    ));
            }
            tool_calls.retain(|tc| tc.name != STRUCTURED_OUTPUT_TOOL);
            return StructuredOutputStep::Proceed;
        }
        let call_id = tool_calls[pos].id.as_ref().to_owned();
        let validated = validate_structured_output(validator, &tool_calls[pos].arguments);
        if let Err(err) = &validated
            && *retries < STRUCTURED_OUTPUT_MAX_RETRIES
        {
            *retries += 1;
            self.chat_state_handle
                .push_tool_result(ConversationItem::tool_result(
                    call_id,
                    format!("{err}\nFix the arguments and call StructuredOutput again."),
                ));
            return StructuredOutputStep::Retry;
        }
        self.chat_state_handle
            .push_tool_result(ConversationItem::tool_result(
                call_id,
                match &validated {
                    Ok(_) => "Structured output accepted.".to_string(),
                    Err(err) => err.clone(),
                },
            ));
        StructuredOutputStep::Complete(validated)
    }
    /// Shared turn-completion bookkeeping (plan cleanup, signals snapshot +
    /// persistence, BigQuery turn delta, feedback prompt). Runs identically for
    /// the native and StructuredOutput-tool completion paths. Returns the
    /// turn-end snapshot for `TurnOutcome::Completed`.
    async fn finalize_turn_bookkeeping(
        &self,
        req_id: &str,
        conv_turn_start: std::time::Instant,
        turn_span_totals: &TurnSpanTotals,
        model_fingerprint: Option<String>,
    ) -> Option<TurnDeltaSnapshot> {
        self.emit_turn_end_plan_cleanup().await;
        self.signals_handle().record_turn_complete();
        let mut snapshot = self.signals_handle().take_turn_end_snapshot().await;
        if let Some(snap) = snapshot.as_mut() {
            self.apply_prompt_modes_to_snapshot(snap);
            snap.turn_input_tokens = turn_span_totals.input_tokens.max(0) as u64;
            snap.turn_output_tokens = turn_span_totals.output_tokens.max(0) as u64;
            snap.turn_cached_input_tokens = turn_span_totals.cache_read_tokens.max(0) as u64;
            for pr in &snap.delta.prs_created_this_turn {
                xai_grok_telemetry::session_ctx::log_event(xai_grok_telemetry::events::PrCreated {
                    source: pr.source,
                    had_commit_in_session: pr.had_commit_in_session,
                });
            }
        }
        if let Some(snap) = snapshot.as_ref() {
            let _ = self
                .notifications
                .persistence_tx
                .send(PersistenceMsg::Signals(snap.current.clone()));
        }
        self.feedback_manager
            .send_turn_delta_with_snapshot(
                snapshot.clone(),
                Some(req_id.to_string()),
                Some(conv_turn_start.elapsed().as_millis() as i64),
                Some("completed".to_string()),
                model_fingerprint,
            )
            .await;
        if let Some(request) = self
            .feedback_manager
            .maybe_request_feedback(Some(req_id.to_string()))
            .await
        {
            self.send_feedback_notification(request).await;
        }
        snapshot
    }
    #[tracing::instrument(
        name = "session.process_conversation_turn",
        skip_all,
        err,
        fields(
            session_id = %self.session_info.id.0,
            model_id,
            turn_tool_count,
            turn_model_calls,
            input_tokens = tracing::field::Empty,
            output_tokens = tracing::field::Empty,
            cache_read_tokens = tracing::field::Empty,
            stop_reason = tracing::field::Empty,
            response.has_tool_call = tracing::field::Empty,
            request_id = tracing::field::Empty,
            ttft_ms = tracing::field::Empty,
            mcp_server.name = tracing::field::Empty,
            mcp_tool.name = tracing::field::Empty,
            agent.name = tracing::field::Empty,
            skill.name = tracing::field::Empty,
            query_source = tracing::field::Empty,
            effort = tracing::field::Empty,
            attempt = tracing::field::Empty,
            parent_agent_id = tracing::field::Empty,
        )
    )]
    async fn process_conversation_turn(
        self: &Arc<Self>,
        req_id: &str,
        trace_gcs_config: Option<crate::session::repo_changes::TraceExportConfig>,
        artifact_tracker: Option<&crate::upload::manifest::ArtifactTracker>,
        json_schema: Option<serde_json::Value>,
    ) -> Result<TurnOutcome, acp::Error> {
        let language_policy = self.language_policy_for_turn();
        let user_supplied_schema = json_schema.is_some();
        let language_envelope_active = language_policy.is_some() && !user_supplied_schema;
        let json_schema = match (json_schema, language_policy.as_ref()) {
            (Some(user), _) => Some(user),
            (None, Some(policy)) => Some(xai_grok_inference_types::language_envelope_schema(
                &policy.conversation,
                &policy.artifact,
            )),
            (None, None) => None,
        };
        let conv_turn_start = std::time::Instant::now();
        self.maybe_refresh_model_metadata_on_resume().await;
        self.maybe_compact_on_model_switch().await?;
        self.chat_state_handle
            .record_turn_start(chrono::Utc::now().timestamp_millis());
        {
            let span = tracing::Span::current();
            if let Some(agent) = self.active_agent_type.lock().clone() {
                span.record("agent.name", agent.as_str());
            }
            if let Some(skill) = self.active_skill.lock().clone() {
                span.record("skill.name", skill.as_str());
            }
            span.record(
                "query_source",
                if self.startup_hints.is_subagent {
                    "subagent"
                } else {
                    "main"
                },
            );
            if let Some(parent) = self.startup_hints.parent_session_id.as_deref() {
                span.record("parent_agent_id", parent);
            }
        }
        if let Some(cfg) = self.chat_state_handle.get_inference_settings().await {
            let span = tracing::Span::current();
            span.record("model_id", cfg.model.as_str());
            if let Some(effort) = cfg.reasoning_effort {
                span.record("effort", effort.as_str());
            }
        }
        let mut prompt_timing = Some(crate::session::prompt_timing::PromptTiming::start());
        let tool_prep_start = std::time::Instant::now();
        let (tool_definitions, mcp_wait_ms) = self.prepare_tool_definitions_timed().await;
        let total_prep_ms = tool_prep_start.elapsed().as_millis() as u64;
        if let Some(ref mut pt) = prompt_timing {
            pt.record_tool_prep(mcp_wait_ms, total_prep_ms);
        }
        xai_grok_telemetry::unified_log::info(
            "shell.turn.tool_prep_done",
            Some(self.session_info.id.0.as_ref()),
            Some(serde_json::json!({
                "tool_count": tool_definitions.len(),
                "mcp_wait_ms": mcp_wait_ms,
                "total_prep_ms": total_prep_ms,
                "elapsed_since_turn_start_ms": conv_turn_start.elapsed().as_millis() as u64,
            })),
        );
        if let Some(ref gcs_config) = trace_gcs_config {
            let gcs_cfg = gcs_config.clone();
            let tool_defs = tool_definitions.clone();
            let manifest_clone = artifact_tracker.cloned();
            let auth_manager = self.auth_manager.clone();
            tokio::spawn(async move {
                crate::upload::trace::upload_tool_definitions(
                    gcs_cfg,
                    auth_manager,
                    &tool_defs,
                    manifest_clone.as_ref(),
                )
                .await;
            });
        }
        self.record_turn_model().await;
        let mut metrics_drop_guard = TurnMetrics::new();
        let mut turn_tools_called: Vec<String> = Vec::new();
        let mut tool_turn_count: usize = 1;
        let mut loop_index: u32 = 0;
        let mut identical_tool_calls = IdenticalToolCallRun::default();
        let mut todo_gate_fires: u32 = 0;
        let mut auth_retry_schedule = AuthRetrySchedule::new();
        let mut context_compaction_recovery_used = false;
        let mut turn_span_totals = TurnSpanTotals::default();
        let mut model_fingerprint: Option<String> = None;
        let mut structured_output_retries: u32 = 0;
        let structured_output_validator = json_schema.as_ref().map(|schema| {
            jsonschema::validator_for(schema).map_err(|e| format!("invalid output schema: {e}"))
        });
        let schema_ok = matches!(structured_output_validator, Some(Ok(_)));
        // Provider/model-aware: ChatCompletions/Responses always native for
        // user-supplied schemas; Messages uses native output_config.format
        // only when the durable model capability (supports_native_schema)
        // opts in. The language envelope is the exception: it always uses
        // the StructuredOutput-tool path so native json_schema cannot steal
        // the tool-call channel. Custom Messages keeps StructuredOutput-tool
        // fallback unless explicitly capable.
        let native_backend = if json_schema.is_some() {
            match self.chat_state_handle.get_inference_settings().await {
                Some(c) => c.effective_supports_native_schema(),
                None => {
                    tracing::warn!(
                        "structured output: no sampling config; using StructuredOutput tool"
                    );
                    false
                }
            }
        } else {
            false
        };
        let (structured_output_native, structured_output_tool) =
            structured_output_modes(schema_ok, native_backend, language_envelope_active);
        self.language_envelope_turn.set(language_envelope_active);
        // Language envelope no longer uses native json_schema, so live prose
        // must stream. Envelope JSON that still arrives as text is decoded
        // at persist time rather than buffered here.
        self.suppress_language_envelope_text.set(false);
        struct ClearLanguageEnvelopeTurn<'a>(&'a SessionActor);
        impl Drop for ClearLanguageEnvelopeTurn<'_> {
            fn drop(&mut self) {
                self.0.language_envelope_turn.set(false);
                self.0.suppress_language_envelope_text.set(false);
            }
        }
        let _clear_language_envelope_turn = ClearLanguageEnvelopeTurn(self);
        if let Some(policy) = language_policy.as_ref() {
            // Per-turn reminder so mid-session [language] changes apply on the
            // next turn without rebuilding the cached system prompt.
            self.push_system_reminder(&format!(
                "Conversational replies MUST be written in {}.\n\
ALL artifacts MUST be written in {}: source code, comments, documentation, \
test names, commit messages, pull-request text, subagent prompts, \
image/video prompts, and diagnostics.\n\
Preserve identifiers, file paths, and quoted text verbatim; do not translate them.",
                policy.conversation, policy.artifact
            ));
        }
        if structured_output_tool && !language_envelope_active {
            self.push_system_reminder(
                "A response schema is required. After any tool use, call the \
                 `StructuredOutput` tool exactly once with your final answer as its \
                 arguments; do not return the answer as text.",
            );
        } else if language_envelope_active && structured_output_tool {
            self.push_system_reminder(
                "You may call tools freely. When you are ready to give the \
user-visible answer, you MAY call the `StructuredOutput` tool exactly once \
with conversation_language, artifact_language, and response; you may also \
write the answer as ordinary text.",
            );
        }
        loop {
            self.emit_event(crate::session::events::Event::LoopStarted { loop_index });
            loop_index += 1;
            if identical_tool_calls.run_len >= MAX_CONSECUTIVE_IDENTICAL_TOOL_CALLS {
                let run_len = identical_tool_calls.run_len;
                let tool_name = identical_tool_calls.tool_name.clone();
                tracing::warn!(
                    session_id = %self.session_info.id,
                    tool_name = %tool_name,
                    run_len,
                    "action stationarity: stopping turn after repeated identical tool calls"
                );
                xai_grok_telemetry::unified_log::warn(
                    "shell.turn.action_stationarity_stop",
                    Some(self.session_info.id.0.as_ref()),
                    Some(serde_json::json!({
                        "loop_index": loop_index,
                        "tool_name": tool_name,
                        "run_len": run_len,
                    })),
                );
                let notice = format!(
                    "Stopped: the agent ran the same command (`{tool_name}`) {run_len} times in \
                     a row with no change in the result. If it's waiting on a long-running job, \
                     use a background task or the `monitor` tool (or a single `sleep` then check) \
                     instead of polling; otherwise send a new instruction."
                );
                self.send_update(
                    acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
                        acp::ContentBlock::Text(acp::TextContent::new(notice)),
                    )),
                    None,
                )
                .await;
                return Ok(TurnOutcome::Cancelled {
                    category: Some(
                        crate::session::events::CancellationCategory::ActionStationarity,
                    ),
                    context: Some(serde_json::json!({
                        "tool_name": tool_name,
                        "run_len": run_len,
                    })),
                });
            }
            self.drain_pending_interjections().await;
            self.flush_pending_skill_reminders().await;
            self.inject_pending_monitor_events().await;
            let memory_reminder = self.first_turn_memory_reminder().await;
            if memory_reminder.is_some() {
                self.memory
                    .injection_count
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                tracing::info!(
                    target: xai_grok_telemetry::memory_log::TARGET,
                    "MEMORY_INJECT: first-turn memory context injected"
                );
            }
            self.maybe_inject_mcp_reminder().await;
            if self.tool_context.task_output_token_budget.is_none()
                && self.two_pass_active()
                && !self.compaction.prefire.has_cache()
                && self.should_prefire_two_pass().await
                && self.compaction.prefire.try_begin()
            {
                let actor = std::sync::Arc::clone(self);
                let handle = tokio::task::spawn_local(async move {
                    actor.run_prefire_pass1().await;
                });
                self.compaction.prefire.set_handle(handle);
            }
            if self.tool_context.task_output_token_budget.is_none() {
                self.refresh_token_if_expired().await;
            }
            let compaction_strategy = self.agent.borrow().compaction_policy().strategy;
            if self.tool_context.task_output_token_budget.is_none()
                && !matches!(
                    compaction_strategy,
                    xai_grok_agent::CompactionStrategy::Rolling
                )
                && let Some(trigger_info) = self.check_auto_compact_needed().await
                && let Err(e) = self.run_compact_only(trigger_info).await
            {
                tracing::error!(error = %e, "Pre-sampling auto-compaction failed");
                if Self::is_auth_compact_error(&e) {
                    return Err(self.surface_compact_auth_failure(e).await);
                }
            }
            let backend_search_active = self.backend_search_active();
            tracing::debug!(
                backend_search_active,
                "backend_search: turn tool resolution"
            );
            let mut effective_tools: Vec<ToolSpec> =
                if let Some(ref override_tools) = self.forked_tool_override {
                    override_tools.clone()
                } else {
                    self.turn_base_tool_specs(&tool_definitions)
                };
            if structured_output_tool && let Some(schema) = json_schema.clone() {
                effective_tools.push(ToolSpec {
                    name: STRUCTURED_OUTPUT_TOOL.to_string(),
                    description: Some(
                        "Return your final answer as JSON matching the required schema. \
                         Call this exactly once, at the end."
                            .to_string(),
                    ),
                    parameters: schema,
                    strict: None,
                });
            }
            let build_req_start = std::time::Instant::now();
            let request = self
                .chat_state_handle
                .build_request(
                    effective_tools,
                    memory_reminder,
                    self.memory.is_enabled(),
                    trace_gcs_config.clone().map(
                        |cfg| -> Box<dyn crate::inference::TraceContext> {
                            Box::new(crate::inference::ConversationRequestTrace {
                                gcs_config: cfg,
                                artifact_tracker: artifact_tracker.cloned(),
                            })
                        },
                    ),
                    self.session_info.id.to_string(),
                    req_id.to_owned(),
                )
                .await
                .expect("chat state actor should be alive");
            xai_grok_telemetry::unified_log::debug(
                "shell.turn.build_request_done",
                Some(self.session_info.id.0.as_ref()),
                Some(serde_json::json!({
                    "build_request_ms": build_req_start.elapsed().as_millis() as u64,
                    "loop_index": loop_index,
                })),
            );
            let mut request = request;
            request.x_grok_session_id = Some(self.session_info.id.to_string());
            request.x_grok_turn_idx =
                Some(self.chat_state_handle.get_prompt_index().await.to_string());
            request.x_grok_agent_id = Some(xai_grok_telemetry::id::agent_id());
            if request.x_grok_deployment_id.is_none() {
                request.x_grok_deployment_id = crate::managed_config::resolve_deployment_id(
                    crate::managed_config::resolve_deployment_key().as_deref(),
                );
            }
            if structured_output_native {
                request.json_schema = json_schema.clone();
            }
            request.hosted_tools = self.hosted_tools_for_turn();
            request.max_output_tokens = self
                .tool_context
                .clamp_task_model_request(request.max_output_tokens)
                .map_err(|message| acp::Error::internal_error().data(message))?;
            self.emit_event(crate::session::events::Event::PhaseChanged {
                phase: crate::session::events::Phase::WaitingForModel,
            });
            self.observability_bridge
                .emit(
                    xai_tool_protocol::session_event::SessionEvent::PhaseChanged {
                        phase: xai_tool_protocol::session_event::SessionPhase::Sampling,
                    },
                )
                .await;
            xai_grok_telemetry::unified_log::info(
                "shell.turn.inference_start",
                Some(self.session_info.id.0.as_ref()),
                Some(serde_json::json!({
                    "loop_index": loop_index,
                    "elapsed_since_turn_start_ms": conv_turn_start.elapsed().as_millis() as u64,
                })),
            );
            let model_timer = std::time::Instant::now();
            // Defense-in-depth: preflight already ran at handle_prompt entry.
            // Re-check so a mid-session mode restore cannot reach InferenceActor.
            if let Err(err) = self.preflight_external_execution_backend().await {
                self.tool_context.fail_task_output_usage_closed();
                return Err(err.into_acp_error());
            }
            let (response, latency) = match self
                .run_turn_via_sampler(request.clone(), !context_compaction_recovery_used)
                .await
            {
                Ok(InferenceTurnOutcome::Response(r, latency)) => (r, latency),
                Err(error) => {
                    self.tool_context.fail_task_output_usage_closed();
                    return Err(error);
                }
                Ok(InferenceTurnOutcome::CompactAndResubmit) => {
                    context_compaction_recovery_used = true;
                    // Compaction is not an inference success; keep every auth
                    // counter unchanged until a model response actually lands.
                    continue;
                }
                Ok(InferenceTurnOutcome::RefreshAuthAndResubmit { credential, store }) => {
                    if auth_retry_schedule.reset_if_incident_spans_suspend() {
                        tracing::info!(
                            "auth 401 retry: incident spanned suspend; charged budget reset"
                        );
                    }
                    match auth_retry_schedule.on_recovered_401(credential) {
                        AuthRetryDecision::UnchargedResubmit { resubmit } => {
                            tracing::warn!(
                                resubmit,
                                "auth 401 retry: no credential was sent; paced resubmit"
                            );
                            self.send_xai_notification(XaiSessionUpdate::RetryState(
                                crate::extensions::notification::RetryState::Retrying {
                                    attempt: resubmit,
                                    max_retries: AuthRetrySchedule::MAX_UNCHARGED_RESUBMITS,
                                    reason: "Re-authenticated after a credential-less 401; retrying request"
                                        .to_string(),
                                    backoff_ms: Some(1_000),
                                    is_rate_limited: false,
                                    provider_name: None,
                                    provider_code: None,
                                },
                            ))
                            .await;
                            pace_uncharged_resubmit(store, self.auth_manager.as_ref()).await;
                            continue;
                        }
                        AuthRetryDecision::Backoff { attempt, delay } => {
                            let delay_ms = delay.as_millis() as u64;
                            tracing::warn!(
                                attempt,
                                delay_ms,
                                authenticated =
                                    credential == xai_grok_inference_types::SentCredential::Sent,
                                "auth 401 retry: backing off before resubmit"
                            );
                            xai_grok_telemetry::unified_log::warn(
                                "shell.turn.auth_retry_backoff",
                                Some(self.session_info.id.0.as_ref()),
                                Some(serde_json::json!({
                                    "loop_index": loop_index,
                                    "attempt": attempt,
                                    "max_retries": AuthRetrySchedule::MAX_RETRIES,
                                    "delay_ms": delay_ms,
                                })),
                            );
                            self.send_xai_notification(XaiSessionUpdate::RetryState(
                                crate::extensions::notification::RetryState::Retrying {
                                    attempt,
                                    max_retries: AuthRetrySchedule::MAX_RETRIES,
                                    reason: "Re-authenticated after 401; retrying request"
                                        .to_string(),
                                    backoff_ms: Some(delay_ms),
                                    is_rate_limited: false,
                                    provider_name: None,
                                    provider_code: None,
                                },
                            ))
                            .await;
                            sleep(delay).await;
                            continue;
                        }
                        decision @ (AuthRetryDecision::Exhausted
                        | AuthRetryDecision::RunawayGuard { .. }) => {
                            let (rejections, authenticated) = auth_retry_schedule.incident_counts();
                            let uncharged = auth_retry_schedule.uncharged_rejections();
                            let msg = match decision {
                                AuthRetryDecision::RunawayGuard { rejections } => format!(
                                    "Auth recovery kept succeeding, but {rejections} credential-less inference requests were rejected (401) without a successful response; stopping to prevent an infinite retry loop"
                                ),
                                _ if rejections == authenticated => format!(
                                    "Auth recovery succeeded, but {rejections} authenticated inference requests were still rejected (401); giving up after {} retries",
                                    AuthRetrySchedule::MAX_RETRIES
                                ),
                                _ => format!(
                                    "Auth retry budget exhausted after {rejections} charged post-recovery 401s ({authenticated} provably carried a credential; {uncharged} credential-less 401s were not charged)"
                                ),
                            };
                            tracing::error!(msg);
                            return Err(acp::Error::internal_error().data(
                                crate::inference::error::error_data_with_status(msg, Some(401)),
                            ));
                        }
                    }
                }
            };
            auth_retry_schedule.reset_on_success();
            context_compaction_recovery_used = false;
            let model_elapsed_ms = model_timer.elapsed().as_millis() as u64;
            let usage = response.usage.as_ref();
            let prompt_tokens = usage.map(|u| u.prompt_tokens);
            let cached_prompt_tokens = usage.map(|u| u.cached_prompt_tokens);
            let completion_tokens = usage.map(|u| u.completion_tokens);
            let reasoning_tokens = usage.map(|u| u.reasoning_tokens);
            let ttft_ms = latency.time_to_first_token_ms;
            let tokens_per_sec = match completion_tokens {
                Some(ct) if ct > 0 => {
                    let decode_ms = match ttft_ms {
                        Some(ttft) if model_elapsed_ms > ttft => model_elapsed_ms - ttft,
                        _ => model_elapsed_ms,
                    };
                    (decode_ms > 0).then(|| {
                        let tps = f64::from(ct) * 1000.0 / decode_ms as f64;
                        (tps * 10.0).round() / 10.0
                    })
                }
                _ => None,
            };
            xai_grok_telemetry::unified_log::info(
                "shell.turn.inference_done",
                Some(self.session_info.id.0.as_ref()),
                Some(serde_json::json!({
                    "loop_index": loop_index,
                    "model_elapsed_ms": model_elapsed_ms,
                    "elapsed_since_turn_start_ms": conv_turn_start.elapsed().as_millis() as u64,
                    "ttft_ms": ttft_ms,
                    "itl_p50_ms": latency.itl_p50_ms,
                    "attempts": latency.attempts,
                    "prompt_tokens": prompt_tokens,
                    "cached_prompt_tokens": cached_prompt_tokens,
                    "completion_tokens": completion_tokens,
                    "reasoning_tokens": reasoning_tokens,
                    "tokens_per_sec": tokens_per_sec,
                })),
            );
            if let Some(usage) = response.usage.as_ref() {
                self.chat_state_handle
                    .record_token_usage(u64::from(usage.total_tokens));
                self.send_available_commands_update().await;
            }
            turn_span_totals.record(&tracing::Span::current(), &response);
            let _ = self.compaction.auto_compact_suppressed.compare_exchange(
                crate::session::compaction_config::SUPPRESS_UNTIL_SUCCESS,
                crate::session::compaction_config::SUPPRESS_NONE,
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
            );
            self.clear_auth_compact_suppression();
            let model_duration_ms = model_timer.elapsed().as_millis() as u64;
            {
                let model_id = self.current_model_id().await;
                // Phase 8: provider/cost attrs on `ModelResponseReceived`.
                // `fallback_served_model` being `Some` implies OpenRouter
                // (the stream transform only sets it for that provider), so
                // the provider name and served model are derived from the
                // response. Cost ticks flow from the stream collector.
                let provider_name = response
                    .fallback_served_model
                    .as_ref()
                    .map(|_| "OpenRouter".to_owned());
                xai_grok_telemetry::session_ctx::log_event(
                    xai_grok_telemetry::events::ModelResponseReceived {
                        model_id,
                        duration_ms: model_duration_ms,
                        stop_reason: response
                            .stop_reason
                            .as_ref()
                            .map(|r| format!("{r:?}").to_ascii_lowercase()),
                        prompt_tokens: response.usage.as_ref().map(|u| u.prompt_tokens),
                        completion_tokens: response.usage.as_ref().map(|u| u.completion_tokens),
                        reasoning_tokens: response.usage.as_ref().map(|u| u.reasoning_tokens),
                        cached_prompt_tokens: response
                            .usage
                            .as_ref()
                            .map(|u| u.cached_prompt_tokens),
                        provider_name,
                        cost_usd_ticks: response.cost_usd_ticks,
                        is_byok: None,
                        generation_id: None,
                        served_model: response.fallback_served_model.clone(),
                    },
                );
            }
            self.record_response_token_usage(&response, Some(model_duration_ms));
            // Surface an OpenRouter fallback (served model differed from the
            // requested model) as a concise, non-modal scrollback note. The
            // stream transform only sets `fallback_served_model` for
            // `provider_identity == OpenRouter`, so a `Some` here is already
            // gated on the provider. Log the mismatch (models only, no
            // content) for diagnosis.
            if let Some(ref served) = response.fallback_served_model {
                let requested = self.current_model_id().await;
                tracing::warn!(
                    requested_model = %requested,
                    served_model = %served,
                    provider = "OpenRouter",
                    "openrouter fallback served: model differs from requested"
                );
                // Phase 8: emit the external OTEL `api_fallback_served`
                // event. Models only, no content or credentials.
                xai_grok_telemetry::session_ctx::log_event(
                    xai_grok_telemetry::events::ApiFallbackServed {
                        requested_model: requested,
                        served_model: served.clone(),
                        provider_name: "OpenRouter".to_owned(),
                    },
                );
                self.send_xai_notification(XaiSessionUpdate::HookAnnotation {
                    message: format!("served by {served} (fallback)"),
                })
                .await;
            }
            if let Some(pt) = prompt_timing.take() {
                let mcp_count = self.mcp_state.lock().await.configs.len() as u32;
                let mcp_tools = self
                    .agent
                    .borrow()
                    .tool_bridge()
                    .tool_definitions()
                    .await
                    .iter()
                    .filter(|t| t.function.name.contains("__"))
                    .count() as u32;
                let turn_index = self
                    .chat_state_handle
                    .get_prompt_index()
                    .await
                    .saturating_sub(1) as u32;
                pt.emit(
                    model_duration_ms,
                    turn_index,
                    mcp_count,
                    mcp_tools,
                    self.mcp_strategy,
                    self.current_model_id().await,
                );
            }
            let mut tool_calls = response.tool_calls().to_vec();
            metrics_drop_guard.record_model_response(tool_calls.len());
            if let Some(fp) = response
                .assistant()
                .and_then(|a| a.model_fingerprint.clone())
            {
                model_fingerprint = Some(fp);
            }
            let fallback_text = response.fallback_text();
            let stop_reason = response.stop_reason;
            let response_is_empty = response.is_empty();
            let turn_refused =
                stop_reason == Some(xai_grok_inference_types::StopReason::ContentFilter);
            let refusal_explanation = response.stop_message.clone();
            let final_answer_text = json_schema.is_some().then(|| response.assistant_text());
            let mut language_envelope_error: Option<String> = None;
            let mut items = response.items;
            // Native language-envelope rounds still decode/validate assistant
            // JSON. The StructuredOutput-tool path (the default for language)
            // must not treat ordinary prose as a schema failure — that is
            // what blocked tools after `[language].conversation` was set.
            // Opportunistically rewrite leftover envelope JSON below.
            if language_envelope_active
                && structured_output_native
                && let Some(validator) = structured_output_validator.as_ref()
                && !turn_refused
                && stop_reason != Some(xai_grok_inference_types::StopReason::Length)
            {
                match final_answer_text.as_ref() {
                    Some(text) => match validate_structured_output(validator, text) {
                        Ok(value) => match extract_language_response(&value) {
                            Some(decoded) => {
                                items = rewrite_assistant_items(items, &decoded);
                            }
                            None => {
                                items = rewrite_assistant_items(items, "");
                                if tool_calls.is_empty() {
                                    language_envelope_error = Some(
                                        "output does not match the required schema: missing response"
                                            .to_string(),
                                    );
                                }
                            }
                        },
                        Err(err) => {
                            items = rewrite_assistant_items(items, "");
                            if tool_calls.is_empty() {
                                language_envelope_error = Some(err);
                            }
                        }
                    },
                    None => {
                        if tool_calls.is_empty() {
                            language_envelope_error = Some(
                                "model output was not valid JSON: empty assistant text".into(),
                            );
                        }
                    }
                }
            } else if language_envelope_active {
                // Length / content-filter / no validator: still never persist
                // envelope JSON as assistant content.
                items = items
                    .into_iter()
                    .map(|item| match &item {
                        xai_grok_inference_types::ConversationItem::Assistant(a)
                            if is_language_envelope_json(a.content.as_ref()) =>
                        {
                            let decoded = decode_language_envelope_text(a.content.as_ref())
                                .unwrap_or_default();
                            rewrite_assistant_content(item, &decoded)
                        }
                        _ => item,
                    })
                    .collect();
            }
            let fallback_decoded = language_envelope_active.then(|| {
                items.iter().find_map(|item| match item {
                    xai_grok_inference_types::ConversationItem::Assistant(a) => {
                        Some(a.content.as_ref().to_owned())
                    }
                    _ => None,
                })
            });
            if let Some(err) = language_envelope_error.as_ref() {
                tracing::warn!(
                    error = %err,
                    "language envelope validation failed after provisional stream; resetting attempt"
                );
                self.send_buffered_xai_update(XaiSessionUpdate::StreamingAttemptReset)
                    .await;
            } else {
                for item in items {
                    match item {
                        xai_grok_inference_types::ConversationItem::Assistant(_) => {
                            self.record_assistant_response(item).await;
                        }
                        _ => {
                            self.chat_state_handle.push_tool_result(item);
                        }
                    }
                }
            }
            // Fallback AgentMessageChunk is the user-visible answer. Skip it on
            // tool-call rounds and never emit raw envelope JSON.
            if language_envelope_error.is_none()
                && tool_calls.is_empty()
                && let Some(text) = fallback_text
            {
                let text = fallback_decoded.flatten().unwrap_or(text);
                let text = if language_envelope_active {
                    if is_language_envelope_json(&text) {
                        decode_language_envelope_text(&text)
                    } else if text.trim().is_empty() {
                        None
                    } else {
                        Some(text)
                    }
                } else {
                    Some(text)
                };
                if let Some(text) = text {
                    tracing::warn!(
                        text_len = text.len(),
                        "emitting fallback AgentMessageChunk — no text chunks were streamed"
                    );
                    self.send_update(
                        acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
                            acp::ContentBlock::Text(acp::TextContent::new(text)),
                        )),
                        None,
                    )
                    .await;
                }
            }
            if turn_refused && response_is_empty {
                let mut notice = "The model provider refused to generate a response \
                     for this turn (content filter)."
                    .to_string();
                if let Some(explanation) = refusal_explanation.as_deref() {
                    notice.push_str("\n\nProvider explanation: ");
                    notice.push_str(explanation);
                }
                tracing::warn!(
                    has_explanation = refusal_explanation.is_some(),
                    "model response was a provider refusal — emitting notice chunk"
                );
                self.send_update(
                    acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
                        acp::ContentBlock::Text(acp::TextContent::new(notice)),
                    )),
                    None,
                )
                .await;
            }
            if tool_calls.is_empty() {
                if !schema_ok
                    && !turn_refused
                    && let Some(gate_cfg) = self.todo_gate_policy()
                {
                    let collected = self.collect_todo_gate_input(req_id).await;
                    let input = collected.as_input();
                    if let TodoGateDecision::Nudge { reminder, reason } = evaluate_todo_gate(&input)
                    {
                        if todo_gate_fires < gate_cfg.max_fires_per_prompt {
                            todo_gate_fires += 1;
                            tracing::info!(
                                prompt_id = %req_id,
                                pending = ?input.pending,
                                unbacked_in_progress = ?input.in_progress_unbacked,
                                backed_in_progress = ?input.in_progress_backed,
                                backing_task_count = input.backing_task_count,
                                todo_gate_fires,
                                reason = reason.as_str(),
                                "turn-end TodoGate: nudging model to advance remaining todos"
                            );
                            self.events
                                .emit(crate::session::events::Event::TodoGateFired {
                                    fires: todo_gate_fires,
                                    pending: input.pending.len(),
                                    in_progress: input.in_progress_unbacked.len()
                                        + input.in_progress_backed.len(),
                                    reason: reason.as_str(),
                                });
                            let rendered = self
                                .tool_bridge_handle()
                                .render_prompt(&reminder, &serde_json::json!({}))
                                .await
                                .unwrap_or(reminder);
                            self.push_system_reminder(&rendered);
                            continue;
                        }
                        let cap = gate_cfg.max_fires_per_prompt;
                        tracing::warn!(
                            prompt_id = %req_id,
                            todo_gate_cap = cap,
                            "turn-end TodoGate: exhausted retries, falling through"
                        );
                        self.events
                            .emit(crate::session::events::Event::TodoGateExhausted {
                                pending: input.pending.len(),
                            });
                        self.push_system_reminder(&format!(
                            "The agent attempted to end this turn {cap} times \
                             with todos still pending or in_progress. Falling through \
                             to user. If you want autonomous progress, prompt the agent \
                             to continue explicitly, or clean up the todo list."
                        ));
                    }
                }
                if self.drain_pending_interjections().await {
                    tracing::info!("Drained interjection(s) before turn completion — continuing");
                    continue;
                }
                let snapshot = self
                    .finalize_turn_bookkeeping(
                        req_id,
                        conv_turn_start,
                        &turn_span_totals,
                        model_fingerprint.clone(),
                    )
                    .await;
                if self.drain_pending_interjections().await {
                    tracing::info!(
                        "Drained late interjection(s) during turn-end bookkeeping — continuing"
                    );
                    continue;
                }
                // Refusal must never be treated as valid schema output.
                //
                // `StopReason::Length` is converted by the sampler actor into
                // `InferenceError::MaxTokensTruncation` before a Completed
                // outcome is delivered, so partial JSON from truncation never
                // reaches this branch on the normal InferenceClient path. The
                // Length arm below is defense-in-depth only.
                let structured_output = match (
                    language_envelope_error,
                    structured_output_validator.as_ref(),
                    final_answer_text.as_ref(),
                    turn_refused,
                    stop_reason,
                ) {
                    (Some(err), _, _, _, _) => Some(Err(err)),
                    (_, Some(_), _, true, _) => Some(Err(
                        "model refused to produce structured output (content filter)".to_string(),
                    )),
                    (_, Some(_), _, _, Some(xai_grok_inference_types::StopReason::Length)) => Some(
                        Err("model hit max_tokens before completing structured output".to_string()),
                    ),
                    // Language envelope uses StructuredOutput-tool; a prose
                    // final answer is valid. Do not treat assistant text as
                    // the envelope unless the native path produced one.
                    (_, Some(_), _, false, _)
                        if language_envelope_active && structured_output_tool =>
                    {
                        None
                    }
                    (_, Some(validator), Some(text), false, _) => {
                        Some(validate_structured_output(validator, text))
                    }
                    _ => None,
                };
                return Ok(TurnOutcome::Completed {
                    snapshot: Box::new(snapshot),
                    tools_called: turn_tools_called,
                    structured_output,
                    refusal: turn_refused.then(|| refusal_explanation.clone().unwrap_or_default()),
                });
            }
            if structured_output_tool && let Some(validator) = structured_output_validator.as_ref()
            {
                match self
                    .handle_structured_output_tool_call(
                        &mut tool_calls,
                        validator,
                        &mut structured_output_retries,
                    )
                    .await
                {
                    StructuredOutputStep::Complete(validated) => {
                        turn_tools_called.push(STRUCTURED_OUTPUT_TOOL.to_string());
                        if language_envelope_active {
                            match &validated {
                                Ok(value) => {
                                    if let Some(decoded) = extract_language_response(value) {
                                        self.send_update(
                                            acp::SessionUpdate::AgentMessageChunk(
                                                acp::ContentChunk::new(acp::ContentBlock::Text(
                                                    acp::TextContent::new(decoded),
                                                )),
                                            ),
                                            None,
                                        )
                                        .await;
                                    }
                                }
                                Err(_) => {}
                            }
                        }
                        let snapshot = self
                            .finalize_turn_bookkeeping(
                                req_id,
                                conv_turn_start,
                                &turn_span_totals,
                                model_fingerprint.clone(),
                            )
                            .await;
                        return Ok(TurnOutcome::Completed {
                            snapshot: Box::new(snapshot),
                            tools_called: turn_tools_called,
                            structured_output: Some(validated),
                            refusal: None,
                        });
                    }
                    StructuredOutputStep::Retry => continue,
                    StructuredOutputStep::Proceed => {}
                }
            }
            for tc in &tool_calls {
                if let Some((server, tool)) =
                    crate::session::mcp_servers::parse_mcp_tool_name(&tc.name)
                {
                    let span = tracing::Span::current();
                    span.record("mcp_server.name", server.as_str());
                    span.record("mcp_tool.name", tool.as_str());
                }
                turn_tools_called.push(tc.name.clone());
            }
            let step_signature = tool_calls
                .iter()
                .map(|tc| format!("{}\u{1f}{}", tc.name, tc.arguments.as_ref()))
                .collect::<Vec<_>>()
                .join("\u{1e}");
            let step_tool_name = tool_calls
                .first()
                .map(|tc| tc.name.clone())
                .unwrap_or_default();
            let identical_run_len = identical_tool_calls.observe(&step_signature, &step_tool_name);
            if identical_run_len == NUDGE_AFTER_IDENTICAL_TOOL_CALLS {
                tracing::warn!(
                    session_id = %self.session_info.id,
                    tool_name = %step_tool_name,
                    run_len = identical_run_len,
                    "action stationarity: nudging model to break repeated identical tool calls"
                );
                xai_grok_telemetry::unified_log::warn(
                    "shell.turn.action_stationarity_nudge",
                    Some(self.session_info.id.0.as_ref()),
                    Some(serde_json::json!({
                        "loop_index": loop_index,
                        "tool_name": step_tool_name,
                        "run_len": identical_run_len,
                    })),
                );
                let reminder = self
                    .tool_bridge_handle()
                    .render_prompt(
                        ACTION_STATIONARITY_NUDGE_TEMPLATE,
                        &serde_json::json!({
                            "tool_name": step_tool_name,
                            "run_len": identical_run_len,
                        }),
                    )
                    .await
                    .unwrap_or_else(|| ACTION_STATIONARITY_NUDGE_TEMPLATE.to_string());
                self.push_system_reminder(&reminder);
            }
            let tool_call_responses: Vec<ToolCallResponse> = tool_calls
                .into_iter()
                .map(|tc| ToolCallResponse {
                    id: tc.id.as_ref().to_owned(),
                    kind: "function".to_string(),
                    function: crate::inference::types::ToolCallFunction {
                        name: tc.name,
                        arguments: tc.arguments.as_ref().to_owned(),
                    },
                })
                .collect();
            self.emit_event(crate::session::events::Event::PhaseChanged {
                phase: crate::session::events::Phase::ToolExecution,
            });
            self.observability_bridge
                .emit(
                    xai_tool_protocol::session_event::SessionEvent::PhaseChanged {
                        phase: xai_tool_protocol::session_event::SessionPhase::ToolExecution,
                    },
                )
                .await;
            let execute_tool_calls_result = self.execute_tool_calls(tool_call_responses).await;
            match execute_tool_calls_result {
                Ok(ToolLoop::PermissionReject { tool_name, reason }) => {
                    return Ok(TurnOutcome::Cancelled {
                        category: Some(
                            crate::session::events::CancellationCategory::PermissionRejected,
                        ),
                        context: Some(serde_json::json!({
                            "tool_name": tool_name,
                            "reason": reason,
                        })),
                    });
                }
                Ok(ToolLoop::HookDenied { .. }) => {}
                Ok(ToolLoop::Cancelled) => {
                    return Ok(TurnOutcome::Cancelled {
                        category: Some(
                            crate::session::events::CancellationCategory::PermissionCancelled,
                        ),
                        context: None,
                    });
                }
                Ok(ToolLoop::FollowupMessage(followup_message)) => {
                    self.add_followup_message_as_user_turn(&followup_message)
                        .await;
                    continue;
                }
                _ => {}
            }
            let next_turn = tool_turn_count + 1;
            if let Some(limit) = self.max_turns
                && next_turn > limit
            {
                tracing::info!(
                    session_id = %self.session_info.id,
                    tool_turn_count,
                    limit,
                    "max-turns limit reached, stopping"
                );
                return Ok(TurnOutcome::MaxTurnsReached { limit });
            }
            tool_turn_count = next_turn;
            if self.tool_context.task_output_token_budget.is_none()
                && let Some(trigger_info) = self.check_preflight_overflow().await
            {
                if let Err(e) = self.run_compact_only(trigger_info).await {
                    tracing::error!(error = %e, "Preflight overflow compaction failed");
                    if Self::is_auth_compact_error(&e) {
                        return Err(self.surface_compact_auth_failure(e).await);
                    }
                }
                continue;
            }
        }
    }
}
const MAX_CONSECUTIVE_IDENTICAL_TOOL_CALLS: u32 = 16;
const NUDGE_AFTER_IDENTICAL_TOOL_CALLS: u32 = 8;
const _: () = assert!(NUDGE_AFTER_IDENTICAL_TOOL_CALLS < MAX_CONSECUTIVE_IDENTICAL_TOOL_CALLS);
const ACTION_STATIONARITY_NUDGE_TEMPLATE: &str = "You have called the same tool \
     (`${{ tool_name }}`) with the exact same arguments ${{ run_len }} times in a row, \
     getting the same result each time — you appear to be stuck in a polling loop. Stop \
     repeating this call. If you are waiting on a long-running job or command, use a \
     background task${%- if tools.by_kind.monitor %} or the `${{ tools.by_kind.monitor }}` \
     tool${%- endif %}, or run a single `sleep` and then check once — do not poll in a tight \
     loop. If you cannot make progress, stop and tell the user what you are waiting for. This \
     turn will be halted automatically if the identical call keeps repeating.";
fn hash_step_signature(signature: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    signature.hash(&mut hasher);
    hasher.finish()
}
#[derive(Default)]
struct IdenticalToolCallRun {
    last_signature_hash: Option<u64>,
    tool_name: String,
    run_len: u32,
}
impl IdenticalToolCallRun {
    fn observe(&mut self, signature: &str, tool_name: &str) -> u32 {
        let hash = hash_step_signature(signature);
        if self.last_signature_hash == Some(hash) {
            self.run_len += 1;
        } else {
            self.run_len = 1;
            self.last_signature_hash = Some(hash);
        }
        self.tool_name = tool_name.to_string();
        self.run_len
    }
}
#[cfg(test)]
mod identical_tool_call_run_tests {
    use super::{IdenticalToolCallRun, MAX_CONSECUTIVE_IDENTICAL_TOOL_CALLS};
    #[test]
    fn counts_consecutive_identical_calls() {
        let mut run = IdenticalToolCallRun::default();
        let sig = "run_terminal_cmd\u{1f}{\"command\":\"squeue\"}";
        assert_eq!(run.observe(sig, "run_terminal_cmd"), 1);
        assert_eq!(run.observe(sig, "run_terminal_cmd"), 2);
        assert_eq!(run.observe(sig, "run_terminal_cmd"), 3);
    }
    #[test]
    fn a_different_call_resets_the_run() {
        let mut run = IdenticalToolCallRun::default();
        run.observe("a", "a");
        run.observe("a", "a");
        assert_eq!(run.observe("b", "b"), 1, "a different signature resets");
        assert_eq!(run.observe("b", "b"), 2);
        assert_eq!(run.tool_name, "b");
        assert_eq!(
            run.observe("a", "a"),
            1,
            "not consecutive with the first run"
        );
    }
    #[test]
    fn run_reaches_the_bound_after_n_identical_calls() {
        let mut run = IdenticalToolCallRun::default();
        let mut last = 0;
        for _ in 0..MAX_CONSECUTIVE_IDENTICAL_TOOL_CALLS {
            last = run.observe("same", "same");
        }
        assert_eq!(last, MAX_CONSECUTIVE_IDENTICAL_TOOL_CALLS);
    }
}
#[cfg(test)]
mod user_echo_broadcast_tests {
    use super::PromptOrigin;
    use super::{UserEchoMode, user_echo_mode};
    /// Notification-drain: persisted (rewind/fork count user-chunk runs as
    /// turn boundaries) but never broadcast live; the pager hides it via the
    /// `hideFromScrollback` chunk meta.
    #[test]
    fn notification_drain_turn_is_persist_only() {
        assert_eq!(
            user_echo_mode(&PromptOrigin::NotificationDrain),
            UserEchoMode::PersistOnly
        );
    }
    /// Real user prompts, cron (`/loop`) fires, and other turns still broadcast
    /// live so multi-client / dashboard viewers stay in sync.
    #[test]
    fn user_and_cron_turns_broadcast_live() {
        assert_eq!(user_echo_mode(&PromptOrigin::User), UserEchoMode::Broadcast);
        assert_eq!(
            user_echo_mode(&PromptOrigin::SchedulerFired),
            UserEchoMode::Broadcast
        );
        assert_eq!(
            user_echo_mode(&PromptOrigin::TaskCompleted {
                task_id: "bg-1".into()
            }),
            UserEchoMode::Broadcast
        );
        assert_eq!(
            user_echo_mode(&PromptOrigin::SubagentCompleted {
                subagent_id: "xyz".into()
            }),
            UserEchoMode::Broadcast
        );
    }
    /// Interject-fallback turns are persist-only: every pane already rendered
    /// the text from the `x.ai/session/interjection` broadcast, so a live
    /// echo would duplicate the block.
    #[test]
    fn interject_fallback_turn_is_persist_only() {
        assert_eq!(
            user_echo_mode(&PromptOrigin::Interjection),
            UserEchoMode::PersistOnly
        );
    }
}
#[cfg(test)]
mod structured_output_validation_tests {
    use super::{
        decode_language_envelope_text, extract_language_response, is_language_envelope_json,
        rewrite_assistant_content, rewrite_assistant_items, structured_output_modes,
        validate_structured_output,
    };
    use crate::inference::ConversationItem;
    fn validator() -> Result<jsonschema::Validator, String> {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {"name": {"type": "string"}, "age": {"type": "integer"}},
            "required": ["name", "age"],
            "additionalProperties": false,
        });
        jsonschema::validator_for(&schema).map_err(|e| e.to_string())
    }
    #[test]
    fn accepts_conforming_json() {
        let v = validate_structured_output(&validator(), r#"{"name":"alice","age":30}"#).unwrap();
        assert_eq!(v["name"], "alice");
    }
    #[test]
    fn rejects_non_json() {
        let err = validate_structured_output(&validator(), "not json").unwrap_err();
        assert!(err.starts_with("model output was not valid JSON: "));
    }
    #[test]
    fn rejects_schema_violation() {
        let err = validate_structured_output(&validator(), r#"{"name":"alice"}"#).unwrap_err();
        assert!(err.starts_with("output does not match the required schema: "));
    }
    #[test]
    fn surfaces_invalid_schema_error() {
        let bad: Result<jsonschema::Validator, String> = Err("invalid output schema: boom".into());
        let err = validate_structured_output(&bad, r#"{"name":"alice","age":1}"#).unwrap_err();
        assert_eq!(err, "invalid output schema: boom");
    }

    #[test]
    fn language_envelope_uses_structured_output_tool_even_on_native_backends() {
        let (native, tool) = structured_output_modes(true, true, true);
        assert!(!native);
        assert!(tool);
    }

    #[test]
    fn user_schema_stays_native_on_capable_backends() {
        let (native, tool) = structured_output_modes(true, true, false);
        assert!(native);
        assert!(!tool);
    }

    #[test]
    fn user_schema_uses_tool_when_backend_cannot() {
        let (native, tool) = structured_output_modes(true, false, false);
        assert!(!native);
        assert!(tool);
    }

    #[test]
    fn no_schema_disables_both_modes() {
        let (native, tool) = structured_output_modes(false, true, true);
        assert!(!native);
        assert!(!tool);
    }

    #[test]
    fn extract_language_response_reads_string_field() {
        let value = serde_json::json!({
            "conversation_language": "pt-BR",
            "artifact_language": "en-US",
            "response": "olá",
        });
        assert_eq!(extract_language_response(&value).as_deref(), Some("olá"));
        assert!(extract_language_response(&serde_json::json!({"response": 1})).is_none());
        assert!(extract_language_response(&serde_json::json!({})).is_none());
    }

    #[test]
    fn rewrite_assistant_content_replaces_envelope() {
        let item = ConversationItem::assistant(r#"{"response":"hi"}"#);
        let rewritten = rewrite_assistant_content(item, "hi");
        match rewritten {
            ConversationItem::Assistant(a) => {
                assert_eq!(a.content.as_ref(), "hi");
                assert!(
                    a.replayable_messages_content().is_none(),
                    "rewritten rounds must not replay envelope wire payloads"
                );
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn rewrite_assistant_content_clears_replayable_payload() {
        let assistant = xai_grok_inference_types::AssistantItem {
            content: std::sync::Arc::<str>::from(r#"{"response":"hi"}"#),
            tool_calls: Vec::new(),
            model_id: None,
            model_fingerprint: None,
            reasoning_effort: None,
            reasoning_details: Vec::new(),
            provider_payload: None,
        }
        .with_messages_payload(
            vec![xai_grok_inference_types::messages::ContentBlock::Text {
                text: r#"{"response":"hi"}"#.into(),
                cache_control: None,
                citations: None,
            }],
            true,
        );
        assert!(assistant.replayable_messages_content().is_some());
        let rewritten = rewrite_assistant_content(ConversationItem::Assistant(assistant), "hi");
        match rewritten {
            ConversationItem::Assistant(a) => {
                assert_eq!(a.content.as_ref(), "hi");
                assert!(a.replayable_messages_content().is_none());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn rewrite_assistant_items_runs_on_tool_call_rounds() {
        let items = vec![ConversationItem::assistant(
            r#"{"conversation_language":"pt-BR","artifact_language":"en-US","response":"olá"}"#,
        )];
        let rewritten = rewrite_assistant_items(items, "olá");
        match &rewritten[0] {
            ConversationItem::Assistant(a) => assert_eq!(a.content.as_ref(), "olá"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn decode_language_envelope_text_reads_response() {
        let raw =
            r#"{"conversation_language":"pt-BR","artifact_language":"en-US","response":"oi"}"#;
        assert!(is_language_envelope_json(raw));
        assert_eq!(decode_language_envelope_text(raw).as_deref(), Some("oi"));
        assert!(!is_language_envelope_json("plain text"));
        assert!(decode_language_envelope_text("plain text").is_none());
    }

    #[test]
    fn prime_trusted_roots_include_workspace_git_root_and_home() {
        let cwd = std::path::Path::new("/repo/pkg");
        let git_root = std::path::Path::new("/repo");
        let home = std::path::Path::new("/home/dev/.grokdev");
        assert_eq!(
            super::SessionActor::prime_trusted_roots(cwd, Some(git_root), home),
            vec![
                std::path::PathBuf::from("/repo/pkg"),
                std::path::PathBuf::from("/repo"),
                std::path::PathBuf::from("/home/dev/.grokdev"),
            ]
        );
        // No git root (outside a repo): cwd + home only.
        assert_eq!(
            super::SessionActor::prime_trusted_roots(cwd, None, home),
            vec![
                std::path::PathBuf::from("/repo/pkg"),
                std::path::PathBuf::from("/home/dev/.grokdev"),
            ]
        );
    }
}
