//! `/subagents` — show or pin the fixed model for `spawn_subagent` this session.
//!
//! `/subagents` with no argument prints the effective subagent model: the
//! session pin (`/subagents model <slug>`) if set, otherwise "inherit" — the
//! child inherits the session's resolved model route.
//! `/subagents model <slug>` sets the session-scoped pin; `/subagents model
//! none` clears it so the next spawn inherits the session model again.
//!
//! The pin is SESSION-EPHEMERAL: it rides on the next prompt's session meta
//! (`subagentModel`) and is never written to disk — the shell reads it from
//! `PromptRequest._meta` when routing `spawn_subagent` calls.

use crate::app::actions::Action;
use crate::slash::command::{AppCtx, ArgItem, CommandExecCtx, CommandResult, SlashCommand};

pub struct SubagentsCommand;

impl SlashCommand for SubagentsCommand {
    fn name(&self) -> &str {
        "subagents"
    }

    fn description(&self) -> &str {
        "Show or set the fixed model used for spawn_subagent this session"
    }

    fn usage(&self) -> &str {
        "/subagents [model <slug>|none]"
    }

    fn takes_args(&self) -> bool {
        true
    }

    fn suggest_args(&self, _ctx: &AppCtx, args_query: &str) -> Option<Vec<ArgItem>> {
        let query = args_query.trim().to_ascii_lowercase();
        let items = ["model"]
            .iter()
            .filter(|c| query.is_empty() || c.starts_with(query.as_str()))
            .map(|c| ArgItem {
                display: (*c).to_string(),
                match_text: (*c).to_string(),
                insert_text: (*c).to_string(),
                description: describe_subcommand(c),
            })
            .collect();
        Some(items)
    }

    fn run(&self, ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        let trimmed = args.trim();
        if trimmed.is_empty() {
            return CommandResult::Message(status_message(
                ctx.pager_state.subagent_model_override.as_deref(),
            ));
        }
        let words: Vec<&str> = trimmed.split_whitespace().collect();
        match words.as_slice() {
            [sub_cmd, slug] if sub_cmd.eq_ignore_ascii_case("model") => {
                if slug.eq_ignore_ascii_case("none") {
                    CommandResult::Action(Action::SetSessionSubagentModel(None))
                } else {
                    CommandResult::Action(Action::SetSessionSubagentModel(Some(
                        (*slug).to_string(),
                    )))
                }
            }
            [sub_cmd] if sub_cmd.eq_ignore_ascii_case("model") => CommandResult::Error(
                "Usage: /subagents model <slug>|none. A slug is a catalog id \
                 (e.g. openrouter:z-ai/glm-5.2); none inherits the session model."
                    .to_string(),
            ),
            other => CommandResult::Error(format!(
                "Unknown subagents command {other:?}. Usage: /subagents [model <slug>|none]"
            )),
        }
    }
}

fn describe_subcommand(cmd: &str) -> String {
    match cmd {
        "model" => {
            "Pin a fixed model for spawn_subagent (pass a slug, or none to clear)".to_string()
        }
        _ => String::new(),
    }
}

