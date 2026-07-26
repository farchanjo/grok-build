//! Typed execution backend for session turns.
//!
//! Distinguishes **native HTTP inference** (Grok's `InferenceActor` path) from
//! **external agent runtimes** (for example Claude Agent CLI). This is not an
//! HTTP wire protocol selector — that remains [`crate::inference::ApiBackend`].
//!
//! Intentionally **not** overloaded onto:
//! - [`super::model_providers::ModelProviderKind`] (provider identity)
//! - model `agent_type` / harness name
//! - provider preset `is_agent`
//! - provider `command` argv
//! - the `x-grok-native-agent-provider` header

use serde::{Deserialize, Serialize};

/// Kind of external agent runtime integrated by the shell.
///
/// Expand this enum when additional external agents are added. Do not reuse
/// Codex-specific field names (`codex_thread_id`, `codex_provider`, …).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalAgentKind {
    /// Anthropic Claude Agent CLI process runtime (implemented in a later PR).
    ClaudeCli,
}

impl ExternalAgentKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ClaudeCli => "claude_cli",
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::ClaudeCli => "Claude Agent (CLI, Experimental)",
        }
    }

    /// Provider peer identity for catalog/UI grouping. Claude CLI is an
    /// experimental execution mode of the Anthropic peer, not a separate
    /// ModelProviderKind.
    pub const fn provider_peer_id(self) -> &'static str {
        match self {
            Self::ClaudeCli => "anthropic",
        }
    }
}

impl std::fmt::Display for ExternalAgentKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How a session executes model turns.
///
/// Defaults to [`Self::NativeInference`] for all existing configs, catalogs,
/// and session summaries (serde default + [`Default`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionBackend {
    /// In-process HTTP sampling via `InferenceActor` / `ApiBackend`.
    #[default]
    NativeInference,
    /// Shell-owned external agent runtime (process or other out-of-process loop).
    ExternalAgent(ExternalAgentKind),
}

impl ExecutionBackend {
    /// `true` for the default native HTTP path. Also used as
    /// `skip_serializing_if` so old-shaped configs stay compact.
    pub fn is_native(&self) -> bool {
        matches!(self, Self::NativeInference)
    }

    pub fn is_external(&self) -> bool {
        !self.is_native()
    }

    pub fn external_kind(self) -> Option<ExternalAgentKind> {
        match self {
            Self::NativeInference => None,
            Self::ExternalAgent(kind) => Some(kind),
        }
    }

    /// Stable mode identity for cross-mode switch guards and persistence.
    ///
    /// NativeInference is one family; each external kind is its own family.
    /// Same-mode model switches stay on the existing agent-type / sampling path.
    pub const fn mode_key(self) -> &'static str {
        match self {
            Self::NativeInference => "native_inference",
            Self::ExternalAgent(kind) => kind.as_str(),
        }
    }

    /// `true` when switching between native HTTP inference and an external
    /// agent (or between distinct external kinds).
    ///
    /// External kinds compare by [`PartialEq`] (`a != b`) so adding a new
    /// [`ExternalAgentKind`] variant automatically participates without
    /// updating this match.
    pub fn is_cross_mode_with(self, other: Self) -> bool {
        match (self, other) {
            (Self::NativeInference, Self::NativeInference) => false,
            (Self::ExternalAgent(a), Self::ExternalAgent(b)) => a != b,
            _ => true,
        }
    }

    pub const fn as_config_str(self) -> &'static str {
        self.mode_key()
    }
}

impl std::fmt::Display for ExecutionBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.mode_key())
    }
}

/// Error code when a model switch crosses execution modes after the first
/// established user turn. Clients should start a new session.
pub const MODEL_SWITCH_CROSS_EXECUTION_MODE: &str = "MODEL_SWITCH_CROSS_EXECUTION_MODE";

/// Structured error payload for cross-mode model switch rejection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelSwitchCrossExecutionModeError {
    pub code: String,
    pub active_execution_backend: String,
    pub required_execution_backend: String,
    pub model_id: String,
    /// Always `"start_new_session"` for client remediation.
    pub suggestion: String,
}

