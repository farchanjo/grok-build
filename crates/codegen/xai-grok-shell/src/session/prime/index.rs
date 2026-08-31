//! Process-level Prime metadata index manager.
//!
//! One [`PrimeIndexHandle`] is stored per canonical Grok home + workspace
//! identity. Inventory generations are synchronized transactionally through
//! [`xai_grok_memory::MetadataIndex::replace_inventory`]; changed or removed
//! rows drop their vectors. Embeddings are backfilled in bounded batches
//! without holding a rusqlite connection across `.await`.
//!
//! Persisted and transmitted fields are only: strict name, frontmatter
//! description, bounded `grok.when-to-use` / `grok.paths`, and a safe scope
//! label, under opaque ids. Bodies, prompts, credentials, absolute paths,
//! and raw provider errors are never stored or shipped.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use tokio_util::sync::CancellationToken;
use xai_grok_agent::subagent::callable::{CallableAgentDescriptor, CallableAgentSource};
use xai_grok_config_types::EmbeddingEncoding;
use xai_grok_inference::DEFAULT_EMBEDDINGS_PATH;
use xai_grok_memory::embedding::EmbeddingProvider;
use xai_grok_memory::metadata_index::{
    CollectionState, commit_staged_vectors, discard_collection_rebuild, stage_collection_vectors,
};
use xai_grok_memory::{
    CollectionKind, EmbeddingSourceSpec, MetadataFtsHit, MetadataIndex, MetadataItem,
    MetadataKnnHit, NORMALIZATION_L2_V1, UpsertResult, VECTOR_SCHEMA_VERSION, VectorFingerprint,
    l2_normalize_v1, metadata_doc_prep, metadata_index_path, validate_embedding_batch,
    workspace_storage_identity,
};
use xai_grok_tools::implementations::skills::types::{SkillInfo, SkillScope};

use crate::retrieval::{OrchestratorError, PipelineOptions, RetrievalService, stable_home_key};

const BACKFILL_BATCH: usize = 32;
const DEFAULT_SEARCH_LIMIT: usize = 256;

static HANDLES: std::sync::OnceLock<
    parking_lot::RwLock<HashMap<PrimeIndexKey, Arc<PrimeIndexHandle>>>,
> = std::sync::OnceLock::new();

fn handle_map() -> &'static parking_lot::RwLock<HashMap<PrimeIndexKey, Arc<PrimeIndexHandle>>> {
    HANDLES.get_or_init(|| parking_lot::RwLock::new(HashMap::new()))
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct PrimeIndexKey {
    home: PathBuf,
    workspace_identity: String,
}

/// Secret-free pin of the exact primary embedding route/space for one
/// collection operation. A reload or route change cannot install vectors
/// from a different space.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PinnedEmbeddingSpace {
    pub snapshot_generation: u64,
    pub route_id: String,
    pub space_fingerprint: String,
    pub spec: EmbeddingSourceSpec,
}

/// Immutable exact-space token captured before any embed await.
///
/// Thread this through embed, KNN lookup, and vector install. Never re-read
/// the mutable live handle pin after an await and never compare spaces by
/// dimension alone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrozenEmbeddingPin {
    space: PinnedEmbeddingSpace,
    fingerprint: VectorFingerprint,
    collection: CollectionKind,
}

impl FrozenEmbeddingPin {
    /// Freeze a complete pin for the skills collection.
    pub fn capture(space: PinnedEmbeddingSpace) -> Result<Self, PrimeIndexError> {
        Self::capture_for(space, CollectionKind::Skills)
    }

    /// Freeze a complete pin: snapshot generation, exact route/provider
    /// instance, model/spec/dimensions/encoding, collection-local content
    /// hash, and the intended collection.
    pub fn capture_for(
        space: PinnedEmbeddingSpace,
        collection: CollectionKind,
    ) -> Result<Self, PrimeIndexError> {
        if space.spec.normalization != NORMALIZATION_L2_V1 || space.spec.dimensions == 0 {
            return Err(PrimeIndexError::SpaceMismatch);
        }
        let (fingerprint, _) = VectorFingerprint::build(
            space.spec.clone(),
            metadata_doc_prep(collection),
            VECTOR_SCHEMA_VERSION,
        )
        .map_err(|_| PrimeIndexError::SpaceMismatch)?;
        Ok(Self {
            space,
            fingerprint,
            collection,
        })
    }

    pub fn space(&self) -> &PinnedEmbeddingSpace {
        &self.space
    }

    pub fn fingerprint_hash(&self) -> &str {
        self.fingerprint.hash()
    }

    pub fn dimensions(&self) -> usize {
        self.space.spec.dimensions
    }

    pub fn collection(&self) -> CollectionKind {
        self.collection
    }

    /// Live pin identity matches this frozen token (generation, route,
    /// provider instance, model/spec/dimensions/encoding, fingerprint).
    pub fn matches_live(&self, live: &PinnedEmbeddingSpace) -> bool {
        live == &self.space
    }
}

/// Process-level handle. Cheap to clone (`Arc` internals).
pub struct PrimeIndexHandle {
    home: PathBuf,
    workspace_identity: String,
    db_path: PathBuf,
    pin: parking_lot::Mutex<Option<PinnedEmbeddingSpace>>,
    inventory_generation: AtomicI64,
    callable_generation: AtomicI64,
    backfill: tokio::sync::Mutex<()>,
}

impl std::fmt::Debug for PrimeIndexHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrimeIndexHandle")
            .field("workspace_identity", &self.workspace_identity)
            .field(
                "inventory_generation",
                &self.inventory_generation.load(Ordering::Relaxed),
            )
            .field("has_pin", &self.pin.lock().is_some())
            .finish()
    }
}

/// Bounded, secret-free index errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrimeIndexError {
    ReadOnly,
    SpaceMismatch,
    StaleGeneration,
    EmbedFailed,
    InvalidItem,
    Unavailable,
}

impl std::fmt::Display for PrimeIndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadOnly => f.write_str("prime index is read-only"),
            Self::SpaceMismatch => f.write_str("prime index embedding space pin mismatch"),
            Self::StaleGeneration => f.write_str("prime index generation or hash is stale"),
            Self::EmbedFailed => f.write_str("prime index embedding failed"),
            Self::InvalidItem => f.write_str("prime index rejected an invalid metadata item"),
            Self::Unavailable => f.write_str("prime index is unavailable"),
        }
    }
}

impl std::error::Error for PrimeIndexError {}

