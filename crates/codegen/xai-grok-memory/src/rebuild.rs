//! Transactional vector rebuild state machine (PR21).
//!
//! Replaces the old immediate destructive dimension-only drop with a
//! fail-closed state machine. Chunks + FTS survive every vector-migration
//! failure.
//!
//! ## State machine
//!
//! - No fingerprint installed (fresh or legacy) with matching dimensions ⇒
//!   **adopt** the current source's canonical fingerprint **only when no
//!   pending marker exists** (parseable or corrupt — `pending_marker_present`
//!   blocks adopt) and the vec set is either genuinely empty on a brand-new
//!   index (zero chunks, zero vectors) or a provably complete same-dims
//!   legacy set. Adopt re-checks the marker inside its transaction (CAS) so a
//!   concurrently created marker aborts the adopt.
//! - Installed fingerprint differs, dimensions differ, or a prior incomplete
//!   marker exists ⇒ mark **rebuild pending** and operate **FTS-only** (old
//!   vectors are never queried / mixed).
//! - **Initial atomic migration vs compatible incremental backfill:** a
//!   matching installed fingerprint proves the atomic initial migration
//!   completed. Later `vec_row_count < chunk_count` is normal incremental
//!   chunk churn or a transient incremental embed failure — the existing
//!   compatible vectors stay **usable** and the caller backfills only the
//!   missing current chunks via `chunks_without_embeddings` on this/next
//!   search (`ReadyMissing`); this never triggers a full-index rebuild. A
//!   torn/corrupt install (fingerprint present but dimensions/schema
//!   mismatch) makes the source *incompatible* and still fails closed into a
//!   rebuild. A fresh index with no fingerprint and a non-empty chunk set is
//!   not yet "compatible" — it must atomically rebuild (and install the
//!   fingerprint) before vectors are ready.
//! - A rebuild is **claimed** with a true compare-and-swap (the claim write
//!   and the staging-binding write are one transaction); only one process
//!   wins. Stale/same-target takeovers observe the completed state as Ready.
//! - Vectors are staged keyed by **chunk id AND content hash**; stale staged
//!   rows for deleted/changed chunks are pruned each pass, so the attempt
//!   converges under chunk churn. On success the complete vector set +
//!   dimensions + fingerprint are installed **atomically** (single SQLite
//!   transaction, with a transactional `(id, hash)` completeness re-check
//!   inside the swap) and pending is cleared.
//! - **Back-off vs batch cap:** a *failed* attempt persists `last_attempt_at`
//!   and subsequent searches stay FTS-only within `backoff_secs`. The
//!   per-search **batch cap** is a normal pause (progress), does **not** arm
//!   the failure back-off, and the next search resumes immediately.
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
    /// This process's last rebuild attempt time (seconds, `0` = never).
    /// Persisted back-off so repeated failures do not rebuild on every search.
    pub last_attempt_at: i64,
}

impl PendingRebuild {
    fn to_json(&self) -> String {
        format!(
            r#"{{"id":"{}","intended":"{}","status":"{}","claim":"{}","claimed_at":{},"reason":"{}","last_attempt_at":{}}}"#,
            self.id,
            self.intended,
            self.status,
            self.claim,
            self.claimed_at,
            self.reason,
            self.last_attempt_at
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
            // No `done` sentinel exists; any parseable marker is active.
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
            last_attempt_at: v
                .get("last_attempt_at")
                .and_then(|x| x.as_i64())
                .unwrap_or(0),
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

/// Run `f` inside one `BEGIN IMMEDIATE … COMMIT` transaction, rolling back on
/// error. Used for every multi-statement marker/staging-binding write and for
/// the atomic adopt, so a crash can never leave torn metadata.
fn in_transaction<T>(
    db: &rusqlite::Connection,
    f: impl FnOnce(&rusqlite::Connection) -> rusqlite::Result<T>,
) -> rusqlite::Result<T> {
    db.execute_batch("BEGIN IMMEDIATE;")?;
    match f(db) {
        Ok(v) => {
            db.execute_batch("COMMIT;")?;
            Ok(v)
        }
        Err(e) => {
            let _ = db.execute_batch("ROLLBACK;");
            Err(e)
        }
    }
}

/// Mark a pending rebuild for a dimension mismatch (fail-closed). Reuses an
/// existing pending attempt id if one is already recorded so a reopen does not
/// spin up a fresh attempt over queued work. Pending marker + staging binding
/// are written in one transaction (no torn state on crash).
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
        Some(p) => p,
        None => PendingRebuild {
            id: new_attempt_id(),
            intended: String::new(),
            status: "pending".into(),
            claim: String::new(),
            claimed_at: 0,
            reason: "dimension_mismatch".into(),
            last_attempt_at: 0,
        },
    };
    in_transaction(db, |db| {
        // Bind staging to this attempt even before the intended fingerprint is
        // known so any stale staging rows under an older id are ignored.
        db.execute(
            schema::UPSERT_META_SQL,
            params![schema::META_VECTOR_REBUILD_PENDING, pending.to_json()],
        )?;
        db.execute(
            schema::UPSERT_META_SQL,
            params![schema::META_VECTOR_STAGING_FP, pending.id],
        )
        .map(|_| ())
    })
}

/// Read the current pending rebuild state (None when empty/absent).
pub fn pending_state(index: &MemoryIndex) -> Option<PendingRebuild> {
    let raw = index.meta_get(schema::META_VECTOR_REBUILD_PENDING)?;
    PendingRebuild::parse(&raw)
}

/// True when *any* pending marker text exists — parseable or not. A corrupt
/// (unparseable) marker is still a marker: adopt must never bypass it.
pub fn pending_marker_present(index: &MemoryIndex) -> bool {
    index
        .meta_get(schema::META_VECTOR_REBUILD_PENDING)
        .map(|raw| !raw.trim().is_empty())
        .unwrap_or(false)
}

/// Reconcile the pending marker for a target fingerprint.
///
/// Ensures a pending marker exists referencing `intended_fp`, reusing the
/// current attempt id when a pending marker is already present (so a reopen of
/// the same attempt resumes it) but switching to a fresh id when the marker is
/// absent or references a different (stale) target. Marker + staging-binding
/// are written atomically.
pub fn ensure_pending(
    index: &MemoryIndex,
    intended_fp: &str,
    reason: &str,
) -> Result<PendingRebuild, rusqlite::Error> {
    let existing = index
        .meta_get(schema::META_VECTOR_REBUILD_PENDING)
        .unwrap_or_default();
    let fresh = match PendingRebuild::parse(&existing) {
        Some(p) if p.intended.is_empty() || p.intended == intended_fp => p,
        _ => PendingRebuild {
            id: new_attempt_id(),
            intended: intended_fp.to_owned(),
            status: "pending".into(),
            claim: String::new(),
            claimed_at: 0,
            reason: reason.to_owned(),
            last_attempt_at: 0,
        },
    };
    let mut ready = fresh;
    ready.intended = intended_fp.to_owned();
    ready.reason = reason.to_owned();
    let db = index.db();
    let ready_json = ready.to_json();
    let id = ready.id.clone();
    in_transaction(db, |db| {
        db.execute(
            schema::UPSERT_META_SQL,
            params![schema::META_VECTOR_REBUILD_PENDING, &ready_json],
        )?;
        db.execute(
            schema::UPSERT_META_SQL,
            params![schema::META_VECTOR_STAGING_FP, &id],
        )
        .map(|_| ())
    })?;
    Ok(ready)
}

fn meta_set_conn(db: &rusqlite::Connection, key: &str, value: &str) -> Result<(), rusqlite::Error> {
    db.execute(schema::UPSERT_META_SQL, params![key, value])
        .map(|_| ())
}

/// Try to claim an exclusive rebuild for the pending attempt.
///
/// A **true compare-and-swap**: the new claimed JSON is written only when the
/// stored JSON still equals the exact snapshot we read, so at most one
/// process/attempt wins. The claim CAS and the staging-binding write happen in
/// **one transaction** (the `in_transaction` helper), so a crash cannot leave
/// a claimed marker bound to a different staging id. Succeeds when unclaimed
/// or stale (`claimed_at < now - stale_secs`) or when this same process
/// already owns the claim (retry after failure). Returns `true` if this caller
/// won; on success `*pending` is mutated to the claimed state.
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

    let old_json = pending.to_json();
    let mut claimed = pending.clone();
    claimed.claim = claim_value;
    claimed.claimed_at = now;
    claimed.status = "running".into();
    let new_json = claimed.to_json();

    let db = index.db();
    // CAS + staging binding in one transaction (accurate comment, real code).
    let updated = in_transaction(db, |db| {
        let updated = db
            .execute(
                "UPDATE meta SET value = ?1 WHERE key = ?2 AND value = ?3",
                params![&new_json, schema::META_VECTOR_REBUILD_PENDING, &old_json],
            )
            .unwrap_or(0);
        if updated == 1 {
            db.execute(
                schema::UPSERT_META_SQL,
                params![schema::META_VECTOR_STAGING_FP, &claimed.id],
            )?;
        }
        #[cfg(test)]
        if updated == 1
            && crate::rebuild::CLAIM_PRE_COMMIT_FAIL.load(std::sync::atomic::Ordering::SeqCst)
        {
            // Injected crash after the CAS update and the binding write: the
            // whole transaction rolls back, so neither the claim nor the
            // staging binding persists (G2).
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(1),
                Some("injected claim pre-commit failure".into()),
            ));
        }
        Ok(updated)
    });
    match updated {
        Ok(1) => {
            *pending = claimed;
            true
        }
        _ => {
            // Lost the CAS (or an injected pre-commit rollback) — the marker
            // and binding are untouched; another owner may have the claim.
            false
        }
    }
}

