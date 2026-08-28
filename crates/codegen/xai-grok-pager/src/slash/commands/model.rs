//! `/model` (alias `/m`) — switch model + (optionally) reasoning effort.
//! Chained autocomplete: pick a reasoning-supported model → trailing space
//! re-opens the dropdown into a `low|medium|high|xhigh` sub-menu.

use agent_client_protocol as acp;
use xai_grok_shell::inference::types::supports_reasoning_effort_meta;

use crate::acp::model_state::ModelState;
use crate::app::actions::Action;
use crate::slash::command::{AppCtx, ArgItem, CommandExecCtx, CommandResult, SlashCommand};
use crate::slash::commands::effort_levels::build_effort_arg_items;

/// Switch the active model (and optionally its reasoning effort).
pub struct ModelCommand;

impl SlashCommand for ModelCommand {
    fn name(&self) -> &str {
        "model"
    }

    fn aliases(&self) -> &[&str] {
        &["m"]
    }

    fn description(&self) -> &str {
        "Switch the active model"
    }

    fn session_scoped(&self) -> bool {
        true
    }

    fn offered_when_session_less(&self) -> bool {
        // The dashboard offers `/model` to pick the model for the next
        // spawned agent (intercepted in `dispatch_dashboard_dispatch_slash`).
        true
    }

    fn usage(&self) -> &str {
        "/model <name> [effort]"
    }

    fn takes_args(&self) -> bool {
        true
    }

    fn args_required(&self) -> bool {
        true
    }

    fn arg_placeholder(&self) -> Option<&str> {
        Some("<model> [effort]")
    }

    fn suggest_args(&self, ctx: &AppCtx, args_query: &str) -> Option<Vec<ArgItem>> {
        if ctx.models.is_empty() {
            return None;
        }

        // Effort phase if input is "<reasoning-model> ", else model phase.
        if let Some(model_id) = detect_effort_phase(ctx.models, args_query) {
            return Some(build_effort_items(ctx.models, &model_id));
        }
        Some(build_model_items(ctx.models))
    }

    fn run(&self, ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        let trimmed = args.trim();
        if trimmed.is_empty() {
            return CommandResult::Error("Usage: /model <name> [effort]".into());
        }

        // Prefer an exact full-string catalog match first. Model display names
        // often contain spaces ("Grok 4.5"); if we split on the last token
        // first, a shorter catalog entry ("Grok") would steal the prefix and
        // treat "4.5" as an effort level.
        //
        // Ambiguous labels (sibling OpenAI/OpenRouter accounts sharing a
        // display name or upstream slug) fail closed with the candidate list
        // — never silently pick one account.
        match ctx.models.resolve_by_name_or_id_detailed(trimmed) {
            crate::acp::model_state::ModelResolveResult::Resolved(id) => {
                return CommandResult::Action(Action::SetDefaultModel(id));
            }
            crate::acp::model_state::ModelResolveResult::Ambiguous { candidates, .. } => {
                return CommandResult::Error(ambiguous_model_message(trimmed, &candidates));
            }
            crate::acp::model_state::ModelResolveResult::Missing { .. } => {}
        }

        // Trailing effort token + reasoning model → session-scoped switch
        // (not persisted as default). Resolve via the shared gate so a rejected
        // level (e.g. `none` on grok-4.5) surfaces the effort error with the
        // model's offered ids — not "Unknown model: … none".
        if let Some((prefix, token)) = split_trailing_token(trimmed) {
            match ctx.models.resolve_by_name_or_id_detailed(prefix) {
                crate::acp::model_state::ModelResolveResult::Resolved(id)
                    if ctx
                        .models
                        .available
                        .get(&id)
                        .map(supports_reasoning_effort)
                        .unwrap_or(false) =>
                {
                    return match ctx.models.resolve_effort_for_model(&id, token) {
                        Ok(effort) => CommandResult::Action(Action::SwitchModel {
                            model_id: id,
                            effort: Some(effort),
                        }),
                        Err(err) => CommandResult::Error(err.message()),
                    };
                }
                crate::acp::model_state::ModelResolveResult::Ambiguous { candidates, .. } => {
                    return CommandResult::Error(ambiguous_model_message(prefix, &candidates));
                }
                _ => {}
            }
        }

        CommandResult::Error(format!("Unknown model: {trimmed}"))
    }
}

