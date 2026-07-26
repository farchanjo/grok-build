//! Shell-owned external agent runtime abstraction.
//!
//! Native sessions keep using [`xai_grok_inference::InferenceActor`]. External
//! backends (Claude Agent CLI, …) implement [`ExternalAgentRuntime`] and are
//! selected through [`ExternalRuntimeRegistry`].
//!
//! PR5 provides the typed foundation and a fail-closed unavailable stub. PR6
//! wires the real Claude CLI process integration — this module must not spawn
//! processes or probe the official CLI.

mod envelope;
mod registry;
mod stub;
mod types;

pub use envelope::{
    EnvelopeValidationError, ExternalResultMetadata, ExternalRuntimeEnvelope,
    ExternalUsageMetadata, MAX_ENVELOPE_JSON_BYTES, MAX_SESSION_POINTER_LEN,
};
pub use registry::{ExternalRuntimeFactory, ExternalRuntimeRegistry, default_registry};
pub use stub::UnavailableExternalRuntime;
pub use types::{
    EXTERNAL_RUNTIME_UNAVAILABLE, EXTERNAL_RUNTIME_UNAVAILABLE_MESSAGE,
    ExternalRuntimeCapabilities, ExternalRuntimeError, ExternalRuntimeErrorKind,
    ExternalRuntimeStatus, ExternalRuntimeTurnEvent,
};

use crate::agent::execution_backend::ExternalAgentKind;
use async_trait::async_trait;

/// Shell-owned interface for an external agent execution backend.
///
/// Implementations must not leak secrets, argv, or raw NDJSON into durable
/// envelopes or normalized events.
#[async_trait]
pub trait ExternalAgentRuntime: Send + Sync {
    fn kind(&self) -> ExternalAgentKind;

    /// Probe runtime availability / version / capabilities without starting a turn.
    async fn probe(&self) -> Result<ExternalRuntimeCapabilities, ExternalRuntimeError>;

    /// Begin a new external session for the given workspace identity.
    async fn start(
        &self,
        request: ExternalStartRequest,
    ) -> Result<ExternalRuntimeEnvelope, ExternalRuntimeError>;

    /// Resume from a durable envelope (session pointer + observed capabilities).
    async fn resume(
        &self,
        envelope: &ExternalRuntimeEnvelope,
    ) -> Result<ExternalRuntimeEnvelope, ExternalRuntimeError>;

    /// Run one user turn; yields normalized events (never raw CLI NDJSON).
    async fn turn(
        &self,
        envelope: &ExternalRuntimeEnvelope,
        request: ExternalTurnRequest,
    ) -> Result<ExternalTurnOutcome, ExternalRuntimeError>;

    /// Cancel an in-flight turn if any.
    async fn cancel(&self, envelope: &ExternalRuntimeEnvelope) -> Result<(), ExternalRuntimeError>;

    /// Release runtime resources for this session.
    async fn shutdown(
        &self,
        envelope: &ExternalRuntimeEnvelope,
    ) -> Result<(), ExternalRuntimeError>;

    fn status(&self, envelope: Option<&ExternalRuntimeEnvelope>) -> ExternalRuntimeStatus;
}

/// Inputs for starting an external runtime session (no secrets).
#[derive(Debug, Clone)]
pub struct ExternalStartRequest {
    pub cwd: String,
    pub worktree_identity: Option<String>,
    pub selected_model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub token_budget: Option<u64>,
}

/// One user turn request into an external runtime.
#[derive(Debug, Clone)]
pub struct ExternalTurnRequest {
    pub prompt: String,
    pub selected_model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub token_budget: Option<u64>,
}

/// Normalized turn result from an external runtime.
#[derive(Debug, Clone)]
pub struct ExternalTurnOutcome {
    pub events: Vec<ExternalRuntimeTurnEvent>,
    pub envelope: ExternalRuntimeEnvelope,
    pub result: Option<ExternalResultMetadata>,
    pub usage: Option<ExternalUsageMetadata>,
}

/// Capability / UI descriptors for the model and provider pickers.
///
/// Claude Agent CLI is labeled experimental and is **not** selectable until
/// PR6 wires the real runtime (`selectable = false` by default).
pub mod capability_matrix {
    use crate::agent::execution_backend::{ExecutionBackend, ExternalAgentKind};

    /// When `true`, a Claude CLI catalog entry may appear as user-selectable.
    /// PR5 keeps this off; PR6 may flip it behind a feature flag.
    pub const CLAUDE_CLI_MODEL_SELECTABLE: bool = false;

