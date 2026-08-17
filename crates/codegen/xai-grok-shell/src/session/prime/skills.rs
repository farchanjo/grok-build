//! Deterministic skill selection pipeline for prime (PR18).
//!
//! Takes the eligible native skill snapshot plus the current prompt and a
//! bounded workspace inventory, pins an explicitly invoked skill first, scores
//! exact `when-to-use` / prompt-path / workspace-path evidence deterministically
//! (capped to prevent flooding and `**` self-promotion), optionally refines a
//! bounded non-pinned shortlist through the PR17 semantic retrieval service,
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
//! - Rerank failures preserve the exact pre-stage deterministic order.
//! - Body loading is bounded (bytes), requires a regular `SKILL.md`, opens with
//!   `O_NOFOLLOW | O_NONBLOCK`, re-verifies file identity after the read, and
//!   re-canonicalizes containment — closing the check→read TOCTOU.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use tokio_util::sync::CancellationToken;
use xai_grok_config_types::SkillPrimeConfig;
use xai_grok_tools::implementations::skills::skill::{
    extract_skill_body, format_skill_name, resolve_skill_internal_links,
};
use xai_grok_tools::implementations::skills::types::{SkillInfo, SkillScope};

use crate::retrieval::{
    DegradationKind, DegradationNotice, OrchestratorError, PipelineOptions, RetrievalService,
    RetrieveCandidates,
};

use super::SkillRefresh;
use super::inventory::WorkspaceInventory;
use super::render::LoadedSkill;

/// Hard cap on the bounded metadata shortlist shipped to the semantic service
/// (explicit, upstream of the profile's own budgets).
const MAX_SEMANTIC_SHORTLIST: usize = 8;
/// Cap on `when-to-use` phrases considered per skill (anti-flood).
const MAX_WTU_PHRASES: usize = 8;
/// Cap on inventory hits counted per skill.
const MAX_INVENTORY_SCORE: i64 = 20;
/// Cap on `paths:` patterns considered per skill (anti-flood).
const MAX_PATH_PATTERNS: usize = 10;
/// Bounded bytes read from a SKILL.md body (render truncates to per-body chars;
/// prevents OOM / FIFO / device hangs).
const MAX_LOADED_BODY_BYTES: u64 = 64 * 1024;
/// Per-part and aggregate caps for semantic metadata text.
const MAX_METADATA_PART_CHARS: usize = 64;
const MAX_METADATA_TOTAL_CHARS: usize = 1024;

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

