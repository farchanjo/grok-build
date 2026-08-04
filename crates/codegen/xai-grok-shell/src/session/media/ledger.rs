//! Auxiliary usage ledger (plan section 12).
//!
//! One row is appended per auxiliary media-understanding attempt to
//! `assets/media/usage.jsonl`. Rows are append-only and written under an
//! exclusive advisory lock so leader-mode concurrent writers never produce
//! torn lines. Auxiliary usage stays distinct from main-loop `TokenUsage`;
//! cache hits have zero new provider cost but remain visible as hits.
//!
//! Costs are optional: providers without catalog-estimated pricing record
//! `cost_unknown = true` and no `estimated_cost_usd_ticks`.

use crate::session::media::{append_jsonl_line_locked, now_ts, read_jsonl};
use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;
use xai_grok_tools::media::domain::{MediaCategory, MediaCategoryStrategy};

/// Purpose of an auxiliary media-understanding call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UsagePurpose {
    /// Explicit `analyze_media` tool request.
    ExplicitTool,
    /// Automatic attachment enrichment for a text-only session model.
    AutoAttachment,
    /// Compaction preflight enrichment.
    Compaction,
}

/// One append-only usage row for a single auxiliary attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct UsageRow {
    /// Unix seconds since the epoch.
    pub timestamp: i64,
    pub purpose: UsagePurpose,
    pub category: MediaCategory,
    pub provider: String,
    pub model: String,
    /// Index of the route within its category's configured route list.
    pub route_index: u32,
    pub strategy: MediaCategoryStrategy,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub tokens_cached: u64,
    /// True when no catalog-estimated cost is available for this attempt.
    pub cost_unknown: bool,
    /// Catalog-estimated cost in USD ticks, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_cost_usd_ticks: Option<u64>,
    pub cache_hit: bool,
    pub cache_miss: bool,
    pub duration_ms: u64,
    /// Outcome label (e.g. `"success"`, `"skipped"`, `"failed"`, `"cached"`).
    pub outcome: String,
    /// Optional non-secret reason (e.g. `"route_skipped: missing_credentials"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl UsageRow {
    /// Start a row for an auxiliary attempt; remaining fields are filled via
    /// the builder-style setters.
    pub(crate) fn new(
        purpose: UsagePurpose,
        category: MediaCategory,
        provider: impl Into<String>,
        model: impl Into<String>,
        route_index: u32,
        strategy: MediaCategoryStrategy,
    ) -> Self {
        Self {
            timestamp: now_ts(),
            purpose,
            category,
            provider: provider.into(),
            model: model.into(),
            route_index,
            strategy,
            tokens_in: 0,
            tokens_out: 0,
            tokens_cached: 0,
            cost_unknown: false,
            estimated_cost_usd_ticks: None,
            cache_hit: false,
            cache_miss: false,
            duration_ms: 0,
            outcome: String::new(),
            reason: None,
        }
    }

    pub(crate) fn with_tokens(mut self, input: u64, output: u64, cached: u64) -> Self {
        self.tokens_in = input;
        self.tokens_out = output;
        self.tokens_cached = cached;
        self
    }

    pub(crate) fn with_cost(mut self, ticks: u64) -> Self {
        self.cost_unknown = false;
        self.estimated_cost_usd_ticks = Some(ticks);
        self
    }

    pub(crate) fn with_cost_unknown(mut self) -> Self {
        self.cost_unknown = true;
        self.estimated_cost_usd_ticks = None;
        self
    }

    pub(crate) fn with_cache_hit(mut self) -> Self {
        self.cache_hit = true;
        self.cache_miss = false;
        self
    }

    pub(crate) fn with_cache_miss(mut self) -> Self {
        self.cache_hit = false;
        self.cache_miss = true;
        self
    }

    pub(crate) fn with_duration(mut self, ms: u64) -> Self {
        self.duration_ms = ms;
        self
    }

    pub(crate) fn with_outcome(mut self, outcome: impl Into<String>) -> Self {
        self.outcome = outcome.into();
        self
    }

    pub(crate) fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

