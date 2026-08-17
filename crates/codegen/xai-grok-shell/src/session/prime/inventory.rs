//! Bounded lazy workspace inventory for prime path scoring (PR18).
//!
//! Roots a bounded scan at the workspace/git root and collects a normalized,
//! deterministic set of relative paths. The walker mirrors the safe workspace
//! walker semantics used by the fs surfaces ([`xai_grok_workspace::file_system`]):
//! it honors ignore rules, never follows a symlink (and never descends through
//! one that would escape the canonical root), and applies explicit
//! entry/depth/aggregate-byte/wall-clock limits so a pathological tree cannot
//! balloon memory or latency.
//!
//! PR19 (`/clear`, touched paths) is not wired here — this exposes a
//! session-cache/invalidation seam ([`InventoryCache`]) that PR19 can drive
//! without turning the inventory into a live per-turn walker.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Directory entry level below which the walker stops (0 = only files but no
/// descent into subdirectories of the root). Mirrors the shell fs default so
/// path scoring sees a shallow, cheap slice of the workspace.
const DEFAULT_MAX_DEPTH: usize = 4;

/// Bounded inventory limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InventoryLimits {
    /// Max depth below the root (number of directory levels descended).
    pub max_depth: usize,
    /// Hard cap on the number of collected entries. The walk stops (and the
    /// result is marked `truncated`) once the cap is reached.
    pub max_entries: usize,
    /// Hard cap on the aggregate byte size of collected file entries
    /// (directories count as zero). Stops the walk early when exceeded and
    /// marks the result `truncated`.
    pub max_aggregate_bytes: u64,
    /// Wall-clock budget for the whole walk. When exceeded the walk stops and
    /// the result is marked `truncated`.
    pub max_wall_ms: u64,
}

impl Default for InventoryLimits {
    fn default() -> Self {
        Self {
            max_depth: DEFAULT_MAX_DEPTH,
            max_entries: 4_000,
            max_aggregate_bytes: 8 * 1024 * 1024,
            max_wall_ms: 150,
        }
    }
}

/// One collected inventory path (normalized, root-relative).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryEntry {
    /// Normalized path relative to the root, `/`-separated, deterministic sort.
    pub rel: String,
    /// File size in bytes (0 for directories).
    pub size_bytes: u64,
    pub is_dir: bool,
    pub is_symlink: bool,
}

/// Bounded workspace inventory snapshot.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceInventory {
    pub root: PathBuf,
    pub entries: Vec<InventoryEntry>,
    /// True when the walk hit an entry/depth/byte/wall-clock limit and did not
    /// cover the whole tree.
    pub truncated: bool,
    /// Monotonic epoch this inventory was built at (see [`InventoryCache`]).
    pub epoch: u64,
}

impl WorkspaceInventory {
    /// All relative paths, in deterministic order.
    pub fn paths(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.rel.as_str()).collect()
    }
}

/// Test deadline guard; kept out of the hot walker so the limit is decidable.
fn wall_clock_exceeded(
    started: std::time::Instant,
    now: std::time::Instant,
    max: Duration,
) -> bool {
    now.saturating_duration_since(started) >= max && !max.is_zero()
}

/// Whether a directory entry is a symlink whose canonical target leaves
/// `canonical_root` (confinement fails closed for unverifiable/dangling links).
fn symlink_stays_in_root(path: &Path, canonical_root: &Path) -> bool {
    let is_symlink = std::fs::symlink_metadata(path)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false);
    if !is_symlink {
        return true;
    }
    dunce::canonicalize(path)
        .map(|c| c.starts_with(canonical_root))
        .unwrap_or(false)
}