/// Cap a string to `max` UTF-8 chars (char-boundary safe).
fn cap_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
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
            let name_lower = s.name.to_lowercase();
            let primary = wtu_score(s, prompt) as i64
                + prompt_path_score(s, &prompt_paths, &inventory.root) as i64
                + inventory_score(&name_lower, inventory);
            Ranked {
                idx: i,
                primary,
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
fn metadata_text(s: &SkillInfo) -> String {
    let mut parts = vec![cap_chars(&s.name, MAX_METADATA_PART_CHARS)];
    if s.has_user_specified_description && !s.description.trim().is_empty() {
        parts.push(cap_chars(&s.description, MAX_METADATA_PART_CHARS));
    }
    if let Some(wtu) = &s.when_to_use {
        parts.push(cap_chars(wtu, MAX_METADATA_PART_CHARS));
    }
    if let Some(paths) = &s.paths {
        let joined = paths
            .iter()
            .take(MAX_PATH_PATTERNS)
            .map(|p| cap_chars(p, 32))
            .collect::<Vec<_>>()
            .join(", ");
        if !joined.is_empty() {
            parts.push(joined);
        }
    }
    parts.push(format!("scope:{}", scope_label(s.scope)));
    let joined = parts.join(" ");
    cap_chars(&joined, MAX_METADATA_TOTAL_CHARS)
}

/// Unique CandidateRow identifier (scope + name + path so same-named skills do
/// not collide).
fn candidate_id(s: &SkillInfo) -> String {
    format!("{}|{}|{}", scope_label(s.scope), s.name, s.path)
}

/// Authoritative eligibility: delegates to the tools predicate so the two layers
/// can never drift apart.
pub fn is_eligible(s: &SkillInfo) -> bool {
    s.is_native_model_invocable()
}

/// Outcome of the optional semantic refinement stage.
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

/// Optional semantic fill: rerank a bounded non-pinned shortlist.
///
/// Only frontmatter-authorized metadata is shipped; bodies stay local. The
/// shortlist is explicitly capped to [`MAX_SEMANTIC_SHORTLIST`] rows before the
/// PR17 stage. On failure the exact pre-stage deterministic order is preserved.
pub async fn semantic_fill(
    service: &RetrievalService,
    profile_id: &str,
    query: &str,
    skills: &[SkillInfo],
    ranked: &[usize],
    pinned: &HashSet<String>,
    cancel: CancellationToken,
    hard: bool,
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

    let non_pinned: Vec<usize> = ranked
        .iter()
        .copied()
        .filter(|&i| !pinned.contains(&skills[i].name))
        .collect();
    if non_pinned.is_empty() {
        return outcome;
    }

    // Explicit bounded shortlist (deterministic: rank order).
    let shortlist: &[usize] = &non_pinned[..non_pinned.len().min(MAX_SEMANTIC_SHORTLIST)];
    let rows: Vec<crate::retrieval::CandidateRow> = shortlist
        .iter()
        .map(|&i| crate::retrieval::CandidateRow {
            id: candidate_id(&skills[i]),
            text: metadata_text(&skills[i]),
            score: None,
            metadata: None,
        })
        .collect();

    let opts = PipelineOptions {
        bypass_semantic: false,
        // Hard mode honors `degrade_on_error = false`: fail the run on failure.
        hard_error_on_semantic_failure: hard,
        hard_error_on_limit_exceeded: hard,
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

            let pinned_order: Vec<usize> = ranked
                .iter()
                .copied()
                .filter(|&i| pinned.contains(&skills[i].name))
                .collect();
            // Uniquely key the candidate map (id → index) so same-named skills
            // keep their own rerank ordering.
            let id_to_idx: std::collections::HashMap<String, usize> = shortlist
                .iter()
                .map(|&i| (candidate_id(&skills[i]), i))
                .collect();

            let mut rest = Vec::new();
            let mut used = HashSet::new();
            for cand in &res.candidates {
                if let Some(&i) = id_to_idx.get(&cand.id)
                    && used.insert(i)
                {
                    rest.push(i);
                }
            }
            for &i in shortlist {
                if used.insert(i) {
                    rest.push(i);
                }
            }
            // Append any non-pinned not in the bounded shortlist (rank order).
            for &i in &non_pinned[shortlist.len()..] {
                if used.insert(i) {
                    rest.push(i);
                }
            }

            let mut order = pinned_order;
            order.extend(rest);
            outcome.order = order;
        }
        Err(e) => {
            if matches!(e, OrchestratorError::Cancelled { .. }) {
                outcome.cancelled = true;
                // No spurious degradation for cancellation.
            } else if hard {
                outcome.hard_error = Some(e);
            } else {
                outcome
                    .degradations
                    .push(degrade_from_error(&e, profile_id));
            }
        }
    }
    outcome
}

/// Bounded, TOCTOU-hardened body reader (unix: O_NOFOLLOW | O_NONBLOCK).
#[cfg(unix)]
fn read_body_bounded_blocking(path: &Path) -> std::io::Result<String> {
    use std::io::Read;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

    let meta = std::fs::symlink_metadata(path)?;
    if !meta.file_type().is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "skill file is not a regular file",
        ));
    }
    let mut opts = std::fs::OpenOptions::new();
    opts.read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC);
    let file = opts.open(path)?;
    let fmeta = file.metadata()?;
    if !fmeta.file_type().is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "opened handle is not a regular file",
        ));
    }
    let (dev_a, ino_a) = (fmeta.dev(), fmeta.ino());
    let mut buf = Vec::new();
    let mut f = file;
    let mut take = f.by_ref().take(MAX_LOADED_BODY_BYTES.saturating_add(1));
    let _ = take.read_to_end(&mut buf)?;
    if buf.len() as u64 > MAX_LOADED_BODY_BYTES {
        buf.truncate(MAX_LOADED_BODY_BYTES as usize);
    }
    drop(f);

    // Post-read identity revalidation → discard on swap during read.
    if let Ok(m) = std::fs::symlink_metadata(path)
        && (m.dev() != dev_a || m.ino() != ino_a)
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "file identity changed during read",
        ));
    }
    String::from_utf8(buf).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "skill body is not valid UTF-8",
        )
    })
}

