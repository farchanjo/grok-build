//! Store-agnostic vector-mirror seam.
//!
//! SQLite (the `chunks_vec` / `vector_staging` tables) remains the vector
//! authority. A [`VectorMirror`] is a best-effort remote overlay — currently
//! Milvus (see `mirror_milvus`, compiled with the `milvus` feature) — that
//! receives upsert/delete fan-out after the SQLite write succeeds and may
//! serve KNN reads when it reports itself in sync. Every consumer must treat
//! the mirror as disposable: any error or readiness mismatch falls back to
//! sqlite-vec.
//!
//! ## Score contract
//!
//! The trait returns distances in the sqlite-vec L2 contract (smaller is
//! closer). Milvus `IP` metric on unit-L2 vectors returns a cosine
//! similarity `sim ∈ [-1, 1]`; the adapter distance is
//! [`similarity_to_l2_distance`]: the true unit-vector Euclidean distance
//! `‖u − v‖₂ = √(2·(1 − sim))`, clamped to `[0, 2]`. Verified against
//! sqlite-vec empirically (orthogonal unit vectors: both report `√2`), so
//! a mirrored hit and a sqlite-vec hit with the same geometry produce the
//! exact same distance and downstream `l2_similarity` / fusion scores.
//!
//! ## Fingerprint compatibility tag
//!
//! Store-specific metadata (Milvus collection descriptions) carries the tag
//! built by [`collection_tag`]: `grok:<fingerprint_hash>:<dims>:
//! <vector_schema_version>`. Backends verify the tag on attach; a mismatch
//! or an absent tag means the mirrored vectors are incompatible and the
//! collection must be dropped, recreated, and resynced from SQLite.
//!
//! ## Resync engine
//!
//! [`resync_collection`] repopulates a mirror collection from SQLite by
//! draining rows through a [`MirrorResyncSource`] (a row callback keeps the
//! trait store-agnostic), upserting idempotently in batches of
//! [`RESYNC_BATCH_ROWS`], and verifying the server-side row count with
//! bounded retries before the handle may report [`MirrorState::Ready`].

use std::future::Future;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;
use async_trait::async_trait;

/// Score-conversion helper mapping a server cosine/IP similarity to the
/// sqlite-vec-compatible L2 distance the trait returns.
///
/// For unit vectors, the true Euclidean distance derives from the cosine
/// similarity: `‖u − v‖₂ = √(2 − 2·cos) = √(2·(1 − sim))`, clamped to
/// `[0, 2]`:
/// - `sim = 1` (identical unit vectors) → `0.0`
/// - `sim = 0` (orthogonal) → `√2 ≈ 1.4142` (matches sqlite-vec exactly)
/// - `sim = -1` (opposite) → `2.0`
///
/// Clamping absorbs tiny floating-point overshoot (`sim = 1 + ε`) and any
/// server quirk reporting similarities outside `[-1, 1]`. A NaN similarity
/// (malformed server response) maps to `2.0` — maximally distant — because
/// `f32::sqrt`/`f32::clamp` propagate NaN and a NaN distance would poison
/// result ordering and the `[0, 2]` distance contract.
#[must_use]
pub fn similarity_to_l2_distance(sim: f32) -> f32 {
    if sim.is_nan() {
        return 2.0;
    }
    // Clamp the similarity into [-1, 1] first so server quirks and float
    // overshoot cannot make the radicand negative (sqrt would return NaN).
    let sim = sim.clamp(-1.0, 1.0);
    (2.0 * (1.0 - sim)).sqrt()
}

/// Build the fingerprint compatibility tag persisted as store metadata
/// (Milvus collection description).
///
/// Format: `grok:<fingerprint_hash>:<dims>:<vector_schema_version>`.
///
/// # Disclosure note
///
/// The tag is readable by anyone with Milvus access. It carries no secrets
/// (lowercase-hex hash, dims, schema version), but the hash derives from
/// low-entropy inputs (provider id, origin host, model, doc prep), so a
/// Milvus-side reader could verify guessed embedding endpoints by
/// recomputing fingerprints. That is accepted: the fingerprint is an
/// identity label, not a credential.
/// `fingerprint_hash` is the short hex digest from
/// [`crate::fingerprint::VectorFingerprint::hash`]; `dims` is the embedding
/// dimension; `vector_schema_version` is
/// [`crate::fingerprint::VECTOR_SCHEMA_VERSION`].
#[must_use]
pub fn collection_tag(fingerprint_hash: &str, dims: u32, vector_schema_version: u32) -> String {
    format!("grok:{fingerprint_hash}:{dims}:{vector_schema_version}")
}

/// Parse a fingerprint compatibility tag built by [`collection_tag`].
///
/// Returns `(fingerprint_hash, dims, vector_schema_version)` or `None` when
/// the value is absent, malformed, or not a `grok:` tag (e.g. a collection
/// created by another tool).
#[must_use]
pub fn parse_collection_tag(description: &str) -> Option<(String, u32, u32)> {
    let rest = description.trim().strip_prefix("grok:")?;
    let mut parts = rest.split(':');
    let hash = parts.next()?;
    let dims = parts.next()?;
    let schema_version = parts.next()?;
    if parts.next().is_some() || hash.is_empty() {
        return None;
    }
    // A fingerprint hash is lowercase hex.
    if !hash
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    {
        return None;
    }
    let dims = dims.parse::<u32>().ok()?;
    let vector_schema_version = schema_version.parse::<u32>().ok()?;
    Some((hash.to_owned(), dims, vector_schema_version))
}

/// Schema version for schema-v2 primary-remote collections (`milvus` mode),
/// distinguishing them from v1 vector-only mirror collections.
pub const MEMORY_SCHEMA_VERSION_V2: u32 = 2;

/// One memory row in a schema-v2 primary-remote collection (`milvus` mode).
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryRow {
    pub id: String,
    pub text: String,
    pub vector: Vec<f32>,
    pub fingerprint_hash: String,
    pub hash: String,
    pub source: String,
    pub path: String,
    pub created_at: i64,
}

/// Candidate returned from remote search (dense KNN or BM25 full-text keyword search).
#[derive(Debug, Clone, PartialEq)]
pub struct RemoteSearchHit {
    pub id: String,
    /// Distance or score:
    /// - Dense KNN: sqlite-vec L2-compatible distance (smaller is closer, in `[0, 2]`).
    /// - BM25: relevance score (higher is better, `>= 0.0`).
    pub score: f32,
    pub text: String,
    pub path: String,
    pub source: String,
    pub created_at: i64,
}

/// Memory-index mirror collection name for a workspace identity hash
/// (see [`crate::workspace_identity::workspace_identity_hash16`]).
///
/// Clones, worktrees, and copies of one repository share the identity hash
/// and therefore share one remote collection.
#[must_use]
pub fn memory_collection_name(workspace_identity_hash16: &str) -> String {
    format!("grok_mem_{workspace_identity_hash16}")
}

/// Prime mirror collection name for a workspace identity hash and a
/// metadata-index collection kind string (`"skills"` / `"callable_agents"`).
#[must_use]
pub fn prime_collection_name(workspace_identity_hash16: &str, collection_kind: &str) -> String {
    format!("grok_prime_{workspace_identity_hash16}_{collection_kind}")
}

/// Categorized mirror failure kind, mirroring
/// [`crate::retrieval::RetrievalErrorKind`]. Safe to log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirrorErrorKind {
    /// Mirror not configured, unreachable, or rejected the credentials.
    SourceUnavailable,
    /// Transient network/server failure (may be retried). Diagnostics never
    /// include provider error text that could carry secrets.
    Transient,
    /// Malformed/unsupported response shape.
    Malformed,
    /// Deadline/limit exceeded (per-call timeout, attempt budget).
    BudgetExhausted,
    /// Cancelled.
    Cancelled,
}

