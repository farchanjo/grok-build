//! Transactional vector rebuild state machine (PR21).
//!
//! Replaces the old immediate destructive dimension-only drop with a
//! fail-closed state machine. Chunks + FTS survive every vector-migration
//! failure.
//!
//! ## State machine
//!
//! - No fingerprint installed (fresh or legacy) with matching dimensions ⇒
//!   **adopt** the current source's canonical fingerprint without rebuilding
//!   (existing users do not reconnect, rewrite config, or rebuild).
//! - Installed fingerprint differs, dimensions differ, or a prior incomplete
//!   marker exists ⇒ mark **rebuild pending** and operate **FTS-only** (old
//!   vectors are never queried / mixed).
//! - A rebuild is **claimed** (across sessions/processes) then vectors are
//!   embedded through the pinned source into a **staging** table in bounded
//!   batches (no DB/write lock held across network awaits).
//! - On success the complete vector set + dimensions + fingerprint are
//!   installed **atomically** (single SQLite transaction) and pending is
//!   cleared. On failure or crash/reopen, chunks/FTS survive, the diagnostic
//!   old fingerprint is retained, and the index remains pending/FTS-only.
//! - Staged rows are bound to the pending attempt `id`; a stale async result
//!   for an old intended fingerprint/incarnation is discarded by never being
//!   selected at install time. Claim expiration/recovery never installs stale
//!   results (install runs only with a matching verified pending marker).

use rusqlite::params;

use super::fingerprint::VectorFingerprint;
use super::index::MemoryIndex;
use super::schema;
use super::storage::MemoryStorage;
use xai_grok_config_types::MemoryIndexConfig;

// ---------------------------------------------------------------------------
// Pending marker payload
// ---------------------------------------------------------------------------

/// A durable pending rebuild marker persisted in `meta['vector_rebuild_pending']`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingRebuild {
    /// Attempt id binding staging rows + claims (fresh per rebuild-round).
    pub id: String,
    /// Target canonical fingerprint hash this attempt will install.
    pub intended: String,
    /// `"pending"` or `"running"`.
    pub status: String,
    /// Rebuild owner claim (`"pid:ts"`, `""` when unclaimed).
    pub claim: String,
    /// Claim timestamp (seconds), `0` when unclaimed.
    pub claimed_at: i64,
    /// Human-readable reason (dimension_mismatch, fingerprint_mismatch, ...).
    pub reason: String,
}

impl PendingRebuild {
    fn to_json(&self) -> String {
        format!(
            r#"{{"id":"{}","intended":"{}","status":"{}","claim":"{}","claimed_at":{},"reason":"{}"}}"#,
            self.id, self.intended, self.status, self.claim, self.claimed_at, self.reason
        )
    }

    /// Parse the JSON pending payload. `""`/garbage ⇒ `None`.
    fn parse(raw: &str) -> Option<Self> {
        if raw.trim().is_empty() {
            return None;
        }
        let v: serde_json::Value = serde_json::from_str(raw).ok()?;
        Some(PendingRebuild {
            id: v.get("id")?.as_str()?.to_owned(),
            intended: v
                .get("intended")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_owned(),
            status: v
                .get("status")
                .and_then(|x| x.as_str())
                .unwrap_or("pending")
                .to_owned(),
            claim: v
                .get("claim")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_owned(),
            claimed_at: v.get("claimed_at").and_then(|x| x.as_i64()).unwrap_or(0),
            reason: v
                .get("reason")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_owned(),
        })
    }
}

fn new_attempt_id() -> String {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Hex-ish safe token (no quotes/commas) suitable for embedding in SQL.
    format!("{now:x}-{}-{n:x}", std::process::id())
}

// ---------------------------------------------------------------------------
// meta helpers over a raw connection (used from the index open path)
// ---------------------------------------------------------------------------

fn meta_set_tx(index: &MemoryIndex, key: &str, value: &str) -> Result<(), rusqlite::Error> {
    index
        .db()
        .execute(schema::UPSERT_META_SQL, params![key, value])
        .map(|_| ())
}

/// Mark a pending rebuild for a dimension mismatch (fail-closed). Reuses an
/// existing pending attempt id if one is already recorded so a reopen does not
/// spin up a fresh attempt over queued work.
pub fn mark_dimension_mismatch_pending(db: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    let existing = db
        .query_row(
            schema::GET_META_SQL,
            params![schema::META_VECTOR_REBUILD_PENDING],
            |r| r.get::<_, String>(0),
        )
        .ok()
        .unwrap_or_default();
    let pending = match PendingRebuild::parse(&existing) {
        Some(p) if p.status != "done" => p,
        _ => PendingRebuild {
            id: new_attempt_id(),
            intended: String::new(),
            status: "pending".into(),
            claim: String::new(),
            claimed_at: 0,
            reason: "dimension_mismatch".into(),
        },
    };
    db.execute(
        schema::UPSERT_META_SQL,
        params![schema::META_VECTOR_REBUILD_PENDING, pending.to_json()],
    )?;
    // Bind staging to this attempt even before the intended fingerprint is
    // known so any stale staging rows under an older id are ignored.
    db.execute(
        schema::UPSERT_META_SQL,
        params![schema::META_VECTOR_STAGING_FP, pending.id],
    )?;
    Ok(())
}

/// Read the current pending rebuild state (None when empty/absent).
pub fn pending_state(index: &MemoryIndex) -> Option<PendingRebuild> {
    let raw = index.meta_get(schema::META_VECTOR_REBUILD_PENDING)?;
    PendingRebuild::parse(&raw)
}

