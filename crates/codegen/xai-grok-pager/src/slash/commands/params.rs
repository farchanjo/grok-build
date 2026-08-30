//! Per-model API request overrides — `/temperature`, `/top-p`, `/max-tokens`.
//!
//! Each command persists `[model."<canonical-id>"].<key>` in `config.toml`
//! (surgical `toml_edit` write) so the shell's config hot-reload applies the
//! override to subsequent turns automatically. Bare invocations report the
//! current override and the active default; `off` clears the override.

use crate::app::actions::Action;
use crate::config_toml_edit::{self, ModelParam};
use agent_client_protocol as acp;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};

/// Shared behavior of the three request-param commands.
trait ParamCommand {
    fn param(&self) -> ModelParam;
    fn usage(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn range_label(&self) -> &'static str;
    fn invalid_message(&self) -> &'static str;
    fn in_range(&self, value: f64) -> bool;

    /// Human hint for the model default when no override exists.
    fn default_hint(&self, info: Option<&acp::ModelInfo>) -> String {
        let _ = info;
        String::new()
    }

    fn run(&self, ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        let trimmed = args.trim();
        let Some(model_id) = ctx.models.current.clone() else {
            return CommandResult::Error("No active model".into());
        };
        let canonical = model_id.0.as_ref().to_string();
        let info = ctx.models.available.get(&model_id);

        if trimmed.is_empty() {
            let current = current_override(&canonical, self.param());
            let current_text = match current {
                Some(value) => format!("current override: {value}"),
                None => format!(
                    "no override (model default{})",
                    default_suffix(self.default_hint(info))
                ),
            };
            return CommandResult::Error(format!(
                "Usage: {} [{}, off] — {current_text}",
                self.usage(),
                self.range_label(),
            ));
        }

        if trimmed.eq_ignore_ascii_case("off") || trimmed.eq_ignore_ascii_case("clear") {
            return CommandResult::Action(Action::SaveModelParam {
                model_id: canonical,
                param: self.param(),
                value: None,
            });
        }

        let value: f64 = match trimmed.parse() {
            Ok(value) => value,
            Err(_) => return CommandResult::Error(self.invalid_message().to_owned()),
        };
        if !self.in_range(value) {
            return CommandResult::Error(format!(
                "{} must be {}",
                self.usage().split(' ').next().unwrap_or("value"),
                self.range_label()
            ));
        }
        CommandResult::Action(Action::SaveModelParam {
            model_id: canonical,
            param: self.param(),
            value: Some(value),
        })
    }
}

fn default_suffix(default_hint: String) -> String {
    if default_hint.is_empty() {
        String::new()
    } else {
        format!(": {default_hint}")
    }
}

fn current_override(model_id: &str, param: ModelParam) -> Option<f64> {
    match param {
        ModelParam::Temperature | ModelParam::TopP => {
            config_toml_edit::read_model_param_f64(model_id, param)
        }
        ModelParam::MaxCompletionTokens => config_toml_edit::read_model_param_u64(model_id, param)
            .map(|value| value as f64),
    }
}

fn meta_f64(info: Option<&acp::ModelInfo>, key: &str) -> Option<f64> {
    info.and_then(|info| info.meta.as_ref())
        .and_then(|meta| meta.get(key))
        .and_then(|value| value.as_f64())
}

/// Catalog capability ceiling; `max-tokens` overrides clamp to it.
fn model_output_ceiling(info: Option<&acp::ModelInfo>) -> Option<f64> {
    meta_f64(info, "maxOutputTokens")
}

// ── /temperature ────────────────────────────────────────────────────────────

pub struct TemperatureCommand;

impl ParamCommand for TemperatureCommand {
    fn param(&self) -> ModelParam {
        ModelParam::Temperature
    }

    fn usage(&self) -> &'static str {
        "/temperature"
    }

    fn description(&self) -> &'static str {
        "Set or clear a per-model sampling temperature override"
    }

    fn range_label(&self) -> &'static str {
        "a number between 0 and 2"
    }

    fn invalid_message(&self) -> &'static str {
        "temperature must be a number between 0 and 2"
    }

    fn in_range(&self, value: f64) -> bool {
        (0.0..=2.0).contains(&value)
    }
}

impl SlashCommand for TemperatureCommand {
    fn name(&self) -> &str {
        "temperature"
    }

    fn description(&self) -> &str {
        <TemperatureCommand as ParamCommand>::description(self)
    }

    fn usage(&self) -> &str {
        <TemperatureCommand as ParamCommand>::usage(self)
    }

    fn session_scoped(&self) -> bool {
        true
    }

    fn takes_args(&self) -> bool {
        true
    }

    fn arg_placeholder(&self) -> Option<&str> {
        Some("[<0..=2>|off]")
    }

    fn run(&self, ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        <TemperatureCommand as ParamCommand>::run(self, ctx, args)
    }
}

// ── /top-p ──────────────────────────────────────────────────────────────────

pub struct TopPCommand;

impl ParamCommand for TopPCommand {
    fn param(&self) -> ModelParam {
        ModelParam::TopP
    }