/// Append-only usage ledger backed by `assets/media/usage.jsonl`.
#[derive(Debug, Clone)]
pub(crate) struct UsageLedger {
    path: std::path::PathBuf,
}

impl UsageLedger {
    pub(crate) fn open(session_dir: &Path) -> io::Result<Self> {
        let store = crate::session::media::artifacts::MediaArtifactStore::open(session_dir)?;
        Ok(Self {
            path: store.usage_path(),
        })
    }

    /// Append one usage row.
    pub(crate) fn append(&self, row: &UsageRow) -> io::Result<()> {
        self.append_batch(std::slice::from_ref(row))
    }

    /// Append several usage rows in one locked write.
    pub(crate) fn append_batch(&self, rows: &[UsageRow]) -> io::Result<()> {
        if rows.is_empty() {
            return Ok(());
        }
        let mut content = Vec::new();
        for row in rows {
            serde_json::to_writer(&mut content, row)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            content.push(b'\n');
        }
        append_jsonl_line_locked(&self.path, content)
    }

    /// Read all usage rows (used by tests and diagnostics).
    pub(crate) fn read(&self) -> io::Result<Vec<UsageRow>> {
        read_jsonl(&self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_row(purpose: UsagePurpose) -> UsageRow {
        UsageRow::new(
            purpose,
            MediaCategory::Image,
            "xai",
            "grok-4.5",
            0,
            MediaCategoryStrategy::Native,
        )
        .with_tokens(120, 40, 0)
        .with_cost(1500)
        .with_cache_miss()
        .with_duration(231)
        .with_outcome("success")
    }

    #[test]
    fn media_ledger_appends_rows_and_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = UsageLedger::open(dir.path()).unwrap();

        let row = sample_row(UsagePurpose::ExplicitTool);
        ledger.append(&row).unwrap();

        let rows = ledger.read().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0], row);
        assert_eq!(rows[0].purpose, UsagePurpose::ExplicitTool);
        assert_eq!(rows[0].tokens_in, 120);
        assert_eq!(rows[0].estimated_cost_usd_ticks, Some(1500));
        assert!(!rows[0].cost_unknown);
        assert!(rows[0].cache_miss);
        assert_eq!(rows[0].outcome, "success");
    }

    #[test]
    fn media_ledger_cost_unknown_flag_and_cache_hit() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = UsageLedger::open(dir.path()).unwrap();

        let unknown = sample_row(UsagePurpose::Compaction)
            .with_cost_unknown()
            .with_cache_hit();
        ledger.append(&unknown).unwrap();
        let rows = ledger.read().unwrap();
        assert!(rows[0].cost_unknown);
        assert!(rows[0].estimated_cost_usd_ticks.is_none());
        assert!(rows[0].cache_hit);
        assert!(!rows[0].cache_miss);
    }

    #[test]
    fn media_ledger_concurrent_append_keeps_every_line_parseable() {
        const WRITERS: usize = 4;
        const ROWS_PER_WRITER: usize = 25;
        let dir = tempfile::tempdir().unwrap();

        let mut handles = Vec::new();
        for writer in 0..WRITERS {
            let ledger = UsageLedger::open(dir.path()).unwrap();
            handles.push(std::thread::spawn(move || {
                for index in 0..ROWS_PER_WRITER {
                    let row = sample_row(UsagePurpose::AutoAttachment)
                        .with_outcome(format!("writer-{writer}-row-{index}"));
                    ledger.append(&row).unwrap();
                }
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }

        let rows = UsageLedger::open(dir.path()).unwrap().read().unwrap();
        assert_eq!(rows.len(), WRITERS * ROWS_PER_WRITER);
        let mut outcomes = std::collections::BTreeSet::new();
        for row in &rows {
            outcomes.insert(row.outcome.clone());
            assert!(row.timestamp > 0);
        }
        assert_eq!(
            outcomes.len(),
            WRITERS * ROWS_PER_WRITER,
            "no duplicate rows"
        );
    }
}