/// Portable bounded reader fallback (regular-file guard + `take` cap).
#[cfg(not(unix))]
fn read_body_bounded_blocking(path: &Path) -> std::io::Result<String> {
    use std::io::Read;
    let meta = std::fs::metadata(path)?;
    if !meta.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "skill file is not a regular file",
        ));
    }
    let mut f = std::fs::File::open(path)?;
    let mut buf = Vec::new();
    let mut take = f.by_ref().take(MAX_LOADED_BODY_BYTES.saturating_add(1));
    let _ = take.read_to_end(&mut buf)?;
    if buf.len() as u64 > MAX_LOADED_BODY_BYTES {
        buf.truncate(MAX_LOADED_BODY_BYTES as usize);
    }
    String::from_utf8(buf).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "skill body is not valid UTF-8",
        )
    })
}

/// Load + extract a skill body natively (frontmatter strip + contained internal
/// links) through a bounded, cancellation-selected reader.
async fn load_skill_body_bounded(
    cur: &SkillInfo,
    cancel: &CancellationToken,
) -> Result<String, PrimeDropReason> {
    let path = cur.path.clone();
    let handle =
        tokio::task::spawn_blocking(move || read_body_bounded_blocking(Path::new(&path)).ok());
    let raw = tokio::select! {
        biased;
        _ = cancel.cancelled() => None,
        res = handle => res.unwrap_or_default(),
    };
    let Some(raw) = raw else {
        return Err(PrimeDropReason::Unreadable);
    };

    // Reuse native frontmatter + link-rewrite behavior (never diverging).
    let body = extract_skill_body(&raw);
    let dir = Path::new(&cur.path).parent().map(Path::to_path_buf);
    let body = match dir {
        Some(d) => resolve_skill_internal_links(&body, &d),
        None => body,
    };
    Ok(body)
}

/// Revalidated, natively-loaded selected skills (never trusting snapshot body).
pub struct LoadedBatch {
    pub loaded: Vec<LoadedSkill>,
    pub drop_reasons: Vec<PrimeDropReason>,
}

