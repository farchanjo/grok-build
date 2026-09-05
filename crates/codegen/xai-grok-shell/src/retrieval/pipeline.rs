//! Ordered deterministic embedding/rerank fallback pipeline.
//!
//! Iterates only config-declared route IDs in declaration order. No implicit
//! kind/host/sibling fallback. Profile budgets are aggregate across routes
//! and across embed+rerank stages of one retrieve. Cancellation stops
//! outstanding work immediately.

use std::fmt;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use tokio_util::sync::CancellationToken;
use xai_grok_inference::{EmbeddingResult, RerankResult, RetrievalError};

use super::bounds::{ProfileBudgetTracker, total_input_bytes};
use super::clients::{RetrievalExecutor, RouteCallPins};
use super::clock::Clock;
use super::cooldown::{CooldownKey, CooldownTable};
use super::error::{
    DegradationKind, DegradationNotice, OrchestratorError, OrchestratorResult, RetrievalStage,
    RouteFailureClass,
};
use super::graph::{EmbeddingSpaceId, RetrievalSnapshot, SnapshotProfile};
use super::telemetry::{
    BudgetFlags, RetrievalTelemetryEvent, TelemetryOutcome, TelemetrySink, duration_ms,
};

/// Options for a single pipeline invocation.
#[derive(Debug, Clone, Default)]
pub struct PipelineOptions {
    /// When true, semantic degradation (all routes failed / soft budget map)
    /// becomes a hard orchestrator error. Typed deadline/attempt/input/output
    /// budget errors always propagate as their original variants in hard mode
    /// for **both embed and rerank** stages.
    ///
    /// This flag does **not** control candidate/result over-limit behavior;
    /// use [`Self::hard_error_on_limit_exceeded`] for that (decoupled).
    pub hard_error_on_semantic_failure: bool,
    /// When true, skip embedding/rerank entirely (native/hard-pinned path).
    pub bypass_semantic: bool,
    /// Optional generation pin; mismatch fails closed.
    pub pin_snapshot_generation: Option<u64>,
    /// When set, the embed stage iterates **only** this route id (exact-route
    /// pin): no ordered sibling-route fallback. Used by the memory facade so a
    /// fallback can never serve vectors from a different embedding space than
    /// the one the fingerprint describes. Missing/cooldown/config-failed pins
    /// fail closed (degrade) rather than falling through to another route.
    pub embed_route_pin: Option<String>,
    /// When true, candidate/result over-limit returns [`OrchestratorError::LimitExceeded`]
    /// instead of soft truncation. Default soft-clamps to profile limits.
    /// Independent of [`Self::hard_error_on_semantic_failure`].
    ///
    /// When true, candidate/result over-limit and **rerank output-budget**
    /// overflow propagate as typed hard errors even if semantic hard mode is
    /// off. Semantic deadline/attempt/input failures still follow the semantic
    /// hard/soft flag only.
    pub hard_error_on_limit_exceeded: bool,
}

/// Successful embedding stage outcome (one embedding space).
#[derive(Debug, Clone)]
pub struct EmbedStageResult {
    pub route_model_id: String,
    pub provider_instance_id: String,
    pub embedding_space: EmbeddingSpaceId,
    pub result: EmbeddingResult,
    pub route_index: u32,
    pub attempts_used: u32,
    pub degradation: Option<DegradationNotice>,
}

/// Rerank stage outcome.
#[derive(Debug, Clone)]
pub struct RerankStageResult {
    pub route_model_id: Option<String>,
    pub result: Option<RerankResult>,
    /// True when pre-rerank order was preserved (all rerankers failed or none configured).
    pub preserved_pre_rerank_order: bool,
    pub degradation: Option<DegradationNotice>,
}

/// Generic candidate row for orchestration (storage-agnostic).
///
/// Debug never prints full text (document/query-adjacent content).
#[derive(Clone)]
pub struct CandidateRow {
    pub id: String,
    pub text: String,
    pub score: Option<f32>,
    pub metadata: Option<serde_json::Value>,
}