fn ambiguous_model_message(query: &str, candidates: &[acp::ModelId]) -> String {
    let list = candidates
        .iter()
        .map(|id| id.0.as_ref())
        .collect::<Vec<_>>()
        .join(", ");
    format!("Ambiguous model '{query}': matches [{list}]. Use the exact provider-qualified id.")
}

fn supports_reasoning_effort(info: &acp::ModelInfo) -> bool {
    supports_reasoning_effort_meta(info.meta.as_ref())
}

/// Split `args` into `(prefix, last_token)` on the final whitespace run.
/// Returns `None` when there is no interior whitespace to split on. The token is
/// resolved to an effort against the picked model's options by the caller.
fn split_trailing_token(args: &str) -> Option<(&str, &str)> {
    let (prefix, last) = args.rsplit_once(char::is_whitespace)?;
    let prefix = prefix.trim_end();
    if prefix.is_empty() || last.is_empty() {
        return None;
    }
    Some((prefix, last))
}

/// True when `/model` should chain into the effort sub-menu.
///
/// A supports-flag without a menu (OpenRouter `Unknown`) must not open an
/// empty effort dropdown. Exact / unrestricted / legacy-fallback menus do.
fn offers_effort_menu(models: &ModelState, id: &acp::ModelId, info: &acp::ModelInfo) -> bool {
    supports_reasoning_effort(info) && !models.reasoning_effort_options_for(id).is_empty()
}

/// Returns the matched model id when `args_query` is `"<reasoning-model> ..."`.
/// Longest-name-first to disambiguate names that share a prefix.
fn detect_effort_phase(models: &ModelState, args_query: &str) -> Option<acp::ModelId> {
    let mut candidates: Vec<(&acp::ModelId, String)> = models
        .available
        .iter()
        .filter(|(id, info)| offers_effort_menu(models, id, info))
        .flat_map(|(id, info)| {
            let mut labels = vec![info.name.clone(), id.0.as_ref().to_string()];
            labels.sort();
            labels.dedup();
            labels.into_iter().map(move |label| (id, label))
        })
        .collect();
    candidates.sort_by_key(|(_, name)| std::cmp::Reverse(name.len()));

    for (id, name) in candidates {
        if args_query.len() > name.len()
            && args_query.is_char_boundary(name.len())
            && args_query[..name.len()].eq_ignore_ascii_case(&name)
            && args_query[name.len()..].starts_with(char::is_whitespace)
        {
            return Some(id.clone());
        }
    }
    None
}

/// Catalog title shown next to a namespaced id (`zdr:…`, `dr:…`).
fn complete_model_title(id: &acp::ModelId, info: &acp::ModelInfo) -> Option<String> {
    info.description
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .filter(|title| !title.eq_ignore_ascii_case(info.name.as_str()))
        .filter(|title| !title.eq_ignore_ascii_case(id.0.as_ref()))
        .map(str::to_owned)
}

/// One row per logical model. Reasoning models get a trailing space in
/// `insert_text` so the prompt widget chains into the effort sub-menu.
fn build_model_items(models: &ModelState) -> Vec<ArgItem> {
    let current_id = models.current.as_ref();
    let mut items: Vec<ArgItem> = Vec::with_capacity(models.available.len());
    for (id, info) in &models.available {
        let is_current = current_id == Some(id);
        let chains_to_effort = offers_effort_menu(models, id, info);

        // Namespaced catalog ids (`zdr:…`, `dr:…`) insert the canonical id so
        // `/model` keeps kind. The complete OpenRouter title sits in
        // description and is also folded into the visible label.
        let label = info.name.clone();
        let title = complete_model_title(id, info);
        let display_label = match title.as_deref() {
            Some(title) => format!("{label} · {title}"),
            None => label.clone(),
        };
        let display = if is_current {
            format!("{display_label} (current)")
        } else {
            display_label
        };

        // Trailing space on models that actually offer an effort menu.
        let insert_text = if chains_to_effort {
            format!("{label} ")
        } else {
            label
        };

        items.push(ArgItem {
            display,
            match_text: model_match_text(id, info),
            insert_text,
            description: title.unwrap_or_else(|| {
                info.description
                    .clone()
                    .filter(|d| !d.trim().is_empty())
                    .unwrap_or_else(|| id.0.as_ref().to_string())
            }),
        });
    }
    items
}

