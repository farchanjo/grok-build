//! Collection-local vector rebuild for the Prime metadata index.
//!
//! Skills and callable_agents keep independent pending markers, claims,
//! staging rows, and vec0 tables. A skills rebuild never drops or blocks
//! callable_agents, and vice versa.
//!
//! A [`rusqlite::Connection`] is never held across `.await`. Each phase
//! opens a fresh [`super::MetadataIndex`] and drops it before embedding.

use std::path::Path;
use std::sync::Arc;

use rusqlite::params;
use tokio_util::sync::CancellationToken;

use super::{
    CollectionKind, MetadataIndex, MetadataIndexError, metadata_doc_prep, schema, table_exists,
};
use crate::embedding::{EmbeddingProvider, l2_normalize_v1, validate_embedding_batch};
use crate::fingerprint::{
    EmbeddingSourceSpec, NORMALIZATION_L2_V1, VECTOR_SCHEMA_VERSION, VectorFingerprint,
};
use crate::rebuild::VectorReadiness;

pub const DEFAULT_BATCH: usize = 32;
const STALE_CLAIM_DEFAULT: i64 = 60;

/// Durable pending rebuild marker stored on one collection row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionPending {
    pub id: String,
    pub intended: String,
    pub status: String,
    pub claim: String,
    pub claimed_at: i64,
    pub reason: String,
    pub last_attempt_at: i64,
}

impl CollectionPending {
    pub(crate) fn to_json(&self) -> String {
        // Keep the historical key order so CAS against previously written
        // markers still matches after an upgrade. String fields go through
        // serde_json so quotes and backslashes round-trip.
        format!(
            "{{\"id\":{},\"intended\":{},\"status\":{},\"claim\":{},\"claimed_at\":{},\"reason\":{},\"last_attempt_at\":{}}}",
            json_string(&self.id),
            json_string(&self.intended),
            json_string(&self.status),
            json_string(&self.claim),
            self.claimed_at,
            json_string(&self.reason),
            self.last_attempt_at
        )
    }

