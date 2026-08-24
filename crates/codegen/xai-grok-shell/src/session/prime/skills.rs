//! Deterministic skill selection pipeline for prime (PR18).
//!
//! Takes the eligible native skill snapshot plus the current prompt and a
//! bounded workspace inventory, pins an explicitly invoked skill first, scores
//! exact `when-to-use` / prompt-path / workspace-path evidence deterministically
//! (capped to prevent flooding and `**` self-promotion), optionally refines the
//! **full indexed inventory** through FTS + sqlite-vec KNN and weighted RRF
//! (reranking only a fused shortlist — never a first-eight inventory cap),
//! then revalidates each surviving skill against a **fresh** async snapshot and
//! loads its body through a bounded, TOCTOU-hardened reader.
//!
//! This is the callable selection seam for PR19. It never splices a
//! conversation and performs no prompt injection.
//!
//! Safety/latency invariants:
//! - Only frontmatter-authorized metadata reaches the semantic provider;
//!   body-derived descriptions are never transmitted, and bodies never leave
//!   the process.
//! - Degradation is reported as secret-free kinds; query/body/vector/raw
//!   provider errors never reach Debug/telemetry.
//! - Rerank failures preserve the fused automatic list; KNN/space failures
//!   preserve the exact pre-stage deterministic order (pins first).
//! - Body loading uses the SKILL.md bytes already captured from the no-follow
//!   revalidation fd. The path string is never re-opened or canonicalized for
//!   the primed payload. Containment lstats original (non-followed) components
//!   under trusted roots.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use xai_grok_config_types::SkillPrimeConfig;
use xai_grok_tools::implementations::skills::skill::{
    extract_skill_body, format_skill_name, resolve_skill_internal_links,
};
use xai_grok_tools::implementations::skills::types::{SkillInfo, SkillScope};

use crate::retrieval::{
    DegradationKind, DegradationNotice, OrchestratorError, PipelineOptions, RetrievalService,
};

use super::SkillRefresh;
use super::fusion::{
    automatic_candidate_allowed, fuse_ranks, l2_similarity, prepend_unique, stricter_threshold,
};
use super::index::{
    PinnedServiceEmbedder, bounded_cancel, inventory_generation_token, opaque_skill_id,
    prime_index_for, skill_rerank_document, skills_to_index_items,
};
use super::inventory::WorkspaceInventory;
use super::render::LoadedSkill;

/// Cap on `when-to-use` phrases considered per skill (anti-flood).
const MAX_WTU_PHRASES: usize = 8;
/// Cap on inventory hits counted per skill.
const MAX_INVENTORY_SCORE: i64 = 20;
/// Cap on `paths:` patterns considered per skill (anti-flood).
const MAX_PATH_PATTERNS: usize = 10;
/// Bounded bytes read from a SKILL.md body (render truncates to per-body chars;
/// prevents OOM / FIFO / device hangs).
#[cfg(test)]
const MAX_LOADED_BODY_BYTES: u64 = 64 * 1024;

/// Why a skill was dropped during revalidation/loading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimeDropReason {
    /// No longer present / no longer eligible in the fresh snapshot.
    ChangedOrGone,
    /// Canonical path escapes the trusted containment roots, is a symlink, or is
    /// not a `SKILL.md` (arbitrary files / followed links are never loadable).
    NotContained,
    /// Body load failed (non-regular file, unreadable, vanished mid-read).
    Unreadable,
    /// Fresh strict validation quarantined the skill.
    Quarantined,
}

/// Secret-free selection budget state for PR19 reporting.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PrimeBudgetState {
    /// Final **loaded** skill names (post-revalidation).
    pub selected_names: Vec<String>,
    pub dropped: usize,
    pub drop_reasons: Vec<PrimeDropReason>,
    pub over_result_limit: bool,
}

/// A ranked candidate (internal). Ordering is deterministic and secret-free.
struct Ranked {
    idx: usize,
    primary: i64,
    secondary: i64,
}

/// Normalize a token stream into significant lowercase words.
fn significant_words(text: &str) -> HashSet<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
        .map(|t| t.to_lowercase())
        .filter(|t| t.len() >= 4)
        .collect()
}

fn scope_label(scope: SkillScope) -> &'static str {
    use SkillScope::*;
    match scope {
        Local => "local",
        Repo => "repo",
        User => "user",
        Server => "server",
        Bundled => "bundled",
        Plugin => "plugin",
    }
}

/// Exact when-to-use evidence: trigger phrases present verbatim plus bounded
/// token overlap. Phrase count is capped to prevent flooding.
fn wtu_score(s: &SkillInfo, prompt: &str) -> u64 {
    let Some(wtu) = &s.when_to_use else {
        return 0;
    };
    let prompt_lower = prompt.to_lowercase();
    let mut score = 0u64;
    let mut matched = 0u8;
    for phrase in wtu.split([':', ',', ';', '\n']) {
        if matched >= MAX_WTU_PHRASES as u8 {
            break;
        }
        let phrase = phrase.trim();
        if phrase.len() >= 6 && prompt_lower.contains(&phrase.to_lowercase()) {
            score = score.saturating_add(10);
            matched += 1;
        }
    }
    let prompt_words = significant_words(prompt);
    let mut overlap = 0u8;
    for word in significant_words(wtu) {
        if overlap >= 8 {
            break;
        }
        if prompt_words.contains(&word) {
            score = score.saturating_add(1);
            overlap += 1;
        }
    }
    score
}

/// A `paths:` pattern is "specific" if it has a concrete segment (not just
/// `*`/`**`) — bare catch-all skills should not get high prompt-path scores.
fn specific_pattern(p: &str) -> bool {
    p.split('/').any(|seg| seg != "*" && seg != "**")
}

/// Prompt-referenced paths matching a skill's `paths:` globs. High priority but
/// capped and gated against `**` self-promotion.
fn prompt_path_score(s: &SkillInfo, prompt_paths: &[String], workspace_root: &Path) -> u64 {
    let Some(patterns) = &s.paths else {
        return 0;
    };
    if patterns.is_empty() || prompt_paths.is_empty() {
        return 0;
    }
    let patterns: Vec<&String> = patterns.iter().take(MAX_PATH_PATTERNS).collect();
    if !patterns.iter().any(|p| specific_pattern(p)) {
        return 2; // only catch-all patterns — de-emphasize near-self-promotion
    }
    use ignore::gitignore::GitignoreBuilder;
    let mut builder = GitignoreBuilder::new(workspace_root);
    for p in &patterns {
        let _ = builder.add_line(None, p.as_str());
    }
    let Ok(matcher) = builder.build() else {
        return 0;
    };
    if prompt_paths.iter().any(|pat| {
        matcher
            .matched_path_or_any_parents(Path::new(pat), false)
            .is_ignore()
    }) {
        40
    } else {
        0
    }
}

/// Weaker workspace-inventory path evidence using precomputed lowercased rel
/// paths (no per-(skill, entry) allocation). Capped.
fn inventory_score(name_lower: &str, inventory: &WorkspaceInventory) -> i64 {
    let mut score = 0i64;
    for (rel_lower, seg_lower) in &inventory.lowered_rels {
        if seg_lower == name_lower || rel_lower.contains(name_lower) {
            score = score.saturating_add(2);
            if score >= MAX_INVENTORY_SCORE {
                break;
            }
        }
    }
    score
}

/// Deterministically rank `skills` for `prompt` + `inventory`.
///
/// Returns indices in ranked order (best first). Order is fully deterministic:
/// primary score desc, secondary desc, then name/path asc. The explicitly
/// pinned skill (if present) is forced first and never duplicated. Pin matching
/// accepts the bare name or the qualified native name (`scope:name`).
pub fn rank_skills(
    skills: &[SkillInfo],
    prompt: &str,
    inventory: &WorkspaceInventory,
    explicit: Option<&str>,
) -> Vec<usize> {
    let prompt_paths = extract_prompt_paths(prompt);

    // Precompute qualified names once per skill.
    let qualified: Vec<String> = skills.iter().map(format_skill_name).collect();

    let ranked: Vec<Ranked> = skills
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let (local, path) = skill_evidence(s, prompt, &prompt_paths, inventory);
            Ranked {
                idx: i,
                primary: local.saturating_add(path),
                secondary: 0i64,
            }
        })
        .collect();

    let mut order: Vec<usize> = ranked.iter().map(|r| r.idx).collect();
    order.sort_by(|&a, &b| {
        let ra = &ranked[a];
        let rb = &ranked[b];
        rb.primary
            .cmp(&ra.primary)
            .then_with(|| rb.secondary.cmp(&ra.secondary))
            .then_with(|| {
                (skills[a].name.as_str(), skills[a].path.as_str())
                    .cmp(&(skills[b].name.as_str(), skills[b].path.as_str()))
            })
    });

    if let Some(name) = explicit
        && let Some(pos) = order
            .iter()
            .position(|&i| skills[i].name == name || qualified[i] == name)
    {
        let idx = order.remove(pos);
        order.insert(0, idx);
    }
    order
}

/// Local (when-to-use + inventory) and prompt-path evidence for one skill.
fn skill_evidence(
    s: &SkillInfo,
    prompt: &str,
    prompt_paths: &[String],
    inventory: &WorkspaceInventory,
) -> (i64, i64) {
    let name_lower = s.name.to_lowercase();
    let local = wtu_score(s, prompt) as i64 + inventory_score(&name_lower, inventory);
    let path = prompt_path_score(s, prompt_paths, &inventory.root) as i64;
    (local, path)
}

pub(crate) fn evidence_lists(
    skills: &[SkillInfo],
    prompt: &str,
    inventory: &WorkspaceInventory,
) -> (Vec<i64>, Vec<i64>, Vec<usize>, Vec<usize>) {
    let prompt_paths = extract_prompt_paths(prompt);
    let mut local_scores = Vec::with_capacity(skills.len());
    let mut path_scores = Vec::with_capacity(skills.len());
    let mut local_order: Vec<(usize, i64)> = Vec::new();
    let mut path_order: Vec<(usize, i64)> = Vec::new();
    for (i, s) in skills.iter().enumerate() {
        let (local, path) = skill_evidence(s, prompt, &prompt_paths, inventory);
        local_scores.push(local);
        path_scores.push(path);
        if local > 0 {
            local_order.push((i, local));
        }
        if path > 0 {
            path_order.push((i, path));
        }
    }
    local_order.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    path_order.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    (
        local_scores,
        path_scores,
        local_order.into_iter().map(|(i, _)| i).collect(),
        path_order.into_iter().map(|(i, _)| i).collect(),
    )
}

/// Extract path-looking tokens from the prompt for `paths:` glob matching.
fn extract_prompt_paths(prompt: &str) -> Vec<String> {
    let mut out = Vec::new();
    for tok in prompt.split_whitespace() {
        let t = tok.trim_matches(|c: char| {
            c == '"'
                || c == '\''
                || c == ','
                || c == '.'
                || c == '('
                || c == ')'
                || c == ':'
                || c == ';'
        });
        if t.len() < 2 {
            continue;
        }
        let looks_like_path = t.contains('/')
            || t.starts_with("./")
            || t.starts_with("../")
            || t.starts_with('/')
            || (t.contains('.') && t.len() > 4);
        if looks_like_path {
            out.push(t.to_string());
        }
    }
    out
}

/// Metadata-only text for semantic shipping. `description` is included **only**
/// when it came from frontmatter ([`SkillInfo::has_user_specified_description`]);
/// body-derived descriptions are never transmitted.
#[allow(dead_code)]
fn metadata_text(s: &SkillInfo) -> String {
    super::index::skill_rerank_document(s)
}

/// Unique, path-free CandidateRow identifier (`scope|name|#sha256`). The hash is
/// the **full** SHA-256 digest (64 hex chars) of the absolute path, so no
/// absolute/home path ever appears in candidate ids or `CandidateRow::Debug`,
/// and same-name collisions across scopes/paths remain distinct.
#[allow(dead_code)]
fn candidate_id(s: &SkillInfo) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"prime-candidate/v1\0");
    hasher.update(s.path.as_bytes());
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for b in &digest[..] {
        hex.push_str(&format!("{b:02x}"));
    }
    format!("{}|{}|#{hex}", scope_label(s.scope), s.name)
}

/// Authoritative eligibility: delegates to the tools predicate so the two layers
/// can never drift apart.
pub fn is_eligible(s: &SkillInfo) -> bool {
    s.is_native_model_invocable()
}

/// Outcome of the optional semantic refinement stage.
#[derive(Clone)]
pub struct SemanticFillOutcome {
    pub order: Vec<usize>,
    /// Secret-free degradation notices (soft mode).
    pub degradations: Vec<DegradationNotice>,
    /// Cancellation/deadline fired (no content should be produced).
    pub cancelled: bool,
    /// Hard semantic failure (hard mode / `degrade_on_error = false`).
    pub hard_error: Option<OrchestratorError>,
    /// Number of non-pinned candidates actually shipped.
    pub shortlist_size: usize,
}

fn degrade_from_error(e: &OrchestratorError, profile: &str) -> DegradationNotice {
    use crate::retrieval::{DegradationKind, OrchestratorError, RetrievalStage};
    let kind = match e {
        OrchestratorError::ServiceDisabled => DegradationKind::ServiceDisabled,
        OrchestratorError::ProfileMissing { .. } => DegradationKind::ProfileMissing,
        OrchestratorError::DeadlineExceeded { .. }
        | OrchestratorError::AttemptBudgetExceeded { .. }
        | OrchestratorError::InputBudgetExceeded { .. }
        | OrchestratorError::OutputBudgetExceeded { .. } => DegradationKind::BudgetExhausted,
        _ => DegradationKind::SemanticUnavailable,
    };
    DegradationNotice::new(kind, profile, RetrievalStage::Orchestrate, None)
}

/// Soft embed/KNN miss: keep pins first and the exact pre-stage order.
/// Fusion is reserved for a same-space KNN/FTS read.
fn restore_pre_stage_order(
    outcome: &mut SemanticFillOutcome,
    pinned_order: &[usize],
    ranked: &[usize],
    profile_id: &str,
    stage: crate::retrieval::RetrievalStage,
) {
    outcome.degradations.push(DegradationNotice::new(
        DegradationKind::SemanticUnavailable,
        profile_id,
        stage,
        None,
    ));
    outcome.order = prepend_unique(pinned_order, ranked);
    outcome.shortlist_size = 0;
}

/// Optional semantic fill over the **full** indexed inventory.
///
/// Query FTS and sqlite-vec KNN cover every valid indexed skill. Deterministic
/// local/path ranks are fused with BM25 and vector ranks via weighted
/// reciprocal-rank fusion. A reranker, when configured, sees only the fused
/// shortlist (profile `max_candidates`) — never a first-eight inventory cap.
/// Soft failure preserves the exact pre-stage deterministic order; hard
/// failure surfaces before user-turn insertion.
pub async fn semantic_fill(
    service: &RetrievalService,
    profile_id: &str,
    query: &str,
    skills: &[SkillInfo],
    ranked: &[usize],
    pinned: &HashSet<String>,
    cancel: CancellationToken,
    hard: bool,
    grok_home: Option<&Path>,
    workspace_root: &Path,
    inventory: &WorkspaceInventory,
    consumer_min_score: f32,
) -> SemanticFillOutcome {
    let mut outcome = SemanticFillOutcome {
        order: ranked.to_vec(),
        degradations: Vec::new(),
        cancelled: false,
        hard_error: None,
        shortlist_size: 0,
    };

    if query.trim().is_empty() || cancel.is_cancelled() {
        outcome.cancelled = cancel.is_cancelled();
        return outcome;
    }

    let pinned_order: Vec<usize> = ranked
        .iter()
        .copied()
        .filter(|&i| pinned.contains(&skills[i].name))
        .collect();
    let non_pinned: Vec<usize> = ranked
        .iter()
        .copied()
        .filter(|&i| !pinned.contains(&skills[i].name))
        .collect();
    if non_pinned.is_empty() {
        return outcome;
    }

    let Some(home) = grok_home else {
        if hard {
            outcome.hard_error = Some(OrchestratorError::ProfileMissing {
                profile_id: profile_id.to_owned(),
            });
        } else {
            outcome.degradations.push(DegradationNotice::new(
                DegradationKind::SemanticUnavailable,
                profile_id,
                crate::retrieval::RetrievalStage::Orchestrate,
                None,
            ));
        }
        return outcome;
    };

    match fill_from_index(
        service,
        profile_id,
        query,
        skills,
        ranked,
        &pinned_order,
        &non_pinned,
        pinned,
        cancel,
        home,
        workspace_root,
        inventory,
        consumer_min_score,
        hard,
        &mut outcome,
    )
    .await
    {
        Ok(()) => outcome,
        Err(e) if matches!(e, OrchestratorError::Cancelled { .. }) => {
            outcome.cancelled = true;
            outcome.order = ranked.to_vec();
            outcome
        }
        Err(e) if hard => {
            outcome.hard_error = Some(e);
            outcome.order = ranked.to_vec();
            outcome
        }
        Err(e) => {
            outcome
                .degradations
                .push(degrade_from_error(&e, profile_id));
            // Soft KNN/space/embed failure must keep pins first and the exact
            // pre-stage deterministic order. Replacing that list with the
            // automatic-only rest (often empty) would omit every non-pinned
            // skill from prime load.
            outcome.order = prepend_unique(&pinned_order, ranked);
            outcome
        }
    }
}

