//! Pure planning primitives for rolling logical-band compaction.
//!
//! The live conversation remains owned by the host. This module only derives
//! budgets, selects one oldest-first source range, and divides that range into
//! deterministic request-sized subchunks without splitting assistant tool calls
//! from their contiguous results.

use std::collections::HashSet;
use std::ops::Range;

use crate::item::CompactionItem;
use crate::token::ItemTokenCounter;

/// Standard number of logical context bands.
pub const DEFAULT_ROLLING_BAND_COUNT: u64 = 4;
/// Smallest supported logical-band count.
pub const MIN_ROLLING_BAND_COUNT: u64 = 3;
/// Largest supported logical-band count.
pub const MAX_ROLLING_BAND_COUNT: u64 = 8;

/// Inputs used to derive one rolling compaction budget plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RollingBudgetInput {
    /// Context window of the active session model.
    pub session_context_window: u64,
    /// Tokens occupied by the fixed system and project prefix.
    pub fixed_prefix_tokens: u64,
    /// Tokens reserved for the next session-model output.
    pub next_turn_output_reserve_tokens: u64,
    /// Additional request/tokenizer uncertainty reserved in the session window.
    pub request_safety_margin_tokens: u64,
    /// Number of logical context bands.
    pub band_count: u64,
    /// Context window of the selected compaction model.
    pub compactor_context_window: u64,
    /// Tokens reserved for the generated compaction summary.
    pub summary_output_reserve_tokens: u64,
    /// Tokens occupied by compaction instructions.
    pub instruction_tokens: u64,
    /// Additional tokenizer uncertainty reserved in the compactor window.
    pub tokenizer_safety_margin_tokens: u64,
    /// Tool-definition overhead, when tools are enabled for compaction.
    pub tool_tax_tokens: u64,
}

/// A successfully derived rolling compaction budget plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RollingBudgetPlan {
    /// Session context available after fixed prefix and headroom reserves.
    pub usable_visible_budget: u64,
    /// Per-band target, rounded down to whole tokens.
    pub nominal_band_target: u64,
    /// Compaction-model input capacity after output and request reserves.
    pub compactor_input_capacity: u64,
    /// Minimum number of requests needed to cover one nominal logical band.
    pub subchunk_count: u64,
    /// Whether one logical band exceeds a single compaction request.
    pub requires_subchunking: bool,
}

/// Invalid or exhausted budget states returned by the planner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RollingBudgetError {
    /// Logical-band count is outside the supported 3–8 range.
    UnsupportedBandCount,
    /// Session reserves consume the entire session context window.
    NoVisibleBudget,
    /// The visible budget is too small to allocate one token to every band.
    NoBandCapacity,
    /// Compactor reserves consume the entire compaction-model context window.
    NoCompactorCapacity,
}

/// Derive a headroom-aware logical-band plan for rolling compaction.
pub fn plan_rolling_budget(
    input: RollingBudgetInput,
) -> Result<RollingBudgetPlan, RollingBudgetError> {
    if !(MIN_ROLLING_BAND_COUNT..=MAX_ROLLING_BAND_COUNT).contains(&input.band_count) {
        return Err(RollingBudgetError::UnsupportedBandCount);
    }

    let usable_visible_budget = input
        .session_context_window
        .saturating_sub(input.fixed_prefix_tokens)
        .saturating_sub(input.next_turn_output_reserve_tokens)
        .saturating_sub(input.request_safety_margin_tokens);
    if usable_visible_budget == 0 {
        return Err(RollingBudgetError::NoVisibleBudget);
    }

    let nominal_band_target = usable_visible_budget / input.band_count;
    if nominal_band_target == 0 {
        return Err(RollingBudgetError::NoBandCapacity);
    }

    let compactor_input_capacity = input
        .compactor_context_window
        .saturating_sub(input.summary_output_reserve_tokens)
        .saturating_sub(input.instruction_tokens)
        .saturating_sub(input.tokenizer_safety_margin_tokens)
        .saturating_sub(input.tool_tax_tokens);
    if compactor_input_capacity == 0 {
        return Err(RollingBudgetError::NoCompactorCapacity);
    }

    // Equivalent to `(n + d - 1) / d` without overflowing near u64::MAX.
    let quotient = nominal_band_target / compactor_input_capacity;
    let remainder = nominal_band_target % compactor_input_capacity;
    let subchunk_count = quotient + u64::from(remainder != 0);

    Ok(RollingBudgetPlan {
        usable_visible_budget,
        nominal_band_target,
        compactor_input_capacity,
        subchunk_count,
        requires_subchunking: subchunk_count > 1,
    })
}

