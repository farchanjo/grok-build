//! Session-local media artifact store, semantic cache, auxiliary usage
//! ledger, and the shell-owned media-understanding backend (plan sections 4,
//! 11, and 12).
//!
//! PR 5 owns the durable, BLAKE3-addressed content store under
//! `<session_dir>/assets/media/`, the canonical semantic cache keyed by every
//! variable that can change the meaning of a delegate result, and the
//! append-only auxiliary usage ledger.
//!
//! PR 6 adds the shell-owned backend on top of that store:
//! route resolution ([`routes`]), provider transport planning ([`transport`]),
//! preprocessing ([`preprocess`]), the policy/ZDR/consent host gates
//! ([`policy`], [`zdr`], [`consent`]), the per-route dedicated
//! `InferenceClient` invoker ([`invoker`]), and the
//! [`ShellMediaUnderstandingBackend`] orchestration ([`backend`]).
//!
//! PR 8 adds the compaction preflight ([`compaction`]): one
//! purpose-scoped enrichment per stable compaction job so every text-only
//! summarization request shares a single pairing-safe enriched snapshot.
//!
//! Invariants established by PR 6:
//! - Every resolved route owns a **separately constructed** `InferenceClient`;
//!   the parent session `InferenceHandle` is never reused.
//! - Provider-hidden fallback routing (OpenRouter fallback models/plugins/
//!   pacing, backend search, unrelated reasoning overrides) is cleared so the
//!   configured route order is authoritative.
//! - A **concrete transport** is known before any bytes leave; routes whose
//!   strategy has no wire path in the current implementation are skipped
//!   without sending bytes.
//! - Two independent host gates run before any provider transmission:
//!   filesystem/tool permission, then purpose-scoped disclosure consent.
//!   Consent is YOLO-proof (not bypassed by always-approve) and is consulted
//!   for every fallback provider.
//! - The ZDR gate fails closed from trusted host/provider/account metadata
//!   only; it never consults a user-authored allowlist.
//! - Cache keys cover every semantic/preprocess variable; results are never
//!   keyed by result text.
//!
//! Layout (relative to the session directory):
//!
//! ```text
//! assets/media/
//! ├── objects/
//! │   ├── blobs/<source-blake3>
//! │   ├── derived/<derivative-key-blake3>
//! │   └── results/<semantic-key-blake3>.json
//! ├── refs/
//! │   ├── attachments/
//! │   ├── compaction/
//! │   └── checkpoints/
//! ├── index.json
//! ├── journal.jsonl
//! └── usage.jsonl
//! ```

pub(crate) mod artifacts;
pub(crate) mod auto_enrich;
pub(crate) mod backend;
pub(crate) mod cache;
pub(crate) mod compaction;
pub(crate) mod consent;
pub(crate) mod invoker;
pub(crate) mod ledger;
pub(crate) mod policy;
pub(crate) mod preprocess;
pub(crate) mod routes;
pub(crate) mod transport;
pub(crate) mod zdr;

pub(crate) use artifacts::{
    ArtifactKind, JournalEvent, LIVE_ATTACHMENT_REF, MediaArtifactStore, MediaIndex,
    MediaRedactionManifest, MediaTextExport, ObjectRef, RefEntry, RefKind, StoredSemanticResult,
    copy_media_store,
};
pub(crate) use auto_enrich::{
    EnrichDecision, EnrichedUserMessage, SessionMediaConsentProvider, SessionMediaContext,
    auto_enrich_kill_switched, decide, modality_for, render_semantics_envelope,
};
pub(crate) use backend::{ShellMediaBackendContext, ShellMediaUnderstandingBackend};
pub(crate) use cache::{SemanticCache, SemanticCacheKey};
pub(crate) use compaction::{
    CompactionAnalyzer, CompactionEnrichmentMode, MediaPreflightError, PreparedCompactionSource,
    compaction_enrich_kill_switched, compaction_enrichment_mode, fingerprint_snapshot,
    prepare_media_semantics, run_compaction_preflight,
};
pub(crate) use consent::{
    ConsentDecision, ConsentRequest, DisclosureConsentGate, DisclosurePurpose,
    InteractiveMediaConsentProvider, MediaConsentProvider,
};
pub(crate) use invoker::{AuxMediaInvoker, DelegateOutcome, InvokerContext};
pub(crate) use ledger::{UsageLedger, UsagePurpose, UsageRow};
pub(crate) use policy::{MediaItemBytes, MediaPolicyLimits, PolicyError};
pub(crate) use preprocess::{PreprocessError, PreprocessOutcome, PreprocessProfile};
pub(crate) use routes::{ResolvedRoute, RouteEligibility, RouteResolution};
pub(crate) use transport::{
    TransportPlan, concrete_strategy_for_auto, route_is_transport_eligible,
};
pub(crate) use zdr::zdr_route_eligible;

