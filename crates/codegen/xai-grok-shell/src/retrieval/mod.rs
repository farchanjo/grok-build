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
pub mod memory_facade;
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
pub use clock::{Clock, MockClock, SystemClock};
pub use cooldown::{CooldownKey, CooldownTable};
pub use error::{
    BudgetKind, DegradationKind, DegradationNotice, LimitKind, OrchestratorError,
    OrchestratorResult, RetrievalStage, RouteFailureClass,
};
pub use graph::{
    EmbeddingRouteDescriptor, EmbeddingSpaceId, RerankerRouteDescriptor, RetrievalSnapshot,
    SnapshotProfile,
};
pub use memory_facade::{RetrievalServiceMemoryFacade, facade_for_profile};
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

#[cfg(test)]
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Home-keyed process registry map for multi-home / multi-agent composition.
///
/// Each home key (stable absolute path; see [`stable_home_key`]) owns at most
/// one [`RetrievalRegistry`]. Concurrent agents with different homes do not
/// overwrite each other. Prefer holding an `Arc<RetrievalRegistry>` on the
/// agent handle when possible; this map supports hot-reload from config/notify
/// fans that only know the home.
static HOME_REGISTRIES: std::sync::OnceLock<
    parking_lot::RwLock<HashMap<PathBuf, Arc<RetrievalRegistry>>>,
> = std::sync::OnceLock::new();

fn home_map() -> &'static parking_lot::RwLock<HashMap<PathBuf, Arc<RetrievalRegistry>>> {
    HOME_REGISTRIES.get_or_init(|| parking_lot::RwLock::new(HashMap::new()))
}

/// Stable registry map key for a Grok home path.
///
/// Builds an absolute path (joins with the process cwd when `home` is relative)
/// **without** filesystem `canonicalize` (which fails when the directory does
/// not yet exist and can change after create/symlink resolution).
///
/// `dunce::simplified` is applied only for Windows UNC / `\\?\` presentation;
/// on Unix/macOS the path is returned as spelled. This function does **not**
/// lexically collapse `.` / `..` components. Callers must pass a **consistent
/// spelling** for install and lookup (production uses the process-cached
/// `grok_home()` for both).
pub fn stable_home_key(home: &Path) -> PathBuf {
    let absolute = if home.is_absolute() {
        home.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(home))
            .unwrap_or_else(|_| home.to_path_buf())
    };
    dunce::simplified(&absolute).to_path_buf()
}

/// Install (or replace) the registry for `home`. Returns the previous entry if any.
pub fn install_registry_for_home(
    home: impl AsRef<Path>,
    registry: Arc<RetrievalRegistry>,
) -> Option<Arc<RetrievalRegistry>> {
    let key = stable_home_key(home.as_ref());
    home_map().write().insert(key, registry)
}

/// Clone the registry for `home` if installed.
pub fn registry_for_home(home: impl AsRef<Path>) -> Option<Arc<RetrievalRegistry>> {
    let key = stable_home_key(home.as_ref());
    home_map().read().get(&key).cloned()
}

#[cfg(test)]
thread_local! {
    static TEST_REGISTRY_OVERRIDE: RefCell<Option<Arc<RetrievalRegistry>>> = const { RefCell::new(None) };
}

#[cfg(test)]
pub(crate) struct TestRegistryOverride(Option<Arc<RetrievalRegistry>>);

#[cfg(test)]
impl Drop for TestRegistryOverride {
    fn drop(&mut self) {
        TEST_REGISTRY_OVERRIDE.with(|slot| slot.replace(self.0.take()));
    }
}

#[cfg(test)]
pub(crate) fn install_test_registry_override(
    registry: Arc<RetrievalRegistry>,
) -> TestRegistryOverride {
    let previous = TEST_REGISTRY_OVERRIDE.with(|slot| slot.replace(Some(registry)));
    TestRegistryOverride(previous)
}

pub(crate) fn registry_for_prime_home(home: impl AsRef<Path>) -> Option<Arc<RetrievalRegistry>> {
    #[cfg(test)]
    if let Some(registry) = TEST_REGISTRY_OVERRIDE.with(|slot| slot.borrow().clone()) {
        return Some(registry);
    }
    registry_for_home(home)
}

/// Remove the registry for `home` (tests / shutdown).
pub fn uninstall_registry_for_home(home: impl AsRef<Path>) -> Option<Arc<RetrievalRegistry>> {
    let key = stable_home_key(home.as_ref());
    home_map().write().remove(&key)
}

/// Clear all home registries (tests).
pub fn clear_all_registries() {
    home_map().write().clear();
}

/// Reload the registry for a specific home (single-flight inside the registry).
pub fn reload_registry_for_home(home: impl AsRef<Path>) -> Option<ReloadOutcome> {
    let reg = registry_for_home(home)?;
    Some(reg.reload_from_home())
}

/// Reload every installed home registry. Used by process-wide notify/config
/// hooks when the changed home is not known precisely.
pub fn reload_all_registries() -> Vec<(PathBuf, ReloadOutcome)> {
    let regs: Vec<(PathBuf, Arc<RetrievalRegistry>)> = home_map()
        .read()
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    regs.into_iter()
        .map(|(home, reg)| (home, reg.reload_from_home()))
        .collect()
}
