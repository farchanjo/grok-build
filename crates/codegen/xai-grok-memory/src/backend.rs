//! Concrete `MemoryBackend` implementation using hybrid search.
//!
//! `MemoryBackendImpl` combines FTS5 keyword search with optional vector
//! KNN similarity via `hybrid_search()`. When embeddings are available
//! (embedding config + API key), the query is vectorized and both signals
//! are merged with recency and source weights. When embeddings are
//! unavailable, gracefully degrades to FTS-only.
//!
//! `rusqlite::Connection` is `!Send + !Sync`, so we open a fresh `MemoryIndex`
//! per query. WAL mode ensures concurrent readers don't block.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rusqlite::params;

use xai_grok_tools::types::memory_backend::{MemoryBackend, MemorySearchResult};

use super::storage::MemoryStorage;
use super::watcher::MemoryFileWatcher;
use crate::schema;

/// Embedding-client credentials scoped to a trusted endpoint. Only
/// [`Self::for_endpoint`] retains a live credential; the empty default fails closed.
#[derive(Clone, Default)]
pub struct EndpointScopedCredentials {
    endpoint: Option<reqwest::Url>,
    auth_credentials: Option<Arc<dyn xai_grok_auth::AuthCredentialProvider>>,
    api_key_provider: Option<xai_grok_tools::types::SharedApiKeyProvider>,
}

// Manual Debug that redacts the credential handles; only their presence shows.
impl std::fmt::Debug for EndpointScopedCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EndpointScopedCredentials")
            .field("endpoint", &self.endpoint)
            .field("has_auth_credentials", &self.auth_credentials.is_some())
            .field("has_api_key_provider", &self.api_key_provider.is_some())
            .finish()
    }
}

impl EndpointScopedCredentials {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.auth_credentials.is_none() && self.api_key_provider.is_none()
    }

    /// Retains the credentials only for a trusted, parsable `endpoint`; otherwise drops them.
    pub fn for_endpoint(
        endpoint: &str,
        is_trusted: impl FnOnce(&str) -> bool,
        auth_credentials: Option<Arc<dyn xai_grok_auth::AuthCredentialProvider>>,
        api_key_provider: Option<xai_grok_tools::types::SharedApiKeyProvider>,
    ) -> Self {
        if is_trusted(endpoint)
            && let Ok(url) = reqwest::Url::parse(endpoint)
        {
            return Self {
                endpoint: Some(url),
                auth_credentials,
                api_key_provider,
            };
        }
        if auth_credentials.is_some() || api_key_provider.is_some() {
            tracing::info!(
                target: xai_grok_telemetry::memory_log::TARGET,
                endpoint,
                "memory embeddings: session credentials withheld for non-first-party endpoint; its own key, if any, still applies"
            );
        }
        Self::none()
    }

    fn auth_credentials(&self) -> Option<&Arc<dyn xai_grok_auth::AuthCredentialProvider>> {
        self.auth_credentials.as_ref()
    }

    fn api_key_provider(&self) -> Option<&xai_grok_tools::types::SharedApiKeyProvider> {
        self.api_key_provider.as_ref()
    }

    fn approved_for(&self, base_url: &str) -> bool {
        match &self.endpoint {
            None => self.is_empty(),
            Some(endpoint) => reqwest::Url::parse(base_url).is_ok_and(|url| &url == endpoint),
        }
    }
}

/// Max candidate-body chars sent to a remote reranker (bounded metadata/text).
const RERANK_BODY_CHAR_BOUND: usize = 4_000;

/// Extract a normalized origin host (no credentials, no path) from a base URL.
fn origin_host(base_url: &str) -> String {
    match reqwest::Url::parse(base_url) {
        Ok(url) => match url.host_str() {
            Some(h) => match url.port() {
                Some(p) => format!("{h}:{p}"),
                None => h.to_owned(),
            },
            None => base_url.trim().to_owned(),
        },
        Err(_) => base_url.trim().to_owned(),
    }
}

/// Synthesize a deterministic, credential-free legacy embedding source spec
/// from `[memory.embedding]` + legacy endpoint. Existing users with populated
/// vectors and no persisted fingerprint adopt this spec so they never have to
/// reconnect/rebuild.
pub fn legacy_embedding_source_spec(
    cfg: &xai_grok_config_types::MemoryEmbeddingConfig,
    base_url: &str,
) -> Option<super::fingerprint::EmbeddingSourceSpec> {
    let model = cfg.model.as_deref().filter(|m| !m.is_empty())?;
    Some(super::fingerprint::EmbeddingSourceSpec {
        provider_instance_id: "legacy:memory.embedding".to_owned(),
        incarnation: None,
        origin_host: origin_host(base_url),
        // Canonical embedding endpoint path shared with the named-profile
        // facade so the same logical endpoint never fingerprints differently
        // purely from the hardcoded literal.
        embedding_path: canonical_embedding_path(),
        protocol: "openai_compatible".to_owned(),
        model: model.to_owned(),
        dimensions: cfg.dimensions,
        encoding: "float".to_owned(),
        normalization: super::fingerprint::NORMALIZATION_NONE.to_owned(),
    })
}

/// The canonical embedding endpoint path used in both named-profile and
/// legacy source identity, matching the wire path the providers hit.
fn canonical_embedding_path() -> String {
    use xai_grok_inference::DEFAULT_EMBEDDINGS_PATH;
    DEFAULT_EMBEDDINGS_PATH.to_owned()
}

/// All configuration needed to build a fully-wired [`MemoryBackendImpl`] for a live session.
///
/// Grouping these in one struct ensures every call site — ToolBridge, first-turn
/// injection, and post-compaction recovery — shares identical config.  Without it,
/// different paths silently fell back to FTS-only search and ignored
/// `[memory.search]` config because no single place applied all builder methods.
#[derive(Clone)]
pub struct MemoryBackendParams {
    /// Session ID for telemetry events.
    pub session_id: String,
    /// Embedding provider config — `None` forces FTS-only fallback everywhere.
    pub embed_config: Option<xai_grok_config_types::MemoryEmbeddingConfig>,
    /// Base URL for embedding API calls (CLI proxy). Must match the endpoint
    /// `embedding_credentials` was scoped to; mismatch fails closed.
    pub embed_base_url: String,
    /// API key for embedding API calls.
    pub embed_api_key: Option<String>,
    /// Hybrid search scoring config (weights, thresholds, decay, MMR).
    pub search_config: xai_grok_config_types::MemorySearchConfig,
    /// File watcher for sync-on-search — `None` disables external-edit detection.
    pub watcher: Option<Arc<MemoryFileWatcher>>,
    /// Seconds before a stale reindex claim is forcibly released.
    pub stale_claim_secs: i64,
    /// Telemetry label emitted with every search event from this backend.
    ///
    /// Differentiates the three runtime search paths in dashboards and logs:
    /// - `"tool"` — model-initiated `memory_search` tool call (ToolBridge)
    /// - `"injection"` — first-turn memory context injection
    /// - `"compaction_recovery"` — post-compaction context re-injection
    pub search_source: &'static str,
    pub embedding_credentials: EndpointScopedCredentials,
    /// Credential-free retrieval facade. When present, embeddings and
    /// optional remote reranking route through it (named profile or a
    /// shell-synthesized legacy source) instead of the legacy
    /// `[memory.embedding]` provider. `None` keeps the legacy path.
    pub retrieval: Option<Arc<dyn super::retrieval::MemoryRetrieval>>,
    /// The exact `MemoryIndexConfig` every chunk writer uses. Both the vector
    /// fingerprint's doc-preparation determinant and the search index must use
    /// this one value; a non-default prep change therefore rebuilds.
    pub index_config: xai_grok_config_types::MemoryIndexConfig,
    /// Seconds to back off between failed vector-rebuild attempts (FTS-only
    /// while backing off), so repeated failures do not rebuild on every search.
    pub rebuild_backoff_secs: i64,
    /// Optional remote vector-store mirror bound to this index's collection.
    /// SQLite stays the authority: the mirror receives best-effort fan-out
    /// and serves KNN reads only when verified-ready; any error falls back
    /// to sqlite-vec. `None` keeps pure sqlite-vec behavior.
    pub vector_mirror: Option<std::sync::Arc<crate::mirror::MirrorHandle>>,
    /// Active memory mode: `local` (default) or `milvus` (primary remote).
    pub mode: xai_grok_config_types::MemoryMode,
}

impl MemoryBackendParams {
    /// Single embedding-provider factory for **every** memory embedding
    /// consumer (search, watcher, flush, Dream, memory_state back-fill,
    /// startup reindex): a named-profile facade (credential-free
    /// [`RetrievalEmbeddingProvider`]) is authoritative; only when no named
    /// profile is configured does the legacy `[memory.embedding]` path apply.
    /// Never falls back to a sibling/active-chat credential path under a named
    /// profile — including an *unresolved* named profile (facade `None` ⇒ also
    /// `embed_config None` ⇒ `None` ⇒ FTS-only).
    pub async fn make_embedding_provider(
        &self,
    ) -> Option<std::sync::Arc<dyn super::embedding::EmbeddingProvider>> {
        resolve_embedding_provider(
            self.retrieval.clone(),
            self.embed_config.clone(),
            &self.embedding_credentials,
            self.embed_api_key.as_deref(),
            &self.embed_base_url,
        )
        .await
    }

    /// Resolve the exact non-secret source identity used by the embedding factory.
    pub fn embedding_source_spec(&self) -> Option<super::fingerprint::EmbeddingSourceSpec> {
        if let Some(retrieval) = &self.retrieval {
            return Some(retrieval.source_spec());
        }
        self.embed_config
            .as_ref()
            .and_then(|cfg| legacy_embedding_source_spec(cfg, &self.embed_base_url))
    }

    /// Single dimension resolution matching the factory: named-profile facade
    /// spec dimensions, else legacy `[memory.embedding]` dimensions, else 1024.
    /// Every writer that opens the index (search, watcher, flush/Dream
    /// write-time back-fill, startup reindex) must use the same space.
    pub fn resolve_embedding_dims(&self) -> usize {
        if let Some(retrieval) = &self.retrieval {
            retrieval.source_spec().dimensions
        } else if let Some(cfg) = &self.embed_config {
            cfg.dimensions
        } else {
            1024
        }
    }
}

/// Single embedding-provider factory used by **every** memory embedding
/// consumer (search, watcher, flush, Dream, memory_state back-fill, startup
/// reindex). A named-profile facade (credential-free
/// [`RetrievalEmbeddingProvider`]) is authoritative when present; the legacy
/// `[memory.embedding]` path applies only when no named profile is configured
/// (callers pass `embed_config = None` under a named profile). Never falls
/// back to active-chat credentials or a sibling path under a named profile —
/// including an *unresolved* named profile (facade `None` ⇒ also
/// `embed_config None` ⇒ `None` ⇒ FTS-only). Async so
/// `current_api_key_async` can drive the AuthManager refresh chain; reindex
/// loops outlive the OIDC TTL.
pub async fn resolve_embedding_provider(
    retrieval: Option<std::sync::Arc<dyn super::retrieval::MemoryRetrieval>>,
    embed_config: Option<xai_grok_config_types::MemoryEmbeddingConfig>,
    credentials: &EndpointScopedCredentials,
    static_api_key: Option<&str>,
    base_url: &str,
) -> Option<std::sync::Arc<dyn super::embedding::EmbeddingProvider>> {
    if let Some(retrieval) = retrieval {
        return Some(std::sync::Arc::new(
            super::embedding::RetrievalEmbeddingProvider::new(retrieval),
        ));
    }
    build_embedding_provider(embed_config.as_ref(), credentials, static_api_key, base_url)
        .await
        .map(
            |p| -> std::sync::Arc<dyn super::embedding::EmbeddingProvider> {
                std::sync::Arc::new(p)
            },
        )
}

async fn build_embedding_provider(
    config: Option<&xai_grok_config_types::MemoryEmbeddingConfig>,
    credentials: &EndpointScopedCredentials,
    static_api_key: Option<&str>,
    base_url: &str,
) -> Option<super::embedding::ApiEmbeddingProvider> {
    let config = config?;
    if config.model.as_ref().is_none_or(|m| m.is_empty()) {
        return None;
    }

    // Enforce at runtime, in release too: a `debug_assert` would compile out of
    // shipped binaries and let a scoped credential reach an unapproved URL.
    let credentials_approved = credentials.approved_for(base_url);
    if !credentials_approved {
        tracing::error!(
            target: xai_grok_telemetry::memory_log::TARGET,
            mismatch = true,
            "memory embeddings: scoped credentials do not match the request origin; dropping them"
        );
    }

    if credentials_approved && let Some(creds) = credentials.auth_credentials() {
        let client = super::embedding::build_middleware_client(creds.clone());
        return super::embedding::ApiEmbeddingProvider::from_config(
            config,
            base_url.to_owned(),
            client,
        );
    }

    let per_call_key = if credentials_approved && let Some(p) = credentials.api_key_provider() {
        p.current_api_key_async().await
    } else {
        None
    };
    let api_key = per_call_key.or_else(|| static_api_key.map(|s| s.to_owned()))?;
    super::embedding::ApiEmbeddingProvider::from_session(config, base_url.to_owned(), api_key)
}

/// `MemoryBackend` implementation backed by hybrid search (FTS5 + vector KNN).
///
/// Stores only `Send + Sync` config data. The `MemoryIndex` and
/// `EmbeddingProvider` are constructed on demand per query.
pub struct MemoryBackendImpl {
    db_path: PathBuf,
    storage: MemoryStorage,
    /// Embedding config — `None` disables vector search (FTS-only fallback).
    embed_config: Option<xai_grok_config_types::MemoryEmbeddingConfig>,
    /// API base URL for embedding requests (cli-chat-proxy).
    embed_base_url: String,
    /// API key for embedding requests.
    embed_api_key: Option<String>,
    /// Search scoring config (weights, min_score, max_results).
    search_config: xai_grok_config_types::MemorySearchConfig,
    /// File watcher for detecting external memory edits.
    watcher: Option<Arc<MemoryFileWatcher>>,
    /// Stale claim threshold for reindex coordination.
    stale_claim_secs: i64,
    /// Session ID for telemetry events.
    session_id: String,
    /// Telemetry label for search events — mirrors [`MemoryBackendParams::search_source`].
    search_source: &'static str,
    /// Shared search counter — read by session summary telemetry.
    ///
    /// Only the ToolBridge backend's counter is shared back to the session actor;
    /// injection and compaction-recovery backends use their own local counters.
    pub search_counter: std::sync::Arc<std::sync::atomic::AtomicU64>,
    embedding_credentials: EndpointScopedCredentials,
    retrieval: Option<Arc<dyn super::retrieval::MemoryRetrieval>>,
    index_config: xai_grok_config_types::MemoryIndexConfig,
    rebuild_backoff_secs: i64,
    vector_mirror: Option<Arc<crate::mirror::MirrorHandle>>,
    mode: xai_grok_config_types::MemoryMode,
}

impl MemoryBackendImpl {
    /// Single embedding-provider factory for every embedding consumer
    /// (search, watcher, flush, Dream, memory_state, startup reindex):
    /// named-profile facade authoritative, legacy only when no named profile
    /// is configured, unresolved named profile ⇒ `None` ⇒ FTS-only.
    pub(crate) async fn make_embedding_provider(
        &self,
    ) -> Option<Arc<dyn super::embedding::EmbeddingProvider>> {
        resolve_embedding_provider(
            self.retrieval.clone(),
            self.embed_config.clone(),
            &self.embedding_credentials,
            self.embed_api_key.as_deref(),
            &self.embed_base_url,
        )
        .await
    }