impl fmt::Debug for CandidateRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CandidateRow")
            .field("id", &self.id)
            .field("text_chars", &self.text.len())
            .field("score", &self.score)
            .field("has_metadata", &self.metadata.is_some())
            .finish()
    }
}

/// Result of full retrieve orchestration.
///
/// Debug omits candidate text bodies.
#[derive(Clone)]
pub struct RetrieveResult {
    pub candidates: Vec<CandidateRow>,
    pub embedding_space: Option<EmbeddingSpaceId>,
    pub embed: Option<EmbedStageResult>,
    pub rerank: Option<RerankStageResult>,
    pub degradations: Vec<DegradationNotice>,
    pub snapshot_generation: u64,
}

impl fmt::Debug for RetrieveResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RetrieveResult")
            .field("candidate_count", &self.candidates.len())
            .field("embedding_space", &self.embedding_space)
            .field("embed", &self.embed)
            .field("rerank", &self.rerank)
            .field("degradations", &self.degradations)
            .field("snapshot_generation", &self.snapshot_generation)
            .finish()
    }
}

/// Shared pipeline context.
pub struct PipelineContext<'a> {
    pub home: &'a Path,
    pub snapshot: Arc<RetrievalSnapshot>,
    pub profile: &'a SnapshotProfile,
    pub executor: Arc<dyn RetrievalExecutor>,
    pub cooldown: Arc<CooldownTable>,
    pub clock: Arc<dyn Clock>,
    pub telemetry: Arc<dyn TelemetrySink>,
}

