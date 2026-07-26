//! Shell-owned external agent runtime abstraction.
//!
//! Native sessions keep using [`xai_grok_inference::InferenceActor`]. External
//! backends (Claude Agent CLI, …) implement [`ExternalAgentRuntime`] and are
//! selected through [`ExternalRuntimeRegistry`].
//!
//! PR5: typed foundation + fail-closed unavailable stub.
//! PR6: optional Claude CLI process integration behind `claude-cli-runtime`
//! feature **and** runtime opt-in ([`gates`]).

mod envelope;
pub mod gates;
pub mod probe_cache;
mod registry;
mod stub;
mod types;

#[cfg(feature = "claude-cli-runtime")]
pub mod claude_cli;

/// Schedule a bounded Claude CLI probe when compile+env gates are open.
/// No-op without the feature. Safe for provider refresh / catalog bootstrap.
pub async fn bootstrap_claude_cli_probe_if_gated() {
    #[cfg(feature = "claude-cli-runtime")]
    {
        claude_cli::runtime::bootstrap_probe_if_gated().await;
    }
    #[cfg(not(feature = "claude-cli-runtime"))]
    {
        // Feature off: leave probe cache empty (catalog stays unselectable).
    }
}

pub use envelope::{
    EnvelopeValidationError, ExternalResultMetadata, ExternalRuntimeEnvelope,
    ExternalUsageMetadata, MAX_ENVELOPE_JSON_BYTES, MAX_SESSION_POINTER_LEN,
};
pub use registry::{
    ExternalRuntimeFactory, ExternalRuntimeRegistry, ExternalRuntimeSessionContext,
    default_registry,
};
pub use stub::UnavailableExternalRuntime;
pub use types::{
    EXTERNAL_RUNTIME_UNAVAILABLE, EXTERNAL_RUNTIME_UNAVAILABLE_MESSAGE,
    ExternalRuntimeCapabilities, ExternalRuntimeError, ExternalRuntimeErrorKind,
    ExternalRuntimeStatus, ExternalRuntimeTurnEvent,
};

use crate::agent::execution_backend::ExternalAgentKind;
use async_trait::async_trait;
use std::sync::Arc;

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

/// Session-retained external runtime instance (one per SessionActor).
///
/// Preflight and turn reuse the same Arc so permission bridge temp dirs,
/// optional persistent Claude children, and cancel tokens stay coherent.
#[derive(Clone)]
pub struct RetainedExternalAgentRuntime {
    pub kind: ExternalAgentKind,
    /// Effective capability mode key snapshotted at create (after plan>yolo
    /// precedence). Mode changes force recreate + shutdown of the prior Arc.
    pub effective_mode: String,
    pub runtime: Arc<dyn ExternalAgentRuntime>,
}

impl RetainedExternalAgentRuntime {
    pub fn new(
        kind: ExternalAgentKind,
        effective_mode: impl Into<String>,
        runtime: Arc<dyn ExternalAgentRuntime>,
    ) -> Self {
        Self {
            kind,
            effective_mode: effective_mode.into(),
            runtime,
        }
    }
}

/// Capability / UI descriptors for the model and provider pickers.
///
/// Claude Agent CLI is labeled experimental. Selectable only when the compile
/// feature, runtime opt-in, **and** a successful binary probe all pass.
pub mod capability_matrix {
    use crate::agent::execution_backend::{ExecutionBackend, ExternalAgentKind};
    use crate::agent::external_runtime::gates;

    /// UI label under the Anthropic provider card.
    pub const CLAUDE_CLI_UI_LABEL: &str = "Claude Agent (CLI, Experimental)";

    /// Limitations blurb for pickers / status.
    pub const CLAUDE_CLI_UI_LIMITATIONS: &str = "\
Experimental subscription-backed Claude Agent CLI. Claude owns auth and tools; \
Grok owns the permission broker, outer process/sandbox, and UI. No Grok tool \
loop, compaction, memory, goals, hooks, checkpoints, or workflow accounting. \
No API keys. No bypassPermissions. Session-scoped runtime reuse across turns; \
persistent multi-turn child when the binary advertises streaming input, \
otherwise one process per turn on the retained runtime.";

    /// Static compile-time hint only (feature present). Not sufficient for
    /// catalog selectability — see [`claude_cli_selectable`].
    pub const CLAUDE_CLI_MODEL_SELECTABLE: bool = cfg!(feature = "claude-cli-runtime");

    /// Gates open (feature + runtime env opt-in). Probe is separate.
    pub fn claude_cli_gates_open() -> bool {
        gates::claude_cli_both_gates_open()
    }

    /// Full production selectability: build gate + runtime opt-in + successful
    /// compatible binary probe (cached). Until probe succeeds, stays false.
    pub fn claude_cli_selectable() -> bool {
        crate::agent::external_runtime::probe_cache::claude_cli_catalog_selectable()
    }

