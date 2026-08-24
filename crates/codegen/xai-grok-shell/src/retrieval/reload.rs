//! Candidate snapshot build and last-known-good reload outcomes.
//!
//! Parse/validate/resolve complete candidate off the write lock. Invalid
//! syntax/semantic/provider refs retain prior working snapshot. Never publish
//! a partial graph. Stale async rebuilds are rejected by generation check.
//!
//! ## Disabled / missing referenced provider (LKG product policy)
//!
//! When a candidate graph fails validation because a referenced provider is
//! disabled, missing, tombstoned, or incapable, the builder returns
//! [`SnapshotBuildError`] and the registry **retains the last working
//! snapshot** (still `enabled=true` with that route listed). Call-time PR16
//! resolve fails closed for that route and the orchestrator may fall through
//! to the next **explicitly configured** route only — never a silent sibling
//! retarget and never a partial “drop dead routes and publish” graph. This is
//! deliberate no-partial-graph policy, not an automatic route rewrite.

use std::path::Path;
use std::sync::Arc;

use indexmap::IndexMap;
use xai_grok_config_types::{
    EmbeddingModelConfig, RerankerModelConfig, RetrievalGraphConfig, RetrievalProfileConfig,
};

use super::bounds::ProfileBudgetLimits;
use super::graph::{
    EmbeddingRouteDescriptor, RerankerRouteDescriptor, RetrievalSnapshot, SnapshotProfile,
    embedding_space_for, origin_host_from_base_url, snapshot_fingerprint,
};
use crate::provider_registry::ProviderService;
use crate::provider_registry::runtime_cache::load_runtime;
use crate::retrieval_config::management::RetrievalManagementService;
use crate::retrieval_config::parse::parse_retrieval_graph;
use crate::retrieval_config::validate::{
    GraphValidationIssue, ProviderCapabilityView, has_hard_errors,
    validate_retrieval_graph_with_providers,
};

/// Outcome of a reload attempt (secret-free).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReloadOutcome {
    /// New snapshot published.
    Published {
        generation: u64,
        fingerprint: String,
        warnings: Vec<String>,
    },
    /// Candidate invalid; prior snapshot retained.
    RetainedLastKnownGood {
        generation: u64,
        reasons: Vec<String>,
    },
    /// Async rebuild was stale relative to the expected base generation.
    StaleDropped {
        expected_generation: u64,
        live_generation: u64,
    },
    /// Snapshot already matches candidate fingerprint (no-op).
    Unchanged {
        generation: u64,
        fingerprint: String,
    },
    /// Built disabled/empty snapshot (no graph).
    Disabled { generation: u64 },
}

/// Inputs for building a candidate snapshot (tests can inject).
#[derive(Debug, Clone)]
pub struct SnapshotBuildInput {
    pub graph: RetrievalGraphConfig,
    pub graph_generation: u64,
    pub provider_generation: u64,
    pub provider_views: Vec<ProviderCapabilityView>,
    /// Optional provider metadata for origin/incarnation (id → meta).
    pub provider_meta: IndexMap<String, ProviderMetaPin>,
    pub parse_warnings: Vec<String>,
}

/// Secret-free provider pins used when resolving route descriptors.
#[derive(Debug, Clone, Default)]
pub struct ProviderMetaPin {
    pub base_url: Option<String>,
    pub incarnation: Option<String>,
    pub request_timeout_secs: Option<u64>,
    pub enabled: bool,
    pub exists: bool,
}

/// Error building a candidate (never panics).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotBuildError {
    pub reasons: Vec<String>,
}

impl std::fmt::Display for SnapshotBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "retrieval snapshot build failed: {}",
            self.reasons.join("; ")
        )
    }
}

impl std::error::Error for SnapshotBuildError {}

