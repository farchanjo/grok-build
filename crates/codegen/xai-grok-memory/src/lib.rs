//! Memory system for cross-session knowledge persistence.
//!
//! This crate provides a markdown-based memory storage layer that allows
//! Grok to persist important information across sessions. Memory files are
//! stored under `~/.grok/memory/` with workspace-scoped subdirectories
//! keyed by a blake3 hash of the workspace path.
//!
//! ## Data Layout
//!
//! ```text
//! ~/.grok/memory/
//!   ├── MEMORY.md                         # Global curated knowledge
//!   └── {workspace_hash}/                 # Per-workspace (blake3(cwd)[..16])
//!       ├── MEMORY.md                     # Project-level curated knowledge
//!       └── sessions/
//!           └── YYYY-MM-DD-{slug}-{sid8}.md  # Session logs
//! ```
//!
//! ## Feature Flag
//!
//! Memory is gated behind `--experimental-memory` CLI flag or
//! `GROK_MEMORY=1` environment variable. When disabled, this crate
//! is not initialized by the host.

pub mod archive;
pub mod backend;
pub mod chunker;
pub mod dream;
pub mod dream_lock;
pub mod embedding;
pub mod fingerprint;
pub mod index;
pub mod metadata_index;
pub mod mirror;
#[cfg(feature = "milvus")]
pub mod mirror_milvus;
pub mod mmr;
pub mod query_expansion;
pub mod rebuild;
pub mod retrieval;
pub mod schema;
pub mod search;
pub mod storage;
pub mod text_utils;
pub mod watcher;
pub mod workspace_identity;

pub use backend::{
    EndpointScopedCredentials, MemoryBackendImpl, MemoryBackendParams, resolve_embedding_provider,
};
pub use embedding::{
    L2_NORMALIZATION_VERSION, NormalizeError, l2_normalize_v1, validate_embedding_batch,
};
pub use fingerprint::{
    EmbeddingSourceSpec, NORMALIZATION_L2_V1, VECTOR_SCHEMA_VERSION, VectorFingerprint,
};
pub use index::{MemoryIndex, init_sqlite_vec};
pub use metadata_index::{
    CollectionKind, MetadataFtsHit, MetadataIndex, MetadataIndexError, MetadataItem,
    MetadataKnnHit, UpsertResult, metadata_doc_prep, metadata_index_path,
    metadata_index_path_for_cwd, reject_persisted_paths,
};
pub use mirror::{
    DEFAULT_MIRROR_TIMEOUT_SECS, InMemoryVectorMirror, MemoryRow, MemoryVecResyncSource,
    MirrorError, MirrorErrorKind, MirrorHandle, MirrorResyncSource, MirrorSnapshot, MirrorState,
    MEMORY_SCHEMA_VERSION_V2, RESYNC_BATCH_ROWS, RemoteSearchHit, ResyncReport, VectorMirror,
    collection_tag, memory_collection_name, mirror_call, mirror_delete_ids, mirror_sync_rows,
    mirror_timeout, parse_collection_tag, prime_collection_name, resync_collection,
    similarity_to_l2_distance,
};
pub use retrieval::{MemoryRetrieval, RetrievalError, RetrievalErrorKind};
pub use storage::{MemoryScope, MemoryStorage};
pub use workspace_identity::{workspace_identity_hash16, workspace_storage_identity};

/// Embed all chunks that don't have embeddings yet.
///
/// Queries the index for unembedded chunks, batches them through the
/// embedding provider, and upserts the results. Logs progress.
///
/// This is the async glue between the sync `MemoryIndex` and the async
/// `EmbeddingProvider`. Call after reindex, flush writes, or session-end writes.
pub async fn embed_missing_chunks(
    index: &MemoryIndex,
    provider: &dyn embedding::EmbeddingProvider,
) -> usize {
    embed_missing_chunks_with_mirror(index, provider, None).await
}

