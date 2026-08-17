//! Immutable validated retrieval snapshot types.
//!
//! Built from one complete PR15 [`RetrievalGraphConfig`] plus the current
//! provider registry generation. Credentials are never stored; PR16 resolves
//! them at each call against exact route pins.

use std::fmt;
use std::sync::Arc;

use indexmap::IndexMap;
use sha2::{Digest, Sha256};
use xai_grok_config_types::{
    EmbeddingEncoding, EmbeddingModelConfig, EmbeddingProtocol, PrimeConfig, RerankerModelConfig,
    RerankerProtocol, RetrievalFallbackStrategy, RetrievalGraphConfig, RetrievalProfileConfig,
};

use super::bounds::ProfileBudgetLimits;

/// Non-secret embedding space identity.
///
/// Derived from provider id/incarnation/origin host/path/protocol/model/dims/
/// encoding/normalization/doc-prep version as available. PR21 may persist a
/// fuller fingerprint later; this is the runtime pin for PR17.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct EmbeddingSpaceId {
    /// Stable hex digest (never raw secrets or full custom URLs with creds).
    fingerprint: String,
    /// Safe diagnostic label (provider/model/dims/protocol only).
    label: String,
}

impl EmbeddingSpaceId {
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    /// Build from secret-free route pins.
    pub fn from_parts(
        provider_instance_id: &str,
        incarnation: Option<&str>,
        origin_host: &str,
        path: &str,
        protocol: EmbeddingProtocol,
        model: &str,
        dimensions: Option<u32>,
        encoding: EmbeddingEncoding,
        normalization: &str,
        doc_prep_version: &str,
    ) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(b"embspace/v1\0");
        hasher.update(provider_instance_id.as_bytes());
        hasher.update(b"\0");
        hasher.update(incarnation.unwrap_or("").as_bytes());
        hasher.update(b"\0");
        // Hash origin host only (never log full custom URL with path secrets).
        hasher.update(origin_host.as_bytes());
        hasher.update(b"\0");
        hasher.update(path.as_bytes());
        hasher.update(b"\0");
        hasher.update(protocol.as_str().as_bytes());
        hasher.update(b"\0");
        hasher.update(model.as_bytes());
        hasher.update(b"\0");
        hasher.update(dimensions.unwrap_or(0).to_le_bytes());
        hasher.update(b"\0");
        hasher.update(encoding.as_str().as_bytes());
        hasher.update(b"\0");
        hasher.update(normalization.as_bytes());
        hasher.update(b"\0");
        hasher.update(doc_prep_version.as_bytes());
        let digest = hasher.finalize();
        let fingerprint = hex_encode(&digest[..16]);
        let label = format!(
            "{provider_instance_id}/{model}/{}d/{}",
            dimensions.unwrap_or(0),
            protocol.as_str()
        );
        Self { fingerprint, label }
    }
}

impl fmt::Debug for EmbeddingSpaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EmbeddingSpaceId")
            .field("fingerprint", &self.fingerprint)
            .field("label", &self.label)
            .finish()
    }
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

/// Secret-free resolved embedding route descriptor stored in a snapshot.
#[derive(Clone, PartialEq)]
pub struct EmbeddingRouteDescriptor {
    pub model_id: String,
    pub config: EmbeddingModelConfig,
    /// Provider instance id pin (exact).
    pub provider_instance_id: String,
    pub incarnation: Option<String>,
    /// Origin host only for space identity (no credentials, no full URL log).
    pub origin_host: String,
    pub embedding_space: EmbeddingSpaceId,
    /// Default request timeout hint from provider metadata (seconds → Duration later).
    pub request_timeout_ms: u64,
}

impl fmt::Debug for EmbeddingRouteDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EmbeddingRouteDescriptor")
            .field("model_id", &self.model_id)
            .field("provider_instance_id", &self.provider_instance_id)
            .field("incarnation", &self.incarnation)
            .field("origin_host", &self.origin_host)
            .field("embedding_space", &self.embedding_space)
            .field("model", &self.config.model)
            .field("protocol", &self.config.protocol)
            .field("dimensions", &self.config.dimensions)
            .field("encoding", &self.config.encoding)
            .field("request_timeout_ms", &self.request_timeout_ms)
            .finish()
    }
}

/// Secret-free resolved reranker route descriptor.
#[derive(Clone, PartialEq)]
pub struct RerankerRouteDescriptor {
    pub model_id: String,
    pub config: RerankerModelConfig,
    pub provider_instance_id: String,
    pub incarnation: Option<String>,
    pub origin_host: String,
    pub request_timeout_ms: u64,
}

impl fmt::Debug for RerankerRouteDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RerankerRouteDescriptor")
            .field("model_id", &self.model_id)
            .field("provider_instance_id", &self.provider_instance_id)
            .field("incarnation", &self.incarnation)
            .field("origin_host", &self.origin_host)
            .field("model", &self.config.model)
            .field("protocol", &self.config.protocol)
            .field("endpoint", &self.config.endpoint)
            .field("request_timeout_ms", &self.request_timeout_ms)
            .finish()
    }
}

/// Named profile frozen into a snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotProfile {
    pub id: String,
    pub config: RetrievalProfileConfig,
    /// Ordered embedding model ids (declaration order only).
    pub embedding_route_ids: Vec<String>,
    /// Ordered reranker model ids (declaration order only).
    pub reranker_route_ids: Vec<String>,
    pub budgets: ProfileBudgetLimits,
    pub fallback_strategy: RetrievalFallbackStrategy,
}

