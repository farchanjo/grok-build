//! [`RetrievalService`]: reusable metadata/document retrieval APIs.
//!
//! Suitable for PR18 skill inventory and PR21 memory without owning storage.
//! Loads an Arc snapshot per call so in-flight work retains the old snapshot
//! while the next call sees a newly published one.
//!
//! One profile-wide [`ProfileBudgetTracker`] spans embed and rerank for
//! `retrieve`, preserving absolute deadline, attempts, and input/output budgets.
//!
//! **Input budget semantics:** the tracker charges **per upstream request
//! payload**. When both embed and rerank run, the query text is charged once
//! for the embedding request and again for the rerank request (plus documents),
//! because each HTTP call carries that payload independently.

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use super::bounds::ProfileBudgetTracker;
use super::clients::{Pr16RetrievalExecutor, RetrievalExecutor};
use super::error::{
    DegradationKind, DegradationNotice, OrchestratorError, OrchestratorResult, RetrievalStage,
};
use super::graph::RetrievalSnapshot;
use super::pipeline::{
    CandidateRow, EmbedStageResult, PipelineContext, PipelineOptions, RerankStageResult,
    RetrieveResult, embed_or_degrade, embed_with_profile, rerank_with_profile,
};
use super::registry::RetrievalRegistry;
use super::telemetry::{TelemetrySink, TracingTelemetrySink};

/// Callback that supplies local/lexical candidate rows (storage-agnostic).
pub type CandidateProvider = Arc<dyn Fn(&str) -> Vec<CandidateRow> + Send + Sync>;

/// Soft upper bound when profile is missing/disabled under bypass (safety net).
const BYPASS_FALLBACK_MAX_RESULTS: usize = 10_000;

/// Shell-facing retrieval service (cloneable Arc registry).
#[derive(Clone)]
pub struct RetrievalService {
    registry: Arc<RetrievalRegistry>,
    executor: Arc<dyn RetrievalExecutor>,
    telemetry: Arc<dyn TelemetrySink>,
    home: PathBuf,
}

impl std::fmt::Debug for RetrievalService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RetrievalService")
            .field("generation", &self.registry.generation())
            .field("home", &self.home)
            .finish()
    }
}

impl RetrievalService {
    pub fn new(registry: Arc<RetrievalRegistry>) -> Self {
        let home = registry.home().to_path_buf();
        Self {
            registry,
            executor: Arc::new(Pr16RetrievalExecutor),
            telemetry: Arc::new(TracingTelemetrySink),
            home,
        }
    }

    pub fn with_executor(mut self, executor: Arc<dyn RetrievalExecutor>) -> Self {
        self.executor = executor;
        self
    }

    pub fn with_telemetry(mut self, telemetry: Arc<dyn TelemetrySink>) -> Self {
        self.telemetry = telemetry;
        self
    }

    pub fn registry(&self) -> &Arc<RetrievalRegistry> {
        &self.registry
    }

    /// Load current snapshot (in-flight holds this Arc independently).
    pub fn load_snapshot(&self) -> Arc<RetrievalSnapshot> {
        self.registry.load()
    }

    pub fn generation(&self) -> u64 {
        self.registry.generation()
    }

    pub fn is_enabled(&self) -> bool {
        self.registry.load().enabled
    }

