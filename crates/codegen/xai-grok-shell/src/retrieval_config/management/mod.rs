//! Shell-authoritative retrieval graph management API.
//!
//! The pager never reads/writes raw TOML for retrieval CRUD. All list/get/
//! validate/upsert/clone/delete/reorder/save operations go through
//! [`RetrievalManagementService`].
//!
//! - Snapshots and async results are generation-tagged.
//! - Stale saves fail closed with reload/clone guidance (no last-writer-wins).
//! - Mutations: exclusive lock → reread disk → apply in memory → validate
//!   complete graph → one durable atomic comment-preserving write → generation bump.
//! - IDs are immutable once referenced; Clone + explicit migration instead of rename.
//! - Synthetic preview is credential-free and network-free.

pub mod dto;

use super::notify;
use super::parse::parse_retrieval_graph;
use super::toml_edit::write_retrieval_graph;
use super::validate::{
    GraphValidationIssue, ProviderCapabilityView, has_hard_errors,
    validate_retrieval_graph_with_providers,
};
use crate::agent::model_providers::parse_model_providers;
use crate::provider_registry::management::dto::RegistryGeneration;
use crate::provider_registry::{BuiltInProviderId, ProviderService, load_lifecycle_state};
use dto::*;
use fs2::FileExt;
use indexmap::IndexMap;
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use xai_grok_config_types::{
    EmbeddingModelConfig, PrimeConfig, RerankerModelConfig, RetrievalGraphConfig,
    RetrievalProfileConfig, normalize_retrieval_id,
};

const GENERATION_REL: &str = "state/retrieval_graph_generation";
const LOCK_REL: &str = "state/retrieval_graph.lock";

const STALE_GUIDANCE: &str = "Retrieval graph generation is stale. Reload the graph, re-apply your \
     edits, or clone into a new id if another client saved first.";

/// Shell-owned retrieval graph management surface.
#[derive(Clone, Debug)]
pub struct RetrievalManagementService {
    home: PathBuf,
    config_path: PathBuf,
}

impl RetrievalManagementService {
    pub fn new(home: impl Into<PathBuf>) -> Self {
        let home = home.into();
        let config_path = home.join("config.toml");
        Self { home, config_path }
    }

