//! Embedding provider abstraction for memory vector search.
//!
//! Defines the `EmbeddingProvider` trait and an API-based implementation
//! that calls an OpenAI-compatible embeddings API endpoint.
//!
//! Embeddings are cached in the sqlite-vec `chunks_vec` table — the vec0
//! virtual table IS the cache. No separate cache needed.

use async_trait::async_trait;

/// Maximum retry attempts for transient API errors (429, 5xx).
const MAX_RETRIES: usize = 3;
/// Initial backoff delay in milliseconds (doubles on each retry: 1s, 2s, 4s).
const INITIAL_BACKOFF_MS: u64 = 1000;

/// Trait for generating text embeddings.
///
/// Implementations must be `Send + Sync` so they can be used in `Send`
/// futures (e.g., inside `tokio::spawn`). The `embed_batch` method is
/// async to support API-based providers.
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Embed a batch of texts, returning one vector per input text.
    async fn embed_batch(
        &self,
        texts: &[&str],
    ) -> Result<Vec<Vec<f32>>, Box<dyn std::error::Error>>;

    /// The model name used for embeddings.
    fn model_name(&self) -> &str;

    /// The dimensionality of the embedding vectors.
    fn dimensions(&self) -> usize;
}

/// Validate an embedding batch **before any SQLite write**: exact result count
/// (`embeddings.len() == texts_len`), exact per-vector dimension, and finite
/// values only. A short response must never be zipped onto the first N inputs —
/// that would permanently mis-associate vectors to wrong chunks — and NaN/Inf
/// must never reach the vec table (F1 / Phase 7 validation surface).
pub(crate) fn validate_embedding_batch(
    texts_len: usize,
    dimensions: usize,
    embeddings: &[Vec<f32>],
) -> Result<(), String> {
    if embeddings.len() != texts_len {
        return Err(format!(
            "embedding provider returned {} vectors for {} inputs; refusing mis-association",
            embeddings.len(),
            texts_len,
        ));
    }
    for (i, v) in embeddings.iter().enumerate() {
        if v.len() != dimensions {
            return Err(format!(
                "embedding {i} has dimension {} (expected {dimensions}); refusing mixed spaces",
                v.len(),
            ));
        }
        if v.iter().any(|f| !f.is_finite()) {
            return Err(format!(
                "embedding {i} contains non-finite values (NaN/Inf); refusing to write"
            ));
        }
    }
    Ok(())
}

/// API-based embedding provider using an OpenAI-compatible embeddings endpoint.
pub struct ApiEmbeddingProvider {
    api_base: String,
    model: String,
    dimensions: usize,
    client: reqwest_middleware::ClientWithMiddleware,
    max_batch_size: usize,
}

impl ApiEmbeddingProvider {
    pub fn new(
        api_base: String,
        model: String,
        dimensions: usize,
        client: reqwest_middleware::ClientWithMiddleware,
    ) -> Self {
        Self {
            api_base,
            model,
            dimensions,
            client,
            max_batch_size: 32,
        }
    }

    pub fn from_config(
        config: &xai_grok_config_types::MemoryEmbeddingConfig,
        api_base: String,
        client: reqwest_middleware::ClientWithMiddleware,
    ) -> Option<Self> {
        let model = config.model.clone().filter(|m| !m.is_empty())?;
        Some(Self::new(api_base, model, config.dimensions, client))
    }

    pub fn from_session(
        config: &xai_grok_config_types::MemoryEmbeddingConfig,
        proxy_base_url: String,
        auth_key: String,
    ) -> Option<Self> {
        let client = build_static_middleware_client(Some(auth_key));
        Self::from_config(config, proxy_base_url, client)
    }
}

pub(super) fn build_middleware_client(
    credentials: std::sync::Arc<dyn xai_grok_auth::AuthCredentialProvider>,
) -> reqwest_middleware::ClientWithMiddleware {
    xai_grok_http::with_auth_retry(xai_grok_http::shared_client(), credentials)
}

