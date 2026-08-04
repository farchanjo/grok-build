//! BLAKE3 content-addressed media artifact store (plan section 11).
//!
//! All objects are immutable and addressed by a lowercase hex BLAKE3 digest.
//! Object names and subdirectory names are strictly validated before any path
//! join so a key can never escape `<session_dir>/assets/media/`. Writes use
//! create-new semantics (an existing object is never overwritten) and reads
//! verify the content hash plus schema version.
//!
//! References (`refs/`), pins, access times, and GC metadata are mutable and
//! stored separately from immutable objects. `journal.jsonl` is an append-only
//! lifecycle audit log. GC is conservative: it retains everything referenced
//! by live refs, everything explicitly pinned in the journal, and every
//! object the store ever recorded writing — so objects referenced by
//! append-only updates, forks, or checkpoints can never be collected. It
//! commits the index before unlinking and deletes only files that were never
//! store-written and never referenced or pinned.

use crate::session::media::{
    append_jsonl_line_locked, now_ts, read_jsonl, verify_blake3, write_object_create_new,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};
use xai_grok_tools::media::backend::MediaUnderstandingResult;

/// Relative location of the media artifact store inside a session directory.
pub(crate) const MEDIA_ARTIFACT_DIR: &str = "assets/media";
/// Schema version for `results/*.json` envelopes and `index.json`.
pub(crate) const MEDIA_SCHEMA_VERSION: u32 = 1;
/// BLAKE3 hex digest length (32 bytes -> 64 lowercase hex chars).
pub(crate) const BLAKE3_HEX_LEN: usize = 64;

/// Name of the single accumulating attachment ref for the current
/// conversation lifecycle (plan section 11.3).
///
/// Every source blob and semantic result that enters the current conversation
/// (explicit `analyze_media`, automatic attachment enrichment, compaction
/// delegation) is merged into this ref under `refs/attachments/`. Refs are
/// never removed on rewind or fork, so the ref conservatively over-approximates
/// the live set.
pub(crate) const LIVE_ATTACHMENT_REF: &str = "live";

/// Category of an immutable object in the store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ArtifactKind {
    /// Original source bytes: `objects/blobs/<blake3>`.
    Blob,
    /// Derived bytes (normalized frames, contact sheets, PCM):
    /// `objects/derived/<blake3>`.
    Derived,
    /// Validated semantic result envelope: `objects/results/<blake3>.json`.
    Result,
}

impl ArtifactKind {
    pub(crate) fn subdir(self) -> &'static str {
        match self {
            ArtifactKind::Blob => "blobs",
            ArtifactKind::Derived => "derived",
            ArtifactKind::Result => "results",
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            ArtifactKind::Blob => "blob",
            ArtifactKind::Derived => "derived",
            ArtifactKind::Result => "result",
        }
    }
}

/// Reference namespace for mutable object references.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RefKind {
    /// Objects referenced by the current live conversation / attachments.
    Attachments,
    /// Objects referenced by append-only compaction state.
    Compaction,
    /// Objects referenced by checkpoints.
    Checkpoints,
}

impl RefKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            RefKind::Attachments => "attachments",
            RefKind::Compaction => "compaction",
            RefKind::Checkpoints => "checkpoints",
        }
    }

    fn all() -> [RefKind; 3] {
        [
            RefKind::Attachments,
            RefKind::Compaction,
            RefKind::Checkpoints,
        ]
    }
}

/// A kind-aware reference to an immutable object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ObjectRef {
    pub kind: ArtifactKind,
    pub hash: String,
}

/// One mutable reference entry under `refs/<namespace>/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RefEntry {
    pub name: String,
    pub objects: Vec<ObjectRef>,
}

/// Envelope for a stored semantic result under `objects/results/<key>.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct StoredSemanticResult {
    pub schema_version: u32,
    pub semantic_key: String,
    pub result: MediaUnderstandingResult,
}

/// On-disk `index.json`: schema version, object counts, GC bookkeeping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct MediaIndex {
    pub schema_version: u32,
    pub blob_count: u64,
    pub derived_count: u64,
    pub result_count: u64,
    pub total_objects: u64,
    /// Unix seconds of the last GC run, or `None` if never run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_gc_at: Option<i64>,
    pub updated_at: i64,
}

/// Result of a conservative GC run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GcReport {
    pub removed_objects: usize,
    pub retained_objects: usize,
}

/// Redaction manifest for redacted/text exports (plan section 11.3).
///
/// A redacted/text export omits raw media bytes (blobs and derived objects)
/// and includes only semantics + provenance plus this manifest describing
/// exactly what was omitted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct MediaRedactionManifest {
    pub schema_version: u32,
    pub exported_at: i64,
    /// BLAKE3 hashes of the raw source blobs omitted from the export.
    pub omitted_blob_hashes: Vec<String>,
    /// Keys of derived objects omitted from the export.
    pub omitted_derived_keys: Vec<String>,
    /// Semantic result keys that ARE included in a redacted/text export.
    pub included_result_keys: Vec<String>,
    pub note: String,
}

/// Text-export payload: semantics + provenance for every stored result, with
/// no raw bytes and no instruction/prompt text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct MediaTextExport {
    pub schema_version: u32,
    pub exported_at: i64,
    pub entries: Vec<MediaTextExportEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct MediaTextExportEntry {
    pub semantic_key: String,
    pub source_category: String,
    pub provider: String,
    pub model: String,
    pub strategy: String,
    pub semantics: String,
}

/// Append-only lifecycle event for `journal.jsonl`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub(crate) enum JournalEvent {
    BlobWrite {
        ts: i64,
        hash: String,
    },
    DerivedWrite {
        ts: i64,
        key: String,
    },
    ResultWrite {
        ts: i64,
        key: String,
    },
    RefAdd {
        ts: i64,
        kind: RefKind,
        name: String,
        objects: Vec<ObjectRef>,
    },
    RefRemove {
        ts: i64,
        kind: RefKind,
        name: String,
    },
    Pin {
        ts: i64,
        reason: String,
        objects: Vec<ObjectRef>,
    },
    Rewind {
        ts: i64,
        target_prompt_index: usize,
    },
    Fork {
        ts: i64,
        objects_copied: usize,
    },
    Gc {
        ts: i64,
        removed: Vec<String>,
        retained_count: usize,
    },
}