impl MirrorErrorKind {
    /// Whether this failure is transient and safe to retry later.
    ///
    /// `Cancelled` counts as retryable: a cancelled mirror operation leaves
    /// no partial state that matters (SQLite is the authority and every
    /// mirror write is an idempotent upsert or keyed delete), so a later
    /// fan-out or resync attempt is always safe.
    #[must_use]
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            MirrorErrorKind::Transient | MirrorErrorKind::Cancelled
        )
    }
}

/// Categorized, secret-free mirror failure.
///
/// The `Debug` impl reports only the category (`kind`); it never renders an
/// arbitrary server/network error string that could contain the bearer
/// token or raw server bodies. The optional `detail` is exposed only via
/// [`MirrorError::detail`] for single-line tracing by the owner, after the
/// constructor redacted the token and truncated the text.
#[derive(Clone)]
pub struct MirrorError {
    kind: MirrorErrorKind,
    /// Sanitized diagnostic detail. Kept out of `Debug` to avoid leaking
    /// server error text into arbitrary derive/format paths.
    detail: Option<String>,
}

impl MirrorError {
    #[must_use]
    pub fn new(kind: MirrorErrorKind) -> Self {
        Self { kind, detail: None }
    }

    /// Build an error with sanitized detail: the bearer token (when given)
    /// is redacted, control characters are dropped, and the text is
    /// truncated so raw server bodies never flow into diagnostics.
    #[must_use]
    pub fn with_detail(
        kind: MirrorErrorKind,
        detail: impl Into<String>,
        token: Option<&str>,
    ) -> Self {
        Self {
            kind,
            detail: Some(sanitize_detail(&detail.into(), token)),
        }
    }

    #[must_use]
    pub fn kind(&self) -> MirrorErrorKind {
        self.kind
    }

    /// Sanitized, token-free detail (may be empty).
    #[must_use]
    pub fn detail(&self) -> &str {
        self.detail.as_deref().unwrap_or("")
    }
}

impl std::fmt::Debug for MirrorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MirrorError")
            .field("kind", &self.kind)
            .finish()
    }
}

impl std::fmt::Display for MirrorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (&self.kind, &self.detail) {
            (kind, Some(detail)) => write!(f, "{kind:?} ({detail})"),
            (kind, None) => write!(f, "{kind:?}"),
        }
    }
}

impl std::error::Error for MirrorError {}

/// Redact the token, drop control characters, and bound the length of a
/// diagnostic detail string. Never panics; never returns the token.
///
/// Control characters are dropped **before** token replacement so a token
/// split by injected control characters (or appearing percent-encoded in a
/// server error URL) cannot survive redaction: the normalized text is
/// matched against both the raw and the percent-encoded token forms.
#[must_use]
fn sanitize_detail(detail: &str, token: Option<&str>) -> String {
    const MAX_DETAIL_CHARS: usize = 240;
    let mut out: String = detail.chars().filter(|c| !c.is_control()).collect();
    if let Some(token) = token.filter(|t| !t.is_empty()) {
        if out.contains(token) {
            out = out.replace(token, "[redacted]");
        }
        if let Some(encoded) = percent_encode(token)
            && out.contains(&encoded)
        {
            out = out.replace(&encoded, "[redacted]");
        }
    }
    out.chars().take(MAX_DETAIL_CHARS).collect()
}

/// Uppercase percent-encoding of a token's UTF-8 bytes (the form a server
/// error URL would carry).
fn percent_encode(token: &str) -> Option<String> {
    if token.is_empty() {
        return None;
    }
    let mut out = String::with_capacity(token.len() * 3);
    for byte in token.as_bytes() {
        // Only escape bytes that URL encoders always escape; the rest are
        // already covered by the raw-token replacement.
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(*byte as char);
        } else {
            const HEX: &[u8; 16] = b"0123456789ABCDEF";
            out.push('%');
            out.push(HEX[(byte >> 4) as usize] as char);
            out.push(HEX[(byte & 0x0F) as usize] as char);
        }
    }
    Some(out)
}

/// Mirror lifecycle state surfaced for inspection (e.g. `/context`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MirrorState {
    /// No mirror configured for this index.
    #[default]
    Unconfigured,
    /// Resync/verification in progress; reads must fall back to SQLite.
    Syncing,
    /// Collection present, fingerprint + dims + count verified; KNN may be
    /// served from the mirror.
    Ready,
    /// Mirror unusable (connect failure, incompatible collection, repeated
    /// errors). Reads fall back to SQLite.
    Unavailable,
}

/// Last-known mirror facts recorded by the owner (resync engine / read
/// interception) and consulted by readiness gating.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MirrorSnapshot {
    /// Lifecycle state.
    pub state: MirrorState,
    /// Fingerprint hash the mirror was last verified against.
    pub fingerprint_hash: Option<String>,
    /// Embedding dimension of the mirrored collection.
    pub dimensions: Option<u32>,
    /// Row count last reported by the mirror.
    pub row_count: Option<u64>,
}

/// Trait-object seam for a remote vector-store mirror.
///
/// Implementations must be failure-isolated: callers already hold the data
/// in SQLite, so every method may fail freely and is bounded by the
/// backend's configured timeout. All `fingerprint` arguments are the short
/// hex hash from [`crate::fingerprint::VectorFingerprint::hash`].
#[async_trait]
pub trait VectorMirror: Send + Sync {
    /// Static, credential-free backend label (e.g. `"milvus"`).
    fn backend_id(&self) -> &str;

    /// Create the collection when missing, verify the fingerprint tag when
    /// present, and drop + recreate on mismatch/absent tag. On recreation
    /// the collection is empty; the resync engine repopulates it from
    /// SQLite and re-verifies via [`VectorMirror::count`].
    async fn ensure_collection(
        &self,
        name: &str,
        dims: u32,
        fingerprint_hash: &str,
    ) -> Result<(), MirrorError>;

    /// Idempotently upsert `(id, vector)` rows tagged with the fingerprint
    /// hash. Existing ids are replaced (VARCHAR primary keys, no auto-id).
    async fn upsert(
        &self,
        name: &str,
        ids: &[String],
        vectors: &[Vec<f32>],
        fingerprint_hash: &str,
    ) -> Result<(), MirrorError>;

    /// Delete rows by primary-key id. Missing ids are not an error.
    async fn delete(&self, name: &str, ids: &[String]) -> Result<(), MirrorError>;

    /// KNN against the mirrored collection, filtered to the fingerprint.
    ///
    /// Returns at most `k` `(id, distance)` pairs where `distance` is in the
    /// sqlite-vec L2 contract (see [`similarity_to_l2_distance`]), ordered
    /// nearest first.
    async fn knn(
        &self,
        name: &str,
        query: &[f32],
        k: usize,
        fingerprint_hash: &str,
    ) -> Result<Vec<(String, f32)>, MirrorError>;

    /// Number of rows currently visible in the collection. Used by resync
    /// verification. A mirrored collection is single-fingerprint (the tag
    /// gate drops + recreates on mismatch), so a collection-wide count is
    /// the fingerprint-filtered count.
    async fn count(&self, name: &str, fingerprint_hash: &str) -> Result<u64, MirrorError>;

    /// Drop the collection entirely.
    async fn drop_collection(&self, name: &str) -> Result<(), MirrorError>;

    /// Schema-v2 collection: creates a collection holding text, BM25 sparse vector,
    /// dense embedding, and metadata (path, source, hash, created_at). Drops and
    /// recreates on fingerprint, dimension, or schema version mismatch.
    async fn ensure_collection_v2(
        &self,
        name: &str,
        dims: u32,
        fingerprint_hash: &str,
    ) -> Result<(), MirrorError> {
        let _ = (name, dims, fingerprint_hash);
        Err(MirrorError::new(MirrorErrorKind::Malformed))
    }

    /// Idempotently upsert schema-v2 memory rows.
    async fn upsert_rows_v2(
        &self,
        name: &str,
        rows: &[MemoryRow],
    ) -> Result<(), MirrorError> {
        let _ = (name, rows);
        Err(MirrorError::new(MirrorErrorKind::Malformed))
    }

