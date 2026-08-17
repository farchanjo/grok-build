//! Exact-route client traits over PR16 adapters.
//!
//! Credentials are resolved at each call (never stored in snapshot/cache).
//! Tests inject [`FakeRetrievalExecutor`] without live HTTP.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use parking_lot::Mutex;
use tokio_util::sync::CancellationToken;
use xai_grok_config_types::{EmbeddingModelConfig, RerankerModelConfig};
use xai_grok_inference::{
    EmbeddingResult, EmbeddingVector, RerankHit, RerankResult, RetrievalError, RetrievalResult,
};

use crate::provider_registry::{
    RetrievalResolveOptions, embed_with_runtime, rerank_with_runtime, resolve_embedding_runtime,
    resolve_reranker_runtime,
};

/// Per-call resolution pins for exact routes.
#[derive(Debug, Clone, Default)]
pub struct RouteCallPins {
    pub provenance_incarnation: Option<String>,
    pub session_registry_generation: Option<u64>,
    pub total_deadline: Option<Duration>,
}

/// Credential-explicit retrieval executor (PR16-backed by default).
#[async_trait]
pub trait RetrievalExecutor: Send + Sync + 'static {
    async fn embed(
        &self,
        home: &Path,
        model_id: &str,
        config: &EmbeddingModelConfig,
        pins: &RouteCallPins,
        inputs: Vec<String>,
        cancel: CancellationToken,
    ) -> RetrievalResult<EmbeddingResult>;

    async fn rerank(
        &self,
        home: &Path,
        model_id: &str,
        config: &RerankerModelConfig,
        pins: &RouteCallPins,
        query: String,
        documents: Vec<String>,
        top_n: Option<u32>,
        cancel: CancellationToken,
    ) -> RetrievalResult<RerankResult>;
}

/// Production executor: PR16 exact resolve + adapter call each attempt.
#[derive(Debug, Default)]
pub struct Pr16RetrievalExecutor;

#[async_trait]
impl RetrievalExecutor for Pr16RetrievalExecutor {
    async fn embed(
        &self,
        home: &Path,
        _model_id: &str,
        config: &EmbeddingModelConfig,
        pins: &RouteCallPins,
        inputs: Vec<String>,
        cancel: CancellationToken,
    ) -> RetrievalResult<EmbeddingResult> {
        let opts = RetrievalResolveOptions {
            provenance_incarnation: pins.provenance_incarnation.as_deref(),
            session_registry_generation: pins.session_registry_generation,
            is_retry: false,
            total_deadline: pins.total_deadline,
        };
        let runtime =
            resolve_embedding_runtime(home, config, &opts, None).map_err(RetrievalError::from)?;
        embed_with_runtime(&runtime, inputs, cancel).await
    }

    async fn rerank(
        &self,
        home: &Path,
        _model_id: &str,
        config: &RerankerModelConfig,
        pins: &RouteCallPins,
        query: String,
        documents: Vec<String>,
        top_n: Option<u32>,
        cancel: CancellationToken,
    ) -> RetrievalResult<RerankResult> {
        let opts = RetrievalResolveOptions {
            provenance_incarnation: pins.provenance_incarnation.as_deref(),
            session_registry_generation: pins.session_registry_generation,
            is_retry: false,
            total_deadline: pins.total_deadline,
        };
        let runtime =
            resolve_reranker_runtime(home, config, &opts, None).map_err(RetrievalError::from)?;
        rerank_with_runtime(&runtime, query, documents, top_n, cancel).await
    }
}

// ---------------------------------------------------------------------------
// Fake executor (tests)
// ---------------------------------------------------------------------------

/// Scripted response for a single route model id.
#[derive(Clone)]
pub enum FakeEmbedScript {
    /// Succeed with `dims`-dimensional vectors of `fill` for each input.
    Ok { dims: usize, fill: f32 },
    /// Fail with a fixed error.
    Err(RetrievalError),
    /// Fail first `n` times then succeed.
    FailThenOk {
        failures: usize,
        err: RetrievalError,
        dims: usize,
        fill: f32,
    },
    /// Hang until cancelled (or return Cancelled).
    WaitForCancel,
}

#[derive(Clone)]
pub enum FakeRerankScript {
    Ok,
    /// Reverse document order with descending scores.
    ReverseOrder,
    Err(RetrievalError),
    WaitForCancel,
}