/// Reconcile the pending marker for a target fingerprint.
///
/// Ensures a pending marker exists referencing `intended_fp`, reusing the
/// current attempt id when a pending marker is already present (so a reopen of
/// the same attempt resumes it) but switching to a fresh id when the marker is
/// absent or references a different (stale) target.
pub fn ensure_pending(
    index: &MemoryIndex,
    intended_fp: &str,
    reason: &str,
) -> Result<PendingRebuild, rusqlite::Error> {
    let existing = index
        .meta_get(schema::META_VECTOR_REBUILD_PENDING)
        .unwrap_or_default();
    let fresh = match PendingRebuild::parse(&existing) {
        Some(p) if p.status != "done" && (p.intended.is_empty() || p.intended == intended_fp) => p,
        _ => PendingRebuild {
            id: new_attempt_id(),
            intended: intended_fp.to_owned(),
            status: "pending".into(),
            claim: String::new(),
            claimed_at: 0,
            reason: reason.to_owned(),
        },
    };
    let mut ready = fresh;
    ready.intended = intended_fp.to_owned();
    ready.reason = reason.to_owned();
    meta_set_tx(index, schema::META_VECTOR_REBUILD_PENDING, &ready.to_json())?;
    meta_set_tx(index, schema::META_VECTOR_STAGING_FP, &ready.id)?;
    Ok(ready)
}

fn meta_set_conn(db: &rusqlite::Connection, key: &str, value: &str) -> Result<(), rusqlite::Error> {
    db.execute(schema::UPSERT_META_SQL, params![key, value])
        .map(|_| ())
}

/// Try to claim an exclusive rebuild for `pending_id` (like the reindex claim).
///
/// Succeeds when unclaimed or stale (`claimed_at < now - stale_secs`). On a
/// stale/claimed-with-validation, a claimed-by-other pending stays claimed so
/// writers serialize across sessions. Returns `true` if this caller won.
pub fn try_claim_rebuild(
    index: &MemoryIndex,
    pending: &mut PendingRebuild,
    stale_secs: i64,
) -> bool {
    let now = now_secs();
    let pid = std::process::id();
    let claim_value = format!("{pid}:{now}");
    let stale_cutoff = now - stale_secs;

    let claimable = pending.claim.is_empty()
        || pending.claimed_at < stale_cutoff
        // Allow the same process to reclaim its own attempt after a failure
        // (repeated failures retry rather than waiting for the claim to
        // expire). A *different* live process holding a fresh claim still
        // defers, so rebuild serializes across sessions/processes.
        || pending.claim.starts_with(&format!("{pid}:"));
    if !claimable {
        return false;
    }
    // Atomic UPDATE; only this process wins.
    let updated = index
        .db()
        .execute(
            "UPDATE meta SET value = ?1 WHERE key = ?2 AND value = ?3",
            params![
                pending.to_json(),
                schema::META_VECTOR_REBUILD_PENDING,
                pending.to_json()
            ],
        )
        .unwrap_or(0);
    if updated != 1 {
        return false;
    }
    pending.claim = claim_value;
    pending.claimed_at = now;
    pending.status = "running".into();
    let _ = meta_set_tx(
        index,
        schema::META_VECTOR_REBUILD_PENDING,
        &pending.to_json(),
    );
    let _ = meta_set_tx(index, schema::META_VECTOR_STAGING_FP, &pending.id);
    true
}

pub fn release_rebuild_claim(index: &MemoryIndex, pending: &PendingRebuild) {
    let _ = meta_set_tx(index, schema::META_VECTOR_REBUILD_PENDING, &{
        let mut p = pending.clone();
        p.claim = String::new();
        p.claimed_at = 0;
        p.status = "pending".into();
        p.to_json()
    });
}

// ---------------------------------------------------------------------------
// Staging
// ---------------------------------------------------------------------------

/// Stage a computed embedding for a chunk under the attempt `pending_id`.
pub fn stage_vector(
    index: &MemoryIndex,
    pending_id: &str,
    intended_fp: &str,
    chunk_id: &str,
    embedding: &[f32],
) -> Result<(), rusqlite::Error> {
    let bytes: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();
    index.db().execute(
        "INSERT OR REPLACE INTO vector_staging(pending_id, intended_fingerprint, chunk_id, embedding) \
         VALUES (?1, ?2, ?3, ?4)",
        params![pending_id, intended_fp, chunk_id, bytes],
    )?;
    Ok(())
}

/// Number of staged rows for the attempt `pending_id`.
pub fn staged_count(index: &MemoryIndex, pending_id: &str) -> i64 {
    index
        .db()
        .query_row(
            "SELECT COUNT(*) FROM vector_staging WHERE pending_id = ?1",
            params![pending_id],
            |r| r.get(0),
        )
        .unwrap_or(0)
}

/// Drop staging rows for an attempt - used on failure/reopen to clear
/// partial staging while keeping chunks/FTS/pending intact.
pub fn discard_staging(index: &MemoryIndex, pending_id: &str) -> Result<(), rusqlite::Error> {
    index
        .db()
        .execute(
            "DELETE FROM vector_staging WHERE pending_id = ?1",
            params![pending_id],
        )
        .map(|_| ())
}

/// Drop staging rows for *other* attempts (stale async results from an old
/// intended fingerprint/incarnation). Only the current `pending_id` survives.
pub fn discard_foreign_staging(
    index: &MemoryIndex,
    pending_id: &str,
) -> Result<(), rusqlite::Error> {
    index
        .db()
        .execute(
            "DELETE FROM vector_staging WHERE pending_id != ?1",
            params![pending_id],
        )
        .map(|_| ())
}

