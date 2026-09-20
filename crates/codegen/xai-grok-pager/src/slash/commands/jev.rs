//! `/jev` -- control Jev-guided compaction pruning.
//!
//! `on` / `off` dispatch the same typed action as the settings row, so the
//! value persists to `[compaction.jev].enabled` and the running session picks
//! it up through the existing `[compaction]` reload fan-out (no restart).
//! `transport` does the same for `[compaction.jev].transport`. Bare `/jev` and
//! `/jev status` report the effective state, including which credential chain
//! link produced the key — never the key itself.

use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};
use xai_grok_shell::session::helpers::jev_prune::{
    JevTransport, ResolvedJevPrune, resolve_credential,
};

/// Control Jev-guided compaction pruning.
pub struct JevCommand;

impl SlashCommand for JevCommand {
    fn name(&self) -> &str {
        "jev"
    }

    fn description(&self) -> &str {
        "Toggle Jev-guided compaction pruning and pick its transport"
    }

    fn usage(&self) -> &str {
        "/jev [on|off|status|transport native|openrouter]"
    }

    fn arg_placeholder(&self) -> Option<&str> {
        Some("on/off/transport")
    }

    fn run(&self, ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        let args = args.trim();
        let (verb, rest) = match args.split_once(char::is_whitespace) {
            Some((verb, rest)) => (verb.to_ascii_lowercase(), rest.trim()),
            None => (args.to_ascii_lowercase(), ""),
        };
        match verb.as_str() {
            "on" | "enable" | "enabled" => {
                CommandResult::Action(Action::SetCompactionJevEnabled(true))
            }
            "off" | "disable" | "disabled" => {
                CommandResult::Action(Action::SetCompactionJevEnabled(false))
            }
            "transport" | "wire" => jev_transport(rest),
            "" | "status" => CommandResult::Message(jev_status(ctx)),
            other => CommandResult::Error(format!(
                "unknown argument `{other}`; usage: {}",
                self.usage()
            )),
        }
    }
}

/// `transport <name>`; a bare `transport` reports the two spellings.
fn jev_transport(rest: &str) -> CommandResult {
    if rest.is_empty() {
        return CommandResult::Message(format!(
            "usage: /jev transport <{}>",
            JevTransport::ALL
                .iter()
                .map(|t| t.as_str())
                .collect::<Vec<_>>()
                .join("|")
        ));
    }
    match JevTransport::parse(rest) {
        Some(transport) => {
            CommandResult::Action(Action::SetJevTransport(transport.as_str().to_owned()))
        }
        None => CommandResult::Error(format!(
            "unknown transport `{rest}`; expected {}",
            JevTransport::ALL
                .iter()
                .map(|t| t.as_str())
                .collect::<Vec<_>>()
                .join(" or ")
        )),
    }
}

/// Effective `[compaction.jev]` policy, from config when set.
fn jev_config() -> Option<xai_grok_shell::agent::config::JevPruneConfig> {
    xai_grok_shell::config::load_effective_config()
        .ok()
        .and_then(|root| root.get("compaction").cloned())
        .and_then(|value| {
            value
                .try_into::<xai_grok_shell::agent::config::CompactionConfig>()
                .ok()
        })
        .and_then(|config| config.jev)
}