#[derive(Default)]
struct FakeState {
    embed_calls: Vec<String>,
    rerank_calls: Vec<String>,
    embed_pins: Vec<RouteCallPins>,
    rerank_pins: Vec<RouteCallPins>,
    /// model_id → remaining fail-then-ok counters
    fail_then_ok_left: HashMapLike,
}

/// Tiny map without importing HashMap at type alias complexity in Debug.
#[derive(Default)]
struct HashMapLike(std::collections::HashMap<String, usize>);

/// Fake executor for hermetic tests.
///
/// Records [`RouteCallPins`] and enforces zero/expired `total_deadline` and
/// cancellation so orchestrator pin/deadline invariants are testable without
/// live HTTP.
pub struct FakeRetrievalExecutor {
    embed_scripts: Mutex<std::collections::HashMap<String, FakeEmbedScript>>,
    rerank_scripts: Mutex<std::collections::HashMap<String, FakeRerankScript>>,
    state: Mutex<FakeState>,
    pub resolve_provider_ids: Mutex<Vec<String>>,
}

impl FakeRetrievalExecutor {
    pub fn new() -> Self {
        Self {
            embed_scripts: Mutex::new(std::collections::HashMap::new()),
            rerank_scripts: Mutex::new(std::collections::HashMap::new()),
            state: Mutex::new(FakeState::default()),
            resolve_provider_ids: Mutex::new(Vec::new()),
        }
    }

    pub fn set_embed(&self, model_id: &str, script: FakeEmbedScript) {
        self.embed_scripts
            .lock()
            .insert(model_id.to_owned(), script);
    }

    pub fn set_rerank(&self, model_id: &str, script: FakeRerankScript) {
        self.rerank_scripts
            .lock()
            .insert(model_id.to_owned(), script);
    }

    pub fn embed_calls(&self) -> Vec<String> {
        self.state.lock().embed_calls.clone()
    }

    pub fn rerank_calls(&self) -> Vec<String> {
        self.state.lock().rerank_calls.clone()
    }

    pub fn provider_ids_seen(&self) -> Vec<String> {
        self.resolve_provider_ids.lock().clone()
    }

    pub fn embed_pins_seen(&self) -> Vec<RouteCallPins> {
        self.state.lock().embed_pins.clone()
    }

    pub fn rerank_pins_seen(&self) -> Vec<RouteCallPins> {
        self.state.lock().rerank_pins.clone()
    }
}

fn enforce_pins(pins: &RouteCallPins, cancel: &CancellationToken) -> RetrievalResult<()> {
    if cancel.is_cancelled() {
        return Err(RetrievalError::Cancelled);
    }
    if let Some(d) = pins.total_deadline
        && d.is_zero()
    {
        return Err(RetrievalError::DeadlineExceeded);
    }
    Ok(())
}

impl Default for FakeRetrievalExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RetrievalExecutor for FakeRetrievalExecutor {
    async fn embed(
        &self,
        _home: &Path,
        model_id: &str,
        config: &EmbeddingModelConfig,
        pins: &RouteCallPins,
        inputs: Vec<String>,
        cancel: CancellationToken,
    ) -> RetrievalResult<EmbeddingResult> {
        self.resolve_provider_ids
            .lock()
            .push(config.provider.clone());
        {
            let mut st = self.state.lock();
            st.embed_calls.push(model_id.to_owned());
            st.embed_pins.push(pins.clone());
        }
        enforce_pins(pins, &cancel)?;

        let script =
            self.embed_scripts
                .lock()
                .get(model_id)
                .cloned()
                .unwrap_or(FakeEmbedScript::Ok {
                    dims: config.dimensions.unwrap_or(8) as usize,
                    fill: 0.1,
                });

        match script {
            FakeEmbedScript::Ok { dims, fill } => {
                if cancel.is_cancelled() {
                    return Err(RetrievalError::Cancelled);
                }
                Ok(make_embedding_result(
                    &config.model,
                    inputs.len(),
                    dims,
                    fill,
                ))
            }
            FakeEmbedScript::Err(e) => Err(e),
            FakeEmbedScript::FailThenOk {
                failures,
                err,
                dims,
                fill,
            } => {
                let mut st = self.state.lock();
                let left = st
                    .fail_then_ok_left
                    .0
                    .entry(model_id.to_owned())
                    .or_insert(failures);
                if *left > 0 {
                    *left -= 1;
                    return Err(err);
                }
                drop(st);
                Ok(make_embedding_result(
                    &config.model,
                    inputs.len(),
                    dims,
                    fill,
                ))
            }
            FakeEmbedScript::WaitForCancel => {
                // Signal readiness so cancel tests can synchronize without races.
                cancel.cancelled().await;
                Err(RetrievalError::Cancelled)
            }
        }
    }