    /// BM25 full-text keyword search against the analyzed `text` / `sparse` field.
    async fn bm25_search_v2(
        &self,
        name: &str,
        query: &str,
        k: usize,
        fingerprint_hash: &str,
    ) -> Result<Vec<RemoteSearchHit>, MirrorError> {
        let _ = (name, query, k, fingerprint_hash);
        Err(MirrorError::new(MirrorErrorKind::Malformed))
    }

    /// KNN dense vector search returning schema-v2 hits with full metadata.
    async fn knn_v2(
        &self,
        name: &str,
        query: &[f32],
        k: usize,
        fingerprint_hash: &str,
    ) -> Result<Vec<RemoteSearchHit>, MirrorError> {
        let _ = (name, query, k, fingerprint_hash);
        Err(MirrorError::new(MirrorErrorKind::Malformed))
    }

    /// List all `(id, content_hash)` entries in the remote collection for change detection.
    async fn list_id_hashes_v2(
        &self,
        name: &str,
        fingerprint_hash: &str,
    ) -> Result<std::collections::HashMap<String, String>, MirrorError> {
        let _ = (name, fingerprint_hash);
        Err(MirrorError::new(MirrorErrorKind::Malformed))
    }
}

/// Shared handle to one mirror backend plus its last-known state, bound to
/// the single collection it fronts.
///
/// State updates are lock-free ([`ArcSwap`]) so read-path gating never
/// blocks on a slow mirror call.
pub struct MirrorHandle {
    mirror: Arc<dyn VectorMirror>,
    collection: String,
    snapshot: ArcSwap<MirrorSnapshot>,
}

impl MirrorHandle {
    #[must_use]
    pub fn new(mirror: Arc<dyn VectorMirror>, collection: impl Into<String>) -> Self {
        Self {
            mirror,
            collection: collection.into(),
            snapshot: ArcSwap::from_pointee(MirrorSnapshot::default()),
        }
    }

    /// The underlying mirror backend.
    #[must_use]
    pub fn mirror(&self) -> &Arc<dyn VectorMirror> {
        &self.mirror
    }

    /// The collection this handle fronts.
    #[must_use]
    pub fn collection(&self) -> &str {
        &self.collection
    }

    /// Current state snapshot.
    #[must_use]
    pub fn snapshot(&self) -> arc_swap::Guard<Arc<MirrorSnapshot>> {
        self.snapshot.load()
    }

    /// Replace the state snapshot.
    pub fn update(&self, snapshot: MirrorSnapshot) {
        self.snapshot.store(Arc::new(snapshot));
    }

    /// Mark the mirror unusable (error path); reads fall back to SQLite.
    pub fn mark_unavailable(&self) {
        self.snapshot.rcu(|current| {
            let mut next = MirrorSnapshot::clone(current);
            next.state = MirrorState::Unavailable;
            Arc::new(next)
        });
    }

    /// Mark a resync/verification in progress.
    pub fn mark_syncing(&self) {
        self.snapshot.rcu(|current| {
            let mut next = MirrorSnapshot::clone(current);
            next.state = MirrorState::Syncing;
            Arc::new(next)
        });
    }

    /// Record a verified-ready state for the given fingerprint/dims/count.
    pub fn mark_ready(&self, fingerprint_hash: &str, dims: u32, row_count: u64) {
        self.snapshot.store(Arc::new(MirrorSnapshot {
            state: MirrorState::Ready,
            fingerprint_hash: Some(fingerprint_hash.to_owned()),
            dimensions: Some(dims),
            row_count: Some(row_count),
        }));
    }

    /// Readiness gate comparing the recorded facts against the fingerprint
    /// and dimensions the caller intends to serve.
    ///
    /// The row-count comparison against the SQLite `vec_row_count` happens
    /// at the call site (only the caller can read the SQLite index): use
    /// [`Self::is_ready_for_count`] when a count is available.
    #[must_use]
    pub fn is_ready_for(&self, fingerprint_hash: &str, dims: u32) -> bool {
        let snapshot = self.snapshot.load();
        snapshot.state == MirrorState::Ready
            && snapshot.fingerprint_hash.as_deref() == Some(fingerprint_hash)
            && snapshot.dimensions == Some(dims)
    }

    /// Readiness gate including the SQLite row-count consistency check.
    ///
    /// Loads the snapshot exactly once so the fingerprint/dims/count
    /// comparison cannot straddle a concurrent [`Self::mark_ready`] or
    /// [`Self::mark_unavailable`] (no TOCTOU window).
    #[must_use]
    pub fn is_ready_for_count(
        &self,
        fingerprint_hash: &str,
        dims: u32,
        sqlite_row_count: u64,
    ) -> bool {
        let snapshot = self.snapshot.load();
        snapshot.state == MirrorState::Ready
            && snapshot.fingerprint_hash.as_deref() == Some(fingerprint_hash)
            && snapshot.dimensions == Some(dims)
            && snapshot.row_count == Some(sqlite_row_count)
    }

    /// Ensure the schema-v2 collection exists on the remote store.
    pub async fn ensure_collection_v2(&self, dims: u32, fingerprint_hash: &str) -> Result<(), MirrorError> {
        self.mirror().ensure_collection_v2(&self.collection, dims, fingerprint_hash).await
    }

    /// Upsert schema-v2 rows into the remote collection.
    pub async fn upsert_rows_v2(&self, rows: &[MemoryRow]) -> Result<(), MirrorError> {
        self.mirror().upsert_rows_v2(&self.collection, rows).await
    }

    /// BM25 full-text keyword search against the remote collection.
    pub async fn bm25_search_v2(&self, query: &str, k: usize, fingerprint_hash: &str) -> Result<Vec<RemoteSearchHit>, MirrorError> {
        self.mirror().bm25_search_v2(&self.collection, query, k, fingerprint_hash).await
    }

    /// Dense KNN vector search against the remote collection.
    pub async fn knn_v2(&self, query: &[f32], k: usize, fingerprint_hash: &str) -> Result<Vec<RemoteSearchHit>, MirrorError> {
        self.mirror().knn_v2(&self.collection, query, k, fingerprint_hash).await
    }

    /// List all `(id, hash)` entries in the remote collection for reconciliation.
    pub async fn list_id_hashes_v2(&self, fingerprint_hash: &str) -> Result<std::collections::HashMap<String, String>, MirrorError> {
        self.mirror().list_id_hashes_v2(&self.collection, fingerprint_hash).await
    }

    /// Delete rows by id from the remote collection.
    pub async fn delete_ids(&self, ids: &[String]) -> Result<(), MirrorError> {
        self.mirror().delete(&self.collection, ids).await
    }
}

impl std::fmt::Debug for MirrorHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MirrorHandle")
            .field("backend", &self.mirror.backend_id())
            .field("snapshot", &*self.snapshot.load())
            .finish()
    }
}

/// Default per-call timeout applied when a store config omits
/// `timeout_secs`.
pub const DEFAULT_MIRROR_TIMEOUT_SECS: u64 = 10;

/// Resolve a configured per-call timeout, defaulting and flooring to at
/// least one second.
#[must_use]
pub fn mirror_timeout(timeout_secs: Option<u64>) -> Duration {
    Duration::from_secs(timeout_secs.unwrap_or(DEFAULT_MIRROR_TIMEOUT_SECS).max(1))
}

/// Bound one mirror call by `timeout`; an elapsed deadline is a
/// [`MirrorErrorKind::BudgetExhausted`] failure, never a hang.
pub async fn mirror_call<T>(
    fut: impl Future<Output = Result<T, MirrorError>>,
    timeout: Duration,
) -> Result<T, MirrorError> {
    match tokio::time::timeout(timeout, fut).await {
        Ok(result) => result,
        Err(_) => Err(MirrorError::new(MirrorErrorKind::BudgetExhausted)),
    }
}

// ---------------------------------------------------------------------------
// Resync engine
// ---------------------------------------------------------------------------

