//! Phase-4 control commands: one `on|off|status` surface per target.
//!
//! Every command here has the same shape as `/jev`: `on` and `off` dispatch
//! **one typed action** that the matching settings row also dispatches, so the
//! command and the row cannot drift; `status` prints the effective values.
//!
//! The rows themselves live in `xai_grok_shell::session::control`, which is
//! also what the consumers read. A command therefore carries no copy of the
//! data — it names a master row and the rows its `status` should print, and
//! the shell resolves both.

use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};
use xai_grok_shell::session::control;

/// One control command: a master row plus the rows `status` reports.
pub struct ControlCommand {
    name: &'static str,
    description: &'static str,
    usage: &'static str,
    /// Row `on`/`off` flips.
    master: &'static str,
    /// Rows `status` prints after the master, in declaration order.
    rows: &'static [&'static str],
    /// Rows that are choices, flipped by `set <value>` instead of on/off.
    choices: &'static [&'static str],
    /// Live lines the pager can add from its own state (no config read).
    live: Option<fn(&CommandExecCtx) -> Vec<String>>,
    /// Trailing sentence printed by `status`.
    note: &'static str,
}

impl SlashCommand for ControlCommand {
    fn name(&self) -> &str {
        self.name
    }

    fn description(&self) -> &str {
        self.description
    }

    fn usage(&self) -> &str {
        self.usage
    }

    fn arg_placeholder(&self) -> Option<&str> {
        Some("on/off/status")
    }

    fn visible(&self, _ctx: &crate::slash::command::AppCtx) -> bool {
        true
    }

    fn run(&self, ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        let args = args.trim();
        let (verb, rest) = match args.split_once(char::is_whitespace) {
            Some((verb, rest)) => (verb.to_ascii_lowercase(), rest.trim()),
            None => (args.to_ascii_lowercase(), ""),
        };
        match verb.as_str() {
            "on" | "enable" | "enabled" if self.has_master() => self.set_bool(true),
            "off" | "disable" | "disabled" if self.has_master() => self.set_bool(false),
            "set" | "backend" | "transport" => self.set_choice(rest),
            "" | "status" => CommandResult::Message(self.status(ctx)),
            // A choice command takes the value bare: `/tool-search dense`.
            other if !self.choices.is_empty() => self.set_choice(other),
            other => {
                CommandResult::Error(format!("unknown argument `{other}`; usage: {}", self.usage))
            }
        }
    }
}

impl ControlCommand {
    fn has_master(&self) -> bool {
        !self.master.is_empty()
    }

    fn set_bool(&self, value: bool) -> CommandResult {
        CommandResult::Action(Action::SetControl(
            self.master,
            crate::settings::SettingValue::Bool(value),
        ))
    }

    /// Choice command: `set <value>` writes the one row it owns.
    fn set_choice(&self, rest: &str) -> CommandResult {
        let Some(path) = self.choices.first() else {
            return CommandResult::Error(format!("usage: {}", self.usage));
        };
        if rest.is_empty() {
            return CommandResult::Message(format!(
                "usage: {} — current: {}",
                self.usage,
                control::render(path)
            ));
        }
        let choices = control::spec_for(path).map(|spec| spec.kind);
        let allowed: &[&str] = match choices {
            Some(control::ControlKind::Choice(choices)) => choices,
            _ => return CommandResult::Error(format!("`{path}` is not a choice row")),
        };
        let normalized = rest.to_ascii_lowercase();
        match allowed.iter().find(|choice| **choice == normalized) {
            Some(choice) => CommandResult::Action(Action::SetControl(
                path,
                crate::settings::SettingValue::Enum(choice),
            )),
            None => CommandResult::Error(format!(
                "unknown value `{rest}`; expected {}",
                allowed.join(" | ")
            )),
        }
    }

    fn status(&self, ctx: &CommandExecCtx) -> String {
        let title = control::spec_for(if self.has_master() {
            self.master
        } else {
            self.rows.first().copied().unwrap_or("")
        })
        .map(|spec| spec.label)
        .unwrap_or(self.name);
        let mut out = format!("{title}: {}", self.headline());
        for row in self.rows {
            let Some(spec) = control::spec_for(row) else {
                continue;
            };
            out.push_str(&format!("\n{}: {}", spec.label, control::render(row)));
        }
        if let Some(live) = self.live {
            for line in live(ctx) {
                out.push('\n');
                out.push_str(&line);
            }
        }
        out.push('\n');
        out.push_str(self.note);
        out
    }

