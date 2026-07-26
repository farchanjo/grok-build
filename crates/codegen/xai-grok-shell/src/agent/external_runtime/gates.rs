//! Always-compiled build/runtime gates for Claude Agent CLI.
//!
//! The full process runtime lives behind `feature = "claude-cli-runtime"`.
//! These helpers are available in every build so capability matrix and
//! fail-closed parsers stay consistent.

/// Environment variable that enables the experimental Claude CLI runtime.
pub const CLAUDE_CLI_ENV_OPT_IN: &str = "GROK_CLAUDE_CLI_RUNTIME";

/// Optional development config key (documentation + callers).
pub const CLAUDE_CLI_CONFIG_KEY: &str = "experimental.claude_cli_runtime";

/// `true` when this build was compiled with `claude-cli-runtime`.
#[inline]
pub const fn claude_cli_feature_compiled() -> bool {
    cfg!(feature = "claude-cli-runtime")
}

/// Fail-closed parser for opt-in strings.
///
/// Accepts (case-insensitive, trimmed): `1`, `true`, `yes`, `on`.
/// Everything else is **disabled**.
pub fn parse_runtime_opt_in_value(raw: &str) -> bool {
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

/// Runtime opt-in from environment only.
pub fn claude_cli_runtime_opt_in() -> bool {
    match std::env::var(CLAUDE_CLI_ENV_OPT_IN) {
        Ok(v) => parse_runtime_opt_in_value(&v),
        Err(_) => false,
    }
}

/// Runtime opt-in combining env and an already-parsed config flag.
pub fn claude_cli_runtime_opt_in_from(config_flag: Option<bool>) -> bool {
    if claude_cli_runtime_opt_in() {
        return true;
    }
    matches!(config_flag, Some(true))
}

/// Both gates open (feature compiled **and** runtime opt-in).
pub fn claude_cli_both_gates_open() -> bool {
    claude_cli_feature_compiled() && claude_cli_runtime_opt_in()
}

/// Same with an explicit config flag.
pub fn claude_cli_both_gates_open_from(config_flag: Option<bool>) -> bool {
    claude_cli_feature_compiled() && claude_cli_runtime_opt_in_from(config_flag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_accepts_only_explicit_truthy() {
        for ok in ["1", "true", "TRUE", " Yes ", "on", "ON"] {
            assert!(parse_runtime_opt_in_value(ok), "expected true for {ok:?}");
        }
        for bad in [
            "", "0", "false", "no", "off", "enable", "enabled", "maybe", "2",
        ] {
            assert!(
                !parse_runtime_opt_in_value(bad),
                "expected false for {bad:?}"
            );
        }
    }

    #[test]
    fn default_runtime_opt_in_is_off_without_env() {
        // Do not assert global env absence (other tests may set it); assert parser.
        assert!(!parse_runtime_opt_in_value(""));
        assert!(!claude_cli_runtime_opt_in_from(None));
        assert!(!claude_cli_runtime_opt_in_from(Some(false)));
        assert!(claude_cli_runtime_opt_in_from(Some(true)) || !claude_cli_feature_compiled());
        // When feature is off, both_gates stays false even with config true.
        if !claude_cli_feature_compiled() {
            assert!(!claude_cli_both_gates_open_from(Some(true)));
        }
    }
}
