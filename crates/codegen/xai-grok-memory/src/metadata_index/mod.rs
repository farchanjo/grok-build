//! Generic disposable Prime metadata index.
//!
//! Stored at `$GROK_HOME/indexes/prime/<workspace-identity>/metadata.sqlite`
//! and independent of [`crate::index::MemoryIndex`]. Skills and callable
//! agents are separate collections: a rebuild of one never drops or blocks
//! the other.
//!
//! A [`rusqlite::Connection`] is owned by [`MetadataIndex`] and is never
//! sent across `.await` points. Async rebuild helpers open a fresh index,
//! drop it before embedding, and reopen after the await.

mod rebuild;
mod schema;

#[cfg(test)]
mod isolation;
#[cfg(test)]
mod tests;

pub use rebuild::{
    CollectionPending, DEFAULT_BATCH, commit_staged_vectors, discard_collection_rebuild,
    ensure_collection_vectors_ready, stage_collection_vectors,
};
pub use schema::{METADATA_CHUNKER_ID, METADATA_PREP_VERSION, SCHEMA_VERSION};

use std::path::{Path, PathBuf};

use rusqlite::params;
use xai_sqlite_journal::JournalMode;

use crate::embedding::{l2_normalize_v1, validate_embedding_batch};
use crate::fingerprint::DocPreparationSpec;
use crate::index::init_sqlite_vec;
use crate::workspace_identity::workspace_storage_identity;

const MAX_ITEM_ID: usize = 128;
const MAX_NAME: usize = 256;
const MAX_DESCRIPTION: usize = 1024;
const MAX_EXTRA: usize = 2048;
const MAX_WHEN_TO_USE: usize = 500;
const MAX_SCOPE: usize = 64;
const MAX_PATH_LABEL: usize = 256;
const MAX_PATHS: usize = 32;

/// Independent collection stored in the metadata index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CollectionKind {
    Skills,
    CallableAgents,
}

impl CollectionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Skills => schema::COLLECTION_SKILLS,
            Self::CallableAgents => schema::COLLECTION_CALLABLE_AGENTS,
        }
    }

    pub fn all() -> [Self; 2] {
        [Self::Skills, Self::CallableAgents]
    }

    fn fts_table(self) -> &'static str {
        match self {
            Self::Skills => "skills_fts",
            Self::CallableAgents => "callable_agents_fts",
        }
    }

    pub(crate) fn vec_table(self) -> &'static str {
        match self {
            Self::Skills => "skills_vec",
            Self::CallableAgents => "callable_agents_vec",
        }
    }
}

/// Bounded, non-secret metadata row. Never carries a body, prompt,
/// credential, absolute path, session history, or vector dump.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataItem {
    pub item_id: String,
    pub content_hash: String,
    pub name: String,
    pub description: String,
    pub extra: String,
}

impl MetadataItem {
    /// Build a validated item and compute its canonical content hash.
    pub fn new(
        item_id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        extra: impl Into<String>,
    ) -> Result<Self, MetadataIndexError> {
        let item_id = item_id.into();
        let name = name.into();
        let description = description.into();
        let extra = extra.into();
        validate_item_id(&item_id)?;
        validate_name(&name)?;
        validate_description(&description)?;
        validate_extra(&extra)?;
        let content_hash = item_content_hash(&name, &description, &extra);
        Ok(Self {
            item_id,
            content_hash,
            name,
            description,
            extra,
        })
    }

    /// FTS document text (name, description, extra). Never a body or path.
    pub fn fts_text(&self) -> String {
        if self.extra.is_empty() {
            format!("{}\n{}", self.name, self.description)
        } else {
            format!("{}\n{}\n{}", self.name, self.description, self.extra)
        }
    }
}

/// Result of an incremental inventory upsert.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UpsertResult {
    pub added: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub removed: usize,
}

/// Durable collection metadata (no secrets, no vectors).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionState {
    pub name: CollectionKind,
    pub embedding_dimensions: usize,
    pub fingerprint_hash: String,
    pub vector_schema_version: u32,
    pub prep_version: String,
    pub pending_json: String,
    pub backoff_until: i64,
    pub item_count: i64,
    pub vec_count: i64,
    pub inventory_generation: i64,
}

/// FTS/BM25 hit.
#[derive(Debug, Clone, PartialEq)]
pub struct MetadataFtsHit {
    pub item_id: String,
    pub rank: f64,
}

/// sqlite-vec KNN hit. Distance is the sqlite-vec L2 distance, not a
/// diagnostic dump of the stored vector.
#[derive(Debug, Clone, PartialEq)]
pub struct MetadataKnnHit {
    pub item_id: String,
    pub distance: f32,
}

/// Errors from the metadata index. Messages are bounded and secret-free.
#[derive(Debug)]
pub enum MetadataIndexError {
    Sqlite(rusqlite::Error),
    ReadOnly,
    Privacy(&'static str),
    InvalidItem(&'static str),
}

impl std::fmt::Display for MetadataIndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sqlite(e) => write!(f, "metadata index sqlite error: {e}"),
            Self::ReadOnly => write!(
                f,
                "metadata index written by a newer schema version; refusing writes"
            ),
            Self::Privacy(reason) => write!(f, "metadata index privacy rejection: {reason}"),
            Self::InvalidItem(reason) => write!(f, "metadata index invalid item: {reason}"),
        }
    }
}

impl std::error::Error for MetadataIndexError {}

impl From<rusqlite::Error> for MetadataIndexError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Sqlite(value)
    }
}

/// Directory `$GROK_HOME/indexes/prime/<workspace-identity>/`.
pub fn metadata_index_dir(grok_home: &Path, workspace_identity: &str) -> PathBuf {
    grok_home
        .join("indexes")
        .join("prime")
        .join(workspace_identity)
}

/// Path `$GROK_HOME/indexes/prime/<workspace-identity>/metadata.sqlite`.
pub fn metadata_index_path(grok_home: &Path, workspace_identity: &str) -> PathBuf {
    metadata_index_dir(grok_home, workspace_identity).join("metadata.sqlite")
}