/// Validate that `key` is exactly a 64-char lowercase hex BLAKE3 digest.
pub(crate) fn validate_blake3_hex(key: &str) -> io::Result<()> {
    // Hex digits `0-9` are not `is_ascii_lowercase`, so the lower-case check
    // must be expressed as "not uppercase" rather than `is_ascii_lowercase`.
    let valid = key.len() == BLAKE3_HEX_LEN
        && key
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase());
    if !valid {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "media artifact key must be a 64-char lowercase hex BLAKE3 digest",
        ));
    }
    Ok(())
}

/// Validate a ref name so it can never escape `refs/<namespace>/`.
fn validate_ref_name(name: &str) -> io::Result<()> {
    let valid = !name.is_empty()
        && name.len() <= 128
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'));
    if !valid {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "media ref name must contain only ASCII letters, digits, '-', '_' or '.'",
        ));
    }
    Ok(())
}

/// The session-local BLAKE3 content-addressed media artifact store.
///
/// `root` is `<session_dir>/assets/media`. The store is cheap to open and can
/// be constructed on demand; all state lives on disk.
#[derive(Debug, Clone)]
pub(crate) struct MediaArtifactStore {
    root: PathBuf,
}

impl MediaArtifactStore {
    /// Open (creating the layout if needed) the store under `session_dir`.
    pub(crate) fn open(session_dir: &Path) -> io::Result<Self> {
        let root = session_dir.join(MEDIA_ARTIFACT_DIR);
        std::fs::create_dir_all(&root)?;
        for sub in [
            "objects/blobs",
            "objects/derived",
            "objects/results",
            "refs/attachments",
            "refs/compaction",
            "refs/checkpoints",
        ] {
            std::fs::create_dir_all(root.join(sub))?;
        }
        Ok(Self { root })
    }

    /// The `assets/media` root of this store.
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    /// Relative location under the session directory (for archive names).
    pub(crate) fn relative_root(&self) -> PathBuf {
        PathBuf::from(MEDIA_ARTIFACT_DIR)
    }

    pub(crate) fn journal_path(&self) -> PathBuf {
        self.root.join("journal.jsonl")
    }

    pub(crate) fn usage_path(&self) -> PathBuf {
        self.root.join("usage.jsonl")
    }

    pub(crate) fn index_path(&self) -> PathBuf {
        self.root.join("index.json")
    }

    fn object_path(&self, kind: ArtifactKind, key: &str) -> io::Result<PathBuf> {
        validate_blake3_hex(key)?;
        let mut path = self.root.join("objects").join(kind.subdir()).join(key);
        if kind == ArtifactKind::Result {
            path = path.with_extension("json");
        }
        Ok(path)
    }

    fn refs_dir(&self, kind: RefKind) -> PathBuf {
        self.root.join("refs").join(kind.as_str())
    }

    // ── Source blobs ──────────────────────────────────────────────────────

    /// Persist immutable source bytes and return their BLAKE3 hex digest.
    /// Idempotent: putting the same bytes twice returns the same digest and
    /// never overwrites the existing object.
    pub(crate) fn put_blob(&self, bytes: &[u8]) -> io::Result<String> {
        let digest = blake3::hash(bytes).to_hex().to_string();
        let path = self.object_path(ArtifactKind::Blob, &digest)?;
        write_object_create_new(&path, bytes)?;
        if let Some(existing) = self.get_blob(&digest)? {
            verify_blake3(&digest, &existing)?;
        }
        self.append_journal(JournalEvent::BlobWrite {
            ts: now_ts(),
            hash: digest.clone(),
        })?;
        Ok(digest)
    }

    /// Read a blob by content address, verifying the content matches the hash.
    pub(crate) fn get_blob(&self, hash: &str) -> io::Result<Option<Vec<u8>>> {
        let path = self.object_path(ArtifactKind::Blob, hash)?;
        if !path.is_file() {
            return Ok(None);
        }
        let bytes = std::fs::read(&path)?;
        verify_blake3(hash, &bytes)?;
        Ok(Some(bytes))
    }

    // ── Derived objects ───────────────────────────────────────────────────

    /// Persist an immutable derived object (keyed by its own BLAKE3 digest).
    pub(crate) fn put_derived(&self, key: &str, bytes: &[u8]) -> io::Result<()> {
        validate_blake3_hex(key)?;
        let path = self.object_path(ArtifactKind::Derived, key)?;
        write_object_create_new(&path, bytes)?;
        if let Some(existing) = self.get_derived(key)? {
            verify_blake3(key, &existing)?;
        }
        self.append_journal(JournalEvent::DerivedWrite {
            ts: now_ts(),
            key: key.to_string(),
        })
    }

    pub(crate) fn get_derived(&self, key: &str) -> io::Result<Option<Vec<u8>>> {
        let path = self.object_path(ArtifactKind::Derived, key)?;
        if !path.is_file() {
            return Ok(None);
        }
        let bytes = std::fs::read(&path)?;
        verify_blake3(key, &bytes)?;
        Ok(Some(bytes))
    }

    // ── Semantic results ──────────────────────────────────────────────────

    /// Persist a validated semantic result under `objects/results/<key>.json`.
    /// Create-new semantics: an existing object path is never overwritten.
    pub(crate) fn put_result(
        &self,
        key: &str,
        result: &MediaUnderstandingResult,
    ) -> io::Result<()> {
        validate_blake3_hex(key)?;
        let stored = StoredSemanticResult {
            schema_version: MEDIA_SCHEMA_VERSION,
            semantic_key: key.to_string(),
            result: result.clone(),
        };
        let bytes = serde_json::to_vec_pretty(&stored)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let path = self.object_path(ArtifactKind::Result, key)?;
        write_object_create_new(&path, &bytes)?;
        self.append_journal(JournalEvent::ResultWrite {
            ts: now_ts(),
            key: key.to_string(),
        })
    }

