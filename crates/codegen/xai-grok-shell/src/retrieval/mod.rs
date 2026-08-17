//! Shell-owned immutable versioned retrieval registry and orchestrator (PR17).
//!
//! Builds validated snapshots from the PR15 retrieval graph and current
//! provider registry generation, then runs ordered explicit fallback with
//! strict profile budgets, cooldowns, safe degradation, and atomic
//! last-known-good reload. Credentials are never stored in snapshots; each
//! attempt resolves exact PR16 routes at call time.
//!
//! Out of scope: PR18 skill inventory/selection, PR19 injection, PR21 durable
//! memory migration.

pub mod bounds;
pub mod clients;
pub mod clock;
pub mod cooldown;
pub mod error;
pub mod graph;
pub mod pipeline;
pub mod registry;
pub mod reload;
pub mod service;
pub mod telemetry;

#[cfg(test)]
mod tests;

pub use bounds::{ProfileBudgetLimits, ProfileBudgetTracker, estimate_tokens_from_bytes};
pub use clients::{
    FakeEmbedScript, FakeRerankScript, FakeRetrievalExecutor, Pr16RetrievalExecutor,
    RetrievalExecutor, RouteCallPins,
};
pub use clock::{Clock, ImmediateSleeper, MockClock, Sleeper, SystemClock, TokioSleeper};
pub use cooldown::{CooldownKey, CooldownTable};
pub use error::{
    BudgetKind, DegradationKind, DegradationNotice, LimitKind, OrchestratorError,
    OrchestratorResult, RetrievalStage, RouteFailureClass,
};
pub use graph::{
    EmbeddingRouteDescriptor, EmbeddingSpaceId, RerankerRouteDescriptor, RetrievalSnapshot,
    SnapshotProfile,
};
pub use pipeline::{
    CandidateRow, EmbedStageResult, PipelineOptions, RerankStageResult, RetrieveResult,
};
pub use registry::RetrievalRegistry;
pub use reload::{
    ProviderMetaPin, ReloadOutcome, SnapshotBuildError, SnapshotBuildInput,
    build_input_from_graph_and_service, build_snapshot, load_build_input_from_home,
};
pub use service::{CandidateProvider, RetrievalService, RetrieveCandidates};
pub use telemetry::{
    BudgetFlags, RecordingTelemetrySink, RetrievalTelemetryEvent, TelemetryOutcome, TelemetrySink,
    TracingTelemetrySink,
};

/// Process-level default registry holder for agent composition (optional).
///
/// Composition roots may install a registry for hot-reload integration. Tests
/// should construct local registries instead of relying on the global.
static GLOBAL_REGISTRY: std::sync::OnceLock<
    parking_lot::RwLock<Option<std::sync::Arc<RetrievalRegistry>>>,
> = std::sync::OnceLock::new();

fn global_slot() -> &'static parking_lot::RwLock<Option<std::sync::Arc<RetrievalRegistry>>> {
    GLOBAL_REGISTRY.get_or_init(|| parking_lot::RwLock::new(None))
}

/// Install the process-level retrieval registry (agent composition).
pub fn install_global_registry(registry: std::sync::Arc<RetrievalRegistry>) {
    *global_slot().write() = Some(registry);
}

/// Clear the process-level registry (tests / shutdown).
pub fn clear_global_registry() {
    *global_slot().write() = None;
}

/// Clone the process-level registry if installed.
pub fn global_registry() -> Option<std::sync::Arc<RetrievalRegistry>> {
    global_slot().read().clone()
}

/// Reload the global registry from its home when installed.
pub fn reload_global_registry() -> Option<ReloadOutcome> {
    let reg = global_registry()?;
    Some(reg.reload_from_home())
}
