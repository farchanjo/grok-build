//! Deterministic skill selection pipeline for prime (PR18).
//!
//! Takes the eligible native skill snapshot plus the current prompt and a
//! bounded workspace inventory, pins an explicitly invoked skill first, scores
//! exact `when-to-use` / prompt-path / workspace-path evidence deterministically,
//! optionally refines a bounded non-pinned shortlist through the PR17 semantic
//! retrieval service (metadata only — full bodies never leave the process),
//! then revalidates eligibility/trust/containment immediately before loading
//! each body natively.
//!
//! This is the callable selection seam for PR19. It never splices a
//! conversation and performs no prompt injection; it returns a structured
//! [`crate::session::prime::PrimeSkillSelection`].
//!
//! Safety/latency invariants:
//! - Query, skill-body, vector, and raw provider errors never reach Debug or
//!   telemetry output. Degradation is reported as secret-free kinds.
//! - Semantic fill only embeds metadata (`name`, `description`, `when-to-use`,
//!   `paths`/scope). Bodies are loaded later, natively, after revalidation.
//! - Rerank failures preserve the exact pre-stage deterministic order.

use std::collections::HashSet;
use std::path::Path;

use tokio_util::sync::CancellationToken;
use xai_grok_config_types::SkillPrimeConfig;
use xai_grok_tools::implementations::skills::skill::load_skill_content;
use xai_grok_tools::implementations::skills::types::{SkillInfo, SkillScope};

use crate::retrieval::{
    DegradationKind, DegradationNotice, PipelineOptions, RetrievalService, RetrieveCandidates,
};

use super::inventory::{InventoryEntry, WorkspaceInventory};
use super::render::LoadedSkill;

/// How a skill was dropped during revalidation/loading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimeDropReason {
    /// No longer present / no longer eligible in the fresh snapshot.
    ChangedOrGone,
    /// Canonical path escapes the trusted containment roots.
    NotContained,
    /// Native body load failed (unreadable / vanished).
    Unreadable,
}

/// Secret-free selection budget state for PR19 reporting.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PrimeBudgetState {
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

/// Exact when-to-use evidence: trigger phrases present verbatim plus bounded
/// token overlap. Higher is better.
fn wtu_score(s: &SkillInfo, prompt: &str) -> u64 {
    let mut score = 0u64;
    let Some(wtu) = &s.when_to_use else {
        return 0;
    };
    let prompt_lower = prompt.to_lowercase();
    // Exact trigger phrases split on ':'/','/';' — verbatim match is strong.
    for phrase in wtu.split(|c| c == ':' || c == ',' || c == ';' || c == '\n') {
        let phrase = phrase.trim();
        if phrase.len() >= 6 && prompt_lower.contains(&phrase.to_lowercase()) {
            score = score.saturating_add(10);
        }
    }
    // Token overlap (weak sub-evidence).
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

/// Prompt-referenced paths matching a skill's `paths:` globs. High priority.
fn prompt_path_score(s: &SkillInfo, prompt_paths: &[String], workspace_root: &Path) -> u64 {
    let Some(patterns) = &s.paths else {
        return 0;
    };
    if patterns.is_empty() || prompt_paths.is_empty() {
        return 0;
    }
    use ignore::gitignore::GitignoreBuilder;
    let mut builder = GitignoreBuilder::new(workspace_root);
    for p in patterns {
        let _ = builder.add_line(None, p);
    }
    let Ok(matcher) = builder.build() else {
        return 0;
    };
    let mut hit = false;
    for pat in prompt_paths {
        let p = Path::new(pat);
        if matcher.matched_path_or_any_parents(p, false).is_ignore() {
            hit = true;
            break;
        }
    }
    if hit {
        return 40;
    }
    0
}

/// Weaker workspace-inventory path evidence: the skill's name appearing as a
/// path segment in the inventory snapshot.
fn inventory_score(s: &SkillInfo, name_lower: &str, inventory: &WorkspaceInventory) -> i64 {
    let mut score = 0i64;
    for entry in &inventory.entries {
        let seg = entry.rel.rsplit('/').next().unwrap_or("").to_lowercase();
        if seg == name_lower || entry.rel.to_lowercase().contains(name_lower) {
            score = score.saturating_add(2);
        }
    }
    score
}

/// Deterministically rank `skills` for `prompt` + `inventory`.
///
/// Returns indices in ranked order (best first). Order is fully deterministic:
/// primary score desc, secondary desc, then name/path asc. The explicitly
/// pinned skill (if present) is forced first and never duplicated.
pub fn rank_skills(
    skills: &[SkillInfo],
    prompt: &str,
    inventory: &WorkspaceInventory,
    explicit: Option<&str>,
) -> Vec<usize> {
    let prompt_paths = extract_prompt_paths(prompt);
    let ranked: Vec<Ranked> = skills
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let name_lower = s.name.to_lowercase();
            let primary = wtu_score(s, prompt) as i64
                + prompt_path_score(s, &prompt_paths, &inventory.root) as i64
                + inventory_score(s, &name_lower, inventory);
            Ranked {
                idx: i,
                primary,
                secondary: 0i64,
            }
        })
        .collect();

    // Deterministic order.
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

    // Pinned first, no duplicate.
    if let Some(name) = explicit {
        if let Some(pos) = order.iter().position(|&i| skills[i].name == name) {
            let idx = order.remove(pos);
            order.insert(0, idx);
        }
    }
    order
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
        // Looks like a path: contains '/' or starts with '.' or a file-like token.
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