fn build_static_middleware_client(
    api_key: Option<String>,
) -> reqwest_middleware::ClientWithMiddleware {
    let provider: std::sync::Arc<dyn xai_grok_auth::AuthCredentialProvider> = std::sync::Arc::new(
        xai_grok_auth::StaticAuthCredentialProvider::new(Box::new(NoopHttpAuth), api_key),
    );
    build_middleware_client(provider)
}

struct NoopHttpAuth;

impl xai_grok_auth::HttpAuth for NoopHttpAuth {
    fn apply(&self, builder: reqwest::RequestBuilder, _base_url: &str) -> reqwest::RequestBuilder {
        builder
    }
}

#[async_trait]
impl EmbeddingProvider for ApiEmbeddingProvider {
    #[tracing::instrument(name = "memory.embed_batch", skip_all, fields(batch_size = texts.len()))]
    async fn embed_batch(
        &self,
        texts: &[&str],
    ) -> Result<Vec<Vec<f32>>, Box<dyn std::error::Error>> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        let mut all_embeddings = Vec::with_capacity(texts.len());

        // Process in batches to respect API payload limits
        for batch in texts.chunks(self.max_batch_size) {
            let input: Vec<&str> = batch.to_vec();
            let body_json = serde_json::json!({
                "model": self.model,
                "input": input,
                "dimensions": self.dimensions,
            });

            // Retry with exponential backoff on transient errors (429, 5xx)
            let mut last_err = String::new();
            let mut success = false;
            for attempt in 0..MAX_RETRIES {
                if attempt > 0 {
                    let delay = INITIAL_BACKOFF_MS * 2u64.pow(attempt as u32 - 1);
                    tracing::warn!(
                        attempt,
                        delay_ms = delay,
                        "retrying embedding API call after transient error"
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                }

                let request = xai_grok_http::shared_client()
                    .post(format!("{}/embeddings", self.api_base))
                    .json(&body_json)
                    .header("X-XAI-Token-Auth", "xai-grok-cli")
                    .header("x-grok-client-version", xai_grok_version::VERSION);

                let req = match request.build() {
                    Ok(r) => r,
                    Err(e) => {
                        return Err(format!("failed to build embedding request: {e}").into());
                    }
                };
                let response = match self.client.execute(req).await {
                    Ok(r) => r,
                    Err(e) => {
                        last_err = format!("request failed: {e}");
                        continue;
                    }
                };

                let status = response.status();
                if status.is_success() {
                    let body: serde_json::Value = response.json().await?;
                    let data = body
                        .get("data")
                        .and_then(|d| d.as_array())
                        .ok_or("embedding response missing 'data' array")?;

                    for item in data {
                        let embedding: Vec<f32> = item
                            .get("embedding")
                            .and_then(|e| e.as_array())
                            .ok_or("embedding item missing 'embedding' array")?
                            .iter()
                            .filter_map(|v| v.as_f64().map(|f| f as f32))
                            .collect();
                        all_embeddings.push(embedding);
                    }
                    success = true;
                    break;
                }

                // Retry on 429 (rate limit) or 5xx (server error)
                if status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
                    last_err = format!(
                        "HTTP {status}: {}",
                        response.text().await.unwrap_or_default()
                    );
                    continue;
                }

                // Non-retryable error (4xx other than 429)
                let body = response.text().await.unwrap_or_default();
                return Err(format!("embedding API error {status}: {body}").into());
            }

            if !success {
                return Err(format!(
                    "embedding API failed after {MAX_RETRIES} attempts: {last_err}"
                )
                .into());
            }
        }

        // Validate before returning: exact count, exact dimensions, finite
        // values (F1). A malformed provider response must fail closed, never
        // mis-associate vectors to chunks.
        validate_embedding_batch(texts.len(), self.dimensions, &all_embeddings)?;

        Ok(all_embeddings)
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }
}

/// Embedding provider backed by the shared OpenAI platform transport
/// (`xai_grok_inference::openai_platform`). Prefer this over ad-hoc HTTP once
/// a provider registry client is available; request bodies remain typed
/// create-embedding JSON, not raw passthrough.
pub struct PlatformEmbeddingProvider {
    api_base: String,
    model: String,
    dimensions: usize,
    token: String,
    max_batch_size: usize,
}