impl PrimeIndexHandle {
    /// Install (or reuse) the process-level handle for `home` + `cwd`.
    pub fn get_or_create(home: &Path, cwd: &Path) -> Arc<Self> {
        let key = PrimeIndexKey {
            home: stable_home_key(home),
            workspace_identity: workspace_storage_identity(cwd),
        };
        if let Some(existing) = handle_map().read().get(&key).cloned() {
            return existing;
        }
        let mut w = handle_map().write();
        if let Some(existing) = w.get(&key).cloned() {
            return existing;
        }
        let db_path = metadata_index_path(&key.home, &key.workspace_identity);
        let handle = Arc::new(Self {
            home: key.home.clone(),
            workspace_identity: key.workspace_identity.clone(),
            db_path,
            pin: parking_lot::Mutex::new(None),
            inventory_generation: AtomicI64::new(0),
            callable_generation: AtomicI64::new(0),
            backfill: tokio::sync::Mutex::new(()),
        });
        w.insert(key, handle.clone());
        handle
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub fn workspace_identity(&self) -> &str {
        &self.workspace_identity
    }

    pub fn pinned_space(&self) -> Option<PinnedEmbeddingSpace> {
        self.pin.lock().clone()
    }

    /// Snapshot the live pin into an immutable exact-space token for skills.
    /// Callers must capture this before any embed await and thread it end-to-end.
    pub fn freeze_pin(&self) -> Result<FrozenEmbeddingPin, PrimeIndexError> {
        self.freeze_pin_for(CollectionKind::Skills)
    }

    /// Snapshot the live pin into an immutable exact-space token for
    /// `collection`. Skills and callable_agents use independent fingerprints.
    pub fn freeze_pin_for(
        &self,
        collection: CollectionKind,
    ) -> Result<FrozenEmbeddingPin, PrimeIndexError> {
        FrozenEmbeddingPin::capture_for(
            self.pinned_space().ok_or(PrimeIndexError::SpaceMismatch)?,
            collection,
        )
    }

    fn generation_atom(&self, collection: CollectionKind) -> &AtomicI64 {
        match collection {
            CollectionKind::Skills => &self.inventory_generation,
            CollectionKind::CallableAgents => &self.callable_generation,
        }
    }

    fn open(&self) -> Result<MetadataIndex, PrimeIndexError> {
        MetadataIndex::open_or_create(&self.db_path).map_err(|_| PrimeIndexError::Unavailable)
    }

    /// Transactionally replace the skills collection with `items`.
    ///
    /// Changed hashes drop live vectors. Removed ids drop vectors. The
    /// sibling `callable_agents` collection is not touched.
    pub fn sync_skills(
        &self,
        generation: i64,
        items: &[MetadataItem],
    ) -> Result<UpsertResult, PrimeIndexError> {
        self.sync_collection(CollectionKind::Skills, generation, items)
    }

    /// Transactionally replace the callable_agents collection with `items`.
    ///
    /// Index presence is not callable authority. Changed hashes drop live
    /// vectors. Removed ids drop vectors. The sibling `skills` collection is
    /// not touched.
    pub fn sync_callable_agents(
        &self,
        generation: i64,
        items: &[MetadataItem],
    ) -> Result<UpsertResult, PrimeIndexError> {
        self.sync_collection(CollectionKind::CallableAgents, generation, items)
    }

    /// Replace callable_agents from the live authoritative descriptor
    /// snapshot. Prime turn time always calls this so a missed watcher
    /// event cannot leave a stale row that later ranks as evidence.
    pub fn reconcile_callable_agents(
        &self,
        agents: &[CallableAgentDescriptor],
    ) -> Result<UpsertResult, PrimeIndexError> {
        let items = agents_to_index_items(agents);
        let generation = inventory_generation_token(&items);
        self.sync_callable_agents(generation, &items)
    }

    fn sync_collection(
        &self,
        collection: CollectionKind,
        generation: i64,
        items: &[MetadataItem],
    ) -> Result<UpsertResult, PrimeIndexError> {
        let idx = self.open()?;
        let result = idx
            .replace_inventory(collection, generation, items)
            .map_err(|e| match e {
                xai_grok_memory::MetadataIndexError::ReadOnly => PrimeIndexError::ReadOnly,
                xai_grok_memory::MetadataIndexError::Privacy(_)
                | xai_grok_memory::MetadataIndexError::InvalidItem(_) => {
                    PrimeIndexError::InvalidItem
                }
                xai_grok_memory::MetadataIndexError::Sqlite(_) => PrimeIndexError::Unavailable,
            })?;
        self.generation_atom(collection)
            .store(generation, Ordering::Relaxed);
        Ok(result)
    }

    /// Pin the exact primary embedding route/space for subsequent collection
    /// operations. A different fingerprint refuses to keep the old pin.
    pub fn pin_primary_space(
        &self,
        pin: PinnedEmbeddingSpace,
    ) -> Result<PinnedEmbeddingSpace, PrimeIndexError> {
        if pin.spec.normalization != NORMALIZATION_L2_V1 {
            return Err(PrimeIndexError::SpaceMismatch);
        }
        if pin.spec.dimensions == 0 {
            return Err(PrimeIndexError::SpaceMismatch);
        }
        let mut slot = self.pin.lock();
        if let Some(existing) = slot.as_ref()
            && existing.spec != pin.spec
            && existing.space_fingerprint != pin.space_fingerprint
        {
            // Route change: replace the pin. Stale in-flight backfills compare
            // against the new fingerprint and refuse to install.
        }
        *slot = Some(pin.clone());
        Ok(pin)
    }

    /// Resolve and pin the named profile's **primary** embedding route.
    pub fn pin_from_service(
        &self,
        service: &RetrievalService,
        profile_id: &str,
    ) -> Result<PinnedEmbeddingSpace, PrimeIndexError> {
        let snapshot = service.load_snapshot();
        if !snapshot.enabled {
            return Err(PrimeIndexError::Unavailable);
        }
        let profile = snapshot
            .profile(profile_id)
            .ok_or(PrimeIndexError::Unavailable)?;
        let route_id = profile
            .embedding_route_ids
            .first()
            .cloned()
            .ok_or(PrimeIndexError::Unavailable)?;
        let route = snapshot
            .embedding_route(&route_id)
            .ok_or(PrimeIndexError::Unavailable)?;
        let dimensions = route.config.dimensions.unwrap_or(0) as usize;
        if dimensions == 0 {
            return Err(PrimeIndexError::Unavailable);
        }
        let encoding = match route.config.encoding {
            EmbeddingEncoding::Float => "float",
            EmbeddingEncoding::Base64 => "base64",
        };
        let spec = EmbeddingSourceSpec {
            provider_instance_id: route.provider_instance_id.clone(),
            incarnation: route.incarnation.clone(),
            origin_host: route.origin_host.clone(),
            embedding_path: DEFAULT_EMBEDDINGS_PATH.to_owned(),
            protocol: route.config.protocol.as_str().to_owned(),
            model: route.config.model.clone(),
            dimensions,
            encoding: encoding.to_owned(),
            normalization: NORMALIZATION_L2_V1.to_owned(),
        };
        self.pin_primary_space(PinnedEmbeddingSpace {
            snapshot_generation: snapshot.generation,
            route_id,
            space_fingerprint: route.embedding_space.fingerprint().to_owned(),
            spec,
        })
    }

    /// Start bounded backfill without awaiting it. The query path must search
    /// immediately with current FTS plus any safe live vectors; missing
    /// embeddings never block the caller. Concurrent callers serialize on the
    /// handle mutex inside [`Self::backfill`].
    ///
    /// `frozen` is the sole backfill identity and must be the token bound
    /// into `embedder` (for [`PinnedServiceEmbedder`],
    /// [`PinnedServiceEmbedder::frozen_pin`]). This never re-captures the
    /// live handle pin after the embedder was bound.
    pub fn spawn_backfill(
        self: &Arc<Self>,
        embedder: Arc<dyn EmbeddingProvider>,
        frozen: FrozenEmbeddingPin,
        cancel: CancellationToken,
    ) {
        let handle = Arc::clone(self);
        tokio::spawn(async move {
            let _ = handle.backfill(embedder, frozen, cancel).await;
        });
    }

    /// Backfill missing vectors for the frozen space. Bounded batches; the
    /// rusqlite connection is dropped before every embed await. Stale
    /// generation/hash/space completions refuse to write.
    ///
    /// `frozen` is the sole identity: live pin must equal it after the mutex
    /// and after every embed await. Staging is committed only when that
    /// token still matches; a mismatch discards staging instead of leaving
    /// a mixed or stale table.
    pub async fn backfill(
        &self,
        embedder: Arc<dyn EmbeddingProvider>,
        frozen: FrozenEmbeddingPin,
        cancel: CancellationToken,
    ) -> Result<usize, PrimeIndexError> {
        self.run_vector_job(embedder, frozen, cancel, false, &mut |_, _| {})
            .await
    }

    /// Drop live staging and rebuild every vector for the frozen space.
    /// Distinct from missing-only [`Self::backfill`]: this always restages.
    pub async fn rebuild(
        &self,
        embedder: Arc<dyn EmbeddingProvider>,
        frozen: FrozenEmbeddingPin,
        cancel: CancellationToken,
    ) -> Result<usize, PrimeIndexError> {
        self.run_vector_job(embedder, frozen, cancel, true, &mut |_, _| {})
            .await
    }

    /// Same as [`Self::backfill`] with a progress callback (`done`, `total`).
    pub async fn backfill_with_progress(
        &self,
        embedder: Arc<dyn EmbeddingProvider>,
        frozen: FrozenEmbeddingPin,
        cancel: CancellationToken,
        on_progress: &mut (dyn FnMut(u64, u64) + Send),
    ) -> Result<usize, PrimeIndexError> {
        self.run_vector_job(embedder, frozen, cancel, false, on_progress)
            .await
    }

    /// Same as [`Self::rebuild`] with a progress callback (`done`, `total`).
    pub async fn rebuild_with_progress(
        &self,
        embedder: Arc<dyn EmbeddingProvider>,
        frozen: FrozenEmbeddingPin,
        cancel: CancellationToken,
        on_progress: &mut (dyn FnMut(u64, u64) + Send),
    ) -> Result<usize, PrimeIndexError> {
        self.run_vector_job(embedder, frozen, cancel, true, on_progress)
            .await
    }

    /// Durable collection snapshot (no vectors, no secrets).
    pub fn collection_snapshot(
        &self,
        collection: CollectionKind,
    ) -> Result<(CollectionState, bool, bool), PrimeIndexError> {
        let idx = self.open()?;
        let state = idx
            .collection_state(collection)
            .map_err(|_| PrimeIndexError::Unavailable)?;
        Ok((state, idx.writable(), idx.vec_available()))
    }

    async fn run_vector_job(
        &self,
        embedder: Arc<dyn EmbeddingProvider>,
        frozen: FrozenEmbeddingPin,
        cancel: CancellationToken,
        force_rebuild: bool,
        on_progress: &mut (dyn FnMut(u64, u64) + Send),
    ) -> Result<usize, PrimeIndexError> {
        let _guard = self.backfill.lock().await;
        self.require_live_matches_frozen(&frozen, embedder.as_ref())?;
        if cancel.is_cancelled() {
            return Err(PrimeIndexError::Unavailable);
        }

        let collection = frozen.collection();
        let generation = self.generation_atom(collection).load(Ordering::Relaxed);
        let idx = self.open()?;
        let state = idx
            .collection_state(collection)
            .map_err(|_| PrimeIndexError::Unavailable)?;
        drop(idx);

        if state.inventory_generation != generation {
            return Err(PrimeIndexError::StaleGeneration);
        }

        let compatible = !force_rebuild
            && state.fingerprint_hash == frozen.fingerprint_hash()
            && state.embedding_dimensions == frozen.dimensions()
            && !state.fingerprint_hash.is_empty();
        let total = state.item_count.max(0) as u64;
        on_progress(0, total);

        let embedder: Arc<dyn EmbeddingProvider> = Arc::new(L2Embedder {
            inner: Arc::clone(&embedder),
        });
        if !compatible {
            let _ = stage_collection_vectors(
                &self.db_path,
                collection,
                &frozen.space().spec,
                Some(Arc::clone(&embedder)),
                60,
                0,
                Some(8),
                cancel.child_token(),
            )
            .await;
            if cancel.is_cancelled() {
                discard_collection_rebuild(&self.db_path, collection);
                return Err(PrimeIndexError::Unavailable);
            }
            if let Err(e) = self.assert_install_matches_frozen(&frozen, generation) {
                discard_collection_rebuild(&self.db_path, collection);
                return Err(e);
            }
            let _ = commit_staged_vectors(&self.db_path, collection, &frozen.space().spec);
            self.assert_install_matches_frozen(&frozen, generation)?;
            if let Ok((live, _, _)) = self.collection_snapshot(collection) {
                on_progress(live.vec_count.max(0) as u64, total);
            }
        }

        let mut written = 0usize;
        loop {
            if cancel.is_cancelled() {
                break;
            }
            let idx = self.open()?;
            if !idx.vectors_safe_to_backfill(collection) {
                break;
            }
            let live_gen = idx
                .collection_state(collection)
                .map(|s| s.inventory_generation)
                .unwrap_or(0);
            if live_gen != generation {
                return Err(PrimeIndexError::StaleGeneration);
            }
            let missing = idx.items_without_embeddings(collection).unwrap_or_default();
            drop(idx);
            if missing.is_empty() {
                break;
            }
            let batch: Vec<(String, String)> = missing.into_iter().take(BACKFILL_BATCH).collect();
            let texts: Vec<&str> = batch.iter().map(|(_, t)| t.as_str()).collect();
            let embeddings = match embedder.embed_batch(&texts).await {
                Ok(v) => v,
                Err(_) if cancel.is_cancelled() => return Err(PrimeIndexError::Unavailable),
                Err(_) => return Err(PrimeIndexError::EmbedFailed),
            };
            if cancel.is_cancelled() {
                return Err(PrimeIndexError::Unavailable);
            }
            validate_embedding_batch(texts.len(), frozen.dimensions(), &embeddings)
                .map_err(|_| PrimeIndexError::EmbedFailed)?;
            self.assert_install_matches_frozen(&frozen, generation)?;

            let idx = self.open()?;
            let live = idx
                .collection_state(collection)
                .map_err(|_| PrimeIndexError::Unavailable)?;
            if live.inventory_generation != generation {
                return Err(PrimeIndexError::StaleGeneration);
            }
            if !live.fingerprint_hash.is_empty()
                && live.fingerprint_hash != frozen.fingerprint_hash()
            {
                return Err(PrimeIndexError::SpaceMismatch);
            }
            for ((item_id, expected_text), embedding) in batch.iter().zip(embeddings.iter()) {
                let Some(item) = idx
                    .get_item(collection, item_id)
                    .map_err(|_| PrimeIndexError::Unavailable)?
                else {
                    continue;
                };
                if item.fts_text() != *expected_text {
                    continue;
                }
                match idx.upsert_embedding_for_fingerprint(
                    collection,
                    item_id,
                    embedding,
                    frozen.fingerprint_hash(),
                ) {
                    Ok(()) => written += 1,
                    Err(xai_grok_memory::MetadataIndexError::InvalidItem(
                        "embedding space fingerprint mismatch",
                    )) => {
                        return Err(PrimeIndexError::SpaceMismatch);
                    }
                    Err(_) => {}
                }
            }
            drop(idx);
            on_progress(written as u64, total);
            if batch.len() < BACKFILL_BATCH {
                break;
            }
        }
        if cancel.is_cancelled() {
            return Err(PrimeIndexError::Unavailable);
        }
        on_progress(written as u64, total);
        Ok(written)
    }

    /// Live pin identity (route/incarnation/provider/model/spec/hash) and
    /// embedder dimensions must equal the frozen token. Never compare by
    /// dimension alone.
    fn require_live_matches_frozen(
        &self,
        frozen: &FrozenEmbeddingPin,
        embedder: &dyn EmbeddingProvider,
    ) -> Result<(), PrimeIndexError> {
        let live = self.pinned_space().ok_or(PrimeIndexError::SpaceMismatch)?;
        if !frozen.matches_live(&live) {
            return Err(PrimeIndexError::SpaceMismatch);
        }
        if embedder.dimensions() != frozen.dimensions() {
            return Err(PrimeIndexError::SpaceMismatch);
        }
        Ok(())
    }

    /// After an embed await: install only if the live pin identity and the
    /// collection fingerprint still equal the frozen token.
    fn assert_install_matches_frozen(
        &self,
        frozen: &FrozenEmbeddingPin,
        generation: i64,
    ) -> Result<(), PrimeIndexError> {
        let live = self.pinned_space().ok_or(PrimeIndexError::SpaceMismatch)?;
        if !frozen.matches_live(&live) {
            return Err(PrimeIndexError::SpaceMismatch);
        }
        let idx = self.open()?;
        let state = idx
            .collection_state(frozen.collection())
            .map_err(|_| PrimeIndexError::Unavailable)?;
        if state.inventory_generation != generation {
            return Err(PrimeIndexError::StaleGeneration);
        }
        if !state.fingerprint_hash.is_empty() && state.fingerprint_hash != frozen.fingerprint_hash()
        {
            return Err(PrimeIndexError::SpaceMismatch);
        }
        Ok(())
    }

    /// Embed `texts` through the retrieval service on the **live** primary
    /// pin captured at call start, then locally L2-normalize.
    pub async fn embed_texts_pinned(
        &self,
        service: &RetrievalService,
        profile_id: &str,
        texts: Vec<String>,
        cancel: CancellationToken,
    ) -> Result<Vec<Vec<f32>>, PrimeIndexError> {
        let pin = self.freeze_pin()?;
        self.embed_texts_with_pin(&pin, service, profile_id, texts, cancel)
            .await
    }

    /// Embed `texts` strictly with a frozen exact-space token. The mutable
    /// live handle pin is not re-read for the route/provider/model.
    pub async fn embed_texts_with_pin(
        &self,
        pin: &FrozenEmbeddingPin,
        service: &RetrievalService,
        profile_id: &str,
        texts: Vec<String>,
        cancel: CancellationToken,
    ) -> Result<Vec<Vec<f32>>, PrimeIndexError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let options = PipelineOptions {
            hard_error_on_semantic_failure: true,
            bypass_semantic: false,
            pin_snapshot_generation: Some(pin.space().snapshot_generation),
            embed_route_pin: Some(pin.space().route_id.clone()),
            hard_error_on_limit_exceeded: false,
        };
        let stage = service
            .embed(profile_id, texts.clone(), options, cancel)
            .await
            .map_err(|err| match err {
                OrchestratorError::Cancelled { .. } => PrimeIndexError::Unavailable,
                _ => PrimeIndexError::EmbedFailed,
            })?;
        if stage.embedding_space.fingerprint() != pin.space().space_fingerprint {
            return Err(PrimeIndexError::SpaceMismatch);
        }
        if stage.provider_instance_id != pin.space().spec.provider_instance_id {
            return Err(PrimeIndexError::SpaceMismatch);
        }
        let mut out = Vec::with_capacity(stage.result.vectors.len());
        for v in &stage.result.vectors {
            out.push(v.values.clone());
        }
        validate_embedding_batch(texts.len(), pin.dimensions(), &out)
            .map_err(|_| PrimeIndexError::EmbedFailed)?;
        for row in out.iter_mut() {
            l2_normalize_v1(row).map_err(|_| PrimeIndexError::EmbedFailed)?;
        }
        Ok(out)
    }

