//! Bounded lazy workspace inventory for prime path scoring (PR18).
//!
//! Roots a bounded scan at the workspace/git root and collects a normalized,
//! deterministic set of relative paths. The walker mirrors the safe workspace
//! walker semantics used by the fs surfaces: it honors ignore rules, never
//! follows an escaping symlink, and applies explicit entry/depth/aggregate-byte/
//! wall-clock limits so a pathological tree cannot balloon memory or latency.
//!
//! **Deterministic under limits:** directory entries are traversed in sorted
//! file-name order, so the *set* of collected paths under truncation is
//! deterministic across runs over the same tree (unlike filesystem readdir
//! order). Hidden entries are included (so `.<grok>/skills/...` can contribute
//! path evidence) while VCS internals (`.git`, `.hg`, `.svn`) are explicitly
//! skipped. Cross-device entries are excluded *and reported* via
//! [`WorkspaceInventory::cross_device`] so an incomplete walk is never silent
//! (device detection is Unix-only; on non-Unix the walker does not perform
//! cross-device detection — documented).
//!
//! PR19 (`/clear`, touched paths) is not wired here — this exposes a
//! race-free session-cache/invalidation seam ([`InventoryCache`]).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Directory entry level below which the walker stops.
const DEFAULT_MAX_DEPTH: usize = 4;
/// Cap on remembered dirty paths before the cache forces a full rebuild.
const CACHE_DIRTY_CAP: usize = 4096;

/// Bounded inventory limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InventoryLimits {
    pub max_depth: usize,
    pub max_entries: usize,
    pub max_aggregate_bytes: u64,
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
    pub rel: String,
    pub size_bytes: u64,
    pub is_dir: bool,
    pub is_symlink: bool,
}

/// Bounded workspace inventory snapshot.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceInventory {
    pub root: PathBuf,
    pub entries: Vec<InventoryEntry>,
    /// Precomputed `(rel_lower, last_segment_lower)` for cheap scoring (no
    /// per-(skill, entry) allocation).
    pub lowered_rels: Vec<(String, String)>,
    /// True when the walk hit an entry/depth/byte/wall-clock limit.
    pub truncated: bool,
    /// True when entries were skipped because they sit on another device.
    pub cross_device: bool,
    pub epoch: u64,
}

impl WorkspaceInventory {
    pub fn paths(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.rel.as_str()).collect()
    }

    /// True when the inventory does not fully cover the tree (limits, other
    /// device, or an error). Path evidence is then treated as incomplete.
    pub fn incomplete(&self) -> bool {
        self.truncated || self.cross_device
    }
}

/// Test deadline guard; kept out of the hot walker so the limit is decidable.
fn wall_clock_exceeded(started: Instant, now: Instant, max: Duration) -> bool {
    now.saturating_duration_since(started) >= max && !max.is_zero()
}

/// Whether a directory entry is a symlink whose canonical target leaves
/// `canonical_root`. Only symlinks are canonicalized (never normal entries).
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

/// Device id of `path`, if determinable (`None` on non-unix or errors).
fn device_of(path: &Path) -> Option<u64> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        std::fs::symlink_metadata(path).ok().map(|m| m.dev())
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}