    /// UI-facing capability row for an execution backend.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ExecutionCapabilityDescriptor {
        pub backend: ExecutionBackend,
        /// Short label for pickers.
        pub label: &'static str,
        /// Longer description for experimental badges / help text.
        pub description: &'static str,
        pub experimental: bool,
        /// Static capability flag. For Claude CLI, use [`Self::is_selectable_now`].
        pub selectable: bool,
        /// Catalog peer id (`anthropic`, `xai`, …) for grouping.
        pub provider_peer_id: Option<&'static str>,
    }

    impl ExecutionCapabilityDescriptor {
        /// Effective selectability (gates + probe cache for Claude CLI).
        pub fn is_selectable_now(&self) -> bool {
            match self.backend {
                ExecutionBackend::ExternalAgent(ExternalAgentKind::ClaudeCli) => {
                    claude_cli_selectable()
                }
                _ => self.selectable,
            }
        }
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
                label: CLAUDE_CLI_UI_LABEL,
                description: CLAUDE_CLI_UI_LIMITATIONS,
                experimental: true,
                // Feature-compiled builds may advertise the row; runtime gate
                // still required via is_selectable_now / apply_catalog_visibility.
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

    /// Experimental external backends that share the Anthropic provider peer.
    pub fn experimental_anthropic_peer_backends()
    -> impl Iterator<Item = &'static ExecutionCapabilityDescriptor> {
        descriptors()
            .iter()
            .filter(|d| d.experimental && d.provider_peer_id == Some("anthropic"))
    }

    /// Catalog model id for the experimental Claude Agent CLI entry.
    pub const CLAUDE_CLI_CATALOG_MODEL_ID: &str = "claude-agent-cli";

    /// When compile+env gates are open, ensure a catalog row exists for the
    /// experimental Claude CLI (still hidden until probe succeeds).
    /// Does not block or probe — call [`super::bootstrap_claude_cli_probe_if_gated`]
    /// from async provider refresh for the probe bootstrap.
    pub fn inject_claude_cli_catalog_entry_if_gated(
        catalog: &mut indexmap::IndexMap<String, crate::agent::config::ModelEntry>,
    ) {
        if !claude_cli_gates_open() {
            return;
        }
        if catalog.contains_key(CLAUDE_CLI_CATALOG_MODEL_ID) {
            return;
        }
        let mut entry = crate::agent::config::ModelEntry::fallback(
            CLAUDE_CLI_CATALOG_MODEL_ID,
            &Default::default(),
        );
        entry.info.execution_backend =
            ExecutionBackend::ExternalAgent(ExternalAgentKind::ClaudeCli);
        entry.info.hidden = true;
        entry.info.user_selectable = false;
        entry.info.description = Some(format!(
            "{CLAUDE_CLI_UI_LABEL}. {CLAUDE_CLI_UI_LIMITATIONS}"
        ));
        catalog.insert(CLAUDE_CLI_CATALOG_MODEL_ID.to_owned(), entry);
    }

    /// Apply catalog visibility from the live probe cache (non-blocking).
    ///
    /// Claude CLI entries stay hidden / non-selectable until gates open **and**
    /// a successful probe is recorded in [`super::probe_cache`].
    pub fn apply_catalog_visibility(
        catalog: &mut indexmap::IndexMap<String, crate::agent::config::ModelEntry>,
    ) {
        inject_claude_cli_catalog_entry_if_gated(catalog);
        let probe_ok = crate::agent::external_runtime::probe_cache::probe_cache_ok();
        apply_catalog_visibility_with_probe(catalog, Some(probe_ok));
    }

    /// Like [`apply_catalog_visibility`] with an explicit probe override
    /// (`Some(true/false)`). Use `Some(false)` in tests to force hide.
    pub fn apply_catalog_visibility_with_probe(
        catalog: &mut indexmap::IndexMap<String, crate::agent::config::ModelEntry>,
        claude_cli_probe_ok: Option<bool>,
    ) {
        for entry in catalog.values_mut() {
            if !entry.info.execution_backend.is_external() {
                continue;
            }
            let selectable = match entry.info.execution_backend {
                ExecutionBackend::ExternalAgent(ExternalAgentKind::ClaudeCli) => {
                    let gates = claude_cli_gates_open();
                    let probe = claude_cli_probe_ok.unwrap_or_else(
                        crate::agent::external_runtime::probe_cache::probe_cache_ok,
                    );
                    gates && probe
                }
                _ => for_backend(entry.info.execution_backend)
                    .map(|d| d.is_selectable_now())
                    .unwrap_or(false),
            };
            if !selectable {
                entry.info.hidden = true;
                entry.info.user_selectable = false;
            } else {
                entry.info.hidden = false;
                entry.info.user_selectable = true;
            }
        }
    }
}

#[cfg(test)]
mod pr5_foundation_tests;

#[cfg(all(test, feature = "claude-cli-runtime"))]
mod pr6_claude_cli_tests;

#[cfg(all(test, feature = "claude-cli-runtime"))]
mod pr7_claude_cli_tests;

#[cfg(test)]
mod pr8_rollout_audit_tests;

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
    async fn registry_resolves_without_feature_to_unavailable() {
        // Without opt-in / or without feature, default registry still provides
        // a runtime object that fails closed on probe when gates are shut.
        let registry = default_registry();
        let runtime = registry
            .create(ExternalAgentKind::ClaudeCli)
            .expect("factory registered");
        if !gates::claude_cli_both_gates_open() {
            let err = runtime.probe().await;
            // Either unavailable stub or gated ClaudeCliRuntime both fail closed.
            assert!(err.is_err());
            let e = err.unwrap_err();
            assert!(!e.is_auth_error());
        }
    }

    #[test]
    fn capability_matrix_labels_claude_cli_experimental() {
        let d = capability_matrix::for_backend(ExecutionBackend::ExternalAgent(
            ExternalAgentKind::ClaudeCli,
        ))
        .expect("descriptor");
        assert!(d.experimental);
        assert_eq!(d.provider_peer_id, Some("anthropic"));
        assert!(d.label.contains("Claude Agent"));
        assert!(d.label.contains("Experimental"));
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

    #[test]
    fn feature_off_means_not_selectable_via_gates() {
        if !cfg!(feature = "claude-cli-runtime") {
            assert!(!capability_matrix::CLAUDE_CLI_MODEL_SELECTABLE);
            assert!(!capability_matrix::claude_cli_selectable());
        }
    }
}