/// Rows per upsert batch during a mirror resync (plan-specified batch size).
pub const RESYNC_BATCH_ROWS: usize = 512;

/// Count-verification attempts before a resync gives up (eventual
/// consistency on the remote server needs bounded retry, not a hang).
const RESYNC_VERIFY_ATTEMPTS: u32 = 3;

/// Backoff between count-verification attempts.
const RESYNC_VERIFY_BACKOFF: Duration = Duration::from_millis(250);

/// Summary of one successful [`resync_collection`] run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResyncReport {
    /// Rows upserted from the SQLite source.
    pub rows_upserted: u64,
    /// Upsert batches issued.
    pub batches: usize,
    /// Mirror row count verified after the drain.
    pub mirror_row_count: u64,
}

/// One drained batch: `(next_cursor, rows)` where `next_cursor` is
/// `Some(last_id)` while more rows may remain and `None` once drained.
pub type MirrorResyncBatch = (Option<String>, Vec<(String, Vec<f32>)>);

/// Row source for [`resync_collection`]: pulls owned batches out of a
/// SQLite vec table so no rusqlite borrow is ever held across an `.await`.
///
/// Implementations must return rows ordered by their text primary key and
/// use `cursor` for keyset pagination (`None` = from the beginning). An
/// empty batch ends the drain.
pub trait MirrorResyncSource: Send {
    /// Fetch up to `max` `(id, vector)` rows after `cursor`.
    ///
    /// Returns [`MirrorResyncBatch`]: an empty batch (or a `None` cursor)
    /// means drained.
    fn next_batch(
        &mut self,
        cursor: Option<String>,
        max: usize,
    ) -> Result<MirrorResyncBatch, MirrorError>;
}

/// Decode little-endian f32 bytes (the SQLite vec BLOB encoding).
pub(crate) fn decode_f32_le(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
}

/// [`MirrorResyncSource`] draining the memory index `chunks_vec` table.
///
/// Opens its own read-only connection so the owning index connection is
/// never borrowed across an await. Orphan vec rows (ids no longer in
/// `chunks`) are mirrored too — the read path re-joins live chunk ids — so
/// the mirrored count matches the raw `chunks_vec` count the readiness gate
/// compares against.
pub struct MemoryVecResyncSource {
    conn: rusqlite::Connection,
}

