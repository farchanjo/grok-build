//! SQL schema for the disposable Prime metadata index.
//!
//! The database is separate from [`crate::index::MemoryIndex`]. It stores
//! only bounded, non-secret metadata for independent `skills` and
//! `callable_agents` collections. Bodies, prompts, credentials, raw provider
//! errors, absolute paths, session history, and diagnostic vector dumps are
//! not part of the schema and must never be persisted.

/// Index-level schema version. Bump when making breaking schema changes.
pub const SCHEMA_VERSION: u32 = 1;

/// meta key for the index-level schema version.
pub const META_SCHEMA_VERSION: &str = "schema_version";

/// Canonical collection names. SQL identifiers are derived only from these.
pub const COLLECTION_SKILLS: &str = "skills";
pub const COLLECTION_CALLABLE_AGENTS: &str = "callable_agents";

pub const GET_META_SQL: &str = "SELECT value FROM meta WHERE key = ?1";

/// Document-preparation version for metadata embedding text.
pub const METADATA_PREP_VERSION: &str = "prime-meta/v1";
/// Chunker/algorithm label used in the canonical fingerprint.
pub const METADATA_CHUNKER_ID: &str = "metadata";

/// Core schema: meta, collections, items, per-collection FTS, staging.
///
/// Vector virtual tables are created separately once a collection's
/// embedding dimensions are known. Connection pragmas live on the open path.
pub fn schema_sql() -> String {
    format!(
        r#"
CREATE TABLE IF NOT EXISTS meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS collections (
    name TEXT PRIMARY KEY NOT NULL,
    embedding_dimensions INTEGER NOT NULL DEFAULT 0,
    fingerprint_hash TEXT NOT NULL DEFAULT '',
    fingerprint_payload TEXT NOT NULL DEFAULT '',
    vector_schema_version INTEGER NOT NULL DEFAULT 0,
    prep_version TEXT NOT NULL DEFAULT '',
    pending_json TEXT NOT NULL DEFAULT '',
    staging_fp TEXT NOT NULL DEFAULT '',
    backoff_until INTEGER NOT NULL DEFAULT 0,
    item_count INTEGER NOT NULL DEFAULT 0,
    vec_count INTEGER NOT NULL DEFAULT 0,
    inventory_generation INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS items (
    rowid INTEGER PRIMARY KEY AUTOINCREMENT,
    collection TEXT NOT NULL,
    item_id TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    extra TEXT NOT NULL DEFAULT '',
    fts_text TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(collection, item_id)
);

CREATE INDEX IF NOT EXISTS idx_items_collection_hash ON items(collection, content_hash);

CREATE VIRTUAL TABLE IF NOT EXISTS skills_fts USING fts5(text, content='');
CREATE VIRTUAL TABLE IF NOT EXISTS callable_agents_fts USING fts5(text, content='');

CREATE TABLE IF NOT EXISTS vector_staging (
    collection TEXT NOT NULL,
    pending_id TEXT NOT NULL,
    intended_fingerprint TEXT NOT NULL,
    item_id TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    embedding BLOB NOT NULL,
    PRIMARY KEY (collection, pending_id, item_id)
);

INSERT OR IGNORE INTO meta(key, value) VALUES ('{META_SCHEMA_VERSION}', '{SCHEMA_VERSION}');
INSERT OR IGNORE INTO collections(name) VALUES ('{COLLECTION_SKILLS}');
INSERT OR IGNORE INTO collections(name) VALUES ('{COLLECTION_CALLABLE_AGENTS}');
"#
    )
}

/// SQL that (re)creates a collection-local vec0 table.
pub fn vec_table_sql(collection: &str, dimensions: usize) -> String {
    format!(
        "CREATE VIRTUAL TABLE IF NOT EXISTS {collection}_vec USING vec0(\n    \
         item_id TEXT PRIMARY KEY,\n    \
         embedding FLOAT[{dimensions}]\n);"
    )
}

pub fn drop_vec_table_sql(collection: &str) -> String {
    format!("DROP TABLE IF EXISTS {collection}_vec")
}
