//! Historical `/login` — no longer a supported surface.
//!
//! Credential repair lives under `/providers`. This command remains registered
//! so typed `/login` prints actionable guidance instead of "unknown command";
//! it is hidden from help/completion/palette and never starts auth.

use crate::slash::command::{AppCtx, CommandExecCtx, CommandResult, SlashCommand};

pub struct LoginCommand;

impl SlashCommand for LoginCommand {
    fn name(&self) -> &str {
        "login"
    }

    fn description(&self) -> &str {
        "Deprecated — use /providers to connect credentials"
    }

    fn usage(&self) -> &str {
        "/login  (deprecated; use /providers)"
    }

    fn visible(&self, _ctx: &AppCtx) -> bool {
        false
    }

    fn run(&self, _ctx: &mut CommandExecCtx, _args: &str) -> CommandResult {
        CommandResult::Message(
            "Global /login is no longer supported. Open /providers and connect \
             the provider you need (xAI OAuth/API key, OpenAI/ChatGPT, OpenRouter, \
             or a custom provider)."
                .into(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::model_state::ModelState;
    use crate::slash::command::{AppCtx, CommandResult};

    static BUNDLE: crate::app::bundle::BundleState = crate::app::bundle::BundleState {
        has_cache: false,
        version: String::new(),
        personas: Vec::new(),
        roles: Vec::new(),
        agents: Vec::new(),
        skills: Vec::new(),
        persona_details: Vec::new(),
        role_details: Vec::new(),
    };

    #[test]
    fn login_is_hidden_and_points_at_providers() {
        let cmd = LoginCommand;
        let models = ModelState::default();
        let cwd = std::path::Path::new(".");
        let app_ctx = AppCtx {
            models: &models,
            cwd,
            has_session_announcements: false,
            billing_surface_visible: true,
            workflows_available: false,
            screen_mode: crate::app::ScreenMode::Inline,
        };
        assert!(!cmd.visible(&app_ctx), "must be hidden from help/palette");
        let mut exec = CommandExecCtx {
            models: &models,
            session_id: None,
            bundle_state: &BUNDLE,
            screen_mode: crate::app::ScreenMode::Inline,
            billing_surface_visible: true,
            pager_state: crate::settings::PagerLocalSnapshot::default(),
        };
        match cmd.run(&mut exec, "") {
            CommandResult::Message(msg) => {
                assert!(msg.contains("/providers"), "{msg}");
                assert!(msg.contains("no longer supported"), "{msg}");
                // Deprecation text may mention the historical /login name, but
                // must not start auth or send the user to a global login flow.
                assert!(!msg.contains("Action::Login"));
            }
            other => panic!("expected Message, got {other:?}"),
        }
    }
}
