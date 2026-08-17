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
    /// When true, semantic degradation becomes a hard orchestrator error.
    pub hard_error_on_semantic_failure: bool,
    /// When true, skip embedding/rerank entirely (native/hard-pinned path).
    pub bypass_semantic: bool,
    /// Optional generation pin; mismatch fails closed.
    pub pin_snapshot_generation: Option<u64>,
    /// When true, candidate/result over-limit returns [`OrchestratorError::LimitExceeded`]
    /// instead of soft truncation. Default soft-clamps to profile limits.
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

    budget.check_batch_documents(texts.len())?;
    let bytes = total_input_bytes(texts.iter().map(|s| s.as_str()));
    budget.charge_input(bytes, RetrievalStage::Embed)?;

    if cancel.is_cancelled() {
        return Err(OrchestratorError::Cancelled {
            profile_id: ctx.profile.id.clone(),
            stage: RetrievalStage::Embed,
        });
    }

    let route_ids = &ctx.profile.embedding_route_ids;
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

        let pins = RouteCallPins {
            provenance_incarnation: route.incarnation.clone(),
            session_registry_generation: Some(ctx.snapshot.provider_generation).filter(|&g| g > 0),
            total_deadline: Some(effective),
        };

        let started = ctx.clock.now();
        let call = ctx.executor.embed(
            ctx.home,
            route_id,
            &route.config,
            &pins,
            texts.clone(),
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
                // First successful route pins this embedding space only.
                // Failed routes never retain vectors — no cross-space merge.
                ctx.cooldown.record_success(&cd_key);
                let out_bytes = result
                    .vectors
                    .first()
                    .map(|v| {
                        v.values
                            .len()
                            .saturating_mul(result.vectors.len())
                            .saturating_mul(4)
                    })
                    .unwrap_or(0);
                // Fail closed on output/response budget overflow (OI-3/OI-4).
                budget.charge_output_bytes(out_bytes)?;

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
                    Some(result.vectors.len() as u32),
                );
                return Ok(EmbedStageResult {
                    route_model_id: route_id.clone(),
                    provider_instance_id: route.provider_instance_id.clone(),
                    embedding_space: route.embedding_space.clone(),
                    result,
                    route_index: idx as u32,
                    attempts_used: budget.attempts_used,
                    degradation: None,
                });
            }
            Err(err) => {
                let class = RouteFailureClass::from_retrieval_error(&err);
                last_failure = Some(class);
                // Auth/config permanent failures: skip route, no cooldown.
                // Retryable classes may enter exact-route cooldown.
                ctx.cooldown.record_failure(cd_key, class);
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
                if !class.allows_explicit_fallback() {
                    break;
                }
                // Terminal-for-route (auth/config/capability/guard/malformed)
                // still may advance to the next explicitly configured route.
                let _ = class.is_terminal_for_route();
                continue;
            }
        }
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
pub async fn rerank_with_profile(
    ctx: &PipelineContext<'_>,
    query: String,
    documents: Vec<String>,
    options: &PipelineOptions,
    cancel: CancellationToken,
    budget: &mut ProfileBudgetTracker,
) -> OrchestratorResult<RerankStageResult> {
    let _ = options;
    let route_ids = &ctx.profile.reranker_route_ids;
    if route_ids.is_empty() {
        return Ok(RerankStageResult {
            route_model_id: None,
            result: None,
            preserved_pre_rerank_order: true,
            degradation: None,
        });
    }

    let shortlist_n = budget.clamp_rerank_shortlist(documents.len());
    let documents: Vec<String> = documents.into_iter().take(shortlist_n).collect();
    let bytes = query.len() + total_input_bytes(documents.iter().map(|s| s.as_str()));
    if let Err(_e) = budget.charge_input(bytes, RetrievalStage::Rerank) {
        return Ok(RerankStageResult {
            route_model_id: None,
            result: None,
            preserved_pre_rerank_order: true,
            degradation: Some(DegradationNotice::new(
                DegradationKind::BudgetExhausted,
                &ctx.profile.id,
                RetrievalStage::Rerank,
                None,
            )),
        });
    }

    let mut last_failure: Option<RouteFailureClass> = None;
    let top_n = Some(budget.limits.max_results);

    for (idx, route_id) in route_ids.iter().enumerate() {
        if cancel.is_cancelled() {
            return Err(OrchestratorError::Cancelled {
                profile_id: ctx.profile.id.clone(),
                stage: RetrievalStage::Rerank,
            });
        }
        if budget
            .ensure_not_expired(ctx.clock.now(), RetrievalStage::Rerank)
            .is_err()
            || budget.attempts_remaining() == 0
        {
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
            break;
        }
        if budget.consume_attempt(RetrievalStage::Rerank).is_err() {
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
                ctx.cooldown.record_success(&cd_key);
                // Approximate output as hit count * 32 bytes for budget accounting.
                let out_bytes = result.hits.len().saturating_mul(32);
                if let Err(e) = budget.charge_output_bytes(out_bytes) {
                    return Err(e);
                }
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
                let _ = class.is_terminal_for_route();
                continue;
            }
        }
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

/// Soft embed that maps total failure to a degradation notice for retrieve.
/// Uses the shared budget so attempts/deadline/input survive into rerank.
pub async fn embed_or_degrade(
    ctx: &PipelineContext<'_>,
    texts: Vec<String>,
    options: &PipelineOptions,
    cancel: CancellationToken,
    budget: &mut ProfileBudgetTracker,
) -> (Option<EmbedStageResult>, Option<DegradationNotice>, bool) {
    // Third value: true when cancelled (caller must hard-stop).
    match embed_with_profile(ctx, texts, options, cancel, budget).await {
        Ok(r) => (Some(r), None, false),
        Err(OrchestratorError::Cancelled { .. }) => (None, None, true),
        Err(OrchestratorError::AllRoutesFailed {
            profile_id,
            last_failure,
            ..
        }) => (
            None,
            Some(DegradationNotice::new(
                DegradationKind::SemanticUnavailable,
                profile_id,
                RetrievalStage::Embed,
                last_failure,
            )),
            false,
        ),
        Err(OrchestratorError::DeadlineExceeded { profile_id, .. })
        | Err(OrchestratorError::AttemptBudgetExceeded { profile_id, .. })
        | Err(OrchestratorError::InputBudgetExceeded { profile_id, .. })
        | Err(OrchestratorError::OutputBudgetExceeded { profile_id, .. }) => (
            None,
            Some(DegradationNotice::new(
                DegradationKind::BudgetExhausted,
                profile_id,
                RetrievalStage::Embed,
                None,
            )),
            false,
        ),
        Err(_) => (
            None,
            Some(DegradationNotice::new(
                DegradationKind::SemanticUnavailable,
                &ctx.profile.id,
                RetrievalStage::Embed,
                None,
            )),
            false,
        ),
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
