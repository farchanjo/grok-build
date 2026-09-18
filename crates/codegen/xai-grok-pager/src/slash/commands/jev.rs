//! `/jev` -- control Jev-guided compaction pruning.
//!
//! `on` / `off` dispatch the same typed action as the settings row, so the
//! value persists to `[compaction.jev].enabled` and the running session picks
//! it up through the existing `[compaction]` reload fan-out (no restart).
//! Bare `/jev` and `/jev status` report the effective state.

use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};

/// Jev model used when `[compaction.jev].model` is unset. Mirrors the shell
/// client default; the decisions endpoint is not a chat surface, so this is a
/// plain string rather than a catalog entry.
const DEFAULT_MODEL: &str = "~typesafe/jev-latest";

/// Decisions endpoint used when `[compaction.jev].endpoint` is unset.
const DEFAULT_ENDPOINT: &str = "https://openrouter.ai/api/alpha/decisions";

/// Control Jev-guided compaction pruning.
pub struct JevCommand;

impl SlashCommand for JevCommand {
    fn name(&self) -> &str {
        "jev"
    }

    fn description(&self) -> &str {
        "Toggle Jev-guided compaction pruning"
    }

    fn usage(&self) -> &str {
        "/jev [on|off|status]"
    }

    fn arg_placeholder(&self) -> Option<&str> {
        Some("on/off")
    }

    fn run(&self, ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        match args.trim().to_ascii_lowercase().as_str() {
            "on" | "enable" | "enabled" => {
                CommandResult::Action(Action::SetCompactionJevEnabled(true))
            }
            "off" | "disable" | "disabled" => {
                CommandResult::Action(Action::SetCompactionJevEnabled(false))
            }
            "" | "status" => CommandResult::Message(jev_status(ctx)),
            other => CommandResult::Error(format!(
                "unknown argument `{other}`; usage: {}",
                self.usage()
            )),
        }
    }
}

/// Effective model and endpoint, from `[compaction.jev]` when set.
fn jev_model_and_endpoint() -> (String, String) {
    let jev = xai_grok_shell::config::load_effective_config()
        .ok()
        .and_then(|root| root.get("compaction").cloned())
        .and_then(|value| {
            value
                .try_into::<xai_grok_shell::agent::config::CompactionConfig>()
                .ok()
        })
        .and_then(|config| config.jev);
    match jev {
        Some(jev) => (
            jev.model.unwrap_or_else(|| DEFAULT_MODEL.to_owned()),
            jev.endpoint.unwrap_or_else(|| DEFAULT_ENDPOINT.to_owned()),
        ),
        None => (DEFAULT_MODEL.to_owned(), DEFAULT_ENDPOINT.to_owned()),
    }
}

fn jev_status(ctx: &CommandExecCtx) -> String {
    let (model, endpoint) = jev_model_and_endpoint();
    format!(
        "Jev-guided pruning: {}\nmodel: {model}\nendpoint: {endpoint}\n\
         Prunes stale tool calls and tool results from the summarizer view only; \
         user and assistant text is untouched.",
        if ctx.pager_state.compaction_jev_enabled {
            "on"
        } else {
            "off"
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::model_state::ModelState;
    use crate::app::bundle::BundleState;
    use crate::settings::PagerLocalSnapshot;

    fn make_ctx<'a>(
        models: &'a ModelState,
        bundle: &'a BundleState,
        jev_enabled: bool,
    ) -> CommandExecCtx<'a> {
        CommandExecCtx {
            models,
            session_id: None,
            bundle_state: bundle,
            screen_mode: crate::app::ScreenMode::Inline,
            billing_surface_visible: true,
            pager_state: PagerLocalSnapshot {
                compaction_jev_enabled: jev_enabled,
                ..PagerLocalSnapshot::default()
            },
        }
    }

    #[test]
    fn on_dispatches_typed_setter_with_true() {
        let cmd = JevCommand;
        let models = ModelState::default();
        let bundle = BundleState::default();
        let mut ctx = make_ctx(&models, &bundle, false);
        match cmd.run(&mut ctx, "on") {
            CommandResult::Action(Action::SetCompactionJevEnabled(b)) => assert!(b),
            other => panic!("expected Action::SetCompactionJevEnabled(true), got {other:?}"),
        }
    }

    #[test]
    fn off_dispatches_typed_setter_with_false() {
        let cmd = JevCommand;
        let models = ModelState::default();
        let bundle = BundleState::default();
        let mut ctx = make_ctx(&models, &bundle, true);
        match cmd.run(&mut ctx, "off") {
            CommandResult::Action(Action::SetCompactionJevEnabled(b)) => assert!(!b),
            other => panic!("expected Action::SetCompactionJevEnabled(false), got {other:?}"),
        }
    }

    #[test]
    fn bare_and_status_report_effective_state() {
        let cmd = JevCommand;
        let models = ModelState::default();
        let bundle = BundleState::default();
        for args in ["", "status"] {
            let mut ctx = make_ctx(&models, &bundle, true);
            match cmd.run(&mut ctx, args) {
                CommandResult::Message(msg) => {
                    assert!(msg.contains("Jev-guided pruning: on"), "got {msg}");
                }
                other => panic!("expected status message for `{args}`, got {other:?}"),
            }
        }
    }

    #[test]
    fn unknown_argument_is_an_error() {
        let cmd = JevCommand;
        let models = ModelState::default();
        let bundle = BundleState::default();
        let mut ctx = make_ctx(&models, &bundle, false);
        assert!(matches!(
            cmd.run(&mut ctx, "maybe"),
            CommandResult::Error(_)
        ));
    }

    #[test]
    fn registered_in_builtin_commands() {
        let reg = crate::slash::registry::CommandRegistry::new(
            crate::slash::commands::builtin_commands(),
        );
        let resolved = reg.get("jev").expect("/jev must be registered");
        assert_eq!(resolved.name(), "jev");
    }
}