/// Bounded metadata-only text for a skill (never the body).
fn metadata_text(s: &SkillInfo) -> String {
    let mut parts = vec![s.name.clone(), s.description.clone()];
    if let Some(wtu) = &s.when_to_use {
        parts.push(wtu.clone());
    }
    if let Some(paths) = &s.paths {
        parts.push(paths.join(", "));
    }
    parts.push(format!("scope:{:?}", s.scope));
    parts.join(" ")
}

/// Whether a skill snapshot entry is eligible for native invocation.
pub fn is_eligible(s: &SkillInfo) -> bool {
    s.enabled && !s.disable_model_invocation
}

/// Outcome of the optional semantic refinement stage.
pub struct SemanticFillOutcome {
    /// Full reordered indices (pinned remain first). On any failure this is the
    /// exact pre-stage deterministic order.
    pub order: Vec<usize>,
    /// Secret-free degradation notices from the retrieval service.
    pub degradations: Vec<crate::retrieval::DegradationNotice>,
    pub cancelled: bool,
    /// Number of non-pinned candidates shipped to the semantic service.
    pub shortlist_size: usize,
}

fn degrade_from_error(
    e: crate::retrieval::OrchestratorError,
    profile: &str,
) -> crate::retrieval::DegradationNotice {
    use crate::retrieval::{DegradationKind, OrchestratorError, RetrievalStage};
    let kind = match &e {
        OrchestratorError::ServiceDisabled => DegradationKind::ServiceDisabled,
        OrchestratorError::ProfileMissing { .. } => DegradationKind::ProfileMissing,
        OrchestratorError::DeadlineExceeded { .. }
        | OrchestratorError::AttemptBudgetExceeded { .. }
        | OrchestratorError::InputBudgetExceeded { .. }
        | OrchestratorError::OutputBudgetExceeded { .. } => DegradationKind::BudgetExhausted,
        _ => DegradationKind::SemanticUnavailable,
    };
    crate::retrieval::DegradationNotice::new(kind, profile, RetrievalStage::Orchestrate, None)
}

