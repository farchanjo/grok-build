//! `/tersify` — switch output-compression level for this session.
//!
//! `/tersify lite|full|ultra` sets the session-scoped level: it wins over the
//! persisted `[hints] tersify_level` for THIS session only and is never
//! written to disk (the shell reads it from session meta at prompt spawn).
//! `/tersify off` clears the override for the session — the next turn uses
//! whatever the persisted scope says, which under `main_only` means the style
//! block is present and under `off` it is gone.
//!
//! `/tersify` with no argument prints the usage line.

use crate::app::actions::Action;
use crate::slash::command::{AppCtx, CommandExecCtx, CommandResult, SlashCommand};

pub struct TersifyCommand;

const LEVELS: &[&str] = &["lite", "full", "ultra", "off"];

impl SlashCommand for TersifyCommand {
    fn name(&self) -> &str {
        "tersify"
    }

    fn description(&self) -> &str {
        "Switch tersify level for this session (lite/full/ultra/off)"
    }

    fn usage(&self) -> &str {
        "/tersify [lite|full|ultra|off]"
    }

    fn takes_args(&self) -> bool {
        true
    }

    fn suggest_args(
        &self,
        _ctx: &AppCtx,
        args_query: &str,
    ) -> Option<Vec<crate::slash::command::ArgItem>> {
        let query = args_query.trim().to_ascii_lowercase();
        let items = LEVELS
            .iter()
            .filter(|l| query.is_empty() || l.starts_with(query.as_str()))
            .map(|l| crate::slash::command::ArgItem {
                display: (*l).to_string(),
                match_text: (*l).to_string(),
                insert_text: (*l).to_string(),
                description: describe_level(l),
            })
            .collect();
        Some(items)
    }

    fn run(&self, _ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        let arg = args.trim().to_ascii_lowercase();
        match arg.as_str() {
            "" => CommandResult::Error(
                "Usage: /tersify lite|full|ultra|off. Current level comes from \
                 [hints] tersify_level unless overridden this session."
                    .to_string(),
            ),
            "lite" | "full" | "ultra" | "off" => {
                CommandResult::Action(Action::SetSessionTersifyLevel(arg))
            }
            other => CommandResult::Error(format!(
                "Unknown level {other:?}. Use lite, full, ultra, or off."
            )),
        }
    }
}

fn describe_level(level: &str) -> String {
    match level {
        "lite" => "No filler/hedging; full sentences stay".to_string(),
        "full" => "Drop articles and filler; fragments OK (default)".to_string(),
        "ultra" => "One word when one word enough".to_string(),
        "off" => "Clear this session's override; fall back to config".to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::model_state::ModelState;
    use crate::slash::command::CommandExecCtx;

    fn ctx() -> CommandExecCtx<'static> {
        // Mirror of commands::tests::make_ctx with leaked statics; the command
        // reads nothing from the ctx, so defaults suffice.
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
    fn unknown_level_errors_without_dispatch() {
        let r = TersifyCommand.run(&mut ctx(), "aggressive");
        assert!(matches!(r, CommandResult::Error(_)));
    }

    #[test]
    fn empty_arg_errors_with_usage() {
        let r = TersifyCommand.run(&mut ctx(), "");
        match r {
            CommandResult::Error(m) => assert!(m.contains("Usage:"), "{m}"),
            _ => panic!("expected usage error"),
        }
    }

    #[test]
    fn every_level_dispatches_the_session_action() {
        for level in ["lite", "full", "ultra", "off"] {
            let r = TersifyCommand.run(&mut ctx(), level);
            let CommandResult::Action(Action::SetSessionTersifyLevel(v)) = r else {
                panic!("{level} must dispatch SetSessionTersifyLevel");
            };
            assert_eq!(v, level);
        }
    }

    #[test]
    fn suggestions_narrow_by_prefix_and_describe_off() {
        let models = ModelState::default();
        let app = AppCtx {
            models: &models,
            cwd: std::path::Path::new("/tmp"),
            has_session_announcements: false,
            billing_surface_visible: true,
            workflows_available: false,
            screen_mode: crate::app::ScreenMode::Inline,
        };
        let items = TersifyCommand.suggest_args(&app, "u").expect("suggestions");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].insert_text, "ultra");
        let all = TersifyCommand.suggest_args(&app, "").expect("suggestions");
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn uppercase_input_normalizes() {
        let r = TersifyCommand.run(&mut ctx(), "  ULTRA ");
        let CommandResult::Action(Action::SetSessionTersifyLevel(v)) = r else {
            panic!("uppercase must dispatch");
        };
        assert_eq!(v, "ultra");
    }
}