    pub fn from_grok_home() -> Self {
        Self::new(xai_grok_config::grok_home())
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    /// Current effective generation (read-only; no sidecar write).
    pub fn current_generation(&self) -> RegistryGeneration {
        RegistryGeneration(self.effective_generation_readonly())
    }

    /// Full secret-free graph snapshot.
    pub fn graph_snapshot(&self) -> Result<RetrievalGraphSnapshot, String> {
        let generation = self.current_generation();
        let (graph, warnings) = self.load_graph()?;
        let providers = self.provider_capability_views()?;
        let issues = validate_retrieval_graph_with_providers(&graph, &providers);
        Ok(self.snapshot_from(generation, graph, warnings, issues))
    }

    /// Validate without mutating. Network-free.
    pub fn validate(&self) -> Result<RetrievalGraphSnapshot, String> {
        self.graph_snapshot()
    }

    /// Bounded synthetic preview: describes validation readiness only.
    /// Never pretends a provider call occurred.
    pub fn preview(
        &self,
        entity_kind: &str,
        entity_id: &str,
        operation_id: Option<String>,
    ) -> RetrievalPreviewResult {
        let generation = self.current_generation();
        let Ok(snap) = self.graph_snapshot() else {
            return RetrievalPreviewResult {
                generation,
                validation_ready: false,
                messages: vec![
                    "Could not load retrieval graph for preview.".into(),
                    "No provider network call was attempted.".into(),
                ],
                operation_id,
            };
        };
        let mut messages = vec![
            format!(
                "Synthetic validation for {entity_kind} `{entity_id}` (network-free, credential-free)."
            ),
            "No provider call was attempted; this only reports config readiness.".into(),
        ];
        if snap.is_valid {
            messages.push("Graph validation: ready (no hard errors).".into());
        } else {
            messages.push("Graph validation: not ready.".into());
            messages.extend(snap.validation_errors.iter().cloned());
        }
        messages.extend(snap.validation_warnings.iter().cloned());
        RetrievalPreviewResult {
            generation,
            validation_ready: snap.is_valid,
            messages,
            operation_id,
        }
    }

    /// Save the complete graph (CAS + lock + atomic write).
    pub fn save_graph(&self, req: RetrievalGraphSaveRequest) -> RetrievalMutationResult {
        let _lock = match self.acquire_mutation_lock() {
            Ok(l) => l,
            Err(e) => return err_result(self.current_generation(), e, req.operation_id.clone()),
        };
        if let Err(msg) = self.require_generation_locked(req.expected_generation) {
            return stale_result(
                req.expected_generation,
                self.current_generation(),
                vec!["generation".into()],
                msg,
                req.operation_id.clone(),
            );
        }
        let mut graph = RetrievalGraphConfig {
            embedding_models: req.embedding_models,
            reranker_models: req.reranker_models,
            retrieval_profiles: req.retrieval_profiles,
            prime: req.prime,
            memory_retrieval_profile: req.memory_retrieval_profile,
        };
        // Normalize ids.
        if let Err(e) = normalize_graph_ids(&mut graph) {
            return err_result(self.current_generation(), e, req.operation_id.clone());
        }

        let prior = self.load_graph().map(|(g, _)| g).unwrap_or_default();
        let reindex = compute_memory_reindex_impact(&prior, &graph);
        if reindex.requires_confirmation && !req.confirm_memory_reindex {
            return RetrievalMutationResult {
                ok: false,
                generation: self.current_generation(),
                error: Some(
                    "Memory reindex confirmation required: selected embedding identity or \
                     dimensions would change. Confirm explicitly; reindex is not performed in \
                     this release."
                        .into(),
                ),
                stale: false,
                guidance: Some(
                    "Review Memory reindex impact, then save again with confirmation.".into(),
                ),
                conflict: None,
                changed_fields: Vec::new(),
                operation_id: req.operation_id,
                memory_reindex: Some(reindex),
                snapshot: None,
            };
        }

        let providers = match self.provider_capability_views() {
            Ok(v) => v,
            Err(e) => {
                return err_result(
                    self.current_generation(),
                    format!("provider registry unavailable (fail closed): {e}"),
                    req.operation_id.clone(),
                );
            }
        };
        let issues = validate_retrieval_graph_with_providers(&graph, &providers);
        if has_hard_errors(&issues) {
            let msgs: Vec<String> = issues
                .iter()
                .filter(|i| i.hard_error)
                .map(|i| format!("{}: {}", i.path, i.message))
                .collect();
            return err_result(
                self.current_generation(),
                format!("graph validation failed: {}", msgs.join("; ")),
                req.operation_id.clone(),
            );
        }

        if let Err(e) = write_retrieval_graph(&self.config_path, &graph) {
            return err_result(self.current_generation(), e, req.operation_id.clone());
        }
        let new_gen = match self.bump_generation_locked() {
            Ok(g) => g,
            Err(e) => {
                return RetrievalMutationResult {
                    ok: false,
                    generation: self.current_generation(),
                    error: Some(format!(
                        "config written but generation bookkeeping failed: {e}. Reload."
                    )),
                    stale: false,
                    guidance: Some("Reload the retrieval graph.".into()),
                    conflict: None,
                    changed_fields: vec!["graph".into()],
                    operation_id: req.operation_id,
                    memory_reindex: Some(reindex),
                    snapshot: None,
                };
            }
        };
        let changed = vec![
            "embedding_models".into(),
            "reranker_models".into(),
            "retrieval_profiles".into(),
            "prime".into(),
            "memory.retrieval_profile".into(),
        ];
        notify::publish_retrieval_update(&self.home, new_gen.get(), &changed);
        let snap = self
            .graph_snapshot()
            .unwrap_or_else(|_| RetrievalGraphSnapshot {
                generation: new_gen,
                ..Default::default()
            });
        RetrievalMutationResult {
            ok: true,
            generation: new_gen,
            error: None,
            stale: false,
            guidance: None,
            conflict: None,
            changed_fields: changed,
            operation_id: req.operation_id,
            memory_reindex: if reindex.requires_confirmation {
                Some(reindex)
            } else {
                None
            },
            snapshot: Some(snap),
        }
    }

    pub fn upsert_embedding(&self, req: UpsertEmbeddingRequest) -> RetrievalMutationResult {
        self.mutate(
            req.expected_generation,
            req.operation_id,
            req.confirm_memory_reindex,
            |graph| {
                let id = normalize_retrieval_id(&req.id).map_err(|e| e)?;
                graph.embedding_models.insert(id.clone(), req.config);
                Ok(vec![format!("embedding_models.{id}")])
            },
        )
    }

    pub fn upsert_reranker(&self, req: UpsertRerankerRequest) -> RetrievalMutationResult {
        self.mutate(
            req.expected_generation,
            req.operation_id,
            req.confirm_memory_reindex,
            |graph| {
                let id = normalize_retrieval_id(&req.id).map_err(|e| e)?;
                graph.reranker_models.insert(id.clone(), req.config);
                Ok(vec![format!("reranker_models.{id}")])
            },
        )
    }

    pub fn upsert_profile(&self, req: UpsertProfileRequest) -> RetrievalMutationResult {
        self.mutate(
            req.expected_generation,
            req.operation_id,
            req.confirm_memory_reindex,
            |graph| {
                let id = normalize_retrieval_id(&req.id).map_err(|e| e)?;
                graph.retrieval_profiles.insert(id.clone(), req.config);
                Ok(vec![format!("retrieval_profiles.{id}")])
            },
        )
    }

    pub fn clone_entity(&self, req: CloneRetrievalEntityRequest) -> RetrievalMutationResult {
        self.mutate(
            req.expected_generation,
            req.operation_id,
            req.confirm_memory_reindex,
            |graph| {
                let new_id = normalize_retrieval_id(&req.new_id)?;
                let source = req.source_id.trim();
                match req.kind.as_str() {
                    "embedding" => {
                        let cfg = graph
                            .embedding_models
                            .get(source)
                            .cloned()
                            .ok_or_else(|| format!("embedding model `{source}` not found"))?;
                        if graph.embedding_models.contains_key(&new_id) {
                            return Err(format!("embedding model `{new_id}` already exists"));
                        }
                        graph.embedding_models.insert(new_id.clone(), cfg);
                        Ok(vec![format!("embedding_models.{new_id}")])
                    }
                    "reranker" => {
                        let cfg = graph
                            .reranker_models
                            .get(source)
                            .cloned()
                            .ok_or_else(|| format!("reranker model `{source}` not found"))?;
                        if graph.reranker_models.contains_key(&new_id) {
                            return Err(format!("reranker model `{new_id}` already exists"));
                        }
                        graph.reranker_models.insert(new_id.clone(), cfg);
                        Ok(vec![format!("reranker_models.{new_id}")])
                    }
                    "profile" => {
                        let cfg = graph
                            .retrieval_profiles
                            .get(source)
                            .cloned()
                            .ok_or_else(|| format!("retrieval profile `{source}` not found"))?;
                        if graph.retrieval_profiles.contains_key(&new_id) {
                            return Err(format!("retrieval profile `{new_id}` already exists"));
                        }
                        graph.retrieval_profiles.insert(new_id.clone(), cfg);
                        Ok(vec![format!("retrieval_profiles.{new_id}")])
                    }
                    other => Err(format!("unknown clone kind `{other}`")),
                }
            },
        )
    }

    pub fn delete_entity(&self, req: DeleteRetrievalEntityRequest) -> RetrievalMutationResult {
        self.mutate(
            req.expected_generation,
            req.operation_id,
            req.confirm_memory_reindex,
            |graph| {
            let id = req.id.trim();
            match req.kind.as_str() {
                "embedding" => {
                    // Block delete when referenced.
                    for (pid, p) in &graph.retrieval_profiles {
                        if p.embedding_models.iter().any(|e| e == id) {
                            return Err(format!(
                                "embedding model `{id}` is referenced by profile `{pid}`; \
                                 remove or retarget the profile first (ids are immutable)"
                            ));
                        }
                    }
                    if graph.embedding_models.shift_remove(id).is_none() {
                        return Err(format!("embedding model `{id}` not found"));
                    }
                    Ok(vec![format!("embedding_models.{id}")])
                }
                "reranker" => {
                    for (pid, p) in &graph.retrieval_profiles {
                        if p.reranker_models.iter().any(|e| e == id) {
                            return Err(format!(
                                "reranker model `{id}` is referenced by profile `{pid}`; \
                                 remove or retarget the profile first"
                            ));
                        }
                    }
                    if graph.reranker_models.shift_remove(id).is_none() {
                        return Err(format!("reranker model `{id}` not found"));
                    }
                    Ok(vec![format!("reranker_models.{id}")])
                }
                "profile" => {
                    if graph.prime.skills.retrieval_profile.as_deref() == Some(id) {
                        return Err(format!(
                            "profile `{id}` is referenced by prime.skills; clear that reference first"
                        ));
                    }
                    if graph.prime.agents.retrieval_profile.as_deref() == Some(id) {
                        return Err(format!(
                            "profile `{id}` is referenced by prime.agents; clear that reference first"
                        ));
                    }
                    if graph.memory_retrieval_profile.as_deref() == Some(id) {
                        return Err(format!(
                            "profile `{id}` is referenced by memory.retrieval_profile; clear that \
                             reference first"
                        ));
                    }
                    if graph.retrieval_profiles.shift_remove(id).is_none() {
                        return Err(format!("retrieval profile `{id}` not found"));
                    }
                    Ok(vec![format!("retrieval_profiles.{id}")])
                }
                other => Err(format!("unknown delete kind `{other}`")),
            }
        },
        )
    }

    pub fn reorder(&self, req: ReorderRetrievalRequest) -> RetrievalMutationResult {
        self.mutate(
            req.expected_generation,
            req.operation_id,
            req.confirm_memory_reindex,
            |graph| match req.kind.as_str() {
                "embedding" => {
                    reorder_map(&mut graph.embedding_models, &req.ordered_ids)?;
                    Ok(vec!["embedding_models.order".into()])
                }
                "reranker" => {
                    reorder_map(&mut graph.reranker_models, &req.ordered_ids)?;
                    Ok(vec!["reranker_models.order".into()])
                }
                "profile" => {
                    reorder_map(&mut graph.retrieval_profiles, &req.ordered_ids)?;
                    Ok(vec!["retrieval_profiles.order".into()])
                }
                other => Err(format!("unknown reorder kind `{other}`")),
            },
        )
    }

    /// Update prime settings.
    pub fn save_prime(
        &self,
        expected: RegistryGeneration,
        prime: PrimeConfig,
        confirm_memory_reindex: bool,
        operation_id: Option<String>,
    ) -> RetrievalMutationResult {
        self.mutate(expected, operation_id, confirm_memory_reindex, |graph| {
            graph.prime = prime;
            Ok(vec!["prime".into()])
        })
    }

    /// Update memory retrieval profile selection.
    pub fn save_memory_profile(
        &self,
        expected: RegistryGeneration,
        profile: Option<String>,
        confirm_memory_reindex: bool,
        operation_id: Option<String>,
    ) -> RetrievalMutationResult {
        self.mutate(expected, operation_id, confirm_memory_reindex, |graph| {
            graph.memory_retrieval_profile = profile;
            Ok(vec!["memory.retrieval_profile".into()])
        })
    }

    // ----- internals -----

    fn mutate(
        &self,
        expected: RegistryGeneration,
        operation_id: Option<String>,
        confirm_memory_reindex: bool,
        apply: impl FnOnce(&mut RetrievalGraphConfig) -> Result<Vec<String>, String>,
    ) -> RetrievalMutationResult {
        let _lock = match self.acquire_mutation_lock() {
            Ok(l) => l,
            Err(e) => return err_result(self.current_generation(), e, operation_id),
        };
        if let Err(msg) = self.require_generation_locked(expected) {
            return stale_result(
                expected,
                self.current_generation(),
                vec!["generation".into()],
                msg,
                operation_id,
            );
        }
        let (mut graph, _) = match self.load_graph() {
            Ok(g) => g,
            Err(e) => return err_result(self.current_generation(), e, operation_id),
        };
        let prior = graph.clone();
        let changed = match apply(&mut graph) {
            Ok(c) => c,
            Err(e) => return err_result(self.current_generation(), e, operation_id),
        };
        let reindex = compute_memory_reindex_impact(&prior, &graph);
        if reindex.requires_confirmation && !confirm_memory_reindex {
            return RetrievalMutationResult {
                ok: false,
                generation: self.current_generation(),
                error: Some(
                    "Memory reindex confirmation required: selected embedding identity or \
                     dimensions would change. Confirm explicitly; reindex is not performed in \
                     this release."
                        .into(),
                ),
                stale: false,
                guidance: Some(
                    "Review Memory reindex impact, then save again with confirmation.".into(),
                ),
                conflict: None,
                changed_fields: Vec::new(),
                operation_id,
                memory_reindex: Some(reindex),
                snapshot: None,
            };
        }
        self.commit_graph_locked(graph, changed, operation_id, Some(reindex))
    }

    fn commit_graph_locked(
        &self,
        graph: RetrievalGraphConfig,
        changed: Vec<String>,
        operation_id: Option<String>,
        memory_reindex: Option<MemoryReindexImpact>,
    ) -> RetrievalMutationResult {
        let providers = match self.provider_capability_views() {
            Ok(v) => v,
            Err(e) => {
                return err_result(
                    self.current_generation(),
                    format!("provider registry unavailable (fail closed): {e}"),
                    operation_id,
                );
            }
        };
        let issues = validate_retrieval_graph_with_providers(&graph, &providers);
        if has_hard_errors(&issues) {
            let msgs: Vec<String> = issues
                .iter()
                .filter(|i| i.hard_error)
                .map(|i| format!("{}: {}", i.path, i.message))
                .collect();
            return err_result(
                self.current_generation(),
                format!("graph validation failed: {}", msgs.join("; ")),
                operation_id,
            );
        }
        if let Err(e) = write_retrieval_graph(&self.config_path, &graph) {
            return err_result(self.current_generation(), e, operation_id);
        }
        let new_gen = match self.bump_generation_locked() {
            Ok(g) => g,
            Err(e) => {
                return RetrievalMutationResult {
                    ok: false,
                    generation: self.current_generation(),
                    error: Some(format!(
                        "config written but generation bookkeeping failed: {e}. Reload."
                    )),
                    stale: false,
                    guidance: Some("Reload the retrieval graph.".into()),
                    conflict: None,
                    changed_fields: changed,
                    operation_id,
                    memory_reindex,
                    snapshot: None,
                };
            }
        };
        notify::publish_retrieval_update(&self.home, new_gen.get(), &changed);
        let snap = self.graph_snapshot().ok();
        RetrievalMutationResult {
            ok: true,
            generation: new_gen,
            error: None,
            stale: false,
            guidance: None,
            conflict: None,
            changed_fields: changed,
            operation_id,
            memory_reindex,
            snapshot: snap,
        }
    }

    fn load_graph(&self) -> Result<(RetrievalGraphConfig, Vec<String>), String> {
        let raw = match fs::read_to_string(&self.config_path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(format!("read config: {e}")),
        };
        let value: toml::Value = if raw.trim().is_empty() {
            toml::Value::Table(toml::map::Map::new())
        } else {
            toml::from_str(&raw).map_err(|e| format!("parse config: {e}"))?
        };
        let parsed = parse_retrieval_graph(&value);
        let warnings = parsed
            .warnings
            .iter()
            .map(|w| format!("{}: {}", w.target.label(), w.reason))
            .collect();
        Ok((parsed.graph, warnings))
    }

    fn snapshot_from(
        &self,
        generation: RegistryGeneration,
        graph: RetrievalGraphConfig,
        warnings: Vec<String>,
        issues: Vec<GraphValidationIssue>,
    ) -> RetrievalGraphSnapshot {
        let validation_errors: Vec<String> = issues
            .iter()
            .filter(|i| i.hard_error)
            .map(|i| format!("{}: {}", i.path, i.message))
            .collect();
        let validation_warnings: Vec<String> = issues
            .iter()
            .filter(|i| !i.hard_error)
            .map(|i| format!("{}: {}", i.path, i.message))
            .collect();
        let is_valid = validation_errors.is_empty();
        RetrievalGraphSnapshot {
            generation,
            embedding_models: graph
                .embedding_models
                .into_iter()
                .map(|(id, config)| EmbeddingModelDto { id, config })
                .collect(),
            reranker_models: graph
                .reranker_models
                .into_iter()
                .map(|(id, config)| RerankerModelDto { id, config })
                .collect(),
            retrieval_profiles: graph
                .retrieval_profiles
                .into_iter()
                .map(|(id, config)| RetrievalProfileDto { id, config })
                .collect(),
            prime: graph.prime.into(),
            memory_retrieval_profile: graph.memory_retrieval_profile,
            warnings,
            validation_errors,
            validation_warnings,
            is_valid,
        }
    }

    fn provider_capability_views(&self) -> Result<Vec<ProviderCapabilityView>, String> {
        let raw = match fs::read_to_string(&self.config_path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(format!("read config for providers: {e}")),
        };
        let value: toml::Value = if raw.trim().is_empty() {
            toml::Value::Table(Default::default())
        } else {
            toml::from_str(&raw).map_err(|e| format!("parse config for providers: {e}"))?
        };
        let (entries, _) = parse_model_providers(&value);
        let service = ProviderService::from_model_providers(&entries)
            .map_err(|e| format!("provider registry build failed: {e}"))?;
        let lifecycle = load_lifecycle_state(&self.home).ok();
        let mut views = Vec::new();
        for desc in service.list() {
            let id = desc.id.as_str().to_owned();
            let meta = service.snapshot().get(&id);
            // Align with runtime reload view (L4): blocking tombstone for id
            // or incarnation-specific tombstone when present.
            let tombstoned = lifecycle
                .as_ref()
                .map(|st| {
                    st.has_blocking_tombstone_for_id(&id)
                        || desc
                            .incarnation
                            .as_ref()
                            .is_some_and(|inc| st.is_tombstoned(&id, inc))
                })
                .unwrap_or(false);
            let mut embeddings = None;
            let mut rerank = None;
            let mut capability_mode_manual = false;
            if let Some(m) = meta {
                embeddings = m.capabilities.embeddings;
                rerank = m
                    .capabilities
                    .extra
                    .get("rerank")
                    .copied()
                    .or_else(|| m.capabilities.extra.get("reranking").copied());
                capability_mode_manual = matches!(
                    m.capability_mode,
                    crate::provider_registry::lifecycle::CapabilityMode::Manual
                );
            }
            // Prefer raw config capabilities when present.
            if let Some(cfg) = entries.get(&id) {
                if let Some(v) = cfg.capabilities.get("embeddings") {
                    embeddings = Some(*v);
                }
                if let Some(v) = cfg.capabilities.get("rerank") {
                    rerank = Some(*v);
                } else if let Some(v) = cfg.capabilities.get("reranking") {
                    rerank = Some(*v);
                }
                if cfg
                    .capability_mode
                    .as_deref()
                    .is_some_and(|m| m.eq_ignore_ascii_case("manual"))
                {
                    capability_mode_manual = true;
                }
            }
            // Built-ins that exist are always "exists".
            let exists = true;
            let api_surface = meta.and_then(|m| {
                // Descriptor path: use kind string as surface hint.
                Some(desc.kind.as_str().to_owned())
            });
            let _ = BuiltInProviderId::parse(&id);
            views.push(ProviderCapabilityView {
                id,
                enabled: desc.enabled,
                tombstoned,
                exists,
                embeddings,
                rerank,
                capability_mode_manual,
                api_surface,
            });
        }
        Ok(views)
    }

    fn acquire_mutation_lock(&self) -> Result<File, String> {
        let path = self.home.join(LOCK_REL);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("create lock parent: {e}"))?;
        }
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|e| format!("open retrieval lock: {e}"))?;
        file.lock_exclusive()
            .map_err(|e| format!("lock retrieval graph: {e}"))?;
        Ok(file)
    }

    fn require_generation_locked(&self, expected: RegistryGeneration) -> Result<(), String> {
        let live = self.effective_generation_locked();
        if live != expected.get() {
            return Err(format!(
                "expected generation {}, live is {live}",
                expected.get()
            ));
        }
        Ok(())
    }

    fn effective_generation_readonly(&self) -> u64 {
        let (stored, fp) = read_generation_raw(&self.home);
        let current_fp = config_fingerprint(&self.config_path);
        // No generation sidecar yet: clients start at 0 even if config.toml exists
        // (first mutation bumps to 1 under lock).
        if stored == 0 {
            return 0;
        }
        if fp != current_fp {
            // External edit: report stored+1 without writing.
            return stored.saturating_add(1);
        }
        stored
    }

    fn effective_generation_locked(&self) -> u64 {
        let (stored, fp) = read_generation_raw(&self.home);
        let current_fp = config_fingerprint(&self.config_path);
        // First mutation: accept expected 0; durable bump writes gen 1 after save.
        if stored == 0 {
            return 0;
        }
        if fp != current_fp {
            // Reconcile under lock when a prior generation exists but config drifted.
            let next = stored.saturating_add(1).max(1);
            let _ = write_generation_state(&self.home, next, &current_fp);
            return next;
        }
        stored
    }

    fn bump_generation_locked(&self) -> Result<RegistryGeneration, String> {
        let (stored, _) = read_generation_raw(&self.home);
        let next = stored.saturating_add(1).max(1);
        let fp = config_fingerprint(&self.config_path);
        write_generation_state(&self.home, next, &fp)?;
        Ok(RegistryGeneration(next))
    }
}

