//! Durable external-runtime envelope stored on session summaries.
//!
//! Keeps secrets, argv, and raw NDJSON out of persistence. Uses generic field
//! names — never `codex_thread_id` / `codex_provider` / `codex_sandbox`.

use crate::agent::execution_backend::ExternalAgentKind;
use serde::{Deserialize, Serialize};

/// Durable external runtime state attached to a session summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalRuntimeEnvelope {
    pub kind: ExternalAgentKind,
    /// Opaque resume pointer owned by the external runtime (session id, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_pointer: Option<String>,
    /// Observed runtime version from the last successful probe/start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_version: Option<String>,
    /// Capability strings reported by the runtime (normalized, not raw).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
    /// Model selected for the external runtime (may differ from catalog id).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_model: Option<String>,
    /// Effort / thinking level selected for the runtime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
    /// Optional token or cost budget observed/selected for the runtime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_budget: Option<u64>,
    /// Workspace cwd identity at last external turn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// Worktree identity (label/path) when the session is worktree-isolated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_identity: Option<String>,
    /// Normalized terminal result metadata from the last external turn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<ExternalResultMetadata>,
    /// Normalized usage from the last external turn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<ExternalUsageMetadata>,
}

impl ExternalRuntimeEnvelope {
    pub fn for_kind(kind: ExternalAgentKind) -> Self {
        Self {
            kind,
            session_pointer: None,
            observed_version: None,
            capabilities: Vec::new(),
            selected_model: None,
            reasoning_effort: None,
            token_budget: None,
            cwd: None,
            worktree_identity: None,
            result: None,
            usage: None,
        }
    }
}

/// Normalized terminal result (no raw logs).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalResultMetadata {
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
}

/// Normalized usage counters from an external runtime turn.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalUsageMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<u64>,
}
