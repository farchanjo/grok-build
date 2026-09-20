//! `/workflows` -- open the runs view, list the catalog, or toggle workflows.
//!
//! Bare `/workflows` keeps its original meaning (open the runs view). `list`
//! prints the catalog the resolver would actually accept, so a workflow the
//! model is told about is one a run can start. `on` / `off` / `status` drive
//! the `workflows.enabled` row through the same typed action as the settings
//! row.

use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};

/// Row that gates the workflow entry point.
const MASTER: &str = "workflows.enabled";

/// Longest `when_to_use` rendered before it is folded to one line.
const MAX_WHEN_CHARS: usize = 120;

pub struct WorkflowsCommand;

impl SlashCommand for WorkflowsCommand {
    fn name(&self) -> &str {
        "workflows"
    }

    fn description(&self) -> &str {
        "Show workflow runs (phases, agents, progress)"
    }

    fn usage(&self) -> &str {
        "/workflows [list|on|off|status]"
    }

    fn arg_placeholder(&self) -> Option<&str> {
        Some("list")
    }

    fn visible(&self, _ctx: &crate::slash::command::AppCtx) -> bool {
        true
    }

    fn run(&self, ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        match args.trim().to_ascii_lowercase().as_str() {
            "list" | "ls" => CommandResult::Message(list(ctx)),
            "on" | "enable" | "enabled" => CommandResult::Action(Action::SetControl(
                MASTER,
                crate::settings::SettingValue::Bool(true),
            )),
            "off" | "disable" | "disabled" => CommandResult::Action(Action::SetControl(
                MASTER,
                crate::settings::SettingValue::Bool(false),
            )),
            "status" => CommandResult::Message(status(ctx)),
            "" => CommandResult::Action(Action::ToggleWorkflows),
            other => CommandResult::Error(format!(
                "unknown argument `{other}`; usage: {}",
                self.usage()
            )),
        }
    }
}

/// Session working directory for catalog discovery.
///
/// `CommandExecCtx` carries no cwd, and the process working directory is what
/// the resolver scans anyway, so the listing and a run agree.
fn cwd() -> Option<std::path::PathBuf> {
    std::env::current_dir().ok()
}

/// Catalog listing: name, source and the `when_to_use` line, grouped by
/// source so a long catalog stays readable.
fn list(_ctx: &CommandExecCtx) -> String {
    let entries = xai_grok_shell::session::workflow::catalog(cwd().as_deref());
    if entries.is_empty() {
        return "No workflows found. Add one under `.grok/workflows/<name>.rhai`.".to_owned();
    }
    let mut out = format!(
        "{} workflows ({}).\n",
        entries.len(),
        if xai_grok_shell::session::control::bool_at(MASTER, true) {
            "on"
        } else {
            "off — the catalog is hidden from the model"
        }
    );
    for source in ["builtin", "project", "user", "session"] {
        let group: Vec<_> = entries
            .iter()
            .filter(|entry| entry.source == source)
            .collect();
        if group.is_empty() {
            continue;
        }
        out.push_str(&format!("\n{} ({})\n", source, group.len()));
        for entry in group {
            out.push_str(&format!("  {}\n", entry.name));
            out.push_str(&format!("    {}\n", fold(&entry.description)));
            if let Some(when) = &entry.when_to_use {
                out.push_str(&format!("    when: {}\n", fold(when)));
            }
        }
    }
    out.trim_end().to_owned()
}

fn status(_ctx: &CommandExecCtx) -> String {
    format!(
        "Workflows: {}\ncatalog: {} visible from {}\n\
         Use `/workflows list` for names and when to use them; bare `/workflows` opens the runs view.",
        xai_grok_shell::session::control::render(MASTER),
        xai_grok_shell::session::workflow::catalog(cwd().as_deref()).len(),
        cwd()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "?".to_owned()),
    )
}