    /// Create a new backend. `db_path` must point to an existing SQLite
    /// database created by `MemoryIndex::open_or_create()`.
    pub fn new(db_path: PathBuf, storage: MemoryStorage) -> Self {
        Self {
            db_path,
            storage,
            embed_config: None,
            embed_base_url: String::new(),
            embed_api_key: None,
            search_config: xai_grok_config_types::MemorySearchConfig::default(),
            watcher: None,
            stale_claim_secs: 60,
            session_id: String::new(),
            search_source: "tool",
            embedding_credentials: EndpointScopedCredentials::none(),
            retrieval: None,
            index_config: xai_grok_config_types::MemoryIndexConfig::default(),
            rebuild_backoff_secs: 0,
            vector_mirror: None,
            mode: xai_grok_config_types::MemoryMode::Local,
            search_counter: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    /// The active memory mode (`local` or `milvus`).
    pub fn mode(&self) -> xai_grok_config_types::MemoryMode {
        self.mode
    }

    /// Set the memory mode.
    pub fn with_mode(mut self, mode: xai_grok_config_types::MemoryMode) -> Self {
        self.mode = mode;
        self
    }

    /// Set the session ID for telemetry.
    pub fn with_session_id(mut self, session_id: String) -> Self {
        self.session_id = session_id;
        self
    }

    /// Configure the embedding provider for hybrid search.
    ///
    /// Without this, `search()` falls back to FTS-only.
    pub fn with_embedding(
        mut self,
        config: xai_grok_config_types::MemoryEmbeddingConfig,
        base_url: String,
        api_key: Option<String>,
    ) -> Self {
        self.embed_config = Some(config);
        self.embed_base_url = base_url;
        self.embed_api_key = api_key;
        self
    }

    /// Override the search scoring config (weights, limits, etc.).
    pub fn with_search_config(mut self, config: xai_grok_config_types::MemorySearchConfig) -> Self {
        self.search_config = config;
        self
    }

    /// Attach a file watcher for sync-on-search (reindex dirty files before querying).
    pub fn with_watcher(mut self, watcher: Arc<MemoryFileWatcher>, stale_claim_secs: i64) -> Self {
        self.watcher = Some(watcher);
        self.stale_claim_secs = stale_claim_secs;
        self
    }

    /// Open a read-only connection for simple queries (`total_chunks`, `get`).
    fn open_readonly(&self) -> Result<rusqlite::Connection, rusqlite::Error> {
        // Journal-mode-aware open (busy_timeout included): never mmap a legacy
        // WAL -shm on network mounts (SIGBUS); see JournalMode::open_readonly.
        xai_sqlite_journal::JournalMode::for_db_path(&self.db_path).open_readonly(&self.db_path)
    }

    /// Build a fully configured backend for a live session.
    ///
    /// Prefer this over calling `new()` + individual builder methods: it ensures
    /// session_id, embeddings, search config, and the file watcher are applied
    /// consistently at every call site (ToolBridge, first-turn injection,
    /// post-compaction recovery).  Using the factory eliminates the silent
    /// per-site drift where some paths got hybrid search while others fell back
    /// to FTS-only, and where `[memory.search]` config was effectively ignored.
    pub fn from_session_params(storage: MemoryStorage, params: &MemoryBackendParams) -> Self {
        let db_path = storage.workspace_dir().join("index.sqlite");
        let mut backend = Self::new(db_path, storage)
            .with_session_id(params.session_id.clone())
            .with_search_config(params.search_config.clone());
        backend.search_source = params.search_source;
        if let Some(ec) = &params.embed_config {
            backend = backend.with_embedding(
                ec.clone(),
                params.embed_base_url.clone(),
                params.embed_api_key.clone(),
            );
        }
        if let Some(w) = &params.watcher {
            backend = backend.with_watcher(w.clone(), params.stale_claim_secs);
        }
        backend.embedding_credentials = params.embedding_credentials.clone();
        backend.retrieval = params.retrieval.clone();
        backend.index_config = params.index_config.clone();
        backend.rebuild_backoff_secs = params.rebuild_backoff_secs;
        backend.vector_mirror = params.vector_mirror.clone();
        backend.mode = params.mode;
        backend
    }

    /// Attach a credential-free retrieval facade.
    pub fn with_retrieval(
        mut self,
        retrieval: Option<Arc<dyn super::retrieval::MemoryRetrieval>>,
    ) -> Self {
        self.retrieval = retrieval;
        self
    }

    /// Resolve the effective (spec, dimensions) for the pinned embedding
    /// source: the named-profile/synthesized-legacy facade when present, else
    /// the legacy `[memory.embedding]` config, else `None` + default dims.
    fn effective_embedding_spec(&self) -> (Option<super::fingerprint::EmbeddingSourceSpec>, usize) {
        if let Some(r) = &self.retrieval {
            let spec = r.source_spec();
            return (Some(spec.clone()), spec.dimensions);
        }
        let cfg = self.embed_config.as_ref();
        match cfg {
            Some(c) => match legacy_embedding_source_spec(c, &self.embed_base_url) {
                Some(spec) => (Some(spec.clone()), spec.dimensions),
                None => (None, 1024),
            },
            None => (None, 1024),
        }
    }
}

/// Test-only field accessors.
///
/// These expose private fields so tests can assert that `from_session_params`
/// actually stored the values it was given, without routing through a full
/// runtime search call whose semantics override some config fields.
#[cfg(test)]
impl MemoryBackendImpl {
    /// Returns the session ID stored in this backend.
    pub fn session_id_for_test(&self) -> &str {
        &self.session_id
    }

    /// Returns the search config stored in this backend.
    pub fn search_config_for_test(&self) -> &xai_grok_config_types::MemorySearchConfig {
        &self.search_config
    }
}

/// Per-search batch cap for the `ReadyMissing` compatible-gap backfill — the
/// same bounded discipline as the rebuild loop (batches of 32, capped at 4),
/// so a large one-shot gap is processed across subsequent searches instead of
/// one synchronous embed of everything.
const READY_MISSING_BACKFILL_BATCH_CAP: usize = 4;
/// Batch size for the incremental backfill path (matches the rebuild batch).
const BACKFILL_BATCH_SIZE: usize = 32;

/// Whether the persisted incremental-backfill backoff is currently active
/// (`now < deadline`). Written only by a genuine incremental embed failure
/// (never by a cap pause); suppresses the `ReadyMissing` gap backfill until it
/// passes, so a failing embedder is not hammered every search while the gap
/// still self-heals eventually.
fn backfill_backoff_active(index: &super::index::MemoryIndex) -> bool {
    let until: i64 = index
        .db()
        .query_row(
            schema::GET_META_SQL,
            params![schema::META_VECTOR_BACKFILL_BACKOFF_UNTIL],
            |r| r.get::<_, String>(0),
        )
        .map(|s| s.trim().parse::<i64>().unwrap_or(0))
        .unwrap_or(0);
    until > 0 && now_secs() < until
}

/// Persist an incremental-backfill backoff deadline (`now + backoff_secs`).
fn record_backfill_backoff(index: &super::index::MemoryIndex, backoff_secs: i64) {
    if backoff_secs <= 0 {
        return;
    }
    let until = now_secs() + backoff_secs;
    let _ = index.db().execute(
        schema::UPSERT_META_SQL,
        params![
            schema::META_VECTOR_BACKFILL_BACKOFF_UNTIL,
            until.to_string()
        ],
    );
}

/// Clear any persisted incremental-backfill backoff (a successful pass means
/// the embedder recovered; the gap self-heals immediately).
fn clear_backfill_backoff(index: &super::index::MemoryIndex) {
    let _ = index.db().execute(
        schema::UPSERT_META_SQL,
        params![schema::META_VECTOR_BACKFILL_BACKOFF_UNTIL, "0"],
    );
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[async_trait::async_trait]
impl MemoryBackend for MemoryBackendImpl {
    #[tracing::instrument(name = "memory.search", skip_all, fields(
        session_id = %self.session_id, max_results, min_score,
    ))]
    async fn search(
        &self,
        query: &str,
        max_results: usize,
        min_score: f64,
    ) -> Result<Vec<MemorySearchResult>, Box<dyn std::error::Error + Send + Sync>> {
        // Open a MemoryIndex for this query (open-per-query, ~1ms).
        //
        // IMPORTANT: `MemoryIndex` is `Send` but `!Sync`, so `&MemoryIndex`
        // is `!Send`. To keep this future `Send`, we must never hold a
        // `&index` borrow across an `.await` point. The code below is
        // structured into sync phases (borrow &index) and async phases
        // (no &index borrow) to satisfy this constraint.
        // Resolve the effective embedding source (named profile via the
        // credential-free facade, or a synthesized legacy `[memory.embedding]`
        // source) and the pinned vector space.
        // The real index config used by every chunk writer drives both the
        // fingerprint's doc-preparation determinant and the chunker.
        let index_config = self.index_config.clone();
        let (spec, embed_dims) = self.effective_embedding_spec();
        // Build an embedder for the pinned source (None ⇒ no vectors /
        // FTS-only) through the single factory: named-profile facade
        // (credential-free) authoritative; legacy `[memory.embedding]` only
        // when no named profile is configured; unresolved named profile ⇒
        // None ⇒ FTS-only.
        let embedder = self.make_embedding_provider().await;
        // Cancel token for the vector rebuild loop (search has no external
        // cancellation; the loop only checks this to stay cooperative). The
        // rebuild is throttled by a persisted back-off and a per-search batch
        // cap so pending searches stay FTS-only.
        let rebuild_cancel = tokio_util::sync::CancellationToken::new();
        let rebuild_backoff_secs = self.rebuild_backoff_secs;
        let rebuild_batches_per_call = 4usize;
        // Reconcile / transactionally rebuild vectors through the pinned source.
        // In milvus mode, local SQLite vec rebuild is bypassed (Milvus handles vectors).
        let readiness = match &spec {
            Some(s) if self.mode == xai_grok_config_types::MemoryMode::Local => {
                crate::rebuild::ensure_vectors_ready(
                    &self.db_path,
                    self.storage.clone(),
                    index_config.clone(),
                    s,
                    embedder.clone(),
                    self.stale_claim_secs,
                    rebuild_backoff_secs,
                    Some(rebuild_batches_per_call),
                    rebuild_cancel,
                )
                .await
            }
            _ => crate::rebuild::VectorReadiness::Disabled,
        };
        // Vector search is active for `Ready` (compatible + complete) and for
        // `ReadyMissing` (compatible, a few current chunks lack vectors — the
        // existing vectors are usable and only the missing rows are backfilled).
        // Only a pending/disabled state must go FTS-only.
        let vec_active = matches!(
            readiness,
            crate::rebuild::VectorReadiness::Ready
                | crate::rebuild::VectorReadiness::ReadyMissing { .. }
        );

        let index_config_fallback = index_config.clone();
        let mut index = super::index::MemoryIndex::open_or_create(
            &self.db_path,
            self.storage.clone(),
            index_config,
            embed_dims,
        )
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
            Box::new(std::io::Error::other(e.to_string()))
        })?;

        // ── Sync phase 1: reindex dirty files, collect chunks needing embeddings ──
        // Watcher-dirty chunks are *fresh* work — accumulated in their own set
        // and are not deferred by the compatible-gap backoff or embedded in full.
        let mut watcher_chunks: Vec<(String, String)> = Vec::new();
        // Owner token for the reindex claim; release is owner-scoped so a
        // stolen (stale-window) claim is never cleared by the loser.
        let mut reindex_claim: Option<String> = None;
        // Watcher-sync telemetry data (populated inside the claim guard below).
        let mut watcher_sync_stats: Option<(usize, usize, std::time::Instant)> = None;
        if let Some(ref watcher) = self.watcher
            && watcher.is_dirty()
            && let Some(claim) = index.try_claim_reindex_owned(self.stale_claim_secs)
        {
            reindex_claim = Some(claim);
            let sync_start = std::time::Instant::now();
            let dirty_files = watcher.take_dirty();
            let dirty_count = dirty_files.len();
            // Sum of all index-chunk changes this cycle: chunks added/updated/
            // removed during reindex_file, plus chunks removed by delete_path.
            // Using one counter rather than two prevents telemetry from
            // under-reporting delete-only syncs (where reindex_file is never
            // called and the old `reindexed_count` would stay at 0).
            let mut changed_chunk_count: usize = 0;
            // Chunk ids removed by file deletions, for mirror delete fan-out.
            let mut deleted_chunk_ids: Vec<String> = Vec::new();
            for file in &dirty_files {
                if file.exists() {
                    // File was created or modified — reindex it.
                    let source = self.storage.classify_source(file);
                    if let Ok(stats) = index.reindex_file(file, source) {
                        changed_chunk_count += stats.added + stats.updated + stats.removed;
                    }
                } else {
                    // File was deleted — remove its stale chunks from the index so
                    // they are no longer searchable.  Without this call, reindex_file
                    // returns early when the file is unreadable and leaves orphaned
                    // chunks behind indefinitely.
                    if let Ok(ids) = index.delete_path_ids(file) {
                        changed_chunk_count += ids.len();
                        deleted_chunk_ids.extend(ids);
                    }
                }
            }
            if let Some(handle) = &self.vector_mirror
                && !deleted_chunk_ids.is_empty()
            {
                let timeout = crate::mirror::mirror_timeout(None);
                if let Err(e) =
                    crate::mirror::mirror_delete_ids(handle, &deleted_chunk_ids, timeout).await
                {
                    tracing::warn!(
                        target: xai_grok_telemetry::memory_log::TARGET,
                        error = %e,
                        "memory mirror delete fan-out failed; stale mirror rows are healed by the next resync"
                    );
                }
            }
            if dirty_count > 0 {
                // Only chunks from currently dirty files are fresh work.
                // are "fresh work". The pre-existing compatible gap (chunks
                // from other files that missed an earlier embed) belongs to
                // the gap set below, which a genuine-failure backoff defers.
                let dirty_paths: Vec<String> = dirty_files
                    .iter()
                    .filter(|f| f.exists())
                    .map(|f| f.to_string_lossy().into_owned())
                    .collect();
                watcher_chunks = index
                    .chunks_without_embeddings_for_paths(&dirty_paths)
                    .unwrap_or_default();
            }
            watcher_sync_stats = Some((dirty_count, changed_chunk_count, sync_start));
        }

        // `ReadyMissing`: compatible but some current chunks lack vectors
        // (incremental chunk churn or a transient incremental embed failure).
        // The gap is a property of the compatible missing set: a genuine embed
        // failure persists a backoff that defers retrying the *same missing
        // set until it passes, but it must not suppress freshly changed
        // watcher-dirty chunks.
        let backfill_backed_off = backfill_backoff_active(&index);
        // Watcher-dirty ids are already covered by the watcher set — exclude
        // them from the gap set so the combined work set has no duplicates.
        let watcher_ids: std::collections::HashSet<&str> =
            watcher_chunks.iter().map(|(id, _)| id.as_str()).collect();
        let mut gap_chunks: Vec<(String, String)> = if matches!(
            readiness,
            crate::rebuild::VectorReadiness::ReadyMissing { .. }
        ) {
            if backfill_backed_off {
                // Persisted backoff from a genuine incremental embed
                // failure — don't re-attempt the gap this search (vector
                // search stays active; the gap retries after the deadline
                // passes).
                Vec::new()
            } else {
                index
                    .chunks_without_embeddings()
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|(id, _)| !watcher_ids.contains(id.as_str()))
                    .collect()
            }
        } else {
            Vec::new()
        };

        // ── Combine watcher & gap work under a single per-search batch cap ──
        // The watcher-dirty incremental embed is bounded per search too,
        // so a large watcher gap progresses over later searches instead of
        // being embedded synchronously in full — and a cap pause never arms the
        // backfill backoff. Watcher chunks take priority; the gap gets the
        // remaining cap.
        let per_search_cap = READY_MISSING_BACKFILL_BATCH_CAP * BACKFILL_BATCH_SIZE;
        watcher_chunks.truncate(per_search_cap);
        let gap_cap_remaining = per_search_cap.saturating_sub(watcher_chunks.len());
        gap_chunks.truncate(gap_cap_remaining);
        let mut reindex_chunks = watcher_chunks;
        reindex_chunks.extend(gap_chunks);

        // ── Async phase: embed missing chunks (no &index borrow) ──
        // Incremental embedding into `chunks_vec` only when the pinned vector
        // space is installed (Ready or ReadyMissing); during a pending rebuild
        // we must never mix old/new-space rows into the vec table.
        let mut embedded_count: usize = 0;
        // The shared cap above bounds the total synchronous incremental embed
        // per search; the gap set is already skipped while backed off, and
        // watcher work must proceed even during a gap backoff.
        if vec_active
            && !reindex_chunks.is_empty()
            && let Some(ref embedder) = embedder
        {
            let mut upserts: Vec<(String, Vec<f32>)> = Vec::new();
            let mut failed = false;
            for batch in reindex_chunks.chunks(BACKFILL_BATCH_SIZE) {
                let texts: Vec<&str> = batch.iter().map(|(_, t)| t.as_str()).collect();
                match embedder.embed_batch(&texts).await {
                    Ok(embeddings) => {
                        for ((chunk_id, _), emb) in batch.iter().zip(embeddings.into_iter()) {
                            upserts.push((chunk_id.clone(), emb));
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            target: xai_grok_telemetry::memory_log::TARGET,
                            error = %e,
                            "embedding batch failed during sync-on-search, skipping; incremental embed backs off"
                        );
                        // Stop after a genuine failure; retries are
                        // gated by the persisted backoff, not repeated per
                        // search, including the watcher-dirty set.
                        failed = true;
                        break;
                    }
                }
            }
            if failed {
                // Persist an incremental-backfill backoff after failure. A cap
                // pause never arms it.
                record_backfill_backoff(&index, self.rebuild_backoff_secs);
            } else {
                // A successful pass means the embedder recovered; clear any
                // stale backoff so the gap self-heals immediately.
                clear_backfill_backoff(&index);
            }
            // Sync: upsert embeddings back (borrows &index, no await)
            for (chunk_id, emb) in &upserts {
                let _ = index.upsert_embedding(chunk_id, emb);
            }
            embedded_count = upserts.len();
            // Best-effort mirror fan-out of the fresh vectors (time-bounded,
            // failure-isolated; SQLite stays the authority). The plan is
            // computed synchronously (borrows &index) so only owned data
            // crosses the execute await — this future must stay `Send`.
            if let Some(handle) = &self.vector_mirror
                && !upserts.is_empty()
                && let Some(plan) = crate::mirror_fanout_plan(&index, handle, &upserts)
            {
                crate::mirror_fanout_execute(plan, handle).await;
            }
        }

        // In milvus mode, reconcile with remote schema-v2 collection when unready or reindex claimed.
        if self.mode == xai_grok_config_types::MemoryMode::Milvus {
            if let Some(ref handle) = self.vector_mirror
                && let Some(ref embedder) = embedder
                && let Some(ref spec) = spec
                && let Ok(fp) = spec.fingerprint(&self.index_config)
            {
                let dims = spec.dimensions as u32;
                if !handle.is_ready_for(&fp.hash, dims) || reindex_claim.is_some() {
                    let _ = crate::reconcile_milvus_mode(&mut index, &**embedder, handle, &fp.hash).await;
                }
            }
        }

        if let Some(claim) = reindex_claim {
            // Owner-scoped release: never clears a claim stolen via the stale
            // window while we were working.
            index.release_claim(&claim);
            // Fire watcher-sync telemetry now that we know the embedded count.
            if let Some((dirty_count, reindexed_count, sync_start)) = watcher_sync_stats {
                xai_grok_telemetry::session_ctx::log_event(
                    xai_grok_telemetry::memory_telemetry::MemoryWatcherSync {
                        session_id: self.session_id.clone(),
                        dirty_file_count: dirty_count,
                        claimed: true,
                        reindexed_count,
                        embedded_count,
                        duration_ms: sync_start.elapsed().as_millis() as u64,
                    },
                );
            }
        }

        // ── Sync phase 2: FTS search ──
        let mut search_config = self.search_config.clone();
        search_config.max_results = max_results;
        search_config.min_score = min_score as f32;

        // In milvus mode, execute hard-remote search (BM25 + KNN) and bypass local FTS/vec search.
        if self.mode == xai_grok_config_types::MemoryMode::Milvus {
            if let Some(ref handle) = self.vector_mirror
                && let Some(ref spec) = spec
            {
                let fp_hash = spec.fingerprint(&self.index_config).map(|f| f.hash).unwrap_or_default();
                let dims = spec.dimensions as u32;

                let results = crate::search::milvus_search(
                    handle,
                    embedder.as_deref(),
                    query,
                    &fp_hash,
                    dims,
                    &search_config,
                )
                .await
                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                    Box::new(std::io::Error::other(e.to_string()))
                })?;

                self.search_counter
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                return Ok(results
                    .into_iter()
                    .map(|r| MemorySearchResult {
                        chunk_id: r.chunk_id,
                        path: r.path,
                        start_line: r.start_line,
                        end_line: r.end_line,
                        score: r.score,
                        snippet: r.snippet,
                        source: r.source,
                        created_at: Some(r.created_at),
                    })
                    .collect());
            } else {
                tracing::warn!(
                    target: xai_grok_telemetry::memory_log::TARGET,
                    "milvus mode active but vector_mirror or spec missing; returning empty results"
                );
                return Ok(Vec::new());
            }
        }

        let search_start = std::time::Instant::now();
        let keyword_count = super::query_expansion::extract_keywords(query).len();
        let candidate_limit = search_config.max_results * 3;
        let mut fts_results = index.search_fts(query, candidate_limit).unwrap_or_default();

        // Supplemental evergreen query: ensure global/workspace MEMORY.md
        // chunks appear in candidates even when session volume crowds them
        // out of the base FTS results. Mirrors hybrid_search() in search.rs.
        let evergreen = index
            .search_fts_by_sources(query, candidate_limit, &["global", "workspace"])
            .unwrap_or_default();
        let existing: std::collections::HashSet<String> =
            fts_results.iter().map(|r| r.chunk_id.clone()).collect();
        for r in evergreen {
            if !existing.contains(&r.chunk_id) {
                fts_results.push(r);
            }
        }

        let vec_available = index.vec_available() && embedder.is_some() && vec_active;

        // ── Async phase: embed query for vector search (no &index borrow) ──
        let query_embedding = if vec_available {
            if let Some(ref embedder) = embedder {
                match embedder.embed_batch(&[query]).await {
                    Ok(embeddings) if !embeddings.is_empty() => {
                        Some(embeddings.into_iter().next().unwrap())
                    }
                    Ok(_) => None,
                    Err(e) => {
                        tracing::warn!(error = %e, "embedding query failed, falling back to FTS-only");
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };

        // ── Vector candidates: mirror-first with sqlite-vec fallback ──
        // The readiness gate runs synchronously with the &index borrow
        // (fingerprint + dims + vec_row_count from one SQLite read set);
        // only owned data (handle + fingerprint + query vector) crosses the
        // mirror `.await`, and the fallback re-opens a fresh index handle —
        // the original borrow has ended — keeping this future `Send`.
        let mirror_decision = super::search::mirror_knn_decision(
            &index,
            self.vector_mirror.as_ref(),
            query_embedding.as_deref(),
        );
        let vec_results = match mirror_decision {
            Some((handle, fingerprint_hash, query)) => {
                match super::search::mirror_knn_execute(
                    handle.clone(),
                    fingerprint_hash,
                    query.clone(),
                    candidate_limit,
                )
                .await
                {
                    Ok(hits) => hits,
                    Err(e) => {
                        tracing::warn!(
                            target: xai_grok_telemetry::memory_log::TARGET,
                            error = %e,
                            "mirror KNN failed; falling back to sqlite-vec"
                        );
                        handle.mark_unavailable();
                        // Re-borrow SQLite: fresh index handle (Send-safe —
                        // the original borrow ended before the await).
                        match super::index::MemoryIndex::open_or_create(
                            &self.db_path,
                            self.storage.clone(),
                            index_config_fallback,
                            embed_dims,
                        ) {
                            Ok(fallback_index) => fallback_index
                                .vector_search(&query, candidate_limit)
                                .unwrap_or_default(),
                            Err(_) => Vec::new(),
                        }
                    }
                }
            }
            None => match query_embedding.as_deref() {
                Some(embedding) => index
                    .vector_search(embedding, candidate_limit)
                    .unwrap_or_default(),
                None => Vec::new(),
            },
        };
        let (mut candidates, mut relevance) = super::search::build_local_candidates_from_vec(
            &index,
            fts_results,
            vec_results,
            &search_config,
        )
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
            Box::new(std::io::Error::other(e.to_string()))
        })?;

        // ── Async phase: optional remote rerank through the pinned source
        // (no &index borrow). Feed bounded text only. On any failure the
        // complete exact local pre-rerank order is restored and MMR/
        // truncation continue. ──
        super::search::remote_rerank(
            &mut candidates,
            &mut relevance,
            self.retrieval.as_deref(),
            query,
            RERANK_BODY_CHAR_BOUND,
        )
        .await;

        // ── Sync phase 4: finalize (MMR + truncation) ──
        let results = super::search::finalize_order(candidates, relevance, &search_config);

        // Record accesses for the returned chunks so access_count and
        // last_accessed stay current.  Non-fatal: a failed write is a no-op
        // for the caller and does not affect the search response.
        for result in &results {
            let _ = index.record_access(&result.chunk_id);
        }

        let duration_ms = search_start.elapsed().as_millis() as u64;
        let search_mode = if vec_available { "hybrid" } else { "fts_only" };
        let top_score = results.first().map_or(0.0, |r| r.score);

        if results.is_empty() {
            xai_grok_telemetry::session_ctx::log_event(
                xai_grok_telemetry::memory_telemetry::MemorySearchEmpty {
                    session_id: self.session_id.clone(),
                    query_length: query.len(),
                    keyword_count,
                    min_score_threshold: min_score,
                    search_mode: search_mode.to_owned(),
                    duration_ms,
                    vec_available,
                    source: self.search_source.to_owned(),
                },
            );
        } else {
            xai_grok_telemetry::session_ctx::log_event(
                xai_grok_telemetry::memory_telemetry::MemorySearch {
                    session_id: self.session_id.clone(),
                    query_length: query.len(),
                    keyword_count,
                    result_count: results.len(),
                    top_score,
                    min_score_threshold: min_score,
                    search_mode: search_mode.to_owned(),
                    duration_ms,
                    vec_available,
                    source: self.search_source.to_owned(),
                },
            );
        }
        self.search_counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        Ok(results
            .into_iter()
            .map(|r| MemorySearchResult {
                chunk_id: r.chunk_id,
                path: r.path,
                start_line: r.start_line,
                end_line: r.end_line,
                score: r.score,
                snippet: r.snippet,
                source: r.source,
                created_at: Some(r.created_at),
            })
            .collect())
    }

    fn get(
        &self,
        path: &str,
        from: Option<usize>,
        lines: Option<usize>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.storage.read_file(Path::new(path), from, lines)?)
    }

    fn total_chunks(&self) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let conn = self.open_readonly()?;
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get(0))?;
        Ok(count as usize)
    }

    /// Return the configured `max_results` from the stored search config.
    ///
    /// Overrides the trait default so the `memory_search` tool honours
    /// `[memory.search].max_results` from config when the model does not
    /// supply an explicit value.
    fn default_search_max_results(&self) -> usize {
        self.search_config.max_results
    }

    /// Return the configured `min_score` from the stored search config.
    fn default_search_min_score(&self) -> f64 {
        self.search_config.min_score as f64
    }
}