fn normalize_graph_ids(graph: &mut RetrievalGraphConfig) -> Result<(), String> {
    let emb = std::mem::take(&mut graph.embedding_models);
    let mut new_emb = IndexMap::new();
    for (id, cfg) in emb {
        let nid = normalize_retrieval_id(&id)?;
        if new_emb.contains_key(&nid) {
            return Err(format!(
                "embedding model id collision after normalize: `{nid}` (rejecting save rather \
                 than silent overwrite)"
            ));
        }
        new_emb.insert(nid, cfg);
    }
    graph.embedding_models = new_emb;
    let rr = std::mem::take(&mut graph.reranker_models);
    let mut new_rr = IndexMap::new();
    for (id, cfg) in rr {
        let nid = normalize_retrieval_id(&id)?;
        if new_rr.contains_key(&nid) {
            return Err(format!(
                "reranker model id collision after normalize: `{nid}` (rejecting save rather \
                 than silent overwrite)"
            ));
        }
        new_rr.insert(nid, cfg);
    }
    graph.reranker_models = new_rr;
    let pr = std::mem::take(&mut graph.retrieval_profiles);
    let mut new_pr = IndexMap::new();
    for (id, cfg) in pr {
        let nid = normalize_retrieval_id(&id)?;
        if new_pr.contains_key(&nid) {
            return Err(format!(
                "retrieval profile id collision after normalize: `{nid}` (rejecting save rather \
                 than silent overwrite)"
            ));
        }
        new_pr.insert(nid, cfg);
    }
    graph.retrieval_profiles = new_pr;
    Ok(())
}