/// [`embed_missing_chunks`] with optional best-effort vector-mirror fan-out.
///
/// SQLite is written first exactly as before and remains the authority; the
/// mirror receives the new vectors afterwards with per-call timeouts and
/// never affects the returned count or any error path. When the mirror is
/// not yet verified-ready this performs a full resync from the SQLite vec
/// table (initial population / healing); when ready it performs an
/// incremental upsert plus count reconciliation. The fan-out also runs when
/// there are no missing chunks (steady state), so a not-yet-populated or
/// drifted mirror is healed on the next embed pass even though nothing was
/// embedded.
pub async fn embed_missing_chunks_with_mirror(
    index: &MemoryIndex,
    provider: &dyn embedding::EmbeddingProvider,
    mirror: Option<&MirrorHandle>,
) -> usize {
    let chunks = match index.chunks_without_embeddings() {
        // An empty list intentionally falls through: the batch loop below
        // is a no-op and the mirror fan-out at the tail still runs, so the
        // mirror is populated/reconciled even in the steady state.
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(
                target: xai_grok_telemetry::memory_log::TARGET,
                error = %e,
                "failed to query chunks without embeddings"
            );
            return 0;
        }
    };

    let total = chunks.len();
    let mut embedded = 0;
    // Rows successfully written to SQLite, for the optional mirror fan-out.
    let mut upserted: Vec<(String, Vec<f32>)> = Vec::new();

    // Batch in groups of 32 (provider's typical max batch size)
    for batch in chunks.chunks(32) {
        let texts: Vec<&str> = batch.iter().map(|(_, text)| text.as_str()).collect();
        match provider.embed_batch(&texts).await {
            Ok(embeddings) => {
                // Validate before any SQLite write: exact count, exact
                // dimension, finite values. A malformed response must fail
                // closed, never zip/mis-associate vectors (defense-in-
                // depth for any provider).
                if let Err(e) = crate::embedding::validate_embedding_batch(
                    batch.len(),
                    provider.dimensions(),
                    &embeddings,
                ) {
                    tracing::warn!(
                        target: xai_grok_telemetry::memory_log::TARGET,
                        error = %e,
                        "embedding batch validation failed; skipping; no vec rows written"
                    );
                } else {
                    for ((chunk_id, _), embedding) in batch.iter().zip(embeddings.iter()) {
                        if let Err(e) = index.upsert_embedding(chunk_id, embedding) {
                            tracing::warn!(
                                target: xai_grok_telemetry::memory_log::TARGET,
                                chunk_id,
                                error = %e,
                                "failed to upsert embedding"
                            );
                        } else {
                            embedded += 1;
                            upserted.push((chunk_id.clone(), embedding.clone()));
                        }
                    }
                }
            }
            Err(_) => {
                tracing::warn!(
                    target: xai_grok_telemetry::memory_log::TARGET,
                    batch_size = texts.len(),
                    "embedding batch failed, skipping"
                );
            }
        }
    }

    if embedded > 0 {
        tracing::info!(
            target: xai_grok_telemetry::memory_log::TARGET,
            embedded,
            total,
            "embedded missing chunks"
        );
    }
    if let Some(handle) = mirror
        && let Some(plan) = mirror_fanout_plan(index, handle, &upserted)
    {
        mirror_fanout_execute(plan, handle).await;
    }
    embedded
}

/// Owned mirror fan-out decision, computed synchronously from the index so
/// no `&MemoryIndex` borrow is ever held across an `.await` (the async fn
/// generator stores its parameters for the whole future lifetime, and the
/// memory backend search future must stay `Send`).
pub(crate) struct MirrorFanoutPlan {
    fingerprint_hash: String,
    dims: u32,
    mode: MirrorFanoutMode,
}

pub(crate) enum MirrorFanoutMode {
    /// Full resync from the SQLite vec table at `db_path` (first use,
    /// drift, or prior failure).
    Resync { db_path: std::path::PathBuf },
    /// Incremental upsert of `rows`, then count reconciliation against
    /// `expected_count` (the SQLite `vec_row_count`).
    Incremental {
        rows: Vec<(String, Vec<f32>)>,
        expected_count: u64,
    },
}

