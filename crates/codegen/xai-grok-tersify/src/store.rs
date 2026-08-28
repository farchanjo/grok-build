//! Persistent recovery store: one SQLite database, byte-exact originals.
//!
//! The store is the gate for every lossy (S4) emission: the engine may only
//! ship compressed bytes when `get(handle)` returns the stored payload
//! byte-for-byte. An unknown handle is an explicit miss — the store never
//! guesses a recovery.
//!
//! Journal mode is not hardcoded to WAL: `xai-sqlite-journal` picks WAL on
//! local disks and a safe rollback journal on network mounts, where WAL's
//! mmap'd `-shm` file can SIGBUS. The 5-second busy timeout is load-bearing,
//! not tuning: this store is opened once per process (the TUI, a leader, and
//! CLI helpers contend), and without the timeout a concurrent writer returns
//! SQLITE_BUSY instead of waiting.
//!
//! A payload budget refuses a NEW recovery before publishing lossy bytes when
//! the configured cap would be exceeded; existing handles stay retrievable and
//! callers must pass through. A full store must degrade to "no compression",
//! never to "lost data".

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;
use xai_sqlite_journal::JournalMode;

use crate::engine::{RecoveryStore, recovery_handle};

/// Default retained-payload budget: 512 MiB, matching the upstream engine.
pub const DEFAULT_MAX_STORAGE_BYTES: i64 = 512 << 20;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS recoveries (
  handle TEXT PRIMARY KEY,
  created_at TEXT NOT NULL,
  original BLOB NOT NULL
);";

/// A `RecoveryStore` backed by a SQLite file (or `:memory:` for tests).
///
/// `rusqlite::Connection` is `Send` but its locking is coarse; a `Mutex` over
/// a single connection serializes access exactly like the upstream design, so
/// two compressions in one process never contend with each other's
/// transaction.
pub struct SqliteStore {
    conn: Mutex<Connection>,
    max_bytes: i64,
}

impl SqliteStore {
    /// Open (creating if needed) the store at `path` and initialize the schema.
    ///
    /// Parent directories must exist; the journal mode is chosen for the
    /// filesystem the path actually lives on.
    pub fn open(path: &Path) -> Result<Self, rusqlite::Error> {
        Self::open_with_budget(path, DEFAULT_MAX_STORAGE_BYTES)
    }

    #[must_use]
    pub fn in_memory() -> Self {
        // A shared-cache in-memory database survives as long as this handle.
        let conn = Connection::open_in_memory().expect("in-memory sqlite opens");
        conn.execute_batch(SCHEMA).expect("in-memory schema");
        Self {
            conn: Mutex::new(conn),
            max_bytes: DEFAULT_MAX_STORAGE_BYTES,
        }
    }

    /// Open with an explicit retained-payload budget, in bytes.
    pub fn open_with_budget(path: &Path, max_bytes: i64) -> Result<Self, rusqlite::Error> {
        let mode = JournalMode::for_db_path(path);
        let conn = mode.open(path)?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self {
            conn: Mutex::new(conn),
            max_bytes,
        })
    }

    fn lock(&self) -> MutexGuard<'_, Connection> {
        // Poison means a panic happened mid-statement, not that SQLite state
        // is corrupt; recovering the guard keeps a store usable after an
        // unrelated panic elsewhere.
        self.conn
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn storage_bytes(&self) -> Result<i64, rusqlite::Error> {
        let conn = self.lock();
        let mut stmt = conn.prepare("SELECT COALESCE(SUM(LENGTH(original)), 0) FROM recoveries")?;
        let mut rows = stmt.query([])?;
        match rows.next()? {
            Some(row) => row.get::<_, i64>(0),
            None => Ok(0),
        }
    }

    /// Total retained payload bytes, for the TUI's store panel.
    #[must_use]
    pub fn used_bytes(&self) -> i64 {
        self.storage_bytes().unwrap_or(0)
    }

    /// Handle count, for the TUI's store panel.
    #[must_use]
    pub fn entry_count(&self) -> i64 {
        let conn = self.lock();
        conn.query_row("SELECT COUNT(*) FROM recoveries", [], |r| r.get(0))
            .unwrap_or(0)
    }
}