    fn parse(raw: &str) -> Option<Self> {
        if raw.trim().is_empty() {
            return None;
        }
        let v: serde_json::Value = serde_json::from_str(raw).ok()?;
        Some(CollectionPending {
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
            last_attempt_at: v
                .get("last_attempt_at")
                .and_then(|x| x.as_i64())
                .unwrap_or(0),
        })
    }
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

fn new_attempt_id() -> String {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let now = now_secs();
    format!("{now:x}-{}-{n:x}", std::process::id())
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Same-process ownership is PID *equality* after `split_once(':')`.
///
/// Prefix matching on `pid` digits would let owner `12` look like PID
/// `1` and owner `123` look like PID `12`. Parse the owner field as
/// `u32` and compare numerically. A claim without a colon, or with a
/// non-integer owner, is never treated as same-process.
pub(crate) fn claim_owned_by_pid(claim: &str, pid: u32) -> bool {
    let Some((owner, _)) = claim.split_once(':') else {
        return false;
    };
    owner.parse::<u32>().is_ok_and(|owner_pid| owner_pid == pid)
}

/// Unclaimed, stale, or owned by `pid` may be reclaimed. Fresh foreign
/// owners are not, even when their PID string is a prefix of ours or
/// vice versa.
pub(crate) fn claim_is_reclaimable(
    claim: &str,
    claimed_at: i64,
    now: i64,
    stale_secs: i64,
    pid: u32,
) -> bool {
    claim.is_empty() || claimed_at < now - stale_secs || claim_owned_by_pid(claim, pid)
}

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

pub fn pending_state(
    index: &MetadataIndex,
    collection: CollectionKind,
) -> Option<CollectionPending> {
    let raw = index.collection_state(collection).ok()?.pending_json;
    CollectionPending::parse(&raw)
}

pub fn pending_marker_present(index: &MetadataIndex, collection: CollectionKind) -> bool {
    index
        .collection_state(collection)
        .ok()
        .map(|s| !s.pending_json.trim().is_empty())
        .unwrap_or(false)
}

pub fn ensure_pending(
    index: &MetadataIndex,
    collection: CollectionKind,
    intended_fp: &str,
    reason: &str,
) -> Result<CollectionPending, rusqlite::Error> {
    index.require_writable().map_err(|_| {
        rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(1),
            Some("metadata index is read-only".into()),
        )
    })?;
    let existing = index
        .collection_state(collection)
        .ok()
        .map(|s| s.pending_json)
        .unwrap_or_default();
    let fresh = match CollectionPending::parse(&existing) {
        Some(p) if p.intended.is_empty() || p.intended == intended_fp => p,
        _ => CollectionPending {
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
    let ready_json = ready.to_json();
    let id = ready.id.clone();
    in_transaction(index.db(), |db| {
        db.execute(
            "UPDATE collections SET pending_json = ?1, staging_fp = ?2 WHERE name = ?3",
            params![&ready_json, &id, collection.as_str()],
        )
        .map(|_| ())
    })?;
    Ok(ready)
}

/// Compare-and-swap collection-local rebuild claim.
pub fn try_claim_rebuild(
    index: &MetadataIndex,
    collection: CollectionKind,
    pending: &mut CollectionPending,
    stale_secs: i64,
) -> bool {
    if !index.writable() {
        return false;
    }
    let now = now_secs();
    let pid = std::process::id();
    let claim_value = format!("{pid}:{now}");
    if !claim_is_reclaimable(&pending.claim, pending.claimed_at, now, stale_secs, pid) {
        return false;
    }

    let old_json = pending.to_json();
    let mut claimed = pending.clone();
    claimed.claim = claim_value;
    claimed.claimed_at = now;
    claimed.status = "running".into();
    let new_json = claimed.to_json();

    let updated = in_transaction(index.db(), |db| {
        let updated = db
            .execute(
                "UPDATE collections SET pending_json = ?1, staging_fp = ?2 \
                 WHERE name = ?3 AND pending_json = ?4",
                params![&new_json, &claimed.id, collection.as_str(), &old_json],
            )
            .unwrap_or(0);
        #[cfg(test)]
        if updated == 1 && CLAIM_PRE_COMMIT_FAIL.load(std::sync::atomic::Ordering::SeqCst) {
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
        _ => false,
    }
}

#[cfg(test)]
pub(crate) static CLAIM_PRE_COMMIT_FAIL: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(test)]
pub(crate) static INSTALL_PRE_COMMIT_FAIL: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(test)]
pub(crate) static CLEAR_PRE_COMMIT_FAIL: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(test)]
pub(crate) static DISCARD_PRE_COMMIT_FAIL: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn snapshot_live_vectors(db: &rusqlite::Connection, name: &str) -> rusqlite::Result<()> {
    let backup = format!("{name}_vec_backup");
    let live = format!("{name}_vec");
    let _ = db.execute(&format!("DROP TABLE IF EXISTS {backup}"), []);
    if table_exists(db, &live) {
        db.execute(
            &format!("CREATE TABLE {backup} (item_id TEXT PRIMARY KEY, embedding BLOB NOT NULL)"),
            [],
        )?;
        db.execute(
            &format!("INSERT INTO {backup} SELECT item_id, embedding FROM {live}"),
            [],
        )?;
    }
    Ok(())
}

fn restore_live_vectors(
    db: &rusqlite::Connection,
    name: &str,
    dimensions: usize,
) -> rusqlite::Result<()> {
    let backup = format!("{name}_vec_backup");
    let live = format!("{name}_vec");
    if !table_exists(db, &backup) {
        return Ok(());
    }
    if !table_exists(db, &live) && dimensions > 0 {
        db.execute_batch(&schema::vec_table_sql(name, dimensions))?;
        let _ = db.execute(
            &format!(
                "INSERT INTO {live}(item_id, embedding) SELECT item_id, embedding FROM {backup}"
            ),
            [],
        );
    }
    let _ = db.execute(&format!("DROP TABLE IF EXISTS {backup}"), []);
    Ok(())
}

/// Atomically clear a stale collection-local pending marker + staging when
/// this builder observes a **completed** target (installed fingerprint ==
/// intended).
///
/// - `expected_id: Some(id)`: clears only when the stored marker still
///   references our attempt id, or references *any* attempt for the same
///   `intended_fp`. A marker for a **different** target is never touched.
/// - `expected_id: None`: clears any parseable marker whose `intended`
///   equals `intended_fp`, and drops that attempt's staging.
///
/// Runs in one `BEGIN IMMEDIATE` transaction so pending and staging cannot
/// tear across a crash. A failed clear is logged without secrets and
/// retried on the next observe-completed pass.
pub(crate) fn clear_completed_target(
    index: &MetadataIndex,
    collection: CollectionKind,
    expected_id: Option<&str>,
    intended_fp: &str,
) {
    if !index.writable() {
        return;
    }
    let result = in_transaction(index.db(), |db| -> rusqlite::Result<()> {
        let raw: String = db
            .query_row(
                "SELECT pending_json FROM collections WHERE name = ?1",
                params![collection.as_str()],
                |r| r.get(0),
            )
            .unwrap_or_default();
        if raw.trim().is_empty() {
            if let Some(id) = expected_id {
                db.execute(
                    "DELETE FROM vector_staging WHERE collection = ?1 AND pending_id = ?2",
                    params![collection.as_str(), id],
                )?;
            }
            return Ok(());
        }
        let Some(p) = CollectionPending::parse(&raw) else {
            return Ok(());
        };
        let ours = expected_id.is_some_and(|id| id == p.id);
        let same_target = p.intended.is_empty() || p.intended == intended_fp;
        if !ours && !same_target {
            return Ok(());
        }
        let updated = db.execute(
            "UPDATE collections SET pending_json = '', staging_fp = '', backoff_until = 0 \
             WHERE name = ?1 AND pending_json = ?2",
            params![collection.as_str(), &raw],
        )?;
        if updated == 1 {
            db.execute(
                "DELETE FROM vector_staging WHERE collection = ?1 AND pending_id = ?2",
                params![collection.as_str(), p.id],
            )?;
        }
        #[cfg(test)]
        if CLEAR_PRE_COMMIT_FAIL.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(1),
                Some("injected clear pre-commit failure".into()),
            ));
        }
        Ok(())
    });
    if let Err(e) = result {
        tracing::debug!(
            error = %e,
            collection = collection.as_str(),
            "failed to clear completed-target pending marker; retained and retried on next pass"
        );
    }
}