#[cfg(test)]
mod factory_tests {
    use super::*;
    use crate::index::{MemoryIndex, init_sqlite_vec};
    use crate::storage::MemoryStorage;
    use tempfile::TempDir;
    use xai_grok_config_types::{MemoryEmbeddingConfig, MemorySearchConfig};

    fn make_storage(tmp: &TempDir) -> MemoryStorage {
        let global = tmp.path().join("memory");
        let workspace = global.join("test_ws");
        MemoryStorage::with_paths(global, workspace)
    }

    fn make_params_fts_only(session_id: &str) -> MemoryBackendParams {
        MemoryBackendParams {
            session_id: session_id.to_string(),
            embed_config: None,
            embed_base_url: String::new(),
            embed_api_key: None,
            search_config: MemorySearchConfig::default(),
            watcher: None,
            stale_claim_secs: 60,
            search_source: "tool",
            embedding_credentials: EndpointScopedCredentials::none(),
            retrieval: None,
            index_config: xai_grok_config_types::MemoryIndexConfig::default(),
            rebuild_backoff_secs: 0,
            vector_mirror: None,
            mode: xai_grok_config_types::MemoryMode::Local,
        }
    }

    /// from_session_params stores the session_id it was given.
    ///
    /// Direct assertion via the `#[cfg(test)]` accessor proves the factory
    /// actually stored the value rather than discarding it.  The counter
    /// increment check additionally confirms the backend is functional.
    #[tokio::test]
    async fn test_factory_sets_session_id() {
        let tmp = TempDir::new().unwrap();
        init_sqlite_vec();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let mut idx = MemoryIndex::open_or_create(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            4,
        )
        .unwrap();
        let file = tmp.path().join("note.md");
        std::fs::write(&file, "# Facts\n\nRust is fast.").unwrap();
        idx.reindex_file(&file, "workspace").unwrap();
        drop(idx);

        let params = make_params_fts_only("test-session-abc");
        let backend = MemoryBackendImpl::from_session_params(storage, &params);

        // Direct assertion: the stored session_id matches what the factory was given.
        assert_eq!(
            backend.session_id_for_test(),
            "test-session-abc",
            "session_id must be stored exactly as supplied"
        );

        // Functional check: the backend actually runs a search.
        let before = backend
            .search_counter
            .load(std::sync::atomic::Ordering::Relaxed);
        let _ = backend.search("rust", 5, 0.0).await;
        let after = backend
            .search_counter
            .load(std::sync::atomic::Ordering::Relaxed);
        assert_eq!(
            after,
            before + 1,
            "search counter must increment per search"
        );
    }