/// One-line fold of a description, so a long `when_to_use` cannot push the
/// next name off the frame.
fn fold(text: &str) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= MAX_WHEN_CHARS {
        return collapsed;
    }
    let head: String = collapsed.chars().take(MAX_WHEN_CHARS).collect();
    format!("{head}...")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::model_state::ModelState;
    use crate::app::bundle::BundleState;
    use crate::settings::PagerLocalSnapshot;

    static DEFAULT_BUNDLE_STATE: BundleState = BundleState {
        has_cache: false,
        version: String::new(),
        personas: Vec::new(),
        roles: Vec::new(),
        agents: Vec::new(),
        skills: Vec::new(),
        persona_details: Vec::new(),
        role_details: Vec::new(),
    };

    fn make_ctx<'a>(models: &'a ModelState, bundle: &'a BundleState) -> CommandExecCtx<'a> {
        CommandExecCtx {
            models,
            session_id: None,
            bundle_state: bundle,
            screen_mode: crate::app::ScreenMode::Minimal,
            billing_surface_visible: true,
            pager_state: PagerLocalSnapshot::default(),
        }
    }

    #[test]
    fn visibility_is_defensive_during_catalog_reload() {
        let models = ModelState::default();
        for available in [false, true] {
            let ctx = crate::slash::command::AppCtx {
                models: &models,
                cwd: std::path::Path::new("."),
                billing_surface_visible: true,
                workflows_available: available,
                screen_mode: crate::app::ScreenMode::Fullscreen,
            };
            assert!(WorkflowsCommand.visible(&ctx));
        }
    }

    #[test]
    fn dispatches_toggle_workflows() {
        let models = ModelState::default();
        let mut ctx = make_ctx(&models, &DEFAULT_BUNDLE_STATE);
        assert!(matches!(
            WorkflowsCommand.run(&mut ctx, ""),
            CommandResult::Action(Action::ToggleWorkflows)
        ));
    }

    /// `on` / `off` drive the row the settings modal also drives.
    #[test]
    fn on_and_off_dispatch_the_settings_row_action() {
        let models = ModelState::default();
        let mut ctx = make_ctx(&models, &DEFAULT_BUNDLE_STATE);
        match WorkflowsCommand.run(&mut ctx, "off") {
            CommandResult::Action(Action::SetControl(key, value)) => {
                assert_eq!(key, MASTER);
                assert_eq!(value, crate::settings::SettingValue::Bool(false));
            }
            other => panic!("expected SetControl, got {other:?}"),
        }
        match WorkflowsCommand.run(&mut ctx, "on") {
            CommandResult::Action(Action::SetControl(_, value)) => {
                assert_eq!(value, crate::settings::SettingValue::Bool(true))
            }
            other => panic!("expected SetControl, got {other:?}"),
        }
    }

    /// `list` names the content: every entry carries its name and, when the
    /// script declares one, its `when_to_use`.
    #[test]
    fn list_names_every_entry_with_its_when_to_use() {
        let models = ModelState::default();
        let mut ctx = make_ctx(&models, &DEFAULT_BUNDLE_STATE);
        let CommandResult::Message(message) = WorkflowsCommand.run(&mut ctx, "list") else {
            panic!("expected a listing");
        };
        let entries = xai_grok_shell::session::workflow::catalog(cwd().as_deref());
        for entry in &entries {
            assert!(message.contains(&entry.name), "missing {}", entry.name);
            if let Some(when) = &entry.when_to_use {
                let folded = fold(when);
                assert!(
                    message.contains(&folded),
                    "missing when_to_use for {}",
                    entry.name
                );
            }
        }
    }

    #[test]
    fn unknown_argument_is_an_error() {
        let models = ModelState::default();
        let mut ctx = make_ctx(&models, &DEFAULT_BUNDLE_STATE);
        assert!(matches!(
            WorkflowsCommand.run(&mut ctx, "maybe"),
            CommandResult::Error(_)
        ));
    }

    #[test]
    fn fold_truncates_on_a_char_boundary() {
        let long = "é".repeat(MAX_WHEN_CHARS + 10);
        let folded = fold(&long);
        assert!(folded.ends_with("..."));
        assert_eq!(folded.chars().count(), MAX_WHEN_CHARS + 3);
    }
}
