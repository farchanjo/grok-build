//! Credential-free retrieval consumer facade for the memory backend.
//!
//! PR21 defines a consumer trait [`MemoryRetrieval`] in this crate so the
//! shell can provide an adapter over the PR17 `RetrievalService` without
//! creating a dependency from `xai-grok-memory` to shell/provider code, and
//! without persisting credentials here.
//!
//! The shell implements this trait for both routing modes:
//! - **Named profile**: routes embedding/reranking through the exact PR15
//!   profile and PR17's exact provider-instance/incarnation routes — never the
//!   active chat model base URL, API key, OAuth, or credential provider.
//! - **Legacy (no profile)**: a synthesized deterministic `EmbeddingSourceSpec`
//!   plus the existing `[memory.embedding]` provider path, so existing users
//!   do not reconnect, rewrite config, or rebuild unnecessarily.
//!
//! Where a handle is absent, or the profile/provider/incarnation fails, the
//! backend degrades to FTS-only / local ordering (never a sibling-credential
//! fallback).

use async_trait::async_trait;

use super::fingerprint::EmbeddingSourceSpec;

/// Categorized retrieval failure kind (safe to log).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetrievalErrorKind {
    /// Profile missing, disabled, or provider/incarnation unresolvable.
    SourceUnavailable,
    /// Transient network/provider failure (may be retried). The backend
    /// degrades to FTS-only / local ordering; it never includes provider
    /// error text in diagnostics.
    Transient,
    /// Malformed/unsupported response shape.
    Malformed,
    /// Cancelled.
    Cancelled,
    /// Budget/limit exceeded (deadline, attempts, input/output).
    BudgetExhausted,
}

impl RetrievalErrorKind {
    /// Whether this failure is transient and safe to retry later.
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            RetrievalErrorKind::Transient | RetrievalErrorKind::Cancelled
        )
    }
}

/// Categorized, credential-free retrieval failure.
///
/// The `Debug` impl reports only the category (`kind`); it never renders an
/// arbitrary provider/network error string that could contain memory text,
/// vectors, or credentials.
#[derive(Clone)]
pub struct RetrievalError {
    kind: RetrievalErrorKind,
    /// Diagnostic detail. Kept out of `Debug` to avoid leaking memory
    /// text/vector/provider error strings into diagnostics.
    detail: Option<String>,
}

impl RetrievalError {
    pub fn new(kind: RetrievalErrorKind) -> Self {
        Self { kind, detail: None }
    }

    pub fn with_detail(kind: RetrievalErrorKind, message: String) -> Self {
        Self {
            kind,
            detail: Some(message),
        }
    }

    pub fn kind(&self) -> RetrievalErrorKind {
        self.kind
    }
}

impl std::fmt::Debug for RetrievalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetrievalError")
            .field("kind", &self.kind)
            .finish()
    }
}

impl std::fmt::Display for RetrievalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (&self.kind, &self.detail) {
            (kind, Some(detail)) => write!(f, "{kind:?} ({detail})"),
            (kind, None) => write!(f, "{kind:?}"),
        }
    }
}

impl std::error::Error for RetrievalError {}

/// Credential-free capability the memory backend uses for embeddings and
/// optional remote reranking. Implemented by a shell adapter over
/// `RetrievalService` (named profile) or wrapped legacy providers.
#[async_trait]
pub trait MemoryRetrieval: Send + Sync {
    /// Static credential-free description of the embedding source pinned to
    /// this handle. Deterministic for a given resolve.
    fn source_spec(&self) -> EmbeddingSourceSpec;

    /// Embed a batch of texts through the pinned source.
    ///
    /// Returns one vector per input text, each of length
    /// `source_spec().dimensions`. Errors degrade to FTS-only.
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, RetrievalError>;

    /// Optionally rerank `documents` against `query`, returning a permutation
    /// of valid indices into `documents` (length equals `documents.len()`).
    ///
    /// Return `Ok(None)` when no reranker is configured or when the reranker
    /// is unavailable/cancelled/malformed/stale — the caller then keeps its
    /// complete exact local pre-rerank order. Invalid/duplicate/missing
    /// indices in the returned permutation must be treated as `None` by the
    /// caller (`validate_rerank_permutation`).
    async fn rerank(
        &self,
        query: &str,
        documents: &[String],
    ) -> Result<Option<Vec<usize>>, RetrievalError>;
}

/// Validate a reranker-provided permutation of `documents`.
///
/// Returns the permutation only if it is a perfect permutation of
/// `0..documents.len()` (all indices present exactly once, all in range).
/// Otherwise returns `None`, signalling "restore the complete exact local
/// pre-rerank order".
pub fn validate_rerank_permutation(
    perm: Option<&[usize]>,
    documents_len: usize,
) -> Option<Vec<usize>> {
    let perm = perm?;
    if perm.len() != documents_len {
        return None;
    }
    let mut seen = vec![false; documents_len];
    for &idx in perm {
        if idx >= documents_len || seen[idx] {
            return None;
        }
        seen[idx] = true;
    }
    Some(perm.to_vec())
}