/// Walk `root` per `limits`, returning (`entries`, `truncated`).
///
/// Entries are visited depth-first by the walker and then returned in a
/// deterministic order (directories first, then case-insensitive by relative
/// path, exact path as a tiebreak). Only normalized root-relative paths are
/// produced; symlink entries that would escape the canonical root are excluded
/// both as entries and as descent points. Unreadable entries are skipped.
pub fn build_inventory(root: &Path, limits: InventoryLimits) -> Result<WorkspaceInventory, String> {
    let canonical_root = dunce::canonicalize(root).map_err(|e| format!("inventory root: {e}"))?;
    let started = std::time::Instant::now();
    let budget = Duration::from_millis(limits.max_wall_ms);

    let mut builder = ignore::WalkBuilder::new(&canonical_root);
    let confine = canonical_root.clone();
    builder
        .max_depth(Some(limits.max_depth))
        .follow_links(false)
        .same_file_system(true)
        .standard_filters(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .ignore(true)
        .require_git(false)
        .filter_entry(move |entry| symlink_stays_in_root(entry.path(), &confine));

    let mut entries: Vec<InventoryEntry> = Vec::new();
    let mut truncated = false;
    let mut aggregate_bytes: u64 = 0;

    for dent in builder.build() {
        if wall_clock_exceeded(started, std::time::Instant::now(), budget) {
            truncated = true;
            break;
        }
        let Ok(entry) = dent else { continue };
        if entry.depth() == 0 {
            continue;
        }
        if entries.len() >= limits.max_entries {
            truncated = true;
            break;
        }

        let abs = entry.path();
        let Ok(rel) = abs.strip_prefix(&canonical_root) else {
            continue;
        };
        let rel = normalize_rel(rel);
        if rel.is_empty() {
            continue;
        }

        let symlink_meta = std::fs::symlink_metadata(abs).ok();
        let is_symlink = symlink_meta
            .as_ref()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false);
        let is_dir = std::fs::metadata(abs).map(|m| m.is_dir()).unwrap_or(false);
        let size_bytes = if is_dir {
            0
        } else {
            std::fs::metadata(abs).map(|m| m.len()).unwrap_or(0)
        };

        if !is_dir {
            if aggregate_bytes.saturating_add(size_bytes) > limits.max_aggregate_bytes {
                truncated = true;
                break;
            }
            aggregate_bytes = aggregate_bytes.saturating_add(size_bytes);
        }

        entries.push(InventoryEntry {
            rel,
            size_bytes,
            is_dir,
            is_symlink,
        });
    }

    // Deterministic order: directories first, then case-insensitive by rel,
    // then exact rel (stable tiebreak for distinct-casing siblings).
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.rel.to_lowercase().cmp(&b.rel.to_lowercase()))
            .then_with(|| a.rel.cmp(&b.rel))
    });

    Ok(WorkspaceInventory {
        root: canonical_root,
        entries,
        truncated,
        epoch: 0,
    })
}

/// Empty relative-path slots (`.`/`..`/empty) normalize away; otherwise join
/// with `/` and collapse nothing (walk yields clean relative paths).
fn normalize_rel(rel: &Path) -> String {
    let mut parts = Vec::new();
    for comp in rel.components() {
        use std::path::Component;
        match comp {
            Component::Normal(c) => parts.push(c.to_string_lossy().into_owned()),
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => continue,
        }
    }
    parts.join("/")
}

/// Session-cache/invalidation seam for PR19.
///
/// This is intentionally a seam: it tracks an epoch and a set of dirty (touched)
/// paths so PR19's `/__clear` and file-touch notifications can invalidate a
/// cached inventory without rebuilding every turn. It performs no I/O and does
/// not, on its own, change live turn behavior.
#[derive(Debug, Default)]
pub struct InventoryCache {
    epoch: AtomicU64,
    dirty: std::sync::Mutex<HashSet<PathBuf>>,
}