fn jev_status(ctx: &CommandExecCtx) -> String {
    let cfg = jev_config();
    let resolved = ResolvedJevPrune::from_config(cfg.as_ref());
    let (model, endpoint) = (
        cfg.as_ref()
            .and_then(|c| c.model.clone())
            .unwrap_or_else(|| resolved.model.clone()),
        cfg.as_ref()
            .and_then(|c| c.endpoint.clone())
            .unwrap_or_else(|| resolved.endpoint.clone()),
    );
    // The credential source is what makes a two-transport setup debuggable: a
    // 401 with no chain named is a dead end. Only the label is printed.
    let credential = match resolve_credential(
        resolved.transport,
        cfg.as_ref().and_then(|c| c.api_key_env.as_deref()),
        &xai_grok_config::grok_home(),
    ) {
        Ok(resolved) => resolved.source.label(),
        Err(error) => format!("unavailable ({error})"),
    };
    format!(
        "Jev-guided pruning: {}\ntransport: {}\nmodel: {model}\nendpoint: {endpoint}\n\
         credential: {credential}\n\
         Prunes stale tool calls and tool results from the summarizer view only; \
         user and assistant text is untouched.",
        if ctx.pager_state.compaction_jev_enabled {
            "on"
        } else {
            "off"
        },
        resolved.transport.as_str(),
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
    fn transport_dispatches_the_same_typed_action_as_the_settings_row() {
        let cmd = JevCommand;
        let models = ModelState::default();
        let bundle = BundleState::default();
        for (arg, expected) in [("native", "native"), ("openrouter", "openrouter")] {
            let mut ctx = make_ctx(&models, &bundle, false);
            match cmd.run(&mut ctx, &format!("transport {arg}")) {
                CommandResult::Action(Action::SetJevTransport(value)) => {
                    assert_eq!(value, expected)
                }
                other => panic!("expected Action::SetJevTransport({expected}), got {other:?}"),
            }
        }
    }

    #[test]
    fn bare_transport_lists_the_spellings() {
        let cmd = JevCommand;
        let models = ModelState::default();
        let bundle = BundleState::default();
        let mut ctx = make_ctx(&models, &bundle, false);
        match cmd.run(&mut ctx, "transport") {
            CommandResult::Message(msg) => {
                assert!(
                    msg.contains("native") && msg.contains("openrouter"),
                    "{msg}"
                )
            }
            other => panic!("expected a usage message, got {other:?}"),
        }
    }

    #[test]
    fn unknown_transport_is_an_error() {
        let cmd = JevCommand;
        let models = ModelState::default();
        let bundle = BundleState::default();
        let mut ctx = make_ctx(&models, &bundle, false);
        assert!(matches!(
            cmd.run(&mut ctx, "transport carrier-pigeon"),
            CommandResult::Error(_)
        ));
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
                    // Every effective field is spelled out; the values come
                    // from the shell resolver, never from a local copy.
                    for field in ["transport: ", "model: ", "endpoint: "] {
                        assert!(msg.contains(field), "missing {field:?} in {msg}");
                    }
                }
                other => panic!("expected status message for `{args}`, got {other:?}"),
            }
        }
    }

    /// The fallbacks the row prints are the shell client's own defaults, so a
    /// bare config and the wire agree by construction.
    #[test]
    fn status_fallbacks_are_the_shell_defaults() {
        use xai_grok_shell::session::helpers::jev_prune::{DEFAULT_ENDPOINT, DEFAULT_MODEL};
        let resolved = ResolvedJevPrune::from_config(None);
        assert_eq!(resolved.model, DEFAULT_MODEL);
        assert_eq!(resolved.endpoint, DEFAULT_ENDPOINT);
    }

    /// `status` names the credential chain link and never the key.
    #[test]
    fn status_prints_a_credential_source_label() {
        let cmd = JevCommand;
        let models = ModelState::default();
        let bundle = BundleState::default();
        let mut ctx = make_ctx(&models, &bundle, false);
        let CommandResult::Message(msg) = cmd.run(&mut ctx, "status") else {
            panic!("expected a status message");
        };
        let line = msg
            .lines()
            .find(|line| line.starts_with("credential: "))
            .expect("status carries a credential line");
        let source = line.trim_start_matches("credential: ");
        assert!(
            source.starts_with("env:")
                || source.starts_with("file:")
                || source.starts_with("auth.json:")
                || source.starts_with("unavailable"),
            "unexpected source label: {source}"
        );
        assert!(msg.contains("transport: "), "{msg}");
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