/// Embed texts via ordered embedding routes, charging the shared budget.
///
/// **Input accounting:** charges the full embed request payload (all input
/// strings). In a `retrieve` call the query is typically charged here and
/// again in [`rerank_with_profile`] as part of that upstream request — this is
/// intentional per-request payload accounting, not unique-token dedupe.
pub async fn embed_with_profile(
    ctx: &PipelineContext<'_>,
    texts: Vec<String>,
    options: &PipelineOptions,
    cancel: CancellationToken,
    budget: &mut ProfileBudgetTracker,
) -> OrchestratorResult<EmbedStageResult> {
    if let Some(pin) = options.pin_snapshot_generation
        && pin != ctx.snapshot.generation
    {
        return Err(OrchestratorError::GenerationMismatch {
            expected: pin,
            live: ctx.snapshot.generation,
        });
    }
    if !ctx.snapshot.enabled {
        return Err(OrchestratorError::ServiceDisabled);
    }
    if texts.is_empty() {
        return Err(OrchestratorError::InvalidRequest(
            "embedding inputs must be non-empty".into(),
        ));
    }

    if cancel.is_cancelled() {
        return Err(OrchestratorError::Cancelled {
            profile_id: ctx.profile.id.clone(),
            stage: RetrievalStage::Embed,
        });
    }

    let route_ids = match &options.embed_route_pin {
        // Exact-route pin: iterate only the pinned route; no sibling fallback.
        Some(pin) => {
            if !ctx.profile.embedding_route_ids.iter().any(|id| id == pin) {
                return semantic_fail(ctx, budget, None);
            }
            std::borrow::Cow::Owned(vec![pin.clone()])
        }
        None => std::borrow::Cow::Borrowed(&ctx.profile.embedding_route_ids),
    };
    if route_ids.is_empty() {
        return semantic_fail(ctx, budget, None);
    }

    let mut last_failure: Option<RouteFailureClass> = None;

    for (idx, route_id) in route_ids.iter().enumerate() {
        if cancel.is_cancelled() {
            return Err(OrchestratorError::Cancelled {
                profile_id: ctx.profile.id.clone(),
                stage: RetrievalStage::Embed,
            });
        }
        if let Err(e) = budget.ensure_not_expired(ctx.clock.now(), RetrievalStage::Embed) {
            emit(
                ctx,
                RetrievalStage::Embed,
                Some(idx as u32),
                Some(route_id),
                None,
                budget,
                BudgetFlags {
                    deadline_hit: true,
                    ..Default::default()
                },
                TelemetryOutcome::SkippedBudget,
                None,
                Some(texts.len() as u32),
                None,
            );
            // Preserve typed deadline so soft retrieve maps to BudgetExhausted.
            return Err(e);
        }
        if budget.attempts_remaining() == 0 {
            return Err(OrchestratorError::AttemptBudgetExceeded {
                profile_id: ctx.profile.id.clone(),
                stage: RetrievalStage::Embed,
                max_attempts: budget.limits.max_attempts,
            });
        }

        let Some(route) = ctx.snapshot.embedding_route(route_id) else {
            last_failure = Some(RouteFailureClass::Config);
            continue;
        };

        let cd_key = CooldownKey::for_embedding(
            ctx.snapshot.generation,
            &ctx.profile.id,
            route_id,
            &route.provider_instance_id,
            route.incarnation.as_deref(),
            &route.config.model,
            route.config.protocol.as_str(),
            Some(&route.embedding_space),
        );
        if ctx.cooldown.is_cooling(&cd_key) {
            emit(
                ctx,
                RetrievalStage::Embed,
                Some(idx as u32),
                Some(route_id),
                Some(&route.provider_instance_id),
                budget,
                BudgetFlags {
                    cooldown_skip: true,
                    ..Default::default()
                },
                TelemetryOutcome::SkippedCooldown,
                None,
                Some(texts.len() as u32),
                None,
            );
            last_failure = Some(RouteFailureClass::Cooldown);
            continue;
        }

        // Check effective timeout before consuming an attempt (OI-6).
        let effective = budget.effective_timeout(
            ctx.clock.now(),
            Duration::from_millis(route.request_timeout_ms.max(1)),
        );
        if effective.is_zero() {
            return Err(OrchestratorError::DeadlineExceeded {
                profile_id: ctx.profile.id.clone(),
                stage: RetrievalStage::Embed,
            });
        }

        if budget.consume_attempt(RetrievalStage::Embed).is_err() {
            return Err(OrchestratorError::AttemptBudgetExceeded {
                profile_id: ctx.profile.id.clone(),
                stage: RetrievalStage::Embed,
                max_attempts: budget.limits.max_attempts,
            });
        }

        // Per-request document bound: `max_batch_documents` (from the route
        // batch_size) caps ONE upstream request, not the stage input. Prime
        // staging/backfill and the memory facade submit large stage inputs,
        // so split them into ordered sub-requests sharing this route
        // attempt; each sub-request is count-checked and charged for its
        // own payload (per-request payload accounting).
        let max_docs = budget.limits.max_batch_documents.max(1) as usize;
        let mut merged_model: Option<String> = None;
        let mut merged_vectors: Vec<xai_grok_inference::EmbeddingVector> =
            Vec::with_capacity(texts.len());
        let route_started = ctx.clock.now();
        let mut route_failed = false;
        for chunk in texts.chunks(max_docs) {
            if cancel.is_cancelled() {
                return Err(OrchestratorError::Cancelled {
                    profile_id: ctx.profile.id.clone(),
                    stage: RetrievalStage::Embed,
                });
            }
            budget.check_batch_documents(chunk.len())?;
            budget.charge_input(
                total_input_bytes(chunk.iter().map(|s| s.as_str())),
                RetrievalStage::Embed,
            )?;
            let pins = RouteCallPins {
                provenance_incarnation: route.incarnation.clone(),
                session_registry_generation: Some(ctx.snapshot.provider_generation)
                    .filter(|&g| g > 0),
                total_deadline: Some(effective),
            };

            let started = ctx.clock.now();
            let call = ctx.executor.embed(
                ctx.home,
                route_id,
                &route.config,
                &pins,
                chunk.to_vec(),
                cancel.child_token(),
            );
            let outcome = tokio::select! {
                biased;
                _ = cancel.cancelled() => Err(RetrievalError::Cancelled),
                res = call => res,
            };
            let elapsed = ctx.clock.now().saturating_duration_since(started);

            match outcome {
                Ok(result) => {
                    // Embedding payloads are `N × dims` floats, not text: the
                    // profile's text-output budget (`max_output_tokens` and its
                    // derived response-byte ceiling) does not apply to them, and
                    // charging it misclassifies every full batch as overflow.
                    // Bound the vector count per request; the transport still
                    // enforces its own response-byte ceiling.
                    budget.check_batch_documents(result.vectors.len())?;
                    if merged_model.is_none() {
                        merged_model = Some(result.model.clone());
                    }
                    merged_vectors.extend(result.vectors);
                }
                Err(err) => {
                    let class = RouteFailureClass::from_retrieval_error(&err);
                    last_failure = Some(class);
                    // Auth/config permanent failures: skip route, no cooldown.
                    // Retryable classes may enter exact-route cooldown.
                    // Cancelled is handled immediately (only non-fallback class).
                    ctx.cooldown.record_failure(cd_key.clone(), class);
                    emit(
                        ctx,
                        RetrievalStage::Embed,
                        Some(idx as u32),
                        Some(route_id),
                        Some(&route.provider_instance_id),
                        budget,
                        BudgetFlags {
                            cancelled: matches!(class, RouteFailureClass::Cancelled),
                            deadline_hit: matches!(class, RouteFailureClass::Deadline),
                            ..Default::default()
                        },
                        TelemetryOutcome::RouteFailure(class),
                        Some(elapsed),
                        Some(texts.len() as u32),
                        None,
                    );
                    if matches!(class, RouteFailureClass::Cancelled) {
                        return Err(OrchestratorError::Cancelled {
                            profile_id: ctx.profile.id.clone(),
                            stage: RetrievalStage::Embed,
                        });
                    }
                    // All remaining classes allow the next explicitly configured route.
                    route_failed = true;
                    break;
                }
            }
        }
        if route_failed {
            continue;
        }
        // First successful route pins this embedding space only. Failed
        // routes never retain vectors — no cross-space merge.
        ctx.cooldown.record_success(&cd_key);

        let elapsed = ctx.clock.now().saturating_duration_since(route_started);
        emit(
            ctx,
            RetrievalStage::Embed,
            Some(idx as u32),
            Some(route_id),
            Some(&route.provider_instance_id),
            budget,
            BudgetFlags::default(),
            TelemetryOutcome::Success,
            Some(elapsed),
            Some(texts.len() as u32),
            Some(merged_vectors.len() as u32),
        );
        return Ok(EmbedStageResult {
            route_model_id: route_id.clone(),
            provider_instance_id: route.provider_instance_id.clone(),
            embedding_space: route.embedding_space.clone(),
            result: EmbeddingResult {
                model: merged_model.unwrap_or_else(|| route.config.model.clone()),
                vectors: merged_vectors,
            },
            route_index: idx as u32,
            attempts_used: budget.attempts_used,
            degradation: None,
        });
    }

    semantic_fail(ctx, budget, last_failure)
}