fn reorder_map<T: Clone>(
    map: &mut IndexMap<String, T>,
    ordered_ids: &[String],
) -> Result<(), String> {
    if ordered_ids.len() != map.len() {
        return Err("reorder id list must contain exactly the existing ids".into());
    }
    let mut seen = std::collections::HashSet::new();
    let mut new_map = IndexMap::new();
    for id in ordered_ids {
        if !seen.insert(id.as_str()) {
            return Err(format!("duplicate id `{id}` in reorder list"));
        }
        let cfg = map
            .get(id)
            .cloned()
            .ok_or_else(|| format!("unknown id `{id}` in reorder list"))?;
        new_map.insert(id.clone(), cfg);
    }
    *map = new_map;
    Ok(())
}

/// Compute whether memory embedding identity/dimensions would change.
/// Does **not** perform reindex.
///
/// The config-level fingerprint mirrors the durable `VectorFingerprint`
/// fields that are readable here (provider, model, embedding-model id,
/// dimensions, protocol, encoding, batch size, input tokens). The durable
/// fingerprint additionally pins normalized origin/path, normalization, and
/// doc-prep parameters that are **not** readable from the retrieval graph, so
/// when memory uses a named profile this helper is conservative: it requires
/// confirmation whenever it cannot prove "no rebuild".
///
/// **Known over-confirmation (N-03):** because the durable fingerprint cannot
/// be fully verified from the retrieval graph, *any* graph save while a named
/// memory profile is selected in either graph requires confirmation — including
/// reranker-only or credential-rotation changes that can never trigger a
/// durable rebuild. This is intentional and fail-closed (it never
/// under-reports a real rebuild); a future refinement may compare only the
/// profile's embedding-route config when the profile id is unchanged.
pub fn compute_memory_reindex_impact(
    prior: &RetrievalGraphConfig,
    next: &RetrievalGraphConfig,
) -> MemoryReindexImpact {
    let prev_fp = memory_embedding_fingerprint(prior);
    let next_fp = memory_embedding_fingerprint(next);
    // Memory is "affected" when a named profile is selected in either graph.
    let memory_affected =
        prior.memory_retrieval_profile.is_some() || next.memory_retrieval_profile.is_some();
    if !memory_affected {
        return MemoryReindexImpact {
            requires_confirmation: false,
            reason: "Embedding models changed but memory has no named profile selection.".into(),
            previous_fingerprint: prev_fp,
            next_fingerprint: next_fp,
        };
    }
    if prev_fp == next_fp {
        // The durable VectorFingerprint also pins normalized origin/path,
        // normalization, and doc-prep params that are not readable from the
        // retrieval graph here, so we cannot prove "no rebuild": be
        // conservative and require confirmation rather than returning false.
        return MemoryReindexImpact {
            requires_confirmation: true,
            reason: "Memory uses a named retrieval profile whose durable vector identity \
                     cannot be fully verified from config here; treat conservatively \
                     (memory stays FTS-only until reconciled)."
                .into(),
            previous_fingerprint: prev_fp,
            next_fingerprint: next_fp,
        };
    }
    MemoryReindexImpact {
        requires_confirmation: true,
        reason: "Selected memory retrieval profile embedding identity or dimensions \
                 (provider/model/embedding id/dimensions/protocol/encoding) would change. \
                 Memory vectors will be rebuilt automatically through the pinned profile; \
                 memory search stays FTS-only until the transactional rebuild completes."
            .into(),
        previous_fingerprint: prev_fp,
        next_fingerprint: next_fp,
    }
}

