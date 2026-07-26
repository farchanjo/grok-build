//! Shared types for external agent runtime probing, status, and errors.

use crate::agent::execution_backend::ExternalAgentKind;
use serde::{Deserialize, Serialize};

/// Stable error code for fail-closed unavailable external runtimes.
pub const EXTERNAL_RUNTIME_UNAVAILABLE: &str = "EXTERNAL_RUNTIME_UNAVAILABLE";

/// Deterministic user-facing message for the PR5 unavailable stub.
pub const EXTERNAL_RUNTIME_UNAVAILABLE_MESSAGE: &str =
    "Claude Agent CLI runtime is unavailable in this build";

/// Classification of external runtime failures. Auth errors are distinct so
/// callers do not trigger OIDC / API-key recovery for stub unavailability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalRuntimeErrorKind {
    /// Runtime not compiled / not registered / intentionally disabled.
    Unavailable,
    /// Process or transport failure (PR6+).
    Transport,
    /// Authentication / credential failure against the external runtime.
    Auth,
    /// Invalid request or envelope.
    InvalidRequest,
    /// Cancelled by the host.
    Cancelled,
    /// Other non-auth failure.
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalRuntimeError {
    pub kind: ExternalRuntimeErrorKind,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_kind: Option<ExternalAgentKind>,
}

impl ExternalRuntimeError {
    pub fn unavailable(agent_kind: ExternalAgentKind) -> Self {
        Self {
            kind: ExternalRuntimeErrorKind::Unavailable,
            message: match agent_kind {
                ExternalAgentKind::ClaudeCli => EXTERNAL_RUNTIME_UNAVAILABLE_MESSAGE.to_owned(),
            },
            agent_kind: Some(agent_kind),
        }
    }

    pub fn code(&self) -> &'static str {
        match self.kind {
            ExternalRuntimeErrorKind::Unavailable => EXTERNAL_RUNTIME_UNAVAILABLE,
            ExternalRuntimeErrorKind::Transport => "EXTERNAL_RUNTIME_TRANSPORT",
            ExternalRuntimeErrorKind::Auth => "EXTERNAL_RUNTIME_AUTH",
            ExternalRuntimeErrorKind::InvalidRequest => "EXTERNAL_RUNTIME_INVALID_REQUEST",
            ExternalRuntimeErrorKind::Cancelled => "EXTERNAL_RUNTIME_CANCELLED",
            ExternalRuntimeErrorKind::Other => "EXTERNAL_RUNTIME_OTHER",
        }
    }

    pub fn is_auth_error(&self) -> bool {
        matches!(self.kind, ExternalRuntimeErrorKind::Auth)
    }

    /// ACP error for turn/model paths.
    ///
    /// Intentional unavailability (`Unavailable`) and other client-facing
    /// rejections map to **InvalidRequest** with `EXTERNAL_RUNTIME_UNAVAILABLE`
    /// (or the matching code) so the TUI does not treat them as infra pauses.
    /// Transport/other failures stay InternalError. Auth never masquerades as
    /// unavailability (`authError` is explicit in data).
    pub fn into_acp_error(self) -> agent_client_protocol::Error {
        let code = match self.kind {
            ExternalRuntimeErrorKind::Unavailable
            | ExternalRuntimeErrorKind::InvalidRequest
            | ExternalRuntimeErrorKind::Cancelled
            | ExternalRuntimeErrorKind::Auth => agent_client_protocol::ErrorCode::InvalidRequest,
            ExternalRuntimeErrorKind::Transport | ExternalRuntimeErrorKind::Other => {
                agent_client_protocol::ErrorCode::InternalError
            }
        };
        let data = serde_json::json!({
            "code": self.code(),
            "kind": self.kind,
            "agentKind": self.agent_kind.map(|k| k.as_str()),
            "authError": self.is_auth_error(),
        });
        agent_client_protocol::Error::new(code.into(), self.message).data(data)
    }
}

impl std::fmt::Display for ExternalRuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code(), self.message)
    }
}

impl std::error::Error for ExternalRuntimeError {}

/// Observed capabilities after a successful probe (PR6 fills these in).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalRuntimeCapabilities {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<String>,
}

/// Lifecycle status of an external runtime binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalRuntimeStatus {
    Idle,
    Running,
    Unavailable,
    Faulted,
}

/// Normalized event stream items (never raw NDJSON).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExternalRuntimeTurnEvent {
    TextDelta {
        text: String,
    },
    ToolCall {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        summary: Option<String>,
    },
    Status {
        message: String,
    },
    Error {
        message: String,
    },
}