/// One indivisible span in the conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtomicGroup {
    /// Half-open item range in the authoritative conversation.
    pub range: Range<usize>,
    /// Trusted token count for all items in the range.
    pub token_count: u64,
}

/// Oldest-first source selected for one rolling operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollingSourcePlan {
    /// Half-open range selected from the authoritative conversation.
    pub source_range: Range<usize>,
    /// Trusted token count of the selected range.
    pub source_tokens: u64,
    /// Number of indivisible groups in the range.
    pub group_count: usize,
    /// True when the oldest atomic group alone exceeds the nominal band target.
    /// The group is still selected intact so planning can make progress.
    pub exceeds_target: bool,
}

/// One request-sized divide-and-conquer input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollingSubchunk {
    /// Half-open item range within the authoritative conversation.
    pub range: Range<usize>,
    /// Trusted token count for the range.
    pub token_count: u64,
}

/// Two non-empty atomic-group-aligned halves of a source range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollingBisect {
    pub left: RollingSubchunk,
    pub right: RollingSubchunk,
}

/// Invalid source history or exhausted source-planning states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RollingSourceError {
    /// A zero token target or capacity was supplied.
    ZeroCapacity,
    /// No compactable items remain after prefix and hot-tail protection.
    NoPlan,
    /// A count or range lies outside the supplied item slice.
    InvalidBoundary,
    /// The token sum exceeded `u64`.
    TokenCountOverflow,
    /// A tool result is orphaned, displaced, duplicated, or separated from its
    /// owning assistant tool call by a protected boundary.
    InvalidToolSequence { index: usize },
    /// One assistant/tool-result group cannot fit in a compactor request.
    AtomicGroupExceedsCapacity {
        range: Range<usize>,
        token_count: u64,
        capacity: u64,
    },
}

/// Return the number of newest items needed to preserve at least
/// `target_tokens`, snapping the boundary to whole atomic groups.
pub fn plan_protected_tail_count<T, C>(
    items: &[T],
    counter: &C,
    mutable_start: usize,
    target_tokens: u64,
) -> Result<usize, RollingSourceError>
where
    T: CompactionItem,
    C: ItemTokenCounter<T>,
{
    if target_tokens == 0 {
        return Err(RollingSourceError::ZeroCapacity);
    }
    if mutable_start > items.len() {
        return Err(RollingSourceError::InvalidBoundary);
    }
    let groups = identify_atomic_groups(items, counter, mutable_start..items.len())?;
    let mut protected_tokens = 0_u64;
    let mut protected_start = items.len();
    for group in groups.iter().rev() {
        protected_tokens = protected_tokens
            .checked_add(group.token_count)
            .ok_or(RollingSourceError::TokenCountOverflow)?;
        protected_start = group.range.start;
        if protected_tokens >= target_tokens {
            break;
        }
    }
    Ok(items.len().saturating_sub(protected_start))
}