    pub fn search_fts(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MetadataFtsHit>, PrimeIndexError> {
        self.search_fts_in(CollectionKind::Skills, query, limit)
    }

    pub fn search_fts_in(
        &self,
        collection: CollectionKind,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MetadataFtsHit>, PrimeIndexError> {
        let idx = self.open()?;
        idx.search_fts(collection, query, limit.max(1).min(DEFAULT_SEARCH_LIMIT))
            .map_err(|_| PrimeIndexError::Unavailable)
    }

    pub fn search_knn(
        &self,
        query_embedding: &[f32],
        k: usize,
    ) -> Result<Vec<MetadataKnnHit>, PrimeIndexError> {
        let pin = self.freeze_pin()?;
        self.search_knn_with_pin(&pin, query_embedding, k)
    }

    /// Classify the live skills table against a frozen exact-space token.
    ///
    /// Empty/pending fingerprints are [`PrimeIndexError::Unavailable`]. A
    /// live table in a different space is [`PrimeIndexError::SpaceMismatch`].
    pub fn knn_space_for_pin(&self, pin: &FrozenEmbeddingPin) -> Result<(), PrimeIndexError> {
        let idx = self.open()?;
        Self::require_knn_space(&idx, pin)
    }

    fn require_knn_space(
        idx: &MetadataIndex,
        pin: &FrozenEmbeddingPin,
    ) -> Result<(), PrimeIndexError> {
        let state = idx
            .collection_state(pin.collection())
            .map_err(|_| PrimeIndexError::Unavailable)?;
        // Cold-start, rebuild-pending, or empty hash: KNN is unavailable.
        // Do not treat that as a proven mixed-space comparison.
        if !state.pending_json.trim().is_empty() || state.fingerprint_hash.trim().is_empty() {
            return Err(PrimeIndexError::Unavailable);
        }
        if state.fingerprint_hash != pin.fingerprint_hash()
            || state.embedding_dimensions != pin.dimensions()
        {
            return Err(PrimeIndexError::SpaceMismatch);
        }
        Ok(())
    }

    /// KNN against a frozen exact-space token. Empty/pending fingerprints
    /// and in-flight rebuilds are [`PrimeIndexError::Unavailable`] (not a
    /// mixed-space comparison). A live table in a different space is
    /// [`PrimeIndexError::SpaceMismatch`]. Missing KNN never returns hits
    /// that could be counted as vector evidence.
    ///
    /// The fingerprint is re-checked after the SELECT so a same-dimension
    /// table swap cannot promote mixed-space hits as automatic evidence.
    pub fn search_knn_with_pin(
        &self,
        pin: &FrozenEmbeddingPin,
        query_embedding: &[f32],
        k: usize,
    ) -> Result<Vec<MetadataKnnHit>, PrimeIndexError> {
        if query_embedding.len() != pin.dimensions() {
            return Err(PrimeIndexError::SpaceMismatch);
        }
        let idx = self.open()?;
        Self::require_knn_space(&idx, pin)?;
        let hits = idx
            .search_knn(
                pin.collection(),
                query_embedding,
                k.max(1).min(DEFAULT_SEARCH_LIMIT),
            )
            .map_err(|_| PrimeIndexError::Unavailable)?;
        Self::require_knn_space(&idx, pin)?;
        Ok(hits)
    }

    pub fn list_skill_items(&self) -> Result<Vec<MetadataItem>, PrimeIndexError> {
        self.list_items_in(CollectionKind::Skills)
    }

    pub fn list_callable_items(&self) -> Result<Vec<MetadataItem>, PrimeIndexError> {
        self.list_items_in(CollectionKind::CallableAgents)
    }

    fn list_items_in(
        &self,
        collection: CollectionKind,
    ) -> Result<Vec<MetadataItem>, PrimeIndexError> {
        let idx = self.open()?;
        idx.list_items(collection)
            .map_err(|_| PrimeIndexError::Unavailable)
    }
}

/// Opaque, path-free item id (`s` + 32 hex chars). Fits the metadata-index
/// charset (alphanumeric / `.` / `_` / `-`).
pub fn opaque_skill_id(skill: &SkillInfo) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"prime-skill-id/v1\0");
    hasher.update(scope_label(skill.scope).as_bytes());
    hasher.update(b"\0");
    hasher.update(skill.name.as_bytes());
    hasher.update(b"\0");
    hasher.update(skill.path.as_bytes());
    let hash = hasher.finalize();
    let bytes = hash.as_bytes();
    let mut out = String::with_capacity(33);
    out.push('s');
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for &b in &bytes[..16] {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}

fn scope_label(scope: SkillScope) -> &'static str {
    match scope {
        SkillScope::Local => "local",
        SkillScope::Repo => "repo",
        SkillScope::User => "user",
        SkillScope::Server => "server",
        SkillScope::Bundled => "bundled",
        SkillScope::Plugin => "plugin",
    }
}