/// Compute the fan-out plan synchronously (borrows the index; no awaits).
pub(crate) fn mirror_fanout_plan(
    index: &MemoryIndex,
    handle: &MirrorHandle,
    upserted: &[(String, Vec<f32>)],
) -> Option<MirrorFanoutPlan> {
    let fingerprint_hash = match index.installed_vector_fingerprint_hash() {
        Some(fp) => fp,
        None => {
            // No pinned vector space yet; the mirror has nothing consistent
            // to mirror. A later fan-out after the rebuild installs the
            // space will populate it.
            tracing::debug!(
                target: xai_grok_telemetry::memory_log::TARGET,
                "mirror fan-out skipped: no installed vector fingerprint"
            );
            return None;
        }
    };
    let dims = index.embedding_dimensions() as u32;
    if !handle.is_ready_for(&fingerprint_hash, dims) {
        Some(MirrorFanoutPlan {
            fingerprint_hash,
            dims,
            mode: MirrorFanoutMode::Resync {
                db_path: index.db_path(),
            },
        })
    } else {
        Some(MirrorFanoutPlan {
            fingerprint_hash,
            dims,
            mode: MirrorFanoutMode::Incremental {
                rows: upserted.to_vec(),
                expected_count: index.vec_row_count().max(0) as u64,
            },
        })
    }
}

/// Execute a computed fan-out plan: time-bounded, failure-isolated mirror
/// calls. Never touches SQLite; any error only downgrades the mirror state
/// so reads fall back to sqlite-vec.
pub(crate) async fn mirror_fanout_execute(plan: MirrorFanoutPlan, handle: &MirrorHandle) {
    let timeout = mirror::mirror_timeout(None);
    match plan.mode {
        MirrorFanoutMode::Resync { db_path } => {
            match mirror::MemoryVecResyncSource::open(&db_path) {
                Ok(mut source) => {
                    if let Err(e) = mirror::resync_collection(
                        handle,
                        &plan.fingerprint_hash,
                        plan.dims,
                        &mut source,
                        timeout,
                    )
                    .await
                    {
                        tracing::warn!(
                            target: xai_grok_telemetry::memory_log::TARGET,
                            error = %e,
                            "memory mirror resync failed; reads fall back to sqlite-vec"
                        );
                    }
                }
                Err(e) => {
                    handle.mark_unavailable();
                    tracing::warn!(
                        target: xai_grok_telemetry::memory_log::TARGET,
                        error = %e,
                        "memory mirror resync source unavailable; reads fall back to sqlite-vec"
                    );
                }
            }
        }
        MirrorFanoutMode::Incremental {
            rows,
            expected_count,
        } => {
            if let Err(e) = mirror::mirror_sync_rows(
                handle,
                &plan.fingerprint_hash,
                plan.dims,
                rows,
                expected_count,
                timeout,
            )
            .await
            {
                tracing::warn!(
                    target: xai_grok_telemetry::memory_log::TARGET,
                    error = %e,
                    "memory mirror fan-out failed; reads fall back to sqlite-vec"
                );
            }
        }
    }
}

/// Summary of a milvus-mode reconciliation pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MilvusReconcileReport {
    pub unchanged: usize,
    pub embedded: usize,
    pub deleted: usize,
    pub total: usize,
}

