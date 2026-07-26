//! Child environment allowlist / scrub for Claude Agent CLI.
//!
//! Removes API keys and Grok/session secrets. Preserves documented minimal
//! PATH / HOME / locale / workspace env needed for official subscription auth.
//! Never passes keys in argv or env. Never redirects production Claude config
//! in automated tests (callers set HOME / CLAUDE_CONFIG_DIR explicitly).

use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::path::Path;

/// Environment variable names that must never be forwarded to the child.
pub const SCRUBBED_SECRET_KEYS: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "OPENAI_API_KEY",
    "OPENROUTER_API_KEY",
    "XAI_API_KEY",
    "ZAI_API_KEY",
    "DASHSCOPE_API_KEY",
    // Common cloud / gateway keys
    "AWS_SECRET_ACCESS_KEY",
    "AWS_SESSION_TOKEN",
    "GOOGLE_API_KEY",
    "GEMINI_API_KEY",
    // Grok / session secrets
    "GROK_API_KEY",
    "GROK_AUTH_TOKEN",
    "GROK_SESSION_TOKEN",
    "XAI_GROK_API_KEY",
    "XAI_API_TOKEN",
];

/// Keys that are always preserved when present (subscription auth + locale).
pub const PRESERVED_KEYS: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "LOGNAME",
    "TMPDIR",
    "TEMP",
    "TMP",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "LC_MESSAGES",
    "TERM",
    "COLORTERM",
    "TZ",
    // Official Claude subscription / config (binary owns login)
    "CLAUDE_CONFIG_DIR",
    "CLAUDE_CODE_SAFE_MODE",
    // SSH agent for git inside Claude tools (not API keys)
    "SSH_AUTH_SOCK",
    "SSH_AGENT_PID",
];

/// Prefixes scrubbed from the child environment (Grok internals / credentials).
pub const SCRUBBED_PREFIXES: &[&str] = &[
    "GROK_",
    "XAI_GROK_",
    "ANTHROPIC_API",
    "OPENAI_",
    "OPENROUTER_",
];

/// Build a scrubbed environment map for the Claude child process.
///
/// Starts from the current process environment, removes secret keys / Grok
/// prefixes, keeps the preserve allowlist plus non-secret unlisted keys that
/// are not in scrub prefixes (so locale and system vars survive). Explicitly
/// forces removal of every key in [`SCRUBBED_SECRET_KEYS`].
pub fn build_scrubbed_env(extra: &[(&str, OsString)]) -> Vec<(OsString, OsString)> {
    let scrub_exact: HashSet<&str> = SCRUBBED_SECRET_KEYS.iter().copied().collect();
    let preserve: HashSet<&str> = PRESERVED_KEYS.iter().copied().collect();

    let mut out: Vec<(OsString, OsString)> = Vec::new();

    for (k, v) in std::env::vars_os() {
        let key = k.to_string_lossy();
        if scrub_exact.contains(key.as_ref()) {
            continue;
        }
        if SCRUBBED_PREFIXES
            .iter()
            .any(|p| key.starts_with(p) && !preserve.contains(key.as_ref()))
        {
            // Allow CLAUDE_* through even if a prefix matched somehow.
            if key.starts_with("CLAUDE_") {
                out.push((k, v));
                continue;
            }
            // GROK_CLAUDE_CLI_* gates must not reach the child.
            continue;
        }
        // Drop other known credential-shaped keys.
        let upper = key.to_ascii_uppercase();
        if upper.contains("API_KEY")
            || upper.contains("AUTH_TOKEN")
            || upper.contains("ACCESS_TOKEN")
            || upper.contains("SECRET_KEY")
            || upper.ends_with("_SECRET")
        {
            continue;
        }
        out.push((k, v));
    }

    for (k, v) in extra {
        // Extra cannot re-introduce scrubbed secrets.
        if scrub_exact.contains(*k) {
            continue;
        }
        // Replace existing key if present.
        out.retain(|(ek, _)| ek.as_os_str() != OsStr::new(*k));
        out.push((OsString::from(*k), v.clone()));
    }

    // Final pass: ensure scrubbed secrets are absent even if re-added.
    out.retain(|(k, _)| {
        let key = k.to_string_lossy();
        !scrub_exact.contains(key.as_ref())
    });

    out
}

/// Apply scrubbed env to a `tokio::process::Command` (clear then set).
pub fn apply_scrubbed_env(cmd: &mut tokio::process::Command, extra: &[(&str, OsString)]) {
    cmd.env_clear();
    for (k, v) in build_scrubbed_env(extra) {
        cmd.env(k, v);
    }
}

/// Convenience: set cwd-related extras without secrets.
pub fn workspace_extras(cwd: Option<&Path>) -> Vec<(String, OsString)> {
    let mut v = Vec::new();
    if let Some(cwd) = cwd {
        v.push(("PWD".to_owned(), cwd.as_os_str().to_os_string()));
    }
    v
}

/// Test helper: assert none of the scrubbed secrets appear in env pairs.
pub fn env_contains_secret(env: &[(OsString, OsString)], secret_key: &str) -> bool {
    env.iter()
        .any(|(k, _)| k.as_os_str() == OsStr::new(secret_key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrubs_api_keys() {
        // SAFETY: test-only env mutation; serial within this test function.
        unsafe {
            std::env::set_var("ANTHROPIC_API_KEY", "sk-ant-secret");
            std::env::set_var("OPENAI_API_KEY", "sk-openai");
            std::env::set_var("XAI_API_KEY", "xai-secret");
            std::env::set_var("OPENROUTER_API_KEY", "or-secret");
            std::env::set_var("ZAI_API_KEY", "zai-secret");
            std::env::set_var("DASHSCOPE_API_KEY", "ds-secret");
            std::env::set_var("PATH", "/usr/bin");
            std::env::set_var("HOME", "/tmp/test-home");
        }
        let env = build_scrubbed_env(&[]);
        for key in SCRUBBED_SECRET_KEYS {
            assert!(
                !env_contains_secret(&env, key),
                "secret {key} must be scrubbed"
            );
        }
        assert!(env_contains_secret(&env, "PATH"));
        assert!(env_contains_secret(&env, "HOME"));
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
            std::env::remove_var("OPENAI_API_KEY");
            std::env::remove_var("XAI_API_KEY");
            std::env::remove_var("OPENROUTER_API_KEY");
            std::env::remove_var("ZAI_API_KEY");
            std::env::remove_var("DASHSCOPE_API_KEY");
        }
    }

    #[test]
    fn extra_cannot_reinject_api_key() {
        let env = build_scrubbed_env(&[("ANTHROPIC_API_KEY", OsString::from("evil"))]);
        assert!(!env_contains_secret(&env, "ANTHROPIC_API_KEY"));
    }
}