impl ModelSwitchCrossExecutionModeError {
    pub fn new(
        active: ExecutionBackend,
        required: ExecutionBackend,
        model_id: impl Into<String>,
    ) -> Self {
        Self {
            code: MODEL_SWITCH_CROSS_EXECUTION_MODE.to_owned(),
            active_execution_backend: active.mode_key().to_owned(),
            required_execution_backend: required.mode_key().to_owned(),
            model_id: model_id.into(),
            suggestion: "start_new_session".to_owned(),
        }
    }

    pub fn into_acp_error(self) -> agent_client_protocol::Error {
        let message = format!(
            "Cannot switch model to '{}': session is using execution mode '{}' but the target requires '{}'. \
             Start a new session to change execution mode.",
            self.model_id, self.active_execution_backend, self.required_execution_backend,
        );
        agent_client_protocol::Error::new(
            agent_client_protocol::ErrorCode::InvalidRequest.into(),
            message,
        )
        .data(serde_json::to_value(&self).ok())
    }

    pub fn from_acp_error(err: &agent_client_protocol::Error) -> Option<Self> {
        let data = err.data.as_ref()?;
        let code = data.get("code")?.as_str()?;
        if code != MODEL_SWITCH_CROSS_EXECUTION_MODE {
            return None;
        }
        serde_json::from_value(data.clone()).ok()
    }

    pub fn user_message(&self) -> String {
        format!(
            "Cannot switch to '{}' — this session uses '{}' but that model requires '{}'. \
             Start /new to change execution mode.",
            self.model_id, self.active_execution_backend, self.required_execution_backend,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_native_inference() {
        assert_eq!(
            ExecutionBackend::default(),
            ExecutionBackend::NativeInference
        );
        let json = serde_json::to_string(&ExecutionBackend::default()).unwrap();
        assert_eq!(json, "\"native_inference\"");
    }

    #[test]
    fn old_summary_missing_field_deserializes_native() {
        // Simulate an old Summary fragment without execution_backend.
        #[derive(Deserialize)]
        struct Frag {
            #[serde(default)]
            execution_backend: ExecutionBackend,
        }
        let frag: Frag = serde_json::from_str("{}").unwrap();
        assert!(frag.execution_backend.is_native());
    }

    #[test]
    fn external_claude_cli_roundtrip() {
        let backend = ExecutionBackend::ExternalAgent(ExternalAgentKind::ClaudeCli);
        let json = serde_json::to_string(&backend).unwrap();
        assert!(json.contains("external_agent"));
        assert!(json.contains("claude_cli"));
        let back: ExecutionBackend = serde_json::from_str(&json).unwrap();
        assert_eq!(back, backend);
    }

    #[test]
    fn cross_mode_detection() {
        let native = ExecutionBackend::NativeInference;
        let claude = ExecutionBackend::ExternalAgent(ExternalAgentKind::ClaudeCli);
        assert!(native.is_cross_mode_with(claude));
        assert!(claude.is_cross_mode_with(native));
        assert!(!native.is_cross_mode_with(native));
        assert!(!claude.is_cross_mode_with(claude));
    }

    /// Locks the external-kind equality rule used by [`ExecutionBackend::is_cross_mode_with`].
    /// When a second production kind is added, same-kind pairs stay same-mode and
    /// distinct kinds become cross-mode via `a != b` (no match-arm update needed).
    #[test]
    fn external_kind_equality_is_cross_mode_source_of_truth() {
        let a = ExternalAgentKind::ClaudeCli;
        let b = ExternalAgentKind::ClaudeCli;
        assert_eq!(a, b);
        assert!(
            !ExecutionBackend::ExternalAgent(a)
                .is_cross_mode_with(ExecutionBackend::ExternalAgent(b))
        );
        // Future kinds: `a != b` is the only check — document the contract.
        fn kinds_differ(x: ExternalAgentKind, y: ExternalAgentKind) -> bool {
            x != y
        }
        assert!(!kinds_differ(a, b));
    }

    #[test]
    fn no_codex_field_names_in_serde() {
        let backend = ExecutionBackend::ExternalAgent(ExternalAgentKind::ClaudeCli);
        let json = serde_json::to_string(&backend).unwrap();
        assert!(!json.contains("codex"));
        assert!(!json.contains("thread_id"));
    }

    #[test]
    fn claude_cli_provider_peer_is_anthropic() {
        assert_eq!(ExternalAgentKind::ClaudeCli.provider_peer_id(), "anthropic");
    }
}