pub fn stage_vector(
    index: &MetadataIndex,
    collection: CollectionKind,
    pending_id: &str,
    intended_fp: &str,
    item_id: &str,
    content_hash: &str,
    embedding: &[f32],
) -> Result<(), rusqlite::Error> {
    let bytes: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();
    index.db().execute(
        "INSERT OR REPLACE INTO vector_staging(\
            collection, pending_id, intended_fingerprint, item_id, content_hash, embedding) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            collection.as_str(),
            pending_id,
            intended_fp,
            item_id,
            content_hash,
            bytes
        ],
    )?;
    Ok(())
}

pub fn staged_count(index: &MetadataIndex, collection: CollectionKind, pending_id: &str) -> i64 {
    index
        .db()
        .query_row(
            "SELECT COUNT(*) FROM vector_staging WHERE collection = ?1 AND pending_id = ?2",
            params![collection.as_str(), pending_id],
            |r| r.get(0),
        )
        .unwrap_or(0)
}

pub fn prune_stale_staging(
    index: &MetadataIndex,
    collection: CollectionKind,
    pending_id: &str,
) -> Result<(), rusqlite::Error> {
    index
        .db()
        .execute(
            "DELETE FROM vector_staging \
             WHERE collection = ?1 AND pending_id = ?2 AND ( \
               NOT EXISTS ( \
                 SELECT 1 FROM items i \
                 WHERE i.collection = vector_staging.collection AND i.item_id = vector_staging.item_id \
               ) \
               OR content_hash != ( \
                 SELECT i.content_hash FROM items i \
                 WHERE i.collection = vector_staging.collection AND i.item_id = vector_staging.item_id \
               ) \
             )",
            params![collection.as_str(), pending_id],
        )
        .map(|_| ())
}

