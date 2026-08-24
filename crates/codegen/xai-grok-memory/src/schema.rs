//! SQL schema constants for the memory index.
//!
//! The index uses three tables:
//! - `meta` — key-value metadata (embedding dimensions, schema version)
//! - `chunks` — indexed text chunks with blake3 content hashes
//! - `chunks_fts` — contentless FTS5 virtual table for BM25 keyword search
//!
//! When sqlite-vec is available, a fourth table is created:
//! - `chunks_vec` — vec0 virtual table for KNN vector search
//!
//! **Persisted identity privacy:** `meta[META_VECTOR_FINGERPRINT]` stores the
//! canonical fingerprint payload, which is exactly the vector-identity
//! determinant set (provider/host/endpoint labels, model, dims, protocol,
//! encoding, doc-prep, schema version). It is credential-free by design: no
//! API keys, tokens, query text, chunk text, vectors, or provider secrets are
//! ever persisted. The identity labels (`provider_instance_id`, `origin_host`)
//! are necessary identity determinants — two accounts on the same host/model
//! are different embedding spaces — and are not secrets.

/// Schema version. Bump when making breaking schema changes that require
/// dropping and recreating tables.
pub const SCHEMA_VERSION: u32 = 1;

/// meta key for the installed canonical vector fingerprint hash.
pub const META_VECTOR_FINGERPRINT_HASH: &str = "vector_fingerprint_hash";
/// meta key for the installed canonical vector fingerprint payload JSON.
pub const META_VECTOR_FINGERPRINT: &str = "vector_fingerprint";
/// meta key for the installed vector schema compat version.
pub const META_VECTOR_SCHEMA_VERSION: &str = "vector_schema_version";
/// meta key encoding the pending rebuild state (JSON, or '' when none).
pub const META_VECTOR_REBUILD_PENDING: &str = "vector_rebuild_pending";
/// meta key for the intended fingerprint currently staged ('' when none).
pub const META_VECTOR_STAGING_FP: &str = "vector_rebuild_staging_fp";
/// meta key for the persisted incremental-backfill backoff deadline (unix
/// seconds; '0' when none). Written **only** by a genuine incremental embed
/// failure (never by a batch-cap pause); suppresses the `ReadyMissing`
/// compatible-gap backfill until it passes, so a failing embedder is not
/// hammered every search while the gap still self-heals eventually (L2).
pub const META_VECTOR_BACKFILL_BACKOFF_UNTIL: &str = "vector_backfill_backoff_until";
/// meta key for the index-level (non-vector) schema version.
pub const META_SCHEMA_VERSION: &str = "schema_version";

/// Generate the SQL schema for the memory index.
///
/// `dimensions` controls the embedding vector size for `chunks_vec`.
/// If `vec_available` is false, the `chunks_vec` table is not created.
///
/// Connection pragmas (busy_timeout, journal_mode) are applied on the open
/// path (`xai_sqlite_journal::JournalMode::open`) — the journal mode depends
/// on the database's filesystem.
pub fn schema_sql(dimensions: usize, vec_available: bool) -> String {
    let mut sql = format!(
        r#"
CREATE TABLE IF NOT EXISTS meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS chunks (
    rowid INTEGER PRIMARY KEY AUTOINCREMENT,
    id TEXT UNIQUE NOT NULL,
    path TEXT NOT NULL,
    start_line INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    text TEXT NOT NULL,
    hash TEXT NOT NULL,
    source TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    access_count INTEGER DEFAULT 0,
    last_accessed INTEGER
);

CREATE INDEX IF NOT EXISTS idx_chunks_path ON chunks(path);
CREATE INDEX IF NOT EXISTS idx_chunks_hash ON chunks(hash);

CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(text, content='');

-- Transactional vector rebuild staging. Plain (non-virtual) table so a
-- partially-completed rebuild can be dropped/ignored and only swapped into
-- the live vector table atomically on success. Rows are keyed by the rebuild
-- attempt id (pending_id) plus the chunk id AND content hash, so stale async
-- results for an old intended fingerprint/incarnation, and vectors computed
-- over superseded chunk text, can never be installed over a newer pending
-- target.
CREATE TABLE IF NOT EXISTS vector_staging (
    pending_id TEXT NOT NULL,
    intended_fingerprint TEXT NOT NULL,
    chunk_id TEXT NOT NULL,
    chunk_hash TEXT NOT NULL,
    embedding BLOB NOT NULL,
    PRIMARY KEY (pending_id, chunk_id)
);

INSERT OR IGNORE INTO meta(key, value) VALUES ('reindex_claim', '');
INSERT OR IGNORE INTO meta(key, value) VALUES ('{META_SCHEMA_VERSION}', '{SCHEMA_VERSION}');
INSERT OR IGNORE INTO meta(key, value) VALUES ('{META_VECTOR_FINGERPRINT_HASH}', '');
INSERT OR IGNORE INTO meta(key, value) VALUES ('{META_VECTOR_FINGERPRINT}', '');
INSERT OR IGNORE INTO meta(key, value) VALUES ('{META_VECTOR_SCHEMA_VERSION}', '0');
INSERT OR IGNORE INTO meta(key, value) VALUES ('{META_VECTOR_REBUILD_PENDING}', '');
INSERT OR IGNORE INTO meta(key, value) VALUES ('{META_VECTOR_STAGING_FP}', '');
INSERT OR IGNORE INTO meta(key, value) VALUES ('{META_VECTOR_BACKFILL_BACKOFF_UNTIL}', '0');
"#
    );

    if vec_available {
        sql.push_str(&format!(
            "\nCREATE VIRTUAL TABLE IF NOT EXISTS chunks_vec USING vec0(\n    \
             chunk_id TEXT PRIMARY KEY,\n    \
             embedding FLOAT[{dimensions}]\n);\n"
        ));
    }

    sql
}

/// SQL to insert or update an embedding dimension record in the meta table.
pub const UPSERT_META_SQL: &str = "INSERT OR REPLACE INTO meta(key, value) VALUES (?1, ?2)";

/// SQL to query a meta value by key.
pub const GET_META_SQL: &str = "SELECT value FROM meta WHERE key = ?1";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_sql_without_vec() {
        let sql = schema_sql(1536, false);
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS chunks"));
        assert!(sql.contains("CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts"));
        assert!(!sql.contains("chunks_vec"));
        // Connection pragmas live on the open path, not in the schema batch.
        assert!(!sql.contains("PRAGMA"));
    }

    #[test]
    fn test_schema_sql_with_vec() {
        let sql = schema_sql(384, true);
        assert!(sql.contains("chunks_vec"));
        assert!(sql.contains("FLOAT[384]"));
    }

    #[test]
    fn test_schema_sql_different_dimensions() {
        let sql = schema_sql(768, true);
        assert!(sql.contains("FLOAT[768]"));
    }
}
