//! Subscription auth status via official `claude auth status` (JSON by default).
//!
//! Only when gates are enabled. Bounded parse. Never logs/persists raw auth
//! output or tokens. Does not implement token extraction or read credential
//! files / keychains.

use std::path::Path;
use std::time::Duration;

use super::process::{self, ProbeCommandResult};

/// Timeout for `claude auth status`.
pub const AUTH_STATUS_TIMEOUT: Duration = Duration::from_secs(8);
/// Cap on captured auth status stdout/stderr.
pub const AUTH_STATUS_OUTPUT_CAP: usize = 4 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeCliAuthStatus {
    /// Whether the official CLI reports a logged-in subscription session.
    pub logged_in: bool,
    /// Redacted account label (email domain only or generic).
    pub account_label: Option<String>,
    /// Human status for UI (never raw JSON).
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaudeCliAuthError {
    Timeout,
    ProbeFailed { detail: String },
    Unparseable,
}

impl std::fmt::Display for ClaudeCliAuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Timeout => write!(f, "Claude CLI auth status timed out"),
            Self::ProbeFailed { detail } => {
                write!(f, "Claude CLI auth status failed: {detail}")
            }
            Self::Unparseable => write!(f, "Claude CLI auth status returned unparseable output"),
        }
    }
}

impl std::error::Error for ClaudeCliAuthError {}

/// Run `claude auth status` (JSON default; no unsupported `--json` flag).
///
/// Exit code 0 ⇒ logged in; 1 ⇒ not logged in (still a successful probe).
/// Never returns raw stdout to callers beyond redacted fields.
pub async fn query_auth_status(
    executable: &Path,
) -> Result<ClaudeCliAuthStatus, ClaudeCliAuthError> {
    let result = process::run_probe_command(
        executable,
        &["auth", "status"],
        AUTH_STATUS_TIMEOUT,
        AUTH_STATUS_OUTPUT_CAP,
    )
    .await
    .map_err(|e| match e {
        process::ProbeError::Timeout => ClaudeCliAuthError::Timeout,
        process::ProbeError::Spawn(m)
        | process::ProbeError::Io(m)
        | process::ProbeError::ProcessGroup(m) => ClaudeCliAuthError::ProbeFailed { detail: m },
    })?;

    parse_auth_status_output(&result)
}

/// Parse bounded auth status JSON. Never retains tokens.
pub fn parse_auth_status_output(
    result: &ProbeCommandResult,
) -> Result<ClaudeCliAuthStatus, ClaudeCliAuthError> {
    let raw = result.stdout.trim();
    // Prefer JSON object parse; fall back to exit-code only.
    if raw.is_empty() {
        return Ok(ClaudeCliAuthStatus {
            logged_in: result.success && result.exit_code == Some(0),
            account_label: None,
            summary: if result.success {
                "Claude CLI reports logged in (no details)".into()
            } else {
                "Claude CLI reports not logged in".into()
            },
        });
    }

    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|_| ClaudeCliAuthError::Unparseable)?;

    // Never look for accessToken / refreshToken / apiKey fields as values to
    // return — only structural logged-in flags and redacted identity.
    let logged_in = value
        .get("loggedIn")
        .or_else(|| value.get("logged_in"))
        .or_else(|| value.get("authenticated"))
        .and_then(|v| v.as_bool())
        .unwrap_or(result.exit_code == Some(0));

    let account_label = value
        .get("email")
        .or_else(|| value.get("account"))
        .or_else(|| value.pointer("/account/email"))
        .and_then(|v| v.as_str())
        .map(redact_email_or_label);

    // Ensure we did not accidentally capture a token-shaped string as label.
    let account_label = account_label.filter(|s| !looks_like_secret(s));

    let summary = if logged_in {
        match &account_label {
            Some(label) => format!("Claude subscription logged in ({label})"),
            None => "Claude subscription logged in".into(),
        }
    } else {
        "Claude CLI not logged in (run `claude auth login` in a terminal)".into()
    };

    Ok(ClaudeCliAuthStatus {
        logged_in,
        account_label,
        summary,
    })
}

fn redact_email_or_label(raw: &str) -> String {
    let t = raw.trim();
    if let Some((user, domain)) = t.split_once('@') {
        let u = if user.is_empty() {
            "*"
        } else {
            &user[..user.chars().next().map(|c| c.len_utf8()).unwrap_or(1)]
        };
        format!("{u}***@{domain}")
    } else if t.len() > 12 {
        format!("{}…", &t[..8])
    } else {
        t.to_owned()
    }
}

fn looks_like_secret(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    lower.contains("sk-")
        || lower.contains("token")
        || lower.contains("bearer")
        || (s.len() > 40
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_logged_in_json() {
        let result = ProbeCommandResult {
            stdout: r#"{"loggedIn":true,"email":"alice@example.com"}"#.into(),
            stderr: String::new(),
            success: true,
            exit_code: Some(0),
        };
        let st = parse_auth_status_output(&result).unwrap();
        assert!(st.logged_in);
        assert_eq!(st.account_label.as_deref(), Some("a***@example.com"));
        assert!(!st.summary.contains("alice@"));
    }

    #[test]
    fn parses_logged_out() {
        let result = ProbeCommandResult {
            stdout: r#"{"loggedIn":false}"#.into(),
            stderr: String::new(),
            success: false,
            exit_code: Some(1),
        };
        let st = parse_auth_status_output(&result).unwrap();
        assert!(!st.logged_in);
    }

    #[test]
    fn never_returns_token_shaped_label() {
        let result = ProbeCommandResult {
            stdout: r#"{"loggedIn":true,"email":"sk-ant-api03-supersecrettokenvaluehere"}"#.into(),
            stderr: String::new(),
            success: true,
            exit_code: Some(0),
        };
        let st = parse_auth_status_output(&result).unwrap();
        assert!(st.account_label.is_none() || !st.account_label.as_ref().unwrap().contains("sk-"));
    }
}