    /// Master state, or the owned choice's value when there is no master.
    fn headline(&self) -> String {
        if self.has_master() {
            control::render(self.master)
        } else {
            self.choices
                .first()
                .map(|path| control::render(path))
                .unwrap_or_else(|| "n/a".to_owned())
        }
    }
}

/// Live line: the auto-mode gate that decides whether the classifier is even
/// reached, which is separate from the classifier row itself.
fn permission_live(ctx: &CommandExecCtx) -> Vec<String> {
    vec![format!(
        "auto mode gate: {}",
        if ctx.pager_state.auto_mode_gate {
            "on"
        } else {
            "off"
        }
    )]
}

/// Live line: the tool catalog the backend selects over. The width is the
/// scale argument for the fused backend, so it belongs next to the row.
fn tool_search_live(ctx: &CommandExecCtx) -> Vec<String> {
    vec![format!(
        "catalog: {} tools",
        ctx.pager_state.tool_catalog.len()
    )]
}

/// Live line: whether a compaction is running right now, so a Jev flip is not
/// read as taking effect mid-run.
fn prime_live(ctx: &CommandExecCtx) -> Vec<String> {
    vec![format!(
        "compaction: {}",
        if ctx.pager_state.compaction_in_progress {
            "running"
        } else {
            "idle"
        }
    )]
}

/// The Phase-4 control commands, in registry display order.
pub fn control_commands() -> Vec<std::sync::Arc<dyn SlashCommand>> {
    control_table()
        .into_iter()
        .map(|command| std::sync::Arc::new(command) as std::sync::Arc<dyn SlashCommand>)
        .collect()
}

