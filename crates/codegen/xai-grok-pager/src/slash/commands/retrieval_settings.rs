//! `/retrieval-settings` — named retrieval graph editor.

use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};

pub struct RetrievalSettingsCommand;

impl SlashCommand for RetrievalSettingsCommand {
    fn name(&self) -> &str {
        "retrieval-settings"
    }

    fn aliases(&self) -> &[&str] {
        &["retrieval", "retrieval-config"]
    }

    fn description(&self) -> &str {
        "Manage embedding models, rerankers, retrieval profiles, and prime settings"
    }

    fn usage(&self) -> &str {
        "/retrieval-settings"
    }

    fn run(&self, _ctx: &mut CommandExecCtx, _args: &str) -> CommandResult {
        CommandResult::Action(Action::OpenRetrievalSettings)
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
    fn opens_retrieval_settings() {
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
            RetrievalSettingsCommand.run(&mut ctx, ""),
            CommandResult::Action(Action::OpenRetrievalSettings)
        ));
    }
}