/// Test-only hook: when set, `try_claim_rebuild` rolls back the claim + binding
/// transaction right before COMMIT (after the CAS update and the staging-binding
/// write), proving a crash can tear neither the claim nor the binding (G2).
#[cfg(test)]
pub(crate) static CLAIM_PRE_COMMIT_FAIL: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Atomically clear a stale pending marker + staging when this builder
/// observes a **completed** target (installed fingerprint == intended).
///
/// - `expected_id: Some(id)`: clears only when the stored marker still
///   references our attempt id, or references *any* attempt for the same
///   `intended_fp` (stale — the target is already installed). A marker for a
///   **different** target (newer foreign attempt) is never touched.
/// - `expected_id: None`: clears any parseable marker whose `intended` equals
///   `intended_fp` (a marker for the already-completed target is stale), and
///   drops that attempt's staging. Markers for other targets are preserved.
/// Runs in one transaction so the clear is crash-atomic (F6). A failure is
/// logged (safe: only the error; no marker/body/secret) and retried on the
/// next observe-completed pass — a stale marker is never silently dropped
/// (G5).
fn clear_completed_target(index: &MemoryIndex, expected_id: Option<&str>, intended_fp: &str) {
    let result = in_transaction(index.db(), |db| -> rusqlite::Result<()> {
        let raw: Option<String> = db
            .query_row(
                schema::GET_META_SQL,
                params![schema::META_VECTOR_REBUILD_PENDING],
                |r| r.get::<_, String>(0),
            )
            .ok();
        match raw {
            None => {
                // No marker; drop our attempt's staging rows if any.
                if let Some(id) = expected_id {
                    db.execute(
                        "DELETE FROM vector_staging WHERE pending_id = ?1",
                        params![id],
                    )?;
                }
                Ok(())
            }
            Some(r) if r.trim().is_empty() => {
                if let Some(id) = expected_id {
                    db.execute(
                        "DELETE FROM vector_staging WHERE pending_id = ?1",
                        params![id],
                    )?;
                }
                Ok(())
            }
            Some(r) => {
                let Some(p) = PendingRebuild::parse(&r) else {
                    // Corrupt/foreign marker — leave it untouched.
                    return Ok(());
                };
                let ours = expected_id.is_some_and(|id| id == p.id);
                let same_target = p.intended.is_empty() || p.intended == intended_fp;
                if !ours && !same_target {
                    // A newer foreign attempt for a different target — never
                    // clear it.
                    return Ok(());
                }
                db.execute(
                    schema::UPSERT_META_SQL,
                    params![schema::META_VECTOR_REBUILD_PENDING, ""],
                )?;
                db.execute(
                    schema::UPSERT_META_SQL,
                    params![schema::META_VECTOR_STAGING_FP, ""],
                )?;
                db.execute(
                    "DELETE FROM vector_staging WHERE pending_id = ?1",
                    params![&p.id],
                )?;
                #[cfg(test)]
                if crate::rebuild::CLEAR_PRE_COMMIT_FAIL.load(std::sync::atomic::Ordering::SeqCst) {
                    return Err(rusqlite::Error::SqliteFailure(
                        rusqlite::ffi::Error::new(1),
                        Some("injected clear pre-commit failure".into()),
                    ));
                }
                Ok(())
            }
        }
    });
    if let Err(e) = result {
        tracing::debug!(
            target: xai_grok_telemetry::memory_log::TARGET,
            error = %e,
            "failed to clear completed-target pending marker; retained and retried on next pass"
        );
    }
}

/// Test-only hook: when set, `clear_completed_target` rolls back the clearing
/// transaction right before COMMIT (after the writes), proving a failed clear
/// leaves the stale marker/staging intact and is retried (G5).
#[cfg(test)]
pub(crate) static CLEAR_PRE_COMMIT_FAIL: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Persist `last_attempt_at` on the pending marker, owner-scoped: only applies
/// when the stored marker still carries our attempt id (a superseded attempt
/// is left untouched). Drives the failure back-off.
pub fn record_attempt(index: &MemoryIndex, pending: &PendingRebuild, last_attempt_at: i64) {
    let Some(current) = pending_state(index) else {
        return;
    };
    if current.id != pending.id {
        return;
    }
    let mut updated = current;
    updated.last_attempt_at = last_attempt_at;
    let old_json = updated.to_json();
    let mut before = updated.clone();
    before.last_attempt_at = pending.last_attempt_at;
    let _ = index.db().execute(
        "UPDATE meta SET value = ?1 WHERE key = ?2 AND value = ?3",
        params![
            old_json,
            schema::META_VECTOR_REBUILD_PENDING,
            before.to_json()
        ],
    );
}

// ---------------------------------------------------------------------------
// Staging
// ---------------------------------------------------------------------------

/// Stage a computed embedding for a chunk under the attempt `pending_id`,
/// pinned to the chunk's **content hash** at the time it was embedded. A chunk
/// edited after staging changes its `chunks.hash`, so the staged row stops
/// matching and is re-embedded on the next pass.
pub fn stage_vector(
    index: &MemoryIndex,
    pending_id: &str,
    intended_fp: &str,
    chunk_id: &str,
    chunk_hash: &str,
    embedding: &[f32],
) -> Result<(), rusqlite::Error> {
    let bytes: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();
    index.db().execute(
        "INSERT OR REPLACE INTO vector_staging(pending_id, intended_fingerprint, chunk_id, chunk_hash, embedding) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![pending_id, intended_fp, chunk_id, chunk_hash, bytes],
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

/// Drop staged rows whose chunk no longer exists **or** whose content hash no
/// longer matches the chunk's current hash (edit/delete churn). Without this,
/// a deleted chunk's staged row would keep `staged_count > chunk_count`
/// forever and strand the attempt.
pub fn prune_stale_staging(index: &MemoryIndex, pending_id: &str) -> Result<(), rusqlite::Error> {
    index
        .db()
        .execute(
            "DELETE FROM vector_staging \
             WHERE pending_id = ?1 AND ( \
               NOT EXISTS (SELECT 1 FROM chunks c WHERE c.id = vector_staging.chunk_id) \
               OR chunk_hash != (SELECT c.hash FROM chunks c WHERE c.id = vector_staging.chunk_id) \
             )",
            params![pending_id],
        )
        .map(|_| ())
}

/// Whether the staged set for `pending_id` exactly matches the current live
/// chunk set **by id and content hash** (no missing, no extra, no stale hash).
///
/// Callers run this (a) cheaply before beginning an install transaction and
/// (b) again inside the install transaction so the completeness decision and
/// the vector swap share one transactional snapshot of `chunks`.
pub fn staging_complete(index: &MemoryIndex, pending_id: &str) -> Result<bool, rusqlite::Error> {
    let live: i64 = index.chunk_count();
    let staged: i64 = staged_count(index, pending_id);
    if staged != live {
        return Ok(false);
    }
    // No staged row for a chunk that no longer exists.
    let orphan: i64 = index.db().query_row(
        "SELECT COUNT(*) FROM vector_staging s \
             WHERE s.pending_id = ?1 AND NOT EXISTS ( \
               SELECT 1 FROM chunks c WHERE c.id = s.chunk_id \
             )",
        params![pending_id],
        |r| r.get(0),
    )?;
    if orphan != 0 {
        return Ok(false);
    }
    // Every live chunk has a staged row with a matching content hash.
    let missing: i64 = index.db().query_row(
        "SELECT COUNT(*) FROM chunks c \
             WHERE NOT EXISTS ( \
               SELECT 1 FROM vector_staging s \
               WHERE s.pending_id = ?1 AND s.chunk_id = c.id AND s.chunk_hash = c.hash \
             )",
        params![pending_id],
        |r| r.get(0),
    )?;
    Ok(missing == 0)
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
/// Safe **only** when the caller has verified: no pending marker exists
/// (parseable or corrupt), and either (a) the index is brand-new and empty
/// (zero chunks + zero vectors) or (b) a populated legacy set was built in the
/// same dimensions. The entire adopt (all metadata + staging clear) is one
/// transaction, so a crash mid-adopt can never leave torn metadata.
///
/// **Marker CAS (F1):** inside the `BEGIN IMMEDIATE` transaction the stored
/// pending marker is re-checked *before any mutation*; if a concurrent writer
/// created a marker since the Phase-0 read, the adopt aborts (Err) and the
/// caller must route to the rebuild path. Because `BEGIN IMMEDIATE` holds the
/// SQLite write lock, no marker can be created between this check and COMMIT.
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
    let db = index.db();
    db.execute_batch("BEGIN IMMEDIATE;")?;
    let result = (|| -> Result<(), rusqlite::Error> {
        // Re-check the pending marker under the write lock. Any non-empty
        // marker (parseable or corrupt) aborts the adopt.
        let marker: Option<String> = db
            .query_row(
                schema::GET_META_SQL,
                params![schema::META_VECTOR_REBUILD_PENDING],
                |r| r.get::<_, String>(0),
            )
            .ok();
        if marker.as_deref().is_some_and(|m| !m.trim().is_empty()) {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(1),
                Some("concurrent pending marker created; adopt aborted".into()),
            ));
        }
        meta_set_conn(db, "embedding_dimensions", &dimensions.to_string())?;
        meta_set_conn(db, schema::META_VECTOR_FINGERPRINT_HASH, &fp.hash)?;
        meta_set_conn(db, schema::META_VECTOR_FINGERPRINT, payload)?;
        meta_set_conn(
            db,
            schema::META_VECTOR_SCHEMA_VERSION,
            &v_schema.to_string(),
        )?;
        meta_set_conn(db, schema::META_VECTOR_REBUILD_PENDING, "")?;
        meta_set_conn(db, schema::META_VECTOR_STAGING_FP, "")?;
        // Clear any stale partial staging from old attempts.
        db.execute("DELETE FROM vector_staging", [])?;
        Ok(())
    })();
    if result.is_err() {
        let _ = db.execute_batch("ROLLBACK;");
        return Err(result.err().unwrap());
    }
    #[cfg(test)]
    if crate::rebuild::ADOPT_PRE_COMMIT_FAIL.load(std::sync::atomic::Ordering::SeqCst) {
        // Injected pre-commit failure (tests): the whole adopt rolls back so
        // no torn metadata is left behind and the caller must not report Ready.
        let _ = db.execute_batch("ROLLBACK;");
        return Err(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(1),
            Some("injected adopt pre-commit failure".into()),
        ));
    }
    db.execute_batch("COMMIT;")?;
    Ok(())
}

/// Test-only hook: when true, `adopt_installed` rolls back right before
/// COMMIT so tests can assert a torn adopt leaves no partial metadata and
/// never reports Ready.
#[cfg(test)]
pub(crate) static ADOPT_PRE_COMMIT_FAIL: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

// ---------------------------------------------------------------------------
// Atomic install
// ---------------------------------------------------------------------------

