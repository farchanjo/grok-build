//! Strict child environment allowlist for Claude Agent CLI.
//!
//! `env_clear` then re-apply only the documented minimal set needed for the
//! official binary and subscription auth. Never forwards provider/Grok/cloud
//! credentials or arbitrary inherited env.

use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::path::Path;

/// Keys that may be preserved when present (exact match, case-sensitive).
pub const ALLOWLIST_KEYS: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "LOGNAME",
    "SHELL",
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
    // Official Claude config directory (binary owns login; tests use temp dirs)
    "CLAUDE_CONFIG_DIR",
    // Optional TLS / proxy (no credentials)
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "REQUESTS_CA_BUNDLE",
    "CURL_CA_BUNDLE",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "NO_PROXY",
    "http_proxy",
    "https_proxy",
    "no_proxy",
    "ALL_PROXY",
    "all_proxy",
];

/// Secrets that must never appear, even if someone tries to allowlist them.
pub const FORBIDDEN_KEYS: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "OPENAI_API_KEY",
    "OPENROUTER_API_KEY",
    "XAI_API_KEY",
    "ZAI_API_KEY",
    "DASHSCOPE_API_KEY",
    "GROK_API_KEY",
    "GROK_AUTH_TOKEN",
    "GROK_SESSION_TOKEN",
    "XAI_GROK_API_KEY",
    "XAI_API_TOKEN",
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
    "AWS_SESSION_TOKEN",
    "GOOGLE_API_KEY",
    "GEMINI_API_KEY",
    "GOOGLE_APPLICATION_CREDENTIALS",
    "AZURE_CLIENT_SECRET",
    "AZURE_CLIENT_ID",
    "AZURE_TENANT_ID",
    "GITHUB_TOKEN",
    "GH_TOKEN",
    "SSH_AUTH_SOCK",
    "SSH_AGENT_PID",
];

/// Prefixes that are always excluded (even if not in FORBIDDEN_KEYS).
const FORBIDDEN_PREFIXES: &[&str] = &[
    "GROK_",
    "XAI_GROK_",
    "ANTHROPIC_",
    "OPENAI_",
    "OPENROUTER_",
    "AWS_",
    "AZURE_",
    "GOOGLE_",
];

/// Build an allowlisted environment for the Claude child.
///
/// Starts empty (callers use `env_clear`). Copies only allowlisted keys from
/// the current process env, plus optional `extra` (still subject to forbid).
pub fn build_scrubbed_env(extra: &[(&str, OsString)]) -> Vec<(OsString, OsString)> {
    let allow: HashSet<&str> = ALLOWLIST_KEYS.iter().copied().collect();
    let forbid: HashSet<&str> = FORBIDDEN_KEYS.iter().copied().collect();

    let mut out: Vec<(OsString, OsString)> = Vec::new();

    for (k, v) in std::env::vars_os() {
        let key = k.to_string_lossy();
        if !is_allowed_key(key.as_ref(), &allow, &forbid) {
            continue;
        }
        out.push((k, v));
    }

    for (k, v) in extra {
        if !is_allowed_key(k, &allow, &forbid) {
            continue;
        }
        out.retain(|(ek, _)| ek.as_os_str() != OsStr::new(*k));
        out.push((OsString::from(*k), v.clone()));
    }

    out
}

fn is_allowed_key(key: &str, allow: &HashSet<&str>, forbid: &HashSet<&str>) -> bool {
    if forbid.contains(key) {
        return false;
    }
    if FORBIDDEN_PREFIXES.iter().any(|p| key.starts_with(p)) {
        // CLAUDE_CONFIG_DIR is allowlisted explicitly; other CLAUDE_* stay out
        // unless in ALLOWLIST_KEYS.
        if allow.contains(key) {
            return true;
        }
        return false;
    }
    // Allow exact allowlist entries and LC_* locale vars (except if forbidden).
    if allow.contains(key) {
        return true;
    }
    if key.starts_with("LC_") {
        return true;
    }
    false
}

/// Apply allowlisted env to a `tokio::process::Command` (clear then set).
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
        // PWD is not on the default allowlist; cwd is set via Command::current_dir.
        let _ = cwd;
    }
    v
}

/// Test helper.
pub fn env_contains_key(env: &[(OsString, OsString)], key: &str) -> bool {
    env.iter().any(|(k, _)| k.as_os_str() == OsStr::new(key))
}

// Back-compat alias used by older tests.
pub const SCRUBBED_SECRET_KEYS: &[&str] = FORBIDDEN_KEYS;

pub fn env_contains_secret(env: &[(OsString, OsString)], secret_key: &str) -> bool {
    env_contains_key(env, secret_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowlist_only_and_forbids_secrets() {
        unsafe {
            std::env::set_var("ANTHROPIC_API_KEY", "sk-ant-secret");
            std::env::set_var("OPENAI_API_KEY", "sk-openai");
            std::env::set_var("XAI_API_KEY", "xai-secret");
            std::env::set_var("OPENROUTER_API_KEY", "or-secret");
            std::env::set_var("ZAI_API_KEY", "zai-secret");
            std::env::set_var("DASHSCOPE_API_KEY", "ds-secret");
            std::env::set_var("AWS_ACCESS_KEY_ID", "AKIAxxx");
            std::env::set_var("AWS_SECRET_ACCESS_KEY", "aws-secret");
            std::env::set_var("AWS_SESSION_TOKEN", "aws-sess");
            std::env::set_var("GOOGLE_APPLICATION_CREDENTIALS", "/tmp/creds.json");
            std::env::set_var("AZURE_CLIENT_SECRET", "az-secret");
            std::env::set_var("GITHUB_TOKEN", "ghp_xxx");
            std::env::set_var("SSH_AUTH_SOCK", "/tmp/ssh.sock");
            std::env::set_var("GROK_API_KEY", "grok-secret");
            std::env::set_var("RANDOM_INHERITED", "should-not-pass");
            std::env::set_var("PATH", "/usr/bin");
            std::env::set_var("HOME", "/tmp/test-home");
            std::env::set_var("LC_TIME", "en_US.UTF-8");
        }
        let env = build_scrubbed_env(&[]);
        for key in FORBIDDEN_KEYS {
            assert!(
                !env_contains_key(&env, key),
                "forbidden {key} must be absent"
            );
        }
        assert!(!env_contains_key(&env, "RANDOM_INHERITED"));
        assert!(env_contains_key(&env, "PATH"));
        assert!(env_contains_key(&env, "HOME"));
        assert!(env_contains_key(&env, "LC_TIME"));
        unsafe {
            for k in [
                "ANTHROPIC_API_KEY",
                "OPENAI_API_KEY",
                "XAI_API_KEY",
                "OPENROUTER_API_KEY",
                "ZAI_API_KEY",
                "DASHSCOPE_API_KEY",
                "AWS_ACCESS_KEY_ID",
                "AWS_SECRET_ACCESS_KEY",
                "AWS_SESSION_TOKEN",
                "GOOGLE_APPLICATION_CREDENTIALS",
                "AZURE_CLIENT_SECRET",
                "GITHUB_TOKEN",
                "SSH_AUTH_SOCK",
                "GROK_API_KEY",
                "RANDOM_INHERITED",
            ] {
                std::env::remove_var(k);
            }
        }
    }

    #[test]
    fn extra_cannot_reinject_api_key() {
        let env = build_scrubbed_env(&[("ANTHROPIC_API_KEY", OsString::from("evil"))]);
        assert!(!env_contains_key(&env, "ANTHROPIC_API_KEY"));
    }
}
