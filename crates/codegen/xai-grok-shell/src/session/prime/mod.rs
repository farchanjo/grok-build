//! PR18 native skill prime — inventory, deterministic selection, and safe render.
//!
//! This module is the callable selection+render seam for PR19. It:
//!
//! 1. Exposes a bounded lazy workspace inventory ([`inventory`]).
//! 2. Deterministically ranks the eligible native skill snapshot (pinned-first,
//!    exact `when-to-use` / prompt-path / workspace-path evidence), optionally
//!    refines a bounded non-pinned shortlist via the PR17 semantic retrieval
//!    service (metadata only — bodies stay local), and selects within
//!    [`SkillPrimeConfig`] result/body/total/token/context-fraction budgets.
//! 3. Revalidates eligibility/trust/canonical containment against the fresh
//!    authoritative snapshot immediately before loading bodies natively.
//! 4. Renders selected bodies as untrusted quoted context with provenance and
//!    a precedence statement, escaped so no skill can forge the wrapper.
//!
//! It never splices a conversation and performs no prompt injection (that is
//! PR19). Query/body/vector/raw provider errors are kept out of Debug/telemetry.

pub mod inventory;
pub mod render;
pub mod skills;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use tokio_util::sync::CancellationToken;
use xai_grok_config_types::SkillPrimeConfig;
use xai_grok_tools::implementations::skills::types::SkillInfo;

use crate::retrieval::{DegradationKind, DegradationNotice, RetrievalService};

pub use self::render::{LoadedSkill, RenderBudgets, RenderedSkills};
pub use self::skills::{PrimeBudgetState, PrimeDropReason};

use self::inventory::{InventoryLimits, WorkspaceInventory, build_inventory};
use self::render::render_skills;

