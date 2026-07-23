//! `/providers` — configure OpenAI, OpenRouter, and Codex/ChatGPT access.

use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};

pub struct ProvidersCommand;

impl SlashCommand for ProvidersCommand {
    fn name(&self) -> &str {
        "providers"
    }

    fn aliases(&self) -> &[&str] {
        &["provider"]
    }

    fn description(&self) -> &str {
        "Manage OpenAI, OpenRouter, and Codex providers"
    }

    fn usage(&self) -> &str {
        "/providers"
    }

    fn run(&self, _ctx: &mut CommandExecCtx, _args: &str) -> CommandResult {
        CommandResult::Action(Action::OpenProviders)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::model_state::ModelState;

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
    fn opens_provider_manager() {
        let models = ModelState::default();
        let mut ctx = CommandExecCtx {
            models: &models,
            session_id: None,
            bundle_state: &BUNDLE,
            screen_mode: crate::app::ScreenMode::Inline,
            billing_surface_visible: true,
            pager_state: crate::settings::PagerLocalSnapshot::default(),
        };
        assert!(matches!(
            ProvidersCommand.run(&mut ctx, ""),
            CommandResult::Action(Action::OpenProviders)
        ));
    }
}