fn knn_space_error(profile_id: &str) -> OrchestratorError {
    OrchestratorError::AllRoutesFailed {
        profile_id: profile_id.to_owned(),
        stage: crate::retrieval::RetrievalStage::Candidates,
        last_failure: None,
    }
}

async fn fill_from_index(
    service: &RetrievalService,
    profile_id: &str,
    query: &str,
    skills: &[SkillInfo],
    ranked: &[usize],
    pinned_order: &[usize],
    non_pinned: &[usize],
    pinned: &HashSet<String>,
    cancel: CancellationToken,
    home: &Path,
    workspace_root: &Path,
    inventory: &WorkspaceInventory,
    consumer_min_score: f32,
    // Mixed-space always returns Err; the caller maps that onto hard vs soft.
    _hard: bool,
    outcome: &mut SemanticFillOutcome,
) -> Result<(), OrchestratorError> {
    let handle = prime_index_for(home, workspace_root);
    let items = skills_to_index_items(skills);
    let generation = inventory_generation_token(&items);
    handle
        .sync_skills(generation, &items)
        .map_err(|_| OrchestratorError::InvalidRequest("prime index sync failed".into()))?;
    handle.pin_from_service(service, profile_id).map_err(|_| {
        OrchestratorError::ProfileMissing {
            profile_id: profile_id.to_owned(),
        }
    })?;

    let bound_ms = service
        .load_snapshot()
        .profile(profile_id)
        .map(|p| p.config.deadline_ms)
        .filter(|&n| n > 0)
        .unwrap_or(3_000);
    // Backfill is bounded independently of query completion so it can finish
    // after search returns, while in-flight embed HTTP still sees a deadline.
    let backfill_cancel = bounded_cancel(bound_ms);
    let frozen = handle
        .freeze_pin()
        .map_err(|_| knn_space_error(profile_id))?;
    let embedder = Arc::new(PinnedServiceEmbedder::with_frozen_pin(
        handle.clone(),
        service.clone(),
        profile_id.to_owned(),
        frozen.clone(),
        backfill_cancel.clone(),
    ));
    // Missing vectors must not block the query path. Search immediately with
    // current FTS plus any safe live vectors; a not-yet-filled collection
    // degrades rather than installing stale data.
    handle.spawn_backfill(embedder, frozen.clone(), backfill_cancel);

    if cancel.is_cancelled() {
        return Err(OrchestratorError::Cancelled {
            profile_id: profile_id.to_owned(),
            stage: crate::retrieval::RetrievalStage::Orchestrate,
        });
    }

    let snapshot = service.load_snapshot();
    let profile =
        snapshot
            .profile(profile_id)
            .ok_or_else(|| OrchestratorError::ProfileMissing {
                profile_id: profile_id.to_owned(),
            })?;
    let threshold = stricter_threshold(consumer_min_score, profile.config.min_score);
    let knn_k = (profile.budgets.max_candidates as usize)
        .max(items.len())
        .max(1);

    // Capture the exact-space token before the query embed await. Embed
    // strictly with that capture; a live pin swap must not retarget the HTTP
    // route or mix spaces after the await.
    //
    // Query-embed failure and KNN unavailability restore the exact pre-stage
    // order (pins first) and record a secret-free SemanticUnavailable.
    // Fusion is reserved for a same-space KNN/FTS read. Mixed-space remains
    // knn_space_error so hard mode fails before user-turn insertion.
    let query_vec = match handle
        .embed_texts_with_pin(
            &frozen,
            service,
            profile_id,
            vec![query.to_owned()],
            cancel.child_token(),
        )
        .await
    {
        Ok(mut query_vecs) if !query_vecs.is_empty() => Some(query_vecs.remove(0)),
        _ => None,
    };

    let live_after = handle.pinned_space();
    let pin_still_frozen = live_after
        .as_ref()
        .is_some_and(|live| frozen.matches_live(live));
    let space_after = handle.knn_space_for_pin(&frozen);

    let Some(query_vec) = query_vec else {
        restore_pre_stage_order(
            outcome,
            pinned_order,
            ranked,
            profile_id,
            crate::retrieval::RetrievalStage::Embed,
        );
        return Ok(());
    };

    let knn = match (pin_still_frozen, space_after) {
        (true, Ok(())) => match handle.search_knn_with_pin(&frozen, &query_vec, knn_k) {
            Ok(hits) => hits,
            Err(super::index::PrimeIndexError::SpaceMismatch) => {
                return Err(knn_space_error(profile_id));
            }
            Err(_) => {
                restore_pre_stage_order(
                    outcome,
                    pinned_order,
                    ranked,
                    profile_id,
                    crate::retrieval::RetrievalStage::Candidates,
                );
                return Ok(());
            }
        },
        (false, _) | (_, Err(super::index::PrimeIndexError::SpaceMismatch)) => {
            return Err(knn_space_error(profile_id));
        }
        _ => {
            restore_pre_stage_order(
                outcome,
                pinned_order,
                ranked,
                profile_id,
                crate::retrieval::RetrievalStage::Candidates,
            );
            return Ok(());
        }
    };
    let fts = handle.search_fts(query, knn_k).unwrap_or_default();

    let mut id_to_idx = std::collections::HashMap::new();
    let mut idx_to_id = vec![String::new(); skills.len()];
    for (i, s) in skills.iter().enumerate() {
        let id = opaque_skill_id(s);
        id_to_idx.insert(id.clone(), i);
        idx_to_id[i] = id;
    }

    let bm25_order: Vec<String> = fts.into_iter().map(|h| h.item_id).collect();
    let mut vector_sim = std::collections::HashMap::new();
    let vector_order: Vec<String> = knn
        .into_iter()
        .map(|h| {
            vector_sim.insert(h.item_id.clone(), l2_similarity(h.distance));
            h.item_id
        })
        .collect();

    let (local_scores, path_scores, local_order, path_order) =
        evidence_lists(skills, query, inventory);
    let fused = fuse_ranks(
        skills.len(),
        &local_order,
        &path_order,
        &local_scores,
        &path_scores,
        &bm25_order,
        &vector_order,
        &vector_sim,
        &id_to_idx,
    );

    let mut automatic = Vec::new();
    for row in &fused {
        if pinned.contains(&skills[row.idx].name) {
            continue;
        }
        if automatic_candidate_allowed(
            row.local_score,
            row.path_score,
            row.vector_similarity,
            threshold,
        ) {
            automatic.push(row.idx);
        }
    }

    let rerank_cap = profile.budgets.max_rerank_shortlist.max(1) as usize;
    let shortlist: Vec<usize> = automatic.iter().copied().take(rerank_cap).collect();
    outcome.shortlist_size = automatic.len();

    let mut rest = shortlist.clone();
    if !shortlist.is_empty() && !profile.reranker_route_ids.is_empty() {
        let docs: Vec<String> = shortlist
            .iter()
            .map(|&i| skill_rerank_document(&skills[i]))
            .collect();
        let opts = PipelineOptions {
            bypass_semantic: false,
            hard_error_on_semantic_failure: false,
            pin_snapshot_generation: Some(frozen.space().snapshot_generation),
            embed_route_pin: None,
            hard_error_on_limit_exceeded: false,
        };
        match service
            .rerank(
                profile_id,
                query.to_owned(),
                docs,
                opts,
                cancel.child_token(),
            )
            .await
        {
            Ok(stage) => {
                if let Some(d) = stage.degradation {
                    outcome.degradations.push(d);
                }
                if let Some(result) = stage.result {
                    let mut reordered = Vec::new();
                    let mut used = HashSet::new();
                    for hit in &result.hits {
                        if hit.index < shortlist.len() && used.insert(shortlist[hit.index]) {
                            reordered.push(shortlist[hit.index]);
                        }
                    }
                    for &i in &shortlist {
                        if used.insert(i) {
                            reordered.push(i);
                        }
                    }
                    rest = reordered;
                }
            }
            Err(OrchestratorError::Cancelled { .. }) => {
                return Err(OrchestratorError::Cancelled {
                    profile_id: profile_id.to_owned(),
                    stage: crate::retrieval::RetrievalStage::Orchestrate,
                });
            }
            Err(_) => {
                outcome.degradations.push(DegradationNotice::new(
                    DegradationKind::RerankUnavailable,
                    profile_id,
                    crate::retrieval::RetrievalStage::Rerank,
                    None,
                ));
            }
        }
    }

    for &i in automatic.iter() {
        if !rest.contains(&i) {
            rest.push(i);
        }
    }
    for &i in non_pinned {
        if !rest.contains(&i)
            && automatic_candidate_allowed(
                local_scores.get(i).copied().unwrap_or(0),
                path_scores.get(i).copied().unwrap_or(0),
                vector_sim.get(&idx_to_id[i]).copied(),
                threshold,
            )
        {
            rest.push(i);
        }
    }

    outcome.order = prepend_unique(pinned_order, &rest);
    let _ = ranked;
    Ok(())
}

/// Smart search over the same index and fusion. Exact name matches are
/// first. Any index/embed failure returns `None` so the caller falls back
/// to local-only ordering immediately. A `SemanticUnavailable` degradation
/// with an empty automatic list is `None`, never `Some([])`.
pub async fn smart_search_names(
    service: Option<&RetrievalService>,
    profile_id: Option<&str>,
    query: &str,
    skills: &[SkillInfo],
    grok_home: Option<&Path>,
    workspace_root: &Path,
    inventory: &WorkspaceInventory,
    consumer_min_score: f32,
    cancel: CancellationToken,
) -> Option<Vec<String>> {
    let q = query.trim();
    if q.is_empty() {
        return None;
    }
    let mut exact: Vec<String> = Vec::new();
    let q_lower = q.to_ascii_lowercase();
    for s in skills {
        if s.name.eq_ignore_ascii_case(q) {
            exact.push(s.name.clone());
        }
    }
    let (service, profile_id, home) = (service?, profile_id?, grok_home?);
    // Same pre-stage ranking prime uses, so a soft KNN miss restores
    // inventory/path-aware local order rather than listing identity.
    let ranked = rank_skills(skills, q, inventory, None);
    let pinned = HashSet::new();
    let out = semantic_fill(
        service,
        profile_id,
        q,
        skills,
        &ranked,
        &pinned,
        cancel,
        false,
        Some(home),
        workspace_root,
        inventory,
        consumer_min_score,
    )
    .await;
    if out.cancelled || out.hard_error.is_some() {
        return None;
    }
    // Soft fill maps embed/pin/sync/KNN unavailability onto pre-stage order
    // plus a degradation. That is not a real index/fusion result: callers
    // must fall back to local-only ranking. RerankUnavailable still keeps
    // the fused automatic list. Never return Some([]) after an embed miss.
    if out.shortlist_size == 0
        && out.degradations.iter().any(|d| {
            matches!(
                d.kind,
                DegradationKind::SemanticUnavailable
                    | DegradationKind::ServiceDisabled
                    | DegradationKind::ProfileMissing
                    | DegradationKind::BudgetExhausted
            )
        })
    {
        return None;
    }
    let mut names: Vec<String> = exact;
    let mut seen: HashSet<String> = names.iter().cloned().collect();
    for &i in &out.order {
        if skills[i].name.to_ascii_lowercase() == q_lower {
            continue;
        }
        if seen.insert(skills[i].name.clone()) {
            names.push(skills[i].name.clone());
        }
    }
    Some(names)
}

/// Trim `buf` to at most `max` bytes while preserving UTF-8 validity: backs up
/// over continuation bytes (10xxxxxx) so a multibyte code point is never split.
///
/// After the back-up `end` is always a valid character boundary (either the
/// start of the next code point, or `max` when `max` already lands on a
/// boundary), so a complete preceding multibyte character is never truncated.
#[cfg(test)]
fn utf8_prefix_trim(buf: &mut Vec<u8>, max: usize) {
    if buf.len() <= max {
        return;
    }
    let mut end = max;
    while end > 0 && (buf[end] & 0xC0) == 0x80 {
        end -= 1;
    }
    buf.truncate(end);
}

/// Bounded, TOCTOU-hardened body reader (Unix).
///
/// Opens the canonical trusted root directory fd and walks every relative
/// component with `openat` + `O_NOFOLLOW|O_DIRECTORY` (final component
/// `O_NOFOLLOW|O_NONBLOCK`), so an intermediate-directory symlink swap cannot
/// redirect the read outside the root: each component is resolved beneath the
/// previously opened directory handle, never through a path string. The final
/// handle must be a regular file; content is read bounded and trimmed to a
/// UTF-8 boundary.
///
/// Prime no longer uses this reader for primed payloads (it consumes the
/// no-follow revalidation bytes). The walk remains as a regression test of
/// the openat component policy.
#[cfg(all(test, unix))]
fn read_body_bounded(root: &Path, rel: &Path, _fallback_path: &Path) -> std::io::Result<String> {
    use std::io::Read;
    use std::os::fd::OwnedFd;
    use std::os::unix::ffi::OsStrExt;

    use nix::fcntl::{OFlag, open, openat};
    use nix::sys::stat::Mode;

    let err_from = |e: nix::errno::Errno| std::io::Error::from_raw_os_error(e as i32);

    let root_fd = open(
        root,
        OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::empty(),
    )
    .map_err(err_from)?;

    let comps: Vec<&[u8]> = rel.components().map(|c| c.as_os_str().as_bytes()).collect();
    if comps.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "skill path has no relative components",
        ));
    }

    let mut dir: OwnedFd = root_fd;
    let mut file_fd: Option<OwnedFd> = None;
    let last = comps.len() - 1;
    for (i, comp) in comps.iter().enumerate() {
        if i == last {
            file_fd = Some(
                openat(
                    &dir,
                    *comp,
                    OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC,
                    Mode::empty(),
                )
                .map_err(err_from)?,
            );
            break;
        }
        let fd = openat(
            &dir,
            *comp,
            OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(err_from)?;
        dir = fd;
    }
    let Some(file_fd) = file_fd else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "skill path has no final component",
        ));
    };

    let mut file = std::fs::File::from(file_fd);
    let meta = file.metadata()?;
    if !meta.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "skill file is not a regular file",
        ));
    }
    let mut buf = Vec::new();
    let mut take = file.by_ref().take(MAX_LOADED_BODY_BYTES.saturating_add(1));
    let _ = take.read_to_end(&mut buf)?;
    utf8_prefix_trim(&mut buf, MAX_LOADED_BODY_BYTES as usize);
    String::from_utf8(buf).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "skill body is not valid UTF-8",
        )
    })
}