/// Walk `root` per `limits`, returning a deterministic, bounded inventory.
///
/// Directory entries are visited in sorted file-name order (deterministic set
/// under truncation). Escaping symlinks are excluded and not descended; cross
/// -device entries are excluded and flagged [`cross_device`]. Unreadable entries
/// are skipped.
pub fn build_inventory(root: &Path, limits: InventoryLimits) -> Result<WorkspaceInventory, String> {
    let canonical_root = dunce::canonicalize(root).map_err(|e| format!("inventory root: {e}"))?;
    let started = Instant::now();
    let budget = Duration::from_millis(limits.max_wall_ms);
    let root_dev = device_of(&canonical_root);
    let cross_device = std::sync::Arc::new(AtomicBool::new(false));

    let mut builder = ignore::WalkBuilder::new(&canonical_root);
    let confine = canonical_root.clone();
    let cross = cross_device.clone();
    builder
        .max_depth(Some(limits.max_depth))
        .follow_links(false)
        // Do not rely on the walker to silently drop other mounts: we report it.
        .same_file_system(false)
        .standard_filters(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .ignore(true)
        .require_git(false)
        .sort_by_file_name(|a, b| a.cmp(b))
        .hidden(false) // include dot-dirs (e.g. `.grok/skills`)
        .filter_entry(move |entry| {
            // Skip VCS internals explicitly (cheap regardless of walker behavior)
            // while retaining dot-dirs like `.grok`.
            let name = entry.file_name().to_str().unwrap_or_default();
            if name == ".git" || name == ".hg" || name == ".svn" {
                return false;
            }
            if !symlink_stays_in_root(entry.path(), &confine) {
                return false;
            }
            if let Some(dev) = root_dev {
                #[cfg(unix)]
                if let Some(entry_dev) = device_of(entry.path())
                    && entry_dev != dev
                {
                    cross.store(true, Ordering::Relaxed);
                    return false;
                }
            }
            true
        });

    let mut entries: Vec<InventoryEntry> = Vec::new();
    let mut truncated = false;
    let mut aggregate_bytes: u64 = 0;

    for dent in builder.build() {
        if wall_clock_exceeded(started, Instant::now(), budget) {
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
        let Ok(rel) = entry.path().strip_prefix(&canonical_root) else {
            continue;
        };
        let rel = normalize_rel(rel);
        if rel.is_empty() {
            continue;
        }

        // One lstat per entry; no redundant `metadata` calls for normal files.
        let smeta = std::fs::symlink_metadata(entry.path());
        let Ok(smeta) = smeta else { continue };
        let is_symlink = smeta.file_type().is_symlink();
        let is_dir = smeta.is_dir();
        let size_bytes = if smeta.is_file() { smeta.len() } else { 0 };

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

    // Deterministic order: dirs first, then case-insensitive, then exact.
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.rel.to_lowercase().cmp(&b.rel.to_lowercase()))
            .then_with(|| a.rel.cmp(&b.rel))
    });

    let lowered_rels = entries
        .iter()
        .map(|e| {
            let seg = e.rel.rsplit('/').next().unwrap_or("").to_lowercase();
            (e.rel.to_lowercase(), seg)
        })
        .collect();

    Ok(WorkspaceInventory {
        root: canonical_root,
        entries,
        lowered_rels,
        truncated,
        cross_device: cross_device.load(Ordering::Relaxed),
        epoch: 0,
    })
}

/// Normalize a relative path to `/`-separated, with `.`/`..`/empty removed.
fn normalize_rel(rel: &Path) -> String {
    let mut parts = Vec::new();
    for comp in rel.components() {
        match comp {
            std::path::Component::Normal(c) => parts.push(c.to_string_lossy().into_owned()),
            _ => continue,
        }
    }
    parts.join("/")
}

/// Session-cache/invalidation seam for PR19 (race-free).
///
/// A single mutex guards the epoch and the (path, marked-at-epoch) dirty list, so
/// `invalidate` / `mark_touched` / `clear_dirty_after` are atomic with respect to
/// one another and never lose a mark across an epoch bump. The dirty list is
/// capped; when it overflows the epoch is bumped to force a full rebuild.
#[derive(Debug)]
pub struct InventoryCache {
    state: std::sync::Mutex<CacheState>,
}

#[derive(Debug, Default)]
struct CacheState {
    epoch: u64,
    dirty: Vec<(PathBuf, u64)>,
}

impl Default for InventoryCache {
    fn default() -> Self {
        Self {
            state: std::sync::Mutex::new(CacheState::default()),
        }
    }
}