/// The same commands, still concrete so the registry and the tests can read
/// their declared rows.
pub fn control_table() -> Vec<ControlCommand> {
    vec![
        ControlCommand {
            name: "permission-classifier",
            description: "Toggle the auto-mode permission classifier and its four floors",
            usage: "/permission-classifier [on|off|status]",
            master: "permission.classifier.enabled",
            rows: &[
                "permission.floors.write",
                "permission.floors.unsafe_env",
                "permission.floors.opaque_shell",
                "permission.floors.exec",
            ],
            choices: &[],
            live: Some(permission_live),
            note: "A floor overrides a classifier allow, so a disarmed floor is the cheapest \
                   prompt to remove; the exec floor is the dominant term.",
        },
        ControlCommand {
            name: "laziness",
            description: "Toggle the laziness detector and report its last gate",
            usage: "/laziness [on|off|status]",
            master: "laziness.enabled",
            rows: &[
                "laziness.min_confidence",
                "laziness.idle_threshold_ms",
                "laziness.max_nudges_per_session",
            ],
            choices: &[],
            live: None,
            note: "Confidence is a percentage; with the nudge cap at 0 the detector classifies \
                   without injecting anything (observation-only).",
        },
        ControlCommand {
            name: "prime",
            description: "Toggle prime injection and report its index width",
            usage: "/prime [on|off|status]",
            master: "prime.enabled",
            rows: &["prime.index_width", "prime.strip_scaffolding"],
            choices: &[],
            live: Some(prime_live),
            note: "The index width is characters per skill body; accuracy saturates at 200.",
        },
        ControlCommand {
            name: "memory-gate",
            description: "Toggle the memory admission gate and report its thresholds",
            usage: "/memory-gate [on|off|status]",
            master: "memory.gate.enabled",
            rows: &[
                "memory.enabled",
                "memory.session.save_on_end",
                "memory.gate.worth_threshold",
                "memory.gate.covered_threshold",
                "memory.gate.scope_routing",
            ],
            choices: &[],
            live: None,
            note: "Thresholds are percentages: worth is the floor to keep a note, covered is the \
                   ceiling above which a note is a restatement.",
        },
        ControlCommand {
            name: "agents-recommend",
            description: "Toggle agent recommendation and its delegates graph",
            usage: "/agents-recommend [on|off|status]",
            master: "agents.recommend.enabled",
            rows: &["agents.recommend.graph", "agents.recommend.default_effort"],
            choices: &[],
            live: None,
            note: "The delegates graph adds on top of retrieval by 0-1 cases, which is why it \
                   ships off.",
        },
        ControlCommand {
            name: "goal-verify",
            description: "Toggle goal verification and report the skeptic panel",
            usage: "/goal-verify [on|off|status]",
            master: "goal.verify.enabled",
            rows: &["goal.verifier_count", "goal.verify.pre_filter"],
            choices: &[],
            live: None,
            note: "Panel size is 1-5 skeptics per verification attempt; the pre-filter is off \
                   until it is measured against the panel.",
        },
        ControlCommand {
            name: "todo-gate",
            description: "Toggle the turn-end todo gate and its caps",
            usage: "/todo-gate [on|off|status]",
            master: "todo_gate.enabled",
            rows: &[
                "todo_gate.max_fires_per_prompt",
                "todo_gate.max_items_named",
                "todo_gate.pick_with_decision",
            ],
            choices: &[],
            live: None,
            note: "The item cap keeps the dump short; the decision pick only fires on a list \
                   longer than three and falls back to insertion order.",
        },
        ControlCommand {
            name: "tool-search",
            description: "Report and pick the tool search backend",
            usage: "/tool-search [status|bm25|dense|fused]",
            master: "",
            rows: &[],
            choices: &["search.tool_backend"],
            live: Some(tool_search_live),
            note: "Fused keeps BM25 and the exact-name short-circuit and adds the dense index.",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::model_state::ModelState;
    use crate::app::bundle::BundleState;
    use crate::settings::PagerLocalSnapshot;

    static BUNDLE: BundleState = BundleState {
        has_cache: false,
        version: String::new(),
        personas: Vec::new(),
        roles: Vec::new(),
        agents: Vec::new(),
        skills: Vec::new(),
        persona_details: Vec::new(),
        role_details: Vec::new(),
    };

    fn ctx<'a>(models: &'a ModelState) -> CommandExecCtx<'a> {
        CommandExecCtx {
            models,
            session_id: None,
            bundle_state: &BUNDLE,
            screen_mode: crate::app::ScreenMode::Inline,
            billing_surface_visible: true,
            pager_state: PagerLocalSnapshot::default(),
        }
    }

    /// Every command must be reachable by name and register exactly once.
    #[test]
    fn every_control_command_is_registered_once() {
        let commands = control_table();
        let mut names: Vec<&str> = commands.iter().map(|c| c.name()).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(names.len(), before, "duplicate command name");
        for expected in [
            "permission-classifier",
            "laziness",
            "prime",
            "memory-gate",
            "agents-recommend",
            "goal-verify",
            "todo-gate",
            "tool-search",
        ] {
            assert!(names.contains(&expected), "{expected} not registered");
        }
    }

    /// `on`/`off` dispatch the same typed action the settings row does, on the
    /// command's master row.
    #[test]
    fn on_and_off_dispatch_the_master_row() {
        let models = ModelState::default();
        for command in control_table() {
            if command.master.is_empty() {
                continue;
            }
            let mut c = ctx(&models);
            match command.run(&mut c, "off") {
                CommandResult::Action(Action::SetControl(key, value)) => {
                    assert_eq!(key, command.master);
                    assert_eq!(value, crate::settings::SettingValue::Bool(false));
                }
                other => panic!("{} off: expected SetControl, got {other:?}", command.name()),
            }
            let mut c = ctx(&models);
            match command.run(&mut c, "on") {
                CommandResult::Action(Action::SetControl(_, value)) => {
                    assert_eq!(value, crate::settings::SettingValue::Bool(true))
                }
                other => panic!("{} on: expected SetControl, got {other:?}", command.name()),
            }
        }
    }

    /// `status` names the content, not just the target.
    #[test]
    fn status_prints_the_named_rows() {
        let models = ModelState::default();
        for command in control_table() {
            let mut c = ctx(&models);
            let CommandResult::Message(message) = command.run(&mut c, "status") else {
                panic!("{} status must print a message", command.name());
            };
            for row in command.rows {
                let label = control::spec_for(row).expect("row spec").label;
                assert!(
                    message.contains(label),
                    "{}: missing `{label}`",
                    command.name()
                );
            }
            assert!(
                !command.note.is_empty(),
                "{} has no trailing note",
                command.name()
            );
        }
    }

    /// A choice command rejects an unknown value instead of writing junk.
    #[test]
    fn tool_search_rejects_an_unknown_backend() {
        let models = ModelState::default();
        let commands = control_table();
        let command = commands
            .iter()
            .find(|command| command.name() == "tool-search")
            .expect("tool-search registered");
        let mut c = ctx(&models);
        assert!(matches!(
            command.run(&mut c, "carrier-pigeon"),
            CommandResult::Error(_)
        ));
        match command.run(&mut c, "dense") {
            CommandResult::Action(Action::SetControl(key, value)) => {
                assert_eq!(key, "search.tool_backend");
                assert_eq!(value, crate::settings::SettingValue::Enum("dense"));
            }
            other => panic!("expected SetControl, got {other:?}"),
        }
    }
}