/// Portable bounded reader fallback (non-Unix).
///
/// Fails closed on non-regular files and reads bounded. Does not offer the
/// `openat` intermediate-directory guarantee on these platforms (documented);
/// content is still capped and UTF-8-trimmed at the boundary.
#[cfg(all(test, not(unix)))]
fn read_body_bounded(_root: &Path, _rel: &Path, fallback_path: &Path) -> std::io::Result<String> {
    use std::io::Read;
    let meta = std::fs::metadata(fallback_path)?;
    if !meta.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "skill file is not a regular file",
        ));
    }
    let mut f = std::fs::File::open(fallback_path)?;
    let mut buf = Vec::new();
    let mut take = f.by_ref().take(MAX_LOADED_BODY_BYTES.saturating_add(1));
    let _ = take.read_to_end(&mut buf)?;
    utf8_prefix_trim(&mut buf, MAX_LOADED_BODY_BYTES as usize);
    String::from_utf8(buf).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "skill body is not valid UTF-8",
        )
    })
}

/// Revalidated, natively-loaded selected skills (never trusting snapshot body).
pub struct LoadedBatch {
    pub loaded: Vec<LoadedSkill>,
    pub drop_reasons: Vec<PrimeDropReason>,
}

/// Containment policy on the original (non-followed) SKILL.md path. Walks
/// ancestors with `lstat` and matches a trusted-root directory by inode so
/// OS-level intermediate links such as `/var` → `/private/var` still count
/// as contained, while a swapped `.grok` / `skills/` / leaf symlink does not.
fn original_path_contained(path: &Path, trusted_roots: &[PathBuf]) -> Result<(), PrimeDropReason> {
    if !path.file_name().is_some_and(|name| name == "SKILL.md") {
        return Err(PrimeDropReason::NotContained);
    }
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        match std::env::current_dir() {
            Ok(cwd) => cwd.join(path),
            Err(_) => return Err(PrimeDropReason::Unreadable),
        }
    };
    if abs
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(PrimeDropReason::NotContained);
    }

    let file_meta = match std::fs::symlink_metadata(&abs) {
        Ok(meta) => meta,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(PrimeDropReason::ChangedOrGone);
        }
        Err(_) => return Err(PrimeDropReason::Unreadable),
    };
    if file_meta.file_type().is_symlink() {
        return Err(PrimeDropReason::NotContained);
    }
    if !file_meta.file_type().is_file() {
        return Err(PrimeDropReason::Unreadable);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let root_ids: Vec<(u64, u64)> = trusted_roots
            .iter()
            .filter_map(|root| {
                let meta = std::fs::symlink_metadata(root).ok()?;
                if meta.file_type().is_symlink() || !meta.is_dir() {
                    return None;
                }
                Some((meta.dev(), meta.ino()))
            })
            .collect();
        if root_ids.is_empty() {
            return Err(PrimeDropReason::NotContained);
        }
        let mut cur = abs
            .parent()
            .ok_or(PrimeDropReason::NotContained)?
            .to_path_buf();
        loop {
            let meta = match std::fs::symlink_metadata(&cur) {
                Ok(meta) => meta,
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                    return Err(PrimeDropReason::ChangedOrGone);
                }
                Err(_) => return Err(PrimeDropReason::Unreadable),
            };
            if meta.file_type().is_symlink() {
                return Err(PrimeDropReason::NotContained);
            }
            if !meta.is_dir() {
                return Err(PrimeDropReason::NotContained);
            }
            if root_ids
                .iter()
                .any(|&(dev, ino)| dev == meta.dev() && ino == meta.ino())
            {
                return Ok(());
            }
            match cur.parent() {
                Some(parent) if parent != cur => cur = parent.to_path_buf(),
                _ => return Err(PrimeDropReason::NotContained),
            }
        }
    }
    #[cfg(not(unix))]
    {
        if trusted_roots.iter().any(|root| abs.starts_with(root)) {
            Ok(())
        } else {
            Err(PrimeDropReason::NotContained)
        }
    }
}

