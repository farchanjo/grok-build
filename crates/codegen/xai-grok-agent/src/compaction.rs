//! Compaction policy — threshold, routing, strategy, and memory flush configuration.

/// Runtime compaction strategy selected by shell configuration.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CompactionStrategy {
    /// Prefer rolling compaction when its prerequisites are available, otherwise
    /// retain the compatible full-replace path.
    #[default]
    Auto,
    /// Compact the oldest logical band and preserve the recent raw tail.
    Rolling,
    /// Replace the full mutable history with one summary.
    FullReplace,
}

/// Policy used to decide when automatic compaction should start.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CompactionTriggerPolicy {
    /// Preserve the existing percentage threshold behavior.
    #[default]
    Fixed,
    /// Account for output, safety, prefix, and compactor capacity reserves.
    Dynamic,
}

/// Session-level compaction policy.
///
/// Controls when and how the session's conversation is compacted
/// to free up context window space, and whether a memory flush
/// runs before each compaction.
#[derive(Debug, Clone)]
pub struct CompactionPolicy {
    /// Percentage of context window that triggers auto-compaction.
    /// E.g., 85 means compact when 85% of the context window is used.
    pub auto_compact_threshold_percent: u32,

    /// Ordered compaction routes. `@session` uses the active session model;
    /// otherwise the value is a stable catalog model ID. At most two routes
    /// are admitted by shell configuration validation.
    pub compact_models: Vec<String>,

    /// Automatic compaction strategy.
    pub strategy: CompactionStrategy,

    /// Automatic compaction trigger policy.
    pub trigger_policy: CompactionTriggerPolicy,

    /// Number of logical bands used by rolling compaction.
    pub rolling_band_count: usize,

    /// Whether the summarizer may run read-only artifact lookups (`read_file`,
    /// `grep`, `list_dir`) before writing the summary, so it can resolve the
    /// files, logs, and media referenced in the conversation it is compressing.
    pub resolver_tools: bool,

    /// Whether to run a memory flush turn before each compaction.
    /// When enabled, the session actor asks the model to summarize
    /// important information from the conversation before it's compacted.
    /// Requires the memory system to be enabled.
    pub memory_flush_enabled: bool,

    /// Per-compaction wall-clock budget (seconds); a generation exceeding it is
    /// cut and retried — the backstop for reasoning runaways token limits miss.
    pub wall_clock_budget_secs: u64,

    /// Prefire two-pass compaction: when usage approaches the threshold,
    /// speculatively summarize the history prefix in the background (pass 1);
    /// at compaction, summarize NOTE₁ + the recent tail (pass 2). Resolved from
    /// config (`two_pass_compaction` flag) at session build; `false` keeps the
    /// legacy single-pass path. Default `false` (real sessions set it from config).
    pub two_pass_enabled: bool,
}

impl Default for CompactionPolicy {
    fn default() -> Self {
        Self {
            auto_compact_threshold_percent: 85,
            compact_models: vec!["@session".to_owned()],
            strategy: CompactionStrategy::Auto,
            trigger_policy: CompactionTriggerPolicy::Fixed,
            rolling_band_count: 4,
            resolver_tools: true,
            memory_flush_enabled: false,
            wall_clock_budget_secs: 300,
            two_pass_enabled: false,
        }
    }
}