impl RecoveryStore for SqliteStore {
    fn put(&mut self, original: &[u8]) -> String {
        let handle = recovery_handle(original);
        let new_bytes = original.len() as i64;
        let size: Option<i64> = self
            .lock()
            .prepare_cached("SELECT LENGTH(original) FROM recoveries WHERE handle = ?1")
            .ok()
            .and_then(|mut stmt| {
                stmt.query_row([handle.as_str()], |r| r.get::<_, Option<i64>>(0))
                    .ok()
                    .flatten()
            });
        // Idempotent: same content -> same handle -> same bytes, no growth.
        if size == Some(new_bytes) {
            return handle;
        }
        let budget_ok = self
            .storage_bytes()
            .map(|used| used + new_bytes <= self.max_bytes)
            .unwrap_or(false);
        if !budget_ok {
            // A store with no room must read as "no handle", so the engine
            // fails closed to pass-through. Empty string is never a valid
            // handle prefix and the engine re-verifies retrieval anyway.
            return String::new();
        }
        let created = chrono_like_now();
        let conn = self.lock();
        let inserted = conn.execute(
            "INSERT INTO recoveries (handle, created_at, original) VALUES (?1, ?2, ?3)
             ON CONFLICT(handle) DO NOTHING",
            rusqlite::params![handle, created, original],
        );
        match inserted {
            Ok(_) => handle,
            Err(_) => String::new(),
        }
    }

    fn get(&self, handle: &str) -> Option<Vec<u8>> {
        if handle.is_empty() {
            return None;
        }
        let conn = self.lock();
        conn.query_row(
            "SELECT original FROM recoveries WHERE handle = ?1",
            [handle],
            |r| r.get::<_, Vec<u8>>(0),
        )
        .ok()
    }
}

/// UTC seconds since the unix epoch as text. Deliberately not a timestamp
/// crate: the column is for debugging and future lifecycle rules, and a
/// monotonic-enough wall clock is all it needs.
fn chrono_like_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_then_get_returns_byte_exact_original() {
        let mut store = SqliteStore::in_memory();
        let payload: Vec<u8> = (0..256u16).map(|i| i as u8).collect();
        let handle = store.put(&payload);
        assert!(handle.starts_with("rcv_"), "valid handle: {handle}");
        assert_eq!(store.get(&handle).as_deref(), Some(payload.as_slice()));
    }

    #[test]
    fn unknown_handle_is_an_explicit_miss_never_a_guess() {
        let store = SqliteStore::in_memory();
        assert_eq!(store.get("rcv_0000000000000000"), None);
        assert_eq!(store.get(""), None);
    }

    #[test]
    fn put_is_idempotent_and_does_not_double_storage() {
        let mut store = SqliteStore::in_memory();
        let h1 = store.put(b"same bytes");
        let used1 = store.used_bytes();
        let h2 = store.put(b"same bytes");
        assert_eq!(h1, h2);
        assert_eq!(used1, store.used_bytes());
        assert_eq!(store.entry_count(), 1);
    }

    #[test]
    fn budget_exhaustion_refuses_new_entries_and_reports_no_handle() {
        // Budget fits the header-sized rows barely; first insert must fit.
        let mut store = SqliteStore::open_with_budget(Path::new(":memory:"), 64)
            .or_else(|_| SqliteStore::open_with_budget(std::path::Path::new(":memory:"), 64))
            .map(|mut s| {
                let _ = s.put(b"x");
                s
            })
            .unwrap_or_else(|_| SqliteStore::in_memory());
        store.max_bytes = 16;
        // Small entry still fits the 16-byte budget.
        let small = store.put(b"tiny");
        assert!(small.starts_with("rcv_"), "small entry must fit: {small}");
        // 32 bytes cannot fit in what is left of 16; must return no handle and
        // leave the existing entry retrievable.
        let big = store.put(&[b'y'; 32]);
        assert!(
            big.is_empty(),
            "over-budget put must yield no handle, got {big}"
        );
        assert_eq!(store.get(&small).as_deref(), Some(&b"tiny"[..]));
    }

    #[test]
    fn empty_handle_from_a_refused_put_fails_engine_verification() {
        // The engine only trusts a handle whose bytes it can read back.
        let store = SqliteStore::in_memory();
        assert_eq!(store.get(""), None);
    }
}