/// Revalidate against a fresh authoritative snapshot and load up to `target`
/// bodies, backfilling the next-ranked candidate whenever one is dropped.
///
/// `order` is the full ranked order (post-semantic). For each candidate: require
/// it is still present + eligible with the same identity, then take the SKILL.md
/// bytes already read from the no-follow revalidation fd. Containment is a
/// policy check on the original path components; the path is never re-opened.
pub async fn load_and_revalidate(
    skills: &[SkillInfo],
    order: &[usize],
    target: usize,
    refresh: &dyn SkillRefresh,
    trusted_roots: &[PathBuf],
    cancel: &CancellationToken,
) -> LoadedBatch {
    let target = target.max(1);
    // Cancellation-race the async fresh-snapshot refresh against the gate.
    let fresh = tokio::select! {
        biased;
        _ = cancel.cancelled() => return LoadedBatch { loaded: Vec::new(), drop_reasons: Vec::new() },
        fresh = refresh.refresh() => fresh,
    };
    let fresh_eligible: Vec<&SkillInfo> = fresh.iter().filter(|s| is_eligible(s)).collect();

    let mut loaded = Vec::new();
    let mut drop_reasons = Vec::new();

    for &idx in order {
        if cancel.is_cancelled() {
            return LoadedBatch {
                loaded,
                drop_reasons,
            };
        }
        let cand = &skills[idx];

        // Revalidation: identity + eligibility against the fresh snapshot.
        let cur = fresh_eligible
            .iter()
            .find(|s| s.name == cand.name && s.path == cand.path && s.scope == cand.scope);
        let Some(cur) = cur else {
            drop_reasons.push(PrimeDropReason::ChangedOrGone);
            continue;
        };

        let revalidated = match xai_grok_tools::implementations::skills::strict::revalidate_skill_file_at_load(cur)
        {
            Ok(loaded_file) => loaded_file,
            Err(xai_grok_tools::implementations::skills::strict::SkillLoadError::Symlink)
            | Err(xai_grok_tools::implementations::skills::strict::SkillLoadError::NotASkill) => {
                drop_reasons.push(PrimeDropReason::NotContained);
                continue;
            }
            Err(xai_grok_tools::implementations::skills::strict::SkillLoadError::NotFound) => {
                drop_reasons.push(PrimeDropReason::ChangedOrGone);
                continue;
            }
            Err(xai_grok_tools::implementations::skills::strict::SkillLoadError::Quarantined)
            | Err(
                xai_grok_tools::implementations::skills::strict::SkillLoadError::IdentityChanged,
            ) => {
                drop_reasons.push(PrimeDropReason::Quarantined);
                continue;
            }
            Err(_) => {
                drop_reasons.push(PrimeDropReason::Unreadable);
                continue;
            }
        };

        if let Err(reason) = original_path_contained(Path::new(&cur.path), trusted_roots) {
            drop_reasons.push(reason);
            continue;
        }

        let body = extract_skill_body(&revalidated.content);
        let dir = Path::new(&cur.path).parent().map(Path::to_path_buf);
        let body = match dir {
            Some(d) => resolve_skill_internal_links(&body, &d),
            None => body,
        };
        loaded.push(LoadedSkill {
            name: cur.name.clone(),
            scope: cur.scope,
            source_path: cur.path.clone(),
            body,
        });
        if loaded.len() >= target {
            break;
        }
    }

    LoadedBatch {
        loaded,
        drop_reasons,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::prime::inventory::InventoryEntry;
    use crate::session::prime::{PrimeError, PrimeInput, run_prime_selection};
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    use xai_grok_config_types::{
        EmbeddingEncoding, EmbeddingModelConfig, EmbeddingProtocol, PrimeConfig,
        RerankerModelConfig, RetrievalFallbackStrategy, RetrievalGraphConfig,
        RetrievalProfileConfig,
    };
    use xai_grok_inference::{
        EmbeddingResult, EmbeddingVector, RerankHit, RerankResult, RetrievalError, RetrievalResult,
    };
    use xai_grok_tools::implementations::skills::types::SkillScope;

    use crate::retrieval::bounds::ProfileBudgetLimits;
    use crate::retrieval::graph::{
        EmbeddingRouteDescriptor, EmbeddingSpaceId, RerankerRouteDescriptor, RetrievalSnapshot,
        SnapshotProfile,
    };
    use crate::retrieval::{RetrievalRegistry, RetrievalService};
    use crate::session::prime::index::PinnedEmbeddingSpace;
    use xai_grok_memory::embedding::{EmbeddingProvider, MockEmbeddingProvider};
    use xai_grok_memory::{EmbeddingSourceSpec, NORMALIZATION_L2_V1};

    fn skill(name: &str, path: &str) -> SkillInfo {
        SkillInfo {
            name: name.into(),
            path: path.into(),
            ..SkillInfo::default()
        }
    }

    fn skill_wtu(name: &str, path: &str, wtu: &str) -> SkillInfo {
        SkillInfo {
            when_to_use: Some(wtu.into()),
            ..skill(name, path)
        }
    }

    fn skill_paths(name: &str, path: &str, paths: &[&str]) -> SkillInfo {
        SkillInfo {
            paths: Some(paths.iter().map(|s| s.to_string()).collect()),
            ..skill(name, path)
        }
    }

    fn skill_desc(name: &str, path: &str, desc: &str) -> SkillInfo {
        SkillInfo {
            has_user_specified_description: true,
            description: desc.into(),
            ..skill(name, path)
        }
    }

    fn spec_b_same_dim() -> EmbeddingSourceSpec {
        EmbeddingSourceSpec {
            provider_instance_id: "p-b".into(),
            incarnation: Some("ci-b".into()),
            origin_host: "embedding-b.example.com".into(),
            embedding_path: "/v1/embeddings".into(),
            protocol: "openai_compatible".into(),
            model: "emb-model-b".into(),
            dimensions: 4,
            encoding: "float".into(),
            normalization: NORMALIZATION_L2_V1.to_owned(),
        }
    }

    /// Async refresh adapter over a fixed snapshot.
    fn refresher(
        snapshot: Vec<SkillInfo>,
    ) -> impl Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<SkillInfo>> + Send>>
    {
        move || {
            let s = snapshot.clone();
            Box::pin(async move { s })
        }
    }

    // ── Ranking: pinned / scoring / deterministic ties ────────────────

    #[test]
    fn routing_quality_when_to_use_outranks_unrelated() {
        let inv = WorkspaceInventory::default();
        let deploy = skill_wtu("deploy", "skills/deploy/SKILL.md", "deploy the release");
        let noise = skill("zzzz", "skills/zzzz/SKILL.md");
        let skills = vec![noise, deploy];
        let rank = rank_skills(&skills, "please deploy the release", &inv, None);
        assert_eq!(skills[rank[0]].name, "deploy");
        assert!(
            !crate::session::prime::skill_index_text(&skills[rank[0]]).contains("BODY"),
            "routing quality fixtures must stay metadata-only"
        );
    }

    #[test]
    fn hidden_reminder_includes_only_selected_bodies() {
        use crate::session::prime::render::{LoadedSkill, RenderBudgets, render_skills};
        let selected = LoadedSkill {
            name: "deploy".into(),
            scope: SkillScope::Local,
            source_path: "skills/deploy/SKILL.md".into(),
            body: "PRIME-SKILL-BODY deploy steps".into(),
        };
        let rendered = render_skills(
            &[selected],
            &RenderBudgets {
                per_body_chars: 2000,
                max_total_chars: 6000,
                max_tokens: Some(1500),
            },
        );
        assert!(rendered.text.contains("PRIME-SKILL-BODY"));
        assert!(!rendered.text.contains("UNSELECTED-BODY"));
    }

    #[test]
    fn pinned_skill_first_and_not_duplicated() {
        let skills = vec![
            skill_wtu("deploy", "/s/deploy/SKILL.md", "deploy the release"),
            skill_wtu("review", "/s/review/SKILL.md", "review a pull request"),
        ];
        let inv = WorkspaceInventory::default();
        let rank = rank_skills(&skills, "please deploy", &inv, Some("review"));
        assert_eq!(rank.len(), 2, "no duplicates");
        assert_eq!(skills[rank[0]].name, "review", "pinned must be first");
        assert_eq!(skills[rank[1]].name, "deploy");
    }

    #[test]
    fn pin_matches_qualified_native_name() {
        let skills = vec![
            skill_wtu("deploy", "/s/deploy/SKILL.md", "deploy the release"),
            skill_wtu("review", "/s/review/SKILL.md", "review a pull request"),
        ];
        let inv = WorkspaceInventory::default();
        // "local:deploy" (qualified native name) pins the deploy skill.
        let rank = rank_skills(&skills, "unrelated", &inv, Some("local:deploy"));
        assert_eq!(skills[rank[0]].name, "deploy");
        assert_eq!(skills[rank[0]].scope, SkillScope::Local);
    }

    #[test]
    fn exact_when_to_use_outranks_weak_and_tie_is_deterministic() {
        let inv = WorkspaceInventory::default();
        let deploy = skill_wtu("deploy", "/s/deploy/SKILL.md", "deploy the release");
        let fuzzy = skill_wtu("zz", "/s/zz/SKILL.md", "zebra crossings");
        let none = skill_wtu("aaa", "/s/aaa/SKILL.md", "unrelated");
        let skills = vec![deploy, fuzzy, none];
        let rank = rank_skills(&skills, "please deploy the release now", &inv, None);
        assert_eq!(skills[rank[0]].name, "deploy", "exact when-to-use wins");
        // Deterministic tie: zero evidence → name asc.
        let rank2 = rank_skills(&skills, "nothing relevant", &inv, None);
        let names: Vec<&str> = rank2.iter().map(|&i| skills[i].name.as_str()).collect();
        assert_eq!(names, vec!["aaa", "deploy", "zz"]);
    }

    #[test]
    fn prompt_path_and_inventory_add_scores() {
        let inv = WorkspaceInventory {
            root: PathBuf::from("/ws"),
            entries: vec![InventoryEntry {
                rel: "src/deploy.rs".into(),
                size_bytes: 10,
                is_dir: false,
                is_symlink: false,
            }],
            truncated: false,
            cross_device: false,
            lowered_rels: vec![("src/deploy.rs".to_string(), "deploy.rs".to_string())],
            epoch: 0,
        };
        let deploy = skill_paths("deploy", "/s/deploy/SKILL.md", &["src/*.rs"]);
        let other = skill("other", "/s/other/SKILL.md");
        let skills = vec![other, deploy];
        let rank = rank_skills(&skills, "please change src/deploy.rs", &inv, None);
        assert_eq!(skills[rank[0]].name, "deploy", "prompt-path evidence wins");
    }

    #[test]
    fn user_invocable_false_remains_eligible_cross_layer() {
        let mut s = skill("auto", "/s/auto/SKILL.md");
        s.user_invocable = false;
        assert!(is_eligible(&s));
        // The shell predicate exactly equals the tools predicate.
        assert_eq!(is_eligible(&s), s.is_native_model_invocable());
        let mut locked = s.clone();
        locked.disable_model_invocation = true;
        assert!(!is_eligible(&locked));
        locked.disable_model_invocation = false;
        locked.enabled = false;
        assert!(!is_eligible(&locked));
    }

    #[test]
    fn catch_all_paths_pattern_does_not_self_promote() {
        let inv = WorkspaceInventory::default();
        // Bare `**` should not dominate scoring.
        let open = skill_paths("open", "/s/open/SKILL.md", &["**"]);
        let real = skill_paths("real", "/s/real/SKILL.md", &["src/**"]);
        let skills = vec![real.clone(), open];
        let rank = rank_skills(&skills, "touch src/main.rs", &inv, None);
        assert_eq!(skills[rank[0]].name, "real", "specific paths outrank **");
    }

    // ── Metadata-only boundary ────────────────────────────────────────

    #[test]
    fn metadata_excludes_body_derived_description() {
        // Derived description (has_user_specified_description = false) is body
        // content and must never ship to the provider.
        let mut s = skill("alpha", "/s/alpha/SKILL.md");
        s.body = Some("BODY-SECRET-DESC".into());
        s.description = "BODY-SECRET-DESC".into();
        s.has_user_specified_description = false;
        assert!(!metadata_text(&s).contains("BODY-SECRET-DESC"));

        // Frontmatter-authorized descriptions are shipped (that is permitted).
        s.has_user_specified_description = true;
        assert!(metadata_text(&s).contains("BODY-SECRET-DESC"));
    }

    #[test]
    fn candidate_ids_unique_path_free_for_same_named_skills() {
        let a = skill("commit", "/local/commit/SKILL.md");
        let b = skill("commit", "/user/commit/SKILL.md");
        assert_ne!(candidate_id(&a), candidate_id(&b));
        // Path-free: no absolute path (or home path) ever appears in the id.
        let id = candidate_id(&a);
        assert!(!id.contains("local/commit"), "path leaked into id: {id}");
        assert!(!id.contains("SKILL.md"), "path leaked into id: {id}");
        assert!(!id.starts_with('/'), "path leaked into id: {id}");
        // Full SHA-256 digest (64 hex chars) for collision resistance.
        assert!(id.contains('#'), "digest marker missing: {id}");
        let hex_len = id.rsplit_once('#').map(|(_, h)| h.len()).unwrap_or(0);
        assert_eq!(hex_len, 64, "full SHA-256 digest expected, got {id}");
    }

    // ── Semantic fill ─────────────────────────────────────────────────

    struct RecordingExecutor {
        rerank_docs: Mutex<Vec<Vec<String>>>,
        embed_inputs: Mutex<Vec<Vec<String>>>,
        embed_models: Mutex<Vec<String>>,
        reverse: Mutex<bool>,
        fail_rerank: Mutex<bool>,
        fail_embed: Mutex<bool>,
        slow_ms: Mutex<u64>,
        slow_batch_ms: Mutex<u64>,
        space_vectors: Mutex<std::collections::HashMap<String, Vec<f32>>>,
    }

    impl RecordingExecutor {
        fn new() -> Self {
            Self {
                rerank_docs: Mutex::new(Vec::new()),
                embed_inputs: Mutex::new(Vec::new()),
                embed_models: Mutex::new(Vec::new()),
                reverse: Mutex::new(false),
                fail_rerank: Mutex::new(false),
                fail_embed: Mutex::new(false),
                slow_ms: Mutex::new(0),
                slow_batch_ms: Mutex::new(0),
                space_vectors: Mutex::new(std::collections::HashMap::new()),
            }
        }
        fn set_reverse(&self, v: bool) {
            *self.reverse.lock().unwrap() = v;
        }
        fn set_fail_rerank(&self, v: bool) {
            *self.fail_rerank.lock().unwrap() = v;
        }
        fn set_fail_embed(&self, v: bool) {
            *self.fail_embed.lock().unwrap() = v;
        }
        fn set_slow_ms(&self, v: u64) {
            *self.slow_ms.lock().unwrap() = v;
        }
        fn set_slow_batch_ms(&self, v: u64) {
            *self.slow_batch_ms.lock().unwrap() = v;
        }
        fn rerank_docs(&self) -> Vec<Vec<String>> {
            self.rerank_docs.lock().unwrap().clone()
        }
        fn embed_inputs(&self) -> Vec<Vec<String>> {
            self.embed_inputs.lock().unwrap().clone()
        }
        fn embed_models(&self) -> Vec<String> {
            self.embed_models.lock().unwrap().clone()
        }
        fn set_space_vector(&self, model: &str, values: Vec<f32>) {
            self.space_vectors
                .lock()
                .unwrap()
                .insert(model.to_owned(), values);
        }
    }

    #[async_trait::async_trait]
    impl crate::retrieval::RetrievalExecutor for RecordingExecutor {
        async fn embed(
            &self,
            _home: &std::path::Path,
            _model_id: &str,
            config: &EmbeddingModelConfig,
            pins: &crate::retrieval::RouteCallPins,
            inputs: Vec<String>,
            cancel: CancellationToken,
        ) -> RetrievalResult<EmbeddingResult> {
            self.embed_inputs.lock().unwrap().push(inputs.clone());
            self.embed_models.lock().unwrap().push(config.model.clone());
            if cancel.is_cancelled() {
                return Err(RetrievalError::Cancelled);
            }
            if *self.fail_embed.lock().unwrap() {
                return Err(RetrievalError::Timeout);
            }
            let slow = if inputs.len() > 1 {
                *self.slow_batch_ms.lock().unwrap()
            } else {
                *self.slow_ms.lock().unwrap()
            };
            if slow > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(slow)).await;
                if cancel.is_cancelled() {
                    return Err(RetrievalError::Cancelled);
                }
            }
            if let Some(d) = pins.total_deadline {
                if d.is_zero() {
                    return Err(RetrievalError::DeadlineExceeded);
                }
            }
            let dims = config.dimensions.unwrap_or(4) as usize;
            let values = self
                .space_vectors
                .lock()
                .unwrap()
                .get(&config.model)
                .cloned()
                .unwrap_or_else(|| vec![0.1; dims]);
            Ok(EmbeddingResult {
                model: config.model.clone(),
                vectors: inputs
                    .iter()
                    .enumerate()
                    .map(|(i, _)| EmbeddingVector {
                        index: i,
                        values: values.clone(),
                    })
                    .collect(),
            })
        }

        async fn rerank(
            &self,
            _home: &std::path::Path,
            _model_id: &str,
            config: &RerankerModelConfig,
            pins: &crate::retrieval::RouteCallPins,
            _query: String,
            documents: Vec<String>,
            top_n: Option<u32>,
            cancel: CancellationToken,
        ) -> RetrievalResult<RerankResult> {
            self.rerank_docs.lock().unwrap().push(documents.clone());
            if cancel.is_cancelled() {
                return Err(RetrievalError::Cancelled);
            }
            if let Some(d) = pins.total_deadline {
                if d.is_zero() {
                    return Err(RetrievalError::DeadlineExceeded);
                }
            }
            if *self.fail_rerank.lock().unwrap() {
                return Err(RetrievalError::Timeout);
            }
            let reverse = *self.reverse.lock().unwrap();
            let mut hits: Vec<RerankHit> = documents
                .iter()
                .enumerate()
                .map(|(i, _)| RerankHit {
                    index: i,
                    score: 1.0 - (i as f32) * 0.01,
                    document: None,
                })
                .collect();
            if reverse {
                hits.reverse();
            }
            if let Some(n) = top_n {
                hits.truncate(n as usize);
            }
            Ok(RerankResult {
                model: config.model.clone(),
                hits,
            })
        }
    }

    fn test_snapshot() -> RetrievalSnapshot {
        let mut cfg = RetrievalProfileConfig::default();
        cfg.embedding_models = vec!["emb-1".into()];
        cfg.reranker_models = vec!["rr-1".into()];
        cfg.max_attempts = 4;
        cfg.deadline_ms = 2_000;
        let budgets = ProfileBudgetLimits::from_profile(&cfg, 8);

        let mut emb = indexmap::IndexMap::new();
        emb.insert(
            "emb-1".to_string(),
            EmbeddingRouteDescriptor {
                model_id: "emb-1".into(),
                config: EmbeddingModelConfig {
                    provider: "p".into(),
                    model: "emb-model".into(),
                    dimensions: Some(4),
                    ..Default::default()
                },
                provider_instance_id: "p".into(),
                incarnation: Some("ci".into()),
                origin_host: "embedding.example.com".into(),
                embedding_space: EmbeddingSpaceId::from_parts(
                    "p",
                    Some("ci"),
                    "embedding.example.com",
                    "/embeddings",
                    EmbeddingProtocol::OpenaiCompatible,
                    "emb-model",
                    Some(4),
                    EmbeddingEncoding::Float,
                    "none",
                    "v0",
                ),
                request_timeout_ms: 5_000,
            },
        );
        let mut rr = indexmap::IndexMap::new();
        rr.insert(
            "rr-1".to_string(),
            RerankerRouteDescriptor {
                model_id: "rr-1".into(),
                config: RerankerModelConfig {
                    provider: "p".into(),
                    model: "rr-model".into(),
                    ..Default::default()
                },
                provider_instance_id: "p".into(),
                incarnation: Some("ci".into()),
                origin_host: "rr.example.com".into(),
                request_timeout_ms: 5_000,
            },
        );
        let mut profiles = indexmap::IndexMap::new();
        profiles.insert(
            "p1".to_string(),
            SnapshotProfile {
                id: "p1".into(),
                config: cfg.clone(),
                embedding_route_ids: vec!["emb-1".into()],
                reranker_route_ids: vec!["rr-1".into()],
                budgets,
                fallback_strategy: RetrievalFallbackStrategy::Deterministic,
            },
        );
        let mut prime = PrimeConfig::default();
        prime.skills.enabled = true;
        prime.skills.retrieval_profile = Some("p1".into());

        RetrievalSnapshot {
            generation: 0,
            graph_generation: 0,
            provider_generation: 1,
            fingerprint: "fp".into(),
            enabled: true,
            embedding_models: emb,
            reranker_models: rr,
            profiles,
            prime,
            memory_retrieval_profile: None,
            warnings: Vec::new(),
            source_graph: RetrievalGraphConfig::default(),
        }
    }

    fn service_with(ex: Arc<RecordingExecutor>) -> RetrievalService {
        let reg = RetrievalRegistry::disabled("/tmp/pr18-sem-service");
        reg.force_publish(Arc::new(test_snapshot()));
        RetrievalService::new(reg).with_executor(ex)
    }

    async fn prime_index_with_vectors(
        home: &Path,
        cwd: &Path,
        skills: &[SkillInfo],
        service: &RetrievalService,
    ) {
        let handle = super::super::index::prime_index_for(home, cwd);
        let items = super::super::index::skills_to_index_items(skills);
        handle.sync_skills(1, &items).unwrap();
        handle.pin_from_service(service, "p1").unwrap();
        let embedder = Arc::new(
            PinnedServiceEmbedder::new(
                handle.clone(),
                service.clone(),
                "p1".to_owned(),
                CancellationToken::new(),
            )
            .expect("pinned embedder"),
        );
        handle
            .backfill(
                embedder,
                handle.freeze_pin().expect("frozen pin"),
                CancellationToken::new(),
            )
            .await
            .expect("test backfill");
    }

    #[tokio::test]
    async fn semantic_fill_shares_only_frontmatter_metadata_and_orders() {
        let ex = Arc::new(RecordingExecutor::new());
        ex.set_reverse(true);
        let service = service_with(ex.clone());
        let mut mk = |n: &str, body: &str| SkillInfo {
            body: Some(body.into()),
            description: format!("BODY-DERIVED-{n}"),
            has_user_specified_description: false,
            ..skill(n, &format!("/s/{n}/SKILL.md"))
        };
        let skills = vec![
            mk("alpha", "BODY-SECRET-ALPHA"),
            mk("beta", "BODY-SECRET-BETA"),
            mk("gamma", "BODY-SECRET-GAMMA"),
        ];
        let ranked = vec![0, 1, 2];
        let pinned: HashSet<String> = HashSet::new();
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let inv = WorkspaceInventory::default();
        prime_index_with_vectors(&home, tmp.path(), &skills, &service).await;
        let out = semantic_fill(
            &service,
            "p1",
            "deploy the release please",
            &skills,
            &ranked,
            &pinned,
            CancellationToken::new(),
            false,
            Some(&home),
            tmp.path(),
            &inv,
            0.0,
        )
        .await;
        super::super::index::uninstall_prime_index(&home, tmp.path());
        let mut sorted = out.order.clone();
        sorted.sort();
        assert_eq!(
            sorted,
            vec![0, 1, 2],
            "full inventory remains in play: {:?}",
            out.order
        );
        assert!(
            out.hard_error.is_none(),
            "soft fill must not hard-fail: {:?}",
            out.hard_error
        );

        // No body-derived content reaches the executor (not even derived
        // description text).
        for docs in ex.rerank_docs() {
            for d in docs {
                assert!(!d.contains("BODY-SECRET"), "body leaked: {d}");
                assert!(!d.contains("BODY-DERIVED"), "derived desc leaked: {d}");
            }
        }
        for inputs in ex.embed_inputs() {
            for t in inputs {
                assert!(!t.contains("BODY-SECRET"), "body leaked: {t}");
                assert!(!t.contains("BODY-DERIVED"), "derived desc leaked: {t}");
            }
        }
    }

    #[tokio::test]
    async fn semantic_fill_failure_preserves_pre_stage_order_without_body() {
        let ex = Arc::new(RecordingExecutor::new());
        ex.set_fail_rerank(true);
        let service = service_with(ex.clone());
        let skills = vec![
            skill("alpha", "/s/alpha/SKILL.md"),
            skill("beta", "/s/beta/SKILL.md"),
            skill("gamma", "/s/gamma/SKILL.md"),
        ];
        let ranked = vec![0, 1, 2];
        let pinned: HashSet<String> = HashSet::new();
        let inv = WorkspaceInventory::default();
        let out = semantic_fill(
            &service,
            "p1",
            "deploy it",
            &skills,
            &ranked,
            &pinned,
            CancellationToken::new(),
            false,
            None,
            Path::new("."),
            &inv,
            0.0,
        )
        .await;
        assert_eq!(out.order, ranked);
        assert!(!out.degradations.is_empty());
        assert!(out.hard_error.is_none());
    }

    #[tokio::test]
    async fn semantic_fill_hard_mode_surfaces_error() {
        let ex = Arc::new(RecordingExecutor::new());
        ex.set_fail_embed(true); // all embedding routes fail → hard error
        let service = service_with(ex.clone());
        let skills = vec![
            skill("alpha", "/s/alpha/SKILL.md"),
            skill("beta", "/s/beta/SKILL.md"),
        ];
        let ranked = vec![0, 1];
        let pinned: HashSet<String> = HashSet::new();
        let inv = WorkspaceInventory::default();
        let out = semantic_fill(
            &service,
            "p1",
            "deploy it",
            &skills,
            &ranked,
            &pinned,
            CancellationToken::new(),
            true, // hard: degrade_on_error = false
            None,
            Path::new("."),
            &inv,
            0.0,
        )
        .await;
        assert!(out.hard_error.is_some(), "hard mode must surface the error");
        assert!(out.degradations.is_empty());
    }

    #[tokio::test]
    async fn semantic_fill_cancelled_has_no_fake_degradation() {
        let ex = Arc::new(RecordingExecutor::new());
        let service = service_with(ex.clone());
        let skills = vec![skill("alpha", "/s/alpha/SKILL.md")];
        let ranked = vec![0];
        let pinned: HashSet<String> = HashSet::new();
        let token = CancellationToken::new();
        token.cancel();
        let inv = WorkspaceInventory::default();
        let out = semantic_fill(
            &service,
            "p1",
            "deploy it",
            &skills,
            &ranked,
            &pinned,
            token,
            false,
            None,
            Path::new("."),
            &inv,
            0.0,
        )
        .await;
        assert!(out.cancelled);
        assert!(out.degradations.is_empty(), "no fake degradation on cancel");
    }

    #[tokio::test]
    async fn semantic_fill_queries_full_inventory_not_first_eight() {
        let ex = Arc::new(RecordingExecutor::new());
        let service = service_with(ex.clone());
        let skills: Vec<SkillInfo> = (0..12)
            .map(|i| {
                let mut s = skill(&format!("s{i:02}"), &format!("skills/s{i:02}/SKILL.md"));
                s.has_user_specified_description = true;
                s.description = format!("skill {i:02} handles query text matching");
                s
            })
            .collect();
        let ranked: Vec<usize> = (0..12).collect();
        let pinned: HashSet<String> = HashSet::new();
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let inv = WorkspaceInventory::default();
        prime_index_with_vectors(&home, tmp.path(), &skills, &service).await;
        let out = semantic_fill(
            &service,
            "p1",
            "query text",
            &skills,
            &ranked,
            &pinned,
            CancellationToken::new(),
            false,
            Some(&home),
            tmp.path(),
            &inv,
            0.0,
        )
        .await;
        let handle = super::super::index::prime_index_for(&home, tmp.path());
        let listed = handle.list_skill_items().unwrap_or_default();
        super::super::index::uninstall_prime_index(&home, tmp.path());
        assert_eq!(
            listed.len(),
            12,
            "full inventory must be indexed, listed={} shortlist={}",
            listed.len(),
            out.shortlist_size
        );
        assert!(
            out.shortlist_size == 0 || out.shortlist_size > 8,
            "no first-eight semantic cap remains, shortlist={}",
            out.shortlist_size
        );
    }

    #[tokio::test]
    async fn semantic_fill_keeps_explicit_pin_first_without_duplication() {
        let ex = Arc::new(RecordingExecutor::new());
        ex.set_reverse(true);
        let service = service_with(ex.clone());
        let skills = vec![
            skill("alpha", "skills/alpha/SKILL.md"),
            skill("beta", "skills/beta/SKILL.md"),
            skill("gamma", "skills/gamma/SKILL.md"),
        ];
        let ranked = vec![0, 1, 2];
        let mut pinned = HashSet::new();
        pinned.insert("gamma".into());
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let inv = WorkspaceInventory::default();
        prime_index_with_vectors(&home, tmp.path(), &skills, &service).await;
        let out = semantic_fill(
            &service,
            "p1",
            "deploy the release please",
            &skills,
            &ranked,
            &pinned,
            CancellationToken::new(),
            false,
            Some(&home),
            tmp.path(),
            &inv,
            0.0,
        )
        .await;
        super::super::index::uninstall_prime_index(&home, tmp.path());
        assert_eq!(skills[out.order[0]].name, "gamma");
        assert_eq!(
            out.order
                .iter()
                .filter(|&&i| skills[i].name == "gamma")
                .count(),
            1,
            "explicit pin must not be duplicated: {:?}",
            out.order
        );
        let mut unique = out.order.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), out.order.len());
    }

    #[tokio::test]
    async fn smart_search_names_exact_first_or_none_for_local_fallback() {
        let inv = WorkspaceInventory::default();
        assert!(
            smart_search_names(
                None,
                Some("p1"),
                "commit",
                &[],
                None,
                Path::new("."),
                &inv,
                0.0,
                CancellationToken::new(),
            )
            .await
            .is_none(),
            "missing service/home must fall back immediately"
        );

        let ex = Arc::new(RecordingExecutor::new());
        let service = service_with(ex.clone());
        let skills = vec![
            skill("alpha", "skills/alpha/SKILL.md"),
            skill("commit", "skills/commit/SKILL.md"),
            skill("zeta", "skills/zeta/SKILL.md"),
        ];
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        prime_index_with_vectors(&home, tmp.path(), &skills, &service).await;
        let names = smart_search_names(
            Some(&service),
            Some("p1"),
            "commit",
            &skills,
            Some(&home),
            tmp.path(),
            &inv,
            0.0,
            CancellationToken::new(),
        )
        .await
        .expect("smart search should return ranked names");
        super::super::index::uninstall_prime_index(&home, tmp.path());
        assert_eq!(names[0], "commit");
        let commit_hits = names.iter().filter(|n| *n == "commit").count();
        assert_eq!(commit_hits, 1);
    }

    #[tokio::test]
    async fn smart_search_names_returns_none_on_embed_failure_for_local_fallback() {
        let ex = Arc::new(RecordingExecutor::new());
        ex.set_fail_embed(true);
        let service = service_with(ex.clone());
        let skills = vec![
            skill("alpha", "skills/alpha/SKILL.md"),
            skill("beta", "skills/beta/SKILL.md"),
            skill("zeta", "skills/zeta/SKILL.md"),
        ];
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let inv = WorkspaceInventory::default();
        let names = smart_search_names(
            Some(&service),
            Some("p1"),
            "zzzz-unrelated-query",
            &skills,
            Some(&home),
            tmp.path(),
            &inv,
            0.0,
            CancellationToken::new(),
        )
        .await;
        super::super::index::uninstall_prime_index(&home, tmp.path());
        assert!(
            names.is_none(),
            "embed failure without automatic evidence must return None, not Some([]): {names:?}"
        );
    }

    #[tokio::test]
    async fn smart_search_names_embed_failure_substring_query_returns_none() {
        let ex = Arc::new(RecordingExecutor::new());
        ex.set_fail_embed(true);
        let service = service_with(ex.clone());
        let skills = vec![skill("alpha", "skills/alpha/SKILL.md")];
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let inv = WorkspaceInventory::default();
        let names = smart_search_names(
            Some(&service),
            Some("p1"),
            "alp",
            &skills,
            Some(&home),
            tmp.path(),
            &inv,
            0.0,
            CancellationToken::new(),
        )
        .await;
        super::super::index::uninstall_prime_index(&home, tmp.path());
        assert!(
            names.is_none(),
            "FTS whole-token 'alp' must not skip local fallback for skill alpha: {names:?}"
        );
    }

    #[tokio::test]
    async fn smart_search_names_keeps_fused_list_when_only_rerank_is_unavailable() {
        let ex = Arc::new(RecordingExecutor::new());
        ex.set_fail_rerank(true);
        let service = service_with(ex.clone());
        let skills = vec![
            skill("alpha", "skills/alpha/SKILL.md"),
            skill("commit", "skills/commit/SKILL.md"),
        ];
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let inv = WorkspaceInventory::default();
        prime_index_with_vectors(&home, tmp.path(), &skills, &service).await;
        let names = smart_search_names(
            Some(&service),
            Some("p1"),
            "commit",
            &skills,
            Some(&home),
            tmp.path(),
            &inv,
            0.0,
            CancellationToken::new(),
        )
        .await;
        super::super::index::uninstall_prime_index(&home, tmp.path());
        let names = names.expect("RerankUnavailable may still return the fused automatic list");
        assert_eq!(names[0], "commit");
    }

    #[tokio::test]
    async fn semantic_fill_mixed_space_knn_is_not_automatic_evidence() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("ws");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&cwd).unwrap();
        let skills = vec![
            skill("alpha", "skills/alpha/SKILL.md"),
            skill("beta", "skills/beta/SKILL.md"),
        ];
        let handle = super::super::index::prime_index_for(&home, &cwd);
        let items = super::super::index::skills_to_index_items(&skills);
        handle.sync_skills(1, &items).unwrap();
        handle
            .pin_primary_space(PinnedEmbeddingSpace {
                snapshot_generation: 1,
                route_id: "emb-a".into(),
                space_fingerprint: "fp-a".into(),
                spec: EmbeddingSourceSpec {
                    provider_instance_id: "other".into(),
                    incarnation: Some("x".into()),
                    origin_host: "other.example.test".into(),
                    embedding_path: "/v1/embeddings".into(),
                    protocol: "openai_compatible".into(),
                    model: "other-model".into(),
                    dimensions: 4,
                    encoding: "float".into(),
                    normalization: NORMALIZATION_L2_V1.to_owned(),
                },
            })
            .unwrap();
        let embedder = Arc::new(MockEmbeddingProvider { dimensions: 4 });
        handle
            .backfill(
                embedder,
                handle.freeze_pin().expect("frozen A"),
                CancellationToken::new(),
            )
            .await
            .expect("spec A backfill must install a live fingerprint");
        // Hold the live spec-A table: a far-future backoff stops the
        // service pin (spec B, same dimensions) from swapping vectors
        // before KNN. That is the route-change window fill must fail closed.
        {
            let conn = rusqlite::Connection::open(handle.db_path()).unwrap();
            conn.execute(
                "UPDATE collections SET backoff_until = ?1 WHERE name = 'skills'",
                rusqlite::params![i64::MAX],
            )
            .unwrap();
        }
        let query = vec![0.5f32, 0.5, 0.5, 0.5];
        handle
            .pin_from_service(&service_with(Arc::new(RecordingExecutor::new())), "p1")
            .unwrap();
        assert_eq!(
            handle.search_knn(&query, 4).unwrap_err(),
            super::super::index::PrimeIndexError::SpaceMismatch,
            "spec B pin must not score spec A's same-dimension live rows"
        );

        let ex = Arc::new(RecordingExecutor::new());
        let service = service_with(ex.clone());
        let ranked = vec![0, 1];
        let pinned: HashSet<String> = HashSet::new();
        let inv = WorkspaceInventory::default();
        let soft = semantic_fill(
            &service,
            "p1",
            "zzzz-unrelated-query",
            &skills,
            &ranked,
            &pinned,
            CancellationToken::new(),
            false,
            Some(&home),
            &cwd,
            &inv,
            0.0,
        )
        .await;
        assert!(
            soft.hard_error.is_none(),
            "soft mixed-space must not hard-fail"
        );
        assert_eq!(
            soft.shortlist_size, 0,
            "same-dimension spec A rows must not become automatic vector evidence under spec B: {:?}",
            soft.order
        );
        assert!(
            !soft.order.iter().any(|&i| {
                // Unrelated query has no FTS/local hit; mixed-space must not
                // promote via stale vector rank.
                skills[i].name == "alpha" && soft.shortlist_size > 0
            }),
            "soft mixed-space must not compare spaces, got {:?}",
            soft.order
        );

        let hard = semantic_fill(
            &service,
            "p1",
            "zzzz-unrelated-query",
            &skills,
            &ranked,
            &pinned,
            CancellationToken::new(),
            true,
            Some(&home),
            &cwd,
            &inv,
            0.0,
        )
        .await;
        super::super::index::uninstall_prime_index(&home, &cwd);
        assert!(
            hard.hard_error.is_some(),
            "hard mode must surface mixed-space KNN before user-turn insertion"
        );
    }

    #[tokio::test]
    async fn semantic_fill_does_not_block_on_inventory_backfill() {
        let ex = Arc::new(RecordingExecutor::new());
        ex.set_slow_batch_ms(800);
        let service = service_with(ex.clone());
        let skills = vec![
            skill("alpha", "skills/alpha/SKILL.md"),
            skill("beta", "skills/beta/SKILL.md"),
            skill("gamma", "skills/gamma/SKILL.md"),
        ];
        let ranked = vec![0, 1, 2];
        let pinned: HashSet<String> = HashSet::new();
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let inv = WorkspaceInventory::default();
        let embed_before = ex.embed_inputs().len();
        let out = semantic_fill(
            &service,
            "p1",
            "zzzz-unrelated-query",
            &skills,
            &ranked,
            &pinned,
            CancellationToken::new(),
            false,
            Some(&home),
            tmp.path(),
            &inv,
            0.0,
        )
        .await;
        super::super::index::uninstall_prime_index(&home, tmp.path());
        let embed_after = ex.embed_inputs().len();
        assert!(
            embed_after.saturating_sub(embed_before) <= 1,
            "query path must not await the slow inventory embed batch, batches={}",
            embed_after.saturating_sub(embed_before)
        );
        assert!(
            out.hard_error.is_none(),
            "pending vectors must not hard-fail"
        );
        assert_eq!(
            out.shortlist_size, 0,
            "a not-yet-filled collection must not count missing KNN as vector evidence, got {:?}",
            out.order
        );
    }

    #[tokio::test]
    async fn pinned_embedder_honors_caller_cancel() {
        let ex = Arc::new(RecordingExecutor::new());
        let service = service_with(ex);
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("ws");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&cwd).unwrap();
        let handle = super::super::index::prime_index_for(&home, &cwd);
        handle.pin_from_service(&service, "p1").unwrap();
        let token = CancellationToken::new();
        token.cancel();
        let embedder =
            PinnedServiceEmbedder::new(handle.clone(), service.clone(), "p1".to_owned(), token)
                .expect("pinned embedder");
        let err = embedder.embed_batch(&["hello"]).await;
        super::super::index::uninstall_prime_index(&home, &cwd);
        assert!(
            err.is_err(),
            "in-flight backfill embed HTTP must observe the caller cancel token"
        );
    }

    #[tokio::test]
    async fn smart_search_names_uses_workspace_inventory_evidence() {
        let ex = Arc::new(RecordingExecutor::new());
        let service = service_with(ex);
        let skills = vec![
            skill("alpha", "skills/alpha/SKILL.md"),
            skill("commit", "skills/commit/SKILL.md"),
        ];
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        prime_index_with_vectors(&home, tmp.path(), &skills, &service).await;
        let inv = WorkspaceInventory {
            root: PathBuf::from("/ws"),
            entries: vec![InventoryEntry {
                rel: "src/commit.rs".into(),
                size_bytes: 10,
                is_dir: false,
                is_symlink: false,
            }],
            truncated: false,
            cross_device: false,
            lowered_rels: vec![("src/commit.rs".to_string(), "commit.rs".to_string())],
            epoch: 0,
        };
        let names = smart_search_names(
            Some(&service),
            Some("p1"),
            "zzzz-unrelated-query",
            &skills,
            Some(&home),
            tmp.path(),
            &inv,
            0.0,
            CancellationToken::new(),
        )
        .await;
        super::super::index::uninstall_prime_index(&home, tmp.path());
        let names = names.expect("inventory local evidence must admit commit");
        assert_eq!(
            names[0], "commit",
            "workspace inventory name evidence must rank commit first: {names:?}"
        );
    }

    #[tokio::test]
    async fn profile_reload_refuses_stale_pinned_embed() {
        let ex = Arc::new(RecordingExecutor::new());
        let service = service_with(ex.clone());
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("ws");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&cwd).unwrap();
        let handle = super::super::index::prime_index_for(&home, &cwd);
        handle.pin_from_service(&service, "p1").unwrap();
        let mut reloaded = test_snapshot();
        reloaded.generation = 9;
        service.registry().force_publish(Arc::new(reloaded));
        let err = handle
            .embed_texts_pinned(
                &service,
                "p1",
                vec!["query".into()],
                CancellationToken::new(),
            )
            .await
            .unwrap_err();
        super::super::index::uninstall_prime_index(&home, &cwd);
        assert_eq!(
            err,
            super::super::index::PrimeIndexError::EmbedFailed,
            "a profile reload must not complete embeddings into the old pin"
        );
    }

    #[tokio::test]
    async fn semantic_fill_cold_start_runs_fts_without_vector_evidence() {
        let ex = Arc::new(RecordingExecutor::new());
        let service = service_with(ex);
        let skills = vec![
            skill_desc(
                "k8s",
                "skills/k8s/SKILL.md",
                "kubernetes canary rollout playbook",
            ),
            skill("noise", "skills/noise/SKILL.md"),
        ];
        let ranked = vec![0, 1];
        let pinned: HashSet<String> = HashSet::new();
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let inv = WorkspaceInventory::default();
        let soft = semantic_fill(
            &service,
            "p1",
            "kubernetes canary",
            &skills,
            &ranked,
            &pinned,
            CancellationToken::new(),
            false,
            Some(&home),
            tmp.path(),
            &inv,
            0.9,
        )
        .await;
        let hard = semantic_fill(
            &service,
            "p1",
            "kubernetes canary",
            &skills,
            &ranked,
            &pinned,
            CancellationToken::new(),
            true,
            Some(&home),
            tmp.path(),
            &inv,
            0.9,
        )
        .await;
        super::super::index::uninstall_prime_index(&home, tmp.path());
        assert!(soft.hard_error.is_none(), "cold-start must not hard-fail");
        assert!(
            hard.hard_error.is_none(),
            "hard mode must not fail on pending vectors: {:?}",
            hard.hard_error
        );
        assert_eq!(
            soft.order, ranked,
            "soft KNN unavailability must restore exact pre-stage order, got {:?}",
            soft.order
        );
        assert_eq!(
            soft.shortlist_size, 0,
            "description FTS without local/path/vector evidence must not enter the automatic shortlist"
        );
        assert!(
            soft.degradations
                .iter()
                .any(|d| d.kind == crate::retrieval::DegradationKind::SemanticUnavailable),
            "cold-start KNN miss must record SemanticUnavailable: {:?}",
            soft.degradations
        );
    }

    #[tokio::test]
    async fn semantic_fill_embed_failure_still_fuses_fts() {
        let ex = Arc::new(RecordingExecutor::new());
        ex.set_fail_embed(true);
        let service = service_with(ex);
        let skills = vec![
            skill_desc(
                "k8s",
                "skills/k8s/SKILL.md",
                "kubernetes canary rollout playbook",
            ),
            skill("noise", "skills/noise/SKILL.md"),
        ];
        let ranked = vec![0, 1];
        let pinned: HashSet<String> = HashSet::new();
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let inv = WorkspaceInventory::default();
        let soft = semantic_fill(
            &service,
            "p1",
            "kubernetes canary",
            &skills,
            &ranked,
            &pinned,
            CancellationToken::new(),
            false,
            Some(&home),
            tmp.path(),
            &inv,
            0.9,
        )
        .await;
        let hard = semantic_fill(
            &service,
            "p1",
            "kubernetes canary",
            &skills,
            &ranked,
            &pinned,
            CancellationToken::new(),
            true,
            Some(&home),
            tmp.path(),
            &inv,
            0.9,
        )
        .await;
        super::super::index::uninstall_prime_index(&home, tmp.path());
        assert!(
            soft.hard_error.is_none(),
            "soft embed failure must restore pre-stage order, not mixed-space-fail: {:?}",
            soft.hard_error
        );
        assert!(
            hard.hard_error.is_none(),
            "embed unavailability is skipped fusion, not mixed-space: {:?}",
            hard.hard_error
        );
        assert_eq!(
            soft.order, ranked,
            "soft embed failure must keep unevidenced tails in exact pre-stage order, got {:?}",
            soft.order
        );
        assert_eq!(
            hard.order, ranked,
            "hard embed unavailability must keep exact pre-stage order, got {:?}",
            hard.order
        );
        assert_eq!(
            soft.shortlist_size, 0,
            "embed miss must not treat description FTS as automatic membership"
        );
        assert!(
            soft.degradations
                .iter()
                .any(|d| d.kind == crate::retrieval::DegradationKind::SemanticUnavailable),
            "embed miss must record SemanticUnavailable: {:?}",
            soft.degradations
        );
    }

    #[tokio::test]
    async fn semantic_fill_rebuild_pending_runs_fts_and_skips_knn() {
        let ex = Arc::new(RecordingExecutor::new());
        let service = service_with(ex);
        let skills = vec![
            skill_desc(
                "k8s",
                "skills/k8s/SKILL.md",
                "kubernetes canary rollout playbook",
            ),
            skill("noise", "skills/noise/SKILL.md"),
        ];
        let ranked = vec![0, 1];
        let pinned: HashSet<String> = HashSet::new();
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let inv = WorkspaceInventory::default();
        let handle = super::super::index::prime_index_for(&home, tmp.path());
        let items = super::super::index::skills_to_index_items(&skills);
        handle.sync_skills(1, &items).unwrap();
        handle
            .pin_primary_space(PinnedEmbeddingSpace {
                snapshot_generation: 1,
                route_id: "emb-a".into(),
                space_fingerprint: "fp-a".into(),
                spec: EmbeddingSourceSpec {
                    provider_instance_id: "other".into(),
                    incarnation: Some("x".into()),
                    origin_host: "other.example.test".into(),
                    embedding_path: "/v1/embeddings".into(),
                    protocol: "openai_compatible".into(),
                    model: "other-model".into(),
                    dimensions: 4,
                    encoding: "float".into(),
                    normalization: NORMALIZATION_L2_V1.to_owned(),
                },
            })
            .unwrap();
        handle
            .backfill(
                Arc::new(MockEmbeddingProvider { dimensions: 4 }),
                handle.freeze_pin().expect("frozen A"),
                CancellationToken::new(),
            )
            .await
            .expect("spec A backfill");
        {
            let conn = rusqlite::Connection::open(handle.db_path()).unwrap();
            conn.execute(
                "UPDATE collections SET pending_json = ?1, backoff_until = ?2 WHERE name = 'skills'",
                rusqlite::params![
                    r#"{"id":"rebuild-1","intended":"other","status":"pending","claim":"","claimed_at":0,"reason":"test","last_attempt_at":0}"#,
                    i64::MAX
                ],
            )
            .unwrap();
        }
        handle.pin_from_service(&service, "p1").unwrap();
        let query = vec![0.5f32, 0.5, 0.5, 0.5];
        assert_eq!(
            handle.search_knn(&query, 4).unwrap_err(),
            super::super::index::PrimeIndexError::Unavailable,
            "rebuild-pending must be KNN unavailability, not mixed-space"
        );
        let hard = semantic_fill(
            &service,
            "p1",
            "kubernetes canary",
            &skills,
            &ranked,
            &pinned,
            CancellationToken::new(),
            true,
            Some(&home),
            tmp.path(),
            &inv,
            0.9,
        )
        .await;
        super::super::index::uninstall_prime_index(&home, tmp.path());
        assert!(
            hard.hard_error.is_none(),
            "hard mode must not fail on rebuild-pending: {:?}",
            hard.hard_error
        );
        assert_eq!(
            hard.order, ranked,
            "rebuild-pending KNN unavailability must restore exact pre-stage order, got {:?}",
            hard.order
        );
        assert_eq!(
            hard.shortlist_size, 0,
            "pending KNN must not count description FTS as automatic membership"
        );
    }

    #[tokio::test]
    async fn semantic_fill_unavailable_knn_still_fuses_fts_hit() {
        let ex = Arc::new(RecordingExecutor::new());
        let service = service_with(ex);
        let skills = vec![
            skill_desc("commit", "skills/commit/SKILL.md", "commit message helper"),
            skill_desc("zzzz", "skills/zzzz/SKILL.md", "unrelated zebra token"),
        ];
        let ranked = vec![0, 1];
        let pinned: HashSet<String> = HashSet::new();
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let inv = WorkspaceInventory::default();
        let out = semantic_fill(
            &service,
            "p1",
            "commit message",
            &skills,
            &ranked,
            &pinned,
            CancellationToken::new(),
            false,
            Some(&home),
            tmp.path(),
            &inv,
            0.5,
        )
        .await;
        super::super::index::uninstall_prime_index(&home, tmp.path());
        assert!(out.hard_error.is_none());
        assert_eq!(
            out.order, ranked,
            "unavailable KNN must restore exact pre-stage order, got {:?}",
            out.order
        );
        assert_eq!(
            out.shortlist_size, 0,
            "a description-only FTS hit without local/path/vector evidence must not enter the automatic shortlist, shortlist={}",
            out.shortlist_size
        );
        assert!(
            out.degradations
                .iter()
                .any(|d| d.kind == crate::retrieval::DegradationKind::SemanticUnavailable),
            "KNN unavailability must record SemanticUnavailable: {:?}",
            out.degradations
        );
    }

    #[tokio::test]
    async fn semantic_fill_bm25_reorders_admitted_rows_without_fts_only_admission() {
        let ex = Arc::new(RecordingExecutor::new());
        let service = service_with(ex);
        let skills = vec![
            skill("aardvark", "skills/aardvark/SKILL.md"),
            SkillInfo {
                when_to_use: Some("kubernetes canary playbook".into()),
                has_user_specified_description: true,
                description: "kubernetes canary rollout playbook".into(),
                ..skill("k8s", "skills/k8s/SKILL.md")
            },
        ];
        let inv = WorkspaceInventory {
            root: PathBuf::from("/ws"),
            entries: vec![InventoryEntry {
                rel: "src/aardvark.rs".into(),
                size_bytes: 10,
                is_dir: false,
                is_symlink: false,
            }],
            truncated: false,
            cross_device: false,
            lowered_rels: vec![("src/aardvark.rs".to_string(), "aardvark.rs".to_string())],
            epoch: 0,
        };
        let ranked = rank_skills(&skills, "kubernetes canary", &inv, None);
        assert_eq!(
            skills[ranked[0]].name, "aardvark",
            "pre-stage local tie must prefer aardvark by name before BM25"
        );
        let pinned: HashSet<String> = HashSet::new();
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        prime_index_with_vectors(&home, tmp.path(), &skills, &service).await;
        let out = semantic_fill(
            &service,
            "p1",
            "kubernetes canary",
            &skills,
            &ranked,
            &pinned,
            CancellationToken::new(),
            false,
            Some(&home),
            tmp.path(),
            &inv,
            0.9,
        )
        .await;
        super::super::index::uninstall_prime_index(&home, tmp.path());
        assert!(out.hard_error.is_none());
        assert_eq!(
            out.shortlist_size, 2,
            "local evidence must admit both skills, shortlist={} order={:?}",
            out.shortlist_size, out.order
        );
        assert_eq!(
            skills[out.order[0]].name, "k8s",
            "BM25 must reorder already-admitted rows so the FTS-stronger local hit is first: {:?}",
            out.order
        );
    }

    #[tokio::test]
    async fn semantic_fill_missing_knn_is_not_threshold_evidence() {
        let ex = Arc::new(RecordingExecutor::new());
        let service = service_with(ex);
        let skills = vec![
            skill("alpha", "skills/alpha/SKILL.md"),
            skill("beta", "skills/beta/SKILL.md"),
        ];
        let ranked = vec![0, 1];
        let pinned: HashSet<String> = HashSet::new();
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let inv = WorkspaceInventory::default();
        let out = semantic_fill(
            &service,
            "p1",
            "zzzz-unrelated-query",
            &skills,
            &ranked,
            &pinned,
            CancellationToken::new(),
            false,
            Some(&home),
            tmp.path(),
            &inv,
            0.0,
        )
        .await;
        super::super::index::uninstall_prime_index(&home, tmp.path());
        assert!(out.hard_error.is_none());
        assert_eq!(
            out.shortlist_size, 0,
            "threshold 0 must not treat missing KNN as vector evidence: {:?}",
            out.order
        );
    }

    #[tokio::test]
    async fn semantic_fill_query_embed_swap_to_spec_b_is_not_vector_evidence() {
        let ex = Arc::new(RecordingExecutor::new());
        ex.set_slow_ms(250);
        let service = service_with(ex.clone());
        let skills = vec![
            skill("alpha", "skills/alpha/SKILL.md"),
            skill("beta", "skills/beta/SKILL.md"),
        ];
        let ranked = vec![0, 1];
        let pinned: HashSet<String> = HashSet::new();
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let inv = WorkspaceInventory::default();
        prime_index_with_vectors(&home, tmp.path(), &skills, &service).await;
        let handle = super::super::index::prime_index_for(&home, tmp.path());
        let before = ex.embed_inputs().len();
        let swap = tokio::spawn({
            let handle = handle.clone();
            let ex = ex.clone();
            async move {
                loop {
                    if ex
                        .embed_inputs()
                        .iter()
                        .skip(before)
                        .any(|batch| batch.len() == 1)
                    {
                        handle
                            .pin_primary_space(PinnedEmbeddingSpace {
                                snapshot_generation: 9,
                                route_id: "emb-2".into(),
                                space_fingerprint: "fp-b".into(),
                                spec: spec_b_same_dim(),
                            })
                            .unwrap();
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                }
            }
        });
        let soft = semantic_fill(
            &service,
            "p1",
            "zzzz-unrelated-query",
            &skills,
            &ranked,
            &pinned,
            CancellationToken::new(),
            false,
            Some(&home),
            tmp.path(),
            &inv,
            0.0,
        )
        .await;
        let _ = swap.await;
        assert!(soft.hard_error.is_none());
        assert_eq!(
            soft.shortlist_size, 0,
            "spec B swap after spec A query embed must not keep stale vector rank: {:?}",
            soft.order
        );

        let before_hard = ex.embed_inputs().len();
        let swap_hard = tokio::spawn({
            let handle = handle.clone();
            let ex = ex.clone();
            async move {
                loop {
                    if ex
                        .embed_inputs()
                        .iter()
                        .skip(before_hard)
                        .any(|batch| batch.len() == 1)
                    {
                        handle
                            .pin_primary_space(PinnedEmbeddingSpace {
                                snapshot_generation: 10,
                                route_id: "emb-2".into(),
                                space_fingerprint: "fp-b".into(),
                                spec: spec_b_same_dim(),
                            })
                            .unwrap();
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                }
            }
        });
        let hard = semantic_fill(
            &service,
            "p1",
            "zzzz-unrelated-query",
            &skills,
            &ranked,
            &pinned,
            CancellationToken::new(),
            true,
            Some(&home),
            tmp.path(),
            &inv,
            0.0,
        )
        .await;
        let _ = swap_hard.await;
        super::super::index::uninstall_prime_index(&home, tmp.path());
        assert!(
            hard.hard_error.is_some(),
            "hard mode must fail before user-turn insertion when the pin swaps during query embed"
        );
    }

    #[tokio::test]
    async fn semantic_fill_table_swap_to_spec_b_is_not_vector_evidence() {
        let ex = Arc::new(RecordingExecutor::new());
        let service = service_with(ex.clone());
        let skills = vec![
            skill("alpha", "skills/alpha/SKILL.md"),
            skill("beta", "skills/beta/SKILL.md"),
        ];
        let ranked = vec![0, 1];
        let pinned: HashSet<String> = HashSet::new();
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let inv = WorkspaceInventory::default();
        prime_index_with_vectors(&home, tmp.path(), &skills, &service).await;
        let handle = super::super::index::prime_index_for(&home, tmp.path());
        let pin_before = handle.pinned_space().expect("service pin");
        super::super::index::install_collection_space_for_tests(&handle, spec_b_same_dim()).await;
        assert_eq!(
            handle.pinned_space().unwrap().route_id,
            pin_before.route_id,
            "live table swap must leave the handle pin on spec A"
        );

        let soft = semantic_fill(
            &service,
            "p1",
            "zzzz-unrelated-query",
            &skills,
            &ranked,
            &pinned,
            CancellationToken::new(),
            false,
            Some(&home),
            tmp.path(),
            &inv,
            0.0,
        )
        .await;
        assert!(soft.hard_error.is_none());
        assert_eq!(
            soft.shortlist_size, 0,
            "spec A query against a spec B table must not become automatic vector evidence: {:?}",
            soft.order
        );

        let hard = semantic_fill(
            &service,
            "p1",
            "zzzz-unrelated-query",
            &skills,
            &ranked,
            &pinned,
            CancellationToken::new(),
            true,
            Some(&home),
            tmp.path(),
            &inv,
            0.0,
        )
        .await;
        super::super::index::uninstall_prime_index(&home, tmp.path());
        assert!(
            hard.hard_error.is_some(),
            "hard mode must fail before user-turn insertion when the live table is a different space"
        );
    }

    #[tokio::test]
    async fn semantic_fill_query_embed_table_swap_to_spec_b_is_not_vector_evidence() {
        let ex = Arc::new(RecordingExecutor::new());
        ex.set_slow_ms(250);
        let service = service_with(ex.clone());
        let skills = vec![
            skill("alpha", "skills/alpha/SKILL.md"),
            skill("beta", "skills/beta/SKILL.md"),
        ];
        let ranked = vec![0, 1];
        let pinned: HashSet<String> = HashSet::new();
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let inv = WorkspaceInventory::default();
        prime_index_with_vectors(&home, tmp.path(), &skills, &service).await;
        let handle = super::super::index::prime_index_for(&home, tmp.path());
        let before = ex.embed_inputs().len();
        let swap = tokio::spawn({
            let handle = handle.clone();
            let ex = ex.clone();
            async move {
                loop {
                    if ex
                        .embed_inputs()
                        .iter()
                        .skip(before)
                        .any(|batch| batch.len() == 1)
                    {
                        super::super::index::install_collection_space_for_tests(
                            &handle,
                            spec_b_same_dim(),
                        )
                        .await;
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                }
            }
        });
        let soft = semantic_fill(
            &service,
            "p1",
            "zzzz-unrelated-query",
            &skills,
            &ranked,
            &pinned,
            CancellationToken::new(),
            false,
            Some(&home),
            tmp.path(),
            &inv,
            0.0,
        )
        .await;
        let _ = swap.await;
        assert!(soft.hard_error.is_none());
        assert_eq!(
            soft.shortlist_size, 0,
            "swapping the live table to spec B after a spec A query embed must not keep stale vector rank: {:?}",
            soft.order
        );

        let before_hard = ex.embed_inputs().len();
        let swap_hard = tokio::spawn({
            let handle = handle.clone();
            let ex = ex.clone();
            async move {
                loop {
                    if ex
                        .embed_inputs()
                        .iter()
                        .skip(before_hard)
                        .any(|batch| batch.len() == 1)
                    {
                        super::super::index::install_collection_space_for_tests(
                            &handle,
                            spec_b_same_dim(),
                        )
                        .await;
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                }
            }
        });
        let hard = semantic_fill(
            &service,
            "p1",
            "zzzz-unrelated-query",
            &skills,
            &ranked,
            &pinned,
            CancellationToken::new(),
            true,
            Some(&home),
            tmp.path(),
            &inv,
            0.0,
        )
        .await;
        let _ = swap_hard.await;
        super::super::index::uninstall_prime_index(&home, tmp.path());
        assert!(
            hard.hard_error.is_some(),
            "hard mode must fail before user-turn insertion when the live table swaps during query embed"
        );
    }

    #[tokio::test]
    async fn pinned_embedder_uses_frozen_pin_not_live_handle() {
        let ex = Arc::new(RecordingExecutor::new());
        ex.set_space_vector("emb-model", vec![1.0, 0.0, 0.0, 0.0]);
        ex.set_space_vector("emb-model-b", vec![0.0, 1.0, 0.0, 0.0]);
        let service = service_with(ex.clone());
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("ws");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&cwd).unwrap();
        let handle = super::super::index::prime_index_for(&home, &cwd);
        handle.pin_from_service(&service, "p1").unwrap();
        let frozen = handle.freeze_pin().expect("frozen A");
        let embedder = PinnedServiceEmbedder::with_frozen_pin(
            handle.clone(),
            service.clone(),
            "p1".to_owned(),
            frozen,
            CancellationToken::new(),
        );
        handle
            .pin_primary_space(PinnedEmbeddingSpace {
                snapshot_generation: 9,
                route_id: "emb-2".into(),
                space_fingerprint: "fp-b".into(),
                spec: spec_b_same_dim(),
            })
            .unwrap();
        let out = embedder
            .embed_batch(&["hello"])
            .await
            .expect("frozen A embed must still run");
        super::super::index::uninstall_prime_index(&home, &cwd);
        assert!(
            ex.embed_models().iter().all(|m| m == "emb-model"),
            "frozen embedder must not retarget the live B route, models={:?}",
            ex.embed_models()
        );
        assert!(
            out[0][0] > out[0][1],
            "frozen A marker must not become a B vector"
        );
    }

    #[tokio::test]
    async fn backfill_pinned_embedder_refuses_same_dimension_live_swap_before_start() {
        let ex = Arc::new(RecordingExecutor::new());
        ex.set_space_vector("emb-model", vec![1.0, 0.0, 0.0, 0.0]);
        ex.set_space_vector("emb-model-b", vec![0.0, 1.0, 0.0, 0.0]);
        let service = service_with(ex);
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("ws");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&cwd).unwrap();
        let handle = super::super::index::prime_index_for(&home, &cwd);
        let items =
            super::super::index::skills_to_index_items(&[skill("alpha", "skills/alpha/SKILL.md")]);
        handle.sync_skills(1, &items).unwrap();
        handle.pin_from_service(&service, "p1").unwrap();
        let frozen_a = handle.freeze_pin().expect("frozen A");
        let embedder = Arc::new(PinnedServiceEmbedder::with_frozen_pin(
            handle.clone(),
            service.clone(),
            "p1".to_owned(),
            frozen_a.clone(),
            CancellationToken::new(),
        ));
        handle
            .pin_primary_space(PinnedEmbeddingSpace {
                snapshot_generation: 9,
                route_id: "emb-2".into(),
                space_fingerprint: "fp-b".into(),
                spec: spec_b_same_dim(),
            })
            .unwrap();
        let err = handle
            .backfill(embedder, frozen_a, CancellationToken::new())
            .await
            .err();
        let state = {
            let idx = xai_grok_memory::MetadataIndex::open_or_create(handle.db_path()).unwrap();
            idx.collection_state(xai_grok_memory::CollectionKind::Skills)
                .unwrap()
        };
        let hash_b = xai_grok_memory::VectorFingerprint::build(
            spec_b_same_dim(),
            xai_grok_memory::metadata_doc_prep(xai_grok_memory::CollectionKind::Skills),
            xai_grok_memory::VECTOR_SCHEMA_VERSION,
        )
        .unwrap()
        .0
        .hash;
        super::super::index::uninstall_prime_index(&home, &cwd);
        assert_eq!(
            err,
            Some(super::super::index::PrimeIndexError::SpaceMismatch),
            "PinnedServiceEmbedder A + live B before backfill must refuse"
        );
        assert_ne!(
            state.fingerprint_hash, hash_b,
            "must not install A vectors under B fingerprint"
        );
        assert_eq!(
            state.vec_count, 0,
            "must not commit mixed-space rows after a pre-start swap"
        );
    }

    #[tokio::test]
    async fn semantic_fill_rerank_docs_omit_absolute_unc_file_url_and_bodies() {
        let ex = Arc::new(RecordingExecutor::new());
        let service = service_with(ex.clone());
        let mut dirty = skill_wtu("deploy", "skills/deploy/SKILL.md", "deploy the release");
        dirty.body = Some("SECRET-BODY".into());
        dirty.paths = Some(vec![
            "/Users/secret/file".into(),
            r"\\server\share\file".into(),
            "file:///etc/passwd".into(),
        ]);
        let mut clean = skill_wtu("format", "skills/format/SKILL.md", "format rust code");
        clean.paths = Some(vec!["src/**".into()]);
        clean.body = Some("CLEAN-BODY-MUST-OMIT".into());
        let skills = vec![dirty, clean];
        let ranked = vec![0, 1];
        let pinned: HashSet<String> = HashSet::new();
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let inv = WorkspaceInventory::default();
        prime_index_with_vectors(&home, tmp.path(), &skills, &service).await;
        let _out = semantic_fill(
            &service,
            "p1",
            "please format rust code and deploy the release",
            &skills,
            &ranked,
            &pinned,
            CancellationToken::new(),
            false,
            Some(&home),
            tmp.path(),
            &inv,
            0.0,
        )
        .await;
        super::super::index::uninstall_prime_index(&home, tmp.path());
        let docs = ex.rerank_docs();
        assert!(
            !docs.is_empty(),
            "rerank must run for a fused shortlist with a configured reranker"
        );
        for batch in &docs {
            for d in batch {
                assert!(!d.contains("SECRET-BODY"), "body leaked: {d}");
                assert!(!d.contains("CLEAN-BODY-MUST-OMIT"), "body leaked: {d}");
                assert!(!d.contains("/Users/"), "absolute path leaked: {d}");
                assert!(!d.contains("file:"), "file URL leaked: {d}");
                assert!(!d.contains("\\\\"), "UNC leaked: {d}");
                assert!(!d.contains("server"), "UNC share leaked: {d}");
            }
        }
        let joined = docs
            .iter()
            .flatten()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            joined.contains("src/**"),
            "accepted relative path must remain: {joined}"
        );
        assert!(
            joined.contains("format rust code"),
            "accepted bounded trigger must remain: {joined}"
        );
    }

    #[tokio::test]
    async fn semantic_fill_rerank_docs_omit_userinfo_and_encoded_paths() {
        let ex = Arc::new(RecordingExecutor::new());
        let service = service_with(ex.clone());
        let mut dirty = skill_wtu("deploy", "skills/deploy/SKILL.md", "deploy the release");
        dirty.body = Some("SECRET-BODY".into());
        dirty.paths = Some(vec![
            "%2FUsers%2Fsecret".into(),
            "%2e%2e/%2e%2e/etc/passwd".into(),
            r"%5c%5cserver%5cshare".into(),
            "file://user:secret@localhost/etc/passwd".into(),
            "file:%2f%2f%2fetc/passwd".into(),
            "https://user:pass@example.com/hidden".into(),
            "src/**".into(),
        ]);
        let mut clean = skill_wtu("format", "skills/format/SKILL.md", "format rust code");
        clean.paths = Some(vec!["src/**".into()]);
        let skills = vec![dirty, clean];
        let ranked = vec![0, 1];
        let pinned: HashSet<String> = HashSet::new();
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let inv = WorkspaceInventory::default();
        prime_index_with_vectors(&home, tmp.path(), &skills, &service).await;
        let _out = semantic_fill(
            &service,
            "p1",
            "please format rust code and deploy the release",
            &skills,
            &ranked,
            &pinned,
            CancellationToken::new(),
            false,
            Some(&home),
            tmp.path(),
            &inv,
            0.0,
        )
        .await;
        super::super::index::uninstall_prime_index(&home, tmp.path());
        let docs = ex.rerank_docs();
        assert!(
            !docs.is_empty(),
            "rerank must run for a fused shortlist with a configured reranker"
        );
        for batch in &docs {
            for d in batch {
                assert!(!d.contains("SECRET-BODY"), "body leaked: {d}");
                assert!(!d.contains("user:"), "userinfo leaked: {d}");
                assert!(!d.contains("secret"), "credential leaked: {d}");
                assert!(!d.contains("/Users/"), "encoded absolute leaked: {d}");
                assert!(!d.contains("%2FUsers"), "encoded absolute leaked: {d}");
                assert!(!d.contains("etc/passwd"), "encoded traversal leaked: {d}");
                assert!(!d.contains("server"), "encoded UNC leaked: {d}");
                assert!(!d.contains("file:"), "file URL leaked: {d}");
            }
        }
        let joined = docs
            .iter()
            .flatten()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            joined.contains("src/**"),
            "accepted relative path must remain: {joined}"
        );
        assert!(
            joined.contains("format rust code") || joined.contains("deploy the release"),
            "accepted bounded trigger must remain: {joined}"
        );
    }

    // ── Revalidation / bounded body loading ───────────────────────────

    #[tokio::test]
    async fn native_body_load_frontmatter_and_internal_links_bounded() {
        let tmp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        let good_dir = root.join("skills").join("good");
        std::fs::create_dir_all(&good_dir).unwrap();
        std::fs::write(
            good_dir.join("SKILL.md"),
            "---\nname: good\ndescription: A loadable skill used in body tests.\n---\nLoad this file.\nSee [doc](notes.md).\n",
        )
        .unwrap();
        std::fs::write(good_dir.join("notes.md"), "Shared reference content.\n").unwrap();

        // Unreadable: a directory at the SKILL.md path (not a regular file).
        let bad_dir = root.join("skills").join("bad");
        std::fs::create_dir_all(bad_dir.join("SKILL.md")).unwrap();

        // Non-SKILL.md regular file → not loadable (arbitrary files refused).
        let secrets = root.join("secrets");
        std::fs::create_dir_all(&secrets).unwrap();
        std::fs::write(secrets.join(".env"), "SECRET=1").unwrap();

        let skills = vec![
            SkillInfo {
                name: "good".into(),
                path: good_dir.join("SKILL.md").to_string_lossy().to_string(),
                scope: SkillScope::Local,
                ..SkillInfo::default()
            },
            SkillInfo {
                name: "bad".into(),
                path: bad_dir.join("SKILL.md").to_string_lossy().to_string(),
                scope: SkillScope::Local,
                ..SkillInfo::default()
            },
            SkillInfo {
                name: "env".into(),
                path: secrets.join(".env").to_string_lossy().to_string(),
                scope: SkillScope::Local,
                ..SkillInfo::default()
            },
        ];
        let refresh = refresher(skills.clone());
        let cancel = CancellationToken::new();
        let batch =
            load_and_revalidate(&skills, &[0, 1, 2], 3, &refresh, &[root.clone()], &cancel).await;

        assert_eq!(batch.drop_reasons.len(), 2);
        assert!(
            batch
                .drop_reasons
                .iter()
                .any(|r| *r == PrimeDropReason::Unreadable)
        );
        assert!(
            batch
                .drop_reasons
                .iter()
                .any(|r| *r == PrimeDropReason::NotContained)
        );
        assert_eq!(batch.loaded.len(), 1);
        let body = &batch.loaded[0].body;
        assert!(body.contains("Load this file."));
        assert!(!body.contains("name: good"), "frontmatter leaked");
        assert!(
            body.contains(&good_dir.join("notes.md").to_string_lossy().to_string()),
            "internal link not resolved: {body}"
        );
    }

    #[tokio::test]
    async fn symlink_at_skill_path_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        let outside_file = outside.path().join("secret.md");
        std::fs::write(&outside_file, "outside secret").unwrap();
        let esc_dir = root.join("skills").join("esc");
        std::fs::create_dir_all(&esc_dir).unwrap();
        std::os::unix::fs::symlink(&outside_file, esc_dir.join("SKILL.md")).unwrap();

        let skills = vec![SkillInfo {
            name: "esc".into(),
            path: esc_dir.join("SKILL.md").to_string_lossy().to_string(),
            scope: SkillScope::Local,
            ..SkillInfo::default()
        }];
        let refresh = refresher(skills.clone());
        let cancel = CancellationToken::new();
        let batch = load_and_revalidate(&skills, &[0], 1, &refresh, &[root.clone()], &cancel).await;
        assert_eq!(batch.drop_reasons, vec![PrimeDropReason::NotContained]);
        assert!(batch.loaded.is_empty());
    }

    #[tokio::test]
    async fn unofficial_skill_is_quarantined_and_not_primed() {
        let tmp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        let sdir = root.join("skills").join("leaky");
        std::fs::create_dir_all(&sdir).unwrap();
        std::fs::write(
            sdir.join("SKILL.md"),
            "---\nname: leaky\ndescription: A quarantined skill.\nwhen-to-use: secret-token\n---\n# Leaky\n",
        )
        .unwrap();
        let skills = vec![SkillInfo {
            name: "leaky".into(),
            path: sdir.join("SKILL.md").to_string_lossy().to_string(),
            scope: SkillScope::Local,
            ..SkillInfo::default()
        }];
        let refresh = refresher(skills.clone());
        let cancel = CancellationToken::new();
        let batch = load_and_revalidate(&skills, &[0], 1, &refresh, &[root.clone()], &cancel).await;
        assert_eq!(batch.drop_reasons, vec![PrimeDropReason::Quarantined]);
        assert!(batch.loaded.is_empty());
        let dump = format!("{:?}", batch.drop_reasons);
        assert!(!dump.contains("secret-token"));
    }

    #[tokio::test]
    async fn revalidation_drops_skill_that_became_ineligible() {
        let tmp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        let sdir = root.join("skills").join("a");
        std::fs::create_dir_all(&sdir).unwrap();
        std::fs::write(sdir.join("SKILL.md"), "# Skill A\n").unwrap();
        let path = sdir.join("SKILL.md").to_string_lossy().to_string();

        let initial = vec![SkillInfo {
            name: "a".into(),
            path: path.clone(),
            ..SkillInfo::default()
        }];
        // Between rank and load, the skill is disabled in the fresh snapshot.
        let disabled = vec![SkillInfo {
            name: "a".into(),
            path,
            enabled: false,
            ..SkillInfo::default()
        }];
        let refresh = refresher(disabled);
        let cancel = CancellationToken::new();
        let batch =
            load_and_revalidate(&initial, &[0], 1, &refresh, &[root.clone()], &cancel).await;
        assert_eq!(batch.drop_reasons, vec![PrimeDropReason::ChangedOrGone]);
        assert!(batch.loaded.is_empty());
    }

    #[tokio::test]
    async fn backfill_promotes_next_ranked_candidate_after_drop() {
        let tmp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        let gdir = root.join("skills").join("good");
        std::fs::create_dir_all(&gdir).unwrap();
        std::fs::write(
            gdir.join("SKILL.md"),
            "---\nname: good\ndescription: A loadable skill used in backfill tests.\n---\n# Good\n",
        )
        .unwrap();
        let gpath = gdir.join("SKILL.md").to_string_lossy().to_string();
        let missing = SkillInfo {
            name: "missing".into(),
            path: root
                .join("skills/missing/SKILL.md")
                .to_string_lossy()
                .to_string(),
            scope: SkillScope::Local,
            ..SkillInfo::default()
        };
        let good = SkillInfo {
            name: "good".into(),
            path: gpath,
            scope: SkillScope::Local,
            ..SkillInfo::default()
        };
        let skills = vec![missing.clone(), good.clone()];
        let refresh = refresher(skills.clone());
        let cancel = CancellationToken::new();
        // order=[0 (missing, drops), 1 (good, loaded)]; target=1 → backfill
        // previously "missing" would fully fill the target; now good is promoted.
        let batch = load_and_revalidate(&skills, &[0, 1], 1, &refresh, &[root], &cancel).await;
        assert_eq!(batch.loaded.len(), 1);
        assert_eq!(
            batch.loaded[0].name, "good",
            "backfill promoted the next candidate"
        );
        assert!(
            batch
                .drop_reasons
                .iter()
                .any(|r| *r == PrimeDropReason::ChangedOrGone)
        );
    }

    #[tokio::test]
    async fn fifo_at_skill_path_is_rejected_without_hang() {
        use nix::sys::stat::Mode;
        use nix::unistd::mkfifo;
        let tmp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        let fdir = root.join("skills").join("fifo");
        std::fs::create_dir_all(&fdir).unwrap();
        let fifo = fdir.join("SKILL.md");
        mkfifo(&fifo, Mode::S_IRUSR | Mode::S_IWUSR).unwrap();

        let skills = vec![SkillInfo {
            name: "fifo".into(),
            path: fifo.to_string_lossy().to_string(),
            scope: SkillScope::Local,
            ..SkillInfo::default()
        }];
        let refresh = refresher(skills.clone());
        let cancel = CancellationToken::new();
        // Must not hang, and returns Unreadable (non-regular file).
        let batch = load_and_revalidate(&skills, &[0], 1, &refresh, &[root], &cancel).await;
        assert_eq!(batch.drop_reasons, vec![PrimeDropReason::Unreadable]);
        assert!(batch.loaded.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn openat_walk_rejects_intermediate_dir_symlink() {
        use std::os::unix::fs::symlink;
        let tmp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("SKILL.md"), "outside").unwrap();
        let sub_dir = root.join("skills");
        std::fs::create_dir_all(&sub_dir).unwrap();
        // `skills/sub` is a symlink to an external directory.
        symlink(outside.path(), sub_dir.join("sub")).unwrap();

        let rel = Path::new("skills/sub/SKILL.md");
        let fallback = root.join("skills/sub/SKILL.md");
        let res = read_body_bounded(&root, rel, &fallback);
        assert!(
            res.is_err(),
            "an intermediate-directory symlink must fail O_NOFOLLOW in the openat walk"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn intermediate_symlinked_dir_skill_refused_via_snapshot() {
        use std::os::unix::fs::symlink;
        let tmp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("SKILL.md"), "outside secret").unwrap();
        let sub_dir = root.join("skills");
        std::fs::create_dir_all(&sub_dir).unwrap();
        symlink(outside.path(), sub_dir.join("sub")).unwrap();

        let skills = vec![SkillInfo {
            name: "sub".into(),
            path: root
                .join("skills/sub/SKILL.md")
                .to_string_lossy()
                .to_string(),
            scope: SkillScope::Local,
            ..SkillInfo::default()
        }];
        let refresh = refresher(skills.clone());
        let cancel = CancellationToken::new();
        let batch = load_and_revalidate(&skills, &[0], 1, &refresh, &[root.clone()], &cancel).await;
        // The canonical path resolves outside the trusted root → refused.
        assert_eq!(batch.drop_reasons, vec![PrimeDropReason::NotContained]);
        assert!(batch.loaded.is_empty());
    }

    fn official_prime_skill(name: &str, description: &str, body: &str) -> String {
        format!("---\nname: {name}\ndescription: {description}\n---\n{body}")
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn prime_drops_leaf_symlink_to_in_root_file_after_nofollow_read() {
        let tmp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        let sdir = root.join("skills").join("good");
        std::fs::create_dir_all(&sdir).unwrap();
        let skill_path = sdir.join("SKILL.md");
        std::fs::write(
            &skill_path,
            official_prime_skill(
                "good",
                "A loadable skill used in prime tests.",
                "Trusted body.\n",
            ),
        )
        .unwrap();
        let evil = root.join("evil.md");
        std::fs::write(&evil, "EVIL_SECRET_BODY\n").unwrap();

        let skills = vec![SkillInfo {
            name: "good".into(),
            path: skill_path.to_string_lossy().to_string(),
            scope: SkillScope::Local,
            ..SkillInfo::default()
        }];
        let swap_path = skill_path.clone();
        xai_grok_tools::implementations::skills::strict::set_after_nofollow_read_hook(move || {
            std::fs::remove_file(&swap_path).unwrap();
            std::os::unix::fs::symlink(&evil, &swap_path).unwrap();
        });
        let refresh = refresher(skills.clone());
        let cancel = CancellationToken::new();
        let batch = load_and_revalidate(&skills, &[0], 1, &refresh, &[root.clone()], &cancel).await;
        assert!(
            batch.loaded.is_empty(),
            "symlinked in-root body must not be primed"
        );
        assert!(
            batch
                .drop_reasons
                .iter()
                .any(|r| *r == PrimeDropReason::NotContained || *r == PrimeDropReason::Quarantined)
        );
        assert!(
            batch
                .loaded
                .iter()
                .all(|skill| !skill.body.contains("EVIL_SECRET_BODY")
                    && !skill.body.contains("Trusted body"))
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn prime_drops_unofficial_overwrite_after_nofollow_read() {
        let tmp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        let sdir = root.join("skills").join("good");
        std::fs::create_dir_all(&sdir).unwrap();
        let skill_path = sdir.join("SKILL.md");
        std::fs::write(
            &skill_path,
            official_prime_skill(
                "good",
                "A loadable skill used in prime tests.",
                "Trusted body.\n",
            ),
        )
        .unwrap();

        let skills = vec![SkillInfo {
            name: "good".into(),
            path: skill_path.to_string_lossy().to_string(),
            scope: SkillScope::Local,
            ..SkillInfo::default()
        }];
        let swap_path = skill_path.clone();
        xai_grok_tools::implementations::skills::strict::set_after_nofollow_read_hook(move || {
            std::fs::write(
                &swap_path,
                "---\nname: good\nwhen-to-use: secret-token\n---\nEVIL_UNOFFICIAL\n",
            )
            .unwrap();
        });
        let refresh = refresher(skills.clone());
        let cancel = CancellationToken::new();
        let batch = load_and_revalidate(&skills, &[0], 1, &refresh, &[root.clone()], &cancel).await;
        assert!(
            batch.loaded.is_empty(),
            "post-read unofficial overwrite must not be primed"
        );
        assert_eq!(batch.drop_reasons, vec![PrimeDropReason::Quarantined]);
        assert!(batch.loaded.iter().all(|skill| {
            !skill.body.contains("secret-token")
                && !skill.body.contains("EVIL_UNOFFICIAL")
                && !skill.body.contains("Trusted body")
        }));
        let dump = format!("{:?}", batch.drop_reasons);
        assert!(!dump.contains("secret-token"));
    }

    #[test]
    fn utf8_prefix_trim_preserves_valid_utf8_at_cap_boundary() {
        let max = MAX_LOADED_BODY_BYTES as usize;
        // `a` * (max-2) + emoji (4 bytes) = max+2 bytes; the cap boundary lands
        // mid-emoji.
        let body = format!("{}🙂", "a".repeat(max - 2));
        assert_eq!(body.len(), max + 2);
        let mut buf = body.into_bytes();
        utf8_prefix_trim(&mut buf, max);
        assert_eq!(buf.len(), max - 2, "must back up to a char boundary");
        assert!(String::from_utf8(buf).is_ok(), "result must be valid UTF-8");
    }

    #[test]
    fn utf8_prefix_trim_exact_boundaries_2_3_4_byte_chars() {
        // Helper: trim `bytes` at `max` and return the valid prefix as a String.
        let trim = |bytes: &[u8], max: usize| {
            let mut buf = bytes.to_vec();
            utf8_prefix_trim(&mut buf, max);
            String::from_utf8(buf).expect("trim must yield valid UTF-8")
        };

        // 2-byte char at the exact boundary: "éa" (C3 A9 61), max=2 → "é".
        assert_eq!(trim("éa".as_bytes(), 2), "é");
        // 3-byte char at the exact boundary: "你a" (E4 BD A0 61), max=3 → "你".
        assert_eq!(trim("你a".as_bytes(), 3), "你");
        // 4-byte char at the exact boundary: "🙂a" (F0 9F 99 82 61), max=4 → "🙂".
        assert_eq!(trim("🙂a".as_bytes(), 4), "🙂");

        // Mid-codepoint caps back up to the previous boundary without dropping a
        // complete preceding multibyte char.
        // 3-byte char split mid-codepoint: "你" (3 bytes), max=2 → "".
        assert_eq!(trim("你".as_bytes(), 2), "");
        // 4-byte char split mid-codepoint: "a🙂" (61 F0 9F 99 82), max=3 → "a".
        assert_eq!(trim("a🙂".as_bytes(), 3), "a");
        // 2-byte char split mid-codepoint: "é" (C3 A9), max=1 → "".
        assert_eq!(trim("é".as_bytes(), 1), "");
        // Complete preceding multibyte char is never truncated at a mid-cap after it:
        // "éa" with max=2 keeps the full "é" (2 bytes), never just its lead byte.
        let mut buf = "éa".as_bytes().to_vec();
        utf8_prefix_trim(&mut buf, 2);
        assert_eq!(buf, vec![0xC3, 0xA9], "complete 2-byte char must be kept");
    }

    #[tokio::test]
    async fn qualified_pin_survives_semantic_fill_e2e() {
        let tmp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        let dep_dir = root.join("skills").join("deploy");
        std::fs::create_dir_all(&dep_dir).unwrap();
        std::fs::write(
            dep_dir.join("SKILL.md"),
            "---\nname: deploy\ndescription: Deploy the release.\n---\n# Deploy\n",
        )
        .unwrap();
        let deploy = SkillInfo {
            name: "deploy".into(),
            path: dep_dir.join("SKILL.md").to_string_lossy().to_string(),
            scope: SkillScope::Local,
            ..SkillInfo::default()
        };
        let oth_dir = root.join("skills").join("other");
        std::fs::create_dir_all(&oth_dir).unwrap();
        std::fs::write(
            oth_dir.join("SKILL.md"),
            "---\nname: other\ndescription: An unpinned candidate skill.\n---\n# Other\n",
        )
        .unwrap();
        let other = SkillInfo {
            name: "other".into(),
            path: oth_dir.join("SKILL.md").to_string_lossy().to_string(),
            scope: SkillScope::Local,
            ..SkillInfo::default()
        };
        let skills = vec![deploy, other];
        let ex = Arc::new(RecordingExecutor::new());
        ex.set_reverse(true); // rerank would displace unpinned rows
        let service = service_with(ex.clone());
        let mut cfg = SkillPrimeConfig::default();
        cfg.enabled = true;
        cfg.max_results = 5;
        let snapshot = skills.clone();
        let refresh = move || {
            let s = snapshot.clone();
            async move { s }
        };
        let input = PrimeInput {
            eligible_skills: &skills,
            refresh_skills: &refresh,
            workspace_root: &root,
            trusted_roots: &[root.clone()],
            prompt: "deploy the release",
            explicit_skill: Some("local:deploy"),
            config: cfg,
            context_window: None,
            semantic_profile: Some("p1"),
            semantic_service: Some(&service),
            inventory: None,
            grok_home: None,
            snapshot_generation: None,
        };
        let sel = run_prime_selection(&input, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(
            sel.selected.first().map(|s| s.name.as_str()),
            Some("deploy"),
            "qualified pin must survive semantic fill: {:?}",
            sel.budget_state.selected_names
        );
    }

    #[tokio::test]
    async fn run_prime_hard_mode_returns_err_e2e() {
        let tmp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        let sdir = root.join("skills").join("a");
        std::fs::create_dir_all(&sdir).unwrap();
        std::fs::write(
            sdir.join("SKILL.md"),
            "---\nname: a\ndescription: Skill A used in hard-mode prime tests.\n---\n# A\n",
        )
        .unwrap();
        let a = SkillInfo {
            name: "a".into(),
            path: sdir.join("SKILL.md").to_string_lossy().to_string(),
            ..SkillInfo::default()
        };
        let skills = vec![a];
        let ex = Arc::new(RecordingExecutor::new());
        ex.set_fail_embed(true);
        let service = service_with(ex.clone());
        let mut cfg = SkillPrimeConfig::default();
        cfg.enabled = true;
        cfg.degrade_on_error = false;
        let snapshot = skills.clone();
        let refresh = move || {
            let s = snapshot.clone();
            async move { s }
        };
        let input = PrimeInput {
            eligible_skills: &skills,
            refresh_skills: &refresh,
            workspace_root: &root,
            trusted_roots: &[root.clone()],
            prompt: "deploy the release",
            explicit_skill: None,
            config: cfg,
            context_window: None,
            semantic_profile: Some("p1"),
            semantic_service: Some(&service),
            inventory: None,
            grok_home: None,
            snapshot_generation: None,
        };
        let res = run_prime_selection(&input, CancellationToken::new()).await;
        assert!(
            matches!(res, Err(PrimeError::SemanticRetrievalFailed)),
            "hard mode must fail the run: {res:?}"
        );
    }

    #[tokio::test]
    async fn run_prime_deadline_fires_during_semantic_fill_e2e() {
        tokio::time::pause();
        let tmp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        let sdir = root.join("skills").join("a");
        std::fs::create_dir_all(&sdir).unwrap();
        std::fs::write(
            sdir.join("SKILL.md"),
            "---\nname: a\ndescription: Skill A used in deadline prime tests.\n---\n# A\n",
        )
        .unwrap();
        let a = SkillInfo {
            name: "a".into(),
            path: sdir.join("SKILL.md").to_string_lossy().to_string(),
            ..SkillInfo::default()
        };
        let skills = vec![a];
        let ex = Arc::new(RecordingExecutor::new());
        ex.set_slow_ms(400); // semantic fill would block past the deadline
        let service = service_with(ex.clone());
        let mut cfg = SkillPrimeConfig::default();
        cfg.enabled = true;
        cfg.deadline_ms = 80;
        let snapshot = skills.clone();
        let refresh = move || {
            let s = snapshot.clone();
            async move { s }
        };
        let home = root.join("grok-home");
        std::fs::create_dir_all(&home).unwrap();
        let input = PrimeInput {
            eligible_skills: &skills,
            refresh_skills: &refresh,
            workspace_root: &root,
            trusted_roots: &[root.clone()],
            prompt: "deploy the release",
            explicit_skill: None,
            config: cfg,
            context_window: None,
            semantic_profile: Some("p1"),
            semantic_service: Some(&service),
            inventory: None,
            grok_home: Some(&home),
            snapshot_generation: None,
        };
        let fut = run_prime_selection(&input, CancellationToken::new());
        tokio::pin!(fut);
        tokio::time::advance(std::time::Duration::from_millis(81)).await;
        let sel = fut.await.unwrap();
        crate::session::prime::uninstall_prime_index(&home, &root);
        assert!(sel.cancelled, "in-flight deadline must abort the run");
        assert!(sel.selected.is_empty());
        assert!(sel.rendered.is_none());
    }
}