/// Atomically install a complete, compatible vector set + fingerprint.
///
/// `install_dimensions` may differ from the current table dimensions; the
/// `chunks_vec` virtual table is (re)created to match inside the transaction.
/// The swap is one SQLite transaction, so a crash or failure can never expose
/// partial vectors. Staging rows for the attempt are the source of truth;
/// foreign attempts are discarded at install time.
///
/// Completeness is verified **twice**: cheaply before `BEGIN`, and again
/// inside the transaction against the same transactional snapshot of
/// `chunks` (id + content hash) that the swap reads, so chunk churn can never
/// install stale/missing vectors. If churn continues, the attempt stays
/// pending (FTS-only) and the next pass converges.
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
    if !staging_complete(index, &pending.id)? {
        // Incomplete/partial — never install. Remain pending, FTS-only.
        return Ok(false);
    }

    let db = index.db();
    db.execute_batch("BEGIN IMMEDIATE;")?;
    let result = (|| -> Result<bool, rusqlite::Error> {
        // Transactional snapshot completeness (churn may have landed between
        // the pre-check and BEGIN; the write lock makes it stable now).
        if !staging_complete(index, &pending.id)? {
            return Ok(false);
        }
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
        // Never downgrade the persisted vector schema compatibility version
        // (F8): use the same max semantics as adopt.
        let v_schema = u32::max(
            index.installed_vector_schema_version(),
            fp.vector_schema_version,
        );
        meta_set_conn(
            db,
            schema::META_VECTOR_SCHEMA_VERSION,
            &v_schema.to_string(),
        )?;
        meta_set_conn(db, schema::META_VECTOR_REBUILD_PENDING, "")?;
        meta_set_conn(db, schema::META_VECTOR_STAGING_FP, "")?;
        // Discard any staging rows under stale/foreign attempts.
        db.execute("DELETE FROM vector_staging", [])?;
        Ok(true)
    })();
    match result {
        Ok(true) => {
            #[cfg(test)]
            if crate::rebuild::PRE_COMMIT_FAIL.load(std::sync::atomic::Ordering::SeqCst) {
                // Injected pre-commit failure (tests): roll the transaction
                // back so the old vector table + metadata stay hidden.
                let _ = db.execute_batch("ROLLBACK;");
                return Ok(false);
            }
            db.execute_batch("COMMIT;")?;
            Ok(true)
        }
        Ok(false) => {
            // Incomplete under the transactional snapshot: no changes made.
            let _ = db.execute_batch("ROLLBACK;");
            Ok(false)
        }
        Err(e) => {
            let _ = db.execute_batch("ROLLBACK;");
            Err(e)
        }
    }
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
    /// Installed vectors are compatible with the pinned source **and** cover
    /// every chunk — query them (`vec_row_count == chunk_count`).
    Ready,
    /// Installed vectors are compatible (a matching fingerprint proves the
    /// atomic initial migration completed) and the existing vectors remain
    /// **usable**, but some current chunks are missing vectors (`vec_row_count
    /// < chunk_count`). That is normal incremental chunk churn or a transient
    /// incremental embed failure, **not** a reason to rebuild the whole index:
    /// query the existing vectors and backfill only the `missing` current
    /// chunks via `chunks_without_embeddings` on this/next search. A torn/
    /// corrupt install (fingerprint present but dimensions/schema mismatch)
    /// is *not* reported here — it makes the source `incompatible` and falls
    /// through to a fail-closed rebuild.
    ReadyMissing { missing: usize },
    /// Rebuild pending — operate FTS-only. `owned` indicates this caller
    /// (tried to) run the rebuild this round.
    Pending { owned: bool },
    /// sqlite-vec unavailable (or no embedding source) — FTS-only.
    Disabled,
}