    /// from_session_params stores the search_config it was given.
    ///
    /// Direct assertion via the `#[cfg(test)]` accessor proves the factory
    /// propagated the config into the backend rather than discarding it.
    /// `max_results` is verified because the `search()` method overrides it
    /// with the caller's argument — so checking the *stored* value is the only
    /// way to confirm the factory wired it correctly.
    #[tokio::test]
    async fn test_factory_wires_search_config() {
        let tmp = TempDir::new().unwrap();
        init_sqlite_vec();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let mut idx = MemoryIndex::open_or_create(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            4,
        )
        .unwrap();
        for i in 0..10 {
            let f = tmp.path().join(format!("note{i}.md"));
            std::fs::write(&f, format!("# Entry {i}\n\nRust tip number {i}.")).unwrap();
            idx.reindex_file(&f, "workspace").unwrap();
        }
        drop(idx);

        let params = MemoryBackendParams {
            search_config: MemorySearchConfig {
                max_results: 3,
                ..Default::default()
            },
            ..make_params_fts_only("test-search-config")
        };
        let backend = MemoryBackendImpl::from_session_params(storage, &params);

        // Direct: the stored config has exactly the value the factory was given.
        assert_eq!(
            backend.search_config_for_test().max_results,
            3,
            "stored max_results must equal what was supplied to the factory"
        );
    }

    /// from_session_params wires non-overridable config fields (MMR, temporal decay)
    /// that `search()` never replaces with caller arguments.
    ///
    /// This is the clearest proof that `[memory.search]` config is actually wired
    /// rather than silently ignored: fields the caller cannot override must arrive
    /// in the stored search_config exactly as given.
    #[test]
    fn test_factory_wires_non_overridable_search_config_fields() {
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);

        let custom_search = MemorySearchConfig {
            max_results: 7,
            mmr: xai_grok_config_types::MmrConfig {
                enabled: true,
                lambda: 0.42,
            },
            temporal_decay: xai_grok_config_types::TemporalDecayConfig {
                enabled: true,
                half_life_days: 14.0,
            },
            ..Default::default()
        };
        let params = MemoryBackendParams {
            search_config: custom_search,
            ..make_params_fts_only("test-full-config")
        };
        let backend = MemoryBackendImpl::from_session_params(storage, &params);
        let stored = backend.search_config_for_test();