    /// UI-facing capability row for an execution backend.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ExecutionCapabilityDescriptor {
        pub backend: ExecutionBackend,
        /// Short label for pickers (e.g. "Native inference", "Claude Agent CLI").
        pub label: &'static str,
        /// Longer description for experimental badges / help text.
        pub description: &'static str,
        pub experimental: bool,
        /// Whether the picker may offer a model that selects this backend.
        pub selectable: bool,
        /// Catalog peer id (`anthropic`, `xai`, …) for grouping — not a
        /// ModelProviderKind overload for external process identity.
        pub provider_peer_id: Option<&'static str>,
    }

    /// Static matrix used by the provider/model picker to label execution modes.
    pub fn descriptors() -> &'static [ExecutionCapabilityDescriptor] {
        &[
            ExecutionCapabilityDescriptor {
                backend: ExecutionBackend::NativeInference,
                label: "Native inference",
                description: "In-process HTTP model sampling (ApiBackend wire protocols).",
                experimental: false,
                selectable: true,
                provider_peer_id: None,
            },
            ExecutionCapabilityDescriptor {
                backend: ExecutionBackend::ExternalAgent(ExternalAgentKind::ClaudeCli),
                label: "Claude Agent CLI",
                description: "Experimental external Claude Agent CLI runtime (not the Anthropic HTTP API).",
                experimental: true,
                selectable: CLAUDE_CLI_MODEL_SELECTABLE,
                provider_peer_id: Some("anthropic"),
            },
        ]
    }

    /// Descriptor for a specific backend, if known.
    pub fn for_backend(
        backend: ExecutionBackend,
    ) -> Option<&'static ExecutionCapabilityDescriptor> {
        descriptors().iter().find(|d| d.backend == backend)
    }

    /// Experimental external backends that share the Anthropic provider peer
    /// in the UI but remain distinct from native Anthropic Messages API models.
    pub fn experimental_anthropic_peer_backends()
    -> impl Iterator<Item = &'static ExecutionCapabilityDescriptor> {
        descriptors()
            .iter()
            .filter(|d| d.experimental && d.provider_peer_id == Some("anthropic"))
    }

    /// Force `hidden` + `!user_selectable` on catalog entries whose execution
    /// backend is not yet selectable (PR5: Claude CLI). Idempotent.
    pub fn apply_catalog_visibility(
        catalog: &mut indexmap::IndexMap<String, crate::agent::config::ModelEntry>,
    ) {
        for entry in catalog.values_mut() {
            if !entry.info.execution_backend.is_external() {
                continue;
            }
            let selectable = for_backend(entry.info.execution_backend)
                .map(|d| d.selectable)
                .unwrap_or(false);
            if !selectable {
                entry.info.hidden = true;
                entry.info.user_selectable = false;
            }
        }
    }
}

#[cfg(test)]
mod pr5_foundation_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::execution_backend::{ExecutionBackend, ExternalAgentKind};

    #[tokio::test]
    async fn unavailable_stub_is_deterministic_non_auth() {
        let runtime = UnavailableExternalRuntime::new(ExternalAgentKind::ClaudeCli);
        let err = runtime.probe().await.expect_err("stub must fail closed");
        assert_eq!(err.kind, ExternalRuntimeErrorKind::Unavailable);
        assert!(!err.is_auth_error());
        assert_eq!(err.code(), EXTERNAL_RUNTIME_UNAVAILABLE);
        assert!(err.message.contains("unavailable in this build"));
    }

    #[tokio::test]
    async fn registry_resolves_claude_cli_to_unavailable_stub() {
        let registry = default_registry();
        let runtime = registry
            .create(ExternalAgentKind::ClaudeCli)
            .expect("factory registered");
        let err = runtime
            .turn(
                &ExternalRuntimeEnvelope::for_kind(ExternalAgentKind::ClaudeCli),
                ExternalTurnRequest {
                    prompt: "hi".into(),
                    selected_model: None,
                    reasoning_effort: None,
                    token_budget: None,
                },
            )
            .await
            .expect_err("stub turn fails closed");
        assert_eq!(err.kind, ExternalRuntimeErrorKind::Unavailable);
        assert!(!err.is_auth_error());
    }

    #[test]
    fn capability_matrix_labels_claude_cli_experimental_not_selectable() {
        let d = capability_matrix::for_backend(ExecutionBackend::ExternalAgent(
            ExternalAgentKind::ClaudeCli,
        ))
        .expect("descriptor");
        assert!(d.experimental);
        assert!(!d.selectable);
        assert_eq!(d.provider_peer_id, Some("anthropic"));
        assert!(d.label.contains("Claude"));
    }

    #[test]
    fn envelope_roundtrip_omits_secrets_and_codex_fields() {
        let env = ExternalRuntimeEnvelope {
            kind: ExternalAgentKind::ClaudeCli,
            session_pointer: Some("sess_abc".into()),
            observed_version: Some("1.2.3".into()),
            capabilities: vec!["tools".into()],
            selected_model: Some("claude-sonnet-5".into()),
            reasoning_effort: Some("high".into()),
            token_budget: Some(100_000),
            cwd: Some("/tmp/ws".into()),
            worktree_identity: Some("wt-1".into()),
            result: Some(ExternalResultMetadata {
                status: "ok".into(),
                stop_reason: Some("end_turn".into()),
            }),
            usage: Some(ExternalUsageMetadata {
                input_tokens: Some(10),
                output_tokens: Some(20),
                total_tokens: Some(30),
            }),
        };
        let json = serde_json::to_string(&env).unwrap();
        assert!(!json.contains("codex"));
        assert!(!json.contains("argv"));
        assert!(!json.contains("api_key"));
        assert!(!json.contains("NDJSON"));
        let back: ExternalRuntimeEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(back.kind, ExternalAgentKind::ClaudeCli);
        assert_eq!(back.session_pointer.as_deref(), Some("sess_abc"));
        assert_eq!(back.selected_model.as_deref(), Some("claude-sonnet-5"));
    }
}