impl MemoryVecResyncSource {
    /// Open a read-only drain connection over the memory index database.
    pub fn open(db_path: &Path) -> Result<Self, MirrorError> {
        super::index::init_sqlite_vec();
        let conn = rusqlite::Connection::open_with_flags(
            db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .map_err(|e| {
            MirrorError::with_detail(
                MirrorErrorKind::SourceUnavailable,
                format!("open memory index for resync: {e}"),
                None,
            )
        })?;
        Ok(Self { conn })
    }
}

impl MirrorResyncSource for MemoryVecResyncSource {
    fn next_batch(
        &mut self,
        cursor: Option<String>,
        max: usize,
    ) -> Result<MirrorResyncBatch, MirrorError> {
        const SQL: &str = "SELECT chunk_id, embedding FROM chunks_vec \
             WHERE (?1 IS NULL OR chunk_id > ?1) \
             ORDER BY chunk_id LIMIT ?2";
        let mut stmt = self.conn.prepare(SQL).map_err(mirror_source_error)?;
        let rows = stmt
            .query_map(rusqlite::params![cursor, max as i64], |row| {
                let id: String = row.get(0)?;
                let blob: Vec<u8> = row.get(1)?;
                Ok((id, decode_f32_le(&blob)))
            })
            .map_err(mirror_source_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(mirror_source_error)?;
        Ok(next_cursor(rows, max))
    }
}

/// Keyset pagination bookkeeping shared by the vec-table sources: returns
/// the next cursor when the batch is full, `None` when drained.
fn next_cursor(
    rows: Vec<(String, Vec<f32>)>,
    max: usize,
) -> (Option<String>, Vec<(String, Vec<f32>)>) {
    if rows.len() < max.max(1) {
        (None, rows)
    } else {
        let cursor = rows.last().map(|(id, _)| id.clone());
        (cursor, rows)
    }
}

fn mirror_source_error(e: rusqlite::Error) -> MirrorError {
    MirrorError::with_detail(MirrorErrorKind::SourceUnavailable, e.to_string(), None)
}

/// Repopulate the mirror collection for `fingerprint_hash` from SQLite.
///
/// Marks the handle [`MirrorState::Syncing`], ensures the collection exists
/// (recreating it on fingerprint-tag mismatch — it then starts empty), drains
/// the source in [`RESYNC_BATCH_ROWS`]-sized idempotent upserts, and verifies
/// the server row count with bounded retries before marking
/// [`MirrorState::Ready`]. Any failure marks the handle
/// [`MirrorState::Unavailable`] and returns the error; SQLite keeps serving.
pub async fn resync_collection(
    handle: &MirrorHandle,
    fingerprint_hash: &str,
    dims: u32,
    source: &mut dyn MirrorResyncSource,
    timeout: Duration,
) -> Result<ResyncReport, MirrorError> {
    handle.mark_syncing();
    let name = handle.collection().to_owned();

    if let Err(e) = mirror_call(
        handle
            .mirror()
            .ensure_collection(&name, dims, fingerprint_hash),
        timeout,
    )
    .await
    {
        handle.mark_unavailable();
        return Err(e);
    }

    let mut cursor: Option<String> = None;
    let mut rows_upserted: u64 = 0;
    let mut batches: usize = 0;
    loop {
        let (next, batch) = match source.next_batch(cursor.clone(), RESYNC_BATCH_ROWS) {
            Ok(result) => result,
            Err(e) => {
                handle.mark_unavailable();
                return Err(e);
            }
        };
        if batch.is_empty() {
            break;
        }
        let ids: Vec<String> = batch.iter().map(|(id, _)| id.clone()).collect();
        let vectors: Vec<Vec<f32>> = batch.into_iter().map(|(_, vector)| vector).collect();
        if let Err(e) = mirror_call(
            handle
                .mirror()
                .upsert(&name, &ids, &vectors, fingerprint_hash),
            timeout,
        )
        .await
        {
            handle.mark_unavailable();
            return Err(e);
        }
        rows_upserted += ids.len() as u64;
        batches += 1;
        cursor = next;
        if cursor.is_none() {
            break;
        }
    }

    // Verify via count with bounded retry: Milvus upserts need a moment
    // before the new rows are visible to stats.
    let mut verified: Option<u64> = None;
    for attempt in 0..RESYNC_VERIFY_ATTEMPTS {
        match mirror_call(handle.mirror().count(&name, fingerprint_hash), timeout).await {
            Ok(count) if count == rows_upserted => {
                verified = Some(count);
                break;
            }
            Ok(_) if attempt + 1 < RESYNC_VERIFY_ATTEMPTS => {
                tokio::time::sleep(RESYNC_VERIFY_BACKOFF).await;
            }
            Ok(_) => break,
            Err(e) => {
                handle.mark_unavailable();
                return Err(e);
            }
        }
    }
    match verified {
        Some(count) => {
            handle.mark_ready(fingerprint_hash, dims, count);
            Ok(ResyncReport {
                rows_upserted,
                batches,
                mirror_row_count: count,
            })
        }
        None => {
            handle.mark_unavailable();
            Err(MirrorError::with_detail(
                MirrorErrorKind::Transient,
                format!(
                    "resync count verification failed: upserted {rows_upserted}, \
                     server never reported the same count"
                ),
                None,
            ))
        }
    }
}

/// Incremental fan-out: upsert `rows` in bounded batches, then reconcile the
/// mirror row count against `expected_count` (the SQLite `vec_row_count`).
///
/// On success the handle is marked ready when the counts agree, or left
/// syncing (reads fall back to SQLite) when they do not — the next full
/// [`resync_collection`] heals the drift. Any failure marks the handle
/// unavailable. An empty `rows` slice only reconciles.
pub async fn mirror_sync_rows(
    handle: &MirrorHandle,
    fingerprint_hash: &str,
    dims: u32,
    rows: Vec<(String, Vec<f32>)>,
    expected_count: u64,
    timeout: Duration,
) -> Result<(), MirrorError> {
    let name = handle.collection().to_owned();
    for batch in rows.chunks(RESYNC_BATCH_ROWS) {
        let ids: Vec<String> = batch.iter().map(|(id, _)| id.clone()).collect();
        let vectors: Vec<Vec<f32>> = batch.iter().map(|(_, vector)| vector.clone()).collect();
        if let Err(e) = mirror_call(
            handle
                .mirror()
                .upsert(&name, &ids, &vectors, fingerprint_hash),
            timeout,
        )
        .await
        {
            handle.mark_unavailable();
            return Err(e);
        }
    }
    match mirror_call(handle.mirror().count(&name, fingerprint_hash), timeout).await {
        Ok(count) if count == expected_count => {
            handle.mark_ready(fingerprint_hash, dims, count);
            Ok(())
        }
        Ok(_) => {
            handle.mark_syncing();
            Ok(())
        }
        Err(e) => {
            handle.mark_unavailable();
            Err(e)
        }
    }
}

/// Best-effort delete fan-out: removes `ids` from the mirror collection in
/// bounded batches. Missing ids are not an error.
pub async fn mirror_delete_ids(
    handle: &MirrorHandle,
    ids: &[String],
    timeout: Duration,
) -> Result<(), MirrorError> {
    if ids.is_empty() {
        return Ok(());
    }
    let name = handle.collection().to_owned();
    for batch in ids.chunks(RESYNC_BATCH_ROWS) {
        if let Err(e) = mirror_call(handle.mirror().delete(&name, batch), timeout).await {
            handle.mark_unavailable();
            return Err(e);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// In-memory test mirror
// ---------------------------------------------------------------------------

#[derive(Default, Clone)]
struct InMemoryCollectionState {
    dims: u32,
    fingerprint_hash: String,
    schema_version: u32,
    rows: std::collections::HashMap<String, MemoryRow>,
}

/// Thread-safe in-memory [`VectorMirror`] for hermetic testing.
///
/// Supports both v1 (vector-only) and v2 (text + BM25 + dense + metadata)
/// schemas, with exact tag recreation and keyword matching.
#[derive(Default)]
pub struct InMemoryVectorMirror {
    collections: std::sync::Mutex<std::collections::HashMap<String, InMemoryCollectionState>>,
}

impl InMemoryVectorMirror {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if collection exists and has a specific schema version.
    pub fn collection_schema_version(&self, name: &str) -> Option<u32> {
        let lock = self.collections.lock().unwrap();
        lock.get(name).map(|c| c.schema_version)
    }

    /// Get all row ids in a collection.
    pub fn row_ids(&self, name: &str) -> Vec<String> {
        let lock = self.collections.lock().unwrap();
        lock.get(name)
            .map(|c| c.rows.keys().cloned().collect())
            .unwrap_or_default()
    }
}

#[async_trait]
impl VectorMirror for InMemoryVectorMirror {
    fn backend_id(&self) -> &str {
        "in-memory"
    }

    async fn ensure_collection(
        &self,
        name: &str,
        dims: u32,
        fingerprint_hash: &str,
    ) -> Result<(), MirrorError> {
        let mut lock = self.collections.lock().unwrap();
        if let Some(col) = lock.get_mut(name) {
            if col.dims == dims && col.fingerprint_hash == fingerprint_hash && col.schema_version == 1 {
                return Ok(());
            }
        }
        lock.insert(
            name.to_owned(),
            InMemoryCollectionState {
                dims,
                fingerprint_hash: fingerprint_hash.to_owned(),
                schema_version: 1,
                rows: std::collections::HashMap::new(),
            },
        );
        Ok(())
    }

    async fn upsert(
        &self,
        name: &str,
        ids: &[String],
        vectors: &[Vec<f32>],
        fingerprint_hash: &str,
    ) -> Result<(), MirrorError> {
        let mut lock = self.collections.lock().unwrap();
        let col = lock.get_mut(name).ok_or_else(|| MirrorError::new(MirrorErrorKind::SourceUnavailable))?;
        for (id, vec) in ids.iter().zip(vectors.iter()) {
            col.rows.insert(
                id.clone(),
                MemoryRow {
                    id: id.clone(),
                    text: String::new(),
                    vector: vec.clone(),
                    fingerprint_hash: fingerprint_hash.to_owned(),
                    hash: String::new(),
                    source: "memory".to_owned(),
                    path: String::new(),
                    created_at: 0,
                },
            );
        }
        Ok(())
    }

    async fn delete(&self, name: &str, ids: &[String]) -> Result<(), MirrorError> {
        let mut lock = self.collections.lock().unwrap();
        if let Some(col) = lock.get_mut(name) {
            for id in ids {
                col.rows.remove(id);
            }
        }
        Ok(())
    }

    async fn knn(
        &self,
        name: &str,
        query: &[f32],
        k: usize,
        fingerprint_hash: &str,
    ) -> Result<Vec<(String, f32)>, MirrorError> {
        let lock = self.collections.lock().unwrap();
        let col = lock.get(name).ok_or_else(|| MirrorError::new(MirrorErrorKind::SourceUnavailable))?;
        let mut scored: Vec<(String, f32)> = col
            .rows
            .values()
            .filter(|r| r.fingerprint_hash == fingerprint_hash)
            .map(|r| {
                let dist = euclidean_l2(&r.vector, query);
                (r.id.clone(), dist)
            })
            .collect();
        scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(k.max(1));
        Ok(scored)
    }

    async fn count(&self, name: &str, fingerprint_hash: &str) -> Result<u64, MirrorError> {
        let lock = self.collections.lock().unwrap();
        let count = lock
            .get(name)
            .map(|c| c.rows.values().filter(|r| r.fingerprint_hash == fingerprint_hash).count() as u64)
            .unwrap_or(0);
        Ok(count)
    }

    async fn drop_collection(&self, name: &str) -> Result<(), MirrorError> {
        let mut lock = self.collections.lock().unwrap();
        lock.remove(name);
        Ok(())
    }

    async fn ensure_collection_v2(
        &self,
        name: &str,
        dims: u32,
        fingerprint_hash: &str,
    ) -> Result<(), MirrorError> {
        let mut lock = self.collections.lock().unwrap();
        if let Some(col) = lock.get_mut(name) {
            if col.dims == dims && col.fingerprint_hash == fingerprint_hash && col.schema_version == MEMORY_SCHEMA_VERSION_V2 {
                return Ok(());
            }
        }
        lock.insert(
            name.to_owned(),
            InMemoryCollectionState {
                dims,
                fingerprint_hash: fingerprint_hash.to_owned(),
                schema_version: MEMORY_SCHEMA_VERSION_V2,
                rows: std::collections::HashMap::new(),
            },
        );
        Ok(())
    }

    async fn upsert_rows_v2(
        &self,
        name: &str,
        rows: &[MemoryRow],
    ) -> Result<(), MirrorError> {
        let mut lock = self.collections.lock().unwrap();
        let col = lock.get_mut(name).ok_or_else(|| MirrorError::new(MirrorErrorKind::SourceUnavailable))?;
        for row in rows {
            col.rows.insert(row.id.clone(), row.clone());
        }
        Ok(())
    }

    async fn bm25_search_v2(
        &self,
        name: &str,
        query: &str,
        k: usize,
        fingerprint_hash: &str,
    ) -> Result<Vec<RemoteSearchHit>, MirrorError> {
        let lock = self.collections.lock().unwrap();
        let col = lock.get(name).ok_or_else(|| MirrorError::new(MirrorErrorKind::SourceUnavailable))?;
        let terms: Vec<String> = query
            .split_whitespace()
            .map(|w| w.to_lowercase())
            .filter(|w| !w.is_empty())
            .collect();
        let mut hits = Vec::new();
        for row in col.rows.values() {
            if row.fingerprint_hash != fingerprint_hash {
                continue;
            }
            let text_lower = row.text.to_lowercase();
            let mut match_count = 0;
            for term in &terms {
                if text_lower.contains(term.as_str()) {
                    match_count += 1;
                }
            }
            if match_count > 0 {
                hits.push(RemoteSearchHit {
                    id: row.id.clone(),
                    score: match_count as f32,
                    text: row.text.clone(),
                    path: row.path.clone(),
                    source: row.source.clone(),
                    created_at: row.created_at,
                });
            }
        }
        hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        hits.truncate(k.max(1));
        Ok(hits)
    }

    async fn knn_v2(
        &self,
        name: &str,
        query: &[f32],
        k: usize,
        fingerprint_hash: &str,
    ) -> Result<Vec<RemoteSearchHit>, MirrorError> {
        let lock = self.collections.lock().unwrap();
        let col = lock.get(name).ok_or_else(|| MirrorError::new(MirrorErrorKind::SourceUnavailable))?;
        let mut hits: Vec<RemoteSearchHit> = col
            .rows
            .values()
            .filter(|r| r.fingerprint_hash == fingerprint_hash)
            .map(|r| {
                let dist = euclidean_l2(&r.vector, query);
                RemoteSearchHit {
                    id: r.id.clone(),
                    score: dist,
                    text: r.text.clone(),
                    path: r.path.clone(),
                    source: r.source.clone(),
                    created_at: r.created_at,
                }
            })
            .collect();
        hits.sort_by(|a, b| a.score.partial_cmp(&b.score).unwrap_or(std::cmp::Ordering::Equal));
        hits.truncate(k.max(1));
        Ok(hits)
    }

    async fn list_id_hashes_v2(
        &self,
        name: &str,
        fingerprint_hash: &str,
    ) -> Result<std::collections::HashMap<String, String>, MirrorError> {
        let lock = self.collections.lock().unwrap();
        let col = lock.get(name).ok_or_else(|| MirrorError::new(MirrorErrorKind::SourceUnavailable))?;
        let map = col
            .rows
            .values()
            .filter(|r| r.fingerprint_hash == fingerprint_hash)
            .map(|r| (r.id.clone(), r.hash.clone()))
            .collect();
        Ok(map)
    }
}

fn euclidean_l2(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f32>()
        .sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn similarity_conversion_matches_sqlite_vec_l2_contract() {
        assert_eq!(similarity_to_l2_distance(1.0), 0.0);
        // Orthogonal unit vectors: sqlite-vec reports sqrt(2) (verified
        // empirically by index::sqlite_vec_knn_returns_true_euclidean_l2).
        assert!((similarity_to_l2_distance(0.0) - f32::sqrt(2.0)).abs() < 1e-6);
        assert_eq!(similarity_to_l2_distance(-1.0), 2.0);
        // 0.92 cosine similarity: sqrt(2·0.08) — the same euclidean
        // distance sqlite-vec would report for that geometry.
        assert!((similarity_to_l2_distance(0.92) - 0.4).abs() < 1e-6);
    }

    #[test]
    fn similarity_conversion_clamps_overshoot() {
        // Tiny float overshoot above 1.0 must clamp to zero, not go negative.
        assert_eq!(similarity_to_l2_distance(1.0 + 1e-6), 0.0);
        // Out-of-range similarities clamp to the [0, 2] distance contract.
        assert_eq!(similarity_to_l2_distance(-1.5), 2.0);
        assert_eq!(similarity_to_l2_distance(2.0), 0.0);
    }

    #[test]
    fn similarity_conversion_maps_nan_to_max_distance() {
        // f32::clamp propagates NaN; a malformed NaN similarity must read as
        // maximally distant instead of poisoning the [0, 2] contract.
        assert_eq!(similarity_to_l2_distance(f32::NAN), 2.0);
        assert!((0.0..=2.0).contains(&similarity_to_l2_distance(f32::NAN)));
    }

    #[test]
    fn collection_tag_round_trips() {
        let tag = collection_tag("ab12cd34ef56ab12", 1024, 1);
        assert_eq!(tag, "grok:ab12cd34ef56ab12:1024:1");
        let parsed = parse_collection_tag(&tag).unwrap();
        assert_eq!(parsed.0, "ab12cd34ef56ab12");
        assert_eq!(parsed.1, 1024);
        assert_eq!(parsed.2, 1);
    }

    #[test]
    fn collection_tag_rejects_absent_and_malformed() {
        assert!(parse_collection_tag("").is_none());
        assert!(parse_collection_tag("hello world").is_none());
        assert!(parse_collection_tag("grok:").is_none());
        // Shape-level validation only: any non-empty lowercase-hex hash
        // parses (the fingerprint digest is fixed-length upstream).
        assert!(parse_collection_tag("grok:abc:1024:1").is_some());
        assert!(parse_collection_tag("grok::1024:1").is_none());
        assert!(parse_collection_tag("grok:ab12cd34ef56ab12:1024").is_none());
        assert!(parse_collection_tag("grok:ab12cd34ef56ab12:1024:1:extra").is_none());
        assert!(parse_collection_tag("grok:ab12cd34ef56ab12:notdims:1").is_none());
        assert!(parse_collection_tag("grok:AB12CD34EF56AB12:1024:1").is_none());
    }

    #[test]
    fn collection_names_follow_plan_scheme() {
        assert_eq!(
            memory_collection_name("0123abcd0123abcd"),
            "grok_mem_0123abcd0123abcd"
        );
        assert_eq!(
            prime_collection_name("0123abcd0123abcd", "skills"),
            "grok_prime_0123abcd0123abcd_skills"
        );
        assert_eq!(
            prime_collection_name("0123abcd0123abcd", "callable_agents"),
            "grok_prime_0123abcd0123abcd_callable_agents"
        );
    }

    #[test]
    fn mirror_error_debug_is_secret_free() {
        let err = MirrorError::with_detail(
            MirrorErrorKind::Transient,
            "upsert failed: token=super-secret-token body=<html>…</html>",
            Some("super-secret-token"),
        );
        let debug = format!("{err:?}");
        assert!(!debug.contains("super-secret-token"), "{debug}");
        assert!(!debug.contains("<html>"), "{debug}");
        assert_eq!(err.kind(), MirrorErrorKind::Transient);
        // Display (single-line tracing) is redacted too.
        assert!(!err.to_string().contains("super-secret-token"));
        assert!(err.detail().contains("[redacted]"));
    }

    #[test]
    fn mirror_error_detail_redacts_control_split_and_percent_encoded_token() {
        // Token split by injected control characters in the server body:
        // normalization re-joins the token before redaction.
        let err = MirrorError::with_detail(
            MirrorErrorKind::Malformed,
            "err: super-\rsecret-token leaked",
            Some("super-secret-token"),
        );
        assert!(
            !err.detail().contains("super-secret-token"),
            "{}",
            err.detail()
        );

        // Token percent-encoded in a server error URL (`:` and `/` are
        // always escaped by URL encoders).
        let err = MirrorError::with_detail(
            MirrorErrorKind::Malformed,
            "GET https://srv/v1?tok=super%3Asecret%2Ftoken failed",
            Some("super:secret/token"),
        );
        assert!(
            !err.detail().contains("super%3Asecret%2Ftoken"),
            "{}",
            err.detail()
        );
    }

    #[test]
    fn mirror_error_detail_is_truncated() {
        let long = "x".repeat(10_000);
        let err = MirrorError::with_detail(MirrorErrorKind::Malformed, long, None);
        assert_eq!(err.detail().chars().count(), 240);
    }

    #[test]
    fn error_kind_classification_matrix() {
        // Only network-shaped failures and cancellation are retryable.
        assert!(!MirrorErrorKind::SourceUnavailable.is_transient());
        assert!(MirrorErrorKind::Transient.is_transient());
        assert!(!MirrorErrorKind::Malformed.is_transient());
        assert!(!MirrorErrorKind::BudgetExhausted.is_transient());
        // Cancelled is retryable: mirror ops are idempotent and SQLite is
        // the authority, so a later attempt is always safe.
        assert!(MirrorErrorKind::Cancelled.is_transient());
    }

    #[test]
    fn readiness_gate_checks_fingerprint_and_dims() {
        let handle = MirrorHandle::new(Arc::new(NoopMirror), "grok_mem_test");
        assert_eq!(handle.collection(), "grok_mem_test");
        assert!(!handle.is_ready_for("h1", 8));
        handle.mark_syncing();
        assert_eq!(handle.snapshot().state, MirrorState::Syncing);
        handle.mark_ready("h1", 8, 42);
        assert!(handle.is_ready_for("h1", 8));
        assert!(!handle.is_ready_for("h2", 8));
        assert!(!handle.is_ready_for("h1", 16));
        assert!(handle.is_ready_for_count("h1", 8, 42));
        assert!(!handle.is_ready_for_count("h1", 8, 41));
        handle.mark_unavailable();
        assert!(!handle.is_ready_for("h1", 8));
    }

    #[test]
    fn readiness_count_gate_reads_one_snapshot() {
        // A concurrent mark_unavailable between the fp/dims and count
        // comparisons must never yield a ready verdict (single snapshot).
        let handle = MirrorHandle::new(Arc::new(NoopMirror), "grok_mem_test");
        handle.mark_ready("h1", 8, 7);
        assert!(handle.is_ready_for_count("h1", 8, 7));
        handle.update(MirrorSnapshot {
            state: MirrorState::Unavailable,
            fingerprint_hash: Some("h1".into()),
            dimensions: Some(8),
            row_count: Some(7),
        });
        assert!(!handle.is_ready_for_count("h1", 8, 7));
    }

    #[test]
    fn timeout_resolution_defaults_and_floors() {
        assert_eq!(mirror_timeout(None), Duration::from_secs(10));
        assert_eq!(mirror_timeout(Some(30)), Duration::from_secs(30));
        assert_eq!(mirror_timeout(Some(0)), Duration::from_secs(1));
    }

    #[test]
    fn decode_f32_le_round_trips() {
        let value = vec![1.5f32, -2.25, 0.0];
        let blob: Vec<u8> = value.iter().flat_map(|f| f.to_le_bytes()).collect();
        assert_eq!(decode_f32_le(&blob), value);
        // Trailing bytes are ignored, not misparsed.
        let mut short = blob.clone();
        short.push(0xAB);
        assert_eq!(decode_f32_le(&short), value);
    }

    #[tokio::test]
    async fn resync_drains_batches_and_verifies_count() {
        let mirror = Arc::new(ScriptedMirror::default());
        let handle = MirrorHandle::new(mirror.clone(), "grok_mem_test");
        let mut source = VecSource::new(vec![
            ("a".to_owned(), vec![1.0, 0.0]),
            ("b".to_owned(), vec![0.0, 1.0]),
            ("c".to_owned(), vec![0.5, 0.5]),
        ]);
        let report = resync_collection(&handle, "fp1", 2, &mut source, Duration::from_secs(5))
            .await
            .unwrap();
        assert_eq!(report.rows_upserted, 3);
        assert_eq!(report.mirror_row_count, 3);
        assert!(handle.is_ready_for_count("fp1", 2, 3));
        // Upserts are idempotent full-row replaces tagged with the fingerprint.
        assert_eq!(mirror.upsert_calls(), 1);
        assert_eq!(
            mirror.upserted_ids(),
            vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]
        );
        assert_eq!(mirror.fingerprints(), vec!["fp1".to_owned()]);
    }

    #[tokio::test]
    async fn resync_count_mismatch_marks_unavailable() {
        let mirror = Arc::new(ScriptedMirror::default());
        mirror.set_count_override(Some(99));
        let handle = MirrorHandle::new(mirror.clone(), "grok_mem_test");
        let mut source = VecSource::new(vec![("a".to_owned(), vec![1.0])]);
        let err = resync_collection(&handle, "fp1", 2, &mut source, Duration::from_secs(5))
            .await
            .unwrap_err();
        assert_eq!(err.kind(), MirrorErrorKind::Transient);
        assert_eq!(handle.snapshot().state, MirrorState::Unavailable);
    }

    #[tokio::test]
    async fn resync_mirror_failure_marks_unavailable_and_falls_back() {
        let mirror = Arc::new(ScriptedMirror::default());
        mirror.fail_upserts(true);
        let handle = MirrorHandle::new(mirror.clone(), "grok_mem_test");
        let mut source = VecSource::new(vec![("a".to_owned(), vec![1.0])]);
        let err = resync_collection(&handle, "fp1", 2, &mut source, Duration::from_secs(5))
            .await
            .unwrap_err();
        assert_eq!(err.kind(), MirrorErrorKind::Transient);
        assert_eq!(handle.snapshot().state, MirrorState::Unavailable);
    }

    #[tokio::test]
    async fn incremental_sync_reconciles_count_and_marks_state() {
        let mirror = Arc::new(ScriptedMirror::default());
        let handle = MirrorHandle::new(mirror.clone(), "grok_mem_test");
        // Count agrees → Ready.
        mirror.set_count_override(Some(4));
        mirror_sync_rows(
            &handle,
            "fp1",
            2,
            vec![("a".to_owned(), vec![1.0, 0.0])],
            4,
            Duration::from_secs(5),
        )
        .await
        .unwrap();
        assert!(handle.is_ready_for_count("fp1", 2, 4));
        // Count disagrees → Syncing (reads fall back, next resync heals).
        mirror.set_count_override(Some(9));
        mirror_sync_rows(&handle, "fp1", 2, vec![], 5, Duration::from_secs(5))
            .await
            .unwrap();
        assert_eq!(handle.snapshot().state, MirrorState::Syncing);
    }

    #[tokio::test]
    async fn delete_fanout_batches_ids() {
        let mirror = Arc::new(ScriptedMirror::default());
        let handle = MirrorHandle::new(mirror.clone(), "grok_mem_test");
        let ids: Vec<String> = (0..600).map(|i| format!("id{i}")).collect();
        mirror_delete_ids(&handle, &ids, Duration::from_secs(5))
            .await
            .unwrap();
        assert_eq!(mirror.deleted_ids().len(), 600);
        // Empty delete is a no-op round-trip.
        mirror_delete_ids(&handle, &[], Duration::from_secs(5))
            .await
            .unwrap();
        assert_eq!(mirror.deleted_ids().len(), 600);
    }

    /// Static in-memory row source for resync tests.
    struct VecSource {
        rows: Vec<(String, Vec<f32>)>,
        pos: usize,
    }

    impl VecSource {
        fn new(rows: Vec<(String, Vec<f32>)>) -> Self {
            Self { rows, pos: 0 }
        }
    }

    impl MirrorResyncSource for VecSource {
        fn next_batch(
            &mut self,
            _cursor: Option<String>,
            max: usize,
        ) -> Result<(Option<String>, Vec<(String, Vec<f32>)>), MirrorError> {
            if self.pos >= self.rows.len() {
                return Ok((None, Vec::new()));
            }
            let end = (self.pos + max).min(self.rows.len());
            let batch = self.rows[self.pos..end].to_vec();
            self.pos = end;
            if self.pos >= self.rows.len() {
                Ok((None, batch))
            } else {
                Ok((batch.last().map(|(id, _)| id.clone()), batch))
            }
        }
    }

    /// Scripted fake mirror recording calls and simulating failure modes.
    #[derive(Default)]
    struct ScriptedMirror {
        state: std::sync::Mutex<ScriptedState>,
    }

    #[derive(Default)]
    struct ScriptedState {
        upserts: Vec<(Vec<String>, String)>,
        deletes: Vec<String>,
        fail_upserts: bool,
        count_override: Option<u64>,
    }

    impl ScriptedMirror {
        fn upsert_calls(&self) -> usize {
            self.state.lock().unwrap().upserts.len()
        }

        fn upserted_ids(&self) -> Vec<String> {
            self.state
                .lock()
                .unwrap()
                .upserts
                .iter()
                .flat_map(|(ids, _)| ids.clone())
                .collect()
        }

        fn fingerprints(&self) -> Vec<String> {
            self.state
                .lock()
                .unwrap()
                .upserts
                .iter()
                .map(|(_, fp)| fp.clone())
                .collect()
        }

        fn deleted_ids(&self) -> Vec<String> {
            self.state.lock().unwrap().deletes.clone()
        }

        fn fail_upserts(&self, fail: bool) {
            self.state.lock().unwrap().fail_upserts = fail;
        }

        fn set_count_override(&self, count: Option<u64>) {
            self.state.lock().unwrap().count_override = count;
        }
    }

    #[async_trait]
    impl VectorMirror for ScriptedMirror {
        fn backend_id(&self) -> &str {
            "scripted"
        }

        async fn ensure_collection(
            &self,
            _name: &str,
            _dims: u32,
            _fingerprint_hash: &str,
        ) -> Result<(), MirrorError> {
            Ok(())
        }

        async fn upsert(
            &self,
            _name: &str,
            ids: &[String],
            _vectors: &[Vec<f32>],
            fingerprint_hash: &str,
        ) -> Result<(), MirrorError> {
            let mut state = self.state.lock().unwrap();
            if state.fail_upserts {
                return Err(MirrorError::new(MirrorErrorKind::Transient));
            }
            state
                .upserts
                .push((ids.to_vec(), fingerprint_hash.to_owned()));
            Ok(())
        }

        async fn delete(&self, _name: &str, ids: &[String]) -> Result<(), MirrorError> {
            self.state
                .lock()
                .unwrap()
                .deletes
                .extend(ids.iter().cloned());
            Ok(())
        }

        async fn knn(
            &self,
            _name: &str,
            _query: &[f32],
            _k: usize,
            _fingerprint_hash: &str,
        ) -> Result<Vec<(String, f32)>, MirrorError> {
            Ok(Vec::new())
        }

        async fn count(&self, _name: &str, _fingerprint_hash: &str) -> Result<u64, MirrorError> {
            let state = self.state.lock().unwrap();
            Ok(state.count_override.unwrap_or_else(|| {
                state
                    .upserts
                    .iter()
                    .map(|(ids, _)| ids.len())
                    .sum::<usize>() as u64
            }))
        }

        async fn drop_collection(&self, _name: &str) -> Result<(), MirrorError> {
            Ok(())
        }
    }

    /// Minimal in-memory mirror used by seam tests.
    struct NoopMirror;

    #[async_trait]
    impl VectorMirror for NoopMirror {
        fn backend_id(&self) -> &str {
            "noop"
        }

        async fn ensure_collection(
            &self,
            _name: &str,
            _dims: u32,
            _fingerprint_hash: &str,
        ) -> Result<(), MirrorError> {
            Ok(())
        }

        async fn upsert(
            &self,
            _name: &str,
            _ids: &[String],
            _vectors: &[Vec<f32>],
            _fingerprint_hash: &str,
        ) -> Result<(), MirrorError> {
            Ok(())
        }

        async fn delete(&self, _name: &str, _ids: &[String]) -> Result<(), MirrorError> {
            Ok(())
        }

        async fn knn(
            &self,
            _name: &str,
            _query: &[f32],
            _k: usize,
            _fingerprint_hash: &str,
        ) -> Result<Vec<(String, f32)>, MirrorError> {
            Ok(Vec::new())
        }

        async fn count(&self, _name: &str, _fingerprint_hash: &str) -> Result<u64, MirrorError> {
            Ok(0)
        }

        async fn drop_collection(&self, _name: &str) -> Result<(), MirrorError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn in_memory_mirror_v1_and_v2_schema_isolation() {
        let mirror = InMemoryVectorMirror::new();
        let name = "test_col";

        // Ensure v1 collection
        mirror.ensure_collection(name, 4, "fp_a").await.unwrap();
        assert_eq!(mirror.collection_schema_version(name), Some(1));

        // Calling ensure_collection_v2 drops v1 and creates v2
        mirror.ensure_collection_v2(name, 4, "fp_a").await.unwrap();
        assert_eq!(mirror.collection_schema_version(name), Some(MEMORY_SCHEMA_VERSION_V2));

        // Re-ensuring same v2 is a no-op
        mirror.ensure_collection_v2(name, 4, "fp_a").await.unwrap();
        assert_eq!(mirror.collection_schema_version(name), Some(MEMORY_SCHEMA_VERSION_V2));

        // Different fingerprint drops and recreates
        mirror.ensure_collection_v2(name, 4, "fp_b").await.unwrap();
        assert_eq!(mirror.collection_schema_version(name), Some(MEMORY_SCHEMA_VERSION_V2));
        assert_eq!(mirror.row_ids(name).len(), 0);
    }

    #[tokio::test]
    async fn in_memory_mirror_v2_upsert_search_and_list_id_hashes() {
        let mirror = InMemoryVectorMirror::new();
        let name = "test_mem_v2";
        mirror.ensure_collection_v2(name, 2, "fp1").await.unwrap();

        let row1 = MemoryRow {
            id: "id_1".to_string(),
            text: "Rust memory management and borrow checker".to_string(),
            vector: vec![1.0, 0.0],
            fingerprint_hash: "fp1".to_string(),
            hash: "hash_1".to_string(),
            source: "user".to_string(),
            path: "doc1.md".to_string(),
            created_at: 100,
        };
        let row2 = MemoryRow {
            id: "id_2".to_string(),
            text: "Python asynchronous programming with asyncio".to_string(),
            vector: vec![0.0, 1.0],
            fingerprint_hash: "fp1".to_string(),
            hash: "hash_2".to_string(),
            source: "user".to_string(),
            path: "doc2.md".to_string(),
            created_at: 200,
        };

        mirror.upsert_rows_v2(name, &[row1.clone(), row2.clone()]).await.unwrap();

        // Count
        assert_eq!(mirror.count(name, "fp1").await.unwrap(), 2);

        // List id hashes
        let id_hashes = mirror.list_id_hashes_v2(name, "fp1").await.unwrap();
        assert_eq!(id_hashes.len(), 2);
        assert_eq!(id_hashes.get("id_1").map(|s| s.as_str()), Some("hash_1"));
        assert_eq!(id_hashes.get("id_2").map(|s| s.as_str()), Some("hash_2"));

        // BM25 keyword search
        let bm25_hits = mirror.bm25_search_v2(name, "Rust borrow", 5, "fp1").await.unwrap();
        assert_eq!(bm25_hits.len(), 1);
        assert_eq!(bm25_hits[0].id, "id_1");
        assert_eq!(bm25_hits[0].text, row1.text);
        assert_eq!(bm25_hits[0].path, "doc1.md");
        assert!(bm25_hits[0].score > 0.0);

        // KNN vector search
        let knn_hits = mirror.knn_v2(name, &[0.0, 1.0], 1, "fp1").await.unwrap();
        assert_eq!(knn_hits.len(), 1);
        assert_eq!(knn_hits[0].id, "id_2");
        assert_eq!(knn_hits[0].text, row2.text);
        assert_eq!(knn_hits[0].score, 0.0);

        // Delete
        mirror.delete(name, &["id_1".to_string()]).await.unwrap();
        assert_eq!(mirror.count(name, "fp1").await.unwrap(), 1);
        let id_hashes_after = mirror.list_id_hashes_v2(name, "fp1").await.unwrap();
        assert_eq!(id_hashes_after.len(), 1);
        assert!(!id_hashes_after.contains_key("id_1"));
        assert!(id_hashes_after.contains_key("id_2"));
    }
}