    /// Read a stored semantic result, verifying schema version and that the
    /// stored semantic key matches the lookup key.
    ///
    /// Result objects are JSON envelopes (not raw bytes), so the content-
    /// addressed BLAKE3 verification applies to blobs/derived objects; for
    /// results the integrity checks are the schema version and the embedded
    /// `semantic_key`, which must equal the lookup key.
    pub(crate) fn get_result(&self, key: &str) -> io::Result<Option<StoredSemanticResult>> {
        let path = self.object_path(ArtifactKind::Result, key)?;
        if !path.is_file() {
            return Ok(None);
        }
        let bytes = std::fs::read(&path)?;
        let stored: StoredSemanticResult = serde_json::from_slice(&bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        if stored.schema_version != MEDIA_SCHEMA_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "unsupported media result schema version {}, expected {}",
                    stored.schema_version, MEDIA_SCHEMA_VERSION
                ),
            ));
        }
        if stored.semantic_key != key {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "stored semantic key does not match lookup key",
            ));
        }
        Ok(Some(stored))
    }

    // ── References ────────────────────────────────────────────────────────

    /// Add (or replace) a named reference entry under `refs/<kind>/`.
    pub(crate) fn add_ref(
        &self,
        kind: RefKind,
        name: &str,
        objects: &[ObjectRef],
    ) -> io::Result<()> {
        validate_ref_name(name)?;
        for object in objects {
            validate_blake3_hex(&object.hash)?;
        }
        let entry = RefEntry {
            name: name.to_string(),
            objects: objects.to_vec(),
        };
        let bytes = serde_json::to_vec_pretty(&entry)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        crate::session::storage::write_bytes_atomic(
            &self.refs_dir(kind).join(format!("{name}.json")),
            &bytes,
        )?;
        self.append_journal(JournalEvent::RefAdd {
            ts: now_ts(),
            kind,
            name: name.to_string(),
            objects: objects.to_vec(),
        })
    }

    /// Remove a named reference. Returns whether a reference was removed.
    pub(crate) fn remove_ref(&self, kind: RefKind, name: &str) -> io::Result<bool> {
        validate_ref_name(name)?;
        let path = self.refs_dir(kind).join(format!("{name}.json"));
        if !path.is_file() {
            return Ok(false);
        }
        std::fs::remove_file(&path)?;
        self.append_journal(JournalEvent::RefRemove {
            ts: now_ts(),
            kind,
            name: name.to_string(),
        })?;
        Ok(true)
    }

    /// Merge objects into a named reference entry under `refs/<kind>/`.
    ///
    /// Unlike [`Self::add_ref`], which replaces the entry, this unions the
    /// given objects with any existing entry so lifecycle stages
    /// (conversation, compaction, checkpoints) can accumulate references
    /// idempotently. A merge that changes nothing is a no-op: no journal
    /// event is appended and no file is rewritten.
    pub(crate) fn merge_ref(
        &self,
        kind: RefKind,
        name: &str,
        objects: &[ObjectRef],
    ) -> io::Result<()> {
        validate_ref_name(name)?;
        if objects.is_empty() {
            return Ok(());
        }
        for object in objects {
            validate_blake3_hex(&object.hash)?;
        }
        let existing: Vec<ObjectRef> = self
            .list_refs(kind)?
            .into_iter()
            .find(|entry| entry.name == name)
            .map(|entry| entry.objects)
            .unwrap_or_default();
        let mut merged = existing;
        let mut changed = false;
        for object in objects {
            if !merged.contains(object) {
                merged.push(object.clone());
                changed = true;
            }
        }
        if !changed {
            return Ok(());
        }
        self.add_ref(kind, name, &merged)
    }

    /// List all reference entries in one namespace.
    pub(crate) fn list_refs(&self, kind: RefKind) -> io::Result<Vec<RefEntry>> {
        let dir = self.refs_dir(kind);
        let mut out = Vec::new();
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return Ok(out);
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension() != Some("json".as_ref()) {
                continue;
            }
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            if let Ok(entry) = serde_json::from_slice::<RefEntry>(&bytes) {
                out.push(entry);
            }
        }
        Ok(out)
    }

    /// All object references across every namespace (the GC live set).
    pub(crate) fn all_referenced_objects(&self) -> io::Result<BTreeSet<(ArtifactKind, String)>> {
        let mut set = BTreeSet::new();
        for kind in RefKind::all() {
            for entry in self.list_refs(kind)? {
                for object in entry.objects {
                    set.insert((object.kind, object.hash));
                }
            }
        }
        Ok(set)
    }

    // ── Pins and lifecycle ────────────────────────────────────────────────

    /// Pin objects so conservative GC retains them (e.g. export pins, fork
    /// pins, live-history pins).
    pub(crate) fn pin(&self, reason: &str, objects: &[ObjectRef]) -> io::Result<()> {
        for object in objects {
            validate_blake3_hex(&object.hash)?;
        }
        self.append_journal(JournalEvent::Pin {
            ts: now_ts(),
            reason: reason.to_string(),
            objects: objects.to_vec(),
        })
    }

    /// Pin every currently referenced object under a checkpoint-named ref
    /// (plan section 11.3).
    ///
    /// Compaction checkpoints replay the compacted history, so the checkpoint
    /// lifecycle retains everything live at commit time. Merges into
    /// `refs/checkpoints/checkpoint-<id>.json`; safe to call once per
    /// committed checkpoint.
    pub(crate) fn pin_checkpoint(&self, checkpoint_id: &str) -> io::Result<()> {
        validate_ref_name(checkpoint_id)?;
        let objects: Vec<ObjectRef> = self
            .all_referenced_objects()?
            .into_iter()
            .map(|(kind, hash)| ObjectRef { kind, hash })
            .collect();
        self.merge_ref(
            RefKind::Checkpoints,
            &format!("checkpoint-{checkpoint_id}"),
            &objects,
        )
    }

    /// Append one lifecycle event to the append-only journal.
    pub(crate) fn append_journal(&self, event: JournalEvent) -> io::Result<()> {
        let mut line = serde_json::to_vec(&event)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        line.push(b'\n');
        append_jsonl_line_locked(&self.journal_path(), line)
    }

    pub(crate) fn read_journal(&self) -> io::Result<Vec<JournalEvent>> {
        read_jsonl(&self.journal_path())
    }

    // ── Index ─────────────────────────────────────────────────────────────

    /// Atomically write `index.json`.
    pub(crate) fn write_index(&self, index: &MediaIndex) -> io::Result<()> {
        let bytes = serde_json::to_vec_pretty(index)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        crate::session::storage::write_bytes_atomic(&self.index_path(), &bytes)
    }

    pub(crate) fn read_index(&self) -> io::Result<Option<MediaIndex>> {
        let path = self.index_path();
        if !path.is_file() {
            return Ok(None);
        }
        let bytes = std::fs::read(&path)?;
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    /// Rebuild and atomically commit `index.json` from the current on-disk
    /// object set (plan section 11.3: refs/index are committed before GC).
    ///
    /// Unlike a fresh [`Self::build_index`], the previous `last_gc_at` stamp
    /// (if any) is preserved, so a mid-lifecycle refresh never erases GC
    /// bookkeeping.
    pub(crate) fn refresh_index(&self) -> io::Result<()> {
        let last_gc_at = self.read_index()?.and_then(|index| index.last_gc_at);
        let mut index = self.build_index()?;
        index.last_gc_at = last_gc_at;
        self.write_index(&index)
    }

    fn build_index(&self) -> io::Result<MediaIndex> {
        let mut blob_count = 0u64;
        let mut derived_count = 0u64;
        let mut result_count = 0u64;
        for (kind, _) in self.list_object_hashes()? {
            match kind {
                ArtifactKind::Blob => blob_count += 1,
                ArtifactKind::Derived => derived_count += 1,
                ArtifactKind::Result => result_count += 1,
            }
        }
        Ok(MediaIndex {
            schema_version: MEDIA_SCHEMA_VERSION,
            blob_count,
            derived_count,
            result_count,
            total_objects: blob_count + derived_count + result_count,
            last_gc_at: None,
            updated_at: now_ts(),
        })
    }

    /// List `(kind, hash)` for every object present on disk, skipping
    /// unexpected or malformed entries defensively.
    fn list_object_hashes(&self) -> io::Result<Vec<(ArtifactKind, String)>> {
        let mut out = Vec::new();
        for kind in [
            ArtifactKind::Blob,
            ArtifactKind::Derived,
            ArtifactKind::Result,
        ] {
            let dir = self.root.join("objects").join(kind.subdir());
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                let hash = if kind == ArtifactKind::Result {
                    name.strip_suffix(".json").unwrap_or(name)
                } else {
                    name
                };
                if validate_blake3_hex(hash).is_err() {
                    continue;
                }
                out.push((kind, hash.to_string()));
            }
        }
        Ok(out)
    }

    // ── Conservative GC ───────────────────────────────────────────────────

    /// Conservative garbage collection (plan section 11.3).
    ///
    /// The retain set is the union of:
    ///
    /// 1. every object referenced by a current ref across all namespaces,
    /// 2. every object explicitly pinned in the append-only journal, and
    /// 3. every object the store ever recorded writing (`BlobWrite` /
    ///    `DerivedWrite` / `ResultWrite` journal events).
    ///
    /// Rule 3 is the decisive conservative invariant: the store never deletes
    /// an object it recorded writing, so objects referenced by append-only
    /// updates, forks, or checkpoints that predate ref wiring can never be
    /// collected. Only files that were never store-written and never
    /// referenced or pinned (stray files) are unlinked.
    ///
    /// Commits index state before unlinking and deletes only unreferenced
    /// objects. Never deletes during active use: callers must not run GC while
    /// a delegate is writing objects. Production invokes this only at a safe
    /// session save/close boundary via [`Self::housekeep_at_close`].
    pub(crate) fn run_gc(&self) -> io::Result<GcReport> {
        // Retain set: current refs across all namespaces plus journal pins
        // plus journal write events.
        let mut retained: BTreeSet<(ArtifactKind, String)> = self.all_referenced_objects()?;
        for event in self.read_journal()? {
            match event {
                JournalEvent::Pin { objects, .. } => {
                    for object in objects {
                        retained.insert((object.kind, object.hash));
                    }
                }
                JournalEvent::BlobWrite { hash, .. } => {
                    retained.insert((ArtifactKind::Blob, hash));
                }
                JournalEvent::DerivedWrite { key, .. } => {
                    retained.insert((ArtifactKind::Derived, key));
                }
                JournalEvent::ResultWrite { key, .. } => {
                    retained.insert((ArtifactKind::Result, key));
                }
                _ => {}
            }
        }

        let objects = self.list_object_hashes()?;
        let mut removed = Vec::new();
        for object in &objects {
            if !retained.contains(object) {
                removed.push(object.clone());
            }
        }

        // Ordering discipline (plan 11.3): commit index/ref state before any
        // unlink. Ref mutations already commit their files atomically; we
        // additionally commit a pre-GC index so a crash between index write
        // and unlink leaves a committed index that still describes the
        // surviving pre-GC objects. After unlinking we commit the final index
        // describing the surviving objects and journal the run.
        let pre_index = self.build_index()?;
        self.write_index(&pre_index)?;

        let retained_count = objects.len() - removed.len();
        for (kind, hash) in &removed {
            let path = self.object_path(*kind, hash)?;
            let _ = std::fs::remove_file(&path);
        }

        let mut final_index = self.build_index()?;
        final_index.last_gc_at = Some(now_ts());
        self.write_index(&final_index)?;
        self.append_journal(JournalEvent::Gc {
            ts: now_ts(),
            removed: removed
                .iter()
                .map(|(kind, hash)| format!("{}/{}", kind.as_str(), hash))
                .collect(),
            retained_count,
        })?;

        Ok(GcReport {
            removed_objects: removed.len(),
            retained_objects: retained_count,
        })
    }

    /// Session save/close boundary housekeeping (plan section 11.3).
    ///
    /// Commits refs/index state (a durable snapshot of the live set), then
    /// runs conservative GC. Only ever called at a safe session save/close
    /// boundary — after every media write and ref update has been persisted —
    /// never while a delegate is writing objects.
    pub(crate) fn housekeep_at_close(&self) -> io::Result<GcReport> {
        self.refresh_index()?;
        self.run_gc()
    }

    // ── Redacted/text export support ───────────────────────────────────────

    /// Build the redaction manifest a redacted/text export uses to describe
    /// which raw media objects were omitted. Raw blobs and derived objects are
    /// always omitted from redacted exports; semantic results are kept.
    pub(crate) fn redaction_manifest(&self) -> io::Result<MediaRedactionManifest> {
        let mut omitted_blob_hashes = Vec::new();
        let mut omitted_derived_keys = Vec::new();
        let mut included_result_keys = Vec::new();
        for (kind, hash) in self.list_object_hashes()? {
            match kind {
                ArtifactKind::Blob => omitted_blob_hashes.push(hash),
                ArtifactKind::Derived => omitted_derived_keys.push(hash),
                ArtifactKind::Result => included_result_keys.push(hash),
            }
        }
        Ok(MediaRedactionManifest {
            schema_version: MEDIA_SCHEMA_VERSION,
            exported_at: now_ts(),
            omitted_blob_hashes,
            omitted_derived_keys,
            included_result_keys,
            note: "raw media bytes are omitted from redacted/text exports; \
                   semantics and provenance are included instead"
                .to_string(),
        })
    }

    /// Build the text-export payload: semantics + provenance for every stored
    /// result. Never includes raw bytes, instructions, prompts, or credentials.
    pub(crate) fn text_export(&self) -> io::Result<MediaTextExport> {
        let mut entries = Vec::new();
        for (kind, key) in self.list_object_hashes()? {
            if kind != ArtifactKind::Result {
                continue;
            }
            let Ok(Some(stored)) = self.get_result(&key) else {
                continue;
            };
            for semantics in &stored.result.results {
                entries.push(MediaTextExportEntry {
                    semantic_key: key.clone(),
                    source_category: media_category_str(semantics.category),
                    provider: semantics.provenance.provider.clone(),
                    model: semantics.provenance.model.clone(),
                    strategy: media_strategy_str(semantics.provenance.strategy),
                    semantics: semantics.text.clone(),
                });
            }
        }
        Ok(MediaTextExport {
            schema_version: MEDIA_SCHEMA_VERSION,
            exported_at: now_ts(),
            entries,
        })
    }
}