impl InventoryCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn epoch(&self) -> u64 {
        self.state.lock().unwrap().epoch
    }

    /// Invalidate the whole inventory (`/clear`): bumps the epoch and drops
    /// every dirty mark (all were recorded under an older epoch).
    pub fn invalidate(&self) -> u64 {
        let mut s = self.state.lock().unwrap();
        s.epoch = s.epoch.saturating_add(1);
        s.dirty.clear();
        s.epoch
    }

    /// Mark `path` as touched at the current epoch. Returns true if the cache
    /// was forced to rebuild (dirty set overflow). Never lost across a concurrent
    /// `invalidate`: `invalidate` bumps first under the same lock so any mark
    /// below carries the new epoch.
    pub fn mark_touched(&self, path: &Path) -> bool {
        let mut s = self.state.lock().unwrap();
        if s.dirty.len() >= CACHE_DIRTY_CAP {
            // Bound memory: force a full rebuild.
            s.epoch = s.epoch.saturating_add(1);
            s.dirty.clear();
            let e = s.epoch;
            s.dirty.push((path.to_path_buf(), e));
            return true;
        }
        s.dirty.retain(|(p, _)| p != path);
        let e = s.epoch;
        s.dirty.push((path.to_path_buf(), e));
        false
    }

    /// Paths marked dirty since the last clear.
    pub fn dirty(&self) -> Vec<PathBuf> {
        self.state
            .lock()
            .unwrap()
            .dirty
            .iter()
            .map(|(p, _)| p.clone())
            .collect()
    }

    /// Remove dirty marks recorded at or before `epoch` (they were consumed).
    /// Marks added after `epoch` are retained, so a late mark is never lost.
    pub fn clear_dirty_after(&self, epoch: u64) {
        let mut s = self.state.lock().unwrap();
        s.dirty.retain(|(_, marked)| *marked > epoch);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_tree(root: &Path, files: &[&str]) {
        for f in files {
            let p = root.join(f);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&p, format!("content {f}")).unwrap();
        }
    }

    fn build_default(root: &Path) -> WorkspaceInventory {
        build_inventory(root, InventoryLimits::default()).unwrap()
    }

    #[test]
    fn inventory_respects_gitignore_but_includes_dot_dirs() {
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
        // A skill dir (dot-dir) must appear now that hidden(false) is set.
        write_tree(&root, &[".grok/skills/deploy/SKILL.md"]);

        let inv = build_default(&root);
        let paths = inv.paths();
        assert!(paths.iter().any(|p| *p == "src/a.rs"));
        assert!(paths.iter().any(|p| *p == "docs/readme.md"));
        assert!(
            paths.iter().any(|p| p.starts_with(".grok/skills/deploy")),
            "dot-dir missed"
        );
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

        let a = build_default(&root);
        let b = build_default(&root);
        assert_eq!(a.paths(), b.paths());
        let rels = a.paths();
        let a_idx = rels.iter().position(|p| *p == "a_dir/y").unwrap();
        let c_idx = rels.iter().position(|p| *p == "C_DIR/x").unwrap();
        assert!(a_idx < c_idx);
    }

    #[test]
    fn inventory_deterministic_set_under_tight_entry_limit() {
        let tmp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        write_tree(&root, &["z.rs", "a.rs", "m.rs", "b.rs"]);
        let a = build_inventory(
            &root,
            InventoryLimits {
                max_entries: 2,
                ..InventoryLimits::default()
            },
        )
        .unwrap();
        let b = build_inventory(
            &root,
            InventoryLimits {
                max_entries: 2,
                ..InventoryLimits::default()
            },
        )
        .unwrap();
        assert_eq!(a.entries, b.entries, "truncated set must be deterministic");
        assert!(
            a.truncated,
            "truncated flag must be true under a tight entry limit"
        );
        assert_eq!(a.entries.len(), 2);
    }

    #[test]
    fn inventory_symlink_escape_is_excluded() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        std::fs::write(outside.path().join("secret.txt"), "outside").unwrap();
        std::os::unix::fs::symlink(outside.path(), root.join("escape")).unwrap();
        std::fs::write(root.join("ok.txt"), "inside").unwrap();

        let inv = build_default(&root);
        assert!(
            !inv.entries
                .iter()
                .any(|e| e.rel == "escape" || e.rel.starts_with("escape/")),
            "escaping symlink leaked"
        );
        assert!(inv.entries.iter().any(|e| e.rel == "ok.txt"));
    }

    #[test]
    fn inventory_wall_clock_guard_deterministic() {
        let start = Instant::now();
        assert!(!wall_clock_exceeded(
            start,
            start,
            Duration::from_millis(1000)
        ));
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
    fn inventory_skips_vcs_dirs_but_keeps_grok() {
        let tmp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        // A populated `.git` tree must never appear in the inventory.
        write_tree(
            &root,
            &[".git/objects/ab/abcdef", ".git/HEAD", "src/main.rs"],
        );
        write_tree(
            &root,
            &[
                ".grok/skills/deploy/SKILL.md",
                ".hg/store/00manifest",
                ".svn/entries",
            ],
        );

        let inv = build_default(&root);
        let paths = inv.paths();
        assert!(
            !paths.iter().any(|p| p.starts_with(".git")),
            "`.git` leaked: {paths:?}"
        );
        assert!(
            !paths.iter().any(|p| p.starts_with(".hg")),
            "`.hg` leaked: {paths:?}"
        );
        assert!(
            !paths.iter().any(|p| p.starts_with(".svn")),
            "`.svn` leaked: {paths:?}"
        );
        assert!(
            paths.iter().any(|p| p.starts_with(".grok")),
            "`.grok` must be retained: {paths:?}"
        );
        assert!(paths.iter().any(|p| *p == "src/main.rs"));
    }

    #[test]
    fn device_of_reports_same_device_for_siblings() {
        let tmp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        let file = root.join("a.rs");
        std::fs::write(&file, "x").unwrap();
        match (device_of(&root), device_of(&file)) {
            (Some(a), Some(b)) => assert_eq!(a, b, "siblings must share a device"),
            // Non-Unix: detection disabled (documented).
            _ => {}
        }
    }

    #[test]
    fn inventory_cache_no_lost_marks_across_invalidate() {
        let cache = InventoryCache::new();
        let e0 = cache.epoch();
        cache.mark_touched(Path::new("src/a.rs"));
        assert_eq!(cache.dirty().len(), 1);
        // invalidate bumps the epoch first (under the same lock), so no mark is
        // lost and the dirty set is cleared.
        let e1 = cache.invalidate();
        assert!(e1 > e0);
        assert!(cache.dirty().is_empty());

        // A mark after the invalidate records the new epoch and survives a
        // clear_dirty_after of the *old* epoch without being lost.
        cache.mark_touched(Path::new("src/b.rs"));
        cache.clear_dirty_after(e0);
        assert_eq!(cache.dirty(), vec![PathBuf::from("src/b.rs")]);
        // clear_dirty_after of its own epoch consumes it.
        cache.clear_dirty_after(cache.epoch());
        assert!(cache.dirty().is_empty());
    }

    #[test]
    fn inventory_cache_dirty_cap_forces_rebuild_and_bounds_memory() {
        let cache = InventoryCache::new();
        // Blow past the cap.
        let mut forced = false;
        for i in 0..(CACHE_DIRTY_CAP as u64 + 100) {
            if cache.mark_touched(&PathBuf::from(format!("/p/{i}"))) {
                forced = true;
                break;
            }
        }
        assert!(forced, "overflow must force a rebuild");
        assert!(
            cache.dirty().len() <= CACHE_DIRTY_CAP,
            "dirty set must stay bounded"
        );
    }
}