        // None of these are overridden by the caller in search() — they must
        // survive the factory path unchanged.
        assert_eq!(stored.max_results, 7);
        assert!(stored.mmr.enabled, "MMR enabled must be stored");
        assert!(
            (stored.mmr.lambda - 0.42).abs() < f64::EPSILON,
            "MMR lambda must be stored exactly"
        );
        assert!(
            stored.temporal_decay.enabled,
            "temporal_decay enabled must be stored"
        );
        assert!(
            (stored.temporal_decay.half_life_days - 14.0).abs() < f64::EPSILON,
            "temporal_decay half_life_days must be stored exactly"
        );
    }

    /// from_session_params propagates search_source into the backend.
    ///
    /// Correctness test: every caller (tool, injection,
    /// compaction_recovery) must be able to set a distinct source label so
    /// dashboards can separate the three search paths.
    #[test]
    fn test_factory_propagates_search_source() {
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);

        for source in ["tool", "injection", "compaction_recovery"] {
            let params = MemoryBackendParams {
                search_source: source,
                ..make_params_fts_only("test-source")
            };
            let backend = MemoryBackendImpl::from_session_params(storage.clone(), &params);
            assert_eq!(
                backend.search_source, source,
                "search_source must be propagated for source='{source}'"
            );
        }
    }

    /// The default search_source is "tool" when constructing via new().
    #[test]
    fn test_default_search_source_is_tool() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.sqlite");
        let storage = make_storage(&tmp);
        let backend = MemoryBackendImpl::new(db_path, storage);
        assert_eq!(backend.search_source, "tool");
    }

    /// MemoryBackendParams with different search_source values is Clone.
    #[test]
    fn test_params_clone_preserves_search_source() {
        let params = MemoryBackendParams {
            search_source: "injection",
            ..make_params_fts_only("test-clone-source")
        };
        let cloned = params.clone();
        assert_eq!(cloned.search_source, "injection");
    }

    /// Watcher startup telemetry reflects actual runtime state.
    ///
    /// `watcher.is_some()` is `true` only when the watcher started successfully.
    /// With a valid directory the watcher should start; without one it should return None.
    /// This guards the contract that `watcher_started` in telemetry must reflect
    /// runtime outcome, not configuration intent.
    #[test]
    fn test_params_watcher_started_reflects_runtime() {
        let tmp = TempDir::new().unwrap();

        // Success path: directory exists → watcher starts.
        let watch_dir = tmp.path().join("memory");
        std::fs::create_dir_all(&watch_dir).unwrap();
        let watcher = crate::watcher::MemoryFileWatcher::start(&watch_dir);
        let params_with_watcher = MemoryBackendParams {
            watcher: watcher.map(std::sync::Arc::new),
            ..make_params_fts_only("test-watcher-runtime")
        };
        // watcher.is_some() reflects whether startup succeeded.
        // (On environments without inotify/FSEvents this may be None; skip rather than fail.)
        let _ = params_with_watcher.watcher.is_some(); // just verify it compiles

        // Failure path: non-existent directory → watcher must return None.
        let missing = tmp.path().join("does_not_exist");
        let no_watcher = crate::watcher::MemoryFileWatcher::start(&missing);
        assert!(
            no_watcher.is_none(),
            "watcher must return None for a non-existent directory"
        );
        let params_no_watcher = MemoryBackendParams {
            watcher: None,
            ..make_params_fts_only("test-no-watcher")
        };
        assert!(
            params_no_watcher.watcher.is_none(),
            "params.watcher.is_none() means telemetry reports watcher_started=false"
        );
    }

    /// default_search_max_results returns the configured value from search_config.
    ///
    /// Verifies that the MemoryBackend trait override in MemoryBackendImpl
    /// exposes search_config.max_results rather than the hardcoded default (6).
    #[test]
    fn test_default_search_max_results_from_config() {
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);

        let params = MemoryBackendParams {
            search_config: MemorySearchConfig {
                max_results: 12,
                ..Default::default()
            },
            ..make_params_fts_only("test-defaults")
        };
        let backend = MemoryBackendImpl::from_session_params(storage, &params);
        assert_eq!(
            backend.default_search_max_results(),
            12,
            "default_search_max_results must return search_config.max_results"
        );
    }

    /// default_search_min_score returns the configured value from search_config.
    #[test]
    fn test_default_search_min_score_from_config() {
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);

        let params = MemoryBackendParams {
            search_config: MemorySearchConfig {
                min_score: 0.42,
                ..Default::default()
            },
            ..make_params_fts_only("test-defaults")
        };
        let backend = MemoryBackendImpl::from_session_params(storage, &params);
        assert!(
            (backend.default_search_min_score() - 0.42_f64).abs() < 1e-6,
            "default_search_min_score must return search_config.min_score"
        );
    }

    /// from_session_params without embed_config produces a backend that does not panic
    /// and returns results using FTS-only path.
    #[tokio::test]
    async fn test_factory_fts_only_without_embed() {
        let tmp = TempDir::new().unwrap();
        init_sqlite_vec();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let mut idx = MemoryIndex::open_or_create(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            4,
        )
        .unwrap();
        let f = tmp.path().join("note.md");
        std::fs::write(&f, "# Guide\n\nRust ownership rules.").unwrap();
        idx.reindex_file(&f, "workspace").unwrap();
        drop(idx);

        let params = make_params_fts_only("test-fts-only");
        let backend = MemoryBackendImpl::from_session_params(storage, &params);
        let results = backend.search("rust ownership", 5, 0.0).await.unwrap();
        assert!(
            !results.is_empty(),
            "FTS-only backend should return results"
        );
        let ts = results[0].created_at;
        assert!(
            ts.is_some() && ts.unwrap() > 0,
            "created_at must be Some(positive) after backend search (got {ts:?})"
        );
    }

    /// from_session_params with embed_config but no api_key gracefully falls back
    /// to FTS-only (the embedding provider requires a key).
    #[tokio::test]
    async fn test_factory_embed_config_without_key_falls_back_to_fts() {
        let tmp = TempDir::new().unwrap();
        init_sqlite_vec();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let mut idx = MemoryIndex::open_or_create(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            4,
        )
        .unwrap();
        let f = tmp.path().join("note.md");
        std::fs::write(&f, "# Guide\n\nRust borrow checker.").unwrap();
        idx.reindex_file(&f, "workspace").unwrap();
        drop(idx);

        let params = MemoryBackendParams {
            embed_config: Some(MemoryEmbeddingConfig::default()),
            embed_base_url: "http://localhost".to_string(),
            embed_api_key: None, // no key → provider cannot be created
            ..make_params_fts_only("test-embed-no-key")
        };
        let backend = MemoryBackendImpl::from_session_params(storage, &params);
        // Must not panic; FTS results should still come back.
        let results = backend.search("rust borrow", 5, 0.0).await.unwrap();
        assert!(
            !results.is_empty(),
            "should fall back to FTS when api_key is None"
        );
    }

    /// MemoryBackendParams is Clone.
    #[test]
    fn test_params_is_clone() {
        let params = make_params_fts_only("clone-test");
        let _cloned = params.clone();
    }

    /// from_session_params without watcher produces a backend that searches correctly.
    #[tokio::test]
    async fn test_factory_no_watcher() {
        let tmp = TempDir::new().unwrap();
        init_sqlite_vec();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let mut idx = MemoryIndex::open_or_create(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            4,
        )
        .unwrap();
        let f = tmp.path().join("note.md");
        std::fs::write(&f, "# Tip\n\nAlways write tests.").unwrap();
        idx.reindex_file(&f, "workspace").unwrap();
        drop(idx);

        let params = MemoryBackendParams {
            watcher: None,
            ..make_params_fts_only("test-no-watcher")
        };
        let backend = MemoryBackendImpl::from_session_params(storage, &params);
        let results = backend.search("tests", 5, 0.0).await.unwrap();
        assert!(
            !results.is_empty(),
            "no-watcher backend should still return results"
        );
    }

    /// `ensure_initialized` must be called before watcher startup.
    ///
    /// Regression test for the ordering fix: on a first-use machine the
    /// memory directories do not exist yet.  If the watcher tries to watch a
    /// non-existent directory it returns `None` (silently dropping the feature).
    /// After `ensure_initialized()` the directories exist and the watcher can
    /// start successfully.
    ///
    /// This mirrors the ordering enforced in `spawn_session_actor`:
    ///   1. `storage.ensure_initialized()`
    ///   2. `MemoryFileWatcher::start(storage.global_dir())`
    #[test]
    fn test_ensure_initialized_before_watcher_ordering() {
        let tmp = TempDir::new().unwrap();
        let global = tmp.path().join("memory");
        let workspace = global.join("test_ws");
        let storage = MemoryStorage::with_paths(global.clone(), workspace.clone());

        // Precondition: neither directory exists yet (fresh machine simulation).
        assert!(
            !global.exists(),
            "global memory dir must not exist before initialization"
        );

        // --- Wrong ordering (watcher before init) ---
        // The watcher returns None because the directory does not exist.
        let watcher_before_init = crate::watcher::MemoryFileWatcher::start(&global);
        assert!(
            watcher_before_init.is_none(),
            "watcher must fail (None) when directory does not exist yet"
        );

        // --- Correct ordering (init, then watcher) ---
        // After ensure_initialized the directories and MEMORY.md templates exist.
        storage.ensure_initialized().unwrap();

        assert!(
            global.exists(),
            "global dir must exist after ensure_initialized"
        );
        assert!(
            workspace.exists(),
            "workspace dir must exist after ensure_initialized"
        );
        assert!(
            global.join("MEMORY.md").exists(),
            "global MEMORY.md template must exist"
        );
        assert!(
            workspace.join("MEMORY.md").exists(),
            "workspace MEMORY.md template must exist"
        );

        // Watcher now succeeds because the directory exists.
        // (Allowed to return None in environments without inotify/kqueue
        //  support — e.g. some CI containers — but must not error-panic.)
        let watcher_after_init = crate::watcher::MemoryFileWatcher::start(&global);
        // If a watcher was returned we can confirm it is usable (not dirty yet).
        if let Some(w) = watcher_after_init {
            assert!(
                !w.is_dirty(),
                "freshly started watcher must report no dirty files"
            );
        }
        // If None, the test environment does not support file-watching —
        // that is acceptable; the directories themselves are what matter here.
    }

    /// End-to-end regression test for the watcher-driven delete path.
    ///
    /// Tests the full chain:
    ///   1. file is indexed
    ///   2. watcher is started
    ///   3. first `backend.search()` confirms content is found
    ///   4. file is deleted (OS fires a Remove event to the watcher)
    ///   5. second `backend.search()` triggers sync-on-search, which calls
    ///      `delete_path()` because the file no longer exists
    ///   6. content is no longer returned
    ///
    /// This test guards against regressions in the `file.exists() → else
    /// delete_path()` branch that would be invisible to the `delete_path`
    /// unit tests alone.
    #[tokio::test]
    async fn test_watcher_delete_clears_stale_chunks() {
        let tmp = TempDir::new().unwrap();
        init_sqlite_vec();

        let global = tmp.path().join("memory");
        let workspace = global.join("test_ws");
        std::fs::create_dir_all(&global).unwrap();
        std::fs::create_dir_all(&workspace).unwrap();

        let storage = MemoryStorage::with_paths(global.clone(), workspace);
        let db_path = storage.workspace_dir().join("index.sqlite");

        // Step 1: Write + canonicalize the file path BEFORE indexing.
        //
        // On macOS, TempDir paths may live under /private/tmp (via a symlink
        // from /tmp).  FSEvents returns canonicalized paths, so the path stored
        // in the index must match what the watcher event delivers.
        let file_raw = global.join("note.md");
        std::fs::write(&file_raw, "# Unique\n\nXyzzy-watcher-delete-token.").unwrap();
        let file = dunce::canonicalize(&file_raw).unwrap_or(file_raw);

        {
            let mut idx = MemoryIndex::open_or_create(
                &db_path,
                storage.clone(),
                xai_grok_config_types::MemoryIndexConfig::default(),
                4,
            )
            .unwrap();
            // Index with the canonical path so DB key matches watcher event paths.
            idx.reindex_file(&file, "workspace").unwrap();
        }

        // Step 2: Start watcher AFTER indexing so the Remove event for the
        // upcoming deletion is the first event the watcher ever sees.
        let watch_dir = dunce::canonicalize(&global).unwrap_or(global.clone());
        let watcher = match crate::watcher::MemoryFileWatcher::start(&watch_dir) {
            Some(w) => w,
            None => {
                // File-watching not supported in this environment (e.g., some CI
                // containers without inotify/FSEvents).  Skip rather than fail.
                return;
            }
        };
        let watcher_arc = std::sync::Arc::new(watcher);

        let params = MemoryBackendParams {
            watcher: Some(watcher_arc.clone()),
            ..make_params_fts_only("test-watcher-delete")
        };
        let backend = MemoryBackendImpl::from_session_params(storage, &params);

        // Step 3: Confirm content is found before deletion.
        let before = backend
            .search("Xyzzy-watcher-delete-token", 5, 0.0)
            .await
            .unwrap();
        assert!(
            !before.is_empty(),
            "content must be found before file is deleted"
        );

        // Step 4: Delete the file — the OS will fire a Remove event.
        std::fs::remove_file(&file).unwrap();

        // Poll until the watcher detects the event (more reliable than a fixed
        // sleep on macOS where FSEvents delivery time varies considerably).
        // Give up after 2 s and skip the timing-sensitive assertion rather than
        // flake — delete_path unit tests cover the underlying logic.
        let mut event_delivered = false;
        for _ in 0..20 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if watcher_arc.is_dirty() {
                event_delivered = true;
                break;
            }
        }
        if !event_delivered {
            // FSEvents not delivered within 2 s — environment is too slow.
            // Skip silently; the logic is covered by delete_path unit tests.
            return;
        }

        // Step 5+6: search triggers sync-on-search, which detects file.exists()
        // == false and calls delete_path(), clearing all stale chunks.
        let after = backend
            .search("Xyzzy-watcher-delete-token", 5, 0.0)
            .await
            .unwrap();
        assert!(
            after.is_empty(),
            "deleted file's content must not appear after watcher-driven delete sync"
        );
    }

    /// Regression: provider build must use `current_api_key_async`,
    /// never sync. Prevents memory_search 401s on rotated tokens.
    #[tokio::test]
    async fn make_embedding_provider_uses_async_api_key_resolution() {
        use std::sync::atomic::{AtomicU32, Ordering};
        use xai_grok_tools::types::ApiKeyProvider;

        struct AsyncProbe {
            sync_calls: Arc<AtomicU32>,
            async_calls: Arc<AtomicU32>,
        }
        impl ApiKeyProvider for AsyncProbe {
            fn current_api_key(&self) -> Option<String> {
                self.sync_calls.fetch_add(1, Ordering::SeqCst);
                Some("sync-stale".into())
            }
            fn current_api_key_async(
                &self,
            ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<String>> + Send + '_>>
            {
                let counter = self.async_calls.clone();
                Box::pin(async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    Some("async-fresh".into())
                })
            }
        }

        let sync_calls = Arc::new(AtomicU32::new(0));
        let async_calls = Arc::new(AtomicU32::new(0));
        let probe: xai_grok_tools::types::SharedApiKeyProvider = Arc::new(AsyncProbe {
            sync_calls: sync_calls.clone(),
            async_calls: async_calls.clone(),
        });

        let params = MemoryBackendParams {
            session_id: "s1".into(),
            embed_config: Some(MemoryEmbeddingConfig {
                model: Some("test-embed-model".into()),
                ..Default::default()
            }),
            embed_base_url: "http://example/v1".into(),
            embed_api_key: Some("static-fallback".into()),
            search_config: MemorySearchConfig::default(),
            watcher: None,
            stale_claim_secs: 60,
            search_source: "tool",
            // Trusted endpoint + no auth_credentials exercises the api_key_provider path.
            embedding_credentials: EndpointScopedCredentials::for_endpoint(
                "http://example/v1",
                |_| true,
                None,
                Some(probe),
            ),
            retrieval: None,
            index_config: xai_grok_config_types::MemoryIndexConfig::default(),
            rebuild_backoff_secs: 0,
            vector_mirror: None,
            mode: xai_grok_config_types::MemoryMode::Local,
        };

        let provider = params.make_embedding_provider().await;
        assert!(
            provider.is_some(),
            "provider must be built when model is set"
        );
        assert_eq!(
            async_calls.load(Ordering::SeqCst),
            1,
            "must call current_api_key_async exactly once per provider build"
        );
        assert_eq!(
            sync_calls.load(Ordering::SeqCst),
            0,
            "sync current_api_key must NOT be called — the async path is the contract"
        );
    }

    /// A backend wired with a credential-free retrieval facade routes
    /// embedding through that facade (pinned space); the legacy
    /// `[memory.embedding]` provider is not consulted.
    #[tokio::test]
    async fn test_facade_search_pins_space_and_installs_fingerprint() {
        let tmp = TempDir::new().unwrap();
        init_sqlite_vec();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        let mut idx = MemoryIndex::open_or_create(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            dims,
        )
        .unwrap();
        let f = tmp.path().join("note.md");
        std::fs::write(&f, "# Guide\n\nRust ownership rules.").unwrap();
        idx.reindex_file(&f, "workspace").unwrap();
        drop(idx);

        let fake = std::sync::Arc::new(crate::retrieval::FakeMemoryRetrieval::new(
            dims,
            "pinned-model",
        ));
        let params = MemoryBackendParams {
            retrieval: Some(fake.clone()),
            ..make_params_fts_only("facade-test")
        };
        let backend = MemoryBackendImpl::from_session_params(storage.clone(), &params);
        let results = backend.search("rust ownership", 5, 0.0).await.unwrap();
        assert!(
            !results.is_empty(),
            "facade-backed search must return results"
        );
        assert!(
            fake.embed_calls() > 0,
            "facade must be the embedding source"
        );

        let idx = MemoryIndex::open_or_create(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            dims,
        )
        .unwrap();
        assert!(
            idx.installed_vector_fingerprint_hash().is_some(),
            "search through the facade must install a durable fingerprint"
        );
    }

    /// A facade whose embedding source fails degrades to FTS-only: results
    /// still come back, no fingerprint is installed, and no partial/mixed
    /// vectors ever appear.
    #[tokio::test]
    async fn test_facade_failure_degrades_to_fts_only() {
        let tmp = TempDir::new().unwrap();
        init_sqlite_vec();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        let mut idx = MemoryIndex::open_or_create(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            dims,
        )
        .unwrap();
        let f = tmp.path().join("note.md");
        std::fs::write(&f, "# Guide\n\nRust borrow checker.").unwrap();
        idx.reindex_file(&f, "workspace").unwrap();
        drop(idx);

        let failing = std::sync::Arc::new(
            crate::retrieval::FakeMemoryRetrieval::new(dims, "m").with_embedment(|_| {
                Err(crate::retrieval::RetrievalError::new(
                    crate::retrieval::RetrievalErrorKind::Transient,
                ))
            }),
        );
        let params = MemoryBackendParams {
            retrieval: Some(failing),
            ..make_params_fts_only("facade-fail-test")
        };
        let backend = MemoryBackendImpl::from_session_params(storage.clone(), &params);
        let results = backend.search("rust borrow", 5, 0.0).await.unwrap();
        assert!(
            !results.is_empty(),
            "FTS must still return results when the facade embedding fails"
        );
        let idx = MemoryIndex::open_or_create(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            dims,
        )
        .unwrap();
        // Failing source => no vector set ever installed; no partial/mixed
        // vectors may ever appear.
        assert_eq!(
            idx.vec_row_count(),
            0,
            "no vectors may be installed on failure"
        );
        assert!(idx.chunk_count() > 0, "chunks must survive");
    }

    /// R-01/G3: a compatible-partial vector set (matching fingerprint, one
    /// missing current chunk) self-heals **on search** even with no watcher and
    /// no dirty files — `ReadyMissing` keeps existing vectors usable and
    /// backfills only the missing chunk instead of triggering a full rebuild.
    #[tokio::test]
    async fn test_search_self_heals_compatible_partial_no_watcher() {
        let tmp = TempDir::new().unwrap();
        init_sqlite_vec();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;

        // Chunk A, then atomically install a matching fingerprint + vector.
        let spec = crate::retrieval::stub_spec(dims, "m");
        {
            let mut idx = MemoryIndex::open_or_create(
                &db_path,
                storage.clone(),
                xai_grok_config_types::MemoryIndexConfig::default(),
                dims,
            )
            .unwrap();
            let fa = tmp.path().join("a.md");
            std::fs::write(&fa, "# A\n\nRust a content.").unwrap();
            idx.reindex_file(&fa, "workspace").unwrap();
        }
        let install_embedder: Option<std::sync::Arc<dyn crate::embedding::EmbeddingProvider>> =
            Some(std::sync::Arc::new(
                crate::embedding::RetrievalEmbeddingProvider::new(std::sync::Arc::new(
                    crate::retrieval::FakeMemoryRetrieval::new(dims, "m"),
                )),
            ));
        let out = crate::rebuild::ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &spec,
            install_embedder,
            60,
            0,
            Some(usize::MAX),
            tokio_util::sync::CancellationToken::new(),
        )
        .await;
        assert!(
            matches!(out, crate::rebuild::VectorReadiness::Ready),
            "{out:?}"
        );

        // Add chunk B but do NOT embed it (a transient incremental embed
        // failure): matching fp, 2 chunks, 1 vector row.
        {
            let mut idx = MemoryIndex::open_or_create(
                &db_path,
                storage.clone(),
                xai_grok_config_types::MemoryIndexConfig::default(),
                dims,
            )
            .unwrap();
            let fb = tmp.path().join("b.md");
            std::fs::write(&fb, "# B\n\nRust b content.").unwrap();
            idx.reindex_file(&fb, "workspace").unwrap();
            assert_eq!(idx.vec_row_count(), 1, "B must not be auto-embedded");
            assert_eq!(idx.chunk_count(), 2);
        }

        // A backend with no watcher (and no dirty files) must still self-heal:
        // search returns results and leaves the vector set complete.
        let params = MemoryBackendParams {
            retrieval: Some(std::sync::Arc::new(
                crate::retrieval::FakeMemoryRetrieval::new(dims, "m"),
            )),
            watcher: None,
            ..make_params_fts_only("self-heal-test")
        };
        let backend = MemoryBackendImpl::from_session_params(storage.clone(), &params);
        let results = backend.search("rust", 5, 0.0).await.unwrap();
        assert!(
            !results.is_empty(),
            "search must still return results during a compatible-partial state"
        );

        let idx = MemoryIndex::open_or_create(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            dims,
        )
        .unwrap();
        assert_eq!(
            idx.vec_row_count(),
            2,
            "ReadyMissing must backfill the missing chunk on search"
        );
        assert_eq!(
            idx.vec_row_count(),
            idx.chunk_count(),
            "vector set is complete after the self-heal"
        );
    }

    /// L1: the `ReadyMissing` incremental backfill is bounded per search with
    /// the same batch discipline as the rebuild loop (4 × 32 = 128). A large
    /// gap is processed across subsequent searches, never all at once, and a
    /// cap pause never arms the backfill backoff.
    #[tokio::test]
    async fn test_search_ready_missing_backfill_is_capped_per_search() {
        let tmp = TempDir::new().unwrap();
        init_sqlite_vec();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;

        // Install a fingerprint + one vector (chunk A).
        let spec = crate::retrieval::stub_spec(dims, "m");
        {
            let mut idx = MemoryIndex::open_or_create(
                &db_path,
                storage.clone(),
                xai_grok_config_types::MemoryIndexConfig::default(),
                dims,
            )
            .unwrap();
            let fa = tmp.path().join("a.md");
            std::fs::write(&fa, "# A\n\nRust a content.").unwrap();
            idx.reindex_file(&fa, "workspace").unwrap();
        }
        let install_embedder: Option<std::sync::Arc<dyn crate::embedding::EmbeddingProvider>> =
            Some(std::sync::Arc::new(
                crate::embedding::RetrievalEmbeddingProvider::new(std::sync::Arc::new(
                    crate::retrieval::FakeMemoryRetrieval::new(dims, "m"),
                )),
            ));
        let out = crate::rebuild::ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &spec,
            install_embedder,
            60,
            0,
            Some(usize::MAX),
            tokio_util::sync::CancellationToken::new(),
        )
        .await;
        assert!(
            matches!(out, crate::rebuild::VectorReadiness::Ready),
            "{out:?}"
        );

        // A large gap: 140 chunks in ONE file, none embedded → 141 chunks / 1 row.
        {
            let mut idx = MemoryIndex::open_or_create(
                &db_path,
                storage.clone(),
                xai_grok_config_types::MemoryIndexConfig::default(),
                dims,
            )
            .unwrap();
            let mut content = String::new();
            for i in 0..140 {
                content.push_str(&format!(
                    "## B{i}\n\nRust gap content {i}.\n\nPadding filler sentence so the \
                     document exceeds max_chunk_chars and the chunker splits by sections.\n\n"
                ));
            }
            let f = tmp.path().join("gap.md");
            std::fs::write(&f, &content).unwrap();
            idx.reindex_file(&f, "workspace").unwrap();
            assert_eq!(idx.chunk_count(), 141);
            assert_eq!(idx.vec_row_count(), 1);
            drop(idx);
        }

        let params = MemoryBackendParams {
            retrieval: Some(std::sync::Arc::new(
                crate::retrieval::FakeMemoryRetrieval::new(dims, "m"),
            )),
            watcher: None,
            ..make_params_fts_only("cap-test")
        };
        let backend = MemoryBackendImpl::from_session_params(storage.clone(), &params);

        // First search: the ReadyMissing backfill is capped at 4×32 = 128.
        let _ = backend.search("rust", 5, 0.0).await.unwrap();
        let idx = MemoryIndex::open_or_create(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            dims,
        )
        .unwrap();
        assert_eq!(
            idx.vec_row_count(),
            1 + 128,
            "cap must bound the synchronous ReadyMissing backfill per search"
        );
        assert_eq!(idx.vec_row_count(), idx.chunk_count() - 12);
        // Cap pause is not a failure: no backfill backoff is persisted.
        let backoff: String = idx
            .db()
            .query_row(
                crate::schema::GET_META_SQL,
                rusqlite::params![crate::schema::META_VECTOR_BACKFILL_BACKOFF_UNTIL],
                |r| r.get(0),
            )
            .unwrap_or_else(|_| "0".into());
        assert_eq!(backoff, "0", "cap pause must not arm the backfill backoff");
        drop(idx);

        // Second search resumes and completes the gap.
        let _ = backend.search("rust", 5, 0.0).await.unwrap();
        let idx = MemoryIndex::open_or_create(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            dims,
        )
        .unwrap();
        assert_eq!(idx.vec_row_count(), idx.chunk_count(), "gap healed");
        assert_eq!(idx.vec_row_count(), 141);
    }

    /// A genuine incremental-embed failure persists a backfill backoff
    /// (production-like 60s); a backed-off search does not re-attempt the gap
    /// (only the query embed runs), and once the deadline passes a healthy
    /// embedder self-heals. A cap pause never arms this backoff.
    #[tokio::test]
    async fn test_search_ready_missing_backfill_backs_off_after_failure() {
        let tmp = TempDir::new().unwrap();
        init_sqlite_vec();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;

        // Install fingerprint + vector for chunk A.
        let spec = crate::retrieval::stub_spec(dims, "m");
        {
            let mut idx = MemoryIndex::open_or_create(
                &db_path,
                storage.clone(),
                xai_grok_config_types::MemoryIndexConfig::default(),
                dims,
            )
            .unwrap();
            let fa = tmp.path().join("a.md");
            std::fs::write(&fa, "# A\n\nRust a content.").unwrap();
            idx.reindex_file(&fa, "workspace").unwrap();
        }
        let install_embedder: Option<std::sync::Arc<dyn crate::embedding::EmbeddingProvider>> =
            Some(std::sync::Arc::new(
                crate::embedding::RetrievalEmbeddingProvider::new(std::sync::Arc::new(
                    crate::retrieval::FakeMemoryRetrieval::new(dims, "m"),
                )),
            ));
        let out = crate::rebuild::ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &spec,
            install_embedder,
            60,
            0,
            Some(usize::MAX),
            tokio_util::sync::CancellationToken::new(),
        )
        .await;
        assert!(
            matches!(out, crate::rebuild::VectorReadiness::Ready),
            "{out:?}"
        );

        // One missing chunk (B) → ReadyMissing { missing: 1 }.
        {
            let mut idx = MemoryIndex::open_or_create(
                &db_path,
                storage.clone(),
                xai_grok_config_types::MemoryIndexConfig::default(),
                dims,
            )
            .unwrap();
            let fb = tmp.path().join("b.md");
            std::fs::write(&fb, "# B\n\nRust b content.").unwrap();
            idx.reindex_file(&fb, "workspace").unwrap();
            drop(idx);
        }

        // A genuinely failing embedder (production-like backoff 60s).
        let failing = std::sync::Arc::new(
            crate::retrieval::FakeMemoryRetrieval::new(dims, "m").with_embedment(|_| {
                Err(crate::retrieval::RetrievalError::new(
                    crate::retrieval::RetrievalErrorKind::Transient,
                ))
            }),
        );
        let params = MemoryBackendParams {
            retrieval: Some(failing.clone()),
            watcher: None,
            rebuild_backoff_secs: 60,
            vector_mirror: None,
            ..make_params_fts_only("backoff-test")
        };
        let backend = MemoryBackendImpl::from_session_params(storage.clone(), &params);

        // Search #1: incremental backfill fails → persisted backoff, gap stays.
        let _ = backend.search("rust", 5, 0.0).await.unwrap();
        let idx = MemoryIndex::open_or_create(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            dims,
        )
        .unwrap();
        assert_eq!(idx.vec_row_count(), 1, "failed backfill leaves the gap");
        let until: i64 = idx
            .db()
            .query_row(
                crate::schema::GET_META_SQL,
                rusqlite::params![crate::schema::META_VECTOR_BACKFILL_BACKOFF_UNTIL],
                |r| r.get::<_, String>(0),
            )
            .map(|s| s.trim().parse::<i64>().unwrap_or(0))
            .unwrap_or(0);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        assert!(
            until > now,
            "genuine failure must persist a backfill backoff"
        );
        drop(idx);

        // Search #2 (immediately): backoff active → gap is NOT re-attempted.
        let embed_calls_before = failing.embed_calls();
        let _ = backend.search("rust", 5, 0.0).await.unwrap();
        let idx = MemoryIndex::open_or_create(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            dims,
        )
        .unwrap();
        assert_eq!(
            idx.vec_row_count(),
            1,
            "backoff must suppress the gap retry"
        );
        drop(idx);
        assert_eq!(
            failing.embed_calls() - embed_calls_before,
            1,
            "a backed-off search must only run the query embed, never the gap backfill"
        );

        // Deadline passes (simulate by clearing the persisted value): a
        // healthy embedder self-heals the gap on the next search.
        {
            let idx = MemoryIndex::open_or_create(
                &db_path,
                storage.clone(),
                xai_grok_config_types::MemoryIndexConfig::default(),
                dims,
            )
            .unwrap();
            idx.db()
                .execute(
                    crate::schema::UPSERT_META_SQL,
                    rusqlite::params![crate::schema::META_VECTOR_BACKFILL_BACKOFF_UNTIL, "0"],
                )
                .unwrap();
            drop(idx);
        }
        let healthy_params = MemoryBackendParams {
            retrieval: Some(std::sync::Arc::new(
                crate::retrieval::FakeMemoryRetrieval::new(dims, "m"),
            )),
            watcher: None,
            ..make_params_fts_only("backoff-heal")
        };
        let healthy_backend =
            MemoryBackendImpl::from_session_params(storage.clone(), &healthy_params);
        let _ = healthy_backend.search("rust", 5, 0.0).await.unwrap();
        let idx = MemoryIndex::open_or_create(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            dims,
        )
        .unwrap();
        assert_eq!(
            idx.vec_row_count(),
            2,
            "gap self-heals once the embedder recovers"
        );
    }

    // -----------------------------------------------------------------------
    // Embedding-provider factory, malformed responses, and backfill behavior.
    // orphan-prune accuracy; watcher-dirty cap/backoff decoupling
    // -----------------------------------------------------------------------

    /// A named-profile facade is authoritative even in a mixed config.
    /// The factory
    /// never consults the legacy config or its chat credentials; embedding
    /// routes through the credential-free facade.
    #[tokio::test]
    async fn test_resolve_embedding_provider_facade_first() {
        let dims = 4;
        let fake = std::sync::Arc::new(crate::retrieval::FakeMemoryRetrieval::new(
            dims,
            "pinned-model",
        ));
        let params = MemoryBackendParams {
            retrieval: Some(fake.clone()),
            embed_config: Some(MemoryEmbeddingConfig {
                model: Some("legacy-model".into()),
                dimensions: dims,
                ..Default::default()
            }),
            embed_base_url: "http://chat.example/v1".into(),
            embed_api_key: Some("chat-secret".into()),
            ..make_params_fts_only("facade-first")
        };
        let provider = params.make_embedding_provider().await;
        assert!(provider.is_some(), "a present facade must win over legacy");
        let provider = provider.unwrap();
        assert_eq!(
            provider.model_name(),
            "pinned-model",
            "provider must route through the facade, not the legacy config"
        );
        // No legacy/chat call is issued: embedding goes through the fake.
        let before = fake.embed_calls();
        let v = provider.embed_batch(&["hello"]).await.unwrap();
        assert_eq!(fake.embed_calls() - before, 1);
        assert_eq!(v[0].len(), dims);
    }

    /// An unresolved named profile must be FTS-only.
    #[tokio::test]
    async fn test_resolve_embedding_provider_unresolved_named_is_none() {
        let params = MemoryBackendParams {
            retrieval: None,
            embed_config: None,
            ..make_params_fts_only("unresolved-named")
        };
        assert!(
            params.make_embedding_provider().await.is_none(),
            "unresolved named profile must be FTS-only (no legacy fallback)"
        );
        // The standalone factory behaves identically even when chat
        // credentials are offered.
        assert!(
            resolve_embedding_provider(
                None,
                None,
                &EndpointScopedCredentials::none(),
                Some("chat-key"),
                "http://chat.example/v1",
            )
            .await
            .is_none(),
            "unresolved named profile must never re-engage chat/legacy credentials"
        );
    }

    /// Legacy `[memory.embedding]` applies only when no named profile is set.
    #[tokio::test]
    async fn test_resolve_embedding_provider_legacy_only() {
        let params = MemoryBackendParams {
            embed_config: Some(MemoryEmbeddingConfig {
                model: Some("legacy-model".into()),
                dimensions: 4,
                ..Default::default()
            }),
            embed_base_url: "http://legacy.example/v1".into(),
            embed_api_key: Some("legacy-key".into()),
            ..make_params_fts_only("legacy-only")
        };
        let provider = params.make_embedding_provider().await;
        assert!(
            provider.is_some(),
            "legacy path applies when no named profile"
        );
        assert_eq!(provider.unwrap().model_name(), "legacy-model");
    }

    /// A malformed provider response (short count, NaN, wrong
    /// dimension) must fail closed before any SQLite write — never zip/
    /// mis-associate vectors onto the wrong chunks.
    #[tokio::test]
    async fn test_embed_missing_chunks_rejects_malformed_provider() {
        init_sqlite_vec();
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        let mut idx = MemoryIndex::open_or_create(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            dims,
        )
        .unwrap();
        for name in ["a.md", "b.md"] {
            let f = tmp.path().join(name);
            std::fs::write(&f, format!("# {name}\n\nRust {name} content.")).unwrap();
            idx.reindex_file(&f, "workspace").unwrap();
        }
        assert_eq!(idx.chunk_count(), 2);

        // Short response: 1 vector for 2 inputs must write nothing.
        struct ShortProvider;
        #[async_trait::async_trait]
        impl crate::embedding::EmbeddingProvider for ShortProvider {
            async fn embed_batch(
                &self,
                _texts: &[&str],
            ) -> Result<Vec<Vec<f32>>, Box<dyn std::error::Error>> {
                Ok(vec![vec![0.25; 4]])
            }
            fn model_name(&self) -> &str {
                "short"
            }
            fn dimensions(&self) -> usize {
                4
            }
        }
        assert_eq!(crate::embed_missing_chunks(&idx, &ShortProvider).await, 0);
        assert_eq!(idx.chunks_without_embeddings().unwrap().len(), 2);

        // NaN response must write nothing.
        struct NaNProvider;
        #[async_trait::async_trait]
        impl crate::embedding::EmbeddingProvider for NaNProvider {
            async fn embed_batch(
                &self,
                texts: &[&str],
            ) -> Result<Vec<Vec<f32>>, Box<dyn std::error::Error>> {
                Ok(texts
                    .iter()
                    .map(|_| vec![f32::NAN, 0.0, 0.0, 0.0])
                    .collect())
            }
            fn model_name(&self) -> &str {
                "nan"
            }
            fn dimensions(&self) -> usize {
                4
            }
        }
        assert_eq!(crate::embed_missing_chunks(&idx, &NaNProvider).await, 0);
        assert_eq!(idx.chunks_without_embeddings().unwrap().len(), 2);

        // Wrong-dimension response must write nothing.
        struct WrongDimProvider;
        #[async_trait::async_trait]
        impl crate::embedding::EmbeddingProvider for WrongDimProvider {
            async fn embed_batch(
                &self,
                texts: &[&str],
            ) -> Result<Vec<Vec<f32>>, Box<dyn std::error::Error>> {
                Ok(texts.iter().map(|_| vec![0.25; 8]).collect())
            }
            fn model_name(&self) -> &str {
                "wrongdim"
            }
            fn dimensions(&self) -> usize {
                4
            }
        }
        assert_eq!(
            crate::embed_missing_chunks(&idx, &WrongDimProvider).await,
            0
        );
        assert_eq!(idx.chunks_without_embeddings().unwrap().len(), 2);
    }

    /// F2: orphan prune reports the actual affected rows and is idempotent;
    /// valid (live) chunk vectors are never deleted.
    #[test]
    fn test_prune_orphan_vector_rows_reports_actual_removed() {
        init_sqlite_vec();
        let tmp = TempDir::new().unwrap();
        let storage = make_storage(&tmp);
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        {
            let mut idx = MemoryIndex::open_or_create(
                &db_path,
                storage.clone(),
                xai_grok_config_types::MemoryIndexConfig::default(),
                dims,
            )
            .unwrap();
            for name in ["a.md", "b.md", "c.md"] {
                let f = tmp.path().join(name);
                std::fs::write(&f, format!("# {name}\n\nRust {name} content.")).unwrap();
                idx.reindex_file(&f, "workspace").unwrap();
            }
            let ids: Vec<String> = idx
                .db()
                .prepare("SELECT id FROM chunks ORDER BY id")
                .unwrap()
                .query_map([], |r| r.get::<_, String>(0))
                .unwrap()
                .filter_map(Result::ok)
                .collect();
            assert_eq!(ids.len(), 3);
            for id in &ids {
                idx.upsert_embedding(id, &vec![0.25; dims]).unwrap();
            }
        }
        let idx = MemoryIndex::open_or_create(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            dims,
        )
        .unwrap();
        let ids: Vec<String> = idx
            .db()
            .prepare("SELECT id FROM chunks ORDER BY id")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .filter_map(Result::ok)
            .collect();
        assert_eq!(ids.len(), 3);
        idx.db()
            .execute(
                "DELETE FROM chunks WHERE id = ?1",
                rusqlite::params![&ids[0]],
            )
            .unwrap();
        idx.db()
            .execute(
                "DELETE FROM chunks WHERE id = ?1",
                rusqlite::params![&ids[1]],
            )
            .unwrap();
        assert_eq!(idx.vec_row_count(), 3);
        assert_eq!(idx.chunk_count(), 1);
        let removed = idx.prune_orphan_vector_rows();
        assert_eq!(removed, 2, "prune must report the actual rows deleted");
        assert_eq!(idx.vec_row_count(), 1);
        assert_eq!(idx.chunk_count(), 1);
        assert!(idx.chunks_without_embeddings().unwrap().is_empty());
        assert_eq!(idx.prune_orphan_vector_rows(), 0, "idempotent");
    }

    /// F3: the watcher-dirty incremental embed is bounded per search with the
    /// same cap as the compatible-gap backfill; a large watcher gap progresses
    /// over later searches and a cap pause never arms the backoff.
    #[tokio::test]
    async fn test_watcher_dirty_backfill_is_capped_per_search() {
        let tmp = TempDir::new().unwrap();
        init_sqlite_vec();
        let global = tmp.path().join("memory");
        let workspace = global.join("test_ws");
        std::fs::create_dir_all(&global).unwrap();
        std::fs::create_dir_all(&workspace).unwrap();
        let storage = MemoryStorage::with_paths(global.clone(), workspace.clone());
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        let watch_dir = dunce::canonicalize(&global).unwrap_or(global.clone());

        // Install a fingerprint + one vector (small file) so readiness is
        // Ready/ReadyMissing (never a rebuild) during the watcher sync.
        let small = watch_dir.join("small.md");
        std::fs::write(&small, "# Small\n\nRust small content.").unwrap();
        let small_canon = dunce::canonicalize(&small).unwrap_or(small);
        {
            let mut idx = MemoryIndex::open_or_create(
                &db_path,
                storage.clone(),
                xai_grok_config_types::MemoryIndexConfig::default(),
                dims,
            )
            .unwrap();
            idx.reindex_file(&small_canon, "global").unwrap();
        }
        let fake = std::sync::Arc::new(crate::retrieval::FakeMemoryRetrieval::new(dims, "m"));
        let install_embedder: Option<std::sync::Arc<dyn crate::embedding::EmbeddingProvider>> =
            Some(std::sync::Arc::new(
                crate::embedding::RetrievalEmbeddingProvider::new(fake.clone()),
            ));
        let out = crate::rebuild::ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &crate::retrieval::stub_spec(dims, "m"),
            install_embedder,
            60,
            0,
            Some(usize::MAX),
            tokio_util::sync::CancellationToken::new(),
        )
        .await;
        assert!(
            matches!(out, crate::rebuild::VectorReadiness::Ready),
            "{out:?}"
        );
        assert_eq!(
            MemoryIndex::open_or_create(
                &db_path,
                storage.clone(),
                xai_grok_config_types::MemoryIndexConfig::default(),
                dims,
            )
            .unwrap()
            .vec_row_count(),
            1
        );

        // Watcher on the canonical global dir, then a big file (140 chunks).
        let watcher = match crate::watcher::MemoryFileWatcher::start(&watch_dir) {
            Some(w) => w,
            None => return, // file watching unsupported here
        };
        let mut big = String::new();
        for i in 0..140 {
            big.push_str(&format!("## B{i}\n\nRust gap content {i}.\n\n"));
        }
        std::fs::write(watch_dir.join("big.md"), &big).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(600));

        let params = MemoryBackendParams {
            retrieval: Some(fake),
            watcher: Some(std::sync::Arc::new(watcher)),
            ..make_params_fts_only("watcher-cap")
        };
        let backend = MemoryBackendImpl::from_session_params(storage.clone(), &params);

        // Search #1: watcher-dirty sync embeds at most the per-search cap
        // (128 chunks); a cap pause never arms the backoff.
        let _ = backend.search("rust", 5, 0.0).await.unwrap();
        let idx = MemoryIndex::open_or_create(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            dims,
        )
        .unwrap();
        assert_eq!(
            idx.vec_row_count(),
            1 + 128,
            "watcher-dirty incremental embed must be capped per search"
        );
        let backoff: String = idx
            .db()
            .query_row(
                crate::schema::GET_META_SQL,
                rusqlite::params![crate::schema::META_VECTOR_BACKFILL_BACKOFF_UNTIL],
                |r| r.get(0),
            )
            .unwrap_or_else(|_| "0".into());
        assert_eq!(backoff, "0", "cap pause must not arm the backfill backoff");
        drop(idx);

        // Search #2: the remaining gap heals through the ReadyMissing path.
        let _ = backend.search("rust", 5, 0.0).await.unwrap();
        let idx = MemoryIndex::open_or_create(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            dims,
        )
        .unwrap();
        assert_eq!(
            idx.vec_row_count(),
            141,
            "watcher gap heals over later searches"
        );
        assert_eq!(idx.vec_row_count(), idx.chunk_count());
    }

    /// A compatible-gap backoff defers only the same missing set — a
    /// freshly changed watcher-dirty chunk still embeds during the window.
    #[tokio::test]
    async fn test_watcher_dirty_backfill_proceeds_during_gap_backoff() {
        let tmp = TempDir::new().unwrap();
        init_sqlite_vec();
        let global = tmp.path().join("memory");
        let workspace = global.join("test_ws");
        std::fs::create_dir_all(&global).unwrap();
        std::fs::create_dir_all(&workspace).unwrap();
        let storage = MemoryStorage::with_paths(global.clone(), workspace.clone());
        let db_path = storage.workspace_dir().join("index.sqlite");
        let dims = 4;
        let watch_dir = dunce::canonicalize(&global).unwrap_or(global.clone());

        // Install fingerprint + 1 vector (small file).
        let small = watch_dir.join("small.md");
        std::fs::write(&small, "# Small\n\nRust small content.").unwrap();
        let small_canon = dunce::canonicalize(&small).unwrap_or(small);
        {
            let mut idx = MemoryIndex::open_or_create(
                &db_path,
                storage.clone(),
                xai_grok_config_types::MemoryIndexConfig::default(),
                dims,
            )
            .unwrap();
            idx.reindex_file(&small_canon, "global").unwrap();
        }
        let fake = std::sync::Arc::new(crate::retrieval::FakeMemoryRetrieval::new(dims, "m"));
        let install_embedder: Option<std::sync::Arc<dyn crate::embedding::EmbeddingProvider>> =
            Some(std::sync::Arc::new(
                crate::embedding::RetrievalEmbeddingProvider::new(fake.clone()),
            ));
        let out = crate::rebuild::ensure_vectors_ready(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            &crate::retrieval::stub_spec(dims, "m"),
            install_embedder,
            60,
            0,
            Some(usize::MAX),
            tokio_util::sync::CancellationToken::new(),
        )
        .await;
        assert!(
            matches!(out, crate::rebuild::VectorReadiness::Ready),
            "{out:?}"
        );

        // A pre-existing compatible gap: chunk gap.md without a vector.
        {
            let mut idx = MemoryIndex::open_or_create(
                &db_path,
                storage.clone(),
                xai_grok_config_types::MemoryIndexConfig::default(),
                dims,
            )
            .unwrap();
            let f = watch_dir.join("gap.md");
            std::fs::write(&f, "# Gap\n\nRust gap chunk.").unwrap();
            let f = dunce::canonicalize(&f).unwrap_or(f);
            idx.reindex_file(&f, "global").unwrap();
        }
        // Prime the compatible-gap backoff (now + 3600).
        {
            let idx = MemoryIndex::open_or_create(
                &db_path,
                storage.clone(),
                xai_grok_config_types::MemoryIndexConfig::default(),
                dims,
            )
            .unwrap();
            let until = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64
                + 3600;
            idx.db()
                .execute(
                    crate::schema::UPSERT_META_SQL,
                    rusqlite::params![
                        crate::schema::META_VECTOR_BACKFILL_BACKOFF_UNTIL,
                        until.to_string()
                    ],
                )
                .unwrap();
            drop(idx);
        }

        // Watcher adds a NEW file (fresh work) during the backoff window.
        let watcher = match crate::watcher::MemoryFileWatcher::start(&watch_dir) {
            Some(w) => w,
            None => return,
        };
        std::fs::write(watch_dir.join("fresh.md"), "# Fresh\n\nRust fresh content.").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(600));

        let params = MemoryBackendParams {
            retrieval: Some(fake),
            watcher: Some(std::sync::Arc::new(watcher)),
            ..make_params_fts_only("watcher-during-backoff")
        };
        let backend = MemoryBackendImpl::from_session_params(storage.clone(), &params);
        let _ = backend.search("rust", 5, 0.0).await.unwrap();

        let idx = MemoryIndex::open_or_create(
            &db_path,
            storage.clone(),
            xai_grok_config_types::MemoryIndexConfig::default(),
            dims,
        )
        .unwrap();
        assert_eq!(
            idx.vec_row_count(),
            2,
            "fresh watcher work must embed even during a gap backoff"
        );
        // The same missing set (gap.md chunk) stays deferred.
        assert_eq!(
            idx.chunks_without_embeddings().unwrap().len(),
            1,
            "the deferred gap chunk must remain unembedded"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{MemoryIndex, init_sqlite_vec};
    use tempfile::TempDir;
    use xai_grok_config_types::MemoryIndexConfig;

    /// An api-key provider that fails the test if its key is ever resolved,
    /// proving a scoped-away credential is never consulted.
    struct PanicKey;
    impl xai_grok_tools::types::ApiKeyProvider for PanicKey {
        fn current_api_key(&self) -> Option<String> {
            panic!("scoped-away credential must not be resolved");
        }
    }

    fn setup_index(tmp: &TempDir) -> (PathBuf, MemoryStorage) {
        init_sqlite_vec();
        let global = tmp.path().join("memory");
        let workspace = global.join("test_ws");
        let storage = MemoryStorage::with_paths(global, workspace);
        let db_path = tmp.path().join("test.sqlite");

        let mut idx =
            MemoryIndex::open_or_create(&db_path, storage.clone(), MemoryIndexConfig::default(), 4)
                .unwrap();

        let file_path = tmp.path().join("test.md");
        std::fs::write(&file_path, "# Guide\n\nRust programming tutorial.").unwrap();
        idx.reindex_file(&file_path, "workspace").unwrap();

        (db_path, storage)
    }

    #[tokio::test]
    async fn test_backend_search() {
        let tmp = TempDir::new().unwrap();
        let (db_path, storage) = setup_index(&tmp);
        let backend = MemoryBackendImpl::new(db_path, storage);

        let results = backend.search("rust programming", 10, 0.0).await.unwrap();
        assert!(!results.is_empty(), "should find indexed content");
        assert!(results[0].snippet.contains("Rust"));
    }

    #[test]
    fn test_backend_total_chunks() {
        let tmp = TempDir::new().unwrap();
        let (db_path, storage) = setup_index(&tmp);
        let backend = MemoryBackendImpl::new(db_path, storage);

        let count = backend.total_chunks().unwrap();
        assert!(count >= 1, "should have at least 1 chunk");
    }

    #[test]
    fn test_backend_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MemoryBackendImpl>();
    }

    /// If credentials approved for one endpoint are used to build against a
    /// different URL (a wiring bug), they are dropped at build time rather than
    /// sent to the wrong endpoint. The session provider would panic if resolved.
    #[tokio::test]
    async fn test_build_drops_credentials_when_request_url_differs() {
        let session: xai_grok_tools::types::SharedApiKeyProvider = Arc::new(PanicKey);

        let scoped = EndpointScopedCredentials::for_endpoint(
            "https://api.x.ai/v1",
            |_| true,
            None,
            Some(session),
        );
        assert!(!scoped.is_empty(), "trusted endpoint keeps the credential");

        let config = xai_grok_config_types::MemoryEmbeddingConfig {
            model: Some("test-embedding-model".to_string()),
            ..Default::default()
        };
        let provider = build_embedding_provider(
            Some(&config),
            &scoped,
            Some("byok-static-key"),
            "https://other.example/v1",
        )
        .await;
        assert!(
            provider.is_some(),
            "mismatched request URL must fall back to the static key, not the scoped credential"
        );
    }

    /// A trusted, URL-matching endpoint builds the provider from the
    /// refresh-capable session credential and never consults the per-call
    /// api-key provider. The api-key provider panics if resolved.
    #[tokio::test]
    async fn test_trusted_endpoint_prefers_session_credential() {
        struct StubAuth;
        impl xai_grok_auth::HttpAuth for StubAuth {
            fn apply(
                &self,
                builder: reqwest::RequestBuilder,
                _base_url: &str,
            ) -> reqwest::RequestBuilder {
                builder
            }
        }
        #[async_trait::async_trait]
        impl xai_grok_auth::AuthCredentialProvider for StubAuth {
            fn snapshot(&self) -> xai_grok_auth::CredentialSnapshot {
                xai_grok_auth::CredentialSnapshot::default()
            }
            async fn refresh_after_unauthorized(&self) -> bool {
                false
            }
        }

        let auth: Arc<dyn xai_grok_auth::AuthCredentialProvider> = Arc::new(StubAuth);
        let api_key: xai_grok_tools::types::SharedApiKeyProvider = Arc::new(PanicKey);
        let scoped = EndpointScopedCredentials::for_endpoint(
            "https://api.x.ai/v1",
            |_| true,
            Some(auth),
            Some(api_key),
        );
        assert!(!scoped.is_empty(), "trusted endpoint keeps the credential");

        let config = xai_grok_config_types::MemoryEmbeddingConfig {
            model: Some("test-embedding-model".to_string()),
            ..Default::default()
        };
        let provider =
            build_embedding_provider(Some(&config), &scoped, None, "https://api.x.ai/v1").await;
        assert!(
            provider.is_some(),
            "trusted endpoint must build a provider from the session credential"
        );
    }

    #[test]
    fn endpoint_scoped_credentials_trust_gate_and_url_match() {
        struct AnyKey;
        impl xai_grok_tools::types::ApiKeyProvider for AnyKey {
            fn current_api_key(&self) -> Option<String> {
                None
            }
        }
        let key = || Arc::new(AnyKey) as xai_grok_tools::types::SharedApiKeyProvider;

        let denied = EndpointScopedCredentials::for_endpoint(
            "https://byok.example/v1",
            |_| false,
            None,
            Some(key()),
        );
        assert!(denied.is_empty(), "untrusted endpoint drops the credential");

        let scoped = EndpointScopedCredentials::for_endpoint(
            "https://api.x.ai/v1",
            |_| true,
            None,
            Some(key()),
        );
        assert!(!scoped.is_empty(), "trusted endpoint keeps the credential");
        assert!(
            scoped.approved_for("https://API.x.ai/v1"),
            "host casing normalizes"
        );
        assert!(
            !scoped.approved_for("https://api.x.ai/v2"),
            "different path rejected"
        );
        assert!(
            !scoped.approved_for("https://other.example/v1"),
            "different host rejected"
        );
        assert!(!scoped.approved_for("not-a-url"), "unparsable fails closed");
    }

    #[tokio::test]
    async fn test_search_with_punctuation_in_query() {
        let tmp = TempDir::new().unwrap();
        let (db_path, storage) = setup_index(&tmp);
        let backend = MemoryBackendImpl::new(db_path, storage);

        // Raw user message with punctuation — should not crash FTS5
        let results = backend
            .search("what is rust? how to use it!", 10, 0.0)
            .await
            .unwrap();
        assert!(
            !results.is_empty(),
            "should match 'rust' despite punctuation in query"
        );
    }

    #[tokio::test]
    async fn test_search_with_special_chars_only() {
        let tmp = TempDir::new().unwrap();
        let (db_path, storage) = setup_index(&tmp);
        let backend = MemoryBackendImpl::new(db_path, storage);

        // Query with only special chars — should return empty, not error
        let results = backend.search("???!!!", 10, 0.0).await.unwrap();
        assert!(
            results.is_empty(),
            "special-chars-only query should return empty"
        );
    }

    #[tokio::test]
    async fn test_search_hybrid_fts_only_fallback() {
        // Without embedding config, hybrid search should degrade to FTS-only
        let tmp = TempDir::new().unwrap();
        let (db_path, storage) = setup_index(&tmp);
        let backend = MemoryBackendImpl::new(db_path, storage);

        // Even with high min_score, hybrid search normalizes scores to [0,1]
        // so results above the threshold should be returned
        let results = backend.search("rust programming", 10, 0.0).await.unwrap();
        assert!(
            !results.is_empty(),
            "FTS-only fallback should still return results"
        );
        // Scores should be normalized (0,1] range from hybrid scoring
        assert!(results[0].score > 0.0, "hybrid scores should be positive");
    }

    /// The supplemental evergreen query in `search()` adds global/workspace
    /// candidates that the base `search_fts` missed due to candidate_limit.
    ///
    /// Tests the mechanism directly at the index level: verifies that with
    /// a tight FTS limit, global/workspace chunks are absent from the base
    /// results but present in the supplemental source-filtered query. Then
    /// confirms the full backend search pipeline surfaces them.
    #[tokio::test]
    async fn test_search_returns_global_and_workspace_memory() {
        let tmp = TempDir::new().unwrap();
        init_sqlite_vec();
        let global = tmp.path().join("memory");
        let workspace = global.join("test_ws");
        let storage = MemoryStorage::with_paths(global, workspace);
        let db_path = storage.workspace_dir().join("index.sqlite");

        let mut idx =
            MemoryIndex::open_or_create(&db_path, storage.clone(), MemoryIndexConfig::default(), 4)
                .unwrap();

        // Index global + workspace with matching content.
        let global_file = tmp.path().join("global_mem.md");
        std::fs::write(
            &global_file,
            "# Preferences\n\nAlways use graphite for PRs. Prefer Rust over Python.",
        )
        .unwrap();
        idx.reindex_file(&global_file, "global").unwrap();

        let ws_file = tmp.path().join("ws_mem.md");
        std::fs::write(
            &ws_file,
            "# Project Decisions\n\nWe chose graphite for PRs in this project.",
        )
        .unwrap();
        idx.reindex_file(&ws_file, "workspace").unwrap();

        // Index session files that also match the query.
        for i in 0..5 {
            let f = tmp.path().join(format!("session_{i}.md"));
            std::fs::write(
                &f,
                format!("# Session {i}\n\nDiscussed graphite for PRs and item {i}."),
            )
            .unwrap();
            idx.reindex_file(&f, "session").unwrap();
        }

        // Verify the supplemental query mechanism: with a tight limit the
        // base FTS returns a mix, but `search_fts_by_sources` for
        // "global"/"workspace" always finds the evergreen chunks.
        let evergreen = idx
            .search_fts_by_sources("graphite PRs", 10, &["global", "workspace"])
            .unwrap();
        assert!(
            evergreen.len() >= 2,
            "supplemental evergreen query must find both global and workspace chunks"
        );
        let evergreen_sources: Vec<String> = evergreen
            .iter()
            .filter_map(|r| idx.get_chunk(&r.chunk_id).ok().flatten())
            .map(|c| c.source)
            .collect();
        assert!(
            evergreen_sources.contains(&"global".to_string()),
            "evergreen query must find global chunk"
        );
        assert!(
            evergreen_sources.contains(&"workspace".to_string()),
            "evergreen query must find workspace chunk"
        );
        drop(idx);

        // Full backend search: global/workspace must appear in results.
        let backend = MemoryBackendImpl::new(db_path, storage);
        let results = backend.search("graphite PRs", 10, 0.0).await.unwrap();

        let has_global = results.iter().any(|r| r.source == "global");
        let has_workspace = results.iter().any(|r| r.source == "workspace");
        assert!(
            has_global,
            "global MEMORY.md chunks must appear in search results"
        );
        assert!(
            has_workspace,
            "workspace MEMORY.md chunks must appear in search results"
        );
    }

    // -----------------------------------------------------------------------
    // Canonical embedding endpoint identity.
    // -----------------------------------------------------------------------

    /// Legacy synthesis and the named-profile facade must produce the same
    /// canonical embedding path (no `/v1/embeddings` vs `/embeddings` drift).
    #[test]
    fn test_legacy_embedding_path_is_canonical() {
        let cfg = xai_grok_config_types::MemoryEmbeddingConfig {
            model: Some("embed-model".into()),
            dimensions: 128,
            ..Default::default()
        };
        let spec = legacy_embedding_source_spec(&cfg, "http://proxy.example/v1").unwrap();
        assert_eq!(
            spec.embedding_path,
            canonical_embedding_path(),
            "legacy synthesis must use the canonical embedding path"
        );
        assert_eq!(spec.origin_host, "proxy.example");
        assert_eq!(spec.dimensions, 128);
    }

    /// A legacy source and an equivalent named-profile source (same host,
    /// model, dims, canonical path) fingerprint identically apart from the
    /// provider-instance label — so a mode switch between equivalent physical
    /// endpoints does not spuriously rebuild.
    #[test]
    fn test_legacy_named_equivalent_source_no_spurious_identity_gap() {
        use crate::fingerprint::EmbeddingSourceSpec;
        let legacy_cfg = xai_grok_config_types::MemoryEmbeddingConfig {
            model: Some("embed-model".into()),
            dimensions: 128,
            ..Default::default()
        };
        let legacy =
            legacy_embedding_source_spec(&legacy_cfg, "http://api.example.com/v1").unwrap();

        // A named-profile route for the same provider host/model/dims/path.
        let named = EmbeddingSourceSpec {
            provider_instance_id: "acct-a".into(),
            incarnation: Some("inc-1".into()),
            origin_host: "api.example.com".into(),
            embedding_path: canonical_embedding_path(),
            protocol: "openai_compatible".into(),
            model: "embed-model".into(),
            dimensions: 128,
            encoding: "float".into(),
            normalization: crate::fingerprint::NORMALIZATION_NONE.into(),
        };
        assert_eq!(legacy.origin_host, named.origin_host);
        assert_eq!(legacy.embedding_path, named.embedding_path);
        assert_eq!(legacy.model, named.model);
        assert_eq!(legacy.dimensions, named.dimensions);
        assert_eq!(legacy.protocol, named.protocol);
        assert_eq!(legacy.encoding, named.encoding);

        // Equalize the provider label: the two fingerprints must be identical.
        let mut eq_legacy = legacy;
        eq_legacy.provider_instance_id = "acct-a".into();
        eq_legacy.incarnation = Some("inc-1".into());
        let fp_l = crate::fingerprint::VectorFingerprint::build(
            eq_legacy,
            crate::fingerprint::DocPreparationSpec::from_index_config(
                &xai_grok_config_types::MemoryIndexConfig::default(),
            ),
            crate::fingerprint::VECTOR_SCHEMA_VERSION,
        )
        .unwrap()
        .0;
        let fp_n = crate::fingerprint::VectorFingerprint::build(
            named,
            crate::fingerprint::DocPreparationSpec::from_index_config(
                &xai_grok_config_types::MemoryIndexConfig::default(),
            ),
            crate::fingerprint::VECTOR_SCHEMA_VERSION,
        )
        .unwrap()
        .0;
        assert_eq!(
            fp_l.hash, fp_n.hash,
            "equivalent physical sources must share a fingerprint (no spurious rebuild)"
        );
    }
}