fn memory_embedding_fingerprint(graph: &RetrievalGraphConfig) -> Option<String> {
    let profile_id = graph.memory_retrieval_profile.as_deref()?;
    let profile = graph.retrieval_profiles.get(profile_id)?;
    let emb_id = profile.embedding_models.first()?;
    let emb = graph.embedding_models.get(emb_id.as_str())?;
    let encoding = match emb.encoding {
        xai_grok_config_types::EmbeddingEncoding::Float => "float",
        xai_grok_config_types::EmbeddingEncoding::Base64 => "base64",
    };
    Some(format!(
        "{}|{}|{}|{}|{}|{}|{}|{}",
        emb.provider,
        emb.model,
        emb_id,
        emb.dimensions.map_or("none".to_string(), |d| d.to_string()),
        emb.protocol.as_str(),
        encoding,
        emb.batch_size,
        emb.max_input_tokens,
    ))
}

fn config_fingerprint(config_path: &Path) -> String {
    match fs::read(config_path) {
        Ok(bytes) => format!("{:x}", Sha256::digest(&bytes)),
        Err(_) => String::new(),
    }
}

fn read_generation_raw(home: &Path) -> (u64, String) {
    let path = home.join(GENERATION_REL);
    match fs::read_to_string(path) {
        Ok(s) => {
            let mut lines = s.lines();
            let generation = lines
                .next()
                .and_then(|l| l.trim().parse().ok())
                .unwrap_or(0);
            let fp = lines.next().unwrap_or("").trim().to_owned();
            (generation, fp)
        }
        Err(_) => (0, String::new()),
    }
}

