//! Provider status for Claude Agent CLI (PR7).
//!
//! Binary readiness, subscription auth status, integration readiness, and
//! permission-bridge readiness are **distinct** from Anthropic API-key status.
//! Subscription auth is only queried when gates are open. No login/logout
//! flows are implemented here.

use serde::{Deserialize, Serialize};

use super::auth::ClaudeCliAuthStatus;
use super::gates;

/// Composite readiness for UI / provider cards.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeCliProviderStatus {
    /// Compile feature + runtime env opt-in.
    pub gates_open: bool,
    /// Official binary discovered and version probe succeeded.
    pub binary_ready: bool,
    pub binary_version: Option<String>,
    pub binary_detail: Option<String>,
    /// Subscription auth via `claude auth status` (gated; not API key).
    pub auth_ready: bool,
    pub auth_summary: Option<String>,
    /// Integration path ready (gates + binary + not faulted).
    pub integration_ready: bool,
    /// Permission bridge socket/broker ready for the session.
    pub permission_bridge_ready: bool,
    /// Explicit: this is **not** Anthropic API-key readiness.
    pub anthropic_api_key_status: ApiKeyStatusNote,
    /// Human summary for the provider card.
    pub summary: String,
}

/// Explicit note that API-key status is out of band for this integration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiKeyStatusNote {
    /// Claude CLI subscription path does not use ANTHROPIC_API_KEY.
    NotApplicableSubscriptionOnly,
}

impl Default for ApiKeyStatusNote {
    fn default() -> Self {
        Self::NotApplicableSubscriptionOnly
    }
}

/// Build status from probe + optional auth + bridge flag.
pub fn build_status(
    binary_ready: bool,
    binary_version: Option<String>,
    binary_detail: Option<String>,
    auth: Option<&ClaudeCliAuthStatus>,
    permission_bridge_ready: bool,
) -> ClaudeCliProviderStatus {
    let gates_open = gates::claude_cli_both_gates_open();
    let (auth_ready, auth_summary) = match auth {
        Some(a) => (a.logged_in, Some(a.summary.clone())),
        None => (false, None),
    };
    let integration_ready = gates_open && binary_ready;
    let summary = if !gates::claude_cli_feature_compiled() {
        "Claude Agent CLI runtime not compiled into this build".to_owned()
    } else if !gates_open {
        format!(
            "Claude Agent CLI disabled (set {}=1 to opt in)",
            gates::CLAUDE_CLI_ENV_OPT_IN
        )
    } else if !binary_ready {
        format!(
            "Claude binary not ready: {}",
            binary_detail.as_deref().unwrap_or("probe failed")
        )
    } else if auth.is_some() && !auth_ready {
        "Claude CLI binary ready; subscription not logged in".to_owned()
    } else if !permission_bridge_ready {
        "Claude CLI ready; permission bridge not started for this session".to_owned()
    } else {
        "Claude Agent CLI ready (subscription; no API key)".to_owned()
    };

    ClaudeCliProviderStatus {
        gates_open,
        binary_ready,
        binary_version,
        binary_detail,
        auth_ready,
        auth_summary,
        integration_ready,
        permission_bridge_ready,
        anthropic_api_key_status: ApiKeyStatusNote::NotApplicableSubscriptionOnly,
        summary,
    }
}

/// Auth status is only delegated when both gates are open.
pub fn may_query_subscription_auth() -> bool {
    gates::claude_cli_both_gates_open()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_key_status_is_not_applicable() {
        let st = build_status(true, Some("2.1.250".into()), None, None, true);
        assert_eq!(
            st.anthropic_api_key_status,
            ApiKeyStatusNote::NotApplicableSubscriptionOnly
        );
        assert!(!st.summary.to_ascii_lowercase().contains("api key ready"));
        assert!(
            st.summary.contains("subscription") || st.summary.contains("opt in") || !st.gates_open
        );
    }

    #[test]
    fn binary_and_auth_and_bridge_are_distinct_fields() {
        let st = build_status(
            true,
            Some("2.1.250".into()),
            None,
            Some(&ClaudeCliAuthStatus {
                logged_in: false,
                account_label: None,
                summary: "not logged in".into(),
            }),
            false,
        );
        assert!(st.binary_ready);
        assert!(!st.auth_ready);
        assert!(!st.permission_bridge_ready);
    }
}