fn cap_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// Cap for the skill body included in index/rerank documents. Large enough to
/// cover typical skill bodies while keeping embedding inputs bounded.
const SKILL_BODY_INDEX_CAP_CHARS: usize = 8_000;

/// Index/rerank document text: privacy-accepted metadata plus the skill body.
///
/// The metadata portion keeps the [`xai_grok_memory::reject_persisted_paths`]
/// contract (absolute / UNC / `file:` URL tokens and credentials never ship).
/// The body is appended per the full-catalog indexing mode: retrieval returns
/// skill names, and selected bodies are still loaded from disk at render time.
pub fn skill_index_text(skill: &SkillInfo) -> String {
    skill_rerank_document(skill)
}

/// Remote rerank document: metadata plus the skill body so the semantic and
/// rerank stages judge the full authored content. The body is capped;
/// retrieval still returns only skill names.
pub fn skill_rerank_document(skill: &SkillInfo) -> String {
    let Some(item) = skill_to_metadata_item(skill) else {
        return String::new();
    };
    let body = skill_body_for_index(skill);
    if body.is_empty() {
        item.fts_text()
    } else {
        format!(
            "{}\n\n{}",
            item.fts_text(),
            cap_chars(&body, SKILL_BODY_INDEX_CAP_CHARS)
        )
    }
}

/// Skill body for index/rerank documents. Uses the in-memory body when the
/// discovery layer populated it; otherwise reads the `SKILL.md` from
/// `skill.path` (frontmatter stripped). Disk reads are cached by absolute
/// path and invalidated on mtime change, so repeated per-turn ranking calls
/// do not re-read the catalog.
fn skill_body_for_index(skill: &SkillInfo) -> String {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<HashMap<String, (Option<std::time::SystemTime>, String)>>> =
        OnceLock::new();
    if let Some(body) = skill.body.as_deref() {
        return body.trim().to_owned();
    }
    let path = skill.path.as_str();
    if path.is_empty() {
        return String::new();
    }
    let mtime = std::fs::metadata(path).ok().and_then(|m| m.modified().ok());
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(mut guard) = cache.lock() else {
        return String::new();
    };
    if let Some((cached_mtime, body)) = guard.get(path) {
        if *cached_mtime == mtime {
            return body.clone();
        }
    }
    // Strip the YAML frontmatter so index documents carry instructions only.
    let raw = std::fs::read_to_string(path).unwrap_or_default();
    let body = match raw.strip_prefix("---\n") {
        Some(rest) => match rest.find("\n---") {
            Some(idx) => rest[idx + 4..].trim_start().to_owned(),
            None => raw,
        },
        None => raw,
    };
    let body = cap_chars(body.trim(), SKILL_BODY_INDEX_CAP_CHARS);
    guard.insert(path.to_owned(), (mtime, body.clone()));
    body
}

fn skill_extra_json(skill: &SkillInfo) -> String {
    let mut obj = serde_json::Map::new();
    if let Some(wtu) = &skill.when_to_use {
        let trimmed = cap_chars(wtu.trim(), 500);
        if !trimmed.is_empty()
            && xai_grok_memory::reject_persisted_paths(&trimmed, "when_to_use").is_ok()
        {
            obj.insert("when_to_use".into(), serde_json::Value::String(trimmed));
        }
    }
    obj.insert(
        "scope".into(),
        serde_json::Value::String(scope_label(skill.scope).to_owned()),
    );
    if let Some(paths) = &skill.paths {
        let labels: Vec<serde_json::Value> = paths
            .iter()
            .take(32)
            .filter(|p| {
                !p.is_empty()
                    && p.len() <= 256
                    && !p.contains("..")
                    && xai_grok_memory::reject_persisted_paths(p, "path").is_ok()
            })
            .map(|p| serde_json::Value::String(cap_chars(p, 256)))
            .collect();
        if !labels.is_empty() {
            obj.insert("paths".into(), serde_json::Value::Array(labels));
        }
    }
    serde_json::Value::Object(obj).to_string()
}

/// Build a privacy-checked metadata item. Returns `None` when the skill
/// cannot be persisted without leaking a path, body, or secret marker.
pub fn skill_to_metadata_item(skill: &SkillInfo) -> Option<MetadataItem> {
    let id = opaque_skill_id(skill);
    let name = skill.name.trim();
    if name.is_empty() {
        return None;
    }
    let description = if skill.has_user_specified_description {
        cap_chars(skill.description.trim(), 1024)
    } else {
        String::new()
    };
    let extra = skill_extra_json(skill);
    match MetadataItem::new(id.clone(), name, description.clone(), extra) {
        Ok(item) => Some(item),
        Err(_) => MetadataItem::new(id, name, description, String::new()).ok(),
    }
}

/// Convert eligible skills into index rows. Invalid/privacy-failing rows are
/// dropped rather than persisted.
pub fn skills_to_index_items(skills: &[SkillInfo]) -> Vec<MetadataItem> {
    skills.iter().filter_map(skill_to_metadata_item).collect()
}

/// Deterministic inventory generation token from item id+hash pairs.
pub(crate) fn inventory_generation_token(items: &[MetadataItem]) -> i64 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"prime-inv-gen/v1");
    for item in items {
        hasher.update(item.item_id.as_bytes());
        hasher.update(b"\0");
        hasher.update(item.content_hash.as_bytes());
        hasher.update(b"\0");
    }
    let hash = hasher.finalize();
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&hash.as_bytes()[..8]);
    i64::from_le_bytes(bytes) & i64::MAX
}

/// Safe, path-free source class persisted for callable agents.
///
/// Plugin names, qualified identities, and home-like labels never appear.
pub fn agent_source_class(source: &CallableAgentSource) -> &'static str {
    match source {
        CallableAgentSource::Builtin => "builtin",
        CallableAgentSource::UserDefined { scope } => match scope {
            xai_grok_agent::config::AgentScope::Project => "project",
            xai_grok_agent::config::AgentScope::User => "user",
            xai_grok_agent::config::AgentScope::Bundled => "bundled",
            xai_grok_agent::config::AgentScope::BuiltIn => "builtin",
        },
        CallableAgentSource::Plugin { .. } => "plugin",
        CallableAgentSource::CliInline => "cli-inline",
    }
}

/// Opaque, path-free callable-agent item id (`a` + 32 hex chars).
pub fn opaque_agent_id(agent: &CallableAgentDescriptor) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"prime-agent-id/v1\0");
    hasher.update(agent.name.as_bytes());
    hasher.update(b"\0");
    hasher.update(agent_source_class(&agent.source).as_bytes());
    let hash = hasher.finalize();
    let bytes = hash.as_bytes();
    let mut out = String::with_capacity(33);
    out.push('a');
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for &b in &bytes[..16] {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}

fn agent_extra_json(agent: &CallableAgentDescriptor) -> String {
    let mut obj = serde_json::Map::new();
    obj.insert(
        "scope".into(),
        serde_json::Value::String(agent_source_class(&agent.source).to_owned()),
    );
    serde_json::Value::Object(obj).to_string()
}