/// Optional semantic fill: rerank a bounded non-pinned shortlist under the
/// named profile's strict budgets. Only bounded metadata reaches the provider;
/// full skill bodies never leave local processing. On failure the exact
/// pre-stage deterministic order is preserved.
pub async fn semantic_fill(
    service: &RetrievalService,
    profile_id: &str,
    query: &str,
    skills: &[SkillInfo],
    ranked: &[usize],
    pinned: &HashSet<&str>,
    cancel: CancellationToken,
) -> SemanticFillOutcome {
    let mut outcome = SemanticFillOutcome {
        order: ranked.to_vec(),
        degradations: Vec::new(),
        cancelled: false,
        shortlist_size: 0,
    };

    if query.trim().is_empty() {
        return outcome;
    }

    let non_pinned: Vec<usize> = ranked
        .iter()
        .copied()
        .filter(|&i| !pinned.contains(skills[i].name.as_str()))
        .collect();
    if non_pinned.is_empty() {
        return outcome;
    }

    let rows: Vec<crate::retrieval::CandidateRow> = non_pinned
        .iter()
        .map(|&i| crate::retrieval::CandidateRow {
            id: skills[i].name.clone(),
            text: metadata_text(&skills[i]),
            score: None,
            metadata: None,
        })
        .collect();

    let opts = PipelineOptions {
        bypass_semantic: false,
        hard_error_on_semantic_failure: false,
        hard_error_on_limit_exceeded: false,
        ..Default::default()
    };

    outcome.shortlist_size = rows.len();
    let result = service
        .retrieve(
            profile_id,
            query,
            RetrieveCandidates::Explicit(rows),
            opts,
            cancel,
        )
        .await;

    match result {
        Ok(res) => {
            outcome
                .degradations
                .extend(res.degradations.iter().cloned());
            // Reconstruct order: pinned (pre-order) first, then returned order,
            // then any non-pinned not returned (retrieve may truncate to results).
            let pinned_order: Vec<usize> = ranked
                .iter()
                .copied()
                .filter(|&i| pinned.contains(skills[i].name.as_str()))
                .collect();
            let id_to_idx: std::collections::HashMap<&str, usize> = non_pinned
                .iter()
                .map(|&i| (skills[i].name.as_str(), i))
                .collect();
            let mut rest = Vec::new();
            let mut used = HashSet::new();
            for cand in &res.candidates {
                if let Some(&i) = id_to_idx.get(cand.id.as_str()) {
                    if used.insert(i) {
                        rest.push(i);
                    }
                }
            }
            for &i in &non_pinned {
                if used.insert(i) {
                    rest.push(i);
                }
            }
            let mut order = pinned_order;
            order.extend(rest);
            outcome.order = order;
        }
        Err(e) => {
            if matches!(e, crate::retrieval::OrchestratorError::Cancelled { .. }) {
                outcome.cancelled = true;
            }
            outcome.degradations.push(degrade_from_error(e, profile_id));
            // Preserve exact pre-stage order.
        }
    }
    outcome
}

/// Bound the ranked order to the configured `max_results`.
pub fn select_limit(ranked: &[usize], config: &SkillPrimeConfig) -> Vec<usize> {
    let max = config.max_results.max(1) as usize;
    ranked.iter().take(max).copied().collect()
}

/// Revalidated, natively-loaded selected skills (never trusting snapshot body).
pub struct LoadedBatch {
    pub loaded: Vec<LoadedSkill>,
    pub drop_reasons: Vec<PrimeDropReason>,
}