/// Copy the source session's `assets/media/` store into `dst_session_dir`
/// (fork/replay lifecycle, plan section 11.3).
///
/// Immutable object files are hard-linked when possible (same inode, cheap,
/// and safe: child GC can only unlink its own directory entry) and copied
/// otherwise (e.g. `EXDEV` across devices). Refs and `index.json` are copied
/// verbatim. The child gets fresh, empty `journal.jsonl`/`usage.jsonl`
/// journals, seeded with a `Fork` journal marker. Returns the number of object
/// files placed in the target.
pub(crate) fn copy_media_store(
    src_session_dir: &Path,
    dst_session_dir: &Path,
) -> io::Result<usize> {
    let src_root = src_session_dir.join(MEDIA_ARTIFACT_DIR);
    if !src_root.is_dir() {
        return Ok(0);
    }
    let dst_store = MediaArtifactStore::open(dst_session_dir)?;
    let dst_root = dst_store.root().to_path_buf();

    let mut copied = 0usize;
    for kind in [
        ArtifactKind::Blob,
        ArtifactKind::Derived,
        ArtifactKind::Result,
    ] {
        let src_dir = src_root.join("objects").join(kind.subdir());
        let dst_dir = dst_root.join("objects").join(kind.subdir());
        std::fs::create_dir_all(&dst_dir)?;
        let Ok(entries) = std::fs::read_dir(&src_dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let dst_path = dst_dir.join(entry.file_name());
            if hardlink_or_copy(&path, &dst_path)? {
                copied += 1;
            }
        }
    }

    // Refs are small mutable metadata; never hard-linked, always copied.
    for kind in RefKind::all() {
        let src_dir = src_root.join("refs").join(kind.as_str());
        let dst_dir = dst_root.join("refs").join(kind.as_str());
        std::fs::create_dir_all(&dst_dir)?;
        let Ok(entries) = std::fs::read_dir(&src_dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            std::fs::copy(&path, dst_dir.join(entry.file_name()))?;
        }
    }

    // index.json rides along best-effort.
    let src_index = src_root.join("index.json");
    if src_index.is_file() {
        let _ = std::fs::copy(&src_index, dst_root.join("index.json"));
    }

    // Seed the child-local journal with a Fork marker.
    let mut line = serde_json::to_vec(&JournalEvent::Fork {
        ts: now_ts(),
        objects_copied: copied,
    })
    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    line.push(b'\n');
    append_jsonl_line_locked(&dst_store.journal_path(), line)?;

    Ok(copied)
}