fn semantic_fail(
    ctx: &PipelineContext<'_>,
    budget: &ProfileBudgetTracker,
    last_failure: Option<RouteFailureClass>,
) -> OrchestratorResult<EmbedStageResult> {
    emit(
        ctx,
        RetrievalStage::Embed,
        None,
        None,
        None,
        budget,
        BudgetFlags::default(),
        TelemetryOutcome::Degraded(DegradationKind::SemanticUnavailable),
        None,
        None,
        None,
    );
    Err(OrchestratorError::AllRoutesFailed {
        profile_id: ctx.profile.id.clone(),
        stage: RetrievalStage::Embed,
        last_failure,
    })
}

/// Rerank documents using the shared profile budget; on total failure preserve
/// pre-rerank order exactly.
///
/// **Input accounting:** input budget is **per upstream request payload**.
/// The query string is charged again here (in addition to the embed stage)
/// because the rerank HTTP request independently transmits the query plus
/// documents. This is intentional payload accounting, not unique-token dedupe.
pub async fn rerank_with_profile(
    ctx: &PipelineContext<'_>,
    query: String,
    documents: Vec<String>,
    options: &PipelineOptions,
    cancel: CancellationToken,
    budget: &mut ProfileBudgetTracker,
) -> OrchestratorResult<RerankStageResult> {
    // N-02: enforce the snapshot generation pin at the top of the rerank path
    // too. A stale facade (mid-session reload) fails closed to the exact
    // local pre-rerank order instead of reranking against the live snapshot.
    if let Some(pin) = options.pin_snapshot_generation
        && pin != ctx.snapshot.generation
    {
        return Ok(RerankStageResult {
            route_model_id: None,
            result: None,
            preserved_pre_rerank_order: true,
            degradation: None,
        });
    }

    let hard = options.hard_error_on_semantic_failure;
    let route_ids = &ctx.profile.reranker_route_ids;
    if route_ids.is_empty() {
        return Ok(RerankStageResult {
            route_model_id: None,
            result: None,
            preserved_pre_rerank_order: true,
            degradation: None,
        });
    }

    // Gate deadline/attempts before charging input so an already-exhausted
    // profile does not flip the cause to InputBudgetExceeded (Issues 2–3).
    // Hard mode: propagate typed budget errors (Issue 10). Soft: BudgetExhausted.
    if let Err(e) = budget.ensure_not_expired(ctx.clock.now(), RetrievalStage::Rerank) {
        return rerank_budget_outcome(hard, e, &ctx.profile.id);
    }
    if budget.attempts_remaining() == 0 {
        return rerank_budget_outcome(
            hard,
            OrchestratorError::AttemptBudgetExceeded {
                profile_id: ctx.profile.id.clone(),
                stage: RetrievalStage::Rerank,
                max_attempts: budget.limits.max_attempts,
            },
            &ctx.profile.id,
        );
    }

    let shortlist_n = budget.clamp_rerank_shortlist(documents.len());
    let documents: Vec<String> = documents.into_iter().take(shortlist_n).collect();
    // Per-request payload: query + documents (query may already have been
    // charged in the embed stage of the same retrieve — see module docs).
    let bytes = query.len() + total_input_bytes(documents.iter().map(|s| s.as_str()));
    if let Err(e) = budget.charge_input(bytes, RetrievalStage::Rerank) {
        return rerank_budget_outcome(hard, e, &ctx.profile.id);
    }

    let mut last_failure: Option<RouteFailureClass> = None;
    // Typed error when the loop stops on budget (deadline/attempts).
    let mut hard_budget_err: Option<OrchestratorError> = None;
    let top_n = Some(budget.limits.max_results);

    for (idx, route_id) in route_ids.iter().enumerate() {
        if cancel.is_cancelled() {
            return Err(OrchestratorError::Cancelled {
                profile_id: ctx.profile.id.clone(),
                stage: RetrievalStage::Rerank,
            });
        }
        if let Err(e) = budget.ensure_not_expired(ctx.clock.now(), RetrievalStage::Rerank) {
            hard_budget_err = Some(e);
            break;
        }
        if budget.attempts_remaining() == 0 {
            hard_budget_err = Some(OrchestratorError::AttemptBudgetExceeded {
                profile_id: ctx.profile.id.clone(),
                stage: RetrievalStage::Rerank,
                max_attempts: budget.limits.max_attempts,
            });
            break;
        }
        let Some(route) = ctx.snapshot.reranker_route(route_id) else {
            last_failure = Some(RouteFailureClass::Config);
            continue;
        };
        let cd_key = CooldownKey::for_rerank(
            ctx.snapshot.generation,
            &ctx.profile.id,
            route_id,
            &route.provider_instance_id,
            route.incarnation.as_deref(),
            &route.config.model,
            route.config.protocol.as_str(),
        );
        if ctx.cooldown.is_cooling(&cd_key) {
            last_failure = Some(RouteFailureClass::Cooldown);
            continue;
        }
        let effective = budget.effective_timeout(
            ctx.clock.now(),
            Duration::from_millis(route.request_timeout_ms.max(1)),
        );
        if effective.is_zero() {
            last_failure = Some(RouteFailureClass::Deadline);
            hard_budget_err = Some(OrchestratorError::DeadlineExceeded {
                profile_id: ctx.profile.id.clone(),
                stage: RetrievalStage::Rerank,
            });
            break;
        }
        if let Err(e) = budget.consume_attempt(RetrievalStage::Rerank) {
            hard_budget_err = Some(e);
            break;
        }
        let pins = RouteCallPins {
            provenance_incarnation: route.incarnation.clone(),
            session_registry_generation: Some(ctx.snapshot.provider_generation).filter(|&g| g > 0),
            total_deadline: Some(effective),
        };
        let started = ctx.clock.now();
        let call = ctx.executor.rerank(
            ctx.home,
            route_id,
            &route.config,
            &pins,
            query.clone(),
            documents.clone(),
            top_n,
            cancel.child_token(),
        );
        let outcome = tokio::select! {
            biased;
            _ = cancel.cancelled() => Err(RetrievalError::Cancelled),
            res = call => res,
        };
        let elapsed = ctx.clock.now().saturating_duration_since(started);
        match outcome {
            Ok(result) => {
                let out_bytes = result.hits.len().saturating_mul(32);
                // Output overflow: hard when semantic-hard *or* limit-hard
                // (`hard_error_on_limit_exceeded` is independent of semantic hard).
                if let Err(e) = budget.charge_output_bytes(out_bytes) {
                    let hard_output = hard || options.hard_error_on_limit_exceeded;
                    return if hard_output {
                        Err(e)
                    } else {
                        rerank_budget_outcome(false, e, &ctx.profile.id)
                    };
                }
                ctx.cooldown.record_success(&cd_key);
                emit(
                    ctx,
                    RetrievalStage::Rerank,
                    Some(idx as u32),
                    Some(route_id),
                    Some(&route.provider_instance_id),
                    budget,
                    BudgetFlags::default(),
                    TelemetryOutcome::Success,
                    Some(elapsed),
                    Some(documents.len() as u32),
                    Some(result.hits.len() as u32),
                );
                return Ok(RerankStageResult {
                    route_model_id: Some(route_id.clone()),
                    result: Some(result),
                    preserved_pre_rerank_order: false,
                    degradation: None,
                });
            }
            Err(err) => {
                let class = RouteFailureClass::from_retrieval_error(&err);
                last_failure = Some(class);
                ctx.cooldown.record_failure(cd_key, class);
                emit(
                    ctx,
                    RetrievalStage::Rerank,
                    Some(idx as u32),
                    Some(route_id),
                    Some(&route.provider_instance_id),
                    budget,
                    BudgetFlags::default(),
                    TelemetryOutcome::RouteFailure(class),
                    Some(elapsed),
                    Some(documents.len() as u32),
                    None,
                );
                if matches!(class, RouteFailureClass::Cancelled) {
                    return Err(OrchestratorError::Cancelled {
                        profile_id: ctx.profile.id.clone(),
                        stage: RetrievalStage::Rerank,
                    });
                }
                // Next explicitly configured route only.
                continue;
            }
        }
    }

    if let Some(e) = hard_budget_err {
        return rerank_budget_outcome(hard, e, &ctx.profile.id);
    }
    Ok(RerankStageResult {
        route_model_id: None,
        result: None,
        preserved_pre_rerank_order: true,
        degradation: Some(DegradationNotice::new(
            DegradationKind::RerankUnavailable,
            &ctx.profile.id,
            RetrievalStage::Rerank,
            last_failure,
        )),
    })
}