/// Select the oldest whole groups up to one nominal logical-band target.
///
/// If the oldest group itself exceeds `target_tokens`, it is selected intact.
/// This target controls rolling cadence, not request size; request-size limits
/// are enforced separately by [`plan_rolling_subchunks`].
pub fn plan_rolling_source<T, C>(
    items: &[T],
    counter: &C,
    fixed_prefix_count: usize,
    protected_tail_count: usize,
    target_tokens: u64,
) -> Result<RollingSourcePlan, RollingSourceError>
where
    T: CompactionItem,
    C: ItemTokenCounter<T>,
{
    if target_tokens == 0 {
        return Err(RollingSourceError::ZeroCapacity);
    }
    if fixed_prefix_count > items.len() || protected_tail_count > items.len() {
        return Err(RollingSourceError::InvalidBoundary);
    }

    let compactable_end = items
        .len()
        .checked_sub(protected_tail_count)
        .ok_or(RollingSourceError::InvalidBoundary)?;
    if fixed_prefix_count >= compactable_end {
        return Err(RollingSourceError::NoPlan);
    }

    let groups = identify_atomic_groups(items, counter, fixed_prefix_count..compactable_end)?;
    let first = groups.first().ok_or(RollingSourceError::NoPlan)?;
    if first.token_count > target_tokens {
        return Ok(RollingSourcePlan {
            source_range: first.range.clone(),
            source_tokens: first.token_count,
            group_count: 1,
            exceeds_target: true,
        });
    }

    let mut source_tokens = 0_u64;
    let mut source_end = fixed_prefix_count;
    let mut group_count = 0;
    for group in groups {
        let next = source_tokens
            .checked_add(group.token_count)
            .ok_or(RollingSourceError::TokenCountOverflow)?;
        if next > target_tokens {
            break;
        }
        source_tokens = next;
        source_end = group.range.end;
        group_count += 1;
    }

    if group_count == 0 {
        return Err(RollingSourceError::NoPlan);
    }
    Ok(RollingSourcePlan {
        source_range: fixed_prefix_count..source_end,
        source_tokens,
        group_count,
        exceeds_target: false,
    })
}

/// Divide a selected source into deterministic contiguous request-sized chunks.
///
/// Every chunk fits `compactor_input_capacity`. The function fails rather than
/// splitting or silently oversizing an assistant/tool-result group.
pub fn plan_rolling_bisect<T, C>(
    items: &[T],
    counter: &C,
    source_range: Range<usize>,
) -> Result<RollingBisect, RollingSourceError>
where
    T: CompactionItem,
    C: ItemTokenCounter<T>,
{
    let groups = identify_atomic_groups(items, counter, source_range)?;
    if groups.len() < 2 {
        return Err(RollingSourceError::NoPlan);
    }

    let total_tokens = groups.iter().try_fold(0_u64, |total, group| {
        total
            .checked_add(group.token_count)
            .ok_or(RollingSourceError::TokenCountOverflow)
    })?;
    let target = total_tokens / 2;
    let mut left_tokens = 0_u64;
    let mut split_group = 1_usize;
    let mut best_distance = u64::MAX;
    for candidate in 1..groups.len() {
        left_tokens = left_tokens
            .checked_add(groups[candidate - 1].token_count)
            .ok_or(RollingSourceError::TokenCountOverflow)?;
        let distance = left_tokens.abs_diff(target);
        if distance < best_distance {
            best_distance = distance;
            split_group = candidate;
        }
    }

    let left_tokens = groups[..split_group]
        .iter()
        .try_fold(0_u64, |total, group| {
            total
                .checked_add(group.token_count)
                .ok_or(RollingSourceError::TokenCountOverflow)
        })?;
    let split_index = groups[split_group].range.start;
    Ok(RollingBisect {
        left: RollingSubchunk {
            range: groups[0].range.start..split_index,
            token_count: left_tokens,
        },
        right: RollingSubchunk {
            range: split_index..groups.last().expect("groups are non-empty").range.end,
            token_count: total_tokens.saturating_sub(left_tokens),
        },
    })
}