pub fn staging_complete(
    index: &MetadataIndex,
    collection: CollectionKind,
    pending_id: &str,
) -> Result<bool, rusqlite::Error> {
    let live = index.item_count(collection);
    let staged = staged_count(index, collection, pending_id);
    if staged != live {
        return Ok(false);
    }
    let missing: i64 = index.db().query_row(
        "SELECT COUNT(*) FROM items i \
         WHERE i.collection = ?1 AND NOT EXISTS ( \
           SELECT 1 FROM vector_staging s \
           WHERE s.collection = i.collection AND s.pending_id = ?2 \
             AND s.item_id = i.item_id AND s.content_hash = i.content_hash \
         )",
        params![collection.as_str(), pending_id],
        |r| r.get(0),
    )?;
    Ok(missing == 0)
}

pub fn discard_foreign_staging(
    index: &MetadataIndex,
    collection: CollectionKind,
    pending_id: &str,
) -> Result<(), rusqlite::Error> {
    index
        .db()
        .execute(
            "DELETE FROM vector_staging WHERE collection = ?1 AND pending_id != ?2",
            params![collection.as_str(), pending_id],
        )
        .map(|_| ())
}

pub fn record_attempt(
    index: &MetadataIndex,
    collection: CollectionKind,
    pending: &CollectionPending,
    last_attempt_at: i64,
    backoff_secs: i64,
) {
    let Some(current) = pending_state(index, collection) else {
        return;
    };
    if current.id != pending.id {
        return;
    }
    let mut updated = current;
    updated.last_attempt_at = last_attempt_at;
    let backoff_until = if backoff_secs > 0 {
        last_attempt_at + backoff_secs
    } else {
        0
    };
    let _ = index.db().execute(
        "UPDATE collections SET pending_json = ?1, backoff_until = ?2 \
         WHERE name = ?3 AND pending_json = ?4",
        params![
            updated.to_json(),
            backoff_until,
            collection.as_str(),
            pending.to_json()
        ],
    );
}

