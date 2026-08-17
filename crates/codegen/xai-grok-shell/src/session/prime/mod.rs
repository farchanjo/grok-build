//! PR18 native skill prime — inventory, deterministic selection, and safe render.
//!
//! This module is the callable selection+render seam for PR19. It:
//!
//! 1. Exposes a bounded lazy workspace inventory ([`inventory`]).
//! 2. Deterministically ranks the eligible native skill snapshot (pinned-first,
//!    exact `when-to-use` / prompt-path / workspace-path evidence), optionally
//!    refines a bounded non-pinned shortlist via the PR17 semantic retrieval
//!    service (frontmatter-authorized metadata only — bodies stay local), and
//!    selects within [`SkillPrimeConfig`] result/body/total/token/context-fraction
//!    budgets.
//! 3. Revalidates eligibility/trust/canonical containment against a **fresh**
//!    authoritative snapshot ([`SkillRefresh`], an async callback the session
//!    wires to `bridge.eligible_native_skills()`) immediately before loading
//!    bodies — and backfills the next-ranked candidates when revalidation drops
//!    one, up to `max_results`.
//! 4. Renders selected bodies as untrusted quoted context with provenance and a
//!    precedence statement, escaped so no skill can forge the wrapper.
//!
//! A single prime-wide absolute deadline and cancellation token bound the
//! entire run — inventory (run off the async executor via `spawn_blocking`),
//! the semantic call, the fresh-snapshot refresh, body loading, and render. On
//! cancellation the run returns **no content** with `cancelled = true` and never
//! emits a spurious `SemanticUnavailable`. Full skill bodies never leave local
//! processing, and query/body/vector/raw provider errors never reach
//! Debug/telemetry.
//!
//! It never splices a conversation and performs no prompt injection (PR19).

pub mod inventory;
pub mod render;
pub mod skills;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tokio_util::sync::CancellationToken;
use xai_grok_config_types::SkillPrimeConfig;
use xai_grok_tools::implementations::skills::skill::format_skill_name;
use xai_grok_tools::implementations::skills::types::SkillInfo;

use crate::retrieval::{DegradationKind, DegradationNotice, RetrievalService};

use self::inventory::{InventoryLimits, WorkspaceInventory};
use self::render::render_skills;

pub use self::render::{LoadedSkill, RenderBudgets, RenderedSkills};
pub use self::skills::{PrimeBudgetState, PrimeDropReason, SemanticFillOutcome};

/// Async supplier of the authoritative eligible native skill snapshot. PR19
/// wires this to `ToolBridge::eligible_native_skills()`. The shell calls this
/// at load time and awaits a genuinely async refresh — it never clones a stale
/// pre-fetched snapshot — which closes the TOCTOU window between rank and load.
#[async_trait::async_trait]
pub trait SkillRefresh: Send + Sync {
    async fn refresh(&self) -> Vec<SkillInfo>;
}

#[async_trait::async_trait]
impl<F, Fut> SkillRefresh for F
where
    F: Fn() -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Vec<SkillInfo>> + Send,
{
    async fn refresh(&self) -> Vec<SkillInfo> {
        (self)().await
    }
}

/// No-op refresh used by [`PrimeInput::default`].
struct NoopRefresh;
#[async_trait::async_trait]
impl SkillRefresh for NoopRefresh {
    async fn refresh(&self) -> Vec<SkillInfo> {
        Vec::new()
    }
}
const NOOP_REFRESH: NoopRefresh = NoopRefresh;

/// Inputs for a prime selection+render run.
pub struct PrimeInput<'a> {
    /// Authoritative eligible native skill snapshot (post-precedence/post-shadowing).
    pub eligible_skills: &'a [SkillInfo],
    /// Fresh async revalidation source (PR19 wires `bridge.eligible_native_skills`).
    pub refresh_skills: &'a dyn SkillRefresh,
    /// Session workspace / git root that roots the inventory.
    pub workspace_root: &'a Path,
    /// Canonical containment roots. Bodies are read only for `SKILL.md` files
    /// under these roots (plus the canonical workspace root).
    pub trusted_roots: &'a [PathBuf],
    /// Current prompt text used for scoring (may be empty).
    pub prompt: &'a str,
    /// An explicitly invoked skill name to pin first (never duplicated). Matches
    /// the bare name or the qualified native name (`scope:name`).
    pub explicit_skill: Option<&'a str>,
    pub config: SkillPrimeConfig,
    /// Reserved context window tokens (used for the context-fraction budget).
    pub context_window: Option<u64>,
    /// Optional semantic refinement profile id (must match a published profile).
    pub semantic_profile: Option<&'a str>,
    /// Optional semantic retrieval service (PR17). `None` = deterministic only.
    pub semantic_service: Option<&'a RetrievalService>,
    /// Optional pre-built inventory (cache seam for PR19). `None` builds one.
    pub inventory: Option<&'a WorkspaceInventory>,
}