pub fn plan_rolling_subchunks<T, C>(
    items: &[T],
    counter: &C,
    source_range: Range<usize>,
    compactor_input_capacity: u64,
) -> Result<Vec<RollingSubchunk>, RollingSourceError>
where
    T: CompactionItem,
    C: ItemTokenCounter<T>,
{
    if compactor_input_capacity == 0 {
        return Err(RollingSourceError::ZeroCapacity);
    }
    let groups = identify_atomic_groups(items, counter, source_range)?;
    if groups.is_empty() {
        return Err(RollingSourceError::NoPlan);
    }

    let mut chunks = Vec::new();
    let mut current_start = groups[0].range.start;
    let mut current_end = current_start;
    let mut current_tokens = 0_u64;

    for group in groups {
        if group.token_count > compactor_input_capacity {
            return Err(RollingSourceError::AtomicGroupExceedsCapacity {
                range: group.range,
                token_count: group.token_count,
                capacity: compactor_input_capacity,
            });
        }

        let next = current_tokens
            .checked_add(group.token_count)
            .ok_or(RollingSourceError::TokenCountOverflow)?;
        if current_tokens > 0 && next > compactor_input_capacity {
            chunks.push(RollingSubchunk {
                range: current_start..current_end,
                token_count: current_tokens,
            });
            current_start = group.range.start;
            current_tokens = group.token_count;
        } else {
            current_tokens = next;
        }
        current_end = group.range.end;
    }

    chunks.push(RollingSubchunk {
        range: current_start..current_end,
        token_count: current_tokens,
    });
    Ok(chunks)
}