// ---------------------------------------------------------------------------
// Adopt (legacy / fresh no-op migration)
// ---------------------------------------------------------------------------

/// Adopt a fingerprint as installed without a rebuild.
///
/// Safe only when (a) the existing vector set is empty, or (b) it was built in
/// the same dimensions and the caller has verified there is no conflicting
/// installed fingerprint. Existing users with populated vectors and no
/// fingerprint (legacy) adopt their synthesized spec's fingerprint so they do
/// not rebuild.
pub fn adopt_installed(
    index: &MemoryIndex,
    fp: &VectorFingerprint,
    payload: &str,
    dimensions: usize,
) -> Result<(), rusqlite::Error> {
    let v_schema = u32::max(
        index.installed_vector_schema_version(),
        fp.vector_schema_version,
    );
    meta_set_conn(index.db(), "embedding_dimensions", &dimensions.to_string())?;
    meta_set_conn(index.db(), schema::META_VECTOR_FINGERPRINT_HASH, &fp.hash)?;
    meta_set_conn(index.db(), schema::META_VECTOR_FINGERPRINT, payload)?;
    meta_set_conn(
        index.db(),
        schema::META_VECTOR_SCHEMA_VERSION,
        &v_schema.to_string(),
    )?;
    meta_set_conn(index.db(), schema::META_VECTOR_REBUILD_PENDING, "")?;
    meta_set_conn(index.db(), schema::META_VECTOR_STAGING_FP, "")?;
    // Clear any stale partial staging from old attempts.
    index.db().execute("DELETE FROM vector_staging", [])?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Atomic install
// ---------------------------------------------------------------------------

/// Atomically install a complete, compatible vector set + fingerprint.
///
/// `install_dimensions` may differ from the current table dimensions; the
/// `chunks_vec` virtual table is (re)created to match inside the transaction.
/// The swap is one SQLite transaction, so a crash or failure can never expose
/// partial vectors. Staging rows for the attempt are the source of truth;
/// foreign/foreign attempts are discarded at install time.
///
/// Returns success only when the staged set covers every chunk (`staged_count
/// == chunk_count`). Otherwise remains pending (FTS-only).
pub fn install_vectors(
    index: &MemoryIndex,
    pending: &PendingRebuild,
    fp: &VectorFingerprint,
    payload: &str,
    install_dimensions: usize,
) -> Result<bool, rusqlite::Error> {
    if !index.vec_available() {
        return Ok(false);
    }
    let staged = staged_count(index, &pending.id);
    let expected = index.chunk_count();
    if staged != expected {
        // Incomplete/partial - never install. Remain pending, FTS-only.
        return Ok(false);
    }

    let db = index.db();
    db.execute_batch("BEGIN IMMEDIATE;")?;
    let result = (|| -> Result<(), rusqlite::Error> {
        let dims = install_dimensions;
        // Recreate vec0 with (possibly new) dimensions to guarantee a fully
        // compatible vector set with zero old rows.
        db.execute("DROP TABLE IF EXISTS chunks_vec", [])?;
        db.execute_batch(&format!(
            "CREATE VIRTUAL TABLE chunks_vec USING vec0(\n    \
             chunk_id TEXT PRIMARY KEY,\n    \
             embedding FLOAT[{dims}]\n);"
        ))?;
        db.execute(
            "INSERT INTO chunks_vec(chunk_id, embedding) \
             SELECT chunk_id, embedding FROM vector_staging WHERE pending_id = ?1 AND intended_fingerprint = ?2",
            params![pending.id, fp.hash],
        )?;
        db.execute(
            "DELETE FROM vector_staging WHERE pending_id = ?1",
            params![pending.id],
        )?;
        db.execute(
            "INSERT OR REPLACE INTO meta(key, value) VALUES ('embedding_dimensions', ?1)",
            params![dims.to_string()],
        )?;
        meta_set_conn(db, schema::META_VECTOR_FINGERPRINT_HASH, &fp.hash)?;
        meta_set_conn(db, schema::META_VECTOR_FINGERPRINT, payload)?;
        meta_set_conn(
            db,
            schema::META_VECTOR_SCHEMA_VERSION,
            &fp.vector_schema_version.to_string(),
        )?;
        meta_set_conn(db, schema::META_VECTOR_REBUILD_PENDING, "")?;
        meta_set_conn(db, schema::META_VECTOR_STAGING_FP, "")?;
        // Discard any staging rows under stale/foreign attempts.
        db.execute("DELETE FROM vector_staging", [])?;
        Ok(())
    })();
    if result.is_err() {
        let _ = db.execute_batch("ROLLBACK;");
        return Err(result.err().unwrap());
    }
    #[cfg(test)]
    if crate::rebuild::PRE_COMMIT_FAIL.load(std::sync::atomic::Ordering::SeqCst) {
        // Injected pre-commit failure (tests): roll the transaction back so
        // the old vector table + metadata are untouched and stay hidden.
        let _ = db.execute_batch("ROLLBACK;");
        return Ok(false);
    }
    db.execute_batch("COMMIT;")?;
    Ok(true)
}

/// Test-only hook: when true, `install_vectors` rolls back right before
/// COMMIT so tests can assert the old vector set + fingerprint remain intact
/// (atomic swap guarantee).
#[cfg(test)]
pub(crate) static PRE_COMMIT_FAIL: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

// ---------------------------------------------------------------------------
// Async rebuild orchestration
// ---------------------------------------------------------------------------

use std::path::Path;
use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use super::embedding::EmbeddingProvider;
use super::fingerprint::{DocPreparationSpec, EmbeddingSourceSpec, VECTOR_SCHEMA_VERSION};

/// Outcome of the vector reconcile/build for this query round.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VectorReadiness {
    /// Installed vectors are compatible with the pinned source — query them.
    Ready,
    /// Rebuild pending — operate FTS-only. `owned` indicates this caller
    /// (tried to) run the rebuild this round.
    Pending { owned: bool },
    /// sqlite-vec unavailable (or no embedding source) — FTS-only.
    Disabled,
}