/// Atomically replace one collection's vec table from hash-bound staging.
///
/// Completeness is checked before BEGIN and again inside the transaction.
/// Stale hashes cannot install. The sibling collection's vec table is not
/// dropped or rewritten.
pub fn install_vectors(
    index: &MetadataIndex,
    collection: CollectionKind,
    pending: &CollectionPending,
    fp: &VectorFingerprint,
    payload: &str,
    install_dimensions: usize,
) -> Result<bool, rusqlite::Error> {
    if !index.vec_available() || !index.writable() {
        return Ok(false);
    }
    if !staging_complete(index, collection, &pending.id)? {
        return Ok(false);
    }
    if pending.intended != fp.hash {
        return Ok(false);
    }

    let db = index.db();
    let name = collection.as_str();
    let old_dims = index
        .collection_state(collection)
        .ok()
        .map(|s| s.embedding_dimensions)
        .unwrap_or(0);
    let _ = snapshot_live_vectors(db, name);
    db.execute_batch("BEGIN IMMEDIATE;")?;
    let result = (|| -> Result<bool, rusqlite::Error> {
        if !staging_complete(index, collection, &pending.id)? {
            return Ok(false);
        }
        let stored: String = db
            .query_row(
                "SELECT pending_json FROM collections WHERE name = ?1",
                params![collection.as_str()],
                |r| r.get(0),
            )
            .unwrap_or_default();
        let Some(current) = CollectionPending::parse(&stored) else {
            return Ok(false);
        };
        if current.id != pending.id || current.intended != fp.hash {
            return Ok(false);
        }

        let name = collection.as_str();
        db.execute(&schema::drop_vec_table_sql(name), [])?;
        db.execute_batch(&format!(
            "CREATE VIRTUAL TABLE {name}_vec USING vec0(\n    \
             item_id TEXT PRIMARY KEY,\n    \
             embedding FLOAT[{install_dimensions}]\n);"
        ))?;
        db.execute(
            &format!(
                "INSERT INTO {name}_vec(item_id, embedding) \
                 SELECT s.item_id, s.embedding FROM vector_staging s \
                 JOIN items i ON i.collection = s.collection AND i.item_id = s.item_id \
                 WHERE s.collection = ?1 AND s.pending_id = ?2 \
                   AND s.intended_fingerprint = ?3 AND s.content_hash = i.content_hash"
            ),
            params![name, pending.id, fp.hash],
        )?;
        db.execute(
            "DELETE FROM vector_staging WHERE collection = ?1 AND pending_id = ?2",
            params![name, pending.id],
        )?;
        let vec_count: i64 = db
            .query_row(&format!("SELECT COUNT(*) FROM {name}_vec"), [], |r| {
                r.get(0)
            })
            .unwrap_or(0);
        let item_count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM items WHERE collection = ?1",
                params![name],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if vec_count != item_count {
            return Ok(false);
        }
        db.execute(
            "UPDATE collections SET embedding_dimensions = ?1, fingerprint_hash = ?2, \
             fingerprint_payload = ?3, vector_schema_version = ?4, prep_version = ?5, \
             pending_json = '', staging_fp = '', backoff_until = 0, vec_count = ?6 \
             WHERE name = ?7",
            params![
                install_dimensions as i64,
                &fp.hash,
                payload,
                fp.vector_schema_version as i64,
                &fp.document_preparation.version,
                vec_count,
                name
            ],
        )?;
        db.execute(&format!("DROP TABLE IF EXISTS {name}_vec_backup"), [])?;
        Ok(true)
    })();
    match result {
        Ok(true) => {
            #[cfg(test)]
            if INSTALL_PRE_COMMIT_FAIL.load(std::sync::atomic::Ordering::SeqCst) {
                let _ = db.execute_batch("ROLLBACK;");
                let _ = restore_live_vectors(db, name, old_dims);
                return Ok(false);
            }
            db.execute_batch("COMMIT;")?;
            Ok(true)
        }
        Ok(false) => {
            let _ = db.execute_batch("ROLLBACK;");
            let _ = restore_live_vectors(db, name, old_dims);
            Ok(false)
        }
        Err(e) => {
            let _ = db.execute_batch("ROLLBACK;");
            let _ = restore_live_vectors(db, name, old_dims);
            Err(e)
        }
    }
}

pub(crate) fn compatible_readiness(
    idx: &MetadataIndex,
    collection: CollectionKind,
) -> VectorReadiness {
    let removed = idx.prune_orphan_vector_rows(collection);
    if removed > 0 {
        tracing::debug!(
            collection = collection.as_str(),
            removed,
            "pruned orphan metadata vector rows for a compatible collection"
        );
    }
    // Prune has already COMMIT/ROLLBACK. Coverage is a JOIN in one later
    // snapshot: Ready only when every items row has a vec row *and* every
    // vec/rowids id exists in items for this collection. COUNT equality is
    // not authority (a ghost plus a missing live embedding can match).
    match idx.vector_join_counts(collection) {
        Some((orphans, _)) if orphans > 0 => {
            // Residual extras after a failed prune stay unsafe. KNN
            // already filters them out, but Ready would claim full
            // coverage. Fail closed without a rebuild.
            VectorReadiness::Pending { owned: false }
        }
        Some((_, missing)) if missing > 0 => VectorReadiness::ReadyMissing {
            missing: missing as usize,
        },
        Some(_) => VectorReadiness::Ready,
        None => VectorReadiness::Pending { owned: false },
    }
}