/// Inputs for a prime selection+render run.
pub struct PrimeInput<'a> {
    /// Authoritative eligible native skill snapshot (post-precedence/post-shadowing).
    pub eligible_skills: &'a [SkillInfo],
    /// Re-fetch the live authoritative eligible snapshot for revalidation (the
    /// session reads `bridge.eligible_native_skills()` here).
    pub refresh_skills: &'a dyn Fn() -> Vec<SkillInfo>,
    /// Session workspace / git root that roots the inventory and default trust.
    pub workspace_root: &'a Path,
    /// Canonical containment roots (workspace root plus any user/grok skill
    /// roots). Bodies are refused if their canonical path escapes every root.
    pub trusted_roots: &'a [PathBuf],
    /// Current prompt text used for scoring (may be empty).
    pub prompt: &'a str,
    /// An explicitly invoked skill name to pin first (never duplicated).
    pub explicit_skill: Option<&'a str>,
    pub config: SkillPrimeConfig,
    /// Context window in tokens (used for the context-fraction budget).
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
        // Most fields are typically supplied; this satisfies callers that only
        // need the deterministic/default shape for tests.
        Self {
            eligible_skills: &[],
            refresh_skills: &(|| Vec::new()) as &dyn Fn() -> Vec<SkillInfo>,
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

/// Secret-free, constitutional result of a prime run.
pub struct PrimeSkillSelection {
    /// Selected + natively-loaded skills (name/scope/path provenance + body).
    pub selected: Vec<LoadedSkill>,
    /// Safe rendered content (None when nothing selected or rendering empty).
    pub rendered: Option<RenderedSkills>,
    /// Secret-free degradations from optional semantic refinement.
    pub degradations: Vec<DegradationNotice>,
    /// Budget/selection state for PR19 accounting.
    pub budget_state: PrimeBudgetState,
    /// True when the workspace inventory walk hit a limit.
    pub inventory_truncated: bool,
    /// Snapshot generation the authoritative snapshot was read at. `None` until
    /// PR19 plumb is a concrete generation.
    pub snapshot_generation: Option<u64>,
    /// Cancellation surfaced from a semantic stage (PR19 may abort the turn).
    pub cancelled: bool,
}

impl PrimeSkillSelection {
    /// All degradation kinds seen (secret-free label).
    pub fn degradation_kinds(&self) -> Vec<DegradationKind> {
        self.degradations.iter().map(|d| d.kind).collect()
    }
}

impl std::fmt::Debug for PrimeSkillSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never expose query/body/vector/raw provider errors through Debug.
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

/// Render budgets derived from `SkillPrimeConfig` (+ context fraction).
pub fn render_budgets(config: &SkillPrimeConfig, context_window: Option<u64>) -> RenderBudgets {
    let per_body = config.max_body_chars.max(1) as usize;
    let max_total = config.max_total_chars.max(1) as usize;
    let mut max_tokens = config.max_tokens as usize;
    if let Some(window) = context_window {
        // context_fraction * window_tokens (f64 to avoid float-truncation drift).
        let allowed = (window as f64 * config.max_context_fraction as f64).round() as usize;
        max_tokens = max_tokens.min(allowed);
    }
    RenderBudgets {
        per_body_chars: per_body,
        max_total_chars: max_total,
        max_tokens,
    }
}

/// Whether the prime deadline has elapsed (used to gate optional semantic fill).
/// `deadline_ms == 0` degrades immediately (deterministic test seam).
fn deadline_reached(started: Instant, deadline_ms: u64) -> bool {
    started.elapsed().as_millis() as u64 >= deadline_ms
}

/// Run deterministic selection + safe render within `config` budgets.
///
/// Public (callable) seam for PR19. This function never splices a
/// conversation — it returns a [`PrimeSkillSelection`].
pub async fn run_prime_selection(
    input: &PrimeInput<'_>,
    cancel: CancellationToken,
) -> PrimeSkillSelection {
    let started = Instant::now();
    let mut degradations: Vec<DegradationNotice> = Vec::new();
    let mut cancelled = false;

    if !input.config.enabled {
        return PrimeSkillSelection {
            selected: Vec::new(),
            rendered: None,
            degradations,
            budget_state: PrimeBudgetState::default(),
            inventory_truncated: false,
            snapshot_generation: None,
            cancelled,
        };
    }

    // Inventory (bounded).
    let owned_inventory;
    let inventory: &WorkspaceInventory = match input.inventory {
        Some(i) => i,
        None => {
            owned_inventory = build_inventory(input.workspace_root, InventoryLimits::default())
                .unwrap_or_else(|_| WorkspaceInventory::default());
            &owned_inventory
        }
    };

    // Deterministic ranking.
    let ranked = skills::rank_skills(
        input.eligible_skills,
        input.prompt,
        inventory,
        input.explicit_skill,
    );

    // Pinned name set for shortlist exclusion / ordering.
    let pinned: HashSet<&str> = input.explicit_skill.into_iter().collect();

    // Optional semantic refinement.
    let mut order = ranked.clone();
    if !input.prompt.trim().is_empty()
        && let Some(profile) = input.semantic_profile
        && let Some(service) = input.semantic_service
    {
        if deadline_reached(started, input.config.deadline_ms) {
            degradations.push(DegradationNotice::new(
                DegradationKind::BudgetExhausted,
                profile,
                crate::retrieval::RetrievalStage::Orchestrate,
                None,
            ));
        } else {
            let outcome = skills::semantic_fill(
                service,
                profile,
                input.prompt,
                input.eligible_skills,
                &order,
                &pinned,
                cancel.child_token(),
            )
            .await;
            cancelled = outcome.cancelled;
            degradations.extend(outcome.degradations);
            order = outcome.order;
        }
    }

    // Select within result budget.
    let selected = skills::select_limit(&order, &input.config);
    let over_result_limit = order.len() > input.config.max_results.max(1) as usize;

    // Revalidate + load bodies.
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
        &selected,
        input.refresh_skills,
        &canonical_roots,
    )
    .await;

    // Render within body/token/context budgets.
    let budgets = render_budgets(&input.config, input.context_window);
    let rendered = if batch.loaded.is_empty() {
        None
    } else {
        Some(self::render::render_skills(&batch.loaded, &budgets))
    };

    let selected_names = selected
        .iter()
        .map(|&i| input.eligible_skills[i].name.clone())
        .collect();

    let budget_state = PrimeBudgetState {
        selected_names,
        dropped: batch.drop_reasons.len(),
        drop_reasons: batch.drop_reasons,
        over_result_limit,
    };

    PrimeSkillSelection {
        selected: batch.loaded,
        rendered,
        degradations,
        budget_state,
        inventory_truncated: inventory.truncated,
        snapshot_generation: None,
        cancelled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deadline_reached_logic() {
        // 0 deadline degrades immediately (deterministic).
        assert!(deadline_reached(Instant::now(), 0));
        // A huge deadline is never reached for a fresh start.
        assert!(!deadline_reached(Instant::now(), u64::MAX));
    }

    #[test]
    fn render_budgets_respect_context_fraction_and_tokens() {
        let mut cfg = SkillPrimeConfig::default();
        cfg.max_tokens = 10_000;
        cfg.max_context_fraction = 0.01;
        // 100k-token window * 1% = 1000 tokens allowed < 10k.
        let b = render_budgets(&cfg, Some(100_000));
        assert_eq!(b.max_tokens, 1_000);
        // Without window, config token budget wins.
        let b2 = render_budgets(&cfg, None);
        assert_eq!(b2.max_tokens, 10_000);
    }

    #[tokio::test]
    async fn api_is_selection_only_no_conversation_splice() {
        let mut cfg = SkillPrimeConfig::default();
        cfg.enabled = true;
        let input = PrimeInput {
            eligible_skills: &[],
            refresh_skills: &(|| Vec::new()),
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
        let sel = run_prime_selection(&input, CancellationToken::new()).await;
        assert!(sel.selected.is_empty());
        assert!(sel.rendered.is_none());
    }
}