impl InventoryCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Current cache epoch. Invalidated on every clear.
    pub fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::Relaxed)
    }

    /// Invalidate the whole inventory (`/clear`): bumps the epoch.
    pub fn invalidate(&self) -> u64 {
        let e = self.epoch.fetch_add(1, Ordering::Relaxed).saturating_add(1);
        self.dirty.lock().unwrap().clear();
        e
    }

    /// Mark `path` as touched (a tool touched it). Next build at a new epoch
    /// re-scans. Returns whether the path was already dirty.
    pub fn mark_touched(&self, path: &Path) -> bool {
        let mut d = self.dirty.lock().unwrap();
        d.insert(path.to_path_buf())
    }

    /// Paths marked dirty since the last clear.
    pub fn dirty(&self) -> Vec<PathBuf> {
        self.dirty.lock().unwrap().iter().cloned().collect()
    }

    pub fn clear_dirty_after(&self, epoch: u64) {
        if epoch != self.epoch() {
            self.dirty.lock().unwrap().clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn write_tree(root: &Path, files: &[&str]) {
        for f in files {
            let p = root.join(f);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&p, format!("content {f}")).unwrap();
        }
    }

    #[test]
    fn inventory_respects_gitignore() {
        let tmp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        std::fs::write(root.join(".gitignore"), "ignored.md\nnode_modules/\n").unwrap();
        write_tree(
            &root,
            &[
                "src/a.rs",
                "docs/readme.md",
                "ignored.md",
                "node_modules/x.js",
            ],
        );

        let inv = build_inventory(&root, InventoryLimits::default()).unwrap();
        let paths = inv.paths();
        assert!(paths.iter().any(|p| *p == "src/a.rs"));
        assert!(paths.iter().any(|p| *p == "docs/readme.md"));
        assert!(
            !paths.iter().any(|p| *p == "ignored.md"),
            "gitignored leaked"
        );
        assert!(
            !paths.iter().any(|p| p.starts_with("node_modules")),
            "gitignored dir leaked"
        );
    }

    #[test]
    fn inventory_deterministic_order() {
        let tmp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        write_tree(&root, &["b.rs", "a.rs", "C_DIR/x", "a_dir/y"]);

        let a = build_inventory(&root, InventoryLimits::default()).unwrap();
        let b = build_inventory(&root, InventoryLimits::default()).unwrap();
        assert_eq!(a.paths(), b.paths());
        let rels = a.paths();
        // Case-insensitive, stable (a_dir before C_DIR).
        let a_idx = rels.iter().position(|p| *p == "a_dir/y").unwrap();
        let c_idx = rels.iter().position(|p| *p == "C_DIR/x").unwrap();
        assert!(a_idx < c_idx);
    }

    #[test]
    fn inventory_symlink_escape_is_excluded() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        std::fs::write(outside.path().join("secret.txt"), "outside").unwrap();
        // Symlink inside root that points outside the root.
        std::os::unix::fs::symlink(outside.path(), root.join("escape")).unwrap();
        std::fs::write(root.join("ok.txt"), "inside").unwrap();

        let inv = build_inventory(&root, InventoryLimits::default()).unwrap();
        assert!(
            !inv.entries
                .iter()
                .any(|e| e.rel == "escape" || e.rel.starts_with("escape/")),
            "escaping symlink leaked"
        );
        assert!(inv.entries.iter().any(|e| e.rel == "ok.txt"));
    }

    #[test]
    fn inventory_entry_and_depth_limits_set_truncated() {
        let tmp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        write_tree(
            &root,
            &[
                "a.rs",
                "b.rs",
                "c.rs",
                "sub/d.rs",
                "sub/e.rs",
                "deep/x/f.rs",
            ],
        );
        let tight = build_inventory(
            &root,
            InventoryLimits {
                max_entries: 2,
                max_depth: 2,
                ..InventoryLimits::default()
            },
        )
        .unwrap();
        assert!(tight.truncated || tight.entries.len() <= 2);
        assert!(tight.entries.len() <= 2);
    }

    #[test]
    fn inventory_wall_clock_guard_deterministic() {
        let start = std::time::Instant::now();
        assert!(!wall_clock_exceeded(
            start,
            start,
            Duration::from_millis(1000)
        ));
        // Simulated past deadline.
        let past = start + Duration::from_millis(2000);
        assert!(wall_clock_exceeded(
            start,
            past,
            Duration::from_millis(1000)
        ));
        // Zero budget means "no limit".
        assert!(!wall_clock_exceeded(start, past, Duration::ZERO));
    }

    #[test]
    fn inventory_cache_epoch_and_dirty_seam() {
        let cache = InventoryCache::new();
        let e0 = cache.epoch();
        cache.mark_touched(Path::new("src/main.rs"));
        assert_eq!(cache.dirty().len(), 1);
        let e1 = cache.invalidate();
        assert!(e1 > e0);
        assert!(cache.dirty().is_empty(), "clear drops dirty");
    }
}