fn open_index(db_path: &Path) -> Result<MetadataIndex, MetadataIndexError> {
    MetadataIndex::open_or_create(db_path)
}

fn fingerprint_for(
    collection: CollectionKind,
    spec: &EmbeddingSourceSpec,
) -> Result<(VectorFingerprint, String), String> {
    let mut spec = spec.clone();
    spec.normalization = NORMALIZATION_L2_V1.to_string();
    VectorFingerprint::build(spec, metadata_doc_prep(collection), VECTOR_SCHEMA_VERSION)
}

/// Reconcile or rebuild vectors for one collection, then install the live
/// vec table when staging is complete.
///
/// Never holds a DB connection across the embedding await. Sibling
/// collections are not claimed, staged, or dropped.
pub async fn ensure_collection_vectors_ready(
    db_path: &Path,
    collection: CollectionKind,
    spec: &EmbeddingSourceSpec,
    embedder: Option<Arc<dyn EmbeddingProvider>>,
    stale_claim_secs: i64,
    backoff_secs: i64,
    max_batches_per_call: Option<usize>,
    cancel: CancellationToken,
) -> VectorReadiness {
    ensure_collection_vectors(
        db_path,
        collection,
        spec,
        embedder,
        stale_claim_secs,
        backoff_secs,
        max_batches_per_call,
        cancel,
        true,
    )
    .await
}

/// Stage embeddings for one collection without committing the live vec table.
///
/// Callers that must re-check a live pin after the embed await should
/// [`commit_staged_vectors`] only when that pin still matches, otherwise
/// [`discard_collection_rebuild`].
pub async fn stage_collection_vectors(
    db_path: &Path,
    collection: CollectionKind,
    spec: &EmbeddingSourceSpec,
    embedder: Option<Arc<dyn EmbeddingProvider>>,
    stale_claim_secs: i64,
    backoff_secs: i64,
    max_batches_per_call: Option<usize>,
    cancel: CancellationToken,
) -> VectorReadiness {
    ensure_collection_vectors(
        db_path,
        collection,
        spec,
        embedder,
        stale_claim_secs,
        backoff_secs,
        max_batches_per_call,
        cancel,
        false,
    )
    .await
}

/// Install hash-bound staging into the live vec table when complete.
pub fn commit_staged_vectors(
    db_path: &Path,
    collection: CollectionKind,
    spec: &EmbeddingSourceSpec,
) -> bool {
    let Ok((fp, payload)) = fingerprint_for(collection, spec) else {
        return false;
    };
    let Ok(idx) = open_index(db_path) else {
        return false;
    };
    if !idx.writable() || !idx.vec_available() {
        return false;
    }
    let Some(pending) = pending_state(&idx, collection) else {
        return false;
    };
    if pending.intended != fp.hash {
        return false;
    }
    let complete = staging_complete(&idx, collection, &pending.id).unwrap_or(false);
    if !complete {
        return false;
    }
    install_vectors(&idx, collection, &pending, &fp, &payload, spec.dimensions).unwrap_or(false)
}

/// Drop collection staging and the pending marker without touching a live
/// vec table. Used when a pin mismatch is observed after an embed await.
pub fn discard_collection_rebuild(db_path: &Path, collection: CollectionKind) {
    let Ok(idx) = open_index(db_path) else {
        return;
    };
    if !idx.writable() {
        return;
    }
    let _ = in_transaction(idx.db(), |db| -> rusqlite::Result<()> {
        db.execute(
            "DELETE FROM vector_staging WHERE collection = ?1",
            params![collection.as_str()],
        )?;
        db.execute(
            "UPDATE collections SET pending_json = '', staging_fp = '', backoff_until = 0 \
             WHERE name = ?1",
            params![collection.as_str()],
        )?;
        #[cfg(test)]
        if DISCARD_PRE_COMMIT_FAIL.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(1),
                Some("injected discard pre-commit failure".into()),
            ));
        }
        Ok(())
    });
}