impl Default for PrimeInput<'_> {
    fn default() -> Self {
        Self {
            eligible_skills: &[],
            refresh_skills: &NOOP_REFRESH,
            workspace_root: Path::new("."),
            trusted_roots: &[],
            prompt: "",
            explicit_skill: None,
            config: SkillPrimeConfig::default(),
            context_window: None,
            semantic_profile: None,
            semantic_service: None,
            inventory: None,
        }
    }
}

/// Secret-free export that lets `degrade_on_error = false` fail the prime run.
/// Never carries query/body/vector/raw provider error text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimeError {
    /// The optional semantic refinement failed and `degrade_on_error` is
    /// disabled, so the run fails closed instead of silently under-filling.
    SemanticRetrievalFailed,
}

impl std::fmt::Display for PrimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrimeError::SemanticRetrievalFailed => {
                f.write_str("skill prime semantic retrieval failed; degrade_on_error disabled")
            }
        }
    }
}

/// Secret-free result of a prime run.
pub struct PrimeSkillSelection {
    /// Selected + natively-loaded skills (name/scope/source path + bounded body).
    pub selected: Vec<LoadedSkill>,
    /// Safe rendered content (None when nothing selected/rendering empty).
    pub rendered: Option<RenderedSkills>,
    /// Secret-free degradations from optional semantic refinement.
    pub degradations: Vec<DegradationNotice>,
    /// Budget/selection state for PR19 accounting.
    pub budget_state: PrimeBudgetState,
    /// True when the inventory walk hit a limit or skipped work (cross-device/error).
    pub inventory_truncated: bool,
    /// Snapshot generation seam for PR19 (currently `None`).
    pub snapshot_generation: Option<u64>,
    /// True when the run was cancelled/deadlined; content is then empty.
    pub cancelled: bool,
}

impl PrimeSkillSelection {
    /// All degradation kinds seen (secret-free).
    pub fn degradation_kinds(&self) -> Vec<DegradationKind> {
        self.degradations.iter().map(|d| d.kind).collect()
    }

    pub(crate) fn empty(cancelled: bool, inventory_truncated: bool) -> Self {
        Self {
            selected: Vec::new(),
            rendered: None,
            degradations: Vec::new(),
            budget_state: PrimeBudgetState::default(),
            inventory_truncated,
            snapshot_generation: None,
            cancelled,
        }
    }
}

impl std::fmt::Debug for PrimeSkillSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never expose query/body/vector/raw provider errors or home paths.
        f.debug_struct("PrimeSkillSelection")
            .field("selected_names", &self.budget_state.selected_names)
            .field("degradations", &self.degradations)
            .field("budget_state", &self.budget_state)
            .field("inventory_truncated", &self.inventory_truncated)
            .field("snapshot_generation", &self.snapshot_generation)
            .field("cancelled", &self.cancelled)
            .field("rendered_chars", &self.rendered.as_ref().map(|r| r.chars))
            .finish()
    }
}

/// Conservative bytes-per-token heuristic used for the configured token budget
/// (see [`render::RenderBudgets`]): a documented proxy, not an absolute
/// guarantee against an arbitrary future tokenizer.
pub const TOKEN_BYTES_EST: usize = 2;