fn open_index(
    db_path: &Path,
    storage: MemoryStorage,
    index_config: MemoryIndexConfig,
    dimensions: usize,
) -> Result<MemoryIndex, String> {
    super::index::MemoryIndex::open_or_create(db_path, storage, index_config, dimensions)
        .map_err(|e| e.to_string())
}

/// Try to reconcile install/rebuild the vector set for the pinned source.
///
/// Fail-closed, transactional, crash-safe (see the module docs). Returns
/// [`VectorReadiness`] guiding whether the caller may query vectors this
/// round. Never holds a DB/write lock across a network await: each phase opens
/// a fresh index and drops it before awaiting embedding.
pub async fn ensure_vectors_ready(
    db_path: &Path,
    storage: MemoryStorage,
    index_config: MemoryIndexConfig,
    spec: &EmbeddingSourceSpec,
    embedder: Option<Arc<dyn EmbeddingProvider>>,
    stale_claim_secs: i64,
    cancel: CancellationToken,
) -> VectorReadiness {
    let (fp, payload) = match VectorFingerprint::build(
        spec.clone(),
        DocPreparationSpec::from_index_config(&index_config),
        VECTOR_SCHEMA_VERSION,
    ) {
        Ok(v) => v,
        Err(_) => return VectorReadiness::Pending { owned: false },
    };

    // Phase 0: open + decide.
    let idx = match open_index(
        db_path,
        storage.clone(),
        index_config.clone(),
        spec.dimensions,
    ) {
        Ok(v) => v,
        Err(_) => return VectorReadiness::Pending { owned: false },
    };
    if !idx.vec_available() {
        return VectorReadiness::Disabled;
    }
    let installed = idx.installed_vector_fingerprint_hash();
    let installed_dims = idx.embedding_dimensions();
    match installed {
        Some(h) if h == fp.hash && installed_dims == spec.dimensions => {
            // Compatible: reuse vectors. No rebuild.
            return VectorReadiness::Ready;
        }
        Some(_) => { /* mismatch -> rebuild below */ }
        None => {
            // No fingerprint: fresh or legacy. Adopt without rebuilding so
            // existing users do not reconnect/rebuild unnecessarily.
            let rows = idx.vec_row_count();
            let chunk_count = idx.chunk_count();
            if rows == 0 || (rows == chunk_count && installed_dims == spec.dimensions) {
                let _ = adopt_installed(&idx, &fp, &payload, spec.dimensions);
                return VectorReadiness::Ready;
            }
            // Populated but dims mismatch or fingerprint-less partial set:
            // fall through to rebuild.
        }
    }
    drop(idx);

    // Cross-phase open because we need a fresh handle after `drop` above.
    let idx = match open_index(
        db_path,
        storage.clone(),
        index_config.clone(),
        spec.dimensions,
    ) {
        Ok(v) => v,
        Err(_) => return VectorReadiness::Pending { owned: false },
    };
    let reason = if installed.is_none() {
        "adopt_incompatible"
    } else if installed.as_deref() == Some(fp.hash.as_str()) {
        "schema_mismatch"
    } else {
        "fingerprint_mismatch"
    };
    let mut pending = match ensure_pending(&idx, &fp.hash, reason) {
        Ok(p) => p,
        Err(_) => return VectorReadiness::Pending { owned: false },
    };
    let _ = discard_foreign_staging(&idx, &pending.id);
    drop(idx);

    if !try_claim_rebuild_open(
        db_path,
        &storage,
        &index_config,
        spec,
        &mut pending,
        stale_claim_secs,
    ) {
        // Another session/process owns (or is recovering) the rebuild.
        return VectorReadiness::Pending { owned: false };
    }

    if cancel.is_cancelled() {
        return VectorReadiness::Pending { owned: true };
    }

    // Build loop.
    loop {
        let idx = match open_index(
            db_path,
            storage.clone(),
            index_config.clone(),
            spec.dimensions,
        ) {
            Ok(v) => v,
            Err(_) => return VectorReadiness::Pending { owned: true },
        };
        // Pending must still reference our attempt id; a superseding rebuild
        // (newer fingerprint/incarnation) must discard our stale staging.
        match pending_state(&idx) {
            Some(p) if p.id == pending.id => {}
            _ => return VectorReadiness::Pending { owned: true },
        }
        if cancel.is_cancelled() {
            return VectorReadiness::Pending { owned: true };
        }
        let needed = match idx.chunks_not_staged(&pending.id) {
            Ok(n) => n,
            Err(_) => return VectorReadiness::Pending { owned: true },
        };
        let staged = staged_count(&idx, &pending.id);
        let total = idx.chunk_count();
        if needed.is_empty() {
            if staged >= total {
                let ok = match install_vectors(&idx, &pending, &fp, &payload, spec.dimensions) {
                    Ok(v) => v,
                    Err(_) => false,
                };
                return if ok {
                    VectorReadiness::Ready
                } else {
                    VectorReadiness::Pending { owned: true }
                };
            }
            return VectorReadiness::Pending { owned: true };
        }
        drop(idx);

        let Some(embedder) = embedder.clone() else {
            return VectorReadiness::Pending { owned: true };
        };
        // Embed a bounded batch, then stage (open fresh index per batch so no
        // &index borrow crosses an await).
        for batch in needed.chunks(batch_size(embedder.as_ref())) {
            if cancel.is_cancelled() {
                return VectorReadiness::Pending { owned: true };
            }
            let texts: Vec<&str> = batch.iter().map(|(_, t)| t.as_str()).collect();
            let vectors = match embedder.embed_batch(&texts).await {
                Ok(v) if v.len() == batch.len() => v,
                _ => return VectorReadiness::Pending { owned: true },
            };
            let idx = match open_index(
                db_path,
                storage.clone(),
                index_config.clone(),
                spec.dimensions,
            ) {
                Ok(v) => v,
                Err(_) => return VectorReadiness::Pending { owned: true },
            };
            match pending_state(&idx) {
                Some(p) if p.id == pending.id => {}
                _ => return VectorReadiness::Pending { owned: true },
            }
            for ((cid, _), v) in batch.iter().zip(vectors.iter()) {
                if stage_vector(
                    &idx,
                    &pending.id,
                    fp.hash.as_str(),
                    cid.as_str(),
                    v.as_slice(),
                )
                .is_err()
                {
                    return VectorReadiness::Pending { owned: true };
                }
            }
            drop(idx);
        }
    }
}