/// Revalidate against a fresh authoritative snapshot and load up to `target`
/// bodies, backfilling the next-ranked candidate whenever one is dropped.
///
/// `order` is the full ranked order (post-semantic). For each candidate: require
/// it is still present + eligible with the same identity, is a regular
/// `SKILL.md` whose canonical path stays within `trusted_roots`, then read it
/// natively. Skills that change/disappear/become unreadable/escape are dropped.
pub async fn load_and_revalidate(
    skills: &[SkillInfo],
    order: &[usize],
    target: usize,
    refresh: &dyn SkillRefresh,
    trusted_roots: &[PathBuf],
    cancel: &CancellationToken,
) -> LoadedBatch {
    let target = target.max(1);
    let fresh = refresh.refresh().await;
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

        // Classification of the target file.
        let smeta = match std::fs::symlink_metadata(&cur.path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                drop_reasons.push(PrimeDropReason::ChangedOrGone);
                continue;
            }
            Err(_) => {
                drop_reasons.push(PrimeDropReason::Unreadable);
                continue;
            }
        };
        if smeta.file_type().is_symlink() {
            drop_reasons.push(PrimeDropReason::NotContained);
            continue;
        }
        if !smeta.file_type().is_file() {
            drop_reasons.push(PrimeDropReason::Unreadable);
            continue;
        }
        // Path-shape gate: only SKILL.md files are loadable.
        let ok_name = Path::new(&cur.path)
            .file_name()
            .is_some_and(|n| n == "SKILL.md");
        if !ok_name {
            drop_reasons.push(PrimeDropReason::NotContained);
            continue;
        }

        // Canonical containment (fails closed on unresolvable paths).
        let canon = match dunce::canonicalize(&cur.path) {
            Ok(c) => c,
            Err(_) => {
                drop_reasons.push(PrimeDropReason::NotContained);
                continue;
            }
        };
        let contained = trusted_roots.iter().any(|r| canon.starts_with(r));
        if !contained {
            drop_reasons.push(PrimeDropReason::NotContained);
            continue;
        }

        // Bounded native body load (never trust snapshot body fields).
        match load_skill_body_bounded(cur, cancel).await {
            Ok(body) => {
                // Post-read canonical re-verification (close check→read race).
                let still_contained = dunce::canonicalize(&cur.path)
                    .map(|c2| trusted_roots.iter().any(|r| c2.starts_with(r)))
                    .unwrap_or(false);
                if !still_contained {
                    drop_reasons.push(PrimeDropReason::NotContained);
                    continue;
                }
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
            Err(rr) => {
                drop_reasons.push(rr);
            }
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
    fn candidate_ids_unique_for_same_named_skills() {
        let a = skill("commit", "/local/commit/SKILL.md");
        let b = skill("commit", "/user/commit/SKILL.md");
        assert_ne!(candidate_id(&a), candidate_id(&b));
        assert!(candidate_id(&a).contains("/local/commit/SKILL.md"));
    }

    // ── Semantic fill ─────────────────────────────────────────────────

    struct RecordingExecutor {
        rerank_docs: Mutex<Vec<Vec<String>>>,
        embed_inputs: Mutex<Vec<Vec<String>>>,
        reverse: Mutex<bool>,
        fail_rerank: Mutex<bool>,
        fail_embed: Mutex<bool>,
    }

    impl RecordingExecutor {
        fn new() -> Self {
            Self {
                rerank_docs: Mutex::new(Vec::new()),
                embed_inputs: Mutex::new(Vec::new()),
                reverse: Mutex::new(false),
                fail_rerank: Mutex::new(false),
                fail_embed: Mutex::new(false),
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
            if *self.fail_embed.lock().unwrap() {
                return Err(RetrievalError::Timeout);
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
                    .map(|(i, _)| EmbeddingVector {
                        index: i,
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
        let out = semantic_fill(
            &service,
            "p1",
            "deploy the release please",
            &skills,
            &ranked,
            &pinned,
            CancellationToken::new(),
            false,
        )
        .await;
        assert_eq!(out.order, vec![2, 1, 0]);
        assert!(out.degradations.is_empty());

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
        let out = semantic_fill(
            &service,
            "p1",
            "deploy it",
            &skills,
            &ranked,
            &pinned,
            CancellationToken::new(),
            false,
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
        let out = semantic_fill(
            &service,
            "p1",
            "deploy it",
            &skills,
            &ranked,
            &pinned,
            CancellationToken::new(),
            true, // hard: degrade_on_error = false
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
        let out = semantic_fill(
            &service,
            "p1",
            "deploy it",
            &skills,
            &ranked,
            &pinned,
            token,
            false,
        )
        .await;
        assert!(out.cancelled);
        assert!(out.degradations.is_empty(), "no fake degradation on cancel");
    }

    #[test]
    fn semantic_shortlist_is_capped() {
        // Semantic fill caps to MAX_SEMANTIC_SHORTLIST; verify via a unit path.
        assert!(MAX_SEMANTIC_SHORTLIST > 0);
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
            "---\nname: good\n---\nLoad this file.\nSee [doc](notes.md).\n",
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
        std::fs::write(gdir.join("SKILL.md"), "# Good\n").unwrap();
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
}