/// Render budgets derived from `SkillPrimeConfig` (+ context window).
///
/// `RenderBudgets.max_tokens` is `Some(0)` when the context-fraction allows a
/// **zero** token allowance (renders no body rows), `Some(n)` otherwise, and
/// `None` only when no token cap is configured (never the case for
/// config-derived budgets). Fraction is clamped to `[0.0, 1.0]`; a zero window
/// yields `Some(0)`.
pub fn render_budgets(config: &SkillPrimeConfig, context_window: Option<usize>) -> RenderBudgets {
    let per_body = config.max_body_chars.max(1) as usize;
    let max_total = config.max_total_chars.max(1) as usize;
    let fraction = config.max_context_fraction.clamp(0.0, 1.0) as f64;

    let max_tokens = if let Some(window) = context_window {
        // context_fraction × window tokens (rounded; f64 avoids float drift).
        // `as usize` saturates for degenerately large values.
        let allowed = (window as f64 * fraction).round() as usize;
        Some((config.max_tokens as usize).min(allowed))
    } else {
        Some(config.max_tokens as usize)
    };

    RenderBudgets {
        per_body_chars: per_body,
        max_total_chars: max_total,
        max_tokens,
    }
}

/// Prime-wide absolute deadline + cancellation. A background task cancels the
/// deadline token after `deadline_ms`; `deadline 0` cancels immediately. The
/// timer task is aborted when the gate is dropped, so it never outlives the run.
struct PrimeGate {
    cancel: CancellationToken,
    deadline: CancellationToken,
    stop: CancellationToken,
}

impl PrimeGate {
    fn new(cancel: CancellationToken, deadline_ms: u64) -> Self {
        let deadline = cancel.child_token();
        let stop = cancel.child_token();
        if deadline_ms == 0 {
            deadline.cancel();
        } else {
            let dl = deadline.clone();
            let stop_tok = stop.clone();
            tokio::spawn(async move {
                tokio::select! {
                    biased;
                    _ = stop_tok.cancelled() => {}
                    _ = tokio::time::sleep(Duration::from_millis(deadline_ms)) => {
                        dl.cancel();
                    }
                }
            });
        }
        Self {
            cancel,
            deadline,
            stop,
        }
    }

    fn cancelled(&self) -> bool {
        self.cancel.is_cancelled() || self.deadline.is_cancelled()
    }
}

impl Drop for PrimeGate {
    fn drop(&mut self) {
        // Abort the deadline timer task and propagate cancellation to any child
        // token still held by in-flight work.
        self.stop.cancel();
        self.deadline.cancel();
    }
}

/// Run the bounded blocking inventory walk off the async executor, cancel-aware.
async fn build_inventory_guarded(
    root: &Path,
    gate: &PrimeGate,
) -> Result<WorkspaceInventory, String> {
    let root = root.to_path_buf();
    let limits = InventoryLimits::default();
    let handle = tokio::task::spawn_blocking(move || inventory::build_inventory(&root, limits));
    tokio::select! {
        biased;
        _ = gate.cancel.cancelled() => Err("prime cancelled".into()),
        _ = gate.deadline.cancelled() => Err("prime deadline".into()),
        res = handle => match res {
            Ok(Ok(inv)) => Ok(inv),
            Ok(Err(e)) => Err(e),
            Err(e) => Err(format!("inventory walk task error: {e}")),
        },
    }
}