#[cfg(test)]
mod index_embedding_tests {
    use crate::index::MemoryIndex;
    use crate::mirror::{MirrorError, MirrorHandle, VectorMirror};
    use crate::storage::MemoryStorage;
    use std::sync::{Arc, Mutex};

    /// Fingerprint hash installed by the test fixture (matches the shape
    /// the rebuild state machine persists).
    const FP: &str = "0123456789abcdef0123456789abcdef";

    /// Recording in-memory mirror: captures resync upserts for assertions.
    #[derive(Default)]
    struct RecordingMirror {
        upserts: Mutex<Vec<(Vec<String>, String)>>,
    }

    impl RecordingMirror {
        fn upserted_ids(&self) -> Vec<String> {
            self.upserts
                .lock()
                .unwrap()
                .iter()
                .flat_map(|(ids, _)| ids.iter().cloned())
                .collect()
        }

        fn fingerprints(&self) -> Vec<String> {
            self.upserts
                .lock()
                .unwrap()
                .iter()
                .map(|(_, fp)| fp.clone())
                .collect()
        }
    }

    #[async_trait::async_trait]
    impl VectorMirror for RecordingMirror {
        fn backend_id(&self) -> &str {
            "recording"
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
            self.upserts
                .lock()
                .unwrap()
                .push((ids.to_vec(), fingerprint_hash.to_owned()));
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
            let upserts = self.upserts.lock().unwrap();
            Ok(upserts.iter().map(|(ids, _)| ids.len()).sum::<usize>() as u64)
        }