async fn ensure_collection_vectors(
    db_path: &Path,
    collection: CollectionKind,
    spec: &EmbeddingSourceSpec,
    embedder: Option<Arc<dyn EmbeddingProvider>>,
    stale_claim_secs: i64,
    backoff_secs: i64,
    max_batches_per_call: Option<usize>,
    cancel: CancellationToken,
    commit: bool,
) -> VectorReadiness {
    let stale_claim_secs = if stale_claim_secs <= 0 {
        STALE_CLAIM_DEFAULT
    } else {
        stale_claim_secs
    };
    let (fp, payload) = match fingerprint_for(collection, spec) {
        Ok(v) => v,
        Err(_) => return VectorReadiness::Pending { owned: false },
    };

    let idx = match open_index(db_path) {
        Ok(v) => v,
        Err(_) => return VectorReadiness::Pending { owned: false },
    };
    if !idx.vec_available() {
        return VectorReadiness::Disabled;
    }
    if !idx.writable() {
        return VectorReadiness::Pending { owned: false };
    }
    let state = match idx.collection_state(collection) {
        Ok(s) => s,
        Err(_) => return VectorReadiness::Pending { owned: false },
    };
    let compatible = state.fingerprint_hash == fp.hash
        && state.embedding_dimensions == spec.dimensions
        && !state.fingerprint_hash.is_empty();
    if compatible {
        clear_completed_target(&idx, collection, None, fp.hash.as_str());
        return compatible_readiness(&idx, collection);
    }

    if state.backoff_until > now_secs() {
        return VectorReadiness::Pending { owned: false };
    }
    drop(idx);

    let idx = match open_index(db_path) {
        Ok(v) => v,
        Err(_) => return VectorReadiness::Pending { owned: false },
    };
    let reason = if state.fingerprint_hash.is_empty() {
        "adopt_incompatible"
    } else {
        "fingerprint_mismatch"
    };
    let mut pending = match ensure_pending(&idx, collection, &fp.hash, reason) {
        Ok(p) => p,
        Err(_) => return VectorReadiness::Pending { owned: false },
    };
    let _ = discard_foreign_staging(&idx, collection, &pending.id);
    drop(idx);

    let idx = match open_index(db_path) {
        Ok(v) => v,
        Err(_) => return VectorReadiness::Pending { owned: false },
    };
    if !try_claim_rebuild(&idx, collection, &mut pending, stale_claim_secs) {
        drop(idx);
        if let Ok(idx) = open_index(db_path)
            && let Ok(s) = idx.collection_state(collection)
            && s.fingerprint_hash == fp.hash
            && s.embedding_dimensions == spec.dimensions
            && !s.fingerprint_hash.is_empty()
        {
            clear_completed_target(&idx, collection, Some(&pending.id), fp.hash.as_str());
            return compatible_readiness(&idx, collection);
        }
        return VectorReadiness::Pending { owned: false };
    }
    drop(idx);

    if cancel.is_cancelled() {
        return VectorReadiness::Pending { owned: true };
    }

    let mut batches_done = 0usize;
    loop {
        let idx = match open_index(db_path) {
            Ok(v) => v,
            Err(_) => return VectorReadiness::Pending { owned: true },
        };
        match idx.collection_state(collection) {
            Ok(s)
                if s.fingerprint_hash == fp.hash
                    && s.embedding_dimensions == spec.dimensions
                    && !s.fingerprint_hash.is_empty() =>
            {
                clear_completed_target(&idx, collection, Some(&pending.id), fp.hash.as_str());
                return compatible_readiness(&idx, collection);
            }
            _ => {}
        }
        match pending_state(&idx, collection) {
            Some(p) if p.id == pending.id => {}
            _ => return VectorReadiness::Pending { owned: true },
        }
        if cancel.is_cancelled() {
            return VectorReadiness::Pending { owned: true };
        }
        let _ = prune_stale_staging(&idx, collection, &pending.id);
        let needed = match idx.items_not_staged(collection, &pending.id) {
            Ok(n) => n,
            Err(_) => return VectorReadiness::Pending { owned: true },
        };
        if needed.is_empty() {
            let complete = staging_complete(&idx, collection, &pending.id).unwrap_or(false);
            if complete {
                if !commit {
                    // Staging is durable; the caller installs only after a
                    // live pin / fingerprint re-check that cannot run here.
                    return VectorReadiness::Pending { owned: true };
                }
                let ok =
                    install_vectors(&idx, collection, &pending, &fp, &payload, spec.dimensions)
                        .unwrap_or(false);
                if ok {
                    return VectorReadiness::Ready;
                }
            }
            record_attempt(&idx, collection, &pending, now_secs(), backoff_secs);
            return VectorReadiness::Pending { owned: true };
        }
        drop(idx);

        let Some(embedder) = embedder.clone() else {
            if let Ok(idx) = open_index(db_path) {
                record_attempt(&idx, collection, &pending, now_secs(), backoff_secs);
            }
            return VectorReadiness::Pending { owned: true };
        };

        for batch in needed.chunks(DEFAULT_BATCH) {
            if let Some(cap) = max_batches_per_call
                && batches_done >= cap
            {
                return VectorReadiness::Pending { owned: true };
            }
            if cancel.is_cancelled() {
                return VectorReadiness::Pending { owned: true };
            }
            let texts: Vec<&str> = batch.iter().map(|(_, _, t)| t.as_str()).collect();
            let vectors = match embedder.embed_batch(&texts).await {
                Ok(v) => v,
                Err(_) => {
                    tracing::warn!(
                        collection = %collection.as_str(),
                        items = batch.len(),
                        "prime index staging embed request failed"
                    );
                    if let Ok(idx) = open_index(db_path) {
                        record_attempt(&idx, collection, &pending, now_secs(), backoff_secs);
                    }
                    return VectorReadiness::Pending { owned: true };
                }
            };
            if validate_embedding_batch(batch.len(), spec.dimensions, &vectors).is_err() {
                tracing::warn!(
                    collection = %collection.as_str(),
                    items = batch.len(),
                    returned = vectors.len(),
                    expected_dims = spec.dimensions,
                    "prime index staging embed failed validation"
                );
                if let Ok(idx) = open_index(db_path) {
                    record_attempt(&idx, collection, &pending, now_secs(), backoff_secs);
                }
                return VectorReadiness::Pending { owned: true };
            }
            let idx = match open_index(db_path) {
                Ok(v) => v,
                Err(_) => return VectorReadiness::Pending { owned: true },
            };
            match pending_state(&idx, collection) {
                Some(p) if p.id == pending.id => {}
                _ => {
                    tracing::warn!(
                        collection = %collection.as_str(),
                        "prime index staging lost its pending claim after embed"
                    );
                    return VectorReadiness::Pending { owned: true };
                }
            }
            for ((item_id, hash, _), v) in batch.iter().zip(vectors.iter()) {
                let mut row = v.clone();
                if l2_normalize_v1(&mut row).is_err() {
                    tracing::warn!(
                        collection = %collection.as_str(),
                        "prime index staging normalization failed"
                    );
                    return VectorReadiness::Pending { owned: true };
                }
                if stage_vector(
                    &idx,
                    collection,
                    &pending.id,
                    fp.hash.as_str(),
                    item_id,
                    hash,
                    row.as_slice(),
                )
                .is_err()
                {
                    tracing::warn!(
                        collection = %collection.as_str(),
                        "prime index staging write failed"
                    );
                    return VectorReadiness::Pending { owned: true };
                }
            }
            drop(idx);
            batches_done += 1;
        }
    }
}

#[cfg(test)]
pub(crate) fn sibling(collection: CollectionKind) -> CollectionKind {
    match collection {
        CollectionKind::Skills => CollectionKind::CallableAgents,
        CollectionKind::CallableAgents => CollectionKind::Skills,
    }
}