/// Immutable validated retrieval snapshot (Arc-shared).
#[derive(Clone, PartialEq)]
pub struct RetrievalSnapshot {
    /// Monotonic registry generation for this published snapshot.
    pub generation: u64,
    /// PR15 management graph generation when built from disk.
    pub graph_generation: u64,
    /// Provider registry generation when built.
    pub provider_generation: u64,
    /// Content fingerprint of graph + provider generation (secret-free).
    pub fingerprint: String,
    /// False when empty/disabled (no graph or no usable profiles).
    pub enabled: bool,
    pub embedding_models: IndexMap<String, EmbeddingRouteDescriptor>,
    pub reranker_models: IndexMap<String, RerankerRouteDescriptor>,
    pub profiles: IndexMap<String, SnapshotProfile>,
    pub prime: PrimeConfig,
    pub memory_retrieval_profile: Option<String>,
    /// Safe warnings from build (missing optional providers, soft issues).
    pub warnings: Vec<String>,
    /// Source graph configs retained for PR16 resolve (credential-free).
    pub source_graph: RetrievalGraphConfig,
}

impl fmt::Debug for RetrievalSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RetrievalSnapshot")
            .field("generation", &self.generation)
            .field("graph_generation", &self.graph_generation)
            .field("provider_generation", &self.provider_generation)
            .field("fingerprint", &self.fingerprint)
            .field("enabled", &self.enabled)
            .field("embedding_model_count", &self.embedding_models.len())
            .field("reranker_model_count", &self.reranker_models.len())
            .field("profile_count", &self.profiles.len())
            .field("warnings_count", &self.warnings.len())
            .finish()
    }
}

impl RetrievalSnapshot {
    /// Empty disabled snapshot (startup with no graph).
    pub fn disabled(generation: u64) -> Arc<Self> {
        Arc::new(Self {
            generation,
            graph_generation: 0,
            provider_generation: 0,
            fingerprint: "disabled".into(),
            enabled: false,
            embedding_models: IndexMap::new(),
            reranker_models: IndexMap::new(),
            profiles: IndexMap::new(),
            prime: PrimeConfig::default(),
            memory_retrieval_profile: None,
            warnings: vec!["retrieval service disabled: no validated graph".into()],
            source_graph: RetrievalGraphConfig::default(),
        })
    }

    pub fn profile(&self, id: &str) -> Option<&SnapshotProfile> {
        self.profiles.get(id)
    }

    pub fn embedding_route(&self, id: &str) -> Option<&EmbeddingRouteDescriptor> {
        self.embedding_models.get(id)
    }

    pub fn reranker_route(&self, id: &str) -> Option<&RerankerRouteDescriptor> {
        self.reranker_models.get(id)
    }
}

/// Extract host (authority without userinfo) from a base URL for space ids.
pub fn origin_host_from_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim();
    // Never include credentials if somehow present in the string.
    let no_scheme = trimmed
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(trimmed);
    let no_user = no_scheme
        .rsplit_once('@')
        .map(|(_, hostport)| hostport)
        .unwrap_or(no_scheme);
    let host = no_user.split('/').next().unwrap_or(no_user);
    // Drop port for stability unless needed — keep host:port for distinct ports.
    host.to_ascii_lowercase()
}

/// Content fingerprint over a validated graph + provider generation.
///
/// Fail-closed: serialization failure returns `Err` so callers never publish
/// a generations-only weak digest (would collide distinct graphs).
pub fn snapshot_fingerprint(
    graph: &RetrievalGraphConfig,
    provider_generation: u64,
    graph_generation: u64,
) -> Result<String, String> {
    let mut hasher = Sha256::new();
    hasher.update(b"retrieval-snap/v1\0");
    hasher.update(provider_generation.to_le_bytes());
    hasher.update(b"\0");
    hasher.update(graph_generation.to_le_bytes());
    hasher.update(b"\0");
    // Stable serialization via JSON (maps preserve IndexMap order in serde).
    let bytes = serde_json::to_vec(graph)
        .map_err(|e| format!("retrieval snapshot fingerprint serialization failed: {e}"))?;
    hasher.update(&bytes);
    let digest = hasher.finalize();
    Ok(hex_encode(&digest[..16]))
}

/// Build embedding space id from a model config + resolved provider pins.
pub fn embedding_space_for(
    provider_instance_id: &str,
    incarnation: Option<&str>,
    base_url: &str,
    config: &EmbeddingModelConfig,
) -> EmbeddingSpaceId {
    EmbeddingSpaceId::from_parts(
        provider_instance_id,
        incarnation,
        &origin_host_from_base_url(base_url),
        xai_grok_inference::DEFAULT_EMBEDDINGS_PATH,
        config.protocol,
        &config.model,
        config.dimensions,
        config.encoding,
        "none",
        "v0",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn space_id_stable_and_secret_free() {
        let a = EmbeddingSpaceId::from_parts(
            "acct-a",
            Some("inc-1"),
            "api.example.com",
            "/embeddings",
            EmbeddingProtocol::OpenaiCompatible,
            "text-embedding-3-small",
            Some(1536),
            EmbeddingEncoding::Float,
            "none",
            "v0",
        );
        let b = EmbeddingSpaceId::from_parts(
            "acct-a",
            Some("inc-1"),
            "api.example.com",
            "/embeddings",
            EmbeddingProtocol::OpenaiCompatible,
            "text-embedding-3-small",
            Some(1536),
            EmbeddingEncoding::Float,
            "none",
            "v0",
        );
        assert_eq!(a, b);
        let dbg = format!("{a:?}");
        assert!(!dbg.contains("sk-"));
        assert!(dbg.contains("fingerprint"));
    }

    #[test]
    fn origin_host_strips_userinfo() {
        assert_eq!(
            origin_host_from_base_url("https://user:pass@api.example.com:443/v1"),
            "api.example.com:443"
        );
    }
}