    fn resolve_profile<'a>(
        &self,
        snapshot: &'a RetrievalSnapshot,
        profile_id: &str,
    ) -> OrchestratorResult<&'a super::graph::SnapshotProfile> {
        if !snapshot.enabled {
            return Err(OrchestratorError::ServiceDisabled);
        }
        snapshot
            .profile(profile_id)
            .ok_or_else(|| OrchestratorError::ProfileMissing {
                profile_id: profile_id.to_owned(),
            })
    }

    fn ctx<'a>(
        &'a self,
        snapshot: &'a Arc<RetrievalSnapshot>,
        profile: &'a super::graph::SnapshotProfile,
    ) -> PipelineContext<'a> {
        PipelineContext {
            home: &self.home,
            snapshot: snapshot.clone(),
            profile,
            executor: self.executor.clone(),
            cooldown: self.registry.cooldown(),
            clock: self.registry.clock(),
            telemetry: self.telemetry.clone(),
        }
    }

    /// Embed query/document texts in bounded batches via a named profile.
    pub async fn embed(
        &self,
        profile_id: &str,
        texts: Vec<String>,
        options: PipelineOptions,
        cancel: CancellationToken,
    ) -> OrchestratorResult<EmbedStageResult> {
        let snapshot = self.registry.load();
        let profile = self.resolve_profile(&snapshot, profile_id)?;
        let ctx = self.ctx(&snapshot, profile);
        let mut budget = ProfileBudgetTracker::new(
            &profile.id,
            profile.budgets.clone(),
            self.registry.clock().now(),
        );
        embed_with_profile(&ctx, texts, &options, cancel, &mut budget).await
    }

    /// Optional rerank of a bounded shortlist via a named profile.
    pub async fn rerank(
        &self,
        profile_id: &str,
        query: String,
        documents: Vec<String>,
        options: PipelineOptions,
        cancel: CancellationToken,
    ) -> OrchestratorResult<RerankStageResult> {
        let snapshot = self.registry.load();
        let profile = self.resolve_profile(&snapshot, profile_id)?;
        let ctx = self.ctx(&snapshot, profile);
        let mut budget = ProfileBudgetTracker::new(
            &profile.id,
            profile.budgets.clone(),
            self.registry.clock().now(),
        );
        rerank_with_profile(&ctx, query, documents, &options, cancel, &mut budget).await
    }

    /// Generic orchestration: candidates from callback or explicit rows.
    ///
    /// **Budget continuity:** a single [`ProfileBudgetTracker`] covers embed
    /// and rerank for absolute deadline, attempts, and input/output limits.
    ///
    /// **Hard semantic mode:** when `hard_error_on_semantic_failure` is set,
    /// typed deadline/attempt/input/output errors and `AllRoutesFailed`
    /// propagate as their original variants for **both embed and rerank**
    /// stages (not collapsed). Soft mode maps them to degradation notices so
    /// lexical / pre-rerank order can continue.
    ///
    /// **Cancellation:** all cancel paths from `retrieve` surface
    /// [`OrchestratorError::Cancelled`] with
    /// [`RetrievalStage::Orchestrate`] (stable public contract).
    pub async fn retrieve(
        &self,
        profile_id: &str,
        query: &str,
        candidates: RetrieveCandidates,
        options: PipelineOptions,
        cancel: CancellationToken,
    ) -> OrchestratorResult<RetrieveResult> {
        let snapshot = self.registry.load();
        if let Some(pin) = options.pin_snapshot_generation
            && pin != snapshot.generation
        {
            return Err(OrchestratorError::GenerationMismatch {
                expected: pin,
                live: snapshot.generation,
            });
        }

        if options.bypass_semantic {
            let mut rows = candidates.into_rows(query);
            let profile = match self.resolve_profile(&snapshot, profile_id) {
                Ok(p) => p,
                Err(OrchestratorError::ServiceDisabled)
                | Err(OrchestratorError::ProfileMissing { .. }) => {
                    // Always bound bypass output even when profile is missing.
                    rows.truncate(BYPASS_FALLBACK_MAX_RESULTS.min(rows.len()));
                    return Ok(RetrieveResult {
                        candidates: rows,
                        embedding_space: None,
                        embed: None,
                        rerank: None,
                        degradations: vec![DegradationNotice::new(
                            DegradationKind::ServiceDisabled,
                            profile_id,
                            RetrievalStage::Orchestrate,
                            None,
                        )],
                        snapshot_generation: snapshot.generation,
                    });
                }
                Err(e) => return Err(e),
            };
            rows.truncate(profile.budgets.max_results as usize);
            return Ok(RetrieveResult {
                candidates: rows,
                embedding_space: None,
                embed: None,
                rerank: None,
                degradations: Vec::new(),
                snapshot_generation: snapshot.generation,
            });
        }

        let profile = self.resolve_profile(&snapshot, profile_id)?;
        let ctx = self.ctx(&snapshot, profile);
        let mut budget = ProfileBudgetTracker::new(
            &profile.id,
            profile.budgets.clone(),
            self.registry.clock().now(),
        );
        let mut degradations = Vec::new();

        let mut rows = candidates.into_rows(query);
        // Limit overflow uses only hard_error_on_limit_exceeded (decoupled
        // from hard_error_on_semantic_failure).
        if rows.len() > profile.budgets.max_candidates as usize {
            if options.hard_error_on_limit_exceeded {
                budget.check_candidate_limit(rows.len())?;
            } else {
                rows.truncate(budget.clamp_candidates(rows.len()));
            }
        }

        let (embed, embed_deg) = match embed_or_degrade(
            &ctx,
            vec![query.to_owned()],
            &options,
            cancel.child_token(),
            &mut budget,
        )
        .await
        {
            // Public retrieve cancel contract: always Orchestrate (Issue 11).
            Err(OrchestratorError::Cancelled { profile_id, .. }) => {
                return Err(OrchestratorError::Cancelled {
                    profile_id,
                    stage: RetrievalStage::Orchestrate,
                });
            }
            other => other?,
        };
        if cancel.is_cancelled() {
            return Err(OrchestratorError::Cancelled {
                profile_id: profile_id.to_owned(),
                stage: RetrievalStage::Orchestrate,
            });
        }
        if let Some(d) = embed_deg {
            // Soft mode only (hard mode already returned Err from embed_or_degrade).
            degradations.push(d);
        }

        let embedding_space = embed.as_ref().map(|e| e.embedding_space.clone());

        let mut rerank_out = None;
        if !rows.is_empty() && !profile.reranker_route_ids.is_empty() {
            let docs: Vec<String> = rows.iter().map(|r| r.text.clone()).collect();
            match rerank_with_profile(
                &ctx,
                query.to_owned(),
                docs,
                &options,
                cancel.child_token(),
                &mut budget,
            )
            .await
            {
                Ok(rr) => {
                    if let Some(ref d) = rr.degradation {
                        degradations.push(d.clone());
                    }
                    if let Some(ref result) = rr.result {
                        let mut reordered = Vec::with_capacity(rows.len());
                        let mut used = vec![false; rows.len()];
                        for hit in &result.hits {
                            if hit.index < rows.len() && !used[hit.index] {
                                let mut row = rows[hit.index].clone();
                                row.score = Some(hit.score);
                                reordered.push(row);
                                used[hit.index] = true;
                            }
                        }
                        for (i, row) in rows.into_iter().enumerate() {
                            if !used[i] {
                                reordered.push(row);
                            }
                        }
                        rows = reordered;
                    }
                    rerank_out = Some(rr);
                }
                Err(OrchestratorError::Cancelled { profile_id, .. }) => {
                    return Err(OrchestratorError::Cancelled {
                        profile_id,
                        stage: RetrievalStage::Orchestrate,
                    });
                }
                // Limit-hard alone: output overflow from rerank (Issue 12).
                Err(e @ OrchestratorError::OutputBudgetExceeded { .. })
                    if options.hard_error_on_limit_exceeded
                        || options.hard_error_on_semantic_failure =>
                {
                    return Err(e);
                }
                // Semantic hard: all typed budget errors from rerank.
                Err(
                    e @ OrchestratorError::DeadlineExceeded { .. }
                    | e @ OrchestratorError::AttemptBudgetExceeded { .. }
                    | e @ OrchestratorError::InputBudgetExceeded { .. },
                ) if options.hard_error_on_semantic_failure => {
                    return Err(e);
                }
                Err(OrchestratorError::DeadlineExceeded { .. })
                | Err(OrchestratorError::AttemptBudgetExceeded { .. })
                | Err(OrchestratorError::InputBudgetExceeded { .. })
                | Err(OrchestratorError::OutputBudgetExceeded { .. }) => {
                    degradations.push(DegradationNotice::new(
                        DegradationKind::BudgetExhausted,
                        profile_id,
                        RetrievalStage::Rerank,
                        None,
                    ));
                }
                Err(e) if options.hard_error_on_semantic_failure => {
                    return Err(e);
                }
                Err(_) => {
                    degradations.push(DegradationNotice::new(
                        DegradationKind::RerankUnavailable,
                        profile_id,
                        RetrievalStage::Rerank,
                        None,
                    ));
                }
            }
        }

        if rows.len() > profile.budgets.max_results as usize {
            if options.hard_error_on_limit_exceeded {
                budget.check_result_limit(rows.len())?;
            } else {
                rows.truncate(budget.clamp_results(rows.len()));
            }
        }

        Ok(RetrieveResult {
            candidates: rows,
            embedding_space,
            embed,
            rerank: rerank_out,
            degradations,
            snapshot_generation: snapshot.generation,
        })
    }
}

/// Candidate source for [`RetrievalService::retrieve`].
pub enum RetrieveCandidates {
    Explicit(Vec<CandidateRow>),
    Provider(CandidateProvider),
}

impl RetrieveCandidates {
    fn into_rows(self, query: &str) -> Vec<CandidateRow> {
        match self {
            Self::Explicit(rows) => rows,
            Self::Provider(p) => p(query),
        }
    }
}