fn batch_size(p: &dyn EmbeddingProvider) -> usize {
    // Bound batches; keep aligned to typical provider max payloads.
    let _ = p.dimensions();
    32
}

fn try_claim_rebuild_open(
    db_path: &Path,
    storage: &MemoryStorage,
    index_config: &MemoryIndexConfig,
    spec: &EmbeddingSourceSpec,
    pending: &mut PendingRebuild,
    stale_claim_secs: i64,
) -> bool {
    let Ok(idx) = super::index::MemoryIndex::open_or_create(
        db_path,
        storage.clone(),
        index_config.clone(),
        spec.dimensions,
    ) else {
        return false;
    };
    try_claim_rebuild(&idx, pending, stale_claim_secs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding::{EmbeddingProvider, MockEmbeddingProvider, RetrievalEmbeddingProvider};
    use crate::fingerprint::{DocPreparationSpec, VECTOR_SCHEMA_VERSION, VectorFingerprint};
    use crate::index::{MemoryIndex, init_sqlite_vec};
    use crate::retrieval::{FakeMemoryRetrieval, MemoryRetrieval as _, stub_spec};
    use std::sync::Arc;
    use tempfile::TempDir;

    fn make_storage(tmp: &TempDir) -> MemoryStorage {
        let global = tmp.path().join("memory");
        let workspace = global.join("ws");
        MemoryStorage::with_paths(global, workspace)
    }

    fn seed_chunk(idx: &mut MemoryIndex, tmp: &TempDir) -> String {
        let f = tmp.path().join("note.md");
        std::fs::write(&f, "# Facts\n\nRust is fast and memory-safe.").unwrap();
        idx.reindex_file(&f, "workspace").unwrap();
        f.to_string_lossy().to_string()
    }

    fn open_index(db_path: &std::path::Path, storage: MemoryStorage, dims: usize) -> MemoryIndex {
        MemoryIndex::open_or_create(
            db_path,
            storage,
            xai_grok_config_types::MemoryIndexConfig::default(),
            dims,
        )
        .unwrap_or_else(|_| panic!("open index"))
    }

    fn fp_for(spec: &crate::fingerprint::EmbeddingSourceSpec) -> VectorFingerprint {
        VectorFingerprint::build(
            spec.clone(),
            DocPreparationSpec::from_index_config(
                &xai_grok_config_types::MemoryIndexConfig::default(),
            ),
            VECTOR_SCHEMA_VERSION,
        )
        .unwrap()
        .0
    }

    /// Install a fingerprint + full vector set for `spec` (as the search path
    /// would after adopting a fresh/legacy index).
    async fn install_vectors_for(
        db_path: &std::path::Path,
        storage: &MemoryStorage,
        dims: usize,
        spec: &crate::fingerprint::EmbeddingSourceSpec,
        model_label: &str,
    ) -> Arc<FakeMemoryRetrieval> {
        let fake = Arc::new(FakeMemoryRetrieval::new(dims, model_label));
        let embedder: Option<Arc<dyn EmbeddingProvider>> =
            Some(Arc::new(RetrievalEmbeddingProvider::new(fake.clone())));
        let out = ensure_vectors_ready(
            db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            spec,
            embedder,
            60,
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(out, VectorReadiness::Ready), "{out:?}");
        // Incremental: embed every chunk into the (adopted) space directly.
        let mut idx = open_index(db_path, storage.clone(), dims);
        let chunks = idx.chunks_without_embeddings().unwrap();
        let mock = Arc::new(MockEmbeddingProvider { dimensions: dims });
        for (cid, text) in &chunks {
            let v = mock.embed_batch(&[text.as_str()]).await.unwrap();
            idx.upsert_embedding(cid, &v[0]).unwrap();
        }
        fake
    }

    #[tokio::test]
    async fn test_legacy_adopt_no_rebuild() {
        init_sqlite_vec();
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        let mut idx = open_index(&db_path, storage.clone(), dims);
        seed_chunk(&mut idx, &tmp);
        // Legacy: vectors present, no fingerprint persisted.
        let mock = Arc::new(MockEmbeddingProvider { dimensions: dims });
        let chunks = idx.chunks_without_embeddings().unwrap();
        for (cid, text) in &chunks {
            let v = mock.embed_batch(&[text.as_str()]).await.unwrap();
            idx.upsert_embedding(cid, &v[0]).unwrap();
        }
        assert!(idx.installed_vector_fingerprint_hash().is_none());
        drop(idx);

        let fake = Arc::new(FakeMemoryRetrieval::new(dims, "legacy-model"));
        let spec = fake.source_spec();
        let embedder: Option<Arc<dyn EmbeddingProvider>> =
            Some(Arc::new(RetrievalEmbeddingProvider::new(fake.clone())));
        let out = ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &spec,
            embedder,
            60,
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(out, VectorReadiness::Ready), "{out:?}");
        // No upgrade rebuild: the existing vectors are adopted as compatible.
        assert_eq!(fake.embed_calls(), 0, "adopt must not re-embed");

        let idx = open_index(&db_path, storage.clone(), dims);
        let installed = idx.installed_vector_fingerprint_hash().unwrap();
        assert_eq!(
            installed,
            fp_for(&spec).hash,
            "adopted fingerprint must match the synthesized legacy source"
        );
        assert_eq!(idx.vec_row_count(), idx.chunk_count());
    }

    #[tokio::test]
    async fn test_same_dims_different_model_rebuilds() {
        init_sqlite_vec();
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        let mut idx = open_index(&db_path, storage.clone(), dims);
        seed_chunk(&mut idx, &tmp);
        drop(idx);

        let spec_a = stub_spec(dims, "model-a");
        install_vectors_for(&db_path, &storage, dims, &spec_a, "model-a").await;
        let idx = open_index(&db_path, storage.clone(), dims);
        assert_eq!(
            idx.installed_vector_fingerprint_hash().unwrap(),
            fp_for(&spec_a).hash
        );
        drop(idx);

        // Same dimensions, different upstream model/endpoint => rebuild.
        let spec_b = crate::fingerprint::EmbeddingSourceSpec {
            model: "model-b".into(),
            ..spec_a.clone()
        };
        let fake = Arc::new(FakeMemoryRetrieval::new(dims, "model-b"));
        let embedder: Option<Arc<dyn EmbeddingProvider>> =
            Some(Arc::new(RetrievalEmbeddingProvider::new(fake.clone())));
        let out = ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &spec_b,
            embedder,
            60,
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(out, VectorReadiness::Ready), "{out:?}");
        assert!(fake.embed_calls() > 0, "model change must rebuild");
        let idx = open_index(&db_path, storage.clone(), dims);
        assert_eq!(
            idx.installed_vector_fingerprint_hash().unwrap(),
            fp_for(&spec_b).hash
        );
        assert_eq!(
            idx.vec_row_count(),
            idx.chunk_count(),
            "complete vector set after rebuild"
        );
    }

    #[tokio::test]
    async fn test_failed_rebuild_keeps_chunks_fts_old_fp() {
        init_sqlite_vec();
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        let mut idx = open_index(&db_path, storage.clone(), dims);
        seed_chunk(&mut idx, &tmp);
        drop(idx);
        let spec_a = stub_spec(dims, "model-a");
        install_vectors_for(&db_path, &storage, dims, &spec_a, "model-a").await;

        // Same dims, different model, failing embedder => rebuild fails.
        let spec_b = crate::fingerprint::EmbeddingSourceSpec {
            model: "model-b".into(),
            ..spec_a.clone()
        };
        let failing: Option<Arc<dyn EmbeddingProvider>> =
            Some(Arc::new(FailingEmbeddingProvider { dims }));
        let out = ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &spec_b,
            failing,
            60,
            CancellationToken::new(),
        )
        .await;
        assert!(
            matches!(out, VectorReadiness::Pending { owned: true }),
            "{out:?}"
        );

        // Chunks + FTS survive; old fingerprint retained; no partial vectors.
        let idx = open_index(&db_path, storage.clone(), dims);
        let chunk_count = idx.chunk_count();
        assert_eq!(chunk_count, 1);
        let fts = idx.search_fts("rust", 10).unwrap();
        assert!(
            !fts.is_empty(),
            "FTS must keep working after failed rebuild"
        );
        assert_eq!(
            idx.installed_vector_fingerprint_hash().unwrap(),
            fp_for(&spec_a).hash,
            "old fingerprint must be retained on failure"
        );
        assert_eq!(idx.vec_row_count(), chunk_count, "no partial/mixed vectors");
        assert!(
            pending_state(&idx).is_some(),
            "must remain pending (FTS-only)"
        );
        drop(idx);

        // Recovery: with a working embedder the same pending rebuild completes.
        let fake = Arc::new(FakeMemoryRetrieval::new(dims, "model-b"));
        let embedder: Option<Arc<dyn EmbeddingProvider>> =
            Some(Arc::new(RetrievalEmbeddingProvider::new(fake.clone())));
        let out = ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &spec_b,
            embedder,
            60,
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(out, VectorReadiness::Ready), "{out:?}");
        let idx = open_index(&db_path, storage.clone(), dims);
        assert_eq!(
            idx.installed_vector_fingerprint_hash().unwrap(),
            fp_for(&spec_b).hash
        );
        assert!(pending_state(&idx).is_none());
        // Repeated failures stay pending (FTS-only).
        drop(idx);
        let spec_c = crate::fingerprint::EmbeddingSourceSpec {
            model: "model-c".into(),
            ..spec_b.clone()
        };
        let failing: Option<Arc<dyn EmbeddingProvider>> =
            Some(Arc::new(FailingEmbeddingProvider { dims }));
        for _ in 0..2 {
            let out = ensure_vectors_ready(
                &db_path,
                storage.clone(),
                xai_grok_config_types::MemoryIndexConfig::default(),
                &spec_c,
                failing.clone(),
                60,
                CancellationToken::new(),
            )
            .await;
            assert!(matches!(out, VectorReadiness::Pending { .. }), "{out:?}");
            let idx = open_index(&db_path, storage.clone(), dims);
            assert_eq!(
                idx.installed_vector_fingerprint_hash().unwrap(),
                fp_for(&spec_b).hash
            );
            assert!(pending_state(&idx).is_some());
        }
    }

    #[tokio::test]
    async fn test_crash_reopen_pending_partial_staging_recovers() {
        init_sqlite_vec();
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        let mut idx = open_index(&db_path, storage.clone(), dims);
        seed_chunk(&mut idx, &tmp);
        drop(idx);
        let spec_a = stub_spec(dims, "model-a");
        install_vectors_for(&db_path, &storage, dims, &spec_a, "model-a").await;

        let spec_b = crate::fingerprint::EmbeddingSourceSpec {
            model: "model-b".into(),
            ..spec_a.clone()
        };
        let fp_b = fp_for(&spec_b);
        // Simulate a crash mid-rebuild: partial staging + stale claim.
        {
            let idx = open_index(&db_path, storage.clone(), dims);
            let mut pending = ensure_pending(&idx, &fp_b.hash, "test").unwrap();
            assert!(
                try_claim_rebuild(&idx, &mut pending, 60),
                "must claim for the crashed attempt"
            );
            // Stage one of two chunks (partial).
            let chunk = idx
                .get_chunk(&format!(
                    "{}:0",
                    tmp.path().join("note.md").to_string_lossy()
                ))
                .unwrap()
                .unwrap();
            let mock = MockEmbeddingProvider { dimensions: dims };
            let v = mock.embed_batch(&[&chunk.text]).await.unwrap();
            stage_vector(
                &idx,
                &pending.id,
                fp_b.hash.as_str(),
                chunk.id.as_str(),
                &v[0],
            )
            .unwrap();
            // Crash: drop the handle (index was never dropped cleanly).
            // Backdate the claim so it looks stale after crash.
            let now = now_secs();
            let mut stale = pending.clone();
            stale.claimed_at = now - 1000;
            stale.claim = format!("999999:{}", now - 1000);
            meta_set_conn(
                idx.db(),
                schema::META_VECTOR_REBUILD_PENDING,
                &stale.to_json(),
            )
            .unwrap();
        }
        // Reopen: stale claim recovered, partial staging reused, completes.
        let fake = Arc::new(FakeMemoryRetrieval::new(dims, "model-b"));
        let embedder: Option<Arc<dyn EmbeddingProvider>> =
            Some(Arc::new(RetrievalEmbeddingProvider::new(fake.clone())));
        let out = ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &spec_b,
            embedder,
            60,
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(out, VectorReadiness::Ready), "{out:?}");
        let idx = open_index(&db_path, storage.clone(), dims);
        assert_eq!(idx.installed_vector_fingerprint_hash().unwrap(), fp_b.hash);
        assert_eq!(idx.vec_row_count(), idx.chunk_count());
        assert!(pending_state(&idx).is_none());
        let staged_left: i64 = idx
            .db()
            .query_row("SELECT COUNT(*) FROM vector_staging", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            staged_left, 0,
            "staging must be drained after atomic install"
        );
    }

    #[tokio::test]
    async fn test_stale_attempt_staging_rejected() {
        init_sqlite_vec();
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        let mut idx = open_index(&db_path, storage.clone(), dims);
        seed_chunk(&mut idx, &tmp);
        drop(idx);
        let spec_a = stub_spec(dims, "model-a");
        install_vectors_for(&db_path, &storage, dims, &spec_a, "model-a").await;

        // A stale async result from an old intended fingerprint left staging.
        let spec_x = crate::fingerprint::EmbeddingSourceSpec {
            model: "model-x".into(),
            ..spec_a.clone()
        };
        let fp_x = fp_for(&spec_x);
        {
            let idx = open_index(&db_path, storage.clone(), dims);
            let pending = ensure_pending(&idx, &fp_x.hash, "stale").unwrap();
            let mock = MockEmbeddingProvider { dimensions: dims };
            let chunks = idx.chunks_without_embeddings().unwrap();
            for (cid, text) in &chunks {
                let v = mock.embed_batch(&[text.as_str()]).await.unwrap();
                stage_vector(&idx, &pending.id, fp_x.hash.as_str(), cid.as_str(), &v[0]).unwrap();
            }
        }
        // New target B must discard X's staging and install B.
        let spec_b = crate::fingerprint::EmbeddingSourceSpec {
            model: "model-b".into(),
            ..spec_a.clone()
        };
        let fake = Arc::new(FakeMemoryRetrieval::new(dims, "model-b"));
        let embedder: Option<Arc<dyn EmbeddingProvider>> =
            Some(Arc::new(RetrievalEmbeddingProvider::new(fake.clone())));
        let out = ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &spec_b,
            embedder,
            60,
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(out, VectorReadiness::Ready), "{out:?}");
        let idx = open_index(&db_path, storage.clone(), dims);
        assert_eq!(
            idx.installed_vector_fingerprint_hash().unwrap(),
            fp_for(&spec_b).hash
        );
        let staged_left: i64 = idx
            .db()
            .query_row("SELECT COUNT(*) FROM vector_staging", [], |r| r.get(0))
            .unwrap();
        assert_eq!(staged_left, 0, "stale attempt staging must be discarded");
    }

    #[tokio::test]
    async fn test_concurrent_claim_defers() {
        init_sqlite_vec();
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        let mut idx = open_index(&db_path, storage.clone(), dims);
        seed_chunk(&mut idx, &tmp);
        drop(idx);
        let spec_a = stub_spec(dims, "model-a");
        install_vectors_for(&db_path, &storage, dims, &spec_a, "model-a").await;

        let spec_b = crate::fingerprint::EmbeddingSourceSpec {
            model: "model-b".into(),
            ..spec_a.clone()
        };
        let fp_b = fp_for(&spec_b);
        // Another process currently owns the rebuild claim (fresh).
        {
            let idx = open_index(&db_path, storage.clone(), dims);
            let mut pending = ensure_pending(&idx, &fp_b.hash, "test").unwrap();
            assert!(try_claim_rebuild(&idx, &mut pending, 60));
            // Rewrite the claim with a foreign process id (tests run in one
            // process; simulate a genuine concurrent owner).
            let now = now_secs();
            pending.claim = format!("424242:{now}");
            pending.claimed_at = now;
            meta_set_conn(
                idx.db(),
                schema::META_VECTOR_REBUILD_PENDING,
                &pending.to_json(),
            )
            .unwrap();
        }
        let fake = Arc::new(FakeMemoryRetrieval::new(dims, "model-b"));
        let embedder: Option<Arc<dyn EmbeddingProvider>> =
            Some(Arc::new(RetrievalEmbeddingProvider::new(fake.clone())));
        let out = ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &spec_b,
            embedder,
            60,
            CancellationToken::new(),
        )
        .await;
        assert!(
            matches!(out, VectorReadiness::Pending { owned: false }),
            "{out:?}"
        );
        assert_eq!(fake.embed_calls(), 0, "deferred caller must not embed");
        let idx = open_index(&db_path, storage.clone(), dims);
        assert_eq!(
            idx.installed_vector_fingerprint_hash().unwrap(),
            fp_for(&spec_a).hash
        );
    }

    #[tokio::test]
    async fn test_pre_commit_failure_is_atomic() {
        init_sqlite_vec();
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        let mut idx = open_index(&db_path, storage.clone(), dims);
        seed_chunk(&mut idx, &tmp);
        drop(idx);
        let spec_a = stub_spec(dims, "model-a");
        install_vectors_for(&db_path, &storage, dims, &spec_a, "model-a").await;
        let rows_before = {
            let idx = open_index(&db_path, storage.clone(), dims);
            idx.vec_row_count()
        };

        let spec_b = crate::fingerprint::EmbeddingSourceSpec {
            model: "model-b".into(),
            ..spec_a.clone()
        };
        // Inject failure at the atomic swap commit point.
        PRE_COMMIT_FAIL.store(true, std::sync::atomic::Ordering::SeqCst);
        let fake = Arc::new(FakeMemoryRetrieval::new(dims, "model-b"));
        let embedder: Option<Arc<dyn EmbeddingProvider>> =
            Some(Arc::new(RetrievalEmbeddingProvider::new(fake.clone())));
        let out = ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &spec_b,
            embedder.clone(),
            60,
            CancellationToken::new(),
        )
        .await;
        PRE_COMMIT_FAIL.store(false, std::sync::atomic::Ordering::SeqCst);
        assert!(matches!(out, VectorReadiness::Pending { .. }), "{out:?}");

        // Old metadata/data hidden & untouched; chunks + FTS intact.
        let idx = open_index(&db_path, storage.clone(), dims);
        assert_eq!(
            idx.installed_vector_fingerprint_hash().unwrap(),
            fp_for(&spec_a).hash,
            "old fingerprint must be retained after injected pre-commit failure"
        );
        assert_eq!(
            idx.vec_row_count(),
            rows_before,
            "old vectors must be untouched"
        );
        assert_eq!(idx.chunk_count(), 1);
        assert!(!idx.search_fts("rust", 10).unwrap().is_empty());
        drop(idx);

        // Retry without the injected failure: atomic swap succeeds.
        let out = ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &spec_b,
            embedder,
            60,
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(out, VectorReadiness::Ready), "{out:?}");
        let idx = open_index(&db_path, storage.clone(), dims);
        assert_eq!(
            idx.installed_vector_fingerprint_hash().unwrap(),
            fp_for(&spec_b).hash
        );
        assert_eq!(idx.vec_row_count(), idx.chunk_count());
    }

    struct FailingEmbeddingProvider {
        dims: usize,
    }

    #[async_trait::async_trait]
    impl EmbeddingProvider for FailingEmbeddingProvider {
        async fn embed_batch(
            &self,
            _texts: &[&str],
        ) -> Result<Vec<Vec<f32>>, Box<dyn std::error::Error>> {
            Err("injected embedding failure".into())
        }

        fn model_name(&self) -> &str {
            "failing"
        }

        fn dimensions(&self) -> usize {
            self.dims
        }
    }
}