/// Hard mode: return the typed budget error. Soft mode: preserve pre-rerank
/// order with [`DegradationKind::BudgetExhausted`].
fn rerank_budget_outcome(
    hard: bool,
    err: OrchestratorError,
    profile_id: &str,
) -> OrchestratorResult<RerankStageResult> {
    if hard {
        return Err(err);
    }
    Ok(RerankStageResult {
        route_model_id: None,
        result: None,
        preserved_pre_rerank_order: true,
        degradation: Some(DegradationNotice::new(
            DegradationKind::BudgetExhausted,
            profile_id,
            RetrievalStage::Rerank,
            None,
        )),
    })
}

/// Soft embed that maps total failure to a degradation notice for retrieve.
/// Uses the shared budget so attempts/deadline/input survive into rerank.
///
/// When `hard_error_on_semantic_failure` is set, **all** embed errors (including
/// typed deadline/attempt/input/output budget errors) propagate as `Err` so
/// callers retain the original classification (Issue 1).
pub async fn embed_or_degrade(
    ctx: &PipelineContext<'_>,
    texts: Vec<String>,
    options: &PipelineOptions,
    cancel: CancellationToken,
    budget: &mut ProfileBudgetTracker,
) -> Result<(Option<EmbedStageResult>, Option<DegradationNotice>), OrchestratorError> {
    match embed_with_profile(ctx, texts, options, cancel, budget).await {
        Ok(r) => Ok((Some(r), None)),
        Err(e) if options.hard_error_on_semantic_failure => Err(e),
        Err(OrchestratorError::Cancelled { profile_id, stage }) => {
            Err(OrchestratorError::Cancelled { profile_id, stage })
        }
        Err(OrchestratorError::AllRoutesFailed {
            profile_id,
            last_failure,
            ..
        }) => Ok((
            None,
            Some(DegradationNotice::new(
                DegradationKind::SemanticUnavailable,
                profile_id,
                RetrievalStage::Embed,
                last_failure,
            )),
        )),
        Err(OrchestratorError::DeadlineExceeded { profile_id, .. })
        | Err(OrchestratorError::AttemptBudgetExceeded { profile_id, .. })
        | Err(OrchestratorError::InputBudgetExceeded { profile_id, .. })
        | Err(OrchestratorError::OutputBudgetExceeded { profile_id, .. }) => Ok((
            None,
            Some(DegradationNotice::new(
                DegradationKind::BudgetExhausted,
                profile_id,
                RetrievalStage::Embed,
                None,
            )),
        )),
        Err(_) => Ok((
            None,
            Some(DegradationNotice::new(
                DegradationKind::SemanticUnavailable,
                &ctx.profile.id,
                RetrievalStage::Embed,
                None,
            )),
        )),
    }
}

#[allow(clippy::too_many_arguments)]
fn emit(
    ctx: &PipelineContext<'_>,
    stage: RetrievalStage,
    route_index: Option<u32>,
    route_model_id: Option<&str>,
    provider_instance_id: Option<&str>,
    budget: &ProfileBudgetTracker,
    budget_flags: BudgetFlags,
    outcome: TelemetryOutcome,
    duration: Option<Duration>,
    input_count: Option<u32>,
    result_count: Option<u32>,
) {
    ctx.telemetry.emit(RetrievalTelemetryEvent {
        profile_id: ctx.profile.id.clone(),
        stage,
        purpose: stage.as_str(),
        route_index,
        route_model_id: route_model_id.map(str::to_owned),
        provider_instance_id: provider_instance_id.map(str::to_owned),
        snapshot_generation: ctx.snapshot.generation,
        provider_generation: Some(ctx.snapshot.provider_generation),
        attempt: budget.attempts_used,
        max_attempts: budget.limits.max_attempts,
        duration_ms: duration.map(duration_ms),
        input_count,
        result_count,
        budget_flags,
        outcome,
    });
}