/// Reconcile local chunks with the remote schema-v2 collection in `milvus` mode.
///
/// Diffs local chunk hashes against remote `list_id_hashes_v2`:
/// - Missing or modified chunks are embedded and upserted.
/// - Vanished chunks are deleted by id.
/// - Unchanged chunks are skipped (no re-embedding).
///
/// On completion, marks the handle ready with the total row count.
/// If Milvus is unreachable, marks the handle unavailable and returns the error.
pub async fn reconcile_milvus_mode(
    index: &mut MemoryIndex,
    provider: &dyn embedding::EmbeddingProvider,
    handle: &MirrorHandle,
    fingerprint_hash: &str,
) -> Result<MilvusReconcileReport, MirrorError> {
    let dims = provider.dimensions() as u32;

    // 1. Ensure remote schema-v2 collection exists and is compatible
    if let Err(e) = handle.ensure_collection_v2(dims, fingerprint_hash).await {
        handle.mark_unavailable();
        tracing::warn!(
            target: xai_grok_telemetry::memory_log::TARGET,
            error = %e,
            "milvus reconciliation failed: cannot ensure schema-v2 collection"
        );
        return Err(e);
    }

    // 2. Fetch existing (id -> hash) map from remote
    let remote_hashes = match handle.list_id_hashes_v2(fingerprint_hash).await {
        Ok(h) => h,
        Err(e) => {
            handle.mark_unavailable();
            tracing::warn!(
                target: xai_grok_telemetry::memory_log::TARGET,
                error = %e,
                "milvus reconciliation failed: cannot list remote id hashes"
            );
            return Err(e);
        }
    };

    // 3. Query all local chunks from SQLite
    let all_chunks = match index.all_chunks() {
        Ok(c) => c,
        Err(e) => {
            return Err(MirrorError::with_detail(
                MirrorErrorKind::SourceUnavailable,
                format!("query all chunks: {e}"),
                None,
            ));
        }
    };

    let total = all_chunks.len();
    let mut to_embed: Vec<index::ChunkRecord> = Vec::new();
    let mut unchanged = 0;
    let mut local_ids = std::collections::HashSet::with_capacity(total);

    for chunk in all_chunks {
        local_ids.insert(chunk.id.clone());
        if remote_hashes.get(&chunk.id).map(String::as_str) == Some(&chunk.hash) {
            unchanged += 1;
        } else {
            to_embed.push(chunk);
        }
    }

    // 4. Find vanished chunks to delete on remote
    let mut vanished_ids: Vec<String> = Vec::new();
    for remote_id in remote_hashes.keys() {
        if !local_ids.contains(remote_id.as_str()) {
            vanished_ids.push(remote_id.clone());
        }
    }

    if !vanished_ids.is_empty() {
        if let Err(e) = handle.delete_ids(&vanished_ids).await {
            handle.mark_unavailable();
            tracing::warn!(
                target: xai_grok_telemetry::memory_log::TARGET,
                error = %e,
                "milvus reconciliation failed: delete vanished ids failed"
            );
            return Err(e);
        }
    }

    // 5. Embed and upsert new or modified chunks in batches of 32
    let mut embedded = 0;
    for batch in to_embed.chunks(32) {
        let texts: Vec<&str> = batch.iter().map(|c| c.text.as_str()).collect();
        let embeddings = match provider.embed_batch(&texts).await {
            Ok(embs) => embs,
            Err(e) => {
                handle.mark_unavailable();
                tracing::warn!(
                    target: xai_grok_telemetry::memory_log::TARGET,
                    error = %e,
                    "milvus reconciliation failed: embedding batch failed"
                );
                return Err(MirrorError::with_detail(
                    MirrorErrorKind::Transient,
                    format!("embedding batch: {e}"),
                    None,
                ));
            }
        };

        if let Err(e) = crate::embedding::validate_embedding_batch(
            batch.len(),
            provider.dimensions(),
            &embeddings,
        ) {
            handle.mark_unavailable();
            return Err(MirrorError::with_detail(
                MirrorErrorKind::Malformed,
                format!("validate embedding batch: {e}"),
                None,
            ));
        }

        let rows: Vec<MemoryRow> = batch
            .iter()
            .zip(embeddings.into_iter())
            .map(|(chunk, vector)| MemoryRow {
                id: chunk.id.clone(),
                text: chunk.text.clone(),
                vector,
                fingerprint_hash: fingerprint_hash.to_owned(),
                hash: chunk.hash.clone(),
                source: chunk.source.clone(),
                path: chunk.path.clone(),
                created_at: chunk.created_at,
            })
            .collect();

        if let Err(e) = handle.upsert_rows_v2(&rows).await {
            handle.mark_unavailable();
            tracing::warn!(
                target: xai_grok_telemetry::memory_log::TARGET,
                error = %e,
                "milvus reconciliation failed: upsert_rows_v2 failed"
            );
            return Err(e);
        }
        embedded += rows.len();
    }

    // 6. Record installed fingerprint in SQLite meta table and mark handle ready
    let _ = index.record_installed_fingerprint(fingerprint_hash);
    handle.mark_ready(fingerprint_hash, dims, total as u64);

    tracing::info!(
        target: xai_grok_telemetry::memory_log::TARGET,
        unchanged,
        embedded,
        deleted = vanished_ids.len(),
        total,
        "milvus mode reconciled successfully"
    );

    Ok(MilvusReconcileReport {
        unchanged,
        embedded,
        deleted: vanished_ids.len(),
        total,
    })
}