/// Resolve the metadata database path for a workspace cwd.
pub fn metadata_index_path_for_cwd(grok_home: &Path, cwd: &Path) -> PathBuf {
    metadata_index_path(grok_home, &workspace_storage_identity(cwd))
}

/// Canonical metadata document-preparation spec for a collection fingerprint.
pub fn metadata_doc_prep(collection: CollectionKind) -> DocPreparationSpec {
    DocPreparationSpec {
        version: schema::METADATA_PREP_VERSION.to_owned(),
        chunker: format!("{}/{}", schema::METADATA_CHUNKER_ID, collection.as_str()),
        max_chunk_chars: 0,
        chunk_overlap_chars: 0,
    }
}

/// SQLite-backed disposable metadata index.
pub struct MetadataIndex {
    db: rusqlite::Connection,
    vec_available: bool,
    writable: bool,
}

impl MetadataIndex {
    /// Open or create the index at the canonical workspace path.
    pub fn open_for_workspace(grok_home: &Path, cwd: &Path) -> Result<Self, MetadataIndexError> {
        let db_path = metadata_index_path_for_cwd(grok_home, cwd);
        Self::open_or_create(&db_path)
    }

    /// Open or create the index at `db_path`.
    pub fn open_or_create(db_path: &Path) -> Result<Self, MetadataIndexError> {
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
            restrict_dir_owner_only(parent);
            if let Some(prime) = parent.parent() {
                restrict_dir_owner_only(prime);
            }
        }
        Self::open_or_create_with_journal_mode(db_path, JournalMode::for_db_path(db_path), false)
    }

    /// Open with an explicit journal mode — the seam tests use to exercise
    /// the network-filesystem decision on a local disk.
    fn open_or_create_with_journal_mode(
        db_path: &Path,
        journal_mode: JournalMode,
        force_fts_only: bool,
    ) -> Result<Self, MetadataIndexError> {
        init_sqlite_vec();
        let db = journal_mode.open(db_path)?;

        let force_fts_only = force_fts_only || test_force_fts_only();
        let vec_loaded = match db.query_row("SELECT vec_version()", [], |r| r.get::<_, String>(0)) {
            Ok(_) => !force_fts_only,
            Err(_) => false,
        };

        let stored_schema: Option<u32> = db
            .query_row(
                schema::GET_META_SQL,
                params![schema::META_SCHEMA_VERSION],
                |r| r.get::<_, String>(0),
            )
            .ok()
            .and_then(|s| s.trim().parse().ok());
        if let Some(stored) = stored_schema
            && stored > schema::SCHEMA_VERSION
        {
            tracing::warn!(
                stored,
                current = schema::SCHEMA_VERSION,
                "prime metadata index written by a newer schema version; refusing all writes, FTS-only"
            );
            db.pragma_update(None, "query_only", true)?;
            return Ok(Self {
                db,
                vec_available: false,
                writable: false,
            });
        }

        db.execute_batch(&schema::schema_sql())?;

        if vec_loaded {
            for kind in CollectionKind::all() {
                let dims = collection_dimensions(&db, kind).unwrap_or(0);
                if dims > 0 {
                    db.execute_batch(&schema::vec_table_sql(kind.as_str(), dims))?;
                }
            }
        }

        Ok(Self {
            db,
            vec_available: vec_loaded,
            writable: true,
        })
    }

    #[cfg(test)]
    fn open_with_journal_mode_for_test(
        db_path: &Path,
        journal_mode: JournalMode,
    ) -> Result<Self, MetadataIndexError> {
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        Self::open_or_create_with_journal_mode(db_path, journal_mode, false)
    }

    pub fn vec_available(&self) -> bool {
        self.vec_available
    }

    pub fn writable(&self) -> bool {
        self.writable
    }

    pub fn schema_version(&self) -> u32 {
        self.meta_get(schema::META_SCHEMA_VERSION)
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0)
    }

    pub(crate) fn db(&self) -> &rusqlite::Connection {
        &self.db
    }

    pub(crate) fn meta_get(&self, key: &str) -> Option<String> {
        self.db
            .query_row(schema::GET_META_SQL, params![key], |r| {
                r.get::<_, String>(0)
            })
            .ok()
    }

    pub fn collection_state(
        &self,
        collection: CollectionKind,
    ) -> Result<CollectionState, MetadataIndexError> {
        let row = self.db.query_row(
            "SELECT embedding_dimensions, fingerprint_hash, vector_schema_version, \
             prep_version, pending_json, backoff_until, item_count, vec_count, \
             inventory_generation FROM collections WHERE name = ?1",
            params![collection.as_str()],
            |r| {
                Ok(CollectionState {
                    name: collection,
                    embedding_dimensions: r.get::<_, i64>(0)? as usize,
                    fingerprint_hash: r.get(1)?,
                    vector_schema_version: r.get::<_, i64>(2)? as u32,
                    prep_version: r.get(3)?,
                    pending_json: r.get(4)?,
                    backoff_until: r.get(5)?,
                    item_count: r.get(6)?,
                    vec_count: r.get(7)?,
                    inventory_generation: r.get(8)?,
                })
            },
        )?;
        Ok(row)
    }

    pub fn item_count(&self, collection: CollectionKind) -> i64 {
        self.db
            .query_row(
                "SELECT COUNT(*) FROM items WHERE collection = ?1",
                params![collection.as_str()],
                |r| r.get(0),
            )
            .unwrap_or(0)
    }

    pub fn vec_count(&self, collection: CollectionKind) -> i64 {
        if !self.vec_available || !self.vec_table_exists(collection) {
            return 0;
        }
        let sql = format!("SELECT COUNT(*) FROM {}", collection.vec_table());
        self.db.query_row(&sql, [], |r| r.get(0)).unwrap_or(0)
    }

    /// Whether a compatible-gap backfill may write into the live collection
    /// vec table. False while sqlite-vec is unavailable, the index is
    /// read-only, no fingerprint is installed, or a rebuild is pending.
    pub fn vectors_safe_to_backfill(&self, collection: CollectionKind) -> bool {
        if !self.writable || !self.vec_available || !self.vec_table_exists(collection) {
            return false;
        }
        if rebuild::pending_marker_present(self, collection) {
            return false;
        }
        self.collection_state(collection)
            .map(|s| !s.fingerprint_hash.trim().is_empty())
            .unwrap_or(false)
    }

    /// Live items that currently lack a row in the collection vec table.
    ///
    /// Used to backfill a compatible gap after incremental inventory upsert.
    /// Returns an empty set when vectors cannot be written.
    pub fn items_without_embeddings(
        &self,
        collection: CollectionKind,
    ) -> Result<Vec<(String, String)>, rusqlite::Error> {
        if !self.vec_available || !self.vec_table_exists(collection) {
            return Ok(vec![]);
        }
        let rowids = format!("{}_rowids", collection.vec_table());
        if !table_exists(&self.db, &rowids) {
            // Torn sqlite-vec shadow: every live item still needs an embedding.
            let mut stmt = self
                .db
                .prepare("SELECT i.item_id, i.fts_text FROM items i WHERE i.collection = ?1")?;
            let rows = stmt
                .query_map(params![collection.as_str()], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(rows);
        }
        let sql = format!(
            "SELECT i.item_id, i.fts_text FROM items i \
             LEFT JOIN {rowids} v ON v.id = i.item_id \
             WHERE i.collection = ?1 AND v.id IS NULL"
        );
        let mut stmt = self.db.prepare(&sql)?;
        let rows = stmt
            .query_map(params![collection.as_str()], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Orphan vec rows and missing live embeddings, observed in one snapshot.
    ///
    /// Returns `(orphan_vec_rows, missing_live_embeddings)`. `None` if the
    /// snapshot cannot be read — callers must fail closed. Count equality is
    /// not coverage: a ghost vec row plus a matching number of missing live
    /// embeddings can make `COUNT(vec) == COUNT(items)` while neither side
    /// is covered. Nested `BEGIN` (already in a writer txn) still reads the
    /// current snapshot.
    pub(crate) fn vector_join_counts(&self, collection: CollectionKind) -> Option<(i64, i64)> {
        let started_tx = self.db.execute_batch("BEGIN;").is_ok();
        let result = (|| -> Result<(i64, i64), rusqlite::Error> {
            let items: i64 = self.db.query_row(
                "SELECT COUNT(*) FROM items WHERE collection = ?1",
                params![collection.as_str()],
                |r| r.get(0),
            )?;
            if !self.vec_available || !self.vec_table_exists(collection) {
                return Ok((0, items));
            }
            let rowids = format!("{}_rowids", collection.vec_table());
            if !table_exists(&self.db, &rowids) {
                return Ok((0, items));
            }
            let orphan_sql = format!(
                "SELECT COUNT(*) FROM {rowids} v \
                 WHERE NOT EXISTS ( \
                   SELECT 1 FROM items i \
                   WHERE i.collection = ?1 AND i.item_id = v.id \
                 )"
            );
            let orphans: i64 =
                self.db
                    .query_row(&orphan_sql, params![collection.as_str()], |r| r.get(0))?;
            let missing_sql = format!(
                "SELECT COUNT(*) FROM items i \
                 WHERE i.collection = ?1 AND NOT EXISTS ( \
                   SELECT 1 FROM {rowids} v WHERE v.id = i.item_id \
                 )"
            );
            let missing: i64 =
                self.db
                    .query_row(&missing_sql, params![collection.as_str()], |r| r.get(0))?;
            Ok((orphans, missing))
        })();
        if started_tx {
            match &result {
                Ok(_) => {
                    let _ = self.db.execute_batch("COMMIT;");
                }
                Err(_) => {
                    let _ = self.db.execute_batch("ROLLBACK;");
                }
            }
        }
        result.ok()
    }

    /// Insert or replace a live embedding for an existing collection item.
    ///
    /// Writes the collection vec table in place; never stages, drops, or
    /// replaces sibling tables. No-op when sqlite-vec is unavailable or
    /// live vectors are not safe to backfill. Ghost ids (not in `items`)
    /// are rejected so KNN cannot surface rows that inventory dropped.
    ///
    /// Authority recheck, optional `vectors_safe_to_backfill` recheck,
    /// `INSERT OR REPLACE`, and `vec_count` update run in one
    /// `BEGIN IMMEDIATE` transaction so a concurrent `replace_inventory`
    /// deletion cannot race a live write into a ghost vec row.
    pub fn upsert_embedding(
        &self,
        collection: CollectionKind,
        item_id: &str,
        embedding: &[f32],
    ) -> Result<(), MetadataIndexError> {
        self.upsert_embedding_inner(collection, item_id, embedding, None)
    }

    /// Same as [`Self::upsert_embedding`], but the live collection fingerprint
    /// must equal `expected_fingerprint` inside the write transaction.
    ///
    /// A same-dimension table swap between embed and install therefore cannot
    /// land vectors from space A under space B's hash.
    pub fn upsert_embedding_for_fingerprint(
        &self,
        collection: CollectionKind,
        item_id: &str,
        embedding: &[f32],
        expected_fingerprint: &str,
    ) -> Result<(), MetadataIndexError> {
        self.upsert_embedding_inner(collection, item_id, embedding, Some(expected_fingerprint))
    }

    fn upsert_embedding_inner(
        &self,
        collection: CollectionKind,
        item_id: &str,
        embedding: &[f32],
        expected_fingerprint: Option<&str>,
    ) -> Result<(), MetadataIndexError> {
        self.require_writable()?;
        if !self.vec_available || !self.vec_table_exists(collection) {
            return Ok(());
        }
        if !self.vectors_safe_to_backfill(collection) {
            return Ok(());
        }
        // Existence is re-checked inside BEGIN IMMEDIATE. A non-transactional
        // SELECT here would race `replace_inventory` and write a ghost vec row.
        #[cfg(test)]
        wait_upsert_before_txn_pause();
        self.db.execute_batch("BEGIN IMMEDIATE;")?;
        let result = (|| -> Result<bool, MetadataIndexError> {
            if !self.vectors_safe_to_backfill(collection) {
                return Ok(false);
            }
            let state = self.collection_state(collection)?;
            if let Some(expected) = expected_fingerprint
                && state.fingerprint_hash != expected
            {
                return Err(MetadataIndexError::InvalidItem(
                    "embedding space fingerprint mismatch",
                ));
            }
            if embedding.len() != state.embedding_dimensions {
                return Err(MetadataIndexError::InvalidItem("embedding dimensions"));
            }
            validate_embedding_batch(1, state.embedding_dimensions, &[embedding.to_vec()])
                .map_err(|_| MetadataIndexError::InvalidItem("embedding batch"))?;
            let mut normalized = embedding.to_vec();
            l2_normalize_v1(&mut normalized)
                .map_err(|_| MetadataIndexError::InvalidItem("embedding normalization"))?;
            if existing_item(&self.db, collection, item_id)?.is_none() {
                return Err(MetadataIndexError::InvalidItem(
                    "embedding item does not exist",
                ));
            }
            let embedding_bytes: Vec<u8> =
                normalized.iter().flat_map(|f| f.to_le_bytes()).collect();
            let sql = format!(
                "INSERT OR REPLACE INTO {}(item_id, embedding) VALUES (?1, ?2)",
                collection.vec_table()
            );
            self.db.execute(&sql, params![item_id, embedding_bytes])?;
            let count = self.vec_count(collection);
            self.db.execute(
                "UPDATE collections SET vec_count = ?1 WHERE name = ?2",
                params![count, collection.as_str()],
            )?;
            Ok(true)
        })();
        match result {
            Ok(true) => {
                self.db.execute_batch("COMMIT;")?;
                Ok(())
            }
            Ok(false) => {
                let _ = self.db.execute_batch("ROLLBACK;");
                Ok(())
            }
            Err(e) => {
                let _ = self.db.execute_batch("ROLLBACK;");
                Err(e)
            }
        }
    }

    /// Remove vector rows whose `item_id` is no longer in `items` for this
    /// collection. Orphans make `vec_count` inaccurate. KNN already filters
    /// to authoritative live items, but prune is still required for count
    /// hygiene. Collection-local: sibling tables and the installed
    /// fingerprint are not touched.
    pub(crate) fn prune_orphan_vector_rows(&self, collection: CollectionKind) -> usize {
        if !self.writable || !self.vec_available || !self.vec_table_exists(collection) {
            return 0;
        }
        let rowids = format!("{}_rowids", collection.vec_table());
        if !table_exists(&self.db, &rowids) {
            return 0;
        }
        if self.db.execute_batch("BEGIN IMMEDIATE;").is_err() {
            return 0;
        }
        let mut removed = 0usize;
        let result = (|| -> Result<(), MetadataIndexError> {
            let sql = format!(
                "SELECT v.id FROM {rowids} v \
                 WHERE NOT EXISTS ( \
                   SELECT 1 FROM items i \
                   WHERE i.collection = ?1 AND i.item_id = v.id \
                 )"
            );
            let mut stmt = self.db.prepare(&sql)?;
            let ids: Vec<String> = stmt
                .query_map(params![collection.as_str()], |r| r.get::<_, String>(0))?
                .filter_map(Result::ok)
                .collect();
            drop(stmt);
            let vec_table = collection.vec_table();
            for id in &ids {
                let still_absent: bool = self
                    .db
                    .query_row(
                        "SELECT NOT EXISTS ( \
                           SELECT 1 FROM items i \
                           WHERE i.collection = ?1 AND i.item_id = ?2 \
                         )",
                        params![collection.as_str(), id],
                        |r| r.get::<_, bool>(0),
                    )
                    .unwrap_or(false);
                if still_absent {
                    let delete_sql = format!("DELETE FROM {vec_table} WHERE item_id = ?1");
                    removed += self.db.execute(&delete_sql, params![id])?;
                }
            }
            let count = self.vec_count(collection);
            self.db.execute(
                "UPDATE collections SET vec_count = ?1 WHERE name = ?2",
                params![count, collection.as_str()],
            )?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                let _ = self.db.execute_batch("COMMIT;");
            }
            Err(_) => {
                let _ = self.db.execute_batch("ROLLBACK;");
                removed = 0;
            }
        }
        removed
    }

    fn vec_table_exists(&self, collection: CollectionKind) -> bool {
        table_exists(&self.db, collection.vec_table())
    }

    /// Incremental upsert plus deletion of items not present in `items`.
    ///
    /// Unchanged content hashes skip FTS/vector work. Changed hashes update
    /// FTS and drop the live vector for that item so a later backfill or
    /// rebuild restages it. The sibling collection is never touched.
    pub fn replace_inventory(
        &self,
        collection: CollectionKind,
        generation: i64,
        items: &[MetadataItem],
    ) -> Result<UpsertResult, MetadataIndexError> {
        self.require_writable()?;
        let now = now_secs();
        let fts = collection.fts_table();
        let vec_table = collection.vec_table();
        let vec_ok = self.vec_available && self.vec_table_exists(collection);

        self.db.execute_batch("BEGIN IMMEDIATE;")?;
        let result = (|| -> Result<UpsertResult, MetadataIndexError> {
            let mut outcome = UpsertResult::default();
            let mut live_ids = std::collections::HashSet::with_capacity(items.len());
            for item in items {
                validate_item_id(&item.item_id)?;
                validate_name(&item.name)?;
                validate_description(&item.description)?;
                validate_extra(&item.extra)?;
                let hash = item_content_hash(&item.name, &item.description, &item.extra);
                if hash != item.content_hash {
                    return Err(MetadataIndexError::InvalidItem(
                        "content hash does not match canonical metadata fields",
                    ));
                }
                live_ids.insert(item.item_id.clone());
                let fts_text = item.fts_text();
                match existing_item(&self.db, collection, &item.item_id)? {
                    Some((_, old_hash, _)) if old_hash == hash => {
                        outcome.unchanged += 1;
                    }
                    Some((rowid, _, old_fts)) => {
                        delete_fts(&self.db, fts, rowid, &old_fts)?;
                        self.db.execute(
                            "UPDATE items SET content_hash = ?1, name = ?2, description = ?3, \
                             extra = ?4, fts_text = ?5, updated_at = ?6 \
                             WHERE collection = ?7 AND item_id = ?8",
                            params![
                                hash,
                                item.name,
                                item.description,
                                item.extra,
                                fts_text,
                                now,
                                collection.as_str(),
                                item.item_id
                            ],
                        )?;
                        insert_fts(&self.db, fts, rowid, &fts_text)?;
                        if vec_ok {
                            let sql = format!("DELETE FROM {vec_table} WHERE item_id = ?1");
                            self.db.execute(&sql, params![item.item_id])?;
                        }
                        outcome.updated += 1;
                    }
                    None => {
                        self.db.execute(
                            "INSERT INTO items(collection, item_id, content_hash, name, \
                             description, extra, fts_text, created_at, updated_at) \
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                            params![
                                collection.as_str(),
                                item.item_id,
                                hash,
                                item.name,
                                item.description,
                                item.extra,
                                fts_text,
                                now,
                                now
                            ],
                        )?;
                        let rowid = self.db.last_insert_rowid();
                        insert_fts(&self.db, fts, rowid, &fts_text)?;
                        outcome.added += 1;
                    }
                }
            }

            let existing_ids = load_item_ids(&self.db, collection)?;
            for id in existing_ids {
                if live_ids.contains(&id) {
                    continue;
                }
                if let Some((rowid, _, old_fts)) = existing_item(&self.db, collection, &id)? {
                    delete_fts(&self.db, fts, rowid, &old_fts)?;
                    if vec_ok {
                        let sql = format!("DELETE FROM {vec_table} WHERE item_id = ?1");
                        self.db.execute(&sql, params![id])?;
                    }
                    self.db.execute(
                        "DELETE FROM items WHERE collection = ?1 AND item_id = ?2",
                        params![collection.as_str(), id],
                    )?;
                    outcome.removed += 1;
                }
            }

            let count = self.item_count(collection);
            self.db.execute(
                "UPDATE collections SET item_count = ?1, inventory_generation = ?2, \
                 vec_count = ?3 WHERE name = ?4",
                params![
                    count,
                    generation,
                    self.vec_count(collection),
                    collection.as_str()
                ],
            )?;
            Ok(outcome)
        })();
        match result {
            Ok(v) => {
                self.db.execute_batch("COMMIT;")?;
                Ok(v)
            }
            Err(e) => {
                let _ = self.db.execute_batch("ROLLBACK;");
                Err(e)
            }
        }
    }

    /// Live collection items in name/id order. Never includes bodies or paths.
    pub fn list_items(
        &self,
        collection: CollectionKind,
    ) -> Result<Vec<MetadataItem>, MetadataIndexError> {
        let mut stmt = self.db.prepare(
            "SELECT item_id, content_hash, name, description, extra \
             FROM items WHERE collection = ?1 ORDER BY name, item_id",
        )?;
        let rows = stmt
            .query_map(params![collection.as_str()], |row| {
                Ok(MetadataItem {
                    item_id: row.get(0)?,
                    content_hash: row.get(1)?,
                    name: row.get(2)?,
                    description: row.get(3)?,
                    extra: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// One live item, if present.
    pub fn get_item(
        &self,
        collection: CollectionKind,
        item_id: &str,
    ) -> Result<Option<MetadataItem>, MetadataIndexError> {
        let mut stmt = self.db.prepare(
            "SELECT item_id, content_hash, name, description, extra \
             FROM items WHERE collection = ?1 AND item_id = ?2",
        )?;
        match stmt.query_row(params![collection.as_str(), item_id], |row| {
            Ok(MetadataItem {
                item_id: row.get(0)?,
                content_hash: row.get(1)?,
                name: row.get(2)?,
                description: row.get(3)?,
                extra: row.get(4)?,
            })
        }) {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn search_fts(
        &self,
        collection: CollectionKind,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MetadataFtsHit>, MetadataIndexError> {
        let keywords = crate::query_expansion::extract_keywords(query);
        let fts_query = keywords.join(" OR ");
        if fts_query.is_empty() {
            return Ok(vec![]);
        }
        let fts = collection.fts_table();
        let sql = format!(
            "SELECT i.item_id, f.rank FROM {fts} f \
             JOIN items i ON f.rowid = i.rowid \
             WHERE {fts} MATCH ?1 AND i.collection = ?2 \
             ORDER BY f.rank LIMIT ?3"
        );
        let mut stmt = self.db.prepare(&sql)?;
        let rows = stmt
            .query_map(
                params![fts_query, collection.as_str(), limit as i64],
                |row| {
                    Ok(MetadataFtsHit {
                        item_id: row.get(0)?,
                        rank: row.get(1)?,
                    })
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn search_knn(
        &self,
        collection: CollectionKind,
        query_embedding: &[f32],
        k: usize,
    ) -> Result<Vec<MetadataKnnHit>, MetadataIndexError> {
        if !self.vec_available || !self.vec_table_exists(collection) {
            return Ok(vec![]);
        }
        let query_bytes: Vec<u8> = query_embedding
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();
        // sqlite-vec requires MATCH + k as constraints on the vec table, so
        // the KNN is computed in a subquery and then INNER JOINed to live
        // `items` for this collection. Ghost/orphan and sibling-collection
        // ids never leave SQL. Fetch at least `vec_count` candidates so
        // closer orphans cannot occupy the k slots and hide live rows.
        // Post-filter remains defense in depth. A deferred snapshot keeps
        // MATCH and the join from tearing if this connection is not
        // already in a transaction.
        let started_tx = self.db.execute_batch("BEGIN;").is_ok();
        let result = (|| -> Result<Vec<MetadataKnnHit>, MetadataIndexError> {
            let fetch_k = (self.vec_count(collection) as usize).max(k.max(1)) as i64;
            let sql = format!(
                "SELECT i.item_id, v.distance FROM ( \
                   SELECT item_id, distance FROM {} \
                   WHERE embedding MATCH ?1 AND k = ?2 \
                 ) v \
                 INNER JOIN items i \
                   ON i.item_id = v.item_id AND i.collection = ?3 \
                 ORDER BY v.distance",
                collection.vec_table()
            );
            let mut stmt = self.db.prepare(&sql)?;
            let raw = stmt
                .query_map(params![query_bytes, fetch_k, collection.as_str()], |row| {
                    Ok(MetadataKnnHit {
                        item_id: row.get(0)?,
                        distance: row.get(1)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            drop(stmt);
            let mut hits = Vec::with_capacity(raw.len().min(k));
            for hit in raw {
                if hits.len() >= k {
                    break;
                }
                if existing_item(&self.db, collection, &hit.item_id)?.is_some() {
                    hits.push(hit);
                }
            }
            Ok(hits)
        })();
        if started_tx {
            match &result {
                Ok(_) => {
                    let _ = self.db.execute_batch("COMMIT;");
                }
                Err(_) => {
                    let _ = self.db.execute_batch("ROLLBACK;");
                }
            }
        }
        result
    }

    /// Items whose live content hash is not staged for `pending_id`.
    pub(crate) fn items_not_staged(
        &self,
        collection: CollectionKind,
        pending_id: &str,
    ) -> Result<Vec<(String, String, String)>, rusqlite::Error> {
        let mut stmt = self.db.prepare(
            "SELECT i.item_id, i.content_hash, i.fts_text FROM items i \
             WHERE i.collection = ?1 AND NOT EXISTS ( \
               SELECT 1 FROM vector_staging s \
               WHERE s.collection = i.collection AND s.pending_id = ?2 \
                 AND s.item_id = i.item_id AND s.content_hash = i.content_hash \
             )",
        )?;
        let rows = stmt
            .query_map(params![collection.as_str(), pending_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub(crate) fn require_writable(&self) -> Result<(), MetadataIndexError> {
        if self.writable {
            Ok(())
        } else {
            Err(MetadataIndexError::ReadOnly)
        }
    }

    /// Scan every TEXT cell. Used by privacy tests; never logs the values.
    pub fn text_cells(&self) -> Result<Vec<String>, MetadataIndexError> {
        let mut out = Vec::new();
        let mut tables = self
            .db
            .prepare("SELECT name FROM sqlite_master WHERE type IN ('table', 'view')")?;
        let names: Vec<String> = tables
            .query_map([], |r| r.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        drop(tables);
        for name in names {
            if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                continue;
            }
            let pragma = format!("PRAGMA table_info({name})");
            let mut info = self.db.prepare(&pragma)?;
            let cols: Vec<(i32, String, String)> = info
                .query_map([], |r| {
                    Ok((
                        r.get::<_, i32>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            drop(info);
            let text_idx: Vec<i32> = cols
                .iter()
                .filter(|(_, _, ty)| ty.eq_ignore_ascii_case("TEXT"))
                .map(|(i, _, _)| *i)
                .collect();
            if text_idx.is_empty() {
                continue;
            }
            let sql = format!("SELECT * FROM {name}");
            let mut stmt = self.db.prepare(&sql)?;
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next()? {
                for i in &text_idx {
                    if let Ok(v) = row.get::<_, String>(*i as usize) {
                        out.push(v);
                    }
                }
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
std::thread_local! {
    static FORCE_FTS_ONLY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn test_force_fts_only() -> bool {
    #[cfg(test)]
    {
        FORCE_FTS_ONLY.with(|c| c.get())
    }
    #[cfg(not(test))]
    {
        false
    }
}

#[cfg(test)]
pub(crate) fn set_force_fts_only(enabled: bool) {
    FORCE_FTS_ONLY.with(|c| c.set(enabled));
}

#[cfg(test)]
pub(crate) static UPSERT_BEFORE_TXN_PAUSE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(test)]
pub(crate) static UPSERT_BEFORE_TXN_REACHED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(test)]
fn wait_upsert_before_txn_pause() {
    use std::sync::atomic::Ordering;
    if !UPSERT_BEFORE_TXN_PAUSE.load(Ordering::SeqCst) {
        return;
    }
    UPSERT_BEFORE_TXN_REACHED.store(true, Ordering::SeqCst);
    while UPSERT_BEFORE_TXN_PAUSE.load(Ordering::SeqCst) {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

fn restrict_dir_owner_only(dir: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }
    let _ = dir;
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

pub(crate) fn table_exists(db: &rusqlite::Connection, name: &str) -> bool {
    db.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
        params![name],
        |_| Ok(()),
    )
    .is_ok()
}

fn collection_dimensions(db: &rusqlite::Connection, collection: CollectionKind) -> Option<usize> {
    db.query_row(
        "SELECT embedding_dimensions FROM collections WHERE name = ?1",
        params![collection.as_str()],
        |r| r.get::<_, i64>(0),
    )
    .ok()
    .map(|d| d as usize)
}

fn existing_item(
    db: &rusqlite::Connection,
    collection: CollectionKind,
    item_id: &str,
) -> Result<Option<(i64, String, String)>, MetadataIndexError> {
    let mut stmt = db.prepare(
        "SELECT rowid, content_hash, fts_text FROM items WHERE collection = ?1 AND item_id = ?2",
    )?;
    match stmt.query_row(params![collection.as_str(), item_id], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    }) {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn load_item_ids(
    db: &rusqlite::Connection,
    collection: CollectionKind,
) -> Result<Vec<String>, MetadataIndexError> {
    let mut stmt = db.prepare("SELECT item_id FROM items WHERE collection = ?1")?;
    let rows = stmt
        .query_map(params![collection.as_str()], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn insert_fts(
    db: &rusqlite::Connection,
    fts: &str,
    rowid: i64,
    text: &str,
) -> Result<(), MetadataIndexError> {
    let sql = format!("INSERT INTO {fts}(rowid, text) VALUES (?1, ?2)");
    db.execute(&sql, params![rowid, text])?;
    Ok(())
}

fn delete_fts(
    db: &rusqlite::Connection,
    fts: &str,
    rowid: i64,
    text: &str,
) -> Result<(), MetadataIndexError> {
    let sql = format!("INSERT INTO {fts}({fts}, rowid, text) VALUES('delete', ?1, ?2)");
    db.execute(&sql, params![rowid, text])?;
    Ok(())
}

fn validate_item_id(id: &str) -> Result<(), MetadataIndexError> {
    if id.is_empty() || id.len() > MAX_ITEM_ID {
        return Err(MetadataIndexError::InvalidItem("item id length"));
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    {
        return Err(MetadataIndexError::InvalidItem("item id charset"));
    }
    Ok(())
}

fn validate_name(name: &str) -> Result<(), MetadataIndexError> {
    if name.is_empty() || name.len() > MAX_NAME {
        return Err(MetadataIndexError::InvalidItem("name length"));
    }
    if name.contains('\0') {
        return Err(MetadataIndexError::Privacy("name must not be a path"));
    }
    reject_persisted_paths(name, "name must not be a path")?;
    Ok(())
}

fn validate_description(description: &str) -> Result<(), MetadataIndexError> {
    if description.len() > MAX_DESCRIPTION {
        return Err(MetadataIndexError::InvalidItem("description length"));
    }
    reject_persisted_paths(description, "description must not be an absolute path")?;
    reject_secret_markers(description)?;
    Ok(())
}

fn validate_extra(extra: &str) -> Result<(), MetadataIndexError> {
    if extra.is_empty() {
        return Ok(());
    }
    if extra.len() > MAX_EXTRA {
        return Err(MetadataIndexError::InvalidItem("extra length"));
    }
    reject_secret_markers(extra)?;
    reject_persisted_paths(extra, "extra must not contain an absolute path")?;
    let v: serde_json::Value = serde_json::from_str(extra)
        .map_err(|_| MetadataIndexError::InvalidItem("extra must be JSON object"))?;
    let obj = v
        .as_object()
        .ok_or(MetadataIndexError::InvalidItem("extra must be JSON object"))?;
    for (key, value) in obj {
        match key.as_str() {
            "when_to_use" => {
                let s = value
                    .as_str()
                    .ok_or(MetadataIndexError::InvalidItem("when_to_use type"))?;
                if s.len() > MAX_WHEN_TO_USE {
                    return Err(MetadataIndexError::InvalidItem("when_to_use length"));
                }
                reject_persisted_paths(s, "when_to_use must not be an absolute path")?;
            }
            "scope" => {
                let s = value
                    .as_str()
                    .ok_or(MetadataIndexError::InvalidItem("scope type"))?;
                if s.len() > MAX_SCOPE {
                    return Err(MetadataIndexError::Privacy("scope must be a label"));
                }
                reject_persisted_paths(s, "scope must be a label")?;
            }
            "paths" => {
                let arr = value
                    .as_array()
                    .ok_or(MetadataIndexError::InvalidItem("paths type"))?;
                if arr.len() > MAX_PATHS {
                    return Err(MetadataIndexError::InvalidItem("paths length"));
                }
                for p in arr {
                    let s = p
                        .as_str()
                        .ok_or(MetadataIndexError::InvalidItem("path label type"))?;
                    if s.is_empty() || s.len() > MAX_PATH_LABEL {
                        return Err(MetadataIndexError::InvalidItem("path label length"));
                    }
                    if s.contains("..") {
                        return Err(MetadataIndexError::Privacy("path labels must be relative"));
                    }
                    reject_persisted_paths(s, "path labels must be relative")?;
                }
            }
            "body" | "prompt" | "credential" | "session" | "vector" | "error" => {
                return Err(MetadataIndexError::Privacy(
                    "extra key is not in the metadata whitelist",
                ));
            }
            _ => {
                return Err(MetadataIndexError::Privacy(
                    "extra key is not in the metadata whitelist",
                ));
            }
        }
    }
    Ok(())
}

fn reject_secret_markers(text: &str) -> Result<(), MetadataIndexError> {
    let lower = text.to_ascii_lowercase();
    for needle in [
        "sk-",
        "api_key",
        "authorization",
        "bearer ",
        "-----begin",
        "session history",
    ] {
        if lower.contains(needle) {
            return Err(MetadataIndexError::Privacy(
                "secret or session marker is not allowed",
            ));
        }
    }
    Ok(())
}

/// Shared persistence-privacy contract for Prime metadata.
///
/// Absolute, UNC, `file:` URL, encoded/traversal, URL userinfo/credentials,
/// and home-rooted path tokens are rejected. Relative labels such as
/// `src/**` and credential-free `https://example.com/docs` are accepted.
pub fn reject_persisted_paths(text: &str, reason: &'static str) -> Result<(), MetadataIndexError> {
    if contains_absolute_path_token(text) {
        Err(MetadataIndexError::Privacy(reason))
    } else {
        Ok(())
    }
}

/// Well-known Unix absolute roots rejected even as a single path component.
const UNIX_ABS_ROOTS: &[&str] = &["etc", "usr", "opt", "var", "tmp", "private", "root"];

fn is_path_token_boundary(prev: Option<char>) -> bool {
    match prev {
        None => true,
        Some(c) => c.is_ascii_whitespace() || c == '"' || c == '\'',
    }
}

fn is_path_component_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-'
}

/// Token scan across name, description, extra, and extra field values.
/// Relative labels such as `src/**` are kept; embedded Unix/Windows/UNC
/// absolute paths, parent traversal, and URL userinfo are not persisted.
/// Percent-decoding is applied to the whole field, not only `file:` tokens.
fn contains_absolute_path_token(s: &str) -> bool {
    if forbidden_path_shape(s) {
        return true;
    }
    let decoded = percent_decode_repeated(s);
    if decoded != s && forbidden_path_shape(&decoded) {
        return true;
    }
    let slash_norm = decoded.replace('\\', "/");
    slash_norm != decoded && forbidden_path_shape(&slash_norm)
}

fn forbidden_path_shape(s: &str) -> bool {
    if contains_url_userinfo(s) || looks_like_absolute_path(s) || contains_parent_escape(s) {
        return true;
    }
    let lower = s.to_ascii_lowercase();
    if lower.contains("/users/")
        || lower.contains("/home/")
        || lower.contains("~/")
        || lower.contains("\\users\\")
        || lower.contains("\\home\\")
        || lower.contains("\\\\")
    {
        return true;
    }
    contains_windows_drive_token(s)
        || contains_unix_or_unc_token(s)
        || contains_absolute_file_url(s)
}

fn contains_parent_escape(s: &str) -> bool {
    s.replace('\\', "/").split('/').any(|seg| seg == "..")
}

/// `scheme://userinfo@host` credentials are never persisted or shipped.
fn contains_url_userinfo(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    let mut rest = lower.as_str();
    while let Some(idx) = rest.find("://") {
        let after = &rest[idx + 3..];
        let auth_end = after
            .find(|c: char| c == '/' || c.is_ascii_whitespace() || c == '"' || c == '\'')
            .unwrap_or(after.len());
        if after[..auth_end].contains('@') {
            return true;
        }
        rest = after;
    }
    false
}

/// Absolute `file:` URLs (`file:/`, `file://`, `file:///`, including
/// `file://localhost/...`) are absolute paths. Percent-encoded, backslash,
/// and `..` traversal forms that resolve to an absolute root are rejected.
/// Relative `file:./` with no parent-segment escape and non-file `http(s)`
/// URLs are not.
fn contains_absolute_file_url(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    let mut rest = lower.as_str();
    while let Some(idx) = rest.find("file:") {
        let after = &rest[idx + 5..];
        let token_end = after
            .find(|c: char| c.is_ascii_whitespace() || c == '"' || c == '\'')
            .unwrap_or(after.len());
        let token = &after[..token_end];
        if file_url_token_is_forbidden(token) {
            return true;
        }
        rest = after;
    }
    false
}

fn file_url_token_is_forbidden(token: &str) -> bool {
    let normalized = percent_decode_repeated(token)
        .to_ascii_lowercase()
        .replace('\\', "/");
    if file_url_has_userinfo(&normalized) {
        return true;
    }
    if normalized.starts_with('/') {
        return true;
    }
    if file_url_has_loopback_authority(&normalized) {
        return true;
    }
    file_url_parent_escapes_to_root(&normalized)
}

fn percent_decode_repeated(input: &str) -> String {
    let mut current = input.to_string();
    for _ in 0..4 {
        let next = percent_decode_once(&current);
        if next == current {
            return current;
        }
        current = next;
    }
    current
}

fn percent_decode_once(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Some(hi) = hex_nibble(bytes[i + 1])
            && let Some(lo) = hex_nibble(bytes[i + 2])
        {
            out.push((hi << 4) | lo);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn file_url_has_userinfo(normalized: &str) -> bool {
    let Some(rest) = normalized.strip_prefix("//") else {
        return false;
    };
    rest.split('/').next().unwrap_or("").contains('@')
}

fn file_url_has_loopback_authority(normalized: &str) -> bool {
    let rest = normalized.strip_prefix("//").unwrap_or(normalized);
    let authority = rest.split('/').next().unwrap_or("");
    let hostport = authority.rsplit('@').next().unwrap_or(authority);
    let host = hostport
        .split(':')
        .next()
        .unwrap_or("")
        .trim_matches(['[', ']']);
    matches!(
        host,
        "localhost" | "127.0.0.1" | "::1" | "0.0.0.0" | "[::1]"
    )
}

fn file_url_parent_escapes_to_root(path: &str) -> bool {
    let mut depth: i32 = 0;
    for segment in path.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." {
            if depth == 0 {
                return true;
            }
            depth -= 1;
        } else {
            depth += 1;
        }
    }
    false
}

fn contains_windows_drive_token(s: &str) -> bool {
    let bytes = s.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i + 2 < n {
        if bytes[i].is_ascii_alphabetic()
            && bytes[i + 1] == b':'
            && (bytes[i + 2] == b'\\' || bytes[i + 2] == b'/')
            && (i == 0 || !bytes[i - 1].is_ascii_alphanumeric())
        {
            return true;
        }
        i += 1;
    }
    false
}

fn contains_unix_or_unc_token(s: &str) -> bool {
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    let mut i = 0;
    while i < n {
        if chars[i] != '/' {
            i += 1;
            continue;
        }
        let prev = if i == 0 { None } else { Some(chars[i - 1]) };
        if !is_path_token_boundary(prev) {
            i += 1;
            continue;
        }
        if i + 1 < n && chars[i + 1] == '/' {
            if unix_unc_share(&chars, i) {
                return true;
            }
            i += 1;
            continue;
        }
        if unix_absolute_after_slash(&chars, i) {
            return true;
        }
        i += 1;
    }
    false
}

fn unix_unc_share(chars: &[char], slash: usize) -> bool {
    let n = chars.len();
    let mut j = slash + 2;
    let mut server_len = 0usize;
    while j < n && is_path_component_char(chars[j]) {
        server_len += 1;
        j += 1;
    }
    if server_len == 0 || j >= n || chars[j] != '/' {
        return false;
    }
    j += 1;
    let mut share_len = 0usize;
    while j < n && is_path_component_char(chars[j]) {
        share_len += 1;
        j += 1;
    }
    share_len > 0
}

fn unix_absolute_after_slash(chars: &[char], slash: usize) -> bool {
    let n = chars.len();
    let mut j = slash + 1;
    let start = j;
    while j < n && is_path_component_char(chars[j]) {
        j += 1;
    }
    if j == start {
        return false;
    }
    let first: String = chars[start..j]
        .iter()
        .collect::<String>()
        .to_ascii_lowercase();
    let has_second = j < n && chars[j] == '/' && j + 1 < n && is_path_component_char(chars[j + 1]);
    if has_second {
        return true;
    }
    UNIX_ABS_ROOTS.iter().any(|root| *root == first)
}

fn looks_like_absolute_path(s: &str) -> bool {
    let t = s.trim();
    if t.is_empty() {
        return false;
    }
    t.starts_with('/')
        || t.starts_with("~/")
        || t.starts_with('\\')
        || (t.len() >= 3
            && t.as_bytes()[0].is_ascii_alphabetic()
            && t.as_bytes()[1] == b':'
            && (t.as_bytes()[2] == b'\\' || t.as_bytes()[2] == b'/'))
}

fn item_content_hash(name: &str, description: &str, extra: &str) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"prime-meta/item/v1\0");
    frame_str(&mut hasher, name);
    frame_str(&mut hasher, description);
    frame_str(&mut hasher, extra);
    hex_encode(&hasher.finalize().as_bytes()[..16])
}

fn frame_str(hasher: &mut blake3::Hasher, value: &str) {
    hasher.update(&(value.len() as u64).to_le_bytes());
    hasher.update(value.as_bytes());
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}
