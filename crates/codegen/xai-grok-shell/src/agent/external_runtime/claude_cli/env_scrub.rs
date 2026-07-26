//! Strict child environment allowlist for Claude Agent CLI.
//!
//! `env_clear` then re-apply only the documented minimal set needed for the
//! official binary and subscription auth. Never forwards provider/Grok/cloud
//! credentials or arbitrary inherited env.
//!
//! Proxy values are preserved only when they can be sanitized to strip URL
//! userinfo (`user:pass@host`). Malformed or non-Unicode proxy values are
//! dropped (fail closed). The allowlist itself is not weakened.

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
    // Optional TLS / proxy (values sanitized; credentials stripped)
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

/// Proxy-related keys whose values must be sanitized (strip URL userinfo).
const PROXY_KEYS: &[&str] = &[
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
/// Proxy values are sanitized (userinfo stripped) or dropped.
pub fn build_scrubbed_env(extra: &[(&str, OsString)]) -> Vec<(OsString, OsString)> {
    let allow: HashSet<&str> = ALLOWLIST_KEYS.iter().copied().collect();
    let forbid: HashSet<&str> = FORBIDDEN_KEYS.iter().copied().collect();

    let mut out: Vec<(OsString, OsString)> = Vec::new();

    for (k, v) in std::env::vars_os() {
        let key = k.to_string_lossy();
        if !is_allowed_key(key.as_ref(), &allow, &forbid) {
            continue;
        }
        if let Some(sanitized) = sanitize_env_value(key.as_ref(), &v) {
            out.push((k, sanitized));
        }
    }

    for (k, v) in extra {
        if !is_allowed_key(k, &allow, &forbid) {
            continue;
        }
        out.retain(|(ek, _)| ek.as_os_str() != OsStr::new(*k));
        if let Some(sanitized) = sanitize_env_value(k, v) {
            out.push((OsString::from(*k), sanitized));
        }
    }

    out
}

fn is_proxy_key(key: &str) -> bool {
    PROXY_KEYS.iter().any(|k| *k == key)
}

/// Sanitize an allowlisted value. Proxy keys: strip URL userinfo, drop on
/// non-Unicode or unparseable forms. Non-proxy keys: pass through as OsString.
fn sanitize_env_value(key: &str, value: &OsString) -> Option<OsString> {
    if !is_proxy_key(key) {
        return Some(value.clone());
    }
    let s = value.to_str()?;
    sanitize_proxy_value(key, s).map(OsString::from)
}

/// Strip credentials from a proxy URL value. `NO_PROXY` / `no_proxy` are host
/// lists (not URLs) and are passed through only when valid Unicode without
/// embedded credentials of the form `user:pass@`.
///
/// Returns `None` to drop the variable (fail closed).
pub fn sanitize_proxy_value(key: &str, value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Host-list forms (NO_PROXY): reject obvious userinfo, otherwise pass through.
    if key.eq_ignore_ascii_case("NO_PROXY") {
        if trimmed.contains('@') {
            // e.g. user:pass@host in a no_proxy list is malformed / credential-like.
            return None;
        }
        return Some(trimmed.to_owned());
    }
    // URL-like proxy values. Accept with or without scheme.
    // Forms: http://user:pass@host:port, user:pass@host:port, host:port, http://host
    let after_scheme = if let Some((scheme, rest)) = trimmed.split_once("://") {
        if scheme.is_empty()
            || !scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
        {
            return None;
        }
        Some((scheme, rest))
    } else {
        None
    };

    let (scheme_prefix, authority_and_path) = match after_scheme {
        Some((scheme, rest)) => (Some(scheme), rest),
        None => (None, trimmed),
    };

    // Authority is up to first '/' or '?' (path/query rare for proxy vars).
    let (authority, path_suffix) = match authority_and_path.find(['/', '?']) {
        Some(i) => (&authority_and_path[..i], &authority_and_path[i..]),
        None => (authority_and_path, ""),
    };

    if authority.is_empty() {
        return None;
    }

    // Strip userinfo: everything before the last '@' in authority is credentials.
    let host_port = if let Some(at) = authority.rfind('@') {
        let host = &authority[at + 1..];
        if host.is_empty() {
            return None;
        }
        host
    } else {
        authority
    };

    // Basic host:port sanity — no whitespace, no second '@'.
    if host_port.chars().any(|c| c.is_whitespace()) || host_port.contains('@') {
        return None;
    }

    let mut out = String::new();
    if let Some(scheme) = scheme_prefix {
        out.push_str(scheme);
        out.push_str("://");
    }
    out.push_str(host_port);
    out.push_str(path_suffix);
    Some(out)
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

    #[test]
    fn sanitize_proxy_strips_userinfo() {
        assert_eq!(
            sanitize_proxy_value("HTTPS_PROXY", "http://user:s3cret@proxy.example:8080"),
            Some("http://proxy.example:8080".into())
        );
        assert_eq!(
            sanitize_proxy_value("HTTP_PROXY", "user:pass@10.0.0.1:3128"),
            Some("10.0.0.1:3128".into())
        );
        assert_eq!(
            sanitize_proxy_value("https_proxy", "https://proxy.local"),
            Some("https://proxy.local".into())
        );
        assert_eq!(
            sanitize_proxy_value("ALL_PROXY", "socks5://alice:bob@socks:1080"),
            Some("socks5://socks:1080".into())
        );
    }

    #[test]
    fn sanitize_proxy_drops_malformed() {
        assert_eq!(sanitize_proxy_value("HTTP_PROXY", ""), None);
        assert_eq!(sanitize_proxy_value("HTTP_PROXY", "   "), None);
        assert_eq!(sanitize_proxy_value("HTTP_PROXY", "http://user@"), None);
        assert_eq!(sanitize_proxy_value("NO_PROXY", "user:pass@host"), None);
        assert_eq!(
            sanitize_proxy_value("NO_PROXY", "localhost,127.0.0.1"),
            Some("localhost,127.0.0.1".into())
        );
    }

    #[test]
    fn build_scrubbed_env_sanitizes_proxy_and_drops_secrets_in_value() {
        unsafe {
            std::env::set_var("HTTPS_PROXY", "http://proxyuser:proxypass@corp-proxy:8080");
            std::env::set_var("NO_PROXY", "localhost");
        }
        let env = build_scrubbed_env(&[]);
        let https = env
            .iter()
            .find(|(k, _)| k.as_os_str() == OsStr::new("HTTPS_PROXY"))
            .map(|(_, v)| v.to_string_lossy().into_owned());
        assert_eq!(https.as_deref(), Some("http://corp-proxy:8080"));
        assert!(
            https
                .as_ref()
                .is_none_or(|v| !v.contains("proxypass") && !v.contains("proxyuser")),
            "proxy credentials must not be forwarded: {https:?}"
        );
        unsafe {
            std::env::remove_var("HTTPS_PROXY");
            std::env::remove_var("NO_PROXY");
        }
    }
}
