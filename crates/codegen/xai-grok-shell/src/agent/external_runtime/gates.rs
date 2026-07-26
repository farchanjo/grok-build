//! Always-compiled build/runtime gates for Claude Agent CLI.
//!
//! The full process runtime lives behind `feature = "claude-cli-runtime"`.
//! Runtime opt-in for MVP is **environment-only** (`GROK_CLAUDE_CLI_RUNTIME`).
//! There is no config-file key in this MVP.

/// Environment variable that enables the experimental Claude CLI runtime.
pub const CLAUDE_CLI_ENV_OPT_IN: &str = "GROK_CLAUDE_CLI_RUNTIME";

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

/// Runtime opt-in from environment only (MVP).
pub fn claude_cli_runtime_opt_in() -> bool {
    match std::env::var(CLAUDE_CLI_ENV_OPT_IN) {
        Ok(v) => parse_runtime_opt_in_value(&v),
        Err(_) => false,
    }
}

/// Both gates open (feature compiled **and** runtime env opt-in).
///
/// Does **not** include binary probe — see [`super::probe_cache`].
pub fn claude_cli_both_gates_open() -> bool {
    claude_cli_feature_compiled() && claude_cli_runtime_opt_in()
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
    fn default_runtime_opt_in_parser_is_fail_closed() {
        assert!(!parse_runtime_opt_in_value(""));
        assert!(!parse_runtime_opt_in_value("false"));
        assert_eq!(
            claude_cli_feature_compiled(),
            cfg!(feature = "claude-cli-runtime")
        );
    }
}