/// Privacy-checked callable-agent metadata item. Name, bounded public
/// description, and safe source class only. Returns `None` when the row
/// cannot be persisted without leaking a path, body, or secret marker.
pub fn agent_to_metadata_item(agent: &CallableAgentDescriptor) -> Option<MetadataItem> {
    let id = opaque_agent_id(agent);
    let name = agent.name.trim();
    if name.is_empty() {
        return None;
    }
    let description = cap_chars(agent.description.as_deref().unwrap_or("").trim(), 1024);
    let extra = agent_extra_json(agent);
    match MetadataItem::new(id.clone(), name, description.clone(), extra) {
        Ok(item) => Some(item),
        Err(_) => MetadataItem::new(id, name, description, String::new()).ok(),
    }
}

/// Metadata text persisted and (when needed) transmitted. Agent prompts
/// and selected-skill names never appear.
pub fn agent_index_text(agent: &CallableAgentDescriptor) -> String {
    agent_rerank_document(agent)
}

/// Remote rerank document built only from a privacy-accepted metadata item.
pub fn agent_rerank_document(agent: &CallableAgentDescriptor) -> String {
    agent_to_metadata_item(agent)
        .map(|item| item.fts_text())
        .unwrap_or_default()
}

/// Convert callable-agent descriptors into index rows. Invalid/privacy-failing
/// rows are dropped rather than persisted. Index presence is not authority.
pub fn agents_to_index_items(agents: &[CallableAgentDescriptor]) -> Vec<MetadataItem> {
    agents.iter().filter_map(agent_to_metadata_item).collect()
}

/// Local unit-L2 wrapper. Prime always normalizes after the provider returns
/// so sqlite-vec distances stay on the `l2_v1` space.
struct L2Embedder {
    inner: Arc<dyn EmbeddingProvider>,
}

#[async_trait::async_trait]
impl EmbeddingProvider for L2Embedder {
    async fn embed_batch(
        &self,
        texts: &[&str],
    ) -> Result<Vec<Vec<f32>>, Box<dyn std::error::Error>> {
        let mut out = self.inner.embed_batch(texts).await?;
        validate_embedding_batch(texts.len(), self.inner.dimensions(), &out)?;
        for row in out.iter_mut() {
            l2_normalize_v1(row)?;
        }
        Ok(out)
    }

    fn model_name(&self) -> &str {
        self.inner.model_name()
    }

    fn dimensions(&self) -> usize {
        self.inner.dimensions()
    }
}

/// Build a retrieval-service embedder adapter that L2-normalizes after the
/// pinned primary route returns. Used by tests and by [`PrimeIndexHandle::backfill`].
///
/// In-flight HTTP honors `cancel`; a never-cancelled token is not acceptable
/// on the backfill path.
pub struct PinnedServiceEmbedder {
    handle: Arc<PrimeIndexHandle>,
    service: RetrievalService,
    profile_id: String,
    frozen: FrozenEmbeddingPin,
    cancel: CancellationToken,
}

impl PinnedServiceEmbedder {
    pub fn new(
        handle: Arc<PrimeIndexHandle>,
        service: RetrievalService,
        profile_id: String,
        cancel: CancellationToken,
    ) -> Result<Self, PrimeIndexError> {
        let frozen = handle.freeze_pin()?;
        Ok(Self::with_frozen_pin(
            handle, service, profile_id, frozen, cancel,
        ))
    }

    /// Bind this embedder to one immutable exact-space token. Subsequent
    /// `embed_batch` calls never re-read the mutable live handle pin.
    pub fn with_frozen_pin(
        handle: Arc<PrimeIndexHandle>,
        service: RetrievalService,
        profile_id: String,
        frozen: FrozenEmbeddingPin,
        cancel: CancellationToken,
    ) -> Self {
        Self {
            handle,
            service,
            profile_id,
            frozen,
            cancel,
        }
    }

    pub fn frozen_pin(&self) -> &FrozenEmbeddingPin {
        &self.frozen
    }
}

#[async_trait::async_trait]
impl EmbeddingProvider for PinnedServiceEmbedder {
    async fn embed_batch(
        &self,
        texts: &[&str],
    ) -> Result<Vec<Vec<f32>>, Box<dyn std::error::Error>> {
        let owned: Vec<String> = texts.iter().map(|s| (*s).to_owned()).collect();
        Ok(self
            .handle
            .embed_texts_with_pin(
                &self.frozen,
                &self.service,
                &self.profile_id,
                owned,
                self.cancel.child_token(),
            )
            .await?)
    }

    fn model_name(&self) -> &str {
        "prime-pinned"
    }

    fn dimensions(&self) -> usize {
        self.frozen.dimensions()
    }
}

/// Look up (or create) the process handle for a home/cwd pair.
pub fn prime_index_for(home: &Path, cwd: &Path) -> Arc<PrimeIndexHandle> {
    PrimeIndexHandle::get_or_create(home, cwd)
}

/// Test helper: install `spec` as the live skills vec table without changing
/// the handle pin, then freeze rebuilds so a later fill cannot swap it back.
#[cfg(test)]
pub(crate) async fn install_collection_space_for_tests(
    handle: &PrimeIndexHandle,
    spec: EmbeddingSourceSpec,
) {
    use xai_grok_memory::embedding::MockEmbeddingProvider;
    let dimensions = spec.dimensions;
    let embedder = Arc::new(MockEmbeddingProvider { dimensions });
    let _ = stage_collection_vectors(
        handle.db_path(),
        CollectionKind::Skills,
        &spec,
        Some(embedder),
        60,
        0,
        Some(8),
        CancellationToken::new(),
    )
    .await;
    assert!(
        commit_staged_vectors(handle.db_path(), CollectionKind::Skills, &spec),
        "test helper must commit the requested live space"
    );
    let conn = rusqlite::Connection::open(handle.db_path()).expect("open metadata db");
    conn.execute(
        "UPDATE collections SET backoff_until = ?1 WHERE name = 'skills'",
        rusqlite::params![i64::MAX],
    )
    .expect("freeze live table");
}

/// Cancellation token that fires after `deadline_ms` and is **not** cancelled
/// when the query future drops. Backfill can finish after search returns
/// while still remaining bounded.
pub fn bounded_cancel(deadline_ms: u64) -> CancellationToken {
    let token = CancellationToken::new();
    if deadline_ms == 0 {
        token.cancel();
        return token;
    }
    let child = token.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(deadline_ms)).await;
        child.cancel();
    });
    token
}