/// Status line for the bare `/subagents` invocation.
///
/// `override_model` is the session pin (`Some(slug)`) or `None` when subagents
/// inherit the session model.
fn status_message(override_model: Option<&str>) -> String {
    match override_model {
        Some(slug) => {
            format!(
                "Subagent model: {slug} (session override)\n\
                 Notes: the pin rides the next prompt's meta (subagentModel) and \
                 applies to spawn_subagent this session only; /subagents model \
                 none clears it."
            )
        }
        None => {
            format!(
                "Subagent model: inherit (session model)\n\
                 Notes: spawn_subagent inherits the session model by default; \
                 /subagents model <slug> pins a fixed model this session only."
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::model_state::ModelState;
    use crate::slash::command::CommandExecCtx;

    fn ctx() -> CommandExecCtx<'static> {
        // Mirror of commands::tests::make_ctx with leaked statics; the command
        // reads only the pager snapshot, so defaults suffice.
        CommandExecCtx {
            models: leak(ModelState::default()),
            session_id: None,
            bundle_state: leak_default_bundle(),
            screen_mode: crate::app::ScreenMode::Inline,
            billing_surface_visible: true,
            pager_state: crate::settings::PagerLocalSnapshot::default(),
        }
    }

    fn leak<T: 'static>(v: T) -> &'static mut T {
        Box::leak(Box::new(v))
    }

    fn leak_default_bundle() -> &'static crate::app::bundle::BundleState {
        leak(crate::app::bundle::BundleState {
            has_cache: false,
            version: String::new(),
            personas: Vec::new(),
            roles: Vec::new(),
            agents: Vec::new(),
            skills: Vec::new(),
            persona_details: Vec::new(),
            role_details: Vec::new(),
        })
    }

    #[test]
    fn bare_invocation_reports_inherit_status() {
        let r = SubagentsCommand.run(&mut ctx(), "");
        match r {
            CommandResult::Message(m) => {
                assert!(m.contains("Subagent model: inherit"), "{m}");
                assert!(m.contains("spawn_subagent"), "{m}");
            }
            other => panic!("expected status Message, got {other:?}"),
        }
    }

    #[test]
    fn status_reports_session_override_when_set() {
        let mut c = ctx();
        c.pager_state.subagent_model_override = Some("openrouter:z-ai/glm-5.2".to_string());
        let r = SubagentsCommand.run(&mut c, "");
        match r {
            CommandResult::Message(m) => {
                assert!(m.contains("Subagent model: openrouter:z-ai/glm-5.2"), "{m}");
                assert!(m.contains("session override"), "{m}");
            }
            other => panic!("expected status Message, got {other:?}"),
        }
    }

    #[test]
    fn model_slug_dispatches_session_override() {
        let r = SubagentsCommand.run(&mut ctx(), "model openrouter:z-ai/glm-5.2");
        let CommandResult::Action(Action::SetSessionSubagentModel(model)) = r else {
            panic!("model <slug> must dispatch SetSessionSubagentModel");
        };
        assert_eq!(model.as_deref(), Some("openrouter:z-ai/glm-5.2"));
    }

    #[test]
    fn model_none_clears_the_override() {
        let r = SubagentsCommand.run(&mut ctx(), "model none");
        let CommandResult::Action(Action::SetSessionSubagentModel(model)) = r else {
            panic!("model none must dispatch SetSessionSubagentModel(None)");
        };
        assert_eq!(model, None);
    }

    #[test]
    fn whitespace_normalizes_like_other_commands() {
        let r = SubagentsCommand.run(&mut ctx(), "  model   none  ");
        let CommandResult::Action(Action::SetSessionSubagentModel(model)) = r else {
            panic!("whitespace around model none must dispatch the clear");
        };
        assert_eq!(model, None);
    }

    #[test]
    fn subcommand_matches_case_insensitively_but_slug_stays_verbatim() {
        let r = SubagentsCommand.run(&mut ctx(), "MODEL Z-AI/GlM-5.2");
        let CommandResult::Action(Action::SetSessionSubagentModel(model)) = r else {
            panic!("uppercase MODEL must dispatch");
        };
        assert_eq!(model.as_deref(), Some("Z-AI/GlM-5.2"));
    }

    #[test]
    fn bare_model_errors_with_usage() {
        let r = SubagentsCommand.run(&mut ctx(), "model");
        match r {
            CommandResult::Error(m) => assert!(m.contains("Usage:"), "{m}"),
            other => panic!("expected usage error, got {other:?}"),
        }
    }

    #[test]
    fn unknown_args_error_without_dispatch() {
        let r = SubagentsCommand.run(&mut ctx(), "bogus value");
        assert!(matches!(r, CommandResult::Error(_)));
        let r = SubagentsCommand.run(&mut ctx(), "none");
        assert!(matches!(r, CommandResult::Error(_)));
    }

    #[test]
    fn suggestions_narrow_by_prefix() {
        let models = ModelState::default();
        let app = AppCtx {
            models: &models,
            cwd: std::path::Path::new("/tmp"),
            has_session_announcements: false,
            billing_surface_visible: true,
            workflows_available: false,
            screen_mode: crate::app::ScreenMode::Inline,
        };
        let items = SubagentsCommand
            .suggest_args(&app, "m")
            .expect("suggestions");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].display, "model");
        assert_eq!(items[0].insert_text, "model");
        let all = SubagentsCommand
            .suggest_args(&app, "")
            .expect("suggestions");
        assert_eq!(all.len(), 1);
        assert!(
            SubagentsCommand
                .suggest_args(&app, "x")
                .is_some_and(|i| i.is_empty())
        );
    }
}