use std::fs::OpenOptions;
use std::io::{self, Read, Seek, Write};
use std::path::{Path, PathBuf};

/// Unix seconds since the epoch for journal/ledger rows.
pub(crate) fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// A unique sibling temp path, e.g. `foo` -> `foo.<uuid>.tmp`.
fn temp_sibling(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(format!(".{}.tmp", uuid::Uuid::now_v7()));
    PathBuf::from(name)
}

/// Append `line` (which must end with `\n`) to a JSONL file under an exclusive
/// advisory lock (`<path>.jsonl.lock`). Concurrent writers in leader mode
/// serialize on the lock and never produce torn lines; the row is fsynced for
/// durability before the lock is released.
pub(crate) fn append_jsonl_line_locked(path: &Path, mut line: Vec<u8>) -> io::Result<()> {
    debug_assert!(line.ends_with(b"\n"), "JSONL record must end with \\n");
    let lock = lock_jsonl_append(path)?;
    let result = (|| {
        let mut file = OpenOptions::new()
            .read(true)
            .create(true)
            .append(true)
            .open(path)?;
        let len = file.metadata()?.len();
        if len > 0 {
            file.seek(io::SeekFrom::Start(len - 1))?;
            let mut last = [0u8; 1];
            file.read_exact(&mut last)?;
            if last[0] != b'\n' {
                line.insert(0, b'\n');
            }
        }
        file.write_all(&line)?;
        file.flush()?;
        crate::session::storage::sync_file_durable(&file)?;
        drop(file);
        crate::session::storage::sync_parent_directory(path)?;
        Ok(())
    })();
    let _ = lock.unlock();
    result
}

/// Take an exclusive advisory lock on `<path>.jsonl.lock` for an append-only
/// JSONL writer. See [`append_jsonl_line_locked`].
fn lock_jsonl_append(path: &Path) -> io::Result<std::fs::File> {
    use fs2::FileExt;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path.with_extension("jsonl.lock"))?;
    lock.lock_exclusive()?;
    Ok(lock)
}

/// Read a whole JSONL file into typed rows. Missing or empty files yield `[]`.
pub(crate) fn read_jsonl<T: serde::de::DeserializeOwned>(path: &Path) -> io::Result<Vec<T>> {
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let contents = std::fs::read_to_string(path)?;
    let mut items = Vec::new();
    for line in contents.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let item: T = serde_json::from_str(line)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        items.push(item);
    }
    Ok(items)
}

/// Write an immutable object file with create-new semantics: the bytes are
/// written to a unique temp sibling, fsynced, then hard-linked into place. A
/// hard link fails with `AlreadyExists` if the object path already exists, so
/// an existing object is never overwritten. The caller is responsible for
/// verifying that any pre-existing content matches the content-addressed key.
pub(crate) fn write_object_create_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = temp_sibling(path);
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&tmp)?;
        file.write_all(bytes)?;
        crate::session::storage::sync_file_durable(&file)?;
        drop(file);
        match std::fs::hard_link(&tmp, path) {
            Ok(()) => {
                std::fs::remove_file(&tmp)?;
                crate::session::storage::sync_parent_directory(path)?;
                Ok(())
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                // Object already present; create-new semantics honored.
                std::fs::remove_file(&tmp)?;
                Ok(())
            }
            Err(e) => {
                std::fs::remove_file(&tmp)?;
                Err(e)
            }
        }
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// Verify that `bytes` hashes to the lowercase hex BLAKE3 digest `key`.
pub(crate) fn verify_blake3(key: &str, bytes: &[u8]) -> io::Result<()> {
    let actual = blake3::hash(bytes).to_hex().to_string();
    if actual != key {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "media object content did not match its content-addressed key \
                 (expected {key}, got {actual})"
            ),
        ));
    }
    Ok(())
}
