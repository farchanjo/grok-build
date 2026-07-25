//! Historical `/logout` — no longer a supported surface.
//!
//! Disconnect credentials under `/providers`. This command remains registered
//! so typed `/logout` prints actionable guidance; it is hidden from
//! help/completion/palette and does not clear credentials.

use crate::slash::command::{AppCtx, CommandExecCtx, CommandResult, SlashCommand};

pub struct LogoutCommand;

impl SlashCommand for LogoutCommand {
    fn name(&self) -> &str {
        "logout"
    }

    fn description(&self) -> &str {
        "Deprecated — use /providers to disconnect credentials"
    }

    fn usage(&self) -> &str {
        "/logout  (deprecated; use /providers)"
    }

    fn visible(&self, _ctx: &AppCtx) -> bool {
        false
    }

    fn run(&self, _ctx: &mut CommandExecCtx, _args: &str) -> CommandResult {
        CommandResult::Message(
            "Global /logout is no longer supported. Open /providers, select the \
             provider, and disconnect it there. This command does not clear \
             credentials."
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
    fn logout_is_hidden_and_points_at_providers() {
        let cmd = LogoutCommand;
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
                assert!(msg.contains("does not clear"), "{msg}");
            }
            other => panic!("expected Message, got {other:?}"),
        }
    }
}