/// Drains existing local chunks and embeddings from SQLite into a schema-v2
/// remote collection, then completes reconciliation for any chunks lacking
/// local embeddings. Non-destructive: local SQLite tables are preserved.
pub async fn drain_local_to_milvus(
    index: &mut MemoryIndex,
    provider: Option<&dyn embedding::EmbeddingProvider>,
    handle: &MirrorHandle,
    fingerprint_hash: &str,
    dims: u32,
) -> Result<MilvusReconcileReport, MirrorError> {
    // 1. Ensure remote schema-v2 collection exists and is compatible
    handle.ensure_collection_v2(dims, fingerprint_hash).await?;

    // 2. If vec_available and chunks_vec has rows, drain in batches of RESYNC_BATCH_ROWS
    let mut drained_ids = std::collections::HashSet::new();
    if index.vec_available() {
        let db_path = index.db_path();
        if let Ok(conn) = rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        ) {
            index::init_sqlite_vec();
            let mut cursor: Option<String> = None;
            loop {
                const SQL: &str = "SELECT c.id, c.text, c.hash, c.source, c.path, c.created_at, v.embedding \
                                   FROM chunks c \
                                   INNER JOIN chunks_vec v ON v.chunk_id = c.id \
                                   WHERE (?1 IS NULL OR c.id > ?1) \
                                   ORDER BY c.id \
                                   LIMIT ?2";
                let Ok(mut stmt) = conn.prepare(SQL) else {
                    break;
                };
                let Ok(rows) = stmt.query_map(rusqlite::params![cursor, mirror::RESYNC_BATCH_ROWS as i64], |row| {
                    let id: String = row.get(0)?;
                    let text: String = row.get(1)?;
                    let hash: String = row.get(2)?;
                    let source: String = row.get(3)?;
                    let path: String = row.get(4)?;
                    let created_at: i64 = row.get(5)?;
                    let blob: Vec<u8> = row.get(6)?;
                    let vector = mirror::decode_f32_le(&blob);
                    Ok(MemoryRow {
                        id,
                        text,
                        vector,
                        fingerprint_hash: fingerprint_hash.to_owned(),
                        hash,
                        source,
                        path,
                        created_at,
                    })
                }) else {
                    break;
                };

                let batch: Vec<MemoryRow> = rows.filter_map(Result::ok).collect();
                if batch.is_empty() {
                    break;
                }
                for r in &batch {
                    drained_ids.insert(r.id.clone());
                }
                cursor = batch.last().map(|r| r.id.clone());
                handle.upsert_rows_v2(&batch).await?;
                if cursor.is_none() {
                    break;
                }
            }
        }
    }

    // 3. Reconcile remaining chunks or any diff
    if let Some(p) = provider {
        reconcile_milvus_mode(index, p, handle, fingerprint_hash).await
    } else {
        let total = index.all_chunks().map(|c| c.len()).unwrap_or(drained_ids.len());
        let _ = index.record_installed_fingerprint(fingerprint_hash);
        handle.mark_ready(fingerprint_hash, dims, total as u64);
        Ok(MilvusReconcileReport {
            unchanged: drained_ids.len(),
            embedded: 0,
            deleted: 0,
            total,
        })
    }
}