    async fn rerank(
        &self,
        _home: &Path,
        model_id: &str,
        config: &RerankerModelConfig,
        pins: &RouteCallPins,
        _query: String,
        documents: Vec<String>,
        top_n: Option<u32>,
        cancel: CancellationToken,
    ) -> RetrievalResult<RerankResult> {
        self.resolve_provider_ids
            .lock()
            .push(config.provider.clone());
        {
            let mut st = self.state.lock();
            st.rerank_calls.push(model_id.to_owned());
            st.rerank_pins.push(pins.clone());
        }
        enforce_pins(pins, &cancel)?;

        let script = self
            .rerank_scripts
            .lock()
            .get(model_id)
            .cloned()
            .unwrap_or(FakeRerankScript::Ok);

        match script {
            FakeRerankScript::Ok | FakeRerankScript::ReverseOrder => {
                if cancel.is_cancelled() {
                    return Err(RetrievalError::Cancelled);
                }
                let mut hits: Vec<RerankHit> = documents
                    .iter()
                    .enumerate()
                    .map(|(i, _)| RerankHit {
                        index: i,
                        score: 1.0 - (i as f32) * 0.01,
                        document: None,
                    })
                    .collect();
                if matches!(script, FakeRerankScript::ReverseOrder) {
                    hits.reverse();
                    for (rank, h) in hits.iter_mut().enumerate() {
                        h.score = 1.0 - (rank as f32) * 0.01;
                    }
                }
                if let Some(n) = top_n {
                    hits.truncate(n as usize);
                }
                Ok(RerankResult {
                    model: config.model.clone(),
                    hits,
                })
            }
            FakeRerankScript::Err(e) => Err(e),
            FakeRerankScript::WaitForCancel => {
                cancel.cancelled().await;
                Err(RetrievalError::Cancelled)
            }
        }
    }
}

fn make_embedding_result(model: &str, n: usize, dims: usize, fill: f32) -> EmbeddingResult {
    EmbeddingResult {
        model: model.to_owned(),
        vectors: (0..n)
            .map(|index| EmbeddingVector {
                index,
                values: vec![fill; dims],
            })
            .collect(),
    }
}

/// Call counters for assertion helpers.
pub struct ExecutorCounters {
    pub embeds: AtomicUsize,
    pub reranks: AtomicUsize,
}

impl Default for ExecutorCounters {
    fn default() -> Self {
        Self {
            embeds: AtomicUsize::new(0),
            reranks: AtomicUsize::new(0),
        }
    }
}

impl ExecutorCounters {
    pub fn embeds(&self) -> usize {
        self.embeds.load(Ordering::SeqCst)
    }
    pub fn reranks(&self) -> usize {
        self.reranks.load(Ordering::SeqCst)
    }
}

/// Wrap any executor with counters.
pub struct CountingExecutor {
    inner: Arc<dyn RetrievalExecutor>,
    pub counters: Arc<ExecutorCounters>,
}

impl CountingExecutor {
    pub fn new(inner: Arc<dyn RetrievalExecutor>) -> Self {
        Self {
            inner,
            counters: Arc::new(ExecutorCounters::default()),
        }
    }
}

#[async_trait]
impl RetrievalExecutor for CountingExecutor {
    async fn embed(
        &self,
        home: &Path,
        model_id: &str,
        config: &EmbeddingModelConfig,
        pins: &RouteCallPins,
        inputs: Vec<String>,
        cancel: CancellationToken,
    ) -> RetrievalResult<EmbeddingResult> {
        self.counters.embeds.fetch_add(1, Ordering::SeqCst);
        self.inner
            .embed(home, model_id, config, pins, inputs, cancel)
            .await
    }

    async fn rerank(
        &self,
        home: &Path,
        model_id: &str,
        config: &RerankerModelConfig,
        pins: &RouteCallPins,
        query: String,
        documents: Vec<String>,
        top_n: Option<u32>,
        cancel: CancellationToken,
    ) -> RetrievalResult<RerankResult> {
        self.counters.reranks.fetch_add(1, Ordering::SeqCst);
        self.inner
            .rerank(
                home, model_id, config, pins, query, documents, top_n, cancel,
            )
            .await
    }
}
