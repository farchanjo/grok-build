//! SQLite-backed FTS5 index for session search.
//!
//! Modelled after the memory system's `MemoryIndex` / `schema.rs`, but
//! purpose-built for searching across *sessions* (titles + user prompts).
//!
//! ## Schema
//!
//! - `meta`              — key-value metadata (schema version)
//! - `session_docs`      — one row per session (title, content, content_hash)
//! - `session_docs_fts`  — content-synced FTS5 over title + content (not cwd)
//!
//! FTS is kept in sync with `session_docs` via `AFTER INSERT/UPDATE/DELETE`
//! triggers so callers never need to touch the FTS table directly.
//! The `cwd` column is intentionally excluded from the FTS table — it is a
//! filter dimension only, applied via JOIN on `session_docs`.

use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, params};
use xai_sqlite_journal::JournalMode;

pub(crate) const META_KEY_BOOTSTRAP_CLAIM: &str = "bootstrap_claimed_at";
pub(crate) const META_KEY_LAST_BOOTSTRAP: &str = "last_bootstrap_at";
const META_KEY_MUTATION_GENERATION: &str = "mutation_generation";
const META_KEY_FRESHNESS_TRIGGERS_ENABLED: &str = "freshness_triggers_enabled";
const CLAIM_TOKEN_SQL: &str = "substr(value, instr(value, ':') + 1)";

fn claim_stamp(now_unix: i64, token: &str) -> String {
    format!("{now_unix}:{token}")
}

/// Bump when making breaking schema changes that require dropping and
/// recreating tables, or to force a rebuild of stale index content
/// (v3 → v4: messages with JSON escapes were silently dropped at indexing).
const SCHEMA_VERSION: &str = "4";

/// A document to be indexed for session search.
#[derive(Debug, Clone)]
pub struct SessionDoc {
    pub session_id: String,
    pub cwd: String,
    pub updated_at_unix: i64,
    pub title: String,
    /// Concatenated user prompts (the searchable body).
    pub content: String,
    /// blake3 hash of `content` — used to skip redundant upserts.
    pub content_hash: String,
    /// Byte offset in `updates.jsonl` up to which content has been indexed.
    /// Used for delta indexing: on subsequent updates, only bytes after this
    /// offset are parsed and merged with existing content.
    pub last_indexed_offset: u64,
}

/// State of a previously indexed session, returned by
/// [`SessionSearchIndex::get_session_index_state`].
#[derive(Debug, Clone)]
pub struct SessionIndexState {
    pub content: String,
    pub content_hash: String,
    pub last_indexed_offset: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClaimedUpsert {
    Lost,
    Stale,
    Unchanged,
    Indexed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BootstrapClaim {
    pub mutation_generation: i64,
}

/// A single search result row.
#[derive(Debug, Clone)]
pub struct SessionSearchRow {
    pub session_id: String,
    pub cwd: String,
    pub title: String,
    pub updated_at_unix: i64,
    pub score: f32,
    pub matched_fields: Vec<String>,
    pub snippet: Option<String>,
}

/// Result of a `SessionSearchIndex::query()` call.
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub results: Vec<SessionSearchRow>,
    pub next_offset: Option<usize>,
    pub total_estimate: Option<usize>,
}

/// Wraps a `rusqlite::Connection` pointing at `session_search.sqlite`.
pub struct SessionSearchIndex {
    db: Connection,
}

impl SessionSearchIndex {
    /// Open (or create) the FTS index at `db_path`.
    ///
    /// Creates the schema and triggers on first use. When the stored schema
    /// version is OLDER than [`SCHEMA_VERSION`], drops and recreates all
    /// tables (simple migration strategy for an index that can be rebuilt)
    /// and deletes the `last_bootstrap_at` completed-bootstrap marker so the
    /// wipe is observable to bootstrap/staleness checks.
    /// A NEWER stored version is tolerated read/write without dropping.
    pub fn open_or_create(db_path: &std::path::Path) -> Result<Self, rusqlite::Error> {
        if let Some(parent) = db_path.parent() {
            let _ = crate::util::grok_home::create_dir_all_owner_only(parent);
        }

        // The mode decision statfs's the parent dir created above.
        Self::open_with_journal_mode(db_path, JournalMode::for_db_path(db_path))
    }