/// Build a complete immutable snapshot from validated inputs.
///
/// Hard validation errors return `Err` so callers retain LKG — including the
/// case where a referenced provider is disabled/missing/tombstoned/incapable
/// (see module docs: no partial graph publish; call-time PR16 still fail-closes).
pub fn build_snapshot(
    input: SnapshotBuildInput,
    next_generation: u64,
) -> Result<Arc<RetrievalSnapshot>, SnapshotBuildError> {
    let issues = validate_retrieval_graph_with_providers(&input.graph, &input.provider_views);
    let mut warnings: Vec<String> = input.parse_warnings;
    for issue in &issues {
        if issue.hard_error {
            // Collect; fail after.
        } else {
            warnings.push(format!("{}: {}", issue.path, issue.message));
        }
    }
    if has_hard_errors(&issues) {
        let reasons: Vec<String> = issues
            .into_iter()
            .filter(|i| i.hard_error)
            .map(|i| format!("{}: {}", i.path, i.message))
            .collect();
        return Err(SnapshotBuildError { reasons });
    }

    let mut embedding_models = IndexMap::new();
    for (id, config) in &input.graph.embedding_models {
        let pin = input
            .provider_meta
            .get(config.provider.as_str())
            .cloned()
            .unwrap_or_default();
        let base = pin.base_url.as_deref().unwrap_or("");
        let space = embedding_space_for(&config.provider, pin.incarnation.as_deref(), base, config);
        embedding_models.insert(
            id.clone(),
            EmbeddingRouteDescriptor {
                model_id: id.clone(),
                config: config.clone(),
                provider_instance_id: config.provider.clone(),
                incarnation: pin.incarnation.clone(),
                origin_host: origin_host_from_base_url(base),
                embedding_space: space,
                request_timeout_ms: pin.request_timeout_secs.unwrap_or(60).saturating_mul(1000),
            },
        );
    }

    let mut reranker_models = IndexMap::new();
    for (id, config) in &input.graph.reranker_models {
        let pin = input
            .provider_meta
            .get(config.provider.as_str())
            .cloned()
            .unwrap_or_default();
        let base = pin.base_url.as_deref().unwrap_or("");
        reranker_models.insert(
            id.clone(),
            RerankerRouteDescriptor {
                model_id: id.clone(),
                config: config.clone(),
                provider_instance_id: config.provider.clone(),
                incarnation: pin.incarnation.clone(),
                origin_host: origin_host_from_base_url(base),
                request_timeout_ms: pin.request_timeout_secs.unwrap_or(60).saturating_mul(1000),
            },
        );
    }

    let mut profiles = IndexMap::new();
    for (id, config) in &input.graph.retrieval_profiles {
        let max_batch = embedding_models
            .values()
            .filter(|r| config.embedding_models.iter().any(|e| e == &r.model_id))
            .map(|r| r.config.batch_size)
            .min()
            .unwrap_or(32);
        let budgets = ProfileBudgetLimits::from_profile(config, max_batch);
        profiles.insert(
            id.clone(),
            SnapshotProfile {
                id: id.clone(),
                config: config.clone(),
                embedding_route_ids: config.embedding_models.clone(),
                reranker_route_ids: config.reranker_models.clone(),
                budgets,
                fallback_strategy: config.fallback_strategy,
            },
        );
    }

    let fingerprint = snapshot_fingerprint(
        &input.graph,
        input.provider_generation,
        input.graph_generation,
    )
    .map_err(|e| SnapshotBuildError { reasons: vec![e] })?;
    let enabled = !profiles.is_empty();
    if !enabled {
        warnings.push("retrieval snapshot has no profiles; service disabled".into());
    }

    Ok(Arc::new(RetrievalSnapshot {
        generation: next_generation,
        graph_generation: input.graph_generation,
        provider_generation: input.provider_generation,
        fingerprint,
        enabled,
        embedding_models,
        reranker_models,
        profiles,
        prime: input.graph.prime.clone(),
        memory_retrieval_profile: input.graph.memory_retrieval_profile.clone(),
        warnings,
        source_graph: input.graph,
    }))
}

/// Load provider capability views + meta pins from a home directory.
pub fn load_provider_context(
    home: &Path,
) -> Result<
    (
        u64,
        Vec<ProviderCapabilityView>,
        IndexMap<String, ProviderMetaPin>,
    ),
    String,
> {
    let (service, lifecycle, generation) = load_runtime(home)?;
    let mut views = Vec::new();
    let mut meta = IndexMap::new();
    for desc in service.list() {
        let id = desc.id.as_str().to_owned();
        let snap_meta = service.snapshot().get(&id);
        let tombstoned = lifecycle.has_blocking_tombstone_for_id(&id)
            || desc
                .incarnation
                .as_ref()
                .is_some_and(|inc| lifecycle.is_tombstoned(&id, inc));
        let embeddings = snap_meta.and_then(|m| m.capabilities.embeddings);
        let rerank = snap_meta.and_then(|m| m.capabilities.extra.get("rerank").copied());
        let capability_mode_manual = snap_meta
            .map(|m| {
                matches!(
                    m.capability_mode,
                    crate::provider_registry::lifecycle::CapabilityMode::Manual
                )
            })
            .unwrap_or(false);
        views.push(ProviderCapabilityView {
            id: id.clone(),
            enabled: desc.enabled,
            tombstoned,
            exists: true,
            embeddings,
            rerank,
            capability_mode_manual,
            api_surface: desc
                .routes
                .first()
                .map(|r| r.api_surface.as_str().to_owned()),
        });
        meta.insert(
            id,
            ProviderMetaPin {
                base_url: snap_meta
                    .and_then(|m| m.base_url.clone())
                    .or_else(|| desc.base_url.clone()),
                incarnation: desc.incarnation.as_ref().map(|i| i.as_str().to_owned()),
                request_timeout_secs: snap_meta.and_then(|m| m.request_timeout_secs),
                enabled: desc.enabled,
                exists: true,
            },
        );
    }
    Ok((generation, views, meta))
}