/// Revalidate eligibility/trust/containment against a fresh authoritative
/// snapshot and load each surviving body through native `load_skill_content`.
///
/// Drops skills that changed, disappeared, became ineligible, are no longer
/// readable, or whose canonical path escapes the trusted containment roots.
/// `trusted_roots` must already be canonicalized.
pub async fn load_and_revalidate(
    skills: &[SkillInfo],
    selected: &[usize],
    refresh: &dyn Fn() -> Vec<SkillInfo>,
    trusted_roots: &[std::path::PathBuf],
) -> LoadedBatch {
    let fresh = refresh();
    let fresh_eligible: Vec<&SkillInfo> = fresh.iter().filter(|s| is_eligible(s)).collect();
    let mut loaded = Vec::new();
    let mut drop_reasons = Vec::new();

    for &i in selected {
        let cand = &skills[i];
        let cur = fresh_eligible
            .iter()
            .find(|s| s.name == cand.name && s.path == cand.path && s.scope == cand.scope);
        let Some(cur) = cur else {
            drop_reasons.push(PrimeDropReason::ChangedOrGone);
            continue;
        };

        // Canonical containment check (fails closed on unverifiable paths).
        let canon = dunce::canonicalize(&cur.path).ok();
        let contained = canon
            .as_ref()
            .map(|c| trusted_roots.iter().any(|r| c.starts_with(r)))
            .unwrap_or(false);
        if !contained {
            drop_reasons.push(PrimeDropReason::NotContained);
            continue;
        }

        // Load body natively — never trust stale snapshot `body` fields.
        match load_skill_content(cur).await {
            Ok(body) => loaded.push(LoadedSkill {
                name: cur.name.clone(),
                scope: cur.scope,
                source_path: cur.path.clone(),
                body,
            }),
            Err(_) => drop_reasons.push(PrimeDropReason::Unreadable),
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

    fn skill_with_body(name: &str, path: &str, body: &str) -> SkillInfo {
        SkillInfo {
            body: Some(body.into()),
            ..skill(name, path)
        }
    }

    // ── Ranking: pinned / scoring / deterministic ties ────────────────

    #[test]
    fn pinned_skill_first_and_not_duplicated() {
        let skills = vec![
            skill_wtu(
                "deploy",
                "/s/deploy/SKILL.md",
                "deploy the release to production",
            ),
            skill_wtu("review", "/s/review/SKILL.md", "review a pull request"),
        ];
        let inv = WorkspaceInventory::default();
        let rank = rank_skills(&skills, "please deploy", &inv, Some("review"));
        assert_eq!(rank.len(), 2, "no duplicates");
        assert_eq!(skills[rank[0]].name, "review", "pinned must be first");
        assert_eq!(skills[rank[1]].name, "deploy");
    }

    #[test]
    fn exact_when_to_use_outranks_weak_and_tie_is_deterministic() {
        let inv = WorkspaceInventory::default();
        let deploy = skill_wtu(
            "deploy",
            "/s/deploy/SKILL.md",
            "deploy the release to production",
        );
        let fuzzy = skill_wtu("zz", "/s/zz/SKILL.md", "zebra crossing patterns");
        let none = skill_wtu("aaa", "/s/aaa/SKILL.md", "unrelated things");
        let skills = vec![deploy, fuzzy, none];
        let rank = rank_skills(
            &skills,
            "please deploy the release to production now",
            &inv,
            None,
        );
        assert_eq!(skills[rank[0]].name, "deploy", "exact when-to-use wins");
        // Deterministic tie: two zero-evidence skills order by name asc (aaa < zz).
        let rank2 = rank_skills(&skills, "nothing relevant at all", &inv, None);
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
            epoch: 0,
        };
        let deploy = skill_paths("deploy", "/s/deploy/SKILL.md", &["src/*.rs"]);
        let other = skill("other", "/s/other/SKILL.md");
        let skills = vec![other, deploy];
        // Prompt references a prompt path matching deploy's `paths:`.
        let rank = rank_skills(&skills, "please change src/deploy.rs", &inv, None);
        assert_eq!(skills[rank[0]].name, "deploy", "prompt-path evidence wins");
    }

    #[test]
    fn user_invocable_false_remains_selectable() {
        let mut s = skill("auto", "/s/auto/SKILL.md");
        s.user_invocable = false;
        assert!(is_eligible(&s));
    }

    // ── Semantic fill: metadata-only boundary, ordering, failure ────────

    /// Records which documents/queries were shipped to the (fake) provider.
    struct RecordingExecutor {
        rerank_docs: Mutex<Vec<Vec<String>>>,
        embed_inputs: Mutex<Vec<Vec<String>>>,
        reverse: Mutex<bool>,
        fail_rerank: Mutex<bool>,
    }

    impl RecordingExecutor {
        fn new() -> Self {
            Self {
                rerank_docs: Mutex::new(Vec::new()),
                embed_inputs: Mutex::new(Vec::new()),
                reverse: Mutex::new(false),
                fail_rerank: Mutex::new(false),
            }
        }
        fn set_reverse(&self, v: bool) {
            *self.reverse.lock().unwrap() = v;
        }
        fn set_fail_rerank(&self, v: bool) {
            *self.fail_rerank.lock().unwrap() = v;
        }
        fn rerank_docs(&self) -> Vec<Vec<String>> {
            self.rerank_docs.lock().unwrap().clone()
        }
        fn embed_inputs(&self) -> Vec<Vec<String>> {
            self.embed_inputs.lock().unwrap().clone()
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
            if cancel.is_cancelled() {
                return Err(RetrievalError::Cancelled);
            }
            if let Some(d) = pins.total_deadline {
                if d.is_zero() {
                    return Err(RetrievalError::DeadlineExceeded);
                }
            }
            Ok(EmbeddingResult {
                model: config.model.clone(),
                vectors: inputs
                    .iter()
                    .enumerate()
                    .map(|(index, _)| EmbeddingVector {
                        index,
                        values: vec![0.1; config.dimensions.unwrap_or(4) as usize],
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
                .map(|(index, _)| RerankHit {
                    index,
                    score: 1.0 - (index as f32) * 0.01,
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

    #[tokio::test]
    async fn semantic_fill_shares_only_metadata_and_orders() {
        let ex = Arc::new(RecordingExecutor::new());
        ex.set_reverse(true);
        let service = service_with(ex.clone());
        let skills = vec![
            skill_with_body("alpha", "/s/alpha/SKILL.md", "BODY-SECRET-ALPHA"),
            skill_with_body("beta", "/s/beta/SKILL.md", "BODY-SECRET-BETA"),
            skill_with_body("gamma", "/s/gamma/SKILL.md", "BODY-SECRET-GAMMA"),
        ];
        let ranked = vec![0, 1, 2];
        let pinned: HashSet<&str> = HashSet::new();
        let out = semantic_fill(
            &service,
            "p1",
            "deploy the release please",
            &skills,
            &ranked,
            &pinned,
            CancellationToken::new(),
        )
        .await;
        // reversed rerank → [2,1,0].
        assert_eq!(out.order, vec![2, 1, 0]);
        assert!(out.degradations.is_empty());

        // No body reaches the executor — only bounded metadata.
        for docs in ex.rerank_docs() {
            for d in docs {
                assert!(!d.contains("BODY-SECRET"), "body leaked to reranker: {d}");
            }
        }
        for inputs in ex.embed_inputs() {
            for t in inputs {
                assert!(!t.contains("BODY-SECRET"), "body leaked to embedder: {t}");
            }
        }
    }

    #[tokio::test]
    async fn semantic_fill_failure_preserves_pre_stage_order() {
        let ex = Arc::new(RecordingExecutor::new());
        ex.set_fail_rerank(true);
        let service = service_with(ex.clone());
        let skills = vec![
            skill("alpha", "/s/alpha/SKILL.md"),
            skill("beta", "/s/beta/SKILL.md"),
            skill("gamma", "/s/gamma/SKILL.md"),
        ];
        let ranked = vec![0, 1, 2];
        let pinned: HashSet<&str> = HashSet::new();
        let out = semantic_fill(
            &service,
            "p1",
            "deploy it",
            &skills,
            &ranked,
            &pinned,
            CancellationToken::new(),
        )
        .await;
        assert_eq!(
            out.order, ranked,
            "failures must preserve exact pre-stage order"
        );
        assert!(!out.degradations.is_empty());
    }

    // ── Revalidation / body loading ────────────────────────────────────

    #[tokio::test]
    async fn native_body_load_frontmatter_and_internal_links() {
        let tmp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        let good_dir = root.join("skills").join("good");
        std::fs::create_dir_all(&good_dir).unwrap();
        std::fs::write(
            good_dir.join("SKILL.md"),
            "---\nname: good\n---\nLoad this file.\nSee [doc](notes.md).\n",
        )
        .unwrap();
        // Internal link target lives inside the skill directory (native link
        // resolution only rewrites links contained by the skill dir).
        std::fs::write(good_dir.join("notes.md"), "Shared reference content.\n").unwrap();

        // Unreadable: a directory at the SKILL.md path (load fails as file).
        let bad_dir = root.join("skills").join("bad");
        std::fs::create_dir_all(bad_dir.join("SKILL.md")).unwrap();

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
        ];
        let refresh = || skills.clone();
        let batch = load_and_revalidate(&skills, &[0, 1], &refresh, &[root.clone()]).await;
        assert_eq!(batch.drop_reasons, vec![PrimeDropReason::Unreadable]);
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
    async fn symlink_source_escape_rejected() {
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
        let refresh = || skills.clone();
        let batch = load_and_revalidate(&skills, &[0], &refresh, &[root.clone()]).await;
        assert_eq!(batch.drop_reasons, vec![PrimeDropReason::NotContained]);
        assert!(batch.loaded.is_empty());
    }

    #[tokio::test]
    async fn revalidation_drops_skill_that_changed_or_became_ineligible() {
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
        let refreshed = vec![SkillInfo {
            name: "a".into(),
            path,
            enabled: false,
            ..SkillInfo::default()
        }];
        // The bar is: it becomes ineligible → changed/gone.
        let batch = load_and_revalidate(&initial, &[0], &|| refreshed.clone(), &[root]).await;
        assert_eq!(batch.drop_reasons, vec![PrimeDropReason::ChangedOrGone]);
        assert!(batch.loaded.is_empty());
    }
}