impl PlatformEmbeddingProvider {
    pub fn new(api_base: String, model: String, dimensions: usize, token: String) -> Self {
        Self {
            api_base,
            model,
            dimensions,
            token,
            max_batch_size: 32,
        }
    }
}

#[async_trait]
impl EmbeddingProvider for PlatformEmbeddingProvider {
    async fn embed_batch(
        &self,
        texts: &[&str],
    ) -> Result<Vec<Vec<f32>>, Box<dyn std::error::Error>> {
        use tokio_util::sync::CancellationToken;
        use xai_grok_inference::openai_platform::generated::openai_types::CreateEmbeddingParams;
        use xai_grok_inference::{OpenAiClient, PlatformClientConfig, TransportPolicy};

        if texts.is_empty() {
            return Ok(vec![]);
        }
        let client = OpenAiClient::from_config(
            PlatformClientConfig {
                provider_id: "memory_embeddings".into(),
                display_name: "Memory embeddings".into(),
                base_url: self.api_base.clone(),
                admin_base_url: None,
                application_token: Some(self.token.clone()),
                admin_token: None,
                extra_headers: Default::default(),
                policy: TransportPolicy::default(),
            },
            CancellationToken::new(),
        )?;

        let mut all = Vec::with_capacity(texts.len());
        for batch in texts.chunks(self.max_batch_size) {
            // Deserialize into schema-derived CreateEmbeddingParams (not Value passthrough).
            let params_val = serde_json::json!({
                "body": {
                    "model": self.model,
                    "input": batch.iter().copied().collect::<Vec<_>>(),
                    "dimensions": self.dimensions,
                }
            });
            let req: CreateEmbeddingParams = serde_json::from_value(params_val)
                .map_err(|e| format!("typed embedding params: {e}"))?;
            let resp = client.create_embedding(req).await?;
            let resp_val = serde_json::to_value(&resp)
                .map_err(|e| format!("embedding response encode: {e}"))?;
            let data = resp_val
                .pointer("/data")
                .or_else(|| resp_val.pointer("/body/data"))
                .and_then(|v| v.as_array())
                .ok_or("embedding response missing data")?;
            for item in data {
                let embedding: Vec<f32> = item
                    .get("embedding")
                    .and_then(|e| e.as_array())
                    .ok_or("embedding item missing embedding array")?
                    .iter()
                    .filter_map(|v| v.as_f64().map(|f| f as f32))
                    .collect();
                all.push(embedding);
            }
        }
        // Validate before returning: exact count, exact dimensions, finite
        // values (F1).
        validate_embedding_batch(texts.len(), self.dimensions, &all)?;
        Ok(all)
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }
}

/// A mock embedding provider for testing that returns deterministic vectors.
/// Uses blake3 hash of text → float values for reproducible results.
#[cfg(any(test, feature = "test-support"))]
pub struct MockEmbeddingProvider {
    pub dimensions: usize,
}

/// Embedding provider adapter over a credential-free [`super::retrieval::MemoryRetrieval`].
///
/// Lets the memory backend embed through a named profile (or synthesized
/// legacy source) via the PR17 `RetrievalService` without this crate holding
/// any credentials. Same source space is guaranteed by construction.
pub struct RetrievalEmbeddingProvider {
    retrieval: std::sync::Arc<dyn super::retrieval::MemoryRetrieval>,
    dimensions: usize,
    model: String,
}

impl RetrievalEmbeddingProvider {
    pub fn new(retrieval: std::sync::Arc<dyn super::retrieval::MemoryRetrieval>) -> Self {
        let spec = retrieval.source_spec();
        let dimensions = spec.dimensions;
        let model = spec.model;
        Self {
            retrieval,
            dimensions,
            model,
        }
    }
}