/// Run deterministic selection + safe render within `config` budgets.
///
/// Public (callable) seam for PR19. Never splices a conversation. Cancellation
/// and deadline are prime-wide; a cancelled run returns an empty selection with
/// `cancelled = true`. Returns [`PrimeError::SemanticRetrievalFailed`] only when
/// `degrade_on_error` is disabled and the optional semantic refinement fails.
pub async fn run_prime_selection(
    input: &PrimeInput<'_>,
    cancel: CancellationToken,
) -> Result<PrimeSkillSelection, PrimeError> {
    let _started_at = Instant::now();
    let mut degradations: Vec<DegradationNotice> = Vec::new();

    // Disabled is a distinct, non-cancelled state: an empty selection with
    // `cancelled = false`. The deadline gate is only constructed when enabled.
    if !input.config.enabled {
        return Ok(PrimeSkillSelection::empty(false, false));
    }
    let gate = PrimeGate::new(cancel, input.config.deadline_ms);
    if gate.cancelled() {
        return Ok(PrimeSkillSelection::empty(true, false));
    }

    // ── Bounded inventory (blocking walk off the executor) ────────────────
    let mut owned_inventory;
    let default_inventory = WorkspaceInventory::default();
    let mut walk_failed = false;
    let inventory: &WorkspaceInventory = if let Some(i) = input.inventory {
        i
    } else if let Ok(owned) = build_inventory_guarded(input.workspace_root, &gate).await {
        owned_inventory = owned;
        &owned_inventory
    } else {
        // Walk failed or aborted: report incomplete, continue without evidence.
        walk_failed = true;
        owned_inventory = default_inventory;
        &owned_inventory
    };
    let inventory_error = inventory.incomplete() || walk_failed;
    if gate.cancelled() {
        return Ok(PrimeSkillSelection::empty(true, inventory_error));
    }

    // ── Deterministic ranking ───────────────────────────────────────────────
    let ranked = skills::rank_skills(
        input.eligible_skills,
        input.prompt,
        inventory,
        input.explicit_skill,
    );

    // Pinned name set for shortlist exclusion / ordering. The explicit name is
    // normalized once to the concrete selected skill identities: any eligible
    // skill whose bare name or qualified native name (`scope:name`) equals the
    // explicit pin is pinned, so semantic fill (which matches the bare names)
    // preserves the same skill.
    let pinned: HashSet<String> = input
        .explicit_skill
        .map(|name| {
            input
                .eligible_skills
                .iter()
                .filter(|s| s.name == name || format_skill_name(s) == name)
                .map(|s| s.name.clone())
                .collect()
        })
        .unwrap_or_default();

    // ── Optional semantic refinement ───────────────────────────────────────
    let mut order = ranked.clone();
    if !input.prompt.trim().is_empty()
        && let Some(profile) = input.semantic_profile
        && let Some(service) = input.semantic_service
    {
        let hard = !input.config.degrade_on_error;
        let outcome: SemanticFillOutcome = skills::semantic_fill(
            service,
            profile,
            input.prompt,
            input.eligible_skills,
            &order,
            &pinned,
            gate.deadline.child_token(),
            hard,
        )
        .await;

        if outcome.cancelled {
            return Ok(PrimeSkillSelection::empty(true, inventory_error));
        }
        if outcome.hard_error.is_some() {
            return Err(PrimeError::SemanticRetrievalFailed);
        }
        degradations.extend(outcome.degradations);
        order = outcome.order;
    }

    let target = input.config.max_results.max(1) as usize;

    // ── Revalidate against a fresh snapshot + bounded body load (with
    //    backfill) ─────────────────────────────────────────────────────────
    let canonical_roots: Vec<PathBuf> = input
        .trusted_roots
        .iter()
        .map(|r| dunce::canonicalize(r).unwrap_or_else(|_| r.clone()))
        .chain(std::iter::once(
            dunce::canonicalize(input.workspace_root)
                .unwrap_or_else(|_| input.workspace_root.to_path_buf()),
        ))
        .collect();
    let batch = skills::load_and_revalidate(
        input.eligible_skills,
        &order,
        target,
        input.refresh_skills,
        &canonical_roots,
        &gate.deadline,
    )
    .await;

    if gate.cancelled() {
        return Ok(PrimeSkillSelection::empty(true, inventory_error));
    }

    // ── Safe render (within body/token/context budgets) ───────────────────
    // A deadline/cancel firing just before render still yields no content.
    if gate.cancelled() {
        return Ok(PrimeSkillSelection::empty(true, inventory_error));
    }
    let budgets = render_budgets(&input.config, input.context_window.map(|w| w as usize));
    let rendered = if batch.loaded.is_empty() {
        None
    } else {
        Some(render_skills(&batch.loaded, &budgets))
    };

    let selected_names = batch.loaded.iter().map(|l| l.name.clone()).collect();
    let budget_state = PrimeBudgetState {
        selected_names,
        dropped: batch.drop_reasons.len(),
        drop_reasons: batch.drop_reasons,
        over_result_limit: order.len() > target,
    };

    Ok(PrimeSkillSelection {
        selected: batch.loaded,
        rendered,
        degradations,
        budget_state,
        inventory_truncated: inventory_error,
        snapshot_generation: None,
        cancelled: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn pre_cancelled_token_returns_empty_without_err() {
        let mut cfg = SkillPrimeConfig::default();
        cfg.enabled = true;
        cfg.deadline_ms = 1000;
        let input = PrimeInput {
            eligible_skills: &[],
            refresh_skills: &(|| async { Vec::new() }),
            workspace_root: Path::new("."),
            trusted_roots: &[],
            prompt: "",
            explicit_skill: None,
            config: cfg,
            context_window: None,
            semantic_profile: None,
            semantic_service: None,
            inventory: None,
        };
        let cancelled = CancellationToken::new();
        cancelled.cancel();
        let sel = run_prime_selection(&input, cancelled).await.unwrap();
        assert!(sel.cancelled, "cancelled flag must be set");
        assert!(sel.selected.is_empty());
        assert!(sel.rendered.is_none());
    }

    #[tokio::test]
    async fn zero_deadline_aborts_with_cancelled_true() {
        let mut cfg = SkillPrimeConfig::default();
        cfg.enabled = true;
        cfg.deadline_ms = 0;
        let input = PrimeInput {
            config: cfg,
            ..PrimeInput::default()
        };
        let sel = run_prime_selection(&input, CancellationToken::new())
            .await
            .unwrap();
        assert!(sel.cancelled);
        assert!(sel.selected.is_empty());
        assert!(sel.rendered.is_none());
    }

    #[test]
    fn render_budgets_respect_context_fraction_zero_and_clamp() {
        let mut cfg = SkillPrimeConfig::default();
        cfg.max_tokens = 10_000;
        cfg.max_context_fraction = 0.01;
        // 100k-token window * 1% = 1000 tokens allowed < 10k.
        let b = render_budgets(&cfg, Some(100_000));
        assert_eq!(b.max_tokens, Some(1_000));
        // Fraction 0.0 => a zero token allowance renders nothing (no sentinel
        // ambiguity with "no cap").
        cfg.max_context_fraction = 0.0;
        let b = render_budgets(&cfg, Some(100_000));
        assert_eq!(b.max_tokens, Some(0));
        // Fraction > 1 is clamped to 1.0.
        cfg.max_context_fraction = 3.0;
        let b = render_budgets(&cfg, Some(10_000));
        assert_eq!(b.max_tokens, Some(10_000));
        // Zero window yields Some(0).
        cfg.max_context_fraction = 0.5;
        let b = render_budgets(&cfg, Some(0));
        assert_eq!(b.max_tokens, Some(0));
        // Without a window, the config's own token budget applies.
        let b = render_budgets(&cfg, None);
        assert_eq!(b.max_tokens, Some(10_000));

        // Negative fraction (parsed negative) clamps to 0 → zero allowance.
        cfg.max_context_fraction = -1.0;
        let b = render_budgets(&cfg, Some(50_000));
        assert_eq!(b.max_tokens, Some(0));
    }

    #[tokio::test]
    async fn selected_names_reflect_loaded_skills_only() {
        // A ranking that selects two skills where one disappears → the budget
        // state lists only the loaded names (never pre-drop).
        let tmp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        let sdir = root.join("skills").join("a");
        std::fs::create_dir_all(&sdir).unwrap();
        std::fs::write(sdir.join("SKILL.md"), "# Skill A\n").unwrap();
        let path_a = sdir.join("SKILL.md").to_string_lossy().to_string();

        let bdir = root.join("skills").join("gone");
        // no SKILL.md for b (unreadable at load).

        let a = SkillInfo {
            name: "a".into(),
            path: path_a,
            ..SkillInfo::default()
        };
        let b = SkillInfo {
            name: "gone".into(),
            path: bdir.join("SKILL.md").to_string_lossy().to_string(),
            ..SkillInfo::default()
        };
        let skills = vec![a, b];
        let snapshot = skills.clone();
        let refresh = move || {
            let s = snapshot.clone();
            async move { s }
        };
        let mut cfg = SkillPrimeConfig::default();
        cfg.enabled = true;
        cfg.max_results = 5;
        let input = PrimeInput {
            eligible_skills: &skills,
            refresh_skills: &refresh,
            workspace_root: &root,
            trusted_roots: &[root.clone()],
            prompt: "",
            explicit_skill: None,
            config: cfg,
            context_window: None,
            semantic_profile: None,
            semantic_service: None,
            inventory: None,
        };
        let sel = run_prime_selection(&input, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(sel.budget_state.selected_names, vec!["a".to_string()]);
        assert!(sel.budget_state.dropped >= 1);
    }
}