fn identify_atomic_groups<T, C>(
    items: &[T],
    counter: &C,
    range: Range<usize>,
) -> Result<Vec<AtomicGroup>, RollingSourceError>
where
    T: CompactionItem,
    C: ItemTokenCounter<T>,
{
    if range.start > range.end || range.end > items.len() {
        return Err(RollingSourceError::InvalidBoundary);
    }
    if range.start == range.end {
        return Ok(Vec::new());
    }
    if items[range.start].is_tool_result() {
        return Err(RollingSourceError::InvalidToolSequence { index: range.start });
    }
    // A protected tail or subchunk source may not begin in the middle of the
    // assistant/tool-result group immediately before it.
    if range.end < items.len() && items[range.end].is_tool_result() {
        return Err(RollingSourceError::InvalidToolSequence { index: range.end });
    }

    let mut groups = Vec::new();
    let mut index = range.start;
    while index < range.end {
        if items[index].is_tool_result() {
            return Err(RollingSourceError::InvalidToolSequence { index });
        }

        let group_start = index;
        let mut group_end = index + 1;
        let mut group_tokens = u64::from(counter.count_item_tokens(&items[index]));

        if items[index].has_tool_requests() {
            let request_ids = items[index].tool_request_ids();
            let mut seen_result_ids = HashSet::new();
            while group_end < range.end && items[group_end].is_tool_result() {
                if !request_ids.is_empty() {
                    let Some(result_id) = items[group_end].tool_result_id() else {
                        return Err(RollingSourceError::InvalidToolSequence { index: group_end });
                    };
                    if !request_ids
                        .iter()
                        .any(|request_id| request_id == &result_id)
                        || !seen_result_ids.insert(result_id)
                    {
                        return Err(RollingSourceError::InvalidToolSequence { index: group_end });
                    }
                }
                group_tokens = group_tokens
                    .checked_add(u64::from(counter.count_item_tokens(&items[group_end])))
                    .ok_or(RollingSourceError::TokenCountOverflow)?;
                group_end += 1;
            }

            // A historical tool-call group must have its results and, when IDs
            // are available, exactly one contiguous result for every request.
            let result_count = group_end - (group_start + 1);
            if result_count == 0
                || (!request_ids.is_empty() && seen_result_ids.len() != request_ids.len())
            {
                return Err(RollingSourceError::InvalidToolSequence { index: group_start });
            }
        }

        groups.push(AtomicGroup {
            range: group_start..group_end,
            token_count: group_tokens,
        });
        index = group_end;
    }
    Ok(groups)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::{CompactionFileRef, CompactionRole};

    #[derive(Debug, Clone)]
    struct MockItem {
        role: CompactionRole,
        tokens: u32,
        request_ids: Vec<String>,
        result_id: Option<String>,
    }

    impl MockItem {
        fn plain(role: CompactionRole, tokens: u32) -> Self {
            Self {
                role,
                tokens,
                request_ids: Vec::new(),
                result_id: None,
            }
        }

        fn assistant_call(tokens: u32, ids: &[&str]) -> Self {
            Self {
                role: CompactionRole::Assistant,
                tokens,
                request_ids: ids.iter().map(|id| (*id).to_owned()).collect(),
                result_id: None,
            }
        }

        fn tool_result(tokens: u32, id: &str) -> Self {
            Self {
                role: CompactionRole::Tool,
                tokens,
                request_ids: Vec::new(),
                result_id: Some(id.to_owned()),
            }
        }
    }

    impl CompactionItem for MockItem {
        fn role(&self) -> CompactionRole {
            self.role
        }

        fn text(&self) -> Option<String> {
            None
        }

        fn has_tool_requests(&self) -> bool {
            !self.request_ids.is_empty()
        }

        fn tool_request_ids(&self) -> Vec<String> {
            self.request_ids.clone()
        }

        fn tool_result_id(&self) -> Option<String> {
            self.result_id.clone()
        }

        fn is_compaction_summary(&self) -> bool {
            false
        }

        fn attachment_refs(&self) -> Vec<CompactionFileRef> {
            Vec::new()
        }
    }

    struct MockCounter;

    impl ItemTokenCounter<MockItem> for MockCounter {
        fn count_item_tokens(&self, item: &MockItem) -> u32 {
            item.tokens
        }
    }

    fn budget_input(session: u64, compactor: u64) -> RollingBudgetInput {
        RollingBudgetInput {
            session_context_window: session,
            fixed_prefix_tokens: 0,
            next_turn_output_reserve_tokens: 0,
            request_safety_margin_tokens: 0,
            band_count: DEFAULT_ROLLING_BAND_COUNT,
            compactor_context_window: compactor,
            summary_output_reserve_tokens: 0,
            instruction_tokens: 0,
            tokenizer_safety_margin_tokens: 0,
            tool_tax_tokens: 0,
        }
    }

    #[test]
    fn standard_windows_have_expected_nominal_bands() {
        assert_eq!(
            plan_rolling_budget(budget_input(1_000_000, 1_000_000))
                .unwrap()
                .nominal_band_target,
            250_000
        );
        assert_eq!(
            plan_rolling_budget(budget_input(1_048_576, 1_048_576))
                .unwrap()
                .nominal_band_target,
            262_144
        );
        assert_eq!(
            plan_rolling_budget(budget_input(500_000, 500_000))
                .unwrap()
                .nominal_band_target,
            125_000
        );
    }

    #[test]
    fn session_reserves_reduce_visible_band_target() {
        let mut input = budget_input(1_000_000, 1_000_000);
        input.fixed_prefix_tokens = 40_000;
        input.next_turn_output_reserve_tokens = 32_000;
        input.request_safety_margin_tokens = 8_000;
        let plan = plan_rolling_budget(input).unwrap();
        assert_eq!(plan.usable_visible_budget, 920_000);
        assert_eq!(plan.nominal_band_target, 230_000);
    }

    #[test]
    fn smaller_compactor_requires_divide_and_conquer() {
        let mut input = budget_input(1_000_000, 200_000);
        input.summary_output_reserve_tokens = 20_000;
        input.instruction_tokens = 5_000;
        input.tokenizer_safety_margin_tokens = 3_000;
        input.tool_tax_tokens = 2_000;
        let plan = plan_rolling_budget(input).unwrap();
        assert_eq!(plan.compactor_input_capacity, 170_000);
        assert_eq!(plan.subchunk_count, 2);
        assert!(plan.requires_subchunking);
    }

    #[test]
    fn unsupported_band_counts_are_rejected() {
        for band_count in [0, 1, 2, 9, u64::MAX] {
            let mut input = budget_input(1_000, 1_000);
            input.band_count = band_count;
            assert_eq!(
                plan_rolling_budget(input),
                Err(RollingBudgetError::UnsupportedBandCount)
            );
        }
    }

    #[test]
    fn exhausted_budgets_are_rejected() {
        let mut visible = budget_input(100, 100);
        visible.fixed_prefix_tokens = 100;
        assert_eq!(
            plan_rolling_budget(visible),
            Err(RollingBudgetError::NoVisibleBudget)
        );

        let mut band = budget_input(3, 100);
        band.band_count = 4;
        assert_eq!(
            plan_rolling_budget(band),
            Err(RollingBudgetError::NoBandCapacity)
        );

        let mut compactor = budget_input(1_000, 100);
        compactor.summary_output_reserve_tokens = 100;
        assert_eq!(
            plan_rolling_budget(compactor),
            Err(RollingBudgetError::NoCompactorCapacity)
        );
    }

    #[test]
    fn protected_tail_reaches_target_on_atomic_group_boundaries() {
        let items = [
            MockItem::plain(CompactionRole::System, 50),
            MockItem::plain(CompactionRole::User, 100),
            MockItem::assistant_call(50, &["a", "b"]),
            MockItem::tool_result(30, "a"),
            MockItem::tool_result(40, "b"),
            MockItem::plain(CompactionRole::User, 150),
        ];
        assert_eq!(
            plan_protected_tail_count(&items, &MockCounter, 1, 200).unwrap(),
            4
        );
    }

    #[test]
    fn source_respects_prefix_tail_and_target() {
        let items = [
            MockItem::plain(CompactionRole::System, 50),
            MockItem::plain(CompactionRole::User, 100),
            MockItem::plain(CompactionRole::Assistant, 200),
            MockItem::plain(CompactionRole::User, 150),
            MockItem::plain(CompactionRole::Assistant, 250),
        ];
        let plan = plan_rolling_source(&items, &MockCounter, 1, 1, 500).unwrap();
        assert_eq!(plan.source_range, 1..4);
        assert_eq!(plan.source_tokens, 450);
        assert_eq!(plan.group_count, 3);
        assert!(!plan.exceeds_target);
    }

    #[test]
    fn source_preserves_oversized_oldest_group_to_make_progress() {
        let items = [
            MockItem::assistant_call(700, &["call-1"]),
            MockItem::tool_result(100, "call-1"),
            MockItem::plain(CompactionRole::User, 50),
        ];
        let plan = plan_rolling_source(&items, &MockCounter, 0, 0, 500).unwrap();
        assert_eq!(plan.source_range, 0..2);
        assert_eq!(plan.source_tokens, 800);
        assert!(plan.exceeds_target);
    }

    #[test]
    fn assistant_and_all_matching_results_are_atomic() {
        let items = [
            MockItem::plain(CompactionRole::User, 100),
            MockItem::assistant_call(50, &["a", "b"]),
            MockItem::tool_result(30, "a"),
            MockItem::tool_result(40, "b"),
            MockItem::plain(CompactionRole::User, 150),
        ];
        let plan = plan_rolling_source(&items, &MockCounter, 0, 0, 220).unwrap();
        assert_eq!(plan.source_range, 0..4);
        assert_eq!(plan.source_tokens, 220);
        assert_eq!(plan.group_count, 2);
    }

    #[test]
    fn orphan_displaced_duplicate_and_missing_results_are_rejected() {
        let orphan = [
            MockItem::plain(CompactionRole::User, 10),
            MockItem::tool_result(10, "x"),
        ];
        assert!(matches!(
            plan_rolling_source(&orphan, &MockCounter, 0, 0, 100),
            Err(RollingSourceError::InvalidToolSequence { index: 1 })
        ));

        let displaced = [
            MockItem::assistant_call(10, &["x"]),
            MockItem::plain(CompactionRole::User, 10),
            MockItem::tool_result(10, "x"),
        ];
        assert!(matches!(
            plan_rolling_source(&displaced, &MockCounter, 0, 0, 100),
            Err(RollingSourceError::InvalidToolSequence { index: 0 })
        ));

        let duplicate = [
            MockItem::assistant_call(10, &["x"]),
            MockItem::tool_result(10, "x"),
            MockItem::tool_result(10, "x"),
        ];
        assert!(matches!(
            plan_rolling_source(&duplicate, &MockCounter, 0, 0, 100),
            Err(RollingSourceError::InvalidToolSequence { index: 2 })
        ));

        let missing = [
            MockItem::assistant_call(10, &["x", "y"]),
            MockItem::tool_result(10, "x"),
        ];
        assert!(matches!(
            plan_rolling_source(&missing, &MockCounter, 0, 0, 100),
            Err(RollingSourceError::InvalidToolSequence { index: 0 })
        ));
    }

    #[test]
    fn a_protected_boundary_cannot_split_a_tool_group() {
        let items = [
            MockItem::assistant_call(10, &["x"]),
            MockItem::tool_result(10, "x"),
            MockItem::plain(CompactionRole::User, 10),
        ];
        assert!(matches!(
            plan_rolling_source(&items, &MockCounter, 0, 2, 100),
            Err(RollingSourceError::InvalidToolSequence { index: 1 })
        ));
    }

    #[test]
    fn bisect_balances_tokens_without_splitting_atomic_groups() {
        let items = [
            MockItem::plain(CompactionRole::User, 100),
            MockItem::assistant_call(50, &["a", "b"]),
            MockItem::tool_result(30, "a"),
            MockItem::tool_result(40, "b"),
            MockItem::plain(CompactionRole::User, 150),
            MockItem::plain(CompactionRole::Assistant, 200),
        ];
        let split = plan_rolling_bisect(&items, &MockCounter, 0..6).unwrap();
        assert_eq!(split.left.range, 0..4);
        assert_eq!(split.left.token_count, 220);
        assert_eq!(split.right.range, 4..6);
        assert_eq!(split.right.token_count, 350);
    }

    #[test]
    fn bisect_requires_two_atomic_groups() {
        let items = [
            MockItem::assistant_call(50, &["a"]),
            MockItem::tool_result(30, "a"),
        ];
        assert_eq!(
            plan_rolling_bisect(&items, &MockCounter, 0..2),
            Err(RollingSourceError::NoPlan)
        );
    }

    #[test]
    fn subchunks_are_contiguous_complete_and_capacity_bounded() {
        let items = [
            MockItem::plain(CompactionRole::User, 100),
            MockItem::assistant_call(50, &["a", "b"]),
            MockItem::tool_result(30, "a"),
            MockItem::tool_result(40, "b"),
            MockItem::plain(CompactionRole::User, 150),
            MockItem::plain(CompactionRole::Assistant, 200),
        ];
        let chunks = plan_rolling_subchunks(&items, &MockCounter, 0..6, 250).unwrap();
        assert_eq!(
            chunks,
            vec![
                RollingSubchunk {
                    range: 0..4,
                    token_count: 220,
                },
                RollingSubchunk {
                    range: 4..5,
                    token_count: 150,
                },
                RollingSubchunk {
                    range: 5..6,
                    token_count: 200,
                },
            ]
        );
        assert_eq!(chunks.first().unwrap().range.start, 0);
        assert_eq!(chunks.last().unwrap().range.end, 6);
        assert!(
            chunks
                .windows(2)
                .all(|pair| pair[0].range.end == pair[1].range.start)
        );
        assert!(chunks.iter().all(|chunk| chunk.token_count <= 250));
    }

    #[test]
    fn subchunks_reject_an_atomic_group_larger_than_compactor_capacity() {
        let items = [
            MockItem::assistant_call(200, &["x"]),
            MockItem::tool_result(100, "x"),
        ];
        assert_eq!(
            plan_rolling_subchunks(&items, &MockCounter, 0..2, 250),
            Err(RollingSourceError::AtomicGroupExceedsCapacity {
                range: 0..2,
                token_count: 300,
                capacity: 250,
            })
        );
    }

    #[test]
    fn invalid_ranges_and_zero_capacity_are_rejected() {
        let items = [MockItem::plain(CompactionRole::User, 10)];
        assert_eq!(
            plan_rolling_source(&items, &MockCounter, 2, 0, 100),
            Err(RollingSourceError::InvalidBoundary)
        );
        assert_eq!(
            plan_rolling_subchunks(&items, &MockCounter, 0..1, 0),
            Err(RollingSourceError::ZeroCapacity)
        );
        assert_eq!(
            plan_rolling_subchunks(&items, &MockCounter, 1..1, 100),
            Err(RollingSourceError::NoPlan)
        );
    }
}