/// Hard-link `src` to `dst` when possible; fall back to a full copy on
/// `EXDEV`, permissions, or other filesystem-level failures. Returns whether
/// a file was newly placed at `dst` (i.e. it was not already present).
fn hardlink_or_copy(src: &Path, dst: &Path) -> io::Result<bool> {
    match std::fs::hard_link(src, dst) {
        Ok(()) => {
            crate::session::storage::sync_parent_directory(dst)?;
            Ok(true)
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(_) => {
            // Cross-device (EXDEV), unsupported, or permission failures fall
            // back to a plain copy of the immutable object.
            std::fs::copy(src, dst)?;
            crate::session::storage::sync_parent_directory(dst)?;
            Ok(true)
        }
    }
}

/// Stable snake_case label for a media category (no `Display` impl upstream).
fn media_category_str(category: xai_grok_tools::media::domain::MediaCategory) -> String {
    use xai_grok_tools::media::domain::MediaCategory as C;
    match category {
        C::Auto => "auto".to_string(),
        C::Image => "image".to_string(),
        C::Audio => "audio".to_string(),
        C::Video => "video".to_string(),
    }
}

/// Stable snake_case label for a delegate strategy.
fn media_strategy_str(strategy: xai_grok_tools::media::domain::MediaCategoryStrategy) -> String {
    use xai_grok_tools::media::domain::MediaCategoryStrategy as S;
    match strategy {
        S::Auto => "auto".to_string(),
        S::Native => "native".to_string(),
        S::Transcription => "transcription".to_string(),
        S::Frames => "frames".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xai_grok_tools::media::backend::{MediaProvenance, MediaSemantics};
    use xai_grok_tools::media::domain::{MediaCategory, MediaCategoryStrategy};

    fn sample_result(category: MediaCategory, text: &str) -> MediaUnderstandingResult {
        MediaUnderstandingResult {
            results: vec![MediaSemantics {
                source: xai_grok_tools::media::domain::MediaSource::Path {
                    path: "assets/sample.png".to_string(),
                },
                category,
                text: text.to_string(),
                provenance: MediaProvenance {
                    provider: "xai".to_string(),
                    model: "grok-4.5".to_string(),
                    strategy: MediaCategoryStrategy::Native,
                },
            }],
            attempts: vec![],
        }
    }

    fn blob_hash(bytes: &[u8]) -> String {
        blake3::hash(bytes).to_hex().to_string()
    }

    #[test]
    fn media_artifacts_blake3_immutability() {
        let dir = tempfile::tempdir().unwrap();
        let store = MediaArtifactStore::open(dir.path()).unwrap();

        let hash1 = store.put_blob(b"hello media").unwrap();
        assert_eq!(hash1, blob_hash(b"hello media"));
        assert_eq!(hash1.len(), BLAKE3_HEX_LEN);

        // Same bytes -> same content address, idempotent.
        let hash2 = store.put_blob(b"hello media").unwrap();
        assert_eq!(hash1, hash2);
        assert_eq!(store.get_blob(&hash1).unwrap().unwrap(), b"hello media");

        // Different bytes -> different address.
        let hash3 = store.put_blob(b"hello media!").unwrap();
        assert_ne!(hash1, hash3);
        assert_eq!(store.get_blob(&hash3).unwrap().unwrap(), b"hello media!");
        assert!(store.get_blob(&hash1).unwrap().is_some());
    }

    #[test]
    fn media_artifacts_read_verifies_hash_and_schema() {
        let dir = tempfile::tempdir().unwrap();
        let store = MediaArtifactStore::open(dir.path()).unwrap();

        let key = blob_hash(b"some bytes");
        store.put_derived(&key, b"some bytes").unwrap();
        assert_eq!(store.get_derived(&key).unwrap().unwrap(), b"some bytes");

        // Tamper with the object: read must fail, not return corrupt data.
        let path = store.object_path(ArtifactKind::Derived, &key).unwrap();
        std::fs::write(&path, b"corrupted").unwrap();
        assert!(store.get_derived(&key).is_err());
    }

    #[test]
    fn media_artifacts_rejects_escaped_and_invalid_keys() {
        let dir = tempfile::tempdir().unwrap();
        let store = MediaArtifactStore::open(dir.path()).unwrap();

        for bad in [
            "../escape",
            "..",
            "abc",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789ABCDEF",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef!",
        ] {
            assert!(
                store.object_path(ArtifactKind::Blob, bad).is_err(),
                "key {bad:?} must be rejected"
            );
        }

        // Valid 64-char lowercase hex passes.
        let ok = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        assert!(store.object_path(ArtifactKind::Blob, ok).is_ok());
    }

    #[test]
    fn media_artifacts_create_new_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let store = MediaArtifactStore::open(dir.path()).unwrap();

        let key = blob_hash(b"result-key-bytes");
        store
            .put_result(
                &key,
                &sample_result(MediaCategory::Image, "first semantics"),
            )
            .unwrap();
        // Same key, different payload: create-new semantics must keep the first.
        store
            .put_result(
                &key,
                &sample_result(MediaCategory::Image, "second semantics"),
            )
            .unwrap();

        let stored = store.get_result(&key).unwrap().unwrap();
        assert_eq!(stored.result.results[0].text, "first semantics");
    }

    #[test]
    fn media_artifacts_gc_retains_journal_written_objects() {
        let dir = tempfile::tempdir().unwrap();
        let store = MediaArtifactStore::open(dir.path()).unwrap();

        let kept = store.put_blob(b"referenced blob").unwrap();
        let pinned = store.put_blob(b"pinned blob").unwrap();
        let plain = store.put_blob(b"unreferenced blob").unwrap();

        store
            .add_ref(
                RefKind::Attachments,
                "live-attachment",
                &[ObjectRef {
                    kind: ArtifactKind::Blob,
                    hash: kept.clone(),
                }],
            )
            .unwrap();
        store
            .pin(
                "export",
                &[ObjectRef {
                    kind: ArtifactKind::Blob,
                    hash: pinned.clone(),
                }],
            )
            .unwrap();

        // Conservative GC (plan 11.3): the retain set is refs + pins +
        // journal write events. Every store-written blob has a `BlobWrite`
        // journal event, so nothing written through the store can ever be
        // collected — including objects that predate ref wiring.
        let report = store.run_gc().unwrap();
        assert_eq!(report.removed_objects, 0);
        assert_eq!(report.retained_objects, 3);

        assert!(store.get_blob(&kept).unwrap().is_some());
        assert!(store.get_blob(&pinned).unwrap().is_some());
        assert!(store.get_blob(&plain).unwrap().is_some());

        // The committed index reflects the post-GC state and the run stamp.
        let index = store.read_index().unwrap().unwrap();
        assert_eq!(index.blob_count, 3);
        assert!(index.last_gc_at.is_some());
    }

    #[test]
    fn media_artifacts_gc_removes_only_unrecorded_stray_files() {
        let dir = tempfile::tempdir().unwrap();
        let store = MediaArtifactStore::open(dir.path()).unwrap();

        let store_blob = store.put_blob(b"recorded blob").unwrap();

        // A file placed directly in the object directory with a valid BLAKE3
        // name but never written through the store: the only deletable class.
        let stray = blob_hash(b"stray bytes never store-written");
        std::fs::write(
            store.object_path(ArtifactKind::Blob, &stray).unwrap(),
            b"stray bytes never store-written",
        )
        .unwrap();

        let report = store.run_gc().unwrap();
        assert_eq!(report.removed_objects, 1);
        assert_eq!(report.retained_objects, 1);

        assert!(store.get_blob(&store_blob).unwrap().is_some());
        assert!(store.get_blob(&stray).unwrap().is_none());

        let index = store.read_index().unwrap().unwrap();
        assert_eq!(index.blob_count, 1);
        assert!(index.last_gc_at.is_some());
    }

    #[test]
    fn media_artifacts_merge_ref_unions_idempotently() {
        let dir = tempfile::tempdir().unwrap();
        let store = MediaArtifactStore::open(dir.path()).unwrap();

        let blob_a = store.put_blob(b"merge a").unwrap();
        let blob_b = store.put_blob(b"merge b").unwrap();

        store
            .merge_ref(
                RefKind::Attachments,
                LIVE_ATTACHMENT_REF,
                &[ObjectRef {
                    kind: ArtifactKind::Blob,
                    hash: blob_a.clone(),
                }],
            )
            .unwrap();
        store
            .merge_ref(
                RefKind::Attachments,
                LIVE_ATTACHMENT_REF,
                &[ObjectRef {
                    kind: ArtifactKind::Blob,
                    hash: blob_b.clone(),
                }],
            )
            .unwrap();
        // Idempotent re-merge: same objects, no growth.
        store
            .merge_ref(
                RefKind::Attachments,
                LIVE_ATTACHMENT_REF,
                &[ObjectRef {
                    kind: ArtifactKind::Blob,
                    hash: blob_a.clone(),
                }],
            )
            .unwrap();

        let refs = store.list_refs(RefKind::Attachments).unwrap();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, LIVE_ATTACHMENT_REF);
        assert_eq!(refs[0].objects.len(), 2);
        assert!(refs[0].objects.contains(&ObjectRef {
            kind: ArtifactKind::Blob,
            hash: blob_a,
        }));
        assert!(refs[0].objects.contains(&ObjectRef {
            kind: ArtifactKind::Blob,
            hash: blob_b,
        }));
    }

    #[test]
    fn media_artifacts_rewind_preserves_refs_and_objects() {
        let dir = tempfile::tempdir().unwrap();
        let store = MediaArtifactStore::open(dir.path()).unwrap();

        let hash = store.put_blob(b"timeline media").unwrap();
        store
            .merge_ref(
                RefKind::Attachments,
                LIVE_ATTACHMENT_REF,
                &[ObjectRef {
                    kind: ArtifactKind::Blob,
                    hash: hash.clone(),
                }],
            )
            .unwrap();

        // Rewind must never delete objects or drop refs (plan 11.3): it only
        // appends journal state.
        store
            .append_journal(JournalEvent::Rewind {
                ts: now_ts(),
                target_prompt_index: 2,
            })
            .unwrap();

        assert!(store.get_blob(&hash).unwrap().is_some());
        let refs = store.list_refs(RefKind::Attachments).unwrap();
        assert_eq!(refs.len(), 1);
        assert!(refs[0].objects.contains(&ObjectRef {
            kind: ArtifactKind::Blob,
            hash,
        }));

        // Journal ordering is preserved: write event before rewind event.
        let events = store.read_journal().unwrap();
        let write_pos = events
            .iter()
            .position(|e| matches!(e, JournalEvent::BlobWrite { .. }))
            .unwrap();
        let rewind_pos = events
            .iter()
            .position(|e| matches!(e, JournalEvent::Rewind { .. }))
            .unwrap();
        assert!(write_pos < rewind_pos);
    }

    #[test]
    fn media_artifacts_replay_reopen_preserves_refs_and_objects() {
        let dir = tempfile::tempdir().unwrap();
        {
            let store = MediaArtifactStore::open(dir.path()).unwrap();
            let hash = store.put_blob(b"replay media").unwrap();
            store
                .merge_ref(
                    RefKind::Attachments,
                    LIVE_ATTACHMENT_REF,
                    &[ObjectRef {
                        kind: ArtifactKind::Blob,
                        hash: hash.clone(),
                    }],
                )
                .unwrap();
            store.refresh_index().unwrap();
        }
        // Replay reopens the SAME session directory: refs, objects, and the
        // committed index must all survive on disk.
        let store = MediaArtifactStore::open(dir.path()).unwrap();
        let restored = store
            .get_blob(&blob_hash(b"replay media"))
            .unwrap()
            .unwrap();
        assert_eq!(restored, b"replay media");
        let refs = store.list_refs(RefKind::Attachments).unwrap();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, LIVE_ATTACHMENT_REF);
        let index = store
            .read_index()
            .unwrap()
            .expect("index committed at close");
        assert_eq!(index.blob_count, 1);
    }

    #[test]
    fn media_artifacts_housekeep_at_close_commits_index_then_gc() {
        let dir = tempfile::tempdir().unwrap();
        let store = MediaArtifactStore::open(dir.path()).unwrap();

        let hash = store.put_blob(b"close media").unwrap();
        store
            .merge_ref(
                RefKind::Attachments,
                LIVE_ATTACHMENT_REF,
                &[ObjectRef {
                    kind: ArtifactKind::Blob,
                    hash: hash.clone(),
                }],
            )
            .unwrap();

        // The session save/close boundary commits refs/index state, then runs
        // conservative GC (which never collects store-written objects).
        let report = store.housekeep_at_close().unwrap();
        assert_eq!(report.removed_objects, 0);
        assert!(store.get_blob(&hash).unwrap().is_some());

        let index = store.read_index().unwrap().unwrap();
        assert_eq!(index.blob_count, 1);
        assert!(index.last_gc_at.is_some());
    }

    #[test]
    fn media_artifacts_pin_checkpoint_captures_live_objects() {
        let dir = tempfile::tempdir().unwrap();
        let store = MediaArtifactStore::open(dir.path()).unwrap();

        let blob = store.put_blob(b"checkpoint media").unwrap();
        let result_key = blob_hash(b"checkpoint result");
        store
            .put_result(
                &result_key,
                &sample_result(MediaCategory::Image, "checkpoint semantics"),
            )
            .unwrap();
        store
            .merge_ref(
                RefKind::Attachments,
                LIVE_ATTACHMENT_REF,
                &[
                    ObjectRef {
                        kind: ArtifactKind::Blob,
                        hash: blob.clone(),
                    },
                    ObjectRef {
                        kind: ArtifactKind::Result,
                        hash: result_key.clone(),
                    },
                ],
            )
            .unwrap();

        // A committed compaction checkpoint pins everything live at commit
        // time under refs/checkpoints/checkpoint-<id>.json.
        store.pin_checkpoint("019f-test-checkpoint-1").unwrap();
        let checkpoints = store.list_refs(RefKind::Checkpoints).unwrap();
        assert_eq!(checkpoints.len(), 1);
        assert_eq!(checkpoints[0].name, "checkpoint-019f-test-checkpoint-1");
        assert!(checkpoints[0].objects.contains(&ObjectRef {
            kind: ArtifactKind::Blob,
            hash: blob,
        }));
        assert!(checkpoints[0].objects.contains(&ObjectRef {
            kind: ArtifactKind::Result,
            hash: result_key,
        }));
    }

    #[test]
    fn media_artifacts_rewind_never_deletes_and_preserves_journal_order() {
        let dir = tempfile::tempdir().unwrap();
        let store = MediaArtifactStore::open(dir.path()).unwrap();

        let hash = store.put_blob(b"timeline media").unwrap();
        store
            .append_journal(JournalEvent::Rewind {
                ts: now_ts(),
                target_prompt_index: 2,
            })
            .unwrap();

        // Rewind must not delete objects.
        assert!(store.get_blob(&hash).unwrap().is_some());

        // Journal ordering is preserved: write event before rewind event.
        let events = store.read_journal().unwrap();
        let write_pos = events
            .iter()
            .position(|e| matches!(e, JournalEvent::BlobWrite { .. }))
            .unwrap();
        let rewind_pos = events
            .iter()
            .position(|e| matches!(e, JournalEvent::Rewind { .. }))
            .unwrap();
        assert!(write_pos < rewind_pos);
    }

    #[test]
    fn media_artifacts_redaction_manifest_and_text_export() {
        let dir = tempfile::tempdir().unwrap();
        let store = MediaArtifactStore::open(dir.path()).unwrap();

        store.put_blob(b"raw image bytes").unwrap();
        let derived_bytes = b"derived for export";
        store
            .put_derived(&blob_hash(derived_bytes), derived_bytes)
            .unwrap();
        let key = blob_hash(b"semantic key bytes");
        store
            .put_result(
                &key,
                &sample_result(MediaCategory::Image, "visible semantics"),
            )
            .unwrap();

        let manifest = store.redaction_manifest().unwrap();
        assert_eq!(manifest.omitted_blob_hashes.len(), 1);
        assert_eq!(manifest.omitted_derived_keys.len(), 1);
        assert_eq!(manifest.included_result_keys, vec![key.clone()]);

        let export = store.text_export().unwrap();
        assert_eq!(export.entries.len(), 1);
        assert_eq!(export.entries[0].semantic_key, key);
        assert_eq!(export.entries[0].provider, "xai");
        assert_eq!(export.entries[0].model, "grok-4.5");
        assert_eq!(export.entries[0].semantics, "visible semantics");
        // No raw bytes anywhere in the serialized text export.
        let json = serde_json::to_vec(&export).unwrap();
        assert!(
            !json
                .windows(b"raw image bytes".len())
                .any(|w| w == b"raw image bytes")
        );
    }

    #[test]
    fn media_artifacts_fork_copy_hardlinks_and_copies() {
        let src_dir = tempfile::tempdir().unwrap();
        let src = MediaArtifactStore::open(src_dir.path()).unwrap();

        let blob = src.put_blob(b"forkable media bytes").unwrap();
        let derived = blob_hash(b"derived for fork");
        src.put_derived(&derived, b"derived for fork").unwrap();
        let result_key = blob_hash(b"result for fork");
        src.put_result(
            &result_key,
            &sample_result(MediaCategory::Image, "forked semantics"),
        )
        .unwrap();
        src.add_ref(
            RefKind::Attachments,
            "fork-ref",
            &[ObjectRef {
                kind: ArtifactKind::Blob,
                hash: blob.clone(),
            }],
        )
        .unwrap();

        let dst_dir = tempfile::tempdir().unwrap();
        let copied = copy_media_store(src_dir.path(), dst_dir.path()).unwrap();
        assert_eq!(copied, 3);

        let dst = MediaArtifactStore::open(dst_dir.path()).unwrap();
        assert_eq!(
            dst.get_blob(&blob).unwrap().unwrap(),
            b"forkable media bytes"
        );
        assert_eq!(
            dst.get_derived(&derived).unwrap().unwrap(),
            b"derived for fork"
        );
        assert!(dst.get_result(&result_key).unwrap().is_some());
        assert_eq!(dst.list_refs(RefKind::Attachments).unwrap().len(), 1);

        // Child journals are fresh and seeded with a Fork marker.
        let events = dst.read_journal().unwrap();
        assert!(matches!(events.as_slice(), [JournalEvent::Fork { .. }]));
        assert!(!dst_dir.path().join("assets/media/usage.jsonl").exists());

        // On the same filesystem the object should be hard-linked (same inode).
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let src_meta =
                std::fs::metadata(src.object_path(ArtifactKind::Blob, &blob).unwrap()).unwrap();
            let dst_meta =
                std::fs::metadata(dst.object_path(ArtifactKind::Blob, &blob).unwrap()).unwrap();
            assert_eq!(
                src_meta.ino(),
                dst_meta.ino(),
                "objects should be hard-linked"
            );
        }
    }

    #[test]
    fn media_artifacts_ref_validation_blocks_escape() {
        let dir = tempfile::tempdir().unwrap();
        let store = MediaArtifactStore::open(dir.path()).unwrap();

        for bad in ["../evil", "a/b", "name with space"] {
            assert!(store.add_ref(RefKind::Attachments, bad, &[]).is_err());
        }
        let ok = store.add_ref(RefKind::Attachments, "safe-name.v2", &[]);
        assert!(ok.is_ok());
        assert_eq!(store.list_refs(RefKind::Attachments).unwrap().len(), 1);
    }
}