fn write_generation_state(home: &Path, generation: u64, fingerprint: &str) -> Result<(), String> {
    let path = home.join(GENERATION_REL);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create generation parent: {e}"))?;
    }
    static NONCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let nonce = NONCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = path.with_extension(format!("tmp.{}.{}", std::process::id(), nonce));
    {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut f = options
            .open(&tmp)
            .map_err(|e| format!("create generation temp: {e}"))?;
        write!(f, "{generation}\n{fingerprint}\n")
            .map_err(|e| format!("write generation temp: {e}"))?;
        f.flush()
            .map_err(|e| format!("flush generation temp: {e}"))?;
        f.sync_all()
            .map_err(|e| format!("sync generation temp: {e}"))?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = fs::metadata(&tmp).map_err(|e| format!("stat generation temp: {e}"))?;
        let mut perms = meta.permissions();
        if perms.mode() & 0o777 != 0o600 {
            perms.set_mode(0o600);
            fs::set_permissions(&tmp, perms).map_err(|e| format!("chmod generation temp: {e}"))?;
        }
    }
    fs::rename(&tmp, &path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("rename generation sidecar: {e}")
    })?;
    Ok(())
}

fn err_result(
    generation: RegistryGeneration,
    msg: String,
    operation_id: Option<String>,
) -> RetrievalMutationResult {
    RetrievalMutationResult {
        ok: false,
        generation,
        error: Some(msg),
        stale: false,
        guidance: None,
        conflict: None,
        changed_fields: Vec::new(),
        operation_id,
        memory_reindex: None,
        snapshot: None,
    }
}

fn stale_result(
    client: RegistryGeneration,
    live: RegistryGeneration,
    changed_fields: Vec<String>,
    msg: String,
    operation_id: Option<String>,
) -> RetrievalMutationResult {
    RetrievalMutationResult {
        ok: false,
        generation: live,
        error: Some(msg),
        stale: true,
        guidance: Some(STALE_GUIDANCE.into()),
        conflict: Some(RetrievalConflictInfo {
            client_generation: client,
            live_generation: live,
            changed_fields: changed_fields.clone(),
            guidance: "Retrieval generation is stale. Choose Reload to discard local edits, or \
                       Clone into a new id."
                .into(),
        }),
        changed_fields: Vec::new(),
        operation_id,
        memory_reindex: None,
        snapshot: None,
    }
}