        async fn drop_collection(&self, _name: &str) -> Result<(), MirrorError> {
            Ok(())
        }
    }

    /// Deterministic provider: every text maps to the same 4-dim vector.
    struct GoodProvider;

    #[async_trait::async_trait]
    impl crate::embedding::EmbeddingProvider for GoodProvider {
        async fn embed_batch(
            &self,
            texts: &[&str],
        ) -> Result<Vec<Vec<f32>>, Box<dyn std::error::Error>> {
            Ok(texts.iter().map(|_| vec![0.25f32; 4]).collect())
        }
        fn model_name(&self) -> &str {
            "good"
        }
        fn dimensions(&self) -> usize {
            4
        }
    }

    /// Regression: with zero missing chunks (steady state) the mirror
    /// fan-out must still run, so a not-yet-populated mirror is healed
    /// from the SQLite vec table on the next embed pass instead of
    /// staying empty forever.
    #[tokio::test]
    async fn test_steady_state_embed_pass_still_populates_mirror() {
        crate::index::init_sqlite_vec();
        let tmp = tempfile::TempDir::new().unwrap();
        let global = tmp.path().join("memory");
        let workspace = global.join("test_ws");
        let storage = MemoryStorage::with_paths(global, workspace);
        // Must match `MemoryIndex::db_path()` (storage workspace layout) —
        // the resync source derives its connection from that path.
        let db_path = storage.workspace_dir().join("index.sqlite");

        let mut idx = MemoryIndex::open_or_create(
            &db_path,
            storage,
            xai_grok_config_types::MemoryIndexConfig::default(),
            4,
        )
        .unwrap();

        if !idx.vec_available() {
            // Without sqlite-vec there is no `chunks_vec` table to drain;
            // the mirror falls back by design and this test is vacuous.
            return;
        }

        let file_path = tmp.path().join("test.md");
        std::fs::write(&file_path, "# Title\n\nSteady-state mirror content.").unwrap();
        idx.reindex_file(&file_path, "workspace").unwrap();

        // Install a pinned vector space the way the rebuild state machine
        // does, then embed the chunk: one row in `chunks_vec`, zero chunks
        // missing embeddings.
        idx.db()
            .execute(
                "INSERT OR REPLACE INTO meta(key, value) VALUES (?1, ?2), (?3, ?4)",
                rusqlite::params![
                    crate::schema::META_VECTOR_FINGERPRINT_HASH,
                    FP,
                    crate::schema::META_VECTOR_SCHEMA_VERSION,
                    "1"
                ],
            )
            .unwrap();

        assert_eq!(crate::embed_missing_chunks(&idx, &GoodProvider).await, 1);
        assert!(idx.chunks_without_embeddings().unwrap().is_empty());
        assert_eq!(idx.vec_row_count(), 1);

        // Fresh (unready) mirror + steady-state pass: nothing to embed,
        // yet the fan-out must resync the row from SQLite into the mirror.
        let mirror = Arc::new(RecordingMirror::default());
        let handle = MirrorHandle::new(mirror.clone(), "grok_mem_test");
        let embedded =
            crate::embed_missing_chunks_with_mirror(&idx, &GoodProvider, Some(&handle)).await;
        assert_eq!(embedded, 0, "steady state: no chunk needed embedding");
        assert_eq!(
            mirror.upserted_ids().len(),
            1,
            "mirror must be populated by the steady-state pass"
        );
        assert_eq!(mirror.fingerprints(), vec![FP.to_owned()]);
        assert!(
            handle.is_ready_for_count(FP, 4, 1),
            "resync verified: mirror ready for the installed space"
        );
    }