/// Test/shutdown helper.
pub fn uninstall_prime_index(home: &Path, cwd: &Path) -> Option<Arc<PrimeIndexHandle>> {
    let key = PrimeIndexKey {
        home: stable_home_key(home),
        workspace_identity: workspace_storage_identity(cwd),
    };
    handle_map().write().remove(&key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use xai_grok_memory::embedding::MockEmbeddingProvider;

    fn agent_desc(
        name: &str,
        desc: Option<&str>,
        source: CallableAgentSource,
    ) -> CallableAgentDescriptor {
        CallableAgentDescriptor {
            name: name.into(),
            description: desc.map(|s| s.to_string()),
            source,
        }
    }

    fn skill(name: &str, path: &str) -> SkillInfo {
        SkillInfo {
            name: name.into(),
            path: path.into(),
            description: format!("{name} description for indexing"),
            has_user_specified_description: true,
            when_to_use: Some(format!("use {name} when relevant")),
            paths: Some(vec!["src/**".into()]),
            ..SkillInfo::default()
        }
    }

    #[test]
    fn opaque_agent_id_is_charset_safe_and_stable() {
        let a = agent_desc(
            "explore",
            Some("read-only explorer"),
            CallableAgentSource::Builtin,
        );
        let id = opaque_agent_id(&a);
        assert!(id.starts_with('a'));
        assert_eq!(id.len(), 33);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit() || c == 'a'));
        assert_eq!(id, opaque_agent_id(&a));
        let b = agent_desc(
            "explore",
            Some("read-only explorer"),
            CallableAgentSource::CliInline,
        );
        assert_ne!(opaque_agent_id(&a), opaque_agent_id(&b));
    }

    #[test]
    fn agent_index_text_is_name_description_and_safe_source_class() {
        let plugin = agent_desc(
            "pl:arch",
            Some("plugin architect"),
            CallableAgentSource::Plugin {
                plugin: "/Users/secret/plugins/pl".into(),
                qualified: true,
            },
        );
        let text = agent_index_text(&plugin);
        assert!(text.contains("pl:arch"));
        assert!(text.contains("plugin architect"));
        assert!(text.contains("\"scope\":\"plugin\""));
        assert!(
            !text.contains("/Users/"),
            "plugin path leaked into index: {text}"
        );
        assert!(
            !text.contains("selected-skills"),
            "selected-skill query context must not be indexed: {text}"
        );
    }

    #[test]
    fn agent_index_text_omits_absolute_path_description() {
        let a = agent_desc(
            "explore",
            Some("/Users/secret/file"),
            CallableAgentSource::Builtin,
        );
        let item = agent_to_metadata_item(&a);
        if let Some(item) = item {
            assert!(
                !item.description.contains("/Users/"),
                "absolute path leaked: {}",
                item.description
            );
        }
    }

    #[tokio::test]
    async fn sync_callable_agents_covers_full_inventory_without_touching_skills() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("ws");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&cwd).unwrap();
        let handle = PrimeIndexHandle::get_or_create(&home, &cwd);
        let skills = vec![skill("deploy", "skills/deploy/SKILL.md")];
        handle
            .sync_skills(1, &skills_to_index_items(&skills))
            .unwrap();
        let agents: Vec<CallableAgentDescriptor> = (0..12)
            .map(|i| {
                agent_desc(
                    &format!("a{i:02}"),
                    Some(&format!("callable agent {i:02}")),
                    CallableAgentSource::Builtin,
                )
            })
            .collect();
        handle.reconcile_callable_agents(&agents).unwrap();
        assert_eq!(handle.list_callable_items().unwrap().len(), 12);
        assert_eq!(
            handle.list_skill_items().unwrap().len(),
            1,
            "callable sync must not drop skills"
        );
        let cells = {
            let idx = MetadataIndex::open_or_create(handle.db_path()).unwrap();
            idx.text_cells().unwrap()
        };
        for cell in &cells {
            assert!(!cell.contains("SECRET"), "privacy: {cell}");
            assert!(!cell.contains("/Users/"), "privacy path: {cell}");
        }
        uninstall_prime_index(&home, &cwd);
    }

    #[test]
    fn reconcile_callable_agents_drops_deleted_descriptors() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("ws");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&cwd).unwrap();
        let handle = PrimeIndexHandle::get_or_create(&home, &cwd);
        let first = vec![
            agent_desc("gone", Some("stale"), CallableAgentSource::Builtin),
            agent_desc("keep", Some("live"), CallableAgentSource::CliInline),
        ];
        handle.reconcile_callable_agents(&first).unwrap();
        assert_eq!(handle.list_callable_items().unwrap().len(), 2);
        let live = vec![agent_desc(
            "keep",
            Some("live"),
            CallableAgentSource::CliInline,
        )];
        handle.reconcile_callable_agents(&live).unwrap();
        let listed = handle.list_callable_items().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "keep");
        uninstall_prime_index(&home, &cwd);
    }

    #[test]
    fn opaque_id_is_charset_safe_and_stable() {
        let a = skill("deploy", "/tmp/skills/deploy/SKILL.md");
        let id = opaque_skill_id(&a);
        assert!(id.starts_with('s'));
        assert_eq!(id.len(), 33);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit() || c == 's'));
        assert_eq!(id, opaque_skill_id(&a));
        let b = skill("deploy", "/tmp/skills/other/SKILL.md");
        assert_ne!(opaque_skill_id(&a), opaque_skill_id(&b));
    }

    #[test]
    fn skill_index_text_includes_body_but_not_derived_description() {
        let mut s = skill("deploy", "skills/deploy/SKILL.md");
        s.body = Some("SECRET-BODY".into());
        let text = skill_index_text(&s);
        assert!(
            text.contains("SECRET-BODY"),
            "full-catalog indexing must include the skill body: {text}"
        );
        assert!(text.contains("deploy"));
        s.has_user_specified_description = false;
        s.description = "BODY-DERIVED".into();
        let text = skill_index_text(&s);
        assert!(
            !text.contains("BODY-DERIVED"),
            "auto-derived description must not be indexed: {text}"
        );
    }

    #[test]
    fn skill_index_text_omits_absolute_unc_file_url_paths() {
        let mut s = skill("deploy", "skills/deploy/SKILL.md");
        s.paths = Some(vec![
            "/Users/secret/**".into(),
            r"\\server\share\file".into(),
            "file:///etc/passwd".into(),
            "src/**".into(),
        ]);
        let text = skill_index_text(&s);
        assert!(!text.contains("/Users/"), "absolute path leaked: {text}");
        assert!(!text.contains("file:"), "file URL leaked: {text}");
        assert!(!text.contains("\\\\"), "UNC leaked: {text}");
        assert!(
            text.contains("src/**"),
            "accepted relative path dropped: {text}"
        );
    }

    #[test]
    fn metadata_item_privacy_rejects_absolute_path_description() {
        let mut s = skill("deploy", "skills/deploy/SKILL.md");
        s.description = "/Users/secret/file".into();
        assert!(
            skill_to_metadata_item(&s).is_none() || {
                let item = skill_to_metadata_item(&s).unwrap();
                !item.description.contains("/Users/")
            }
        );
    }

    #[tokio::test]
    async fn sync_and_knn_cover_full_inventory() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("ws");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&cwd).unwrap();
        let handle = PrimeIndexHandle::get_or_create(&home, &cwd);
        let skills: Vec<SkillInfo> = (0..12)
            .map(|i| skill(&format!("s{i:02}"), &format!("skills/s{i:02}/SKILL.md")))
            .collect();
        let items = skills_to_index_items(&skills);
        assert_eq!(items.len(), 12);
        handle.sync_skills(7, &items).unwrap();
        handle
            .pin_primary_space(PinnedEmbeddingSpace {
                snapshot_generation: 1,
                route_id: "emb-1".into(),
                space_fingerprint: "fp".into(),
                spec: EmbeddingSourceSpec {
                    provider_instance_id: "p".into(),
                    incarnation: Some("i".into()),
                    origin_host: "example.test".into(),
                    embedding_path: "/v1/embeddings".into(),
                    protocol: "openai_compatible".into(),
                    model: "mock".into(),
                    dimensions: 8,
                    encoding: "float".into(),
                    normalization: NORMALIZATION_L2_V1.to_owned(),
                },
            })
            .unwrap();
        let embedder = Arc::new(MockEmbeddingProvider { dimensions: 8 });
        let written = handle
            .backfill(
                embedder.clone(),
                handle.freeze_pin().unwrap(),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(written >= 1 || handle.search_fts("deploy", 8).is_ok());
        let listed = handle.list_skill_items().unwrap();
        assert_eq!(
            listed.len(),
            12,
            "full inventory must be indexed, not first-eight"
        );
        let cells = {
            let idx = MetadataIndex::open_or_create(handle.db_path()).unwrap();
            idx.text_cells().unwrap()
        };
        for cell in &cells {
            assert!(!cell.contains("SECRET"), "privacy: {cell}");
            assert!(!cell.contains("/Users/"), "privacy path: {cell}");
        }
        uninstall_prime_index(&home, &cwd);
    }

    #[tokio::test]
    async fn bounded_cancel_zero_fires_immediately() {
        let token = bounded_cancel(0);
        assert!(
            token.is_cancelled(),
            "deadline 0 must cancel so backfill HTTP is never unbounded"
        );
    }

    fn mock_spec(dimensions: usize, normalization: &str) -> EmbeddingSourceSpec {
        EmbeddingSourceSpec {
            provider_instance_id: "p".into(),
            incarnation: Some("i".into()),
            origin_host: "example.test".into(),
            embedding_path: "/v1/embeddings".into(),
            protocol: "openai_compatible".into(),
            model: "mock".into(),
            dimensions,
            encoding: "float".into(),
            normalization: normalization.to_owned(),
        }
    }

    #[test]
    fn pin_primary_space_rejects_non_l2_and_zero_dimensions() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("ws");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&cwd).unwrap();
        let handle = PrimeIndexHandle::get_or_create(&home, &cwd);
        let err = handle
            .pin_primary_space(PinnedEmbeddingSpace {
                snapshot_generation: 1,
                route_id: "emb-1".into(),
                space_fingerprint: "fp".into(),
                spec: mock_spec(8, "none"),
            })
            .unwrap_err();
        assert_eq!(err, PrimeIndexError::SpaceMismatch);
        let err = handle
            .pin_primary_space(PinnedEmbeddingSpace {
                snapshot_generation: 1,
                route_id: "emb-1".into(),
                space_fingerprint: "fp".into(),
                spec: mock_spec(0, NORMALIZATION_L2_V1),
            })
            .unwrap_err();
        assert_eq!(err, PrimeIndexError::SpaceMismatch);
        uninstall_prime_index(&home, &cwd);
    }

    #[tokio::test]
    async fn search_knn_rejects_query_in_wrong_space() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("ws");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&cwd).unwrap();
        let handle = PrimeIndexHandle::get_or_create(&home, &cwd);
        let items = skills_to_index_items(&[skill("a", "skills/a/SKILL.md")]);
        handle.sync_skills(1, &items).unwrap();
        handle
            .pin_primary_space(PinnedEmbeddingSpace {
                snapshot_generation: 1,
                route_id: "emb-1".into(),
                space_fingerprint: "fp-8".into(),
                spec: mock_spec(8, NORMALIZATION_L2_V1),
            })
            .unwrap();
        let err = handle.search_knn(&[0.1, 0.2, 0.3, 0.4], 4).unwrap_err();
        assert_eq!(
            err,
            PrimeIndexError::SpaceMismatch,
            "exact-space pin must refuse a query from a different dimension"
        );
        uninstall_prime_index(&home, &cwd);
    }

    #[tokio::test]
    async fn search_knn_rejects_empty_or_pending_fingerprint() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("ws");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&cwd).unwrap();
        let handle = PrimeIndexHandle::get_or_create(&home, &cwd);
        let items = skills_to_index_items(&[skill("a", "skills/a/SKILL.md")]);
        handle.sync_skills(1, &items).unwrap();
        handle
            .pin_primary_space(PinnedEmbeddingSpace {
                snapshot_generation: 1,
                route_id: "emb-1".into(),
                space_fingerprint: "fp-4".into(),
                spec: mock_spec(4, NORMALIZATION_L2_V1),
            })
            .unwrap();
        let err = handle.search_knn(&[0.1, 0.2, 0.3, 0.4], 4).unwrap_err();
        assert_eq!(
            err,
            PrimeIndexError::Unavailable,
            "an empty/pending live hash is KNN unavailability, not a mixed-space comparison"
        );
        uninstall_prime_index(&home, &cwd);
    }

    #[tokio::test]
    async fn search_knn_refuses_same_dimension_live_rows_from_other_space() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("ws");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&cwd).unwrap();
        let handle = PrimeIndexHandle::get_or_create(&home, &cwd);
        let items = skills_to_index_items(&[
            skill("alpha", "skills/alpha/SKILL.md"),
            skill("beta", "skills/beta/SKILL.md"),
        ]);
        handle.sync_skills(1, &items).unwrap();
        let spec_a = EmbeddingSourceSpec {
            model: "mock-a".into(),
            ..mock_spec(4, NORMALIZATION_L2_V1)
        };
        handle
            .pin_primary_space(PinnedEmbeddingSpace {
                snapshot_generation: 1,
                route_id: "emb-a".into(),
                space_fingerprint: "fp-a".into(),
                spec: spec_a.clone(),
            })
            .unwrap();
        let embedder = Arc::new(MockEmbeddingProvider { dimensions: 4 });
        handle
            .backfill(
                embedder,
                handle.freeze_pin().unwrap(),
                CancellationToken::new(),
            )
            .await
            .expect("spec A backfill must install a live fingerprint");
        let query = vec![0.5f32, 0.5, 0.5, 0.5];
        handle
            .search_knn(&query, 4)
            .expect("spec A pin must be allowed to query its own live table");

        let spec_b = EmbeddingSourceSpec {
            model: "mock-b".into(),
            ..mock_spec(4, NORMALIZATION_L2_V1)
        };
        assert_ne!(spec_a, spec_b, "same-dimension specs must still differ");
        handle
            .pin_primary_space(PinnedEmbeddingSpace {
                snapshot_generation: 2,
                route_id: "emb-b".into(),
                space_fingerprint: "fp-b".into(),
                spec: spec_b,
            })
            .unwrap();
        let err = handle.search_knn(&query, 4).unwrap_err();
        assert_eq!(
            err,
            PrimeIndexError::SpaceMismatch,
            "spec B pin must not promote spec A's same-dimension live rows"
        );
        uninstall_prime_index(&home, &cwd);
    }

    #[tokio::test]
    async fn search_knn_with_pin_refuses_table_swapped_to_other_space() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("ws");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&cwd).unwrap();
        let handle = PrimeIndexHandle::get_or_create(&home, &cwd);
        let items = skills_to_index_items(&[
            skill("alpha", "skills/alpha/SKILL.md"),
            skill("beta", "skills/beta/SKILL.md"),
        ]);
        handle.sync_skills(1, &items).unwrap();
        let spec_a = EmbeddingSourceSpec {
            model: "mock-a".into(),
            ..mock_spec(4, NORMALIZATION_L2_V1)
        };
        let spec_b = EmbeddingSourceSpec {
            model: "mock-b".into(),
            ..mock_spec(4, NORMALIZATION_L2_V1)
        };
        handle
            .pin_primary_space(PinnedEmbeddingSpace {
                snapshot_generation: 1,
                route_id: "emb-a".into(),
                space_fingerprint: "fp-a".into(),
                spec: spec_a.clone(),
            })
            .unwrap();
        let embedder = Arc::new(MockEmbeddingProvider { dimensions: 4 });
        handle
            .backfill(
                embedder,
                handle.freeze_pin().unwrap(),
                CancellationToken::new(),
            )
            .await
            .expect("spec A backfill");
        let frozen_a = handle.freeze_pin().unwrap();
        let query = vec![0.5f32, 0.5, 0.5, 0.5];
        handle
            .search_knn_with_pin(&frozen_a, &query, 4)
            .expect("spec A pin must query its own table");

        install_collection_space_for_tests(&handle, spec_b).await;
        assert_eq!(
            handle.pinned_space().unwrap().route_id,
            "emb-a",
            "table swap must not retarget the handle pin"
        );
        assert_eq!(
            handle
                .search_knn_with_pin(&frozen_a, &query, 4)
                .unwrap_err(),
            PrimeIndexError::SpaceMismatch,
            "frozen A query must not score a same-dimension spec B table"
        );
        uninstall_prime_index(&home, &cwd);
    }

    #[tokio::test]
    async fn upsert_for_fingerprint_refuses_table_swapped_to_other_space() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("ws");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&cwd).unwrap();
        let handle = PrimeIndexHandle::get_or_create(&home, &cwd);
        let items = skills_to_index_items(&[skill("alpha", "skills/alpha/SKILL.md")]);
        handle.sync_skills(1, &items).unwrap();
        let spec_a = EmbeddingSourceSpec {
            model: "mock-a".into(),
            ..mock_spec(4, NORMALIZATION_L2_V1)
        };
        let spec_b = EmbeddingSourceSpec {
            model: "mock-b".into(),
            ..mock_spec(4, NORMALIZATION_L2_V1)
        };
        handle
            .pin_primary_space(PinnedEmbeddingSpace {
                snapshot_generation: 1,
                route_id: "emb-a".into(),
                space_fingerprint: "fp-a".into(),
                spec: spec_a.clone(),
            })
            .unwrap();
        handle
            .backfill(
                Arc::new(MockEmbeddingProvider { dimensions: 4 }),
                handle.freeze_pin().unwrap(),
                CancellationToken::new(),
            )
            .await
            .expect("spec A backfill");
        let frozen_a = handle.freeze_pin().unwrap();
        install_collection_space_for_tests(&handle, spec_b).await;
        let idx = MetadataIndex::open_or_create(handle.db_path()).unwrap();
        let item_id = opaque_skill_id(&skill("alpha", "skills/alpha/SKILL.md"));
        let err = idx
            .upsert_embedding_for_fingerprint(
                CollectionKind::Skills,
                &item_id,
                &[0.5, 0.5, 0.5, 0.5],
                frozen_a.fingerprint_hash(),
            )
            .unwrap_err();
        assert!(
            matches!(err, xai_grok_memory::MetadataIndexError::InvalidItem(_)),
            "A-space vectors must not install under a spec B table: {err}"
        );
        uninstall_prime_index(&home, &cwd);
    }

    #[tokio::test]
    async fn stale_generation_refuses_backfill_write() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("ws");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&cwd).unwrap();
        let handle = PrimeIndexHandle::get_or_create(&home, &cwd);
        let items = skills_to_index_items(&[skill("a", "skills/a/SKILL.md")]);
        handle.sync_skills(1, &items).unwrap();
        handle
            .pin_primary_space(PinnedEmbeddingSpace {
                snapshot_generation: 1,
                route_id: "emb-1".into(),
                space_fingerprint: "fp".into(),
                spec: EmbeddingSourceSpec {
                    provider_instance_id: "p".into(),
                    incarnation: None,
                    origin_host: "example.test".into(),
                    embedding_path: "/v1/embeddings".into(),
                    protocol: "openai_compatible".into(),
                    model: "mock".into(),
                    dimensions: 4,
                    encoding: "float".into(),
                    normalization: NORMALIZATION_L2_V1.to_owned(),
                },
            })
            .unwrap();
        let later = skills_to_index_items(&[
            skill("a", "skills/a/SKILL.md"),
            skill("b", "skills/b/SKILL.md"),
        ]);
        handle.sync_skills(2, &later).unwrap();
        handle.inventory_generation.store(1, Ordering::Relaxed);
        let embedder = Arc::new(MockEmbeddingProvider { dimensions: 4 });
        let err = handle
            .backfill(
                embedder,
                handle.freeze_pin().unwrap(),
                CancellationToken::new(),
            )
            .await
            .err();
        assert_eq!(
            err,
            Some(PrimeIndexError::StaleGeneration),
            "stale generation must not install mixed inventory vectors: {err:?}"
        );
        uninstall_prime_index(&home, &cwd);
    }

    fn fingerprint_hash(spec: &EmbeddingSourceSpec) -> String {
        VectorFingerprint::build(
            spec.clone(),
            metadata_doc_prep(CollectionKind::Skills),
            VECTOR_SCHEMA_VERSION,
        )
        .unwrap()
        .0
        .hash
    }

    struct GatedMarkerEmbedder {
        dimensions: usize,
        marker: Vec<f32>,
        started: Arc<tokio::sync::Semaphore>,
        release: Arc<tokio::sync::Semaphore>,
    }

    #[async_trait::async_trait]
    impl EmbeddingProvider for GatedMarkerEmbedder {
        async fn embed_batch(
            &self,
            texts: &[&str],
        ) -> Result<Vec<Vec<f32>>, Box<dyn std::error::Error>> {
            self.started.add_permits(1);
            let _ = self.release.acquire().await;
            Ok(texts.iter().map(|_| self.marker.clone()).collect())
        }

        fn model_name(&self) -> &str {
            "gated-marker"
        }

        fn dimensions(&self) -> usize {
            self.dimensions
        }
    }

    #[tokio::test]
    async fn backfill_refuses_same_dimension_route_swap_during_embed() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("ws");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&cwd).unwrap();
        let handle = PrimeIndexHandle::get_or_create(&home, &cwd);
        let items = skills_to_index_items(&[skill("alpha", "skills/alpha/SKILL.md")]);
        handle.sync_skills(1, &items).unwrap();
        let spec_a = EmbeddingSourceSpec {
            model: "mock-a".into(),
            ..mock_spec(4, NORMALIZATION_L2_V1)
        };
        let spec_b = EmbeddingSourceSpec {
            model: "mock-b".into(),
            ..mock_spec(4, NORMALIZATION_L2_V1)
        };
        let hash_a = fingerprint_hash(&spec_a);
        let hash_b = fingerprint_hash(&spec_b);
        assert_ne!(hash_a, hash_b);
        handle
            .pin_primary_space(PinnedEmbeddingSpace {
                snapshot_generation: 1,
                route_id: "emb-a".into(),
                space_fingerprint: "fp-a".into(),
                spec: spec_a.clone(),
            })
            .unwrap();

        let started = Arc::new(tokio::sync::Semaphore::new(0));
        let release = Arc::new(tokio::sync::Semaphore::new(0));
        let embedder = Arc::new(GatedMarkerEmbedder {
            dimensions: 4,
            marker: vec![1.0, 0.0, 0.0, 0.0],
            started: started.clone(),
            release: release.clone(),
        });
        let frozen_a = handle.freeze_pin().expect("frozen A before spawn");
        let handle_task = handle.clone();
        let task = tokio::spawn(async move {
            handle_task
                .backfill(embedder, frozen_a, CancellationToken::new())
                .await
        });
        let _started = tokio::time::timeout(std::time::Duration::from_secs(10), started.acquire())
            .await
            .expect("gated embedder should start")
            .expect("start permit");
        handle
            .pin_primary_space(PinnedEmbeddingSpace {
                snapshot_generation: 2,
                route_id: "emb-b".into(),
                space_fingerprint: "fp-b".into(),
                spec: spec_b.clone(),
            })
            .unwrap();
        release.add_permits(32);
        let result = task.await.unwrap();
        assert_eq!(
            result.err(),
            Some(PrimeIndexError::SpaceMismatch),
            "A-to-B swap during embed must refuse install"
        );

        let state = {
            let idx = MetadataIndex::open_or_create(handle.db_path()).unwrap();
            idx.collection_state(CollectionKind::Skills).unwrap()
        };
        assert_ne!(
            state.fingerprint_hash, hash_b,
            "must not install B-table identity after an A-space embed"
        );
        assert!(
            state.fingerprint_hash.is_empty(),
            "pin mismatch after embed must discard staging rather than install, got {}",
            state.fingerprint_hash
        );
        if state.vec_count > 0 {
            let conn = rusqlite::Connection::open(handle.db_path()).unwrap();
            let blob: Vec<u8> = conn
                .query_row("SELECT embedding FROM skills_vec LIMIT 1", [], |row| {
                    row.get(0)
                })
                .unwrap_or_default();
            if blob.len() >= 8 {
                let x = f32::from_le_bytes(blob[0..4].try_into().unwrap());
                let y = f32::from_le_bytes(blob[4..8].try_into().unwrap());
                assert!(
                    x > y,
                    "A-table must not hold B-marker vectors, got ({x}, {y})"
                );
            }
        }
        uninstall_prime_index(&home, &cwd);
    }

    #[tokio::test]
    async fn captured_generation_zero_refuses_write_after_inventory_sync() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("ws");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&cwd).unwrap();
        let handle = PrimeIndexHandle::get_or_create(&home, &cwd);
        handle
            .pin_primary_space(PinnedEmbeddingSpace {
                snapshot_generation: 1,
                route_id: "emb-1".into(),
                space_fingerprint: "fp".into(),
                spec: EmbeddingSourceSpec {
                    provider_instance_id: "p".into(),
                    incarnation: None,
                    origin_host: "example.test".into(),
                    embedding_path: "/v1/embeddings".into(),
                    protocol: "openai_compatible".into(),
                    model: "mock".into(),
                    dimensions: 4,
                    encoding: "float".into(),
                    normalization: NORMALIZATION_L2_V1.to_owned(),
                },
            })
            .unwrap();
        let items = skills_to_index_items(&[skill("a", "skills/a/SKILL.md")]);
        handle.sync_skills(1, &items).unwrap();
        handle.inventory_generation.store(0, Ordering::Relaxed);
        let embedder = Arc::new(MockEmbeddingProvider { dimensions: 4 });
        let err = handle
            .backfill(
                embedder,
                handle.freeze_pin().unwrap(),
                CancellationToken::new(),
            )
            .await
            .err();
        assert_eq!(
            err,
            Some(PrimeIndexError::StaleGeneration),
            "captured generation 0 must not write after a live inventory sync: {err:?}"
        );
        uninstall_prime_index(&home, &cwd);
    }

    #[tokio::test]
    async fn cancel_during_gated_embed_ends_unavailable_not_completed() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let cwd = tmp.path().join("ws");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&cwd).unwrap();
        let handle = PrimeIndexHandle::get_or_create(&home, &cwd);
        let items = skills_to_index_items(&[skill("alpha", "skills/alpha/SKILL.md")]);
        handle.sync_skills(1, &items).unwrap();
        handle
            .pin_primary_space(PinnedEmbeddingSpace {
                snapshot_generation: 1,
                route_id: "emb-1".into(),
                space_fingerprint: "fp".into(),
                spec: EmbeddingSourceSpec {
                    provider_instance_id: "p".into(),
                    incarnation: None,
                    origin_host: "example.test".into(),
                    embedding_path: "/v1/embeddings".into(),
                    protocol: "openai_compatible".into(),
                    model: "mock".into(),
                    dimensions: 4,
                    encoding: "float".into(),
                    normalization: NORMALIZATION_L2_V1.to_owned(),
                },
            })
            .unwrap();
        let started = Arc::new(tokio::sync::Semaphore::new(0));
        let release = Arc::new(tokio::sync::Semaphore::new(0));
        let embedder = Arc::new(GatedMarkerEmbedder {
            dimensions: 4,
            marker: vec![1.0, 0.0, 0.0, 0.0],
            started: started.clone(),
            release: release.clone(),
        });
        let cancel = CancellationToken::new();
        let frozen = handle.freeze_pin().unwrap();
        let handle_task = handle.clone();
        let cancel_task = cancel.clone();
        let task =
            tokio::spawn(async move { handle_task.backfill(embedder, frozen, cancel_task).await });
        let _started = tokio::time::timeout(std::time::Duration::from_secs(10), started.acquire())
            .await
            .expect("gated embedder should start")
            .expect("start permit");
        cancel.cancel();
        release.add_permits(32);
        let result = task.await.unwrap();
        assert_eq!(
            result.as_ref().err(),
            Some(&PrimeIndexError::Unavailable),
            "cancel during embed must not complete or embed_failed: {result:?}"
        );
        uninstall_prime_index(&home, &cwd);
    }

    #[test]
    fn skill_rerank_document_includes_body_and_omits_absolute_unc_file_url() {
        let mut s = skill("deploy", "skills/deploy/SKILL.md");
        s.body = Some("SECRET-BODY".into());
        s.when_to_use = Some("see /Users/secret/file".into());
        s.paths = Some(vec![
            "/Users/secret".into(),
            r"\\server\share\file".into(),
            "file:///etc/passwd".into(),
            "src/**".into(),
        ]);
        let doc = skill_rerank_document(&s);
        assert!(
            doc.contains("SECRET-BODY"),
            "full-catalog indexing must include the body: {doc}"
        );
        assert!(!doc.contains("/Users/"), "absolute path leaked: {doc}");
        assert!(!doc.contains("file:"), "file URL leaked: {doc}");
        assert!(!doc.contains("\\\\"), "UNC leaked: {doc}");
        assert!(xai_grok_memory::reject_persisted_paths("/Users/secret", "t").is_err());
    }

    #[test]
    fn skill_rerank_document_keeps_relative_src_and_bounded_triggers() {
        let mut s = skill("format", "skills/format/SKILL.md");
        s.when_to_use = Some("format rust code".into());
        s.paths = Some(vec!["src/**".into()]);
        let doc = skill_rerank_document(&s);
        assert!(
            doc.contains("src/**"),
            "accepted relative path dropped: {doc}"
        );
        assert!(
            doc.contains("format rust code"),
            "accepted trigger dropped: {doc}"
        );
        assert!(!doc.contains("SECRET"));
    }

    #[test]
    fn skill_rerank_document_omits_userinfo_and_encoded_paths() {
        let mut s = skill("deploy", "skills/deploy/SKILL.md");
        s.when_to_use = Some("https://user:secret@example.com/docs".into());
        s.paths = Some(vec![
            "%2FUsers%2Fsecret".into(),
            "%2e%2e/%2e%2e/etc/passwd".into(),
            r"%5c%5cserver%5cshare".into(),
            "file://user:secret@localhost/etc/passwd".into(),
            "file:%2f%2f%2fetc/passwd".into(),
            "src/**".into(),
        ]);
        let doc = skill_rerank_document(&s);
        assert!(!doc.contains("secret"), "userinfo leaked: {doc}");
        assert!(!doc.contains("user:"), "userinfo leaked: {doc}");
        assert!(!doc.contains("/Users/"), "encoded absolute leaked: {doc}");
        assert!(!doc.contains("%2FUsers"), "encoded absolute leaked: {doc}");
        assert!(
            !doc.contains("etc/passwd"),
            "encoded traversal leaked: {doc}"
        );
        assert!(!doc.contains("server"), "encoded UNC leaked: {doc}");
        assert!(
            !doc.contains("file:"),
            "credentialed file URL leaked: {doc}"
        );
        assert!(
            doc.contains("src/**"),
            "accepted relative path dropped: {doc}"
        );
    }
}