/// Readiness for an already-`compatible` index (installed fingerprint and
/// dimensions both match the pinned source): `Ready` when the installed vector
/// set covers every chunk, else `ReadyMissing` with the missing-row count.
/// Shared by the Phase-0 branch and every observe-completed path so a
/// compatible-partial index always stays vector-usable (never a rebuild).
fn compatible_readiness(idx: &MemoryIndex) -> VectorReadiness {
    let rows = idx.vec_row_count();
    let chunks = idx.chunk_count();
    if rows == chunks {
        VectorReadiness::Ready
    } else {
        VectorReadiness::ReadyMissing {
            missing: chunks.saturating_sub(rows) as usize,
        }
    }
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
///
/// `backoff_secs` throttles repeated failed rebuilds (they stay FTS-only
/// without burning a full rebuild per search). `max_batches_per_call` caps
/// synchronous rebuild work per invocation; a search that hits the cap returns
/// `Pending` and the next search continues.
pub async fn ensure_vectors_ready(
    db_path: &Path,
    storage: MemoryStorage,
    index_config: MemoryIndexConfig,
    spec: &EmbeddingSourceSpec,
    embedder: Option<Arc<dyn EmbeddingProvider>>,
    stale_claim_secs: i64,
    backoff_secs: i64,
    max_batches_per_call: Option<usize>,
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
    let compatible =
        installed.as_deref() == Some(fp.hash.as_str()) && installed_dims == spec.dimensions;
    match installed {
        Some(_) if compatible => {
            // A matching fingerprint proves the atomic initial migration
            // completed. Later `vec_row_count < chunk_count` is normal
            // incremental chunk churn or a transient incremental embed
            // failure — keep the existing compatible vectors **usable** and
            // backfill only the missing rows (ReadyMissing); never a full-index
            // rebuild solely for compatible missing rows (G3/R-01). A torn/
            // corrupt install (fingerprint present but dimensions/schema
            // mismatch) makes `compatible` false and falls through below to a
            // fail-closed rebuild. Clear any stale marker for this already
            // completed target (F6).
            clear_completed_target(&idx, None, fp.hash.as_str());
            return compatible_readiness(&idx);
        }
        Some(_) => { /* fingerprint/dimensions/schema mismatch (incl. torn install) -> rebuild below */
        }
        None => {
            // No fingerprint: adopt WITHOUT a rebuild ONLY when there is no
            // pending marker at all (parseable or corrupt), and the vec set is
            // either genuinely empty on a brand-new index (zero chunks, zero
            // vectors) or a provably complete same-dims legacy set.
            if !pending_marker_present(&idx) {
                let rows = idx.vec_row_count();
                let chunk_count = idx.chunk_count();
                let fresh_empty = rows == 0 && chunk_count == 0;
                let legacy_complete =
                    rows > 0 && rows == chunk_count && installed_dims == spec.dimensions;
                if (fresh_empty || legacy_complete)
                    && adopt_installed(&idx, &fp, &payload, spec.dimensions).is_ok()
                {
                    return VectorReadiness::Ready;
                }
                // Torn/failed adopt falls through to a rebuild (never Ready).
            }
            // Anything else (pending marker present, partial set, dims drift)
            // falls through to the rebuild path.
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
    // Reaching here with `installed == fp.hash` means the fingerprint matches
    // but `embedding_dimensions` does not — a torn/corrupt install invariant —
    // so it is reported as a schema mismatch and rebuilt fail-closed. A fresh
    // index that failed to adopt (no fingerprint) is "adopt_incompatible".
    let reason = if installed.is_none() {
        "adopt_incompatible"
    } else if installed.as_deref() == Some(fp.hash.as_str()) {
        "schema_mismatch"
    } else {
        "fingerprint_mismatch"
    };

    // Failure back-off: read the existing marker FIRST and, when back-off is
    // active for our target, return FTS-only without rewriting the marker or
    // discarding foreign staging (N-06 — no write amplification during
    // back-off). If the target is already installed, observe completion and
    // clear our stale marker (F6).
    let existing_marker = idx
        .meta_get(schema::META_VECTOR_REBUILD_PENDING)
        .unwrap_or_default();
    if let Some(p) = PendingRebuild::parse(&existing_marker)
        && backoff_secs > 0
        && p.last_attempt_at > 0
        && now_secs() - p.last_attempt_at < backoff_secs
        && (p.intended.is_empty() || p.intended == fp.hash)
    {
        if idx.installed_vector_fingerprint_hash().as_deref() == Some(fp.hash.as_str())
            && idx.embedding_dimensions() == spec.dimensions
        {
            clear_completed_target(&idx, Some(&p.id), fp.hash.as_str());
            return compatible_readiness(&idx);
        }
        return VectorReadiness::Pending { owned: false };
    }
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
        // Another session/process owns (or just finished) the rebuild: if it
        // already completed, observe Ready (and clear our stale marker), else
        // defer (FTS-only).
        if let Ok(idx) = open_index(
            db_path,
            storage.clone(),
            index_config.clone(),
            spec.dimensions,
        ) {
            if idx.installed_vector_fingerprint_hash().as_deref() == Some(fp.hash.as_str())
                && idx.embedding_dimensions() == spec.dimensions
            {
                clear_completed_target(&idx, Some(&pending.id), fp.hash.as_str());
                return compatible_readiness(&idx);
            }
        }
        return VectorReadiness::Pending { owned: false };
    }

    if cancel.is_cancelled() {
        return VectorReadiness::Pending { owned: true };
    }

    // Build loop. `batches_done` is a per-call budget so `max_batches_per_call`
    // bounds the synchronous rebuild work across outer iterations too.
    let mut batches_done = 0usize;
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
        // If a concurrent owner completed the same target while we were
        // preparing, observe the completed state (loser sees Ready) and clear
        // our stale marker (F6).
        if idx.installed_vector_fingerprint_hash().as_deref() == Some(fp.hash.as_str())
            && idx.embedding_dimensions() == spec.dimensions
        {
            clear_completed_target(&idx, Some(&pending.id), fp.hash.as_str());
            return compatible_readiness(&idx);
        }
        // Pending must still reference our attempt id; a superseding rebuild
        // (newer fingerprint/incarnation) must discard our stale staging.
        match pending_state(&idx) {
            Some(p) if p.id == pending.id => {}
            _ => return VectorReadiness::Pending { owned: true },
        }
        if cancel.is_cancelled() {
            return VectorReadiness::Pending { owned: true };
        }
        // Drop staged rows for deleted/changed chunks so the attempt can
        // converge under churn.
        let _ = prune_stale_staging(&idx, &pending.id);
        let needed = match idx.chunks_not_staged(&pending.id) {
            Ok(n) => n,
            Err(_) => return VectorReadiness::Pending { owned: true },
        };
        if needed.is_empty() {
            let complete = staging_complete(&idx, &pending.id).unwrap_or(false);
            if complete {
                let ok = match install_vectors(&idx, &pending, &fp, &payload, spec.dimensions) {
                    Ok(v) => v,
                    Err(_) => false,
                };
                if ok {
                    return VectorReadiness::Ready;
                }
            }
            // Incomplete under churn or failed install: record back-off and
            // remain pending (FTS-only). Never install partial/stale vectors.
            record_attempt(&idx, &pending, now_secs());
            return VectorReadiness::Pending { owned: true };
        }
        drop(idx);

        let Some(embedder) = embedder.clone() else {
            if let Ok(idx) = open_index(
                db_path,
                storage.clone(),
                index_config.clone(),
                spec.dimensions,
            ) {
                record_attempt(&idx, &pending, now_secs());
            }
            return VectorReadiness::Pending { owned: true };
        };
        // Embed bounded batches, then stage (open fresh index per batch so no
        // &index borrow crosses an await).
        for batch in needed.chunks(batch_size(embedder.as_ref())) {
            if let Some(cap) = max_batches_per_call
                && batches_done >= cap
            {
                // Cap is a normal pause (progress), NOT a failure: do not
                // record an attempt so the failure back-off stays idle and
                // the next search resumes immediately (F2).
                return VectorReadiness::Pending { owned: true };
            }
            if cancel.is_cancelled() {
                return VectorReadiness::Pending { owned: true };
            }
            let texts: Vec<&str> = batch.iter().map(|(_, t)| t.as_str()).collect();
            let vectors = match embedder.embed_batch(&texts).await {
                Ok(v) if v.len() == batch.len() => v,
                _ => {
                    if let Ok(idx) = open_index(
                        db_path,
                        storage.clone(),
                        index_config.clone(),
                        spec.dimensions,
                    ) {
                        record_attempt(&idx, &pending, now_secs());
                    }
                    return VectorReadiness::Pending { owned: true };
                }
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
            for ((cid, text), v) in batch.iter().zip(vectors.iter()) {
                let hash = super::chunker::chunk_hash(text);
                if stage_vector(
                    &idx,
                    &pending.id,
                    fp.hash.as_str(),
                    cid.as_str(),
                    &hash,
                    v.as_slice(),
                )
                .is_err()
                {
                    return VectorReadiness::Pending { owned: true };
                }
            }
            drop(idx);
            batches_done += 1;
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
            0,
            Some(usize::MAX),
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
            0,
            Some(usize::MAX),
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
            0,
            Some(usize::MAX),
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
            0,
            Some(usize::MAX),
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
            0,
            Some(usize::MAX),
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
                0,
                Some(usize::MAX),
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
                chunk.hash.as_str(),
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
            0,
            Some(usize::MAX),
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
                let hash = crate::chunker::chunk_hash(text);
                stage_vector(
                    &idx,
                    &pending.id,
                    fp_x.hash.as_str(),
                    cid.as_str(),
                    &hash,
                    &v[0],
                )
                .unwrap();
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
            0,
            Some(usize::MAX),
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
            0,
            Some(usize::MAX),
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
            0,
            Some(usize::MAX),
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
            0,
            Some(usize::MAX),
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

    // -----------------------------------------------------------------------
    // PR21 review repair tests
    // -----------------------------------------------------------------------

    fn write_note(tmp: &TempDir, name: &str, content: &str) -> std::path::PathBuf {
        let f = tmp.path().join(name);
        std::fs::write(&f, content).unwrap();
        f
    }

    fn open_index_cfg(
        db_path: &std::path::Path,
        storage: MemoryStorage,
        dims: usize,
        cfg: &xai_grok_config_types::MemoryIndexConfig,
    ) -> MemoryIndex {
        MemoryIndex::open_or_create(db_path, storage, cfg.clone(), dims)
            .unwrap_or_else(|_| panic!("open index"))
    }

    fn payload_for(spec: &crate::fingerprint::EmbeddingSourceSpec) -> String {
        VectorFingerprint::build(
            spec.clone(),
            DocPreparationSpec::from_index_config(
                &xai_grok_config_types::MemoryIndexConfig::default(),
            ),
            VECTOR_SCHEMA_VERSION,
        )
        .unwrap()
        .1
    }

    /// A1/#2: a pending marker (any, parseable or not) must block adopt — the
    /// empty-vec disjunct must never wipe in-flight staging and mark Ready.
    #[tokio::test]
    async fn test_adopt_blocked_by_pending_marker() {
        init_sqlite_vec();
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        // Fresh index; a pending marker exists for spec X (another process's
        // in-flight rebuild); no fingerprint installed yet; vec rows = 0.
        let spec_x = stub_spec(dims, "model-x");
        let fp_x = fp_for(&spec_x);
        {
            let mut idx = open_index(&db_path, storage.clone(), dims);
            let mut pending = ensure_pending(&idx, &fp_x.hash, "test").unwrap();
            assert!(try_claim_rebuild(&idx, &mut pending, 60));
            let f = write_note(&tmp, "note.md", "# Facts\n\nRust is fast.");
            idx.reindex_file(&f, "workspace").unwrap();
            // Stage one row (in-flight).
            let chunk = idx
                .get_chunk(&format!("{}:0", f.to_string_lossy()))
                .unwrap()
                .unwrap();
            let mock = MockEmbeddingProvider { dimensions: dims };
            let v = mock.embed_batch(&[&chunk.text]).await.unwrap();
            stage_vector(
                &idx,
                &pending.id,
                &fp_x.hash,
                chunk.id.as_str(),
                chunk.hash.as_str(),
                &v[0],
            )
            .unwrap();
            // Leave marker + staging in place.
        }
        // A NEW builder for a different target B must NOT adopt-and-wipe; it
        // must rebuild to completion (superseding X's attempt is correct).
        let spec_b = stub_spec(dims, "model-b");
        let fp_b = fp_for(&spec_b);
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
            0,
            Some(usize::MAX),
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(out, VectorReadiness::Ready), "{out:?}");
        let idx = open_index(&db_path, storage.clone(), dims);
        // The empty-vec adopt bug would leave 0 vectors installed while
        // reporting Ready; the rebuild must install the live chunk's vector.
        assert_eq!(idx.vec_row_count(), idx.chunk_count());
        assert_eq!(
            idx.installed_vector_fingerprint_hash().unwrap(),
            fp_b.hash,
            "fingerprint must reflect the rebuild, not an adopt wipe"
        );
    }

    /// A2/#2: a torn adopt (crash at the commit point) leaves no partial
    /// metadata and errors instead of reporting Ready.
    #[tokio::test]
    async fn test_torn_adopt_is_atomic() {
        init_sqlite_vec();
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        let idx = open_index(&db_path, storage.clone(), dims);
        let spec = stub_spec(dims, "m");
        let fp = fp_for(&spec);
        let payload = payload_for(&spec);
        ADOPT_PRE_COMMIT_FAIL.store(true, std::sync::atomic::Ordering::SeqCst);
        let r = adopt_installed(&idx, &fp, &payload, dims);
        ADOPT_PRE_COMMIT_FAIL.store(false, std::sync::atomic::Ordering::SeqCst);
        assert!(r.is_err(), "torn adopt must error");
        assert!(
            idx.installed_vector_fingerprint_hash().is_none(),
            "torn adopt must not leave a fingerprint"
        );
        assert!(
            !pending_marker_present(&idx),
            "torn adopt must not create/clear a pending marker"
        );
        let staged_left: i64 = idx
            .db()
            .query_row("SELECT COUNT(*) FROM vector_staging", [], |r| r.get(0))
            .unwrap();
        assert_eq!(staged_left, 0, "staging untouched by torn adopt");
    }

    /// A3/#4: a chunk deleted mid-rebuild no longer strands the attempt.
    #[tokio::test]
    async fn test_chunk_deleted_mid_rebuild_converges() {
        init_sqlite_vec();
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        let mut idx = open_index(&db_path, storage.clone(), dims);
        let fa = write_note(&tmp, "a.md", "# A\n\nRust a content.");
        let fb = write_note(&tmp, "b.md", "# B\n\nRust b content.");
        idx.reindex_file(&fa, "workspace").unwrap();
        idx.reindex_file(&fb, "workspace").unwrap();
        drop(idx);
        let spec_a = stub_spec(dims, "a");
        install_vectors_for(&db_path, &storage, dims, &spec_a, "a").await;

        let spec_b = crate::fingerprint::EmbeddingSourceSpec {
            model: "b".into(),
            ..spec_a.clone()
        };
        let fp_b = fp_for(&spec_b);
        // Start rebuild for B and stage BOTH chunks.
        {
            let idx = open_index(&db_path, storage.clone(), dims);
            let mut pending = ensure_pending(&idx, &fp_b.hash, "test").unwrap();
            assert!(try_claim_rebuild(&idx, &mut pending, 60));
            let mock = MockEmbeddingProvider { dimensions: dims };
            let chunks = idx.chunks_not_staged(&pending.id).unwrap();
            assert_eq!(chunks.len(), 2);
            for (cid, text) in &chunks {
                let v = mock.embed_batch(&[text.as_str()]).await.unwrap();
                let hash = crate::chunker::chunk_hash(text);
                stage_vector(&idx, &pending.id, &fp_b.hash, cid.as_str(), &hash, &v[0]).unwrap();
            }
            // Delete b.md mid-rebuild.
            let mut idx2 = open_index(&db_path, storage.clone(), dims);
            idx2.delete_path(&fb).unwrap();
        }
        // Next pass prunes the stale row and installs the surviving chunk.
        let fake = Arc::new(FakeMemoryRetrieval::new(dims, "b"));
        let embedder: Option<Arc<dyn EmbeddingProvider>> =
            Some(Arc::new(RetrievalEmbeddingProvider::new(fake.clone())));
        let out = ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &spec_b,
            embedder,
            60,
            0,
            Some(usize::MAX),
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(out, VectorReadiness::Ready), "{out:?}");
        let idx = open_index(&db_path, storage.clone(), dims);
        assert_eq!(idx.chunk_count(), 1);
        assert_eq!(idx.vec_row_count(), 1, "no stranding on chunk deletion");
        assert_eq!(idx.installed_vector_fingerprint_hash().unwrap(), fp_b.hash);
    }

    /// A3/#4: a chunk edited mid-rebuild is re-embedded over its new text.
    #[tokio::test]
    async fn test_chunk_edited_mid_rebuild_converges() {
        init_sqlite_vec();
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        let mut idx = open_index(&db_path, storage.clone(), dims);
        let fa = write_note(&tmp, "a.md", "# A\n\nRust old content.");
        idx.reindex_file(&fa, "workspace").unwrap();
        drop(idx);
        let spec_a = stub_spec(dims, "a");
        install_vectors_for(&db_path, &storage, dims, &spec_a, "a").await;

        let spec_b = crate::fingerprint::EmbeddingSourceSpec {
            model: "b".into(),
            ..spec_a.clone()
        };
        let fp_b = fp_for(&spec_b);
        {
            let idx = open_index(&db_path, storage.clone(), dims);
            let mut pending = ensure_pending(&idx, &fp_b.hash, "test").unwrap();
            assert!(try_claim_rebuild(&idx, &mut pending, 60));
            let mock = MockEmbeddingProvider { dimensions: dims };
            let chunks = idx.chunks_not_staged(&pending.id).unwrap();
            for (cid, text) in &chunks {
                let v = mock.embed_batch(&[text.as_str()]).await.unwrap();
                let hash = crate::chunker::chunk_hash(text);
                stage_vector(&idx, &pending.id, &fp_b.hash, cid.as_str(), &hash, &v[0]).unwrap();
            }
            // Edit the file mid-rebuild (hash changes).
            write_note(&tmp, "a.md", "# A\n\nRust edited content.");
            let mut idx2 = open_index(&db_path, storage.clone(), dims);
            idx2.reindex_file(&fa, "workspace").unwrap();
        }
        let fake = Arc::new(FakeMemoryRetrieval::new(dims, "b"));
        let embedder: Option<Arc<dyn EmbeddingProvider>> =
            Some(Arc::new(RetrievalEmbeddingProvider::new(fake.clone())));
        let out = ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &spec_b,
            embedder,
            60,
            0,
            Some(usize::MAX),
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(out, VectorReadiness::Ready), "{out:?}");
        let idx = open_index(&db_path, storage.clone(), dims);
        assert_eq!(idx.vec_row_count(), 1);
        assert_eq!(idx.installed_vector_fingerprint_hash().unwrap(), fp_b.hash);
        // The installed vector must be over the NEW text: a query embedding of
        // the new text matches with distance 0 (deterministic mock).
        let mock = MockEmbeddingProvider { dimensions: dims };
        let q = mock
            .embed_batch(&["# A\n\nRust edited content."])
            .await
            .unwrap();
        let hits = idx.vector_search(&q[0], 5).unwrap();
        assert!(
            !hits.is_empty() && hits[0].1 < 1e-6,
            "stale (pre-edit) vector must never be installed: {hits:?}"
        );
    }

    /// A3/#4: a chunk added mid-rebuild is embedded before install.
    #[tokio::test]
    async fn test_chunk_added_mid_rebuild_converges() {
        init_sqlite_vec();
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        let mut idx = open_index(&db_path, storage.clone(), dims);
        let fa = write_note(&tmp, "a.md", "# A\n\nRust a content.");
        idx.reindex_file(&fa, "workspace").unwrap();
        drop(idx);
        let spec_a = stub_spec(dims, "a");
        install_vectors_for(&db_path, &storage, dims, &spec_a, "a").await;

        let spec_b = crate::fingerprint::EmbeddingSourceSpec {
            model: "b".into(),
            ..spec_a.clone()
        };
        let fp_b = fp_for(&spec_b);
        {
            let idx = open_index(&db_path, storage.clone(), dims);
            let mut pending = ensure_pending(&idx, &fp_b.hash, "test").unwrap();
            assert!(try_claim_rebuild(&idx, &mut pending, 60));
            let mock = MockEmbeddingProvider { dimensions: dims };
            let chunks = idx.chunks_not_staged(&pending.id).unwrap();
            for (cid, text) in &chunks {
                let v = mock.embed_batch(&[text.as_str()]).await.unwrap();
                let hash = crate::chunker::chunk_hash(text);
                stage_vector(&idx, &pending.id, &fp_b.hash, cid.as_str(), &hash, &v[0]).unwrap();
            }
            // Add a second file mid-rebuild.
            let fc = write_note(&tmp, "c.md", "# C\n\nRust c content.");
            let mut idx2 = open_index(&db_path, storage.clone(), dims);
            idx2.reindex_file(&fc, "workspace").unwrap();
        }
        let fake = Arc::new(FakeMemoryRetrieval::new(dims, "b"));
        let embedder: Option<Arc<dyn EmbeddingProvider>> =
            Some(Arc::new(RetrievalEmbeddingProvider::new(fake.clone())));
        let out = ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &spec_b,
            embedder,
            60,
            0,
            Some(usize::MAX),
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(out, VectorReadiness::Ready), "{out:?}");
        let idx = open_index(&db_path, storage.clone(), dims);
        assert_eq!(idx.chunk_count(), 2);
        assert_eq!(
            idx.vec_row_count(),
            2,
            "added chunk must be embedded before install"
        );
        assert_eq!(idx.installed_vector_fingerprint_hash().unwrap(), fp_b.hash);
    }

    /// F-02/A4: the rebuild claim is a true CAS — only one of two identical
    /// snapshots wins.
    #[test]
    fn test_cas_claim_only_one_winner() {
        init_sqlite_vec();
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        let idx = open_index(&db_path, storage.clone(), dims);
        let spec = stub_spec(dims, "m");
        let fp = fp_for(&spec);
        let pending_a = ensure_pending(&idx, &fp.hash, "test").unwrap();
        let mut p1 = pending_a.clone();
        let mut p2 = pending_a.clone();
        assert!(try_claim_rebuild(&idx, &mut p1, 60), "first claimant wins");
        assert!(
            !try_claim_rebuild(&idx, &mut p2, 60),
            "second CAS against the same old snapshot must lose"
        );
        assert!(!p1.claim.is_empty());
        assert!(
            p2.claim.is_empty(),
            "loser must not adopt the claim locally"
        );
        let stored = pending_state(&idx).unwrap();
        assert_eq!(
            stored, p1,
            "stored marker must equal the winner's claimed state"
        );
    }

    /// A4/#3: a builder that loses the claim to a stale-window takeover still
    /// observes the completed state as Ready (not a spurious Pending).
    #[tokio::test]
    async fn test_stale_takeover_observes_completed_state() {
        init_sqlite_vec();
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        let mut idx = open_index(&db_path, storage.clone(), dims);
        let fa = write_note(&tmp, "a.md", "# A\n\nRust a content.");
        idx.reindex_file(&fa, "workspace").unwrap();
        drop(idx);
        let spec_a = stub_spec(dims, "a");
        install_vectors_for(&db_path, &storage, dims, &spec_a, "a").await;
        let spec_b = crate::fingerprint::EmbeddingSourceSpec {
            model: "b".into(),
            ..spec_a.clone()
        };
        let fp_b = fp_for(&spec_b);
        // A crashed winner left a stale foreign claim with fully staged rows.
        {
            let idx = open_index(&db_path, storage.clone(), dims);
            let mut pending = ensure_pending(&idx, &fp_b.hash, "test").unwrap();
            assert!(try_claim_rebuild(&idx, &mut pending, 60));
            let mock = MockEmbeddingProvider { dimensions: dims };
            let chunks = idx.chunks_not_staged(&pending.id).unwrap();
            for (cid, text) in &chunks {
                let v = mock.embed_batch(&[text.as_str()]).await.unwrap();
                let hash = crate::chunker::chunk_hash(text);
                stage_vector(&idx, &pending.id, &fp_b.hash, cid.as_str(), &hash, &v[0]).unwrap();
            }
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
        let fake = Arc::new(FakeMemoryRetrieval::new(dims, "b"));
        let embedder: Option<Arc<dyn EmbeddingProvider>> =
            Some(Arc::new(RetrievalEmbeddingProvider::new(fake.clone())));
        let out = ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &spec_b,
            embedder.clone(),
            60,
            0,
            Some(usize::MAX),
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(out, VectorReadiness::Ready), "{out:?}");
        // A subsequent builder (even one that loses a fresh claim) sees Ready.
        let out2 = ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &spec_b,
            embedder,
            60,
            0,
            Some(usize::MAX),
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(out2, VectorReadiness::Ready), "{out2:?}");
    }

    /// A11/#7: a corrupt (unparseable) pending marker never lets adopt bypass
    /// a required rebuild.
    #[tokio::test]
    async fn test_corrupt_pending_marker_no_adopt_bypass() {
        init_sqlite_vec();
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        {
            let idx = open_index(&db_path, storage.clone(), dims);
            meta_set_conn(
                idx.db(),
                schema::META_VECTOR_REBUILD_PENDING,
                "garbage-not-json",
            )
            .unwrap();
        }
        let mut idx = open_index(&db_path, storage.clone(), dims);
        let f = write_note(&tmp, "note.md", "# Facts\n\nRust is fast.");
        idx.reindex_file(&f, "workspace").unwrap();
        drop(idx);

        let spec = stub_spec(dims, "m");
        let fp = fp_for(&spec);
        let fake = Arc::new(FakeMemoryRetrieval::new(dims, "m"));
        let embedder: Option<Arc<dyn EmbeddingProvider>> =
            Some(Arc::new(RetrievalEmbeddingProvider::new(fake.clone())));
        let out = ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &spec,
            embedder,
            60,
            0,
            Some(usize::MAX),
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(out, VectorReadiness::Ready), "{out:?}");
        let idx = open_index(&db_path, storage.clone(), dims);
        assert_eq!(
            idx.vec_row_count(),
            idx.chunk_count(),
            "corrupt marker must not short-circuit into an empty adopt"
        );
        assert_eq!(idx.installed_vector_fingerprint_hash().unwrap(), fp.hash);
    }

    /// A12/#8: repeated failed rebuilds back off and stay FTS-only.
    #[tokio::test]
    async fn test_rebuild_backoff_stays_fts_only() {
        init_sqlite_vec();
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        let mut idx = open_index(&db_path, storage.clone(), dims);
        let fa = write_note(&tmp, "a.md", "# A\n\nRust a content.");
        idx.reindex_file(&fa, "workspace").unwrap();
        drop(idx);
        let spec_a = stub_spec(dims, "a");
        install_vectors_for(&db_path, &storage, dims, &spec_a, "a").await;
        let spec_b = crate::fingerprint::EmbeddingSourceSpec {
            model: "b".into(),
            ..spec_a.clone()
        };
        let failing: Option<Arc<dyn EmbeddingProvider>> =
            Some(Arc::new(FailingEmbeddingProvider { dims }));
        let out1 = ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &spec_b,
            failing,
            60,
            3600,
            Some(usize::MAX),
            CancellationToken::new(),
        )
        .await;
        assert!(
            matches!(out1, VectorReadiness::Pending { owned: true }),
            "{out1:?}"
        );

        // Immediate retry with a working embedder: back-off is active, so no
        // embedding work happens and the search stays FTS-only.
        let fake = Arc::new(FakeMemoryRetrieval::new(dims, "b"));
        let embedder: Option<Arc<dyn EmbeddingProvider>> =
            Some(Arc::new(RetrievalEmbeddingProvider::new(fake.clone())));
        let out2 = ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &spec_b,
            embedder.clone(),
            60,
            3600,
            Some(usize::MAX),
            CancellationToken::new(),
        )
        .await;
        assert!(
            matches!(out2, VectorReadiness::Pending { owned: false }),
            "{out2:?}"
        );
        assert_eq!(fake.embed_calls(), 0, "back-off must suppress rebuild work");

        // Back-off elapsed (0 disables it): rebuild proceeds to completion.
        let out3 = ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &spec_b,
            embedder,
            60,
            0,
            Some(usize::MAX),
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(out3, VectorReadiness::Ready), "{out3:?}");
    }

    /// #8: the per-search batch cap bounds synchronous rebuild work and a
    /// later search resumes and completes.
    #[tokio::test]
    async fn test_batch_cap_resumes() {
        init_sqlite_vec();
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        let mut idx = open_index(&db_path, storage.clone(), dims);
        for i in 0..40 {
            let f = write_note(
                &tmp,
                &format!("a{i}.md"),
                &format!("# A{i}\n\nRust content {i}."),
            );
            idx.reindex_file(&f, "workspace").unwrap();
        }
        drop(idx);
        let spec_a = stub_spec(dims, "a");
        install_vectors_for(&db_path, &storage, dims, &spec_a, "a").await;
        let spec_b = crate::fingerprint::EmbeddingSourceSpec {
            model: "b".into(),
            ..spec_a.clone()
        };
        let fake = Arc::new(FakeMemoryRetrieval::new(dims, "b"));
        let embedder: Option<Arc<dyn EmbeddingProvider>> =
            Some(Arc::new(RetrievalEmbeddingProvider::new(fake.clone())));
        let out1 = ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &spec_b,
            embedder.clone(),
            60,
            0,
            Some(1),
            CancellationToken::new(),
        )
        .await;
        assert!(
            matches!(out1, VectorReadiness::Pending { owned: true }),
            "{out1:?}"
        );
        let idx = open_index(&db_path, storage.clone(), dims);
        let pending = pending_state(&idx).unwrap();
        let staged = staged_count(&idx, &pending.id);
        assert!(
            staged > 0 && staged < 40,
            "cap must bound staged work: {staged}"
        );
        drop(idx);
        // Second search resumes and completes (8 remaining chunks in one batch).
        let out2 = ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &spec_b,
            embedder,
            60,
            0,
            Some(2),
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(out2, VectorReadiness::Ready), "{out2:?}");
        let idx = open_index(&db_path, storage.clone(), dims);
        assert_eq!(idx.vec_row_count(), 40);
    }

    /// A8/#5: a non-default doc-prep config changes the fingerprint and
    /// rebuilds, using the actual index config on both sides.
    #[tokio::test]
    async fn test_non_default_prep_change_rebuilds() {
        init_sqlite_vec();
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        let cfg_custom = xai_grok_config_types::MemoryIndexConfig {
            max_chunk_chars: 400,
            chunk_overlap_chars: 40,
            ..Default::default()
        };
        let cfg_default = xai_grok_config_types::MemoryIndexConfig::default();
        let spec = stub_spec(dims, "m");
        let fp_custom = VectorFingerprint::build(
            spec.clone(),
            DocPreparationSpec::from_index_config(&cfg_custom),
            VECTOR_SCHEMA_VERSION,
        )
        .unwrap()
        .0;
        let fp_default = VectorFingerprint::build(
            spec.clone(),
            DocPreparationSpec::from_index_config(&cfg_default),
            VECTOR_SCHEMA_VERSION,
        )
        .unwrap()
        .0;
        assert_ne!(fp_custom.hash, fp_default.hash, "prep params are identity");

        let mut idx = open_index_cfg(&db_path, storage.clone(), dims, &cfg_custom);
        let f = write_note(&tmp, "note.md", "# Facts\n\nRust is fast and memory-safe.");
        idx.reindex_file(&f, "workspace").unwrap();
        drop(idx);

        let fake = Arc::new(FakeMemoryRetrieval::new(dims, "m"));
        let embedder: Option<Arc<dyn EmbeddingProvider>> =
            Some(Arc::new(RetrievalEmbeddingProvider::new(fake.clone())));
        let out = ensure_vectors_ready(
            &db_path,
            storage.clone(),
            cfg_custom.clone(),
            &spec,
            embedder.clone(),
            60,
            0,
            Some(usize::MAX),
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(out, VectorReadiness::Ready), "{out:?}");
        let idx = open_index_cfg(&db_path, storage.clone(), dims, &cfg_custom);
        assert_eq!(
            idx.installed_vector_fingerprint_hash().unwrap(),
            fp_custom.hash
        );
        drop(idx);

        // Same source, default prep => fingerprint differs => rebuild.
        let out2 = ensure_vectors_ready(
            &db_path,
            storage.clone(),
            cfg_default.clone(),
            &spec,
            embedder,
            60,
            0,
            Some(usize::MAX),
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(out2, VectorReadiness::Ready), "{out2:?}");
        let idx = open_index_cfg(&db_path, storage.clone(), dims, &cfg_default);
        assert_eq!(
            idx.installed_vector_fingerprint_hash().unwrap(),
            fp_default.hash,
            "prep change must rebuild to the new fingerprint"
        );
    }

    /// A10/#7: a DB written by a newer schema version fails closed (FTS-only)
    /// without destructive writes.
    #[tokio::test]
    async fn test_schema_newer_version_fails_closed() {
        init_sqlite_vec();
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        let mut idx = open_index(&db_path, storage.clone(), dims);
        let f = write_note(&tmp, "note.md", "# Facts\n\nRust is fast.");
        idx.reindex_file(&f, "workspace").unwrap();
        // Simulate a newer writer bumping the schema version.
        meta_set_conn(
            idx.db(),
            schema::META_SCHEMA_VERSION,
            &(schema::SCHEMA_VERSION + 100).to_string(),
        )
        .unwrap();
        drop(idx);

        let idx = open_index(&db_path, storage.clone(), dims);
        assert!(!idx.vec_available(), "newer-schema DB must open FTS-only");
        assert_eq!(idx.chunk_count(), 1, "chunks must survive untouched");
        assert!(
            idx.installed_vector_fingerprint_hash().is_none(),
            "no fingerprint writes against a newer-schema DB"
        );
    }

    // -----------------------------------------------------------------------
    // PR21 re-review repair tests (F1-F10, N-01/N-06)
    // -----------------------------------------------------------------------

    /// F1: adopt re-checks the pending marker inside its transaction — a
    /// concurrently created marker aborts the adopt (deterministic via two
    /// connections).
    #[tokio::test]
    async fn test_adopt_cas_aborts_on_concurrent_marker() {
        init_sqlite_vec();
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        // Connection A: fresh db, passes a Phase-0-style no-marker read.
        let idx_a = open_index(&db_path, storage.clone(), dims);
        // Connection B commits a pending marker (the race window).
        {
            let idx_b = open_index(&db_path, storage.clone(), dims);
            let spec_x = stub_spec(dims, "x");
            let fp_x = fp_for(&spec_x);
            ensure_pending(&idx_b, &fp_x.hash, "concurrent").unwrap();
        }
        // A's adopt must abort on the marker CAS.
        let spec_a = stub_spec(dims, "a");
        let fp_a = fp_for(&spec_a);
        let payload_a = payload_for(&spec_a);
        let r = adopt_installed(&idx_a, &fp_a, &payload_a, dims);
        assert!(r.is_err(), "concurrent marker must abort the adopt");
        assert!(
            idx_a.installed_vector_fingerprint_hash().is_none(),
            "aborted adopt must not install a fingerprint"
        );
        assert!(
            pending_marker_present(&idx_a),
            "concurrent marker must survive the aborted adopt"
        );
    }

    /// F2/N-01: a batch-cap pause is progress, not failure — it must NOT arm
    /// the failure back-off; the next search (with the production back-off
    /// value) resumes immediately and completes.
    #[tokio::test]
    async fn test_cap_pause_does_not_back_off() {
        init_sqlite_vec();
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        let mut idx = open_index(&db_path, storage.clone(), dims);
        for i in 0..40 {
            let f = write_note(
                &tmp,
                &format!("a{i}.md"),
                &format!("# A{i}\n\nRust content {i}."),
            );
            idx.reindex_file(&f, "workspace").unwrap();
        }
        drop(idx);
        let spec_a = stub_spec(dims, "a");
        install_vectors_for(&db_path, &storage, dims, &spec_a, "a").await;
        let spec_b = crate::fingerprint::EmbeddingSourceSpec {
            model: "b".into(),
            ..spec_a.clone()
        };
        let fake = Arc::new(FakeMemoryRetrieval::new(dims, "b"));
        let embedder: Option<Arc<dyn EmbeddingProvider>> =
            Some(Arc::new(RetrievalEmbeddingProvider::new(fake.clone())));
        // First search with production back-off 60s: cap pauses after 32.
        let out1 = ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &spec_b,
            embedder.clone(),
            60,
            60,
            Some(1),
            CancellationToken::new(),
        )
        .await;
        assert!(
            matches!(out1, VectorReadiness::Pending { owned: true }),
            "{out1:?}"
        );
        let idx = open_index(&db_path, storage.clone(), dims);
        let pending = pending_state(&idx).unwrap();
        assert_eq!(
            pending.last_attempt_at, 0,
            "cap pause must not record a failure attempt (no back-off)"
        );
        let staged = staged_count(&idx, &pending.id);
        assert!(staged > 0 && staged < 40, "cap bounds progress: {staged}");
        drop(idx);
        // Next search with the same 60s back-off resumes immediately and
        // completes (back-off must not suppress a healthy rebuild).
        let out2 = ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &spec_b,
            embedder,
            60,
            60,
            Some(2),
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(out2, VectorReadiness::Ready), "{out2:?}");
        let idx = open_index(&db_path, storage.clone(), dims);
        assert_eq!(idx.vec_row_count(), 40);
    }

    /// F6: observing a completed target clears the stale marker + staging.
    #[tokio::test]
    async fn test_observe_completed_clears_stale_marker() {
        init_sqlite_vec();
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        let fa = write_note(&tmp, "a.md", "# A\n\nRust a content.");
        let mut idx = open_index(&db_path, storage.clone(), dims);
        idx.reindex_file(&fa, "workspace").unwrap();
        drop(idx);
        let spec_a = stub_spec(dims, "a");
        install_vectors_for(&db_path, &storage, dims, &spec_a, "a").await;
        let spec_b = crate::fingerprint::EmbeddingSourceSpec {
            model: "b".into(),
            ..spec_a.clone()
        };
        let fp_b = fp_for(&spec_b);
        let pending;
        // A "winner" builds and installs B, clearing the marker.
        {
            let idx = open_index(&db_path, storage.clone(), dims);
            let mut p = ensure_pending(&idx, &fp_b.hash, "test").unwrap();
            assert!(try_claim_rebuild(&idx, &mut p, 60));
            let mock = MockEmbeddingProvider { dimensions: dims };
            let chunks = idx.chunks_not_staged(&p.id).unwrap();
            for (cid, text) in &chunks {
                let v = mock.embed_batch(&[text.as_str()]).await.unwrap();
                let hash = crate::chunker::chunk_hash(text);
                stage_vector(&idx, &p.id, &fp_b.hash, cid.as_str(), &hash, &v[0]).unwrap();
            }
            let payload_b = payload_for(&spec_b);
            assert!(install_vectors(&idx, &p, &fp_b, &payload_b, dims).unwrap());
            pending = p;
        }
        // Re-arm a stale marker for OUR attempt id with staging.
        {
            let idx = open_index(&db_path, storage.clone(), dims);
            let mut stale = pending.clone();
            stale.claim = String::new();
            stale.claimed_at = 0;
            stale.status = "pending".into();
            meta_set_conn(
                idx.db(),
                schema::META_VECTOR_REBUILD_PENDING,
                &stale.to_json(),
            )
            .unwrap();
            let mock = MockEmbeddingProvider { dimensions: dims };
            let v = mock.embed_batch(&["# A\n\nRust a content."]).await.unwrap();
            stage_vector(
                &idx,
                &pending.id,
                &fp_b.hash,
                &format!("{}:0", fa.to_string_lossy()),
                "stale-hash",
                &v[0],
            )
            .unwrap();
        }
        // A builder for B observes the completed target and clears stale state.
        let fake = Arc::new(FakeMemoryRetrieval::new(dims, "b"));
        let embedder: Option<Arc<dyn EmbeddingProvider>> =
            Some(Arc::new(RetrievalEmbeddingProvider::new(fake.clone())));
        let out = ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &spec_b,
            embedder,
            60,
            0,
            Some(usize::MAX),
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(out, VectorReadiness::Ready), "{out:?}");
        let idx = open_index(&db_path, storage.clone(), dims);
        assert!(
            !pending_marker_present(&idx),
            "stale marker must be cleared on observe-completed"
        );
        let staged_left: i64 = idx
            .db()
            .query_row("SELECT COUNT(*) FROM vector_staging", [], |r| r.get(0))
            .unwrap();
        assert_eq!(staged_left, 0, "stale staging must be cleared");
    }

    /// F6: a foreign marker for a different target is preserved when we
    /// observe our completed target.
    #[tokio::test]
    async fn test_observe_completed_preserves_foreign_marker() {
        init_sqlite_vec();
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        let fa = write_note(&tmp, "a.md", "# A\n\nRust a content.");
        let mut idx = open_index(&db_path, storage.clone(), dims);
        idx.reindex_file(&fa, "workspace").unwrap();
        drop(idx);
        let spec_a = stub_spec(dims, "a");
        install_vectors_for(&db_path, &storage, dims, &spec_a, "a").await;
        let spec_b = crate::fingerprint::EmbeddingSourceSpec {
            model: "b".into(),
            ..spec_a.clone()
        };
        let fp_b = fp_for(&spec_b);
        // Install B (winner clears its marker).
        {
            let idx = open_index(&db_path, storage.clone(), dims);
            let mut p = ensure_pending(&idx, &fp_b.hash, "test").unwrap();
            assert!(try_claim_rebuild(&idx, &mut p, 60));
            let mock = MockEmbeddingProvider { dimensions: dims };
            let chunks = idx.chunks_not_staged(&p.id).unwrap();
            for (cid, text) in &chunks {
                let v = mock.embed_batch(&[text.as_str()]).await.unwrap();
                let hash = crate::chunker::chunk_hash(text);
                stage_vector(&idx, &p.id, &fp_b.hash, cid.as_str(), &hash, &v[0]).unwrap();
            }
            let payload_b = payload_for(&spec_b);
            assert!(install_vectors(&idx, &p, &fp_b, &payload_b, dims).unwrap());
        }
        // A NEWER foreign attempt for a DIFFERENT target C leaves a marker +
        // staging.
        let spec_c = crate::fingerprint::EmbeddingSourceSpec {
            model: "c".into(),
            ..spec_a.clone()
        };
        let fp_c = fp_for(&spec_c);
        {
            let idx = open_index(&db_path, storage.clone(), dims);
            let p = ensure_pending(&idx, &fp_c.hash, "foreign").unwrap();
            let mock = MockEmbeddingProvider { dimensions: dims };
            let v = mock.embed_batch(&["# A\n\nRust a content."]).await.unwrap();
            stage_vector(
                &idx,
                &p.id,
                &fp_c.hash,
                &format!("{}:0", fa.to_string_lossy()),
                "h",
                &v[0],
            )
            .unwrap();
        }
        // A builder for B observes completion; the foreign C marker must
        // survive.
        let fake = Arc::new(FakeMemoryRetrieval::new(dims, "b"));
        let embedder: Option<Arc<dyn EmbeddingProvider>> =
            Some(Arc::new(RetrievalEmbeddingProvider::new(fake.clone())));
        let out = ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &spec_b,
            embedder,
            60,
            0,
            Some(usize::MAX),
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(out, VectorReadiness::Ready), "{out:?}");
        let idx = open_index(&db_path, storage.clone(), dims);
        let raw = idx
            .meta_get(schema::META_VECTOR_REBUILD_PENDING)
            .unwrap_or_default();
        let p = PendingRebuild::parse(&raw).expect("foreign marker must survive");
        assert_eq!(
            p.intended, fp_c.hash,
            "foreign marker for a different target must be preserved"
        );
        let staged_left: i64 = idx
            .db()
            .query_row("SELECT COUNT(*) FROM vector_staging", [], |r| r.get(0))
            .unwrap();
        assert_eq!(staged_left, 1, "foreign staging must be preserved");
    }

    /// F3: a newer-schema DB is opened vec-disabled BEFORE any schema DDL —
    /// sentinel objects/metadata are untouched and no migration runs.
    #[test]
    fn test_schema_newer_sentinel_untouched() {
        init_sqlite_vec();
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        // Manually build an old-schema DB with a NEWER schema_version.
        {
            std::fs::create_dir_all(db_path.parent().unwrap()).unwrap();
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            let schema_version = schema::SCHEMA_VERSION + 100;
            conn.execute_batch(&format!(
                "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 CREATE TABLE chunks (rowid INTEGER PRIMARY KEY, id TEXT, path TEXT, start_line INTEGER, end_line INTEGER, text TEXT, hash TEXT, source TEXT, created_at INTEGER, updated_at INTEGER);
                 CREATE VIRTUAL TABLE chunks_fts USING fts5(text, content='');
                 CREATE TABLE vector_staging (pending_id TEXT, intended_fingerprint TEXT, chunk_id TEXT, embedding BLOB, PRIMARY KEY (pending_id, chunk_id));
                 CREATE TABLE zz_sentinel (x INTEGER);
                 INSERT INTO meta VALUES ('schema_version', '{schema_version}');
                 INSERT INTO meta VALUES ('zz_sentinel_key', 'keep');"
            ))
            .unwrap();
        }
        let idx = open_index(&db_path, storage.clone(), 4);
        assert!(
            !idx.vec_available(),
            "newer-schema DB must open vec-disabled"
        );
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        // ALTER migration did not run.
        let cols: Vec<String> = conn
            .prepare("PRAGMA table_info(vector_staging)")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(
            !cols.iter().any(|c| c == "chunk_hash"),
            "ALTER must not run on a newer-schema DB"
        );
        // schema_sql did not run (no new meta default keys).
        let fp: Option<String> = conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'vector_fingerprint_hash'",
                [],
                |r| r.get(0),
            )
            .ok();
        assert!(fp.is_none(), "schema_sql defaults must not be written");
        // Sentinel objects intact.
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM zz_sentinel", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
        let sentinel: String = conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'zz_sentinel_key'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(sentinel, "keep");
        // chunks table still queryable (FTS-only reads).
        let c: i64 = conn
            .query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(c, 0);
    }

    // -----------------------------------------------------------------------
    // PR21 final re-review (G1/G3/G4/G5 + R-01): compatible-partial state
    // -----------------------------------------------------------------------

    /// G1/G3/R-01: a matching fingerprint with one missing current chunk
    /// returns `ReadyMissing` (existing vectors stay usable) — *not* a full
    /// rebuild — and backend backfills only that missing row, preserving the
    /// compatible fingerprint.
    #[tokio::test]
    async fn test_compatible_partial_backfills_missing_only() {
        init_sqlite_vec();
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        let mut idx = open_index(&db_path, storage.clone(), dims);
        let fa = write_note(&tmp, "a.md", "# A\n\nRust a content.");
        idx.reindex_file(&fa, "workspace").unwrap();
        drop(idx);
        let spec = stub_spec(dims, "m");
        install_vectors_for(&db_path, &storage, dims, &spec, "m").await;

        // Add chunk B without embedding it (normal incremental churn / a
        // transient incremental embed failure): matching fp, 2 chunks, 1 row.
        let mut idx = open_index(&db_path, storage.clone(), dims);
        let fb = write_note(&tmp, "b.md", "# B\n\nRust b content.");
        idx.reindex_file(&fb, "workspace").unwrap();
        assert_eq!(
            idx.vec_row_count(),
            1,
            "new chunk must not be auto-embedded"
        );
        assert_eq!(idx.chunk_count(), 2);
        assert_eq!(
            idx.chunks_without_embeddings().unwrap().len(),
            1,
            "exactly chunk B is missing vectors"
        );
        drop(idx);

        // Next search: NO rebuild — existing compatible vectors usable, only
        // the missing chunk reported for backfill.
        let fake = Arc::new(FakeMemoryRetrieval::new(dims, "m"));
        let embedder: Option<Arc<dyn EmbeddingProvider>> =
            Some(Arc::new(RetrievalEmbeddingProvider::new(fake.clone())));
        let out = ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &spec,
            embedder.clone(),
            60,
            0,
            Some(usize::MAX),
            CancellationToken::new(),
        )
        .await;
        assert!(
            matches!(out, VectorReadiness::ReadyMissing { missing: 1 }),
            "{out:?}"
        );
        assert_eq!(
            fake.embed_calls(),
            0,
            "ReadyMissing must not re-embed (no full rebuild)"
        );
        assert!(
            !pending_marker_present(&open_index(&db_path, storage.clone(), dims)),
            "ReadyMissing must not arm a pending marker"
        );

        // Backend backfills ONLY the missing current chunk.
        let idx = open_index(&db_path, storage.clone(), dims);
        let missing = idx.chunks_without_embeddings().unwrap();
        assert_eq!(missing.len(), 1, "backfill is bounded to the missing row");
        let (cid, text) = missing.into_iter().next().unwrap();
        let mock = Arc::new(MockEmbeddingProvider { dimensions: dims });
        let v = mock.embed_batch(&[text.as_str()]).await.unwrap();
        idx.upsert_embedding(&cid, &v[0]).unwrap();
        assert_eq!(idx.vec_row_count(), 2);
        drop(idx);

        // Next search: compatible + complete => Ready, fingerprint preserved.
        let out2 = ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &spec,
            embedder,
            60,
            0,
            Some(usize::MAX),
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(out2, VectorReadiness::Ready), "{out2:?}");
        let idx = open_index(&db_path, storage.clone(), dims);
        assert_eq!(idx.vec_row_count(), 2);
        assert_eq!(
            idx.installed_vector_fingerprint_hash().as_deref(),
            Some(fp_for(&spec).hash.as_str()),
            "compatible fingerprint preserved, not reinstalled"
        );
    }

    /// R-01: after a transient incremental embed failure (a caller-side
    /// failing backfill leaves the gap), the next search reports `ReadyMissing`
    /// (vector search stays active — never FTS-only, never a full rebuild) and
    /// heals the gap once the embedder recovers.
    #[tokio::test]
    async fn test_transient_incremental_failure_heals_next_search() {
        init_sqlite_vec();
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        let mut idx = open_index(&db_path, storage.clone(), dims);
        let fa = write_note(&tmp, "a.md", "# A\n\nRust a content.");
        idx.reindex_file(&fa, "workspace").unwrap();
        drop(idx);
        let spec = stub_spec(dims, "m");
        install_vectors_for(&db_path, &storage, dims, &spec, "m").await;

        // Add chunk B; its incremental embed fails transiently, leaving the
        // index with a matching fingerprint, 2 chunks, 1 vector row.
        let mut idx = open_index(&db_path, storage.clone(), dims);
        let fb = write_note(&tmp, "b.md", "# B\n\nRust b content.");
        idx.reindex_file(&fb, "workspace").unwrap();
        let missing = idx.chunks_without_embeddings().unwrap();
        assert_eq!(missing.len(), 1);
        let failing = Arc::new(FailingEmbeddingProvider { dims });
        let res = failing.embed_batch(&[missing[0].1.as_str()]).await;
        assert!(res.is_err(), "incremental embed fails transiently");
        drop(idx);

        // Next search with a HEALTHY embedder: ReadyMissing — the index stays
        // vector-active (not FTS-only, not a rebuild).
        let healthy = Arc::new(FakeMemoryRetrieval::new(dims, "m"));
        let embedder: Option<Arc<dyn EmbeddingProvider>> =
            Some(Arc::new(RetrievalEmbeddingProvider::new(healthy.clone())));
        let out = ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &spec,
            embedder.clone(),
            60,
            0,
            Some(usize::MAX),
            CancellationToken::new(),
        )
        .await;
        assert!(
            matches!(out, VectorReadiness::ReadyMissing { missing: 1 }),
            "must stay compatible-partial, not rebuild: {out:?}"
        );
        assert!(
            !matches!(out, VectorReadiness::Pending { .. })
                && !matches!(out, VectorReadiness::Disabled),
            "vector search must stay active"
        );
        assert_eq!(healthy.embed_calls(), 0, "no full-index rebuild embed");

        // Incremental backfill succeeds now → next search is Ready.
        let idx = open_index(&db_path, storage.clone(), dims);
        let missing = idx.chunks_without_embeddings().unwrap();
        assert_eq!(missing.len(), 1);
        let (cid, text) = missing.into_iter().next().unwrap();
        let mock = Arc::new(MockEmbeddingProvider { dimensions: dims });
        let v = mock.embed_batch(&[text.as_str()]).await.unwrap();
        idx.upsert_embedding(&cid, &v[0]).unwrap();
        drop(idx);

        let out2 = ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &spec,
            embedder,
            60,
            0,
            Some(usize::MAX),
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(out2, VectorReadiness::Ready), "{out2:?}");
        let idx = open_index(&db_path, storage.clone(), dims);
        assert_eq!(idx.vec_row_count(), 2);
        assert_eq!(idx.vec_row_count(), idx.chunk_count());
    }

    /// G4/R-01: a non-empty index with NO installed fingerprint is not yet
    /// compatible — it must atomically rebuild (install the fingerprint) before
    /// vectors are ready, and can never be reported `ReadyMissing` (which
    /// requires a matching installed fingerprint).
    #[tokio::test]
    async fn test_no_partial_initial_fingerprint_is_ready() {
        init_sqlite_vec();
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        let mut idx = open_index(&db_path, storage.clone(), dims);
        let fa = write_note(&tmp, "a.md", "# A\n\nRust a content.");
        idx.reindex_file(&fa, "workspace").unwrap();
        assert_eq!(idx.chunk_count(), 1);
        assert_eq!(idx.vec_row_count(), 0);
        assert!(idx.installed_vector_fingerprint_hash().is_none());
        drop(idx);

        let spec = stub_spec(dims, "m");
        let fake = Arc::new(FakeMemoryRetrieval::new(dims, "m"));
        let embedder: Option<Arc<dyn EmbeddingProvider>> =
            Some(Arc::new(RetrievalEmbeddingProvider::new(fake.clone())));
        let out = ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &spec,
            embedder,
            60,
            0,
            Some(usize::MAX),
            CancellationToken::new(),
        )
        .await;
        // Atomic initial migration: only Ready after the install completes,
        // and never ReadyMissing (no fingerprint was installed to match).
        assert!(matches!(out, VectorReadiness::Ready), "{out:?}");
        let idx = open_index(&db_path, storage.clone(), dims);
        assert_eq!(
            idx.vec_row_count(),
            idx.chunk_count(),
            "complete after rebuild"
        );
        assert_eq!(idx.vec_row_count(), 1);
        assert_eq!(
            idx.installed_vector_fingerprint_hash().as_deref(),
            Some(fp_for(&spec).hash.as_str()),
            "fingerprint installed atomically before Ready"
        );
    }

    /// G4: a brand-new index (zero chunks, zero vectors, no fingerprint)
    /// adopts the canonical fingerprint with zero embedding work and reports
    /// Ready — the deliberate initial-migration path.
    #[tokio::test]
    async fn test_fresh_empty_index_adopts_zero_chunks() {
        init_sqlite_vec();
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        let spec = stub_spec(dims, "m");
        let fake = Arc::new(FakeMemoryRetrieval::new(dims, "m"));
        let embedder: Option<Arc<dyn EmbeddingProvider>> =
            Some(Arc::new(RetrievalEmbeddingProvider::new(fake.clone())));
        let out = ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &spec,
            embedder,
            60,
            0,
            Some(usize::MAX),
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(out, VectorReadiness::Ready), "{out:?}");
        assert_eq!(fake.embed_calls(), 0, "empty adopt must not embed");
        let idx = open_index(&db_path, storage.clone(), dims);
        assert_eq!(idx.vec_row_count(), 0);
        assert_eq!(idx.chunk_count(), 0);
        assert_eq!(
            idx.installed_vector_fingerprint_hash().as_deref(),
            Some(fp_for(&spec).hash.as_str()),
            "empty adopt persists the canonical fingerprint"
        );
    }

    /// G2: a crash between the claim CAS / staging-binding write and COMMIT
    /// tears neither — the marker stays unclaimed and the binding stays the
    /// pending id; a retry without the crash wins.
    #[test]
    fn test_claim_binding_rolls_back_on_crash() {
        init_sqlite_vec();
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        let idx = open_index(&db_path, storage.clone(), dims);
        let spec = stub_spec(dims, "m");
        let fp_x = fp_for(&spec);
        let mut pending = ensure_pending(&idx, &fp_x.hash, "test").unwrap();
        let pre = pending.clone();
        assert!(pending.claim.is_empty(), "precondition: unclaimed");

        crate::rebuild::CLAIM_PRE_COMMIT_FAIL.store(true, std::sync::atomic::Ordering::SeqCst);
        let won = try_claim_rebuild(&idx, &mut pending, 60);
        crate::rebuild::CLAIM_PRE_COMMIT_FAIL.store(false, std::sync::atomic::Ordering::SeqCst);
        assert!(!won, "injected crash must tear the claim attempt");

        // Neither the claim fields nor the staging binding tore.
        let st = pending_state(&idx).unwrap();
        assert_eq!(st.claim, pre.claim, "claim must not persist on rollback");
        assert_eq!(st.claimed_at, pre.claimed_at, "claimed_at must not persist");
        assert_eq!(st.status, pre.status, "status must not flip to running");
        assert!(st.claim.is_empty(), "marker stays unclaimed after rollback");
        let binding = idx
            .meta_get(schema::META_VECTOR_STAGING_FP)
            .unwrap_or_default();
        assert_eq!(
            binding, pre.id,
            "staging binding must remain the pending id"
        );

        // Retry without the hook: the CAS still matches (rolled-back) state.
        assert!(try_claim_rebuild(&idx, &mut pending, 60), "retry must win");
        let st = pending_state(&idx).unwrap();
        assert!(!st.claim.is_empty(), "claim persisted on success");
        assert_eq!(st.status, "running");
        let binding = idx
            .meta_get(schema::META_VECTOR_STAGING_FP)
            .unwrap_or_default();
        assert_eq!(binding, st.id, "binding matches the claimed attempt id");
    }

    /// G5: a failed `clear_completed_target` is logged, retains the stale
    /// marker + staging (safe), and the next pass clears both.
    #[tokio::test]
    async fn test_clear_completed_target_failure_retains_and_retries() {
        init_sqlite_vec();
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        let mut idx = open_index(&db_path, storage.clone(), dims);
        let spec = stub_spec(dims, "m");
        let fp_hash = fp_for(&spec).hash;
        let f = write_note(&tmp, "a.md", "# A\n\nRust a lock.");
        idx.reindex_file(&f, "workspace").unwrap();
        let pending = ensure_pending(&idx, &fp_hash, "test").unwrap();
        let chunk = idx
            .get_chunk(&format!("{}:0", f.to_string_lossy()))
            .unwrap()
            .unwrap();
        let mock = MockEmbeddingProvider { dimensions: dims };
        let v = mock.embed_batch(&[chunk.text.as_str()]).await.unwrap();
        stage_vector(
            &idx,
            &pending.id,
            &fp_hash,
            chunk.id.as_str(),
            chunk.hash.as_str(),
            &v[0],
        )
        .unwrap();

        // Inject a crash on the clear; marker + staging survive (retryable).
        crate::rebuild::CLEAR_PRE_COMMIT_FAIL.store(true, std::sync::atomic::Ordering::SeqCst);
        clear_completed_target(&idx, Some(&pending.id), &fp_hash);
        crate::rebuild::CLEAR_PRE_COMMIT_FAIL.store(false, std::sync::atomic::Ordering::SeqCst);
        assert!(
            pending_marker_present(&idx),
            "failed clear must retain the marker"
        );
        assert!(
            staged_count(&idx, &pending.id) > 0,
            "failed clear must retain staging"
        );

        // Next pass succeeds.
        clear_completed_target(&idx, Some(&pending.id), &fp_hash);
        assert!(!pending_marker_present(&idx), "retry must clear the marker");
        assert_eq!(
            staged_count(&idx, &pending.id),
            0,
            "retry must drain staging"
        );
    }
}