#[async_trait]
impl EmbeddingProvider for RetrievalEmbeddingProvider {
    async fn embed_batch(
        &self,
        texts: &[&str],
    ) -> Result<Vec<Vec<f32>>, Box<dyn std::error::Error>> {
        let owned: Vec<String> = texts.iter().map(|s| s.to_string()).collect();
        let out = self.retrieval.embed_batch(&owned).await?;
        // Validation before any SQLite write (Phase 7 / F1) — including for
        // the facade-routed provider: exact count, exact dimension, finite
        // values. Never surface vectors that would mis-associate or mix a
        // different embedding space into this index.
        validate_embedding_batch(texts.len(), self.dimensions, &out)
            .map_err(|e| format!("facade embedding validation: {e}"))?;
        Ok(out)
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }
}

#[cfg(any(test, feature = "test-support"))]
#[async_trait]
impl EmbeddingProvider for MockEmbeddingProvider {
    async fn embed_batch(
        &self,
        texts: &[&str],
    ) -> Result<Vec<Vec<f32>>, Box<dyn std::error::Error>> {
        Ok(texts
            .iter()
            .map(|text| {
                let hash = blake3::hash(text.as_bytes());
                let bytes = hash.as_bytes();
                (0..self.dimensions)
                    .map(|i| bytes[i % 32] as f32 / 255.0)
                    .collect()
            })
            .collect())
    }

    fn model_name(&self) -> &str {
        "mock-embedding"
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_embedding_deterministic() {
        let provider = MockEmbeddingProvider { dimensions: 4 };
        let r1 = provider.embed_batch(&["hello"]).await.unwrap();
        let r2 = provider.embed_batch(&["hello"]).await.unwrap();
        assert_eq!(r1, r2);
    }

    #[tokio::test]
    async fn test_mock_embedding_different_texts() {
        let provider = MockEmbeddingProvider { dimensions: 4 };
        let results = provider.embed_batch(&["hello", "world"]).await.unwrap();
        assert_eq!(results.len(), 2);
        assert_ne!(results[0], results[1]);
    }

    #[tokio::test]
    async fn test_mock_embedding_empty_input() {
        let provider = MockEmbeddingProvider { dimensions: 4 };
        let results = provider.embed_batch(&[]).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_mock_embedding_correct_dimensions() {
        let provider = MockEmbeddingProvider { dimensions: 128 };
        let results = provider.embed_batch(&["test"]).await.unwrap();
        assert_eq!(results[0].len(), 128);
    }

    // F1 / Phase 7: validate embedding batches before any SQLite write.

    #[test]
    fn validate_embedding_batch_accepts_exact_count_and_dimensions() {
        let ok = vec![vec![1.0, 2.0, 3.0, 4.0], vec![5.0, 6.0, 7.0, 8.0]];
        assert!(validate_embedding_batch(2, 4, &ok).is_ok());
    }

    #[test]
    fn validate_embedding_batch_rejects_short_response() {
        // 2 inputs but only 1 vector returned: a zip would mis-associate the
        // single vector onto the first input and claim it embedded.
        let short = vec![vec![1.0, 2.0, 3.0, 4.0]];
        assert!(validate_embedding_batch(2, 4, &short).is_err());
    }

    #[test]
    fn validate_embedding_batch_rejects_extra_response() {
        let extra = vec![vec![1.0; 4], vec![2.0; 4], vec![3.0; 4]];
        assert!(validate_embedding_batch(2, 4, &extra).is_err());
    }

    #[test]
    fn validate_embedding_batch_rejects_wrong_dimension() {
        let wrong_dim = vec![vec![1.0; 8]]; // expected 4
        assert!(validate_embedding_batch(1, 4, &wrong_dim).is_err());
    }

    #[test]
    fn validate_embedding_batch_rejects_nan() {
        let nan = vec![vec![f32::NAN, 1.0, 2.0, 3.0]];
        assert!(validate_embedding_batch(1, 4, &nan).is_err());
    }

    #[test]
    fn validate_embedding_batch_rejects_infinity() {
        let inf = vec![vec![f32::INFINITY, 1.0, 2.0, 3.0]];
        let neg_inf = vec![vec![f32::NEG_INFINITY, 1.0, 2.0, 3.0]];
        assert!(validate_embedding_batch(1, 4, &inf).is_err());
        assert!(validate_embedding_batch(1, 4, &neg_inf).is_err());
    }
}