    #[tokio::test]
    async fn test_reconcile_milvus_mode_lifecycle() {
        let tmp = tempfile::TempDir::new().unwrap();
        let global = tmp.path().join("memory");
        let workspace = global.join("test_ws");
        let storage = MemoryStorage::with_paths(global, workspace);
        let db_path = storage.workspace_dir().join("index.sqlite");

        let mut idx = MemoryIndex::open_or_create(
            &db_path,
            storage,
            xai_grok_config_types::MemoryIndexConfig::default(),
            4,
        )
        .unwrap();

        let file1 = tmp.path().join("test1.md");
        let file2 = tmp.path().join("test2.md");
        std::fs::write(&file1, "# Section 1\n\nFirst chunk content.").unwrap();
        std::fs::write(&file2, "# Section 2\n\nSecond chunk content.").unwrap();
        idx.reindex_file(&file1, "workspace").unwrap();
        idx.reindex_file(&file2, "workspace").unwrap();

        let mirror = Arc::new(crate::mirror::InMemoryVectorMirror::new());
        let handle = MirrorHandle::new(mirror.clone(), "grok_mem_milvus_test");

        // First reconcile: 2 chunks embedded and upserted
        let report1 = crate::reconcile_milvus_mode(&mut idx, &GoodProvider, &handle, FP)
            .await
            .expect("first reconcile succeeds");
        assert_eq!(report1.embedded, 2);
        assert_eq!(report1.unchanged, 0);
        assert_eq!(report1.deleted, 0);
        assert_eq!(report1.total, 2);
        assert!(handle.is_ready_for(FP, 4));

        // Second reconcile (steady state): unchanged chunks skipped, 0 embedded!
        let report2 = crate::reconcile_milvus_mode(&mut idx, &GoodProvider, &handle, FP)
            .await
            .expect("second reconcile succeeds");
        assert_eq!(report2.embedded, 0);
        assert_eq!(report2.unchanged, 2);
        assert_eq!(report2.deleted, 0);
        assert_eq!(report2.total, 2);

        // Edit one file
        std::fs::write(&file1, "# Section 1\n\nFirst chunk content UPDATED.").unwrap();
        idx.reindex_file(&file1, "workspace").unwrap();

        // Third reconcile: 1 embedded (changed), 1 unchanged
        let report3 = crate::reconcile_milvus_mode(&mut idx, &GoodProvider, &handle, FP)
            .await
            .expect("third reconcile succeeds");
        assert_eq!(report3.embedded, 1);
        assert_eq!(report3.unchanged, 1);
        assert_eq!(report3.deleted, 0);

        // Delete file2 and reindex
        std::fs::remove_file(&file2).unwrap();
        idx.delete_path_ids(&file2).unwrap();

        // Fourth reconcile: 0 embedded, 1 unchanged (file1), 1 deleted (file2)
        let report4 = crate::reconcile_milvus_mode(&mut idx, &GoodProvider, &handle, FP)
            .await
            .expect("fourth reconcile succeeds");
        assert_eq!(report4.embedded, 0);
        assert_eq!(report4.unchanged, 1);
        assert_eq!(report4.deleted, 1);
        assert_eq!(report4.total, 1);
    }

    #[tokio::test]
    async fn test_drain_local_to_milvus_preserves_local_and_populates_remote() {
        crate::index::init_sqlite_vec();
        let tmp = tempfile::TempDir::new().unwrap();
        let global = tmp.path().join("memory");
        let workspace = global.join("test_ws");
        let storage = MemoryStorage::with_paths(global, workspace);
        let db_path = storage.workspace_dir().join("index.sqlite");

        let mut idx = MemoryIndex::open_or_create(
            &db_path,
            storage,
            xai_grok_config_types::MemoryIndexConfig::default(),
            4,
        )
        .unwrap();

        if !idx.vec_available() {
            return;
        }

        let file = tmp.path().join("local_note.md");
        std::fs::write(&file, "# Local Memory\n\nExisting local chunk content.").unwrap();
        idx.reindex_file(&file, "workspace").unwrap();

        // Install local fingerprint and embed locally
        idx.db()
            .execute(
                "INSERT OR REPLACE INTO meta(key, value) VALUES (?1, ?2), (?3, ?4)",
                rusqlite::params![
                    crate::schema::META_VECTOR_FINGERPRINT_HASH,
                    FP,
                    crate::schema::META_VECTOR_SCHEMA_VERSION,
                    "1"
                ],
            )
            .unwrap();
        assert_eq!(crate::embed_missing_chunks(&idx, &GoodProvider).await, 1);
        assert_eq!(idx.vec_row_count(), 1);

        // Switch to milvus mode: drain local SQLite rows into Milvus
        let mirror = Arc::new(crate::mirror::InMemoryVectorMirror::new());
        let handle = MirrorHandle::new(mirror.clone(), "grok_mem_drain_test");

        let report = crate::drain_local_to_milvus(&mut idx, None, &handle, FP, 4)
            .await
            .expect("drain local to milvus succeeds");
        assert_eq!(report.unchanged, 1, "1 local chunk drained without re-embedding");
        assert_eq!(report.embedded, 0);
        assert!(handle.is_ready_for(FP, 4));

        // Milvus remote collection now has the chunk
        assert_eq!(mirror.count("grok_mem_drain_test", FP).await.unwrap(), 1);

        // Local SQLite tables are completely intact (non-destructive)
        assert_eq!(idx.vec_row_count(), 1);
        assert_eq!(idx.all_chunks().unwrap().len(), 1);
    }

    #[test]
    fn test_chunks_without_embeddings() {
        let tmp = tempfile::TempDir::new().unwrap();
        let global = tmp.path().join("memory");
        let workspace = global.join("test_ws");
        let storage = MemoryStorage::with_paths(global, workspace);
        let db_path = tmp.path().join("test.sqlite");

        let mut idx = MemoryIndex::open_or_create(
            &db_path,
            storage,
            xai_grok_config_types::MemoryIndexConfig::default(),
            4,
        )
        .unwrap();

        if !idx.vec_available() {
            // sqlite-vec not available — chunks_without_embeddings returns empty
            let missing = idx.chunks_without_embeddings().unwrap();
            assert!(missing.is_empty(), "no-vec: should return empty");
            return;
        }

        let file_path = tmp.path().join("test.md");
        std::fs::write(&file_path, "# Title\n\nSome content here.").unwrap();
        idx.reindex_file(&file_path, "workspace").unwrap();

        // After reindex, chunks should exist but have no embeddings
        let missing = idx.chunks_without_embeddings().unwrap();
        assert!(
            !missing.is_empty(),
            "newly indexed chunks should be missing embeddings"
        );

        // After upserting an embedding, the chunk should disappear from missing
        let (chunk_id, _) = &missing[0];
        let dummy_embedding = vec![0.0f32; 4];
        idx.upsert_embedding(chunk_id, &dummy_embedding).unwrap();

        let missing_after = idx.chunks_without_embeddings().unwrap();
        assert_eq!(
            missing_after.len(),
            missing.len() - 1,
            "one fewer chunk should be missing after embedding"
        );
    }
}