/// Search haystack for the `/model` dropdown filter.
///
/// Display names are often shortened in `config.toml` (`name = "DeepSeek V4
/// Flash"` for `openrouter:deepseek/deepseek-v4-flash-0731`). Include the
/// canonical selection id, the upstream slug, and the account label so
/// typing `0731`, `deepseek-v4-flash-0731`, or `Work` still finds the row.
fn model_match_text(id: &acp::ModelId, info: &acp::ModelInfo) -> String {
    let mut parts = vec![info.name.as_str(), id.0.as_ref()];
    let meta = info.meta.as_ref();
    for key in ["upstreamModelId", "accountLabel", "providerInstanceId"] {
        if let Some(value) = meta.and_then(|m| m.get(key)).and_then(|v| v.as_str())
            && !value.is_empty()
            && !parts.iter().any(|part| part.eq_ignore_ascii_case(value))
        {
            parts.push(value);
        }
    }
    parts.join(" ")
}

/// One row per effort level for the `/model` chained effort phase.
/// `insert_text` is `"ModelName high"` so selecting a row completes both tokens.
fn build_effort_items(models: &ModelState, model_id: &acp::ModelId) -> Vec<ArgItem> {
    let info = match models.available.get(model_id) {
        Some(info) => info,
        None => return Vec::new(),
    };
    let model_name = info.name.clone();
    let is_current_model = models.current.as_ref() == Some(model_id);
    let options = models.reasoning_effort_options_for(model_id);
    build_effort_arg_items(
        &options,
        models.reasoning_effort,
        is_current_model,
        |option| format!("{model_name} {}", option.id),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use xai_grok_shell::inference::types::ReasoningEffort;

    fn model_with_reasoning(id: &str, name: &str) -> (acp::ModelId, acp::ModelInfo) {
        let id = acp::ModelId::new(Arc::from(id));
        let mut meta = serde_json::Map::new();
        meta.insert(
            "supportsReasoningEffort".into(),
            serde_json::Value::Bool(true),
        );
        let info = acp::ModelInfo::new(id.clone(), name.to_string())
            .meta(serde_json::Value::Object(meta).as_object().cloned());
        (id, info)
    }

    fn plain_model(id: &str, name: &str) -> (acp::ModelId, acp::ModelInfo) {
        let id = acp::ModelId::new(Arc::from(id));
        let info = acp::ModelInfo::new(id.clone(), name.to_string());
        (id, info)
    }

    static EMPTY_BUNDLE: crate::app::bundle::BundleState = crate::app::bundle::BundleState {
        has_cache: false,
        version: String::new(),
        personas: Vec::new(),
        roles: Vec::new(),
        agents: Vec::new(),
        skills: Vec::new(),
        persona_details: Vec::new(),
        role_details: Vec::new(),
    };

    fn dummy_exec_ctx(models: &ModelState) -> CommandExecCtx<'_> {
        CommandExecCtx {
            models,
            session_id: None,
            bundle_state: &EMPTY_BUNDLE,
            screen_mode: crate::app::ScreenMode::Inline,
            billing_surface_visible: true,
            pager_state: crate::settings::PagerLocalSnapshot {
                multiline_mode: false,
                yolo_mode: false,
                ..crate::settings::PagerLocalSnapshot::default()
            },
        }
    }

    #[test]
    fn split_trailing_token_splits_on_final_whitespace() {
        assert_eq!(
            split_trailing_token("Reasoning X high"),
            Some(("Reasoning X", "high"))
        );
        assert_eq!(
            split_trailing_token("reasoning-x  xhigh"),
            Some(("reasoning-x", "xhigh"))
        );
        // No interior whitespace → nothing to split off.
        assert!(split_trailing_token("reasoning-x-pro").is_none());
    }

    #[test]
    fn empty_query_returns_one_row_per_logical_model() {
        let mut state = ModelState::default();
        let (rid, rinfo) = model_with_reasoning("reasoning-x", "Reasoning X");
        let (pid, pinfo) = plain_model("grok-4.5", "Grok 4.5");
        state.available.insert(rid, rinfo);
        state.available.insert(pid, pinfo);

        let cmd = ModelCommand;
        let ctx = AppCtx {
            models: &state,
            cwd: std::path::Path::new("."),
            has_session_announcements: false,
            billing_surface_visible: true,
            workflows_available: true,
            screen_mode: crate::app::ScreenMode::Fullscreen,
        };
        let items = cmd.suggest_args(&ctx, "").unwrap();
        assert_eq!(items.len(), 2, "model phase: one row per logical model");

        // Reasoning model has trailing space in insert_text -- this is the
        // signal the prompt widget reads to keep the dropdown open after
        // Enter so the effort sub-menu can render.
        let reasoning = items
            .iter()
            .find(|i| i.insert_text == "Reasoning X ")
            .unwrap();
        assert!(
            reasoning.match_text.contains("Reasoning X"),
            "match_text should still contain the display name: {}",
            reasoning.match_text
        );
        assert_eq!(reasoning.insert_text, "Reasoning X ");

        // Plain model has no trailing space -- Enter commits immediately.
        let plain = items.iter().find(|i| i.insert_text == "Grok 4.5").unwrap();
        assert_eq!(plain.insert_text, "Grok 4.5");
        assert!(plain.match_text.contains("Grok 4.5"));
        assert!(plain.match_text.contains("grok-4.5"));
    }

    #[test]
    fn match_text_includes_canonical_id_when_display_name_drops_the_slug() {
        let mut state = ModelState::default();
        // Config override often shortens the catalog label, stripping the
        // date suffix the user actually searches for.
        let (id, info) = plain_model(
            "openrouter:deepseek/deepseek-v4-flash-0731",
            "DeepSeek V4 Flash",
        );
        state.available.insert(id, info);

        let items = build_model_items(&state);
        assert_eq!(items.len(), 1);
        let haystack = items[0].match_text.to_lowercase();
        assert!(
            haystack.contains("0731"),
            "typing 0731 must match: {}",
            items[0].match_text
        );
        assert!(haystack.contains("deepseek-v4-flash-0731"));
        assert!(haystack.contains("deepseek v4 flash"));
        assert_eq!(
            items[0].description,
            "openrouter:deepseek/deepseek-v4-flash-0731"
        );
    }

    #[test]
    fn match_text_includes_canonical_id_and_account_label() {
        let mut state = ModelState::default();
        let id = acp::ModelId::new(Arc::from("openrouter-work:z-ai/glm-5.3-flash"));
        let info = acp::ModelInfo::new(id.clone(), "GLM 5.3 Flash · Work · ZDR".to_string()).meta(
            serde_json::json!({
                "upstreamModelId": "z-ai/glm-5.3-flash",
                "accountLabel": "Work",
                "providerInstanceId": "openrouter-work",
                "canonicalSelectionId": "openrouter-work:z-ai/glm-5.3-flash",
            })
            .as_object()
            .cloned(),
        );
        state.available.insert(id, info);

        let items = build_model_items(&state);
        assert_eq!(items.len(), 1);
        let haystack = items[0].match_text.to_lowercase();
        assert!(
            haystack.contains("openrouter-work:z-ai/glm-5.3-flash"),
            "canonical id must be in haystack: {}",
            items[0].match_text
        );
        assert!(
            haystack.contains("work"),
            "account label must be in haystack: {}",
            items[0].match_text
        );
        assert_eq!(
            items[0].insert_text, "GLM 5.3 Flash · Work · ZDR",
            "insert_text follows ACP display name (canonical id once the catalog qualifies it)"
        );
    }

    #[test]
    fn namespaced_catalog_id_is_the_insert_text() {
        let mut state = ModelState::default();
        let id = acp::ModelId::new(Arc::from("zdr:z-ai/glm-5.3-flash"));
        let info = acp::ModelInfo::new(id.clone(), "zdr:z-ai/glm-5.3-flash".to_string())
            .description(Some("Z.ai: GLM 5.3 Flash".into()));
        state.available.insert(id, info);

        let items = build_model_items(&state);
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0].display,
            "zdr:z-ai/glm-5.3-flash · Z.ai: GLM 5.3 Flash"
        );
        assert_eq!(items[0].insert_text, "zdr:z-ai/glm-5.3-flash");
        assert_eq!(items[0].description, "Z.ai: GLM 5.3 Flash");
        assert!(items[0].match_text.contains("zdr:z-ai/glm-5.3-flash"));
    }

    fn model_with_exact_efforts(
        id: &str,
        name: &str,
        title: &str,
        efforts: &[&str],
    ) -> (acp::ModelId, acp::ModelInfo) {
        let id = acp::ModelId::new(Arc::from(id));
        let mut meta = serde_json::Map::new();
        meta.insert(
            "supportsReasoningEffort".into(),
            serde_json::Value::Bool(true),
        );
        meta.insert(
            "reasoningEffortSelection".into(),
            serde_json::Value::String("exact".into()),
        );
        meta.insert("reasoningEfforts".into(), serde_json::json!(efforts));
        let info = acp::ModelInfo::new(id.clone(), name.to_string())
            .description(Some(title.to_string()))
            .meta(serde_json::Value::Object(meta).as_object().cloned());
        (id, info)
    }

    #[test]
    fn namespaced_reasoning_model_lists_complete_name_and_enters_effort_phase() {
        let mut state = ModelState::default();
        let (id, info) = model_with_exact_efforts(
            "zdr:z-ai/glm-5.3-flash",
            "zdr:z-ai/glm-5.3-flash",
            "Z.ai: GLM 5.3 Flash",
            &["max", "high", "low"],
        );
        state.available.insert(id, info);

        let cmd = ModelCommand;
        let ctx = AppCtx {
            models: &state,
            cwd: std::path::Path::new("."),
            has_session_announcements: false,
            billing_surface_visible: true,
            workflows_available: true,
            screen_mode: crate::app::ScreenMode::Fullscreen,
        };
        let items = cmd.suggest_args(&ctx, "").unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0].display,
            "zdr:z-ai/glm-5.3-flash · Z.ai: GLM 5.3 Flash"
        );
        assert_eq!(items[0].insert_text, "zdr:z-ai/glm-5.3-flash ");
        assert_eq!(items[0].description, "Z.ai: GLM 5.3 Flash");

        let efforts = cmd.suggest_args(&ctx, "zdr:z-ai/glm-5.3-flash ").unwrap();
        assert_eq!(efforts.len(), 3);
        assert_eq!(efforts[0].insert_text, "zdr:z-ai/glm-5.3-flash max");
        assert_eq!(efforts[1].insert_text, "zdr:z-ai/glm-5.3-flash high");
        assert_eq!(efforts[2].insert_text, "zdr:z-ai/glm-5.3-flash low");
        assert_eq!(efforts[0].display, "Max");
    }

    #[test]
    fn trailing_space_after_reasoning_model_enters_effort_phase() {
        let mut state = ModelState::default();
        let (id, info) = model_with_reasoning("reasoning-x", "Reasoning X");
        state.available.insert(id, info);

        let cmd = ModelCommand;
        let ctx = AppCtx {
            models: &state,
            cwd: std::path::Path::new("."),
            has_session_announcements: false,
            billing_surface_visible: true,
            workflows_available: true,
            screen_mode: crate::app::ScreenMode::Fullscreen,
        };
        // Args query has a trailing space -> effort phase. Items come out
        // ordered xhigh -> low (strongest first) per EFFORT_LEVELS.
        let items = cmd.suggest_args(&ctx, "Reasoning X ").unwrap();
        assert_eq!(items.len(), 4);
        assert_eq!(items[0].insert_text, "Reasoning X xhigh");
        assert_eq!(items[1].insert_text, "Reasoning X high");
        assert_eq!(items[2].insert_text, "Reasoning X medium");
        assert_eq!(items[3].insert_text, "Reasoning X low");
        // Display is just the level so the user sees a clean column.
        assert_eq!(items[0].display, "xhigh");
        // match_text carries the sort-key prefix that forces the matcher's
        // alphabetical tiebreak to render rows in EFFORT_LEVELS order.
        assert!(items[0].match_text.starts_with("a "));
        assert!(items[3].match_text.starts_with("d "));
    }

    #[test]
    fn partial_effort_query_still_in_effort_phase() {
        let mut state = ModelState::default();
        let (id, info) = model_with_reasoning("reasoning-x", "Reasoning X");
        state.available.insert(id, info);

        let cmd = ModelCommand;
        let ctx = AppCtx {
            models: &state,
            cwd: std::path::Path::new("."),
            has_session_announcements: false,
            billing_surface_visible: true,
            workflows_available: true,
            screen_mode: crate::app::ScreenMode::Fullscreen,
        };
        // Still in effort phase; matcher upstream narrows to high / xhigh.
        let items = cmd.suggest_args(&ctx, "Reasoning X h").unwrap();
        assert_eq!(items.len(), 4);
    }

    #[test]
    fn partial_model_query_stays_in_model_phase() {
        let mut state = ModelState::default();
        let (id, info) = model_with_reasoning("reasoning-x", "Reasoning X");
        state.available.insert(id, info);

        let cmd = ModelCommand;
        let ctx = AppCtx {
            models: &state,
            cwd: std::path::Path::new("."),
            has_session_announcements: false,
            billing_surface_visible: true,
            workflows_available: true,
            screen_mode: crate::app::ScreenMode::Fullscreen,
        };
        // No trailing space, user is still typing the model name.
        let items = cmd.suggest_args(&ctx, "Reason").unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].insert_text, "Reasoning X ");
    }

    #[test]
    fn run_parses_model_plus_effort_when_supported() {
        let mut state = ModelState::default();
        let (id, info) = model_with_reasoning("reasoning-x", "Reasoning X");
        state.available.insert(id, info);
        let mut ctx = dummy_exec_ctx(&state);
        let result = ModelCommand.run(&mut ctx, "Reasoning X xhigh");
        match result {
            CommandResult::Action(Action::SwitchModel { model_id, effort }) => {
                assert_eq!(model_id.0.as_ref(), "reasoning-x");
                assert_eq!(effort, Some(ReasoningEffort::Xhigh));
            }
            other => panic!("expected SwitchModel with effort, got {other:?}"),
        }
    }

    #[test]
    fn run_rejects_unoffered_effort_with_effort_error_not_unknown_model() {
        // Regression: previously `resolve_effort_token_for` returned None and
        // the handler fell through to `Unknown model: Reasoning X none`.
        let mut state = ModelState::default();
        let (id, info) = model_with_reasoning("reasoning-x", "Reasoning X");
        state.available.insert(id, info);
        let mut ctx = dummy_exec_ctx(&state);
        let result = ModelCommand.run(&mut ctx, "Reasoning X none");
        match result {
            CommandResult::Error(msg) => {
                assert!(
                    msg.contains("unknown effort level 'none'"),
                    "expected effort error, got {msg}"
                );
                assert!(
                    msg.contains("use one of:"),
                    "expected offered levels in message, got {msg}"
                );
                assert!(
                    !msg.to_lowercase().contains("unknown model"),
                    "must not misreport as unknown model: {msg}"
                );
                let offered = msg.split_once("; ").map(|(_, r)| r).unwrap_or("");
                assert!(
                    !offered.contains("none"),
                    "must not list none as offered: {msg}"
                );
            }
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn run_prefers_full_multi_word_model_name_over_prefix_plus_effort() {
        // Catalog has both "Grok" (reasoning) and "Grok 4.5". `/model Grok 4.5`
        // must select the full name, not treat "4.5" as an effort on "Grok".
        let mut state = ModelState::default();
        let (short_id, short_info) = model_with_reasoning("grok", "Grok");
        let (long_id, long_info) = model_with_reasoning("grok-4.5", "Grok 4.5");
        state.available.insert(short_id, short_info);
        state.available.insert(long_id.clone(), long_info);
        let mut ctx = dummy_exec_ctx(&state);
        let result = ModelCommand.run(&mut ctx, "Grok 4.5");
        match result {
            CommandResult::Action(Action::SetDefaultModel(resolved_id)) => {
                assert_eq!(resolved_id, long_id);
            }
            other => panic!("expected SetDefaultModel(Grok 4.5), got {other:?}"),
        }
    }

    #[test]
    fn run_rejects_effort_for_non_reasoning_model() {
        let mut state = ModelState::default();
        let (id, info) = plain_model("grok-4.5", "Grok 4.5");
        state.available.insert(id, info);
        let mut ctx = dummy_exec_ctx(&state);
        let result = ModelCommand.run(&mut ctx, "Grok 4.5 high");
        // Falls through to "is the whole string a model name?" — which
        // it isn't, so we get an Unknown error.
        assert!(matches!(result, CommandResult::Error(_)));
    }

    /// The bare `/model <name>` form dispatches
    /// `Action::SetDefaultModel(<ModelId>)` instead of the legacy
    /// `Action::SwitchModel { effort: None }`. The dispatcher routes
    /// the typed setter through both `Effect::SwitchModel`
    /// (session-level mutation) AND `Effect::PersistSetting`
    /// (next-session default).
    ///
    /// The payload is the typed `acp::ModelId` (resolved at the slash
    /// boundary), not a String.
    #[test]
    fn run_bare_model_name_dispatches_set_default_model() {
        let mut state = ModelState::default();
        let (id, info) = plain_model("grok-4.5", "Grok 4.5");
        state.available.insert(id.clone(), info);
        let mut ctx = dummy_exec_ctx(&state);
        let result = ModelCommand.run(&mut ctx, "Grok 4.5");
        match result {
            CommandResult::Action(Action::SetDefaultModel(resolved_id)) => {
                assert_eq!(resolved_id, id);
            }
            other => panic!("expected Action::SetDefaultModel(<id>), got {other:?}"),
        }
    }

    /// Case-insensitive matching against the catalog: `/model grok 4.5`
    /// resolves to the same `ModelId` as `/model Grok 4.5`.
    #[test]
    fn run_set_default_model_resolves_case_insensitively() {
        let mut state = ModelState::default();
        let (id, info) = plain_model("grok-4.5", "Grok 4.5");
        state.available.insert(id.clone(), info);
        let mut ctx = dummy_exec_ctx(&state);
        let result = ModelCommand.run(&mut ctx, "grok 4.5");
        match result {
            CommandResult::Action(Action::SetDefaultModel(resolved_id)) => {
                assert_eq!(resolved_id, id);
            }
            other => panic!("expected Action::SetDefaultModel(<id>), got {other:?}"),
        }
    }

    #[test]
    fn run_ambiguous_openrouter_short_name_fails_closed_with_both_canonical_ids() {
        let mut state = ModelState::default();
        let home = acp::ModelId::new(Arc::from("openrouter:z-ai/glm-5.3-flash"));
        let work = acp::ModelId::new(Arc::from("openrouter-work:z-ai/glm-5.3-flash"));
        state.available.insert(
            home.clone(),
            acp::ModelInfo::new(home, "GLM 5.3 Flash · Home".to_string()).meta(
                serde_json::json!({
                    "upstreamModelId": "z-ai/glm-5.3-flash",
                    "accountLabel": "Home",
                    "providerInstanceId": "openrouter",
                })
                .as_object()
                .cloned(),
            ),
        );
        state.available.insert(
            work.clone(),
            acp::ModelInfo::new(work, "GLM 5.3 Flash · Work · ZDR".to_string()).meta(
                serde_json::json!({
                    "upstreamModelId": "z-ai/glm-5.3-flash",
                    "accountLabel": "Work",
                    "providerInstanceId": "openrouter-work",
                })
                .as_object()
                .cloned(),
            ),
        );

        let mut ctx = dummy_exec_ctx(&state);
        let result = ModelCommand.run(&mut ctx, "z-ai/glm-5.3-flash");
        match result {
            CommandResult::Error(msg) => {
                assert!(
                    msg.contains("Ambiguous model"),
                    "short name must fail closed: {msg}"
                );
                assert!(msg.contains("openrouter:z-ai/glm-5.3-flash"));
                assert!(msg.contains("openrouter-work:z-ai/glm-5.3-flash"));
            }
            other => panic!("expected Ambiguous error, got {other:?}"),
        }

        let result = ModelCommand.run(&mut ctx, "openrouter-work:z-ai/glm-5.3-flash");
        match result {
            CommandResult::Action(Action::SetDefaultModel(id)) => {
                assert_eq!(id.0.as_ref(), "openrouter-work:z-ai/glm-5.3-flash");
            }
            other => panic!("canonical id must resolve uniquely, got {other:?}"),
        }
    }
}