    /// Open an existing index without creating or migrating it. Used only for
    /// best-effort deletion while the feature gate is off.
    pub(crate) fn open_existing(db_path: &std::path::Path) -> Result<Self, rusqlite::Error> {
        let mode = JournalMode::for_db_path(db_path);
        let db = Connection::open_with_flags(
            mode.effective_db_path(db_path),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        db.busy_timeout(Duration::from_secs(5))?;
        mode.apply(&db)?;
        Ok(Self { db })
    }

    /// Open with an explicit journal mode — the seam tests use to exercise
    /// the network-filesystem decision on a local disk.
    fn open_with_journal_mode(
        db_path: &std::path::Path,
        journal_mode: JournalMode,
    ) -> Result<Self, rusqlite::Error> {
        // busy_timeout + journal pragma live in the helper (see JournalMode::open).
        let db = journal_mode.open(db_path)?;

        // Serialize version detection, destructive upgrade, schema rebuild,
        // and version stamping. A concurrent opener waits, then rechecks the
        // version after the first opener has committed the complete schema.
        let current: u64 = SCHEMA_VERSION
            .parse()
            .expect("SCHEMA_VERSION is an integer");
        let schema =
            rusqlite::Transaction::new_unchecked(&db, rusqlite::TransactionBehavior::Immediate)?;
        let stored_version: Option<String> = schema
            .query_row(
                "SELECT value FROM meta WHERE key = 'session_search_schema_version'",
                [],
                |row| row.get(0),
            )
            .optional()
            .unwrap_or(None);

        // One-way ratchet: drop only on UPGRADE (stored < current). Multiple
        // grok generations share this DB (stable vs alpha); an equality check
        // made each binary wipe the other's index in a ping-pong that left
        // search empty mid-rebootstrap. A newer index is safe to read.
        let stored: Option<u64> = stored_version.as_deref().map(|v| v.parse().unwrap_or(0));
        let owned_by_newer = stored.is_some_and(|s| s > current);
        if stored.is_some_and(|s| s < current) {
            // Atomically invalidate both bootstrap coordination rows with the
            // destructive drop. All unrelated metadata survives.
            schema.execute_batch(
                "
                DROP TRIGGER IF EXISTS session_docs_ai;
                DROP TRIGGER IF EXISTS session_docs_ad;
                DROP TRIGGER IF EXISTS session_docs_au;
                DROP TRIGGER IF EXISTS session_docs_freshness_ai;
                DROP TRIGGER IF EXISTS session_docs_freshness_ad;
                DROP TRIGGER IF EXISTS session_docs_freshness_au;
                DROP TABLE IF EXISTS session_docs_fts;
                DROP TABLE IF EXISTS session_docs;
                DROP TABLE IF EXISTS session_freshness;
                DELETE FROM meta WHERE key IN (
                    'last_bootstrap_at',
                    'bootstrap_claimed_at',
                    'mutation_generation',
                    'freshness_triggers_enabled'
                );
                ",
            )?;
        } else if owned_by_newer {
            tracing::debug!(
                stored = stored.unwrap_or_default(),
                current,
                "session search index owned by a newer schema version; keeping tables"
            );
        }

        // Create tables + content-synced FTS5 with auto-sync triggers while
        // still holding the serialized schema transaction.
        schema.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            INSERT OR IGNORE INTO meta(key, value)
            VALUES ('mutation_generation', '0');
            INSERT OR IGNORE INTO meta(key, value)
            VALUES ('freshness_triggers_enabled', '1');

            CREATE TABLE IF NOT EXISTS session_docs (
                session_id TEXT PRIMARY KEY,
                cwd TEXT NOT NULL,
                updated_at INTEGER NOT NULL,
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                content_hash TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS session_freshness (
                session_id TEXT PRIMARY KEY,
                generation INTEGER NOT NULL
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS session_docs_fts USING fts5(
                title,
                content,
                content='session_docs',
                content_rowid='rowid'
            );

            CREATE TRIGGER IF NOT EXISTS session_docs_ai AFTER INSERT ON session_docs BEGIN
                INSERT INTO session_docs_fts(rowid, title, content)
                VALUES (new.rowid, new.title, new.content);
            END;

            CREATE TRIGGER IF NOT EXISTS session_docs_ad AFTER DELETE ON session_docs BEGIN
                INSERT INTO session_docs_fts(session_docs_fts, rowid, title, content)
                VALUES ('delete', old.rowid, old.title, old.content);
            END;

            CREATE TRIGGER IF NOT EXISTS session_docs_au AFTER UPDATE ON session_docs BEGIN
                INSERT INTO session_docs_fts(session_docs_fts, rowid, title, content)
                VALUES ('delete', old.rowid, old.title, old.content);
                INSERT INTO session_docs_fts(rowid, title, content)
                VALUES (new.rowid, new.title, new.content);
            END;

            CREATE TRIGGER IF NOT EXISTS session_docs_freshness_ai
            AFTER INSERT ON session_docs
            WHEN (SELECT value FROM meta WHERE key = 'freshness_triggers_enabled') = '1'
            BEGIN
                INSERT INTO session_freshness(session_id, generation)
                VALUES (
                    new.session_id,
                    CAST((SELECT value FROM meta WHERE key = 'mutation_generation') AS INTEGER) + 1
                )
                ON CONFLICT(session_id) DO UPDATE SET generation = excluded.generation;
                UPDATE meta
                SET value = CAST(value AS INTEGER) + 1
                WHERE key = 'mutation_generation';
            END;

            CREATE TRIGGER IF NOT EXISTS session_docs_freshness_ad
            AFTER DELETE ON session_docs
            WHEN (SELECT value FROM meta WHERE key = 'freshness_triggers_enabled') = '1'
            BEGIN
                INSERT INTO session_freshness(session_id, generation)
                VALUES (
                    old.session_id,
                    CAST((SELECT value FROM meta WHERE key = 'mutation_generation') AS INTEGER) + 1
                )
                ON CONFLICT(session_id) DO UPDATE SET generation = excluded.generation;
                UPDATE meta
                SET value = CAST(value AS INTEGER) + 1
                WHERE key = 'mutation_generation';
            END;

            CREATE TRIGGER IF NOT EXISTS session_docs_freshness_au
            AFTER UPDATE ON session_docs
            WHEN (SELECT value FROM meta WHERE key = 'freshness_triggers_enabled') = '1'
            BEGIN
                INSERT INTO session_freshness(session_id, generation)
                VALUES (
                    new.session_id,
                    CAST((SELECT value FROM meta WHERE key = 'mutation_generation') AS INTEGER) + 1
                )
                ON CONFLICT(session_id) DO UPDATE SET generation = excluded.generation;
                UPDATE meta
                SET value = CAST(value AS INTEGER) + 1
                WHERE key = 'mutation_generation';
            END;
            ",
        )?;

        match schema.execute(
            "ALTER TABLE session_docs ADD COLUMN last_indexed_offset INTEGER NOT NULL DEFAULT 0",
            [],
        ) {
            Ok(_) => {}
            Err(e) => {
                let msg = e.to_string();
                if !msg.contains("duplicate column") {
                    return Err(e);
                }
            }
        }

        // Never regress a version row owned by a newer generation.
        if stored != Some(current) && !owned_by_newer {
            schema.execute(
                "INSERT OR REPLACE INTO meta(key, value) \
                 VALUES ('session_search_schema_version', ?1)",
                params![SCHEMA_VERSION],
            )?;
        }
        // A process that died during a fenced bootstrap transaction rolls the
        // transaction back. Resetting here also heals a legacy/crash residue
        // before any ordinary writer uses the triggers.
        schema.execute(
            "UPDATE meta SET value = '1' WHERE key = ?1",
            params![META_KEY_FRESHNESS_TRIGGERS_ENABLED],
        )?;
        schema.commit()?;

        Ok(Self { db })
    }

    fn write_doc(
        tx: &rusqlite::Transaction<'_>,
        doc: &SessionDoc,
    ) -> Result<bool, rusqlite::Error> {
        let changed = tx.execute(
            "INSERT INTO session_docs(session_id, cwd, updated_at, title, content, content_hash, last_indexed_offset)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(session_id) DO UPDATE SET
                 cwd = excluded.cwd,
                 updated_at = excluded.updated_at,
                 title = excluded.title,
                 content = excluded.content,
                 content_hash = excluded.content_hash,
                 last_indexed_offset = excluded.last_indexed_offset
             WHERE session_docs.cwd != excluded.cwd
                OR session_docs.updated_at != excluded.updated_at
                OR session_docs.title != excluded.title
                OR session_docs.content != excluded.content
                OR session_docs.content_hash != excluded.content_hash
                OR session_docs.last_indexed_offset != excluded.last_indexed_offset",
            params![
                doc.session_id,
                doc.cwd,
                doc.updated_at_unix,
                doc.title,
                doc.content,
                doc.content_hash,
                doc.last_indexed_offset as i64
            ],
        )?;
        Ok(changed == 1)
    }

    /// Insert or update a session document as an incremental mutation.
    ///
    /// Every notification receives a database-global generation, even when
    /// the indexed fields are unchanged. The schema triggers stamp changed
    /// rows; this method stamps unchanged observations explicitly. Recording
    /// that generation alongside the session lets an older bootstrap snapshot
    /// decline to overwrite or prune the row. The generation bump, document
    /// write, and freshness row commit in one cross-process SQLite transaction.
    pub(crate) fn upsert_doc_incremental(&self, doc: &SessionDoc) -> Result<bool, rusqlite::Error> {
        let tx = rusqlite::Transaction::new_unchecked(
            &self.db,
            rusqlite::TransactionBehavior::Immediate,
        )?;
        Self::set_freshness_triggers_enabled(&tx, true)?;
        let changed = Self::write_doc(&tx, doc)?;
        if !changed {
            // An unchanged incremental observation still fences an older
            // bootstrap snapshot. Advance freshness explicitly because the
            // session_docs update trigger did not fire.
            tx.execute(
                "UPDATE meta
                 SET value = CAST(value AS INTEGER) + 1
                 WHERE key = ?1",
                params![META_KEY_MUTATION_GENERATION],
            )?;
            tx.execute(
                "INSERT INTO session_freshness(session_id, generation)
                 SELECT ?1, CAST(value AS INTEGER) FROM meta WHERE key = ?2
                 ON CONFLICT(session_id) DO UPDATE SET generation = excluded.generation",
                params![doc.session_id, META_KEY_MUTATION_GENERATION],
            )?;
        }
        tx.commit()?;
        Ok(changed)
    }

    /// Insert or update a session document in the index.
    ///
    /// The content-synced FTS triggers handle updating `session_docs_fts`
    /// automatically. This public helper uses incremental freshness semantics;
    /// bootstrap writes use the separately fenced claim-owner API.
    pub fn upsert_doc(&self, doc: &SessionDoc) -> Result<(), rusqlite::Error> {
        self.upsert_doc_incremental(doc).map(|_| ())
    }

    /// Insert a session document only if no row exists for its `session_id`.
    ///
    /// Atomic alternative to a check-then-insert: the index DB is shared
    /// across processes, so a two-step gate could clobber a full-content row
    /// written between the check and the insert.
    pub fn insert_doc_if_absent(&self, doc: &SessionDoc) -> Result<(), rusqlite::Error> {
        let tx = rusqlite::Transaction::new_unchecked(
            &self.db,
            rusqlite::TransactionBehavior::Immediate,
        )?;
        Self::set_freshness_triggers_enabled(&tx, true)?;
        tx.execute(
            "INSERT INTO session_docs(session_id, cwd, updated_at, title, content, content_hash, last_indexed_offset)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(session_id) DO NOTHING",
            params![
                doc.session_id,
                doc.cwd,
                doc.updated_at_unix,
                doc.title,
                doc.content,
                doc.content_hash,
                doc.last_indexed_offset as i64
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Remove a session document from the index and retain a generation
    /// tombstone so an older bootstrap cannot resurrect it.
    pub fn delete_doc(&self, session_id: &str) -> Result<(), rusqlite::Error> {
        let tx = rusqlite::Transaction::new_unchecked(
            &self.db,
            rusqlite::TransactionBehavior::Immediate,
        )?;
        Self::set_freshness_triggers_enabled(&tx, true)?;
        let deleted = tx.execute(
            "DELETE FROM session_docs WHERE session_id = ?1",
            params![session_id],
        )?;
        if deleted == 0 {
            // Preserve a tombstone for an absent/deleted session. No DELETE
            // trigger fired, so advance the generation explicitly.
            let bumped = tx.execute(
                "UPDATE meta
                 SET value = CAST(value AS INTEGER) + 1
                 WHERE key = ?1",
                params![META_KEY_MUTATION_GENERATION],
            )?;
            if bumped == 1 {
                tx.execute(
                    "INSERT INTO session_freshness(session_id, generation)
                     SELECT ?1, CAST(value AS INTEGER) FROM meta WHERE key = ?2
                     ON CONFLICT(session_id) DO UPDATE SET generation = excluded.generation",
                    params![session_id, META_KEY_MUTATION_GENERATION],
                )?;
            }
            // `open_existing` intentionally does not migrate a legacy index.
            // When the generation row is missing, deletion above still
            // preserves best-effort gate-off eviction.
        }
        tx.commit()?;
        Ok(())
    }

    /// Return the stored content_hash for a session, if any.
    ///
    /// Used to skip redundant upserts when content hasn't changed.
    pub fn get_content_hash(&self, session_id: &str) -> Result<Option<String>, rusqlite::Error> {
        self.db
            .query_row(
                "SELECT content_hash FROM session_docs WHERE session_id = ?1",
                params![session_id],
                |row| row.get(0),
            )
            .optional()
    }

    /// Return the full index state for a session: content, content_hash,
    /// and last_indexed_offset. Used by the delta indexing path.
    pub fn get_session_index_state(
        &self,
        session_id: &str,
    ) -> Result<Option<SessionIndexState>, rusqlite::Error> {
        self.db
            .query_row(
                "SELECT content, content_hash, last_indexed_offset FROM session_docs WHERE session_id = ?1",
                params![session_id],
                |row| {
                    let content: String = row.get(0)?;
                    let content_hash: String = row.get(1)?;
                    let offset: i64 = row.get(2)?;
                    Ok(SessionIndexState {
                        content,
                        content_hash,
                        last_indexed_offset: offset as u64,
                    })
                },
            )
            .optional()
    }

    /// Update only the `last_indexed_offset` for a session without touching
    /// content or hash (avoids firing FTS triggers when content is unchanged).
    pub fn update_indexed_offset(
        &self,
        session_id: &str,
        offset: u64,
    ) -> Result<(), rusqlite::Error> {
        let tx = rusqlite::Transaction::new_unchecked(
            &self.db,
            rusqlite::TransactionBehavior::Immediate,
        )?;
        Self::set_freshness_triggers_enabled(&tx, true)?;
        let changed = tx.execute(
            "UPDATE session_docs SET last_indexed_offset = ?2 WHERE session_id = ?1",
            params![session_id, offset as i64],
        )?;
        if changed == 0 {
            tx.execute(
                "UPDATE meta
                 SET value = CAST(value AS INTEGER) + 1
                 WHERE key = ?1",
                params![META_KEY_MUTATION_GENERATION],
            )?;
            tx.execute(
                "INSERT INTO session_freshness(session_id, generation)
                 SELECT ?1, CAST(value AS INTEGER) FROM meta WHERE key = ?2
                 ON CONFLICT(session_id) DO UPDATE SET generation = excluded.generation",
                params![session_id, META_KEY_MUTATION_GENERATION],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Read a value from the `meta` key-value table.
    pub fn get_meta(&self, key: &str) -> Result<Option<String>, rusqlite::Error> {
        self.db
            .query_row(
                "SELECT value FROM meta WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
    }

    /// Write a value to the `meta` key-value table (insert or replace).
    pub fn set_meta(&self, key: &str, value: &str) -> Result<(), rusqlite::Error> {
        self.db.execute(
            "INSERT OR REPLACE INTO meta(key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    /// Atomically claim an absent, expired, malformed, or implausibly
    /// future-dated bootstrap lease and snapshot the current incremental
    /// mutation generation in the same SQLite transaction.
    pub(crate) fn try_claim_bootstrap(
        &self,
        now_unix: i64,
        lease: Duration,
        token: &str,
    ) -> Result<Option<BootstrapClaim>, rusqlite::Error> {
        let tx = rusqlite::Transaction::new_unchecked(
            &self.db,
            rusqlite::TransactionBehavior::Immediate,
        )?;
        let lease_secs = lease.as_secs() as i64;
        let changed = tx.execute(
            "INSERT INTO meta(key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value
             WHERE CAST(meta.value AS INTEGER) <= ?3
                OR CAST(meta.value AS INTEGER) > ?4",
            params![
                META_KEY_BOOTSTRAP_CLAIM,
                claim_stamp(now_unix, token),
                now_unix.saturating_sub(lease_secs),
                now_unix.saturating_add(lease_secs),
            ],
        )?;
        let claim = if changed == 1 {
            Some(BootstrapClaim {
                mutation_generation: tx.query_row(
                    "SELECT CAST(value AS INTEGER) FROM meta WHERE key = ?1",
                    params![META_KEY_MUTATION_GENERATION],
                    |row| row.get(0),
                )?,
            })
        } else {
            None
        };
        tx.commit()?;
        Ok(claim)
    }

    pub(crate) fn refresh_bootstrap_claim(
        &self,
        now_unix: i64,
        lease: Duration,
        token: &str,
    ) -> Result<bool, rusqlite::Error> {
        let changed = self.db.execute(
            &format!(
                "UPDATE meta SET value = ?2
                 WHERE key = ?1
                   AND {CLAIM_TOKEN_SQL} = ?3
                   AND CAST(value AS INTEGER) > ?4
                   AND CAST(value AS INTEGER) <= ?5"
            ),
            params![
                META_KEY_BOOTSTRAP_CLAIM,
                claim_stamp(now_unix, token),
                token,
                now_unix.saturating_sub(lease.as_secs() as i64),
                now_unix.saturating_add(lease.as_secs() as i64),
            ],
        )?;
        Ok(changed == 1)
    }

    pub(crate) fn set_meta_if_claim_owner(
        &self,
        key: &str,
        value: &str,
        now_unix: i64,
        lease: Duration,
        token: &str,
    ) -> Result<bool, rusqlite::Error> {
        let changed = self.db.execute(
            &format!(
                "INSERT INTO meta(key, value)
                 SELECT ?1, ?2
                 WHERE EXISTS (
                     SELECT 1 FROM meta
                     WHERE key = ?3
                       AND {CLAIM_TOKEN_SQL} = ?4
                       AND CAST(value AS INTEGER) > ?5
                       AND CAST(value AS INTEGER) <= ?6
                 )
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value"
            ),
            params![
                key,
                value,
                META_KEY_BOOTSTRAP_CLAIM,
                token,
                now_unix.saturating_sub(lease.as_secs() as i64),
                now_unix.saturating_add(lease.as_secs() as i64),
            ],
        )?;
        Ok(changed == 1)
    }

    pub(crate) fn release_bootstrap_claim(&self, token: &str) -> Result<bool, rusqlite::Error> {
        let changed = self.db.execute(
            &format!("DELETE FROM meta WHERE key = ?1 AND {CLAIM_TOKEN_SQL} = ?2"),
            params![META_KEY_BOOTSTRAP_CLAIM, token],
        )?;
        Ok(changed == 1)
    }

    fn set_freshness_triggers_enabled(
        tx: &rusqlite::Transaction<'_>,
        enabled: bool,
    ) -> Result<(), rusqlite::Error> {
        tx.execute(
            "UPDATE meta SET value = ?2 WHERE key = ?1",
            params![
                META_KEY_FRESHNESS_TRIGGERS_ENABLED,
                if enabled { "1" } else { "0" }
            ],
        )?;
        Ok(())
    }

    /// Insert or update a bootstrap document only while the current claim is
    /// still owned and unexpired and no newer incremental mutation exists for
    /// this session. The lease and generation checks are part of the same SQL
    /// transaction as the mutation, so neither a takeover nor an incremental
    /// writer can race between the fence and the write.
    pub(crate) fn upsert_doc_if_claim_owner(
        &self,
        doc: &SessionDoc,
        now_unix: i64,
        lease: Duration,
        token: &str,
        bootstrap_generation: i64,
    ) -> Result<ClaimedUpsert, rusqlite::Error> {
        let tx = rusqlite::Transaction::new_unchecked(
            &self.db,
            rusqlite::TransactionBehavior::Immediate,
        )?;
        Self::set_freshness_triggers_enabled(&tx, false)?;
        let changed = tx.execute(
            &format!(
                "INSERT INTO session_docs(
                     session_id, cwd, updated_at, title, content, content_hash, last_indexed_offset
                 )
                 SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7
                 WHERE EXISTS (
                     SELECT 1 FROM meta
                     WHERE key = ?8
                       AND {CLAIM_TOKEN_SQL} = ?9
                       AND CAST(value AS INTEGER) > ?10
                       AND CAST(value AS INTEGER) <= ?11
                 )
                   AND NOT EXISTS (
                     SELECT 1 FROM session_freshness
                     WHERE session_id = ?1 AND generation > ?12
                 )
                   AND (
                     NOT EXISTS (
                         SELECT 1 FROM session_docs WHERE session_id = ?1
                     )
                     OR EXISTS (
                         SELECT 1 FROM session_docs
                         WHERE session_id = ?1
                           AND (
                               cwd != ?2
                               OR updated_at != ?3
                               OR title != ?4
                               OR content != ?5
                               OR content_hash != ?6
                               OR last_indexed_offset != ?7
                           )
                     )
                   )
                 ON CONFLICT(session_id) DO UPDATE SET
                     cwd = excluded.cwd,
                     updated_at = excluded.updated_at,
                     title = excluded.title,
                     content = excluded.content,
                     content_hash = excluded.content_hash,
                     last_indexed_offset = excluded.last_indexed_offset
                 WHERE session_docs.cwd != excluded.cwd
                    OR session_docs.updated_at != excluded.updated_at
                    OR session_docs.title != excluded.title
                    OR session_docs.content != excluded.content
                    OR session_docs.content_hash != excluded.content_hash
                    OR session_docs.last_indexed_offset != excluded.last_indexed_offset"
            ),
            params![
                doc.session_id,
                doc.cwd,
                doc.updated_at_unix,
                doc.title,
                doc.content,
                doc.content_hash,
                doc.last_indexed_offset as i64,
                META_KEY_BOOTSTRAP_CLAIM,
                token,
                now_unix.saturating_sub(lease.as_secs() as i64),
                now_unix.saturating_add(lease.as_secs() as i64),
                bootstrap_generation,
            ],
        )?;
        let incrementally_newer = tx
            .query_row(
                "SELECT 1 FROM session_freshness
                 WHERE session_id = ?1 AND generation > ?2",
                params![doc.session_id, bootstrap_generation],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        let still_owner = tx
            .query_row(
                &format!(
                    "SELECT 1 FROM meta
                     WHERE key = ?1
                       AND {CLAIM_TOKEN_SQL} = ?2
                       AND CAST(value AS INTEGER) > ?3
                       AND CAST(value AS INTEGER) <= ?4"
                ),
                params![
                    META_KEY_BOOTSTRAP_CLAIM,
                    token,
                    now_unix.saturating_sub(lease.as_secs() as i64),
                    now_unix.saturating_add(lease.as_secs() as i64),
                ],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        Self::set_freshness_triggers_enabled(&tx, true)?;
        tx.commit()?;
        Ok(if changed == 1 {
            ClaimedUpsert::Indexed
        } else if !still_owner {
            ClaimedUpsert::Lost
        } else if incrementally_newer {
            ClaimedUpsert::Stale
        } else {
            ClaimedUpsert::Unchanged
        })
    }

    pub(crate) fn insert_doc_if_absent_if_claim_owner(
        &self,
        doc: &SessionDoc,
        now_unix: i64,
        lease: Duration,
        token: &str,
        bootstrap_generation: i64,
    ) -> Result<bool, rusqlite::Error> {
        let tx = rusqlite::Transaction::new_unchecked(
            &self.db,
            rusqlite::TransactionBehavior::Immediate,
        )?;
        let owned = tx
            .query_row(
                &format!(
                    "SELECT 1 FROM meta
                     WHERE key = ?1
                       AND {CLAIM_TOKEN_SQL} = ?2
                       AND CAST(value AS INTEGER) > ?3
                       AND CAST(value AS INTEGER) <= ?4"
                ),
                params![
                    META_KEY_BOOTSTRAP_CLAIM,
                    token,
                    now_unix.saturating_sub(lease.as_secs() as i64),
                    now_unix.saturating_add(lease.as_secs() as i64),
                ],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if owned {
            Self::set_freshness_triggers_enabled(&tx, false)?;
            tx.execute(
                "INSERT INTO session_docs(
                     session_id, cwd, updated_at, title, content, content_hash, last_indexed_offset
                 )
                 SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7
                 WHERE NOT EXISTS (
                     SELECT 1 FROM session_freshness
                     WHERE session_id = ?1 AND generation > ?8
                 )
                 ON CONFLICT(session_id) DO NOTHING",
                params![
                    doc.session_id,
                    doc.cwd,
                    doc.updated_at_unix,
                    doc.title,
                    doc.content,
                    doc.content_hash,
                    doc.last_indexed_offset as i64,
                    bootstrap_generation,
                ],
            )?;
            Self::set_freshness_triggers_enabled(&tx, true)?;
        }
        tx.commit()?;
        Ok(owned)
    }

    fn is_claim_owner(
        &self,
        now_unix: i64,
        lease: Duration,
        token: &str,
    ) -> Result<bool, rusqlite::Error> {
        self.db
            .query_row(
                &format!(
                    "SELECT 1 FROM meta
                     WHERE key = ?1
                       AND {CLAIM_TOKEN_SQL} = ?2
                       AND CAST(value AS INTEGER) > ?3
                       AND CAST(value AS INTEGER) <= ?4"
                ),
                params![
                    META_KEY_BOOTSTRAP_CLAIM,
                    token,
                    now_unix.saturating_sub(lease.as_secs() as i64),
                    now_unix.saturating_add(lease.as_secs() as i64),
                ],
                |_| Ok(()),
            )
            .optional()
            .map(|owner| owner.is_some())
    }

    /// Prune under one immediate transaction. A stale claimant cannot delete
    /// rows based on an obsolete disk snapshot, and rows created, relocated,
    /// updated, or deleted incrementally after the claim snapshot are retained.
    pub(crate) fn prune_missing_if_claim_owner(
        &self,
        now_unix: i64,
        lease: Duration,
        token: &str,
        bootstrap_generation: i64,
        keep: &std::collections::HashSet<String>,
    ) -> Result<bool, rusqlite::Error> {
        let tx = rusqlite::Transaction::new_unchecked(
            &self.db,
            rusqlite::TransactionBehavior::Immediate,
        )?;
        let owned = tx
            .query_row(
                &format!(
                    "SELECT 1 FROM meta
                     WHERE key = ?1
                       AND {CLAIM_TOKEN_SQL} = ?2
                       AND CAST(value AS INTEGER) > ?3
                       AND CAST(value AS INTEGER) <= ?4"
                ),
                params![
                    META_KEY_BOOTSTRAP_CLAIM,
                    token,
                    now_unix.saturating_sub(lease.as_secs() as i64),
                    now_unix.saturating_add(lease.as_secs() as i64),
                ],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if !owned {
            return Ok(false);
        }
        Self::set_freshness_triggers_enabled(&tx, false)?;
        let ids = {
            let mut stmt = tx.prepare("SELECT session_id FROM session_docs")?;
            stmt.query_map([], |row| row.get(0))?
                .collect::<Result<Vec<String>, _>>()?
        };
        for id in ids {
            if !keep.contains(&id) {
                tx.execute(
                    "DELETE FROM session_docs
                     WHERE session_id = ?1
                       AND NOT EXISTS (
                           SELECT 1 FROM session_freshness
                           WHERE session_id = ?1 AND generation > ?2
                       )",
                    params![id, bootstrap_generation],
                )?;
            }
        }
        tx.execute(
            "DELETE FROM session_freshness
             WHERE generation <= ?1
               AND NOT EXISTS (
                   SELECT 1 FROM session_docs
                   WHERE session_docs.session_id = session_freshness.session_id
               )",
            params![bootstrap_generation],
        )?;
        Self::set_freshness_triggers_enabled(&tx, true)?;
        tx.commit()?;
        Ok(true)
    }

    /// Return all session IDs currently in the index.
    ///
    /// Used during reindex to detect and prune orphaned entries.
    pub fn all_indexed_session_ids(&self) -> Result<Vec<String>, rusqlite::Error> {
        let mut stmt = self.db.prepare("SELECT session_id FROM session_docs")?;
        let ids = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(ids)
    }

    /// Run a BM25-ranked FTS5 query over indexed sessions.
    ///
    /// Multi-token queries require every token (AND) first; when that
    /// intersection matches nothing the query reruns as an OR so partial
    /// matches still surface. Returns `(results, next_offset, total_estimate)`.
    pub fn query(
        &self,
        query: &str,
        cwd: Option<&str>,
        limit: usize,
        offset: usize,
        include_content: bool,
    ) -> Result<QueryResult, rusqlite::Error> {
        let Some((and_query, or_query)) = Self::build_match_queries(query) else {
            return Ok(QueryResult {
                results: Vec::new(),
                next_offset: None,
                total_estimate: Some(0),
            });
        };

        let result = self.run_match_query(&and_query, cwd, limit, offset, include_content)?;
        // Gate the fallback on the total (not the page) so every offset of one
        // logical query is served by the same match string.
        if result.total_estimate == Some(0) && and_query != or_query {
            return self.run_match_query(&or_query, cwd, limit, offset, include_content);
        }
        Ok(result)
    }

    /// Execute one FTS5 MATCH string; `total_estimate` is computed with the
    /// same match string that produced the rows.
    fn run_match_query(
        &self,
        match_query: &str,
        cwd: Option<&str>,
        limit: usize,
        offset: usize,
        include_content: bool,
    ) -> Result<QueryResult, rusqlite::Error> {
        let total: i64 = self.db.query_row(
            "SELECT COUNT(*)
             FROM session_docs_fts
             JOIN session_docs d ON d.rowid = session_docs_fts.rowid
             WHERE session_docs_fts MATCH ?1
               AND (?2 IS NULL OR d.cwd = ?2)",
            params![match_query, cwd],
            |row| row.get(0),
        )?;

        let snippet_expr = if include_content {
            "snippet(session_docs_fts, 1, '[', ']', ' … ', 18)"
        } else {
            "NULL"
        };

        // BM25 weights: title=10.0, content=1.0
        let sql = format!(
            "SELECT
               d.session_id,
               d.cwd,
               d.title,
               d.updated_at,
               bm25(session_docs_fts, 10.0, 1.0) AS rank,
               {snippet_expr} AS snippet,
               highlight(session_docs_fts, 0, '\x01', '\x02') AS hl_title,
               highlight(session_docs_fts, 1, '\x01', '\x02') AS hl_content
             FROM session_docs_fts
             JOIN session_docs d ON d.rowid = session_docs_fts.rowid
             WHERE session_docs_fts MATCH ?1
               AND (?2 IS NULL OR d.cwd = ?2)
             ORDER BY rank ASC, d.updated_at DESC, d.session_id ASC
             LIMIT ?3 OFFSET ?4"
        );

        let mut stmt = self.db.prepare(&sql)?;
        let rows = stmt.query_map(
            params![match_query, cwd, limit as i64, offset as i64],
            |row| {
                let session_id: String = row.get("session_id")?;
                let row_cwd: String = row.get("cwd")?;
                let title: String = row.get("title")?;
                let updated_at_unix: i64 = row.get("updated_at")?;
                let rank: f64 = row.get("rank")?;
                let snippet: Option<String> = row.get("snippet")?;
                let hl_title: String = row.get("hl_title")?;
                let hl_content: String = row.get("hl_content")?;

                let score = if rank.is_finite() {
                    -(rank as f32)
                } else {
                    0.0
                };

                let mut matched_fields = Vec::new();
                if hl_title.contains('\x01') {
                    matched_fields.push("title".to_string());
                }
                if hl_content.contains('\x01') {
                    matched_fields.push("content".to_string());
                }
                if matched_fields.is_empty() {
                    matched_fields.push("content".to_string());
                }

                Ok(SessionSearchRow {
                    session_id,
                    cwd: row_cwd,
                    title,
                    updated_at_unix,
                    score,
                    matched_fields,
                    snippet,
                })
            },
        )?;

        let results: Vec<SessionSearchRow> = rows.collect::<Result<_, _>>()?;
        let total_usize = usize::try_from(total).unwrap_or(0);
        let next_offset = (offset + results.len() < total_usize).then_some(offset + results.len());

        Ok(QueryResult {
            results,
            next_offset,
            total_estimate: Some(total_usize),
        })
    }

    /// Build the AND-joined and OR-joined FTS5 MATCH strings for a query.
    ///
    /// The strings are identical for single-token queries, which lets the
    /// caller skip the fallback rerun.
    fn build_match_queries(query: &str) -> Option<(String, String)> {
        let prefixes: Vec<String> = query
            .split_whitespace()
            .flat_map(Self::sanitize_token)
            .map(Self::token_prefix)
            .collect();

        if prefixes.is_empty() {
            let fallback = query.trim();
            if fallback.is_empty() {
                return None;
            }
            let cleaned = fallback.replace('"', "");
            let phrase = format!("\"{cleaned}\" *");
            return Some((phrase.clone(), phrase));
        }

        Some((prefixes.join(" AND "), prefixes.join(" OR ")))
    }

    /// Split a query word on every stripped character instead of gluing the
    /// fragments: `session_picker.rs` must search as `session_picker` + `rs`,
    /// not as the never-indexed `session_pickerrs`. Fragments without any
    /// alphanumeric (`-`, `->`, `_`) are dropped — they tokenize to empty
    /// phrases, and an empty phrase inside an AND silently matches nothing.
    fn sanitize_token(token: &str) -> impl Iterator<Item = &str> {
        token
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
            .filter(|part| part.chars().any(|c| c.is_ascii_alphanumeric()))
    }

    /// One quoted FTS5 prefix per token, stemmed on the query side only.
    ///
    /// Plural queries reach singular docs by searching the shorter stem
    /// (`sessions` → `session*`, `caches` → `cach*`); the trailing `*` covers
    /// the reverse direction and typed stems like `ing`/`ed`, so no OR-group
    /// is needed — a `(base OR stem)` group double-counts bm25 and ranks
    /// inflected docs above exact matches. Short (< 4) words, identifiers
    /// with digits/`_`/`-`, and `ss`-tail words (`pass`, `class`) stay exact.
    fn token_prefix(token: &str) -> String {
        let stem = if token.len() < 4 || !token.chars().all(|c| c.is_ascii_alphabetic()) {
            token
        } else {
            let lower = token.to_ascii_lowercase();
            if lower.ends_with("es") {
                // The stem's prefix `*` also covers `e`-singulars (caches → cach*).
                &token[..token.len() - 2]
            } else if lower.ends_with('s') && !lower.ends_with("ss") {
                &token[..token.len() - 1]
            } else {
                token
            }
        };
        format!("\"{stem}\" *")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_doc(id: &str, title: &str, content: &str) -> SessionDoc {
        SessionDoc {
            session_id: id.to_string(),
            cwd: "/test/workspace".to_string(),
            updated_at_unix: 1700000000,
            title: title.to_string(),
            content: content.to_string(),
            content_hash: blake3::hash(content.as_bytes()).to_hex().to_string(),
            last_indexed_offset: 0,
        }
    }

    fn open(tmp: &TempDir) -> SessionSearchIndex {
        SessionSearchIndex::open_or_create(&tmp.path().join("session_search.sqlite")).unwrap()
    }

    #[test]
    fn test_open_or_create_is_idempotent() {
        let tmp = TempDir::new().unwrap();
        let _i1 = open(&tmp);
        let _i2 = open(&tmp);
    }

    fn journal_mode(index: &SessionSearchIndex) -> String {
        index
            .db
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn test_open_or_create_uses_wal_on_local_fs() {
        // Ambient kill-switch would override the decision; skip if set.
        if std::env::var("GROK_SQLITE_JOURNAL_MODE").is_ok() {
            return;
        }
        let tmp = TempDir::new().unwrap();
        assert_eq!(journal_mode(&open(&tmp)), "wal");
    }

    #[test]
    fn test_network_mode_uses_fresh_per_host_truncate_db() {
        // Network mode opens a per-host sibling of the given path (the
        // legacy shared file is left untouched — a live old binary can flip
        // it back to WAL at any time) in rollback-journal mode, and the
        // index is fully usable there.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("session_search.sqlite");

        let index =
            SessionSearchIndex::open_with_journal_mode(&path, JournalMode::Truncate).unwrap();
        assert_eq!(journal_mode(&index), "truncate");
        index
            .upsert_doc(&test_doc("s1", "NFS crash", "sigbus walIndexTryHdr"))
            .unwrap();
        let hits = index.query("sigbus", None, 10, 0, false).unwrap();
        assert_eq!(hits.results.len(), 1);
        drop(index);

        let eff = JournalMode::Truncate.effective_db_path(&path);
        assert_ne!(eff, path);
        let base = eff.display().to_string();
        assert!(!std::fs::exists(format!("{base}-wal")).unwrap());
        assert!(!std::fs::exists(format!("{base}-shm")).unwrap());
    }

    #[test]
    fn test_version_mismatch_drops_docs_and_preserves_unrelated_meta() {
        let tmp = TempDir::new().unwrap();
        {
            let index = open(&tmp);
            index
                .upsert_doc(&test_doc("s1", "Rust debugging", "borrow checker"))
                .unwrap();
            index.set_meta("last_bootstrap_at", "1700000000").unwrap();
            index.set_meta("last_upload_at", "1700000001").unwrap();
        }

        {
            // Guard against the drop branch firing on every open: a reopen at
            // the current version must keep the docs.
            let same_version = open(&tmp);
            assert_eq!(
                same_version.all_indexed_session_ids().unwrap(),
                vec!["s1".to_string()],
                "docs must survive a same-version reopen"
            );
            // Simulate a database written by an older schema version.
            same_version
                .set_meta("session_search_schema_version", "3")
                .unwrap();
            assert_eq!(
                same_version
                    .get_meta("session_search_schema_version")
                    .unwrap()
                    .as_deref(),
                Some("3"),
                "version downgrade must take effect for the migration to fire"
            );
        }

        let reopened = open(&tmp);
        assert!(
            reopened.all_indexed_session_ids().unwrap().is_empty(),
            "stale docs must be dropped on version mismatch"
        );
        assert_eq!(
            reopened
                .get_meta("session_search_schema_version")
                .unwrap()
                .as_deref(),
            Some(SCHEMA_VERSION),
            "schema version must be rewritten to current"
        );
        // The drop batch invalidates the completed-bootstrap marker (the
        // dropped tables no longer reflect a completed bootstrap) but leaves
        // every other `meta` key alone.
        assert_eq!(
            reopened.get_meta("last_bootstrap_at").unwrap(),
            None,
            "the completed-bootstrap marker must be invalidated by the drop"
        );
        assert_eq!(
            reopened.get_meta("last_upload_at").unwrap().as_deref(),
            Some("1700000001"),
            "unrelated meta keys must survive the drop"
        );
        assert_eq!(
            reopened
                .get_meta(META_KEY_MUTATION_GENERATION)
                .unwrap()
                .as_deref(),
            Some("0"),
            "a destructive rebuild must reset the freshness generation"
        );
        // Recreated tables + FTS triggers must be functional end-to-end.
        reopened
            .upsert_doc(&test_doc("s2", "Python profiling", "flamegraph"))
            .unwrap();
        let qr = reopened.query("python", None, 10, 0, false).unwrap();
        assert_eq!(qr.total_estimate, Some(1));
        assert_eq!(qr.results[0].session_id, "s2");
    }

    #[test]
    fn concurrent_upgrade_openers_drop_only_once() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("session_search.sqlite");
        {
            let index = SessionSearchIndex::open_or_create(&path).unwrap();
            index
                .upsert_doc(&test_doc("old", "Old", "stale content"))
                .unwrap();
            index
                .set_meta("session_search_schema_version", "3")
                .unwrap();
            index.set_meta(META_KEY_LAST_BOOTSTRAP, "done").unwrap();
            index
                .set_meta(META_KEY_BOOTSTRAP_CLAIM, "1000:owner")
                .unwrap();
        }

        let start = std::sync::Arc::new(std::sync::Barrier::new(3));
        let opened = std::sync::Arc::new(std::sync::Barrier::new(3));
        let write_done = std::sync::Arc::new(std::sync::Barrier::new(2));
        let opens: Vec<_> = (0..2)
            .map(|i| {
                let path = path.clone();
                let start = start.clone();
                let opened = opened.clone();
                let write_done = write_done.clone();
                std::thread::spawn(move || {
                    start.wait();
                    let index = SessionSearchIndex::open_or_create(&path).unwrap();
                    opened.wait();
                    if i == 0 {
                        index
                            .upsert_doc(&test_doc("fresh", "Fresh", "survives waiter"))
                            .unwrap();
                        write_done.wait();
                    } else {
                        write_done.wait();
                        assert!(index.get_content_hash("fresh").unwrap().is_some());
                    }
                })
            })
            .collect();
        start.wait();
        opened.wait();
        for open in opens {
            open.join().unwrap();
        }

        let index = SessionSearchIndex::open_or_create(&path).unwrap();
        assert_eq!(
            index
                .get_meta("session_search_schema_version")
                .unwrap()
                .as_deref(),
            Some(SCHEMA_VERSION)
        );
        assert_eq!(index.get_meta(META_KEY_LAST_BOOTSTRAP).unwrap(), None);
        assert_eq!(index.get_meta(META_KEY_BOOTSTRAP_CLAIM).unwrap(), None);
        assert!(
            index.get_content_hash("fresh").unwrap().is_some(),
            "the opener that waited for the upgrade must not drop a row written after both opens"
        );
    }

    #[test]
    fn test_newer_version_index_is_tolerated_not_dropped() {
        let tmp = TempDir::new().unwrap();
        {
            let index = open(&tmp);
            index
                .upsert_doc(&test_doc("s1", "Rust debugging", "borrow checker"))
                .unwrap();
            // Simulate an index owned by a newer grok generation that has
            // completed a bootstrap.
            index
                .set_meta("session_search_schema_version", "5")
                .unwrap();
            index.set_meta("last_bootstrap_at", "1700000000").unwrap();
        }

        let reopened = open(&tmp);
        assert_eq!(
            reopened.all_indexed_session_ids().unwrap(),
            vec!["s1".to_string()],
            "docs must survive an older binary opening a newer index"
        );
        assert_eq!(
            reopened
                .get_meta("session_search_schema_version")
                .unwrap()
                .as_deref(),
            Some("5"),
            "the newer generation keeps ownership of the version row"
        );
        assert_eq!(
            reopened.get_meta("last_bootstrap_at").unwrap().as_deref(),
            Some("1700000000"),
            "no drop happened, so the newer index's bootstrap marker must survive"
        );
        // The tolerated index must stay fully usable for the older binary.
        let qr = reopened.query("borrow", None, 10, 0, false).unwrap();
        assert_eq!(qr.results[0].session_id, "s1");
    }

    #[test]
    fn test_corrupt_version_row_drops_index() {
        let tmp = TempDir::new().unwrap();
        {
            let index = open(&tmp);
            index
                .upsert_doc(&test_doc("s1", "Rust debugging", "borrow checker"))
                .unwrap();
            index
                .set_meta("session_search_schema_version", "garbage")
                .unwrap();
        }

        let reopened = open(&tmp);
        assert!(
            reopened.all_indexed_session_ids().unwrap().is_empty(),
            "a corrupt version row must drop and rebuild"
        );
        assert_eq!(
            reopened
                .get_meta("session_search_schema_version")
                .unwrap()
                .as_deref(),
            Some(SCHEMA_VERSION),
            "rebuild rewrites the current version"
        );
    }

    /// Repro: the on-disk state left behind by a pre-ratchet binary that
    /// wiped the shared DB and ran its own bootstrap — a v3-stamped index
    /// with a *recent* bootstrap marker. Pins that the current binary's open
    /// drops the tables AND deletes the marker together (see the drop batch
    /// in `open_or_create`); a surviving marker would suppress re-bootstrap
    /// over empty tables.
    #[test]
    fn test_upgrade_drop_invalidates_completed_bootstrap_marker() {
        let tmp = TempDir::new().unwrap();
        {
            let index = open(&tmp);
            index
                .upsert_doc(&test_doc("s1", "old-binary doc", "indexed by v3"))
                .unwrap();
            index
                .set_meta("session_search_schema_version", "3")
                .unwrap();
            index.set_meta("last_bootstrap_at", "1783393389").unwrap();
        }

        let reopened = open(&tmp);
        assert!(
            reopened.all_indexed_session_ids().unwrap().is_empty(),
            "v3 docs must be dropped on upgrade"
        );
        assert_eq!(
            reopened
                .get_meta("session_search_schema_version")
                .unwrap()
                .as_deref(),
            Some(SCHEMA_VERSION),
            "upgrade must stamp the current version"
        );
        assert_eq!(
            reopened.get_meta("last_bootstrap_at").unwrap(),
            None,
            "the stale bootstrap marker must not survive the upgrade drop, \
             or the wiped index would be treated as fully bootstrapped"
        );

        // A subsequent bootstrap can repopulate and re-stamp the marker.
        reopened
            .upsert_doc(&test_doc("s2", "fresh doc", "indexed by v4"))
            .unwrap();
        reopened
            .set_meta("last_bootstrap_at", "1783393999")
            .unwrap();
        let qr = reopened.query("fresh", None, 10, 0, false).unwrap();
        assert_eq!(qr.results[0].session_id, "s2");
    }

    #[test]
    fn test_upsert_and_query() {
        let tmp = TempDir::new().unwrap();
        let index = open(&tmp);
        index
            .upsert_doc(&test_doc(
                "s1",
                "Rust debugging",
                "fix the borrow checker issue",
            ))
            .unwrap();

        let qr = index.query("rust", None, 10, 0, false).unwrap();
        assert_eq!(qr.total_estimate, Some(1));
        assert_eq!(qr.results[0].session_id, "s1");
        assert!(qr.results[0].score > 0.0);
        assert!(qr.results[0].matched_fields.contains(&"title".to_string()));
    }

    #[test]
    fn test_upsert_updates_existing() {
        let tmp = TempDir::new().unwrap();
        let index = open(&tmp);
        index
            .upsert_doc(&test_doc("s1", "Old title", "old content"))
            .unwrap();
        index
            .upsert_doc(&test_doc("s1", "New title about kubernetes", "new content"))
            .unwrap();

        let old = index.query("old", None, 10, 0, false).unwrap();
        assert!(
            old.results.is_empty(),
            "old content should not be searchable"
        );

        let new = index.query("kubernetes", None, 10, 0, false).unwrap();
        assert_eq!(new.results.len(), 1);
    }

    #[test]
    fn bootstrap_claim_is_exclusive_expiring_and_owner_fenced() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("session_search.sqlite");
        let first = SessionSearchIndex::open_or_create(&path).unwrap();
        let second = SessionSearchIndex::open_or_create(&path).unwrap();

        assert!(
            first
                .try_claim_bootstrap(1_000, Duration::from_secs(300), "first")
                .unwrap()
                .is_some()
        );
        assert!(
            second
                .try_claim_bootstrap(1_100, Duration::from_secs(300), "second")
                .unwrap()
                .is_none()
        );
        assert!(
            !second
                .refresh_bootstrap_claim(1_101, Duration::from_secs(300), "second")
                .unwrap()
        );
        assert!(
            first
                .refresh_bootstrap_claim(1_150, Duration::from_secs(300), "first")
                .unwrap()
        );
        assert!(
            second
                .try_claim_bootstrap(1_451, Duration::from_secs(300), "second")
                .unwrap()
                .is_some()
        );
        assert!(!first.release_bootstrap_claim("first").unwrap());
        assert!(
            !first
                .set_meta_if_claim_owner(
                    META_KEY_LAST_BOOTSTRAP,
                    "done",
                    1_451,
                    Duration::from_secs(300),
                    "first",
                )
                .unwrap()
        );
        assert!(
            second
                .set_meta_if_claim_owner(
                    META_KEY_LAST_BOOTSTRAP,
                    "done",
                    1_451,
                    Duration::from_secs(300),
                    "second",
                )
                .unwrap()
        );
        assert_eq!(
            second.get_meta(META_KEY_LAST_BOOTSTRAP).unwrap().as_deref(),
            Some("done")
        );
        assert!(second.release_bootstrap_claim("second").unwrap());
    }

    #[test]
    fn bootstrap_upsert_cannot_overwrite_newer_incremental_state() {
        let tmp = TempDir::new().unwrap();
        let bootstrap = open(&tmp);
        let incremental = open(&tmp);
        let lease = Duration::from_secs(300);
        let claim = bootstrap
            .try_claim_bootstrap(1_000, lease, "bootstrap")
            .unwrap()
            .unwrap();

        let stale = test_doc("session", "Old cwd", "before scan");
        let mut fresh = test_doc("session", "New cwd", "after scan");
        fresh.cwd = "/relocated/workspace".to_string();
        incremental.upsert_doc_incremental(&fresh).unwrap();

        assert_eq!(
            bootstrap
                .upsert_doc_if_claim_owner(
                    &stale,
                    1_001,
                    lease,
                    "bootstrap",
                    claim.mutation_generation,
                )
                .unwrap(),
            ClaimedUpsert::Stale
        );
        let hits = bootstrap.query("after", None, 10, 0, false).unwrap();
        assert_eq!(hits.results[0].cwd, "/relocated/workspace");
        assert!(
            bootstrap
                .query("before", None, 10, 0, false)
                .unwrap()
                .results
                .is_empty()
        );
    }

    #[test]
    fn bootstrap_upsert_cannot_resurrect_incrementally_deleted_session() {
        let tmp = TempDir::new().unwrap();
        let bootstrap = open(&tmp);
        let incremental = open(&tmp);
        let lease = Duration::from_secs(300);
        let original = test_doc("deleted", "Deleted", "original content");
        incremental.upsert_doc(&original).unwrap();
        let claim = bootstrap
            .try_claim_bootstrap(1_000, lease, "bootstrap")
            .unwrap()
            .unwrap();

        incremental.delete_doc("deleted").unwrap();
        assert_eq!(
            bootstrap
                .upsert_doc_if_claim_owner(
                    &original,
                    1_001,
                    lease,
                    "bootstrap",
                    claim.mutation_generation,
                )
                .unwrap(),
            ClaimedUpsert::Stale
        );
        assert_eq!(bootstrap.get_content_hash("deleted").unwrap(), None);
    }

    #[test]
    fn bootstrap_prune_retains_late_created_or_relocated_rows() {
        let tmp = TempDir::new().unwrap();
        let bootstrap = open(&tmp);
        let incremental = open(&tmp);
        let lease = Duration::from_secs(300);
        let claim = bootstrap
            .try_claim_bootstrap(1_000, lease, "bootstrap")
            .unwrap()
            .unwrap();

        incremental
            .upsert_doc_incremental(&test_doc("late", "Late", "created after scan"))
            .unwrap();
        let mut relocated = test_doc("relocated", "Moved", "new path");
        relocated.cwd = "/new/path".to_string();
        incremental.upsert_doc_incremental(&relocated).unwrap();

        assert!(
            bootstrap
                .prune_missing_if_claim_owner(
                    1_001,
                    lease,
                    "bootstrap",
                    claim.mutation_generation,
                    &std::collections::HashSet::new(),
                )
                .unwrap()
        );
        let ids: std::collections::HashSet<_> = bootstrap
            .all_indexed_session_ids()
            .unwrap()
            .into_iter()
            .collect();
        assert_eq!(ids, ["late".to_string(), "relocated".to_string()].into());
        let moved = bootstrap.query("moved", None, 10, 0, false).unwrap();
        assert_eq!(moved.results[0].cwd, "/new/path");
    }

    #[test]
    fn incremental_update_before_claim_is_replaced_by_later_bootstrap_scan() {
        let tmp = TempDir::new().unwrap();
        let bootstrap = open(&tmp);
        let incremental = open(&tmp);
        let lease = Duration::from_secs(300);
        incremental
            .upsert_doc_incremental(&test_doc("session", "Old", "before claim"))
            .unwrap();
        let claim = bootstrap
            .try_claim_bootstrap(1_000, lease, "bootstrap")
            .unwrap()
            .unwrap();

        assert_eq!(
            bootstrap
                .upsert_doc_if_claim_owner(
                    &test_doc("session", "Fresh", "scanned after claim"),
                    1_001,
                    lease,
                    "bootstrap",
                    claim.mutation_generation,
                )
                .unwrap(),
            ClaimedUpsert::Indexed
        );
        assert_eq!(
            bootstrap
                .query("scanned", None, 10, 0, false)
                .unwrap()
                .results
                .len(),
            1
        );
        assert!(
            bootstrap
                .query("before", None, 10, 0, false)
                .unwrap()
                .results
                .is_empty()
        );
    }

    #[test]
    fn bootstrap_prune_removes_rows_not_mutated_after_claim() {
        let tmp = TempDir::new().unwrap();
        let bootstrap = open(&tmp);
        let lease = Duration::from_secs(300);
        bootstrap
            .upsert_doc(&test_doc("orphan", "Orphan", "old row"))
            .unwrap();
        let claim = bootstrap
            .try_claim_bootstrap(1_000, lease, "bootstrap")
            .unwrap()
            .unwrap();

        assert!(
            bootstrap
                .prune_missing_if_claim_owner(
                    1_001,
                    lease,
                    "bootstrap",
                    claim.mutation_generation,
                    &std::collections::HashSet::new(),
                )
                .unwrap()
        );
        assert_eq!(bootstrap.get_content_hash("orphan").unwrap(), None);
    }

    #[test]
    fn reclaimed_lease_keeps_original_generation_fence() {
        let tmp = TempDir::new().unwrap();
        let stale_owner = open(&tmp);
        let new_owner = open(&tmp);
        let incremental = open(&tmp);
        let lease = Duration::from_secs(300);
        let first_claim = stale_owner
            .try_claim_bootstrap(1_000, lease, "first")
            .unwrap()
            .unwrap();
        let fresh = test_doc("session", "Fresh", "incremental after first claim");
        incremental.upsert_doc_incremental(&fresh).unwrap();
        let second_claim = new_owner
            .try_claim_bootstrap(1_301, lease, "second")
            .unwrap()
            .unwrap();
        assert!(second_claim.mutation_generation > first_claim.mutation_generation);

        assert_eq!(
            stale_owner
                .upsert_doc_if_claim_owner(
                    &test_doc("session", "Stale", "first bootstrap"),
                    1_301,
                    lease,
                    "first",
                    first_claim.mutation_generation,
                )
                .unwrap(),
            ClaimedUpsert::Lost
        );
        assert_eq!(
            new_owner
                .upsert_doc_if_claim_owner(
                    &fresh,
                    1_301,
                    lease,
                    "second",
                    second_claim.mutation_generation,
                )
                .unwrap(),
            ClaimedUpsert::Unchanged
        );
        assert_eq!(
            new_owner
                .query("incremental", None, 10, 0, false)
                .unwrap()
                .results
                .len(),
            1
        );
    }

    #[test]
    fn expired_owner_cannot_refresh_write_prune_or_mark_complete() {
        let tmp = TempDir::new().unwrap();
        let index = open(&tmp);
        let lease = Duration::from_secs(300);
        let claim = index
            .try_claim_bootstrap(1_000, lease, "expired")
            .unwrap()
            .unwrap();
        index
            .upsert_doc(&test_doc("stale", "Stale", "row"))
            .unwrap();

        assert!(
            !index
                .refresh_bootstrap_claim(1_301, lease, "expired")
                .unwrap()
        );
        assert_eq!(
            index
                .upsert_doc_if_claim_owner(
                    &test_doc("late", "Late", "write"),
                    1_301,
                    lease,
                    "expired",
                    claim.mutation_generation,
                )
                .unwrap(),
            ClaimedUpsert::Lost
        );
        assert!(
            !index
                .insert_doc_if_absent_if_claim_owner(
                    &test_doc("placeholder", "Placeholder", ""),
                    1_301,
                    lease,
                    "expired",
                    claim.mutation_generation,
                )
                .unwrap()
        );
        assert!(
            !index
                .prune_missing_if_claim_owner(
                    1_301,
                    lease,
                    "expired",
                    claim.mutation_generation,
                    &std::collections::HashSet::new(),
                )
                .unwrap()
        );
        assert!(
            !index
                .set_meta_if_claim_owner(META_KEY_LAST_BOOTSTRAP, "done", 1_301, lease, "expired",)
                .unwrap()
        );
        assert_eq!(index.get_content_hash("late").unwrap(), None);
        assert_eq!(index.get_content_hash("placeholder").unwrap(), None);
        assert!(index.get_content_hash("stale").unwrap().is_some());
        assert_eq!(index.get_meta(META_KEY_LAST_BOOTSTRAP).unwrap(), None);
    }

    #[test]
    fn malformed_and_future_bootstrap_claims_are_reclaimable() {
        let tmp = TempDir::new().unwrap();
        let index = open(&tmp);
        index.set_meta(META_KEY_BOOTSTRAP_CLAIM, "garbage").unwrap();
        assert!(
            index
                .try_claim_bootstrap(1_000, Duration::from_secs(300), "owner")
                .unwrap()
                .is_some()
        );
        index
            .set_meta(META_KEY_BOOTSTRAP_CLAIM, "9999999999:future")
            .unwrap();
        assert!(
            index
                .try_claim_bootstrap(1_000, Duration::from_secs(300), "owner-2")
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn test_delete_doc() {
        let tmp = TempDir::new().unwrap();
        let index = open(&tmp);
        index
            .upsert_doc(&test_doc("s1", "Delete me", "some content about python"))
            .unwrap();
        assert_eq!(index.all_indexed_session_ids().unwrap().len(), 1);

        index.delete_doc("s1").unwrap();
        assert!(index.all_indexed_session_ids().unwrap().is_empty());

        assert!(
            index
                .query("python", None, 10, 0, false)
                .unwrap()
                .results
                .is_empty()
        );
    }

    #[test]
    fn test_content_hash_dedup() {
        let tmp = TempDir::new().unwrap();
        let index = open(&tmp);
        let doc = test_doc("s1", "Title", "body");
        index.upsert_doc(&doc).unwrap();

        assert_eq!(
            index.get_content_hash("s1").unwrap().as_deref(),
            Some(doc.content_hash.as_str())
        );
        assert_eq!(index.get_content_hash("nonexistent").unwrap(), None);
    }

    #[test]
    fn test_insert_doc_if_absent_never_overwrites() {
        let tmp = TempDir::new().unwrap();
        let index = open(&tmp);
        let full = test_doc("s1", "Rust debugging", "borrow checker");
        index.upsert_doc(&full).unwrap();

        // Conflict arm: an existing (fuller) row must be left untouched.
        index
            .insert_doc_if_absent(&test_doc("s1", "placeholder", ""))
            .unwrap();
        assert_eq!(
            index.get_content_hash("s1").unwrap().as_deref(),
            Some(full.content_hash.as_str()),
            "existing row must not be downgraded to the placeholder"
        );
        let qr = index.query("borrow", None, 10, 0, false).unwrap();
        assert_eq!(
            qr.results[0].session_id, "s1",
            "full content must remain FTS-queryable after the no-op insert"
        );

        // Insert arm: a new id must land and fire the FTS trigger.
        index
            .insert_doc_if_absent(&test_doc("s2", "Python profiling", ""))
            .unwrap();
        let qr = index.query("python", None, 10, 0, false).unwrap();
        assert_eq!(qr.total_estimate, Some(1));
        assert_eq!(qr.results[0].session_id, "s2");
    }

    #[test]
    fn test_query_cwd_filter() {
        let tmp = TempDir::new().unwrap();
        let index = open(&tmp);

        let mut doc_a = test_doc("s1", "Rust project", "cargo build");
        doc_a.cwd = "/workspace/a".to_string();
        let mut doc_b = test_doc("s2", "Rust library", "cargo test");
        doc_b.cwd = "/workspace/b".to_string();
        index.upsert_doc(&doc_a).unwrap();
        index.upsert_doc(&doc_b).unwrap();

        let all = index.query("rust", None, 10, 0, false).unwrap();
        assert_eq!(all.results.len(), 2);

        let filtered = index
            .query("rust", Some("/workspace/a"), 10, 0, false)
            .unwrap();
        assert_eq!(filtered.results.len(), 1);
        assert_eq!(filtered.results[0].session_id, "s1");
    }

    #[test]
    fn test_query_pagination() {
        let tmp = TempDir::new().unwrap();
        let index = open(&tmp);
        for i in 0..5 {
            index
                .upsert_doc(&test_doc(
                    &format!("s{i}"),
                    &format!("Session {i}"),
                    &format!("rust content {i}"),
                ))
                .unwrap();
        }

        let page1 = index.query("rust", None, 2, 0, false).unwrap();
        assert_eq!(page1.results.len(), 2);
        assert_eq!(page1.total_estimate, Some(5));
        assert_eq!(page1.next_offset, Some(2));

        let page2 = index.query("rust", None, 2, 2, false).unwrap();
        assert_eq!(page2.results.len(), 2);
    }

    #[test]
    fn test_query_with_snippets() {
        let tmp = TempDir::new().unwrap();
        let index = open(&tmp);
        index
            .upsert_doc(&test_doc(
                "s1",
                "Debugging session",
                "the rust borrow checker was causing lifetime errors in the parser",
            ))
            .unwrap();

        let qr = index.query("borrow checker", None, 10, 0, true).unwrap();
        assert_eq!(qr.results.len(), 1);
        assert!(qr.results[0].snippet.is_some());
    }

    #[test]
    fn test_query_empty_string() {
        let tmp = TempDir::new().unwrap();
        let index = open(&tmp);
        index.upsert_doc(&test_doc("s1", "Title", "body")).unwrap();

        let qr = index.query("", None, 10, 0, false).unwrap();
        assert!(qr.results.is_empty());
        assert_eq!(qr.total_estimate, Some(0));
    }

    #[test]
    fn test_query_special_chars_sanitized() {
        let tmp = TempDir::new().unwrap();
        let index = open(&tmp);
        index
            .upsert_doc(&test_doc("s1", "Title", "hello world"))
            .unwrap();

        // Special chars should be stripped, leaving "hello"
        let qr = index.query("hello!!!", None, 10, 0, false).unwrap();
        assert_eq!(qr.results.len(), 1);
    }

    #[test]
    fn test_matched_fields_title_vs_content() {
        let tmp = TempDir::new().unwrap();
        let index = open(&tmp);
        index
            .upsert_doc(&test_doc(
                "s1",
                "kubernetes deployment",
                "unrelated body text",
            ))
            .unwrap();

        let qr = index.query("kubernetes", None, 10, 0, false).unwrap();
        assert_eq!(qr.results.len(), 1);
        assert!(qr.results[0].matched_fields.contains(&"title".to_string()));
    }

    /// cwd is a filter dimension, not a search dimension. A term that only
    /// appears in the cwd must never cause a session to match.
    #[test]
    fn test_cwd_not_searchable() {
        let tmp = TempDir::new().unwrap();
        let index = open(&tmp);
        let mut doc = test_doc("s1", "unrelated title", "unrelated content");
        doc.cwd = "/Users/alice/workspace/supercalifragilistic".to_string();
        index.upsert_doc(&doc).unwrap();

        let qr = index
            .query("supercalifragilistic", None, 10, 0, false)
            .unwrap();
        assert!(
            qr.results.is_empty(),
            "cwd-only term must not match, got {} results",
            qr.results.len()
        );
    }

    #[test]
    fn test_query_filename_tokens_split() {
        let tmp = TempDir::new().unwrap();
        let index = open(&tmp);
        index
            .upsert_doc(&test_doc(
                "s1",
                "Fix list rendering",
                "the bug lives in session_picker.rs near the filter",
            ))
            .unwrap();

        // Pins splitting on stripped chars: gluing the fragments produced the
        // never-indexed token `session_pickerrs`, so this query found nothing.
        let qr = index
            .query("session_picker.rs", None, 10, 0, false)
            .unwrap();
        assert_eq!(qr.total_estimate, Some(1));
        assert_eq!(qr.results[0].session_id, "s1");
    }

    #[test]
    fn test_query_and_first_with_or_fallback() {
        let tmp = TempDir::new().unwrap();
        let index = open(&tmp);
        index
            .upsert_doc(&test_doc(
                "s1",
                "Borrow both",
                "fix the borrow checker issue",
            ))
            .unwrap();
        index
            .upsert_doc(&test_doc("s2", "Borrow only", "borrow money from the bank"))
            .unwrap();
        index
            .upsert_doc(&test_doc("s3", "Tokio doc", "tokio runtime setup"))
            .unwrap();
        index
            .upsert_doc(&test_doc("s4", "Sqlite doc", "sqlite index tuning"))
            .unwrap();

        // AND has hits: only the doc matching every token is returned, so
        // partial matches cannot dilute the result set.
        let qr = index.query("borrow checker", None, 10, 0, false).unwrap();
        assert_eq!(qr.total_estimate, Some(1));
        assert_eq!(qr.results[0].session_id, "s1");

        // A separator-only word (`->`) must be dropped, not become an empty
        // phrase that silently makes the whole AND match nothing.
        let qr = index.query("fix -> borrow", None, 10, 0, false).unwrap();
        assert_eq!(qr.total_estimate, Some(1));
        assert_eq!(qr.results[0].session_id, "s1");

        // No doc has both tokens: the OR rerun surfaces the partial matches.
        let qr = index.query("tokio sqlite", None, 10, 0, false).unwrap();
        assert_eq!(qr.total_estimate, Some(2));
        let ids: Vec<&str> = qr.results.iter().map(|r| r.session_id.as_str()).collect();
        assert!(
            ids.contains(&"s3") && ids.contains(&"s4"),
            "OR fallback must return both partial matches: {ids:?}"
        );
    }

    #[test]
    fn test_query_plural_variants() {
        let tmp = TempDir::new().unwrap();
        let index = open(&tmp);
        index
            .upsert_doc(&test_doc("s1", "Plural doc", "resumed sessions list"))
            .unwrap();
        index
            .upsert_doc(&test_doc("s2", "Singular doc", "resume the session flow"))
            .unwrap();

        // Plural query, singular doc: pins the query-side stem — without it
        // `sessions*` cannot prefix-match `session`.
        let qr = index.query("sessions", None, 10, 0, false).unwrap();
        let ids: Vec<&str> = qr.results.iter().map(|r| r.session_id.as_str()).collect();
        assert!(
            ids.contains(&"s2"),
            "singular doc must match a plural query: {ids:?}"
        );

        // Singular query, plural doc: pins the prefix-`*` coverage that makes
        // an added plural variant unnecessary.
        let qr = index.query("session", None, 10, 0, false).unwrap();
        let ids: Vec<&str> = qr.results.iter().map(|r| r.session_id.as_str()).collect();
        assert!(
            ids.contains(&"s1"),
            "plural doc must match a singular query: {ids:?}"
        );
    }

    #[test]
    fn test_query_pure_symbol_fallback() {
        let tmp = TempDir::new().unwrap();
        let index = open(&tmp);
        index.upsert_doc(&test_doc("s1", "Title", "body")).unwrap();

        // No indexable characters: the raw-phrase fallback must not error.
        let qr = index.query("…", None, 10, 0, false).unwrap();
        assert!(qr.results.is_empty());
        assert_eq!(qr.total_estimate, Some(0));
    }

    #[test]
    #[cfg(unix)]
    fn open_or_create_creates_owner_only_parent() {
        let tmp = TempDir::new().unwrap();
        let parent = tmp.path().join("sessions");
        let db_path = parent.join("session_search.sqlite");
        let _ = SessionSearchIndex::open_or_create(&db_path).unwrap();
        assert_eq!(
            crate::test_support::unix_mode(&parent),
            0o700,
            "sessions parent created by open_or_create must be 0700"
        );
    }
}