    fn usage(&self) -> &'static str {
        "/top-p"
    }

    fn description(&self) -> &'static str {
        "Set or clear a per-model top_p (nucleus sampling) override"
    }

    fn range_label(&self) -> &'static str {
        "a number between 0 and 1"
    }

    fn invalid_message(&self) -> &'static str {
        "top_p must be a number between 0 and 1"
    }

    fn in_range(&self, value: f64) -> bool {
        (0.0..=1.0).contains(&value)
    }
}

impl SlashCommand for TopPCommand {
    fn name(&self) -> &str {
        "top-p"
    }

    fn description(&self) -> &str {
        <TopPCommand as ParamCommand>::description(self)
    }

    fn usage(&self) -> &str {
        <TopPCommand as ParamCommand>::usage(self)
    }

    fn session_scoped(&self) -> bool {
        true
    }

    fn takes_args(&self) -> bool {
        true
    }

    fn arg_placeholder(&self) -> Option<&str> {
        Some("[<0..=1>|off]")
    }

    fn run(&self, ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        <TopPCommand as ParamCommand>::run(self, ctx, args)
    }
}

// ── /max-tokens ─────────────────────────────────────────────────────────────

pub struct MaxTokensCommand;

impl ParamCommand for MaxTokensCommand {
    fn param(&self) -> ModelParam {
        ModelParam::MaxCompletionTokens
    }

    fn usage(&self) -> &'static str {
        "/max-tokens"
    }

    fn description(&self) -> &'static str {
        "Set or clear a per-model max completion tokens override"
    }

    fn range_label(&self) -> &'static str {
        "a positive integer"
    }

    fn invalid_message(&self) -> &'static str {
        "max-tokens must be a positive integer"
    }

    fn in_range(&self, value: f64) -> bool {
        value >= 1.0 && value <= u32::MAX as f64 && value.fract() == 0.0
    }

    fn default_hint(&self, info: Option<&acp::ModelInfo>) -> String {
        match model_output_ceiling(info) {
            Some(ceiling) => format!(", capability ceiling {ceiling}"),
            None => String::new(),
        }
    }

    fn run(&self, ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        let trimmed = args.trim();
        if !trimmed.is_empty() && !trimmed.eq_ignore_ascii_case("off") {
            let value: f64 = match trimmed.parse() {
                Ok(value) => value,
                Err(_) => {
                    return CommandResult::Error(self.invalid_message().to_owned());
                }
            };
            let info = ctx
                .models
                .current
                .as_ref()
                .and_then(|id| ctx.models.available.get(id));
            if !self.in_range(value) {
                return CommandResult::Error(format!(
                    "max-tokens must be an integer between 1 and {}",
                    u32::MAX
                ));
            }
            if let Some(ceiling) = model_output_ceiling(info)
                && value > ceiling
            {
                return CommandResult::Error(format!(
                    "max-tokens {value} exceeds this model's capability ceiling of {ceiling}"
                ));
            }
        }
        MaxTokensCommand::run_shared(self, ctx, args)
    }
}

impl MaxTokensCommand {
    fn run_shared(&self, ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        ParamCommand::run(self, ctx, args)
    }
}

impl SlashCommand for MaxTokensCommand {
    fn name(&self) -> &str {
        "max-tokens"
    }

    fn description(&self) -> &str {
        <MaxTokensCommand as ParamCommand>::description(self)
    }

    fn usage(&self) -> &str {
        <MaxTokensCommand as ParamCommand>::usage(self)
    }

    fn session_scoped(&self) -> bool {
        true
    }

    fn takes_args(&self) -> bool {
        true
    }

    fn arg_placeholder(&self) -> Option<&str> {
        Some("[<n>|off]")
    }

    fn run(&self, ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        <MaxTokensCommand as ParamCommand>::run(self, ctx, args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temperature_range_is_inclusive_zero_to_two() {
        let cmd = TemperatureCommand;
        assert!(cmd.in_range(0.0) && cmd.in_range(2.0));
        assert!(!cmd.in_range(2.1));
        assert!(!cmd.in_range(-0.1));
    }

    #[test]
    fn top_p_range_is_inclusive_zero_to_one() {
        let cmd = TopPCommand;
        assert!(cmd.in_range(0.0) && cmd.in_range(1.0));
        assert!(!cmd.in_range(1.1));
    }

    #[test]
    fn max_tokens_requires_positive_integers() {
        let cmd = MaxTokensCommand;
        assert!(cmd.in_range(1.0) && cmd.in_range(u32::MAX as f64));
        assert!(!cmd.in_range(0.0));
        assert!(!cmd.in_range(1.5));
    }

    #[test]
    fn max_tokens_default_hint_reports_capability_ceiling() {
        let mut info = acp::ModelInfo::new("zdr:z-ai/glm-5.3-flash", "Z.ai: GLM 5.3 Flash");
        info.meta = serde_json::json!({ "maxOutputTokens": 64000.0 }).as_object().cloned();
        assert_eq!(
            MaxTokensCommand.default_hint(Some(&info)),
            ", capability ceiling 64000"
        );
    }
}