// Make WarningTarget::label accessible — it's pub(crate) on the parse module.
trait WarningLabel {
    fn label(&self) -> String;
}
impl WarningLabel for crate::agent::config_model_override_parse::WarningTarget {
    fn label(&self) -> String {
        // Duplicate of pub(crate) label for management summary strings.
        match self {
            Self::ModelSection => "model".to_owned(),
            Self::Model { key, .. } => format!("model.\"{key}\""),
            Self::AuthProviderSection => "auth_provider".to_owned(),
            Self::AuthProvider { name, .. } => format!("auth_provider.\"{name}\""),
            Self::ModelProviderSection => "model_providers".to_owned(),
            Self::ModelProvider { id, .. } => format!("model_providers.\"{id}\""),
            Self::EmbeddingModelsSection => "embedding_models".to_owned(),
            Self::EmbeddingModel { id, .. } => format!("embedding_models.\"{id}\""),
            Self::RerankerModelsSection => "reranker_models".to_owned(),
            Self::RerankerModel { id, .. } => format!("reranker_models.\"{id}\""),
            Self::RetrievalProfilesSection => "retrieval_profiles".to_owned(),
            Self::RetrievalProfile { id, .. } => format!("retrieval_profiles.\"{id}\""),
            Self::PrimeSection => "prime".to_owned(),
            Self::Prime { consumer, .. } => format!("prime.{consumer}"),
            Self::MemoryRetrieval { .. } => "memory".to_owned(),
            Self::LanguageSection => "language".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider_registry::ProviderId;
    use crate::provider_registry::toml_edit::{ProviderTomlPatch, upsert_provider};
    use tempfile::TempDir;
    use xai_grok_config_types::{EmbeddingModelConfig, RetrievalProfileConfig};

    fn svc(dir: &TempDir) -> RetrievalManagementService {
        RetrievalManagementService::new(dir.path())
    }

    fn seed_provider(dir: &TempDir, id: &str) {
        let pid = ProviderId::new(id).unwrap();
        let path = dir.path().join("config.toml");
        upsert_provider(
            &path,
            &pid,
            &ProviderTomlPatch {
                kind: Some("openai_compatible".into()),
                base_url: Some("http://127.0.0.1:9/v1".into()),
                enabled: Some(true),
                capabilities: Some(IndexMap::from([
                    ("embeddings".into(), true),
                    ("rerank".into(), true),
                ])),
                capability_mode: Some("manual".into()),
                ..Default::default()
            },
            true,
        )
        .unwrap();
    }

    #[test]
    fn empty_snapshot_is_valid() {
        let dir = TempDir::new().unwrap();
        let s = svc(&dir);
        let snap = s.graph_snapshot().unwrap();
        assert!(snap.is_valid);
        assert_eq!(snap.generation.get(), 0);
    }

    #[test]
    fn upsert_and_reload_roundtrip() {
        let dir = TempDir::new().unwrap();
        seed_provider(&dir, "lab");
        let s = svc(&dir);
        let g0 = s.current_generation();
        let r = s.upsert_embedding(UpsertEmbeddingRequest {
            expected_generation: g0,
            id: "e1".into(),
            config: EmbeddingModelConfig {
                provider: "lab".into(),
                model: "text-embedding-3-small".into(),
                dimensions: Some(1536),
                ..Default::default()
            },
            confirm_memory_reindex: false,
            operation_id: Some("op1".into()),
        });
        assert!(r.ok, "{:?}", r.error);
        assert_eq!(r.operation_id.as_deref(), Some("op1"));
        let snap = s.graph_snapshot().unwrap();
        assert_eq!(snap.embedding_models.len(), 1);
        assert_eq!(snap.embedding_models[0].id, "e1");
        assert_eq!(snap.generation.get(), r.generation.get());
    }

    #[test]
    fn stale_save_fails_closed() {
        let dir = TempDir::new().unwrap();
        seed_provider(&dir, "lab");
        let s = svc(&dir);
        let g0 = s.current_generation();
        assert!(
            s.upsert_embedding(UpsertEmbeddingRequest {
                expected_generation: g0,
                id: "e1".into(),
                config: EmbeddingModelConfig {
                    provider: "lab".into(),
                    model: "m".into(),
                    ..Default::default()
                },
                confirm_memory_reindex: false,
                operation_id: None,
            })
            .ok
        );
        // Second client with stale generation.
        let r = s.upsert_embedding(UpsertEmbeddingRequest {
            expected_generation: g0,
            id: "e2".into(),
            config: EmbeddingModelConfig {
                provider: "lab".into(),
                model: "m2".into(),
                ..Default::default()
            },
            confirm_memory_reindex: false,
            operation_id: Some("stale-op".into()),
        });
        assert!(!r.ok);
        assert!(r.stale);
        assert!(r.conflict.is_some());
        assert_eq!(r.operation_id.as_deref(), Some("stale-op"));
    }

    #[test]
    fn missing_provider_rejected_on_save() {
        let dir = TempDir::new().unwrap();
        let s = svc(&dir);
        let g0 = s.current_generation();
        let r = s.upsert_embedding(UpsertEmbeddingRequest {
            expected_generation: g0,
            id: "e1".into(),
            config: EmbeddingModelConfig {
                provider: "does-not-exist".into(),
                model: "m".into(),
                ..Default::default()
            },
            confirm_memory_reindex: false,
            operation_id: None,
        });
        assert!(!r.ok);
        assert!(
            r.error.as_deref().unwrap_or("").contains("not registered")
                || r.error.as_deref().unwrap_or("").contains("validation"),
            "{:?}",
            r.error
        );
    }

    #[test]
    fn clone_embedding() {
        let dir = TempDir::new().unwrap();
        seed_provider(&dir, "lab");
        let s = svc(&dir);
        let g0 = s.current_generation();
        assert!(
            s.upsert_embedding(UpsertEmbeddingRequest {
                expected_generation: g0,
                id: "e1".into(),
                config: EmbeddingModelConfig {
                    provider: "lab".into(),
                    model: "m".into(),
                    ..Default::default()
                },
                confirm_memory_reindex: false,
                operation_id: None,
            })
            .ok
        );
        let g1 = s.current_generation();
        let r = s.clone_entity(CloneRetrievalEntityRequest {
            expected_generation: g1,
            kind: "embedding".into(),
            source_id: "e1".into(),
            new_id: "e1-copy".into(),
            confirm_memory_reindex: false,
            operation_id: None,
        });
        assert!(r.ok, "{:?}", r.error);
        let snap = s.graph_snapshot().unwrap();
        assert_eq!(snap.embedding_models.len(), 2);
    }

    #[test]
    fn delete_blocked_when_profile_references() {
        let dir = TempDir::new().unwrap();
        seed_provider(&dir, "lab");
        let s = svc(&dir);
        let g0 = s.current_generation();
        assert!(
            s.upsert_embedding(UpsertEmbeddingRequest {
                expected_generation: g0,
                id: "e1".into(),
                config: EmbeddingModelConfig {
                    provider: "lab".into(),
                    model: "m".into(),
                    ..Default::default()
                },
                confirm_memory_reindex: false,
                operation_id: None,
            })
            .ok
        );
        let g1 = s.current_generation();
        assert!(
            s.upsert_profile(UpsertProfileRequest {
                expected_generation: g1,
                id: "p1".into(),
                config: RetrievalProfileConfig {
                    embedding_models: vec!["e1".into()],
                    max_candidates: 20,
                    max_results: 5,
                    ..Default::default()
                },
                confirm_memory_reindex: false,
                operation_id: None,
            })
            .ok
        );
        let g2 = s.current_generation();
        let r = s.delete_entity(DeleteRetrievalEntityRequest {
            expected_generation: g2,
            kind: "embedding".into(),
            id: "e1".into(),
            confirm_memory_reindex: false,
            operation_id: None,
        });
        assert!(!r.ok);
        assert!(r.error.as_deref().unwrap_or("").contains("referenced"));
    }

    #[test]
    fn memory_reindex_confirmation_required() {
        let dir = TempDir::new().unwrap();
        seed_provider(&dir, "lab");
        let s = svc(&dir);
        let g0 = s.current_generation();
        assert!(
            s.upsert_embedding(UpsertEmbeddingRequest {
                expected_generation: g0,
                id: "e1".into(),
                config: EmbeddingModelConfig {
                    provider: "lab".into(),
                    model: "m".into(),
                    dimensions: Some(1024),
                    ..Default::default()
                },
                confirm_memory_reindex: false,
                operation_id: None,
            })
            .ok
        );
        let g1 = s.current_generation();
        assert!(
            s.upsert_profile(UpsertProfileRequest {
                expected_generation: g1,
                id: "p1".into(),
                config: RetrievalProfileConfig {
                    embedding_models: vec!["e1".into()],
                    ..Default::default()
                },
                confirm_memory_reindex: false,
                operation_id: None,
            })
            .ok
        );
        let g2 = s.current_generation();
        // First set memory profile (no prior → may or may not need confirm).
        let r = s.save_memory_profile(g2, Some("p1".into()), false, None);
        // From none → profile is a fingerprint change.
        if !r.ok {
            assert!(r.memory_reindex.is_some());
            let g2b = s.current_generation();
            let r2 = s.save_memory_profile(g2b, Some("p1".into()), true, None);
            assert!(r2.ok, "{:?}", r2.error);
            // Confirm no reindex side effect: no extra files under memory.
            assert!(
                !dir.path().join("memory").exists()
                    || fs::read_dir(dir.path().join("memory"))
                        .map(|d| d.count() == 0)
                        .unwrap_or(true)
            );
        }
    }

    #[test]
    fn preview_is_network_free() {
        let dir = TempDir::new().unwrap();
        let s = svc(&dir);
        let p = s.preview("embedding", "e1", Some("prev-1".into()));
        assert!(p.messages.iter().any(|m| m.contains("No provider")));
        assert_eq!(p.operation_id.as_deref(), Some("prev-1"));
    }

    #[test]
    fn restart_reconstructs_from_disk() {
        let dir = TempDir::new().unwrap();
        seed_provider(&dir, "lab");
        let s1 = svc(&dir);
        let g0 = s1.current_generation();
        assert!(
            s1.upsert_embedding(UpsertEmbeddingRequest {
                expected_generation: g0,
                id: "e1".into(),
                config: EmbeddingModelConfig {
                    provider: "lab".into(),
                    model: "m".into(),
                    ..Default::default()
                },
                confirm_memory_reindex: false,
                operation_id: None,
            })
            .ok
        );
        // New service instance = restart reconstruction.
        let s2 = svc(&dir);
        let snap = s2.graph_snapshot().unwrap();
        assert_eq!(snap.embedding_models.len(), 1);
        assert!(snap.generation.get() > 0);
    }

    #[test]
    fn none_to_profile_requires_confirm_and_confirmed_writes() {
        let dir = TempDir::new().unwrap();
        seed_provider(&dir, "lab");
        let s = svc(&dir);
        let g0 = s.current_generation();
        assert!(
            s.upsert_embedding(UpsertEmbeddingRequest {
                expected_generation: g0,
                id: "e1".into(),
                config: EmbeddingModelConfig {
                    provider: "lab".into(),
                    model: "m".into(),
                    dimensions: Some(1024),
                    ..Default::default()
                },
                confirm_memory_reindex: false,
                operation_id: Some("op1".into()),
            })
            .ok
        );
        let g1 = s.current_generation();
        assert!(
            s.upsert_profile(UpsertProfileRequest {
                expected_generation: g1,
                id: "p1".into(),
                config: RetrievalProfileConfig {
                    embedding_models: vec!["e1".into()],
                    ..Default::default()
                },
                confirm_memory_reindex: false,
                operation_id: Some("op2".into()),
            })
            .ok
        );
        let g2 = s.current_generation();
        let denied = s.save_memory_profile(g2, Some("p1".into()), false, Some("op3".into()));
        assert!(!denied.ok, "none→profile must require confirm");
        assert!(
            denied
                .memory_reindex
                .as_ref()
                .unwrap()
                .requires_confirmation
        );
        // Disk unchanged.
        assert!(
            s.graph_snapshot()
                .unwrap()
                .memory_retrieval_profile
                .is_none()
        );
        // Confirmed with exact draft profile writes.
        let ok = s.save_memory_profile(g2, Some("p1".into()), true, Some("op4".into()));
        assert!(ok.ok, "{:?}", ok.error);
        assert_eq!(
            s.graph_snapshot()
                .unwrap()
                .memory_retrieval_profile
                .as_deref(),
            Some("p1")
        );
        // No reindex side effect.
        assert!(
            !dir.path().join("memory").exists()
                || fs::read_dir(dir.path().join("memory"))
                    .map(|d| d.count() == 0)
                    .unwrap_or(true)
        );
    }

    #[test]
    fn embedding_dimension_change_under_memory_profile_requires_confirm() {
        let dir = TempDir::new().unwrap();
        seed_provider(&dir, "lab");
        let s = svc(&dir);
        let g0 = s.current_generation();
        assert!(
            s.upsert_embedding(UpsertEmbeddingRequest {
                expected_generation: g0,
                id: "e1".into(),
                config: EmbeddingModelConfig {
                    provider: "lab".into(),
                    model: "m".into(),
                    dimensions: Some(1024),
                    ..Default::default()
                },
                confirm_memory_reindex: false,
                operation_id: None,
            })
            .ok
        );
        let g1 = s.current_generation();
        assert!(
            s.upsert_profile(UpsertProfileRequest {
                expected_generation: g1,
                id: "p1".into(),
                config: RetrievalProfileConfig {
                    embedding_models: vec!["e1".into()],
                    ..Default::default()
                },
                confirm_memory_reindex: false,
                operation_id: None,
            })
            .ok
        );
        let g2 = s.current_generation();
        assert!(s.save_memory_profile(g2, Some("p1".into()), true, None).ok);
        let g3 = s.current_generation();
        let denied = s.upsert_embedding(UpsertEmbeddingRequest {
            expected_generation: g3,
            id: "e1".into(),
            config: EmbeddingModelConfig {
                provider: "lab".into(),
                model: "m".into(),
                dimensions: Some(1536),
                ..Default::default()
            },
            confirm_memory_reindex: false,
            operation_id: Some("dim".into()),
        });
        assert!(!denied.ok);
        assert!(
            denied
                .memory_reindex
                .as_ref()
                .unwrap()
                .requires_confirmation
        );
        // Disk dimensions unchanged.
        assert_eq!(
            s.graph_snapshot().unwrap().embedding_models[0]
                .config
                .dimensions,
            Some(1024)
        );
        // Confirmed exact draft applies.
        let ok = s.upsert_embedding(UpsertEmbeddingRequest {
            expected_generation: g3,
            id: "e1".into(),
            config: EmbeddingModelConfig {
                provider: "lab".into(),
                model: "m".into(),
                dimensions: Some(1536),
                ..Default::default()
            },
            confirm_memory_reindex: true,
            operation_id: Some("dim-ok".into()),
        });
        assert!(ok.ok, "{:?}", ok.error);
        assert_eq!(
            s.graph_snapshot().unwrap().embedding_models[0]
                .config
                .dimensions,
            Some(1536)
        );
    }
}