/// Apply a validated rerank permutation to `rows` in place.
///
/// `rows` preserves insertion order; a perfect permutation is applied by
/// reordering `rows` to a new array in the given index order. Every element is
/// moved exactly once, so no partial loss can occur. Returns the new row order.
pub fn apply_rerank_permutation<T>(rows: &mut Vec<T>, perm: &[usize]) {
    if perm.len() != rows.len() {
        return;
    }
    // Swap-based in-place permutation (O(n)), preserving every element.
    let mut pos: Vec<usize> = (0..rows.len()).collect();
    for (i, &target) in perm.iter().enumerate() {
        while pos[i] != target {
            let j = pos[i];
            let k = pos[j];
            rows.swap(j, k);
            pos[j] = k;
            pos[k] = j;
        }
    }
}

/// Build a deterministic source spec for hermetic tests / legacy synthesis.
pub fn stub_spec(dimensions: usize, model: &str) -> EmbeddingSourceSpec {
    EmbeddingSourceSpec {
        provider_instance_id: "test-provider".into(),
        incarnation: Some("test-inc#1".into()),
        origin_host: "embeds.test.local".into(),
        embedding_path: "/v1/embeddings".into(),
        protocol: "openai_compatible".into(),
        model: model.to_owned(),
        dimensions,
        encoding: "float".into(),
        normalization: super::fingerprint::NORMALIZATION_NONE.into(),
    }
}

#[cfg(any(test, feature = "test-support"))]
use std::sync::Arc;
#[cfg(any(test, feature = "test-support"))]
use std::sync::atomic::{AtomicU64, Ordering};

/// Deterministic hermetic `MemoryRetrieval` for tests and downstream test
/// targets. Embeddings = blake3-derived floats (like `MockEmbeddingProvider`);
/// rerank defaults to `Ok(None)` (no reranker → keep local order). Each field
/// can be overridden to inject failures, invalid permutations, or assertions.
#[cfg(any(test, feature = "test-support"))]
pub struct FakeMemoryRetrieval {
    pub spec: EmbeddingSourceSpec,
    pub embed_override:
        Option<Arc<dyn Fn(&[String]) -> Result<Vec<Vec<f32>>, RetrievalError> + Send + Sync>>,
    pub rerank_override: Option<
        Arc<dyn Fn(&str, &[String]) -> Result<Option<Vec<usize>>, RetrievalError> + Send + Sync>,
    >,
    embed_calls: Arc<AtomicU64>,
    rerank_calls: Arc<AtomicU64>,
}

#[cfg(any(test, feature = "test-support"))]
impl FakeMemoryRetrieval {
    pub fn new(dimensions: usize, model: &str) -> Self {
        Self {
            spec: stub_spec(dimensions, model),
            embed_override: None,
            rerank_override: None,
            embed_calls: Arc::new(AtomicU64::new(0)),
            rerank_calls: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn with_embedment(
        mut self,
        f: impl Fn(&[String]) -> Result<Vec<Vec<f32>>, RetrievalError> + Send + Sync + 'static,
    ) -> Self {
        self.embed_override = Some(Arc::new(f));
        self
    }

    pub fn with_rerank(
        mut self,
        f: impl Fn(&str, &[String]) -> Result<Option<Vec<usize>>, RetrievalError>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.rerank_override = Some(Arc::new(f));
        self
    }

    pub fn embed_calls(&self) -> u64 {
        self.embed_calls.load(Ordering::Relaxed)
    }

    pub fn rerank_calls(&self) -> u64 {
        self.rerank_calls.load(Ordering::Relaxed)
    }
}

#[cfg(any(test, feature = "test-support"))]
fn mock_embed(texts: &[String], dims: usize) -> Result<Vec<Vec<f32>>, RetrievalError> {
    Ok(texts
        .iter()
        .map(|text| {
            let hash = blake3::hash(text.as_bytes());
            let bytes = hash.as_bytes();
            (0..dims).map(|i| bytes[i % 32] as f32 / 255.0).collect()
        })
        .collect())
}

#[cfg(any(test, feature = "test-support"))]
#[async_trait]
impl MemoryRetrieval for FakeMemoryRetrieval {
    fn source_spec(&self) -> EmbeddingSourceSpec {
        self.spec.clone()
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, RetrievalError> {
        self.embed_calls.fetch_add(1, Ordering::Relaxed);
        if let Some(f) = &self.embed_override {
            return f(texts);
        }
        mock_embed(texts, self.spec.dimensions)
    }

    async fn rerank(
        &self,
        query: &str,
        documents: &[String],
    ) -> Result<Option<Vec<usize>>, RetrievalError> {
        self.rerank_calls.fetch_add(1, Ordering::Relaxed);
        if let Some(f) = &self.rerank_override {
            return f(query, documents);
        }
        Ok(None)
    }
}