/// Load graph + providers from home into a build input.
pub fn load_build_input_from_home(home: &Path) -> Result<SnapshotBuildInput, String> {
    let mgmt = RetrievalManagementService::new(home);
    let graph_generation = mgmt.current_generation().get();
    let config_path = home.join("config.toml");
    let (graph, parse_warnings) = match std::fs::read_to_string(&config_path) {
        Ok(raw) => match toml::from_str::<toml::Value>(&raw) {
            Ok(val) => {
                let parsed = parse_retrieval_graph(&val);
                let warns = parsed.warnings.iter().map(|w| w.reason.clone()).collect();
                (parsed.graph, warns)
            }
            Err(e) => return Err(format!("config.toml parse error: {e}")),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            (RetrievalGraphConfig::default(), Vec::new())
        }
        Err(e) => return Err(format!("read config.toml: {e}")),
    };
    let (provider_generation, provider_views, provider_meta) = load_provider_context(home)?;
    Ok(SnapshotBuildInput {
        graph,
        graph_generation,
        provider_generation,
        provider_views,
        provider_meta,
        parse_warnings,
    })
}

/// Build from an in-memory graph + provider service (tests / composition).
pub fn build_input_from_graph_and_service(
    graph: RetrievalGraphConfig,
    service: &ProviderService,
    graph_generation: u64,
) -> SnapshotBuildInput {
    let mut provider_views = Vec::new();
    let mut provider_meta = IndexMap::new();
    for desc in service.list() {
        let id = desc.id.as_str().to_owned();
        let snap_meta = service.snapshot().get(&id);
        let embeddings = snap_meta.and_then(|m| m.capabilities.embeddings);
        let rerank = snap_meta.and_then(|m| m.capabilities.extra.get("rerank").copied());
        let capability_mode_manual = snap_meta
            .map(|m| {
                matches!(
                    m.capability_mode,
                    crate::provider_registry::lifecycle::CapabilityMode::Manual
                )
            })
            .unwrap_or(false);
        provider_views.push(ProviderCapabilityView {
            id: id.clone(),
            enabled: desc.enabled,
            tombstoned: false,
            exists: true,
            embeddings,
            rerank,
            capability_mode_manual,
            api_surface: desc
                .routes
                .first()
                .map(|r| r.api_surface.as_str().to_owned()),
        });
        provider_meta.insert(
            id,
            ProviderMetaPin {
                base_url: snap_meta
                    .and_then(|m| m.base_url.clone())
                    .or_else(|| desc.base_url.clone()),
                incarnation: desc.incarnation.as_ref().map(|i| i.as_str().to_owned()),
                request_timeout_secs: snap_meta.and_then(|m| m.request_timeout_secs),
                enabled: desc.enabled,
                exists: true,
            },
        );
    }
    SnapshotBuildInput {
        graph,
        graph_generation,
        provider_generation: service.generation(),
        provider_views,
        provider_meta,
        parse_warnings: Vec::new(),
    }
}

/// Test helper: minimal valid graph with two embedding routes and one profile.
#[cfg(test)]
pub fn test_graph_two_embed_routes() -> RetrievalGraphConfig {
    let mut graph = RetrievalGraphConfig::default();
    graph.embedding_models.insert(
        "emb-a".into(),
        EmbeddingModelConfig {
            provider: "acct-a".into(),
            model: "embed-a".into(),
            dimensions: Some(8),
            ..Default::default()
        },
    );
    graph.embedding_models.insert(
        "emb-b".into(),
        EmbeddingModelConfig {
            provider: "acct-b".into(),
            model: "embed-b".into(),
            dimensions: Some(8),
            ..Default::default()
        },
    );
    graph.reranker_models.insert(
        "rr-a".into(),
        RerankerModelConfig {
            provider: "acct-a".into(),
            model: "rerank-a".into(),
            ..Default::default()
        },
    );
    graph.retrieval_profiles.insert(
        "default".into(),
        RetrievalProfileConfig {
            embedding_models: vec!["emb-a".into(), "emb-b".into()],
            reranker_models: vec!["rr-a".into()],
            max_candidates: 20,
            max_results: 5,
            deadline_ms: 5_000,
            max_attempts: 3,
            max_input_tokens: 8_192,
            max_output_tokens: 4_096,
            ..Default::default()
        },
    );
    graph
}

#[cfg(test)]
pub fn test_provider_views_capable(
    ids: &[&str],
) -> (
    Vec<ProviderCapabilityView>,
    IndexMap<String, ProviderMetaPin>,
) {
    let mut views = Vec::new();
    let mut meta = IndexMap::new();
    for id in ids {
        views.push(ProviderCapabilityView {
            id: (*id).to_owned(),
            enabled: true,
            tombstoned: false,
            exists: true,
            embeddings: Some(true),
            rerank: Some(true),
            capability_mode_manual: true,
            api_surface: Some("openai_compatible".into()),
        });
        meta.insert(
            (*id).to_owned(),
            ProviderMetaPin {
                base_url: Some(format!("http://127.0.0.1:9/{id}/v1")),
                incarnation: Some(format!("inc-{id}")),
                request_timeout_secs: Some(5),
                enabled: true,
                exists: true,
            },
        );
    }
    (views, meta)
}

// Silence unused import warning for GraphValidationIssue in non-test.
#[allow(dead_code)]
fn _use_issue(_: &GraphValidationIssue) {}
