//! Named retrieval configuration value types (credential-free).
//!
//! These DTOs describe embedding models, rerankers, retrieval profiles, and
//! prime skill/agent consumers. Provider connection details and credentials
//! remain exclusively in `[model_providers.<id>]` — never in these structs
//! beyond a validated relative reranker path.
//!
//! PR15 owns configuration schema + validation only. Network adapters (PR16),
//! runtime fallback/orchestration (PR17), and prime turn injection (PR18/19)
//! are intentionally out of scope.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Bounds (shared by parse-time clamp and management validation)
// ---------------------------------------------------------------------------

/// Maximum length of a retrieval entity slug (embedding/reranker/profile id).
pub const MAX_RETRIEVAL_ID_LEN: usize = 64;
/// Minimum embedding dimensions when specified.
pub const MIN_EMBEDDING_DIMENSIONS: u32 = 1;
/// Maximum embedding dimensions when specified.
pub const MAX_EMBEDDING_DIMENSIONS: u32 = 16_384;
/// Default embedding batch size.
pub const DEFAULT_EMBEDDING_BATCH_SIZE: u32 = 32;
/// Maximum embedding/reranker batch size.
pub const MAX_BATCH_SIZE: u32 = 512;
/// Default max input tokens for embeddings.
pub const DEFAULT_MAX_INPUT_TOKENS: u32 = 8_192;
/// Maximum allowed max_input_tokens.
pub const MAX_INPUT_TOKENS_BOUND: u32 = 1_000_000;
/// Default profile candidate limit.
pub const DEFAULT_MAX_CANDIDATES: u32 = 50;
/// Default profile result limit.
pub const DEFAULT_MAX_RESULTS: u32 = 10;
/// Absolute maximum for candidate/result limits.
pub const MAX_RESULT_LIMIT: u32 = 10_000;
/// Default profile-wide deadline (milliseconds).
pub const DEFAULT_DEADLINE_MS: u64 = 10_000;
/// Maximum deadline (milliseconds).
pub const MAX_DEADLINE_MS: u64 = 300_000;
/// Default attempt budget.
pub const DEFAULT_MAX_ATTEMPTS: u32 = 2;
/// Maximum attempt budget.
pub const MAX_ATTEMPTS: u32 = 16;
/// Default max output tokens for profile budgets.
pub const DEFAULT_MAX_OUTPUT_TOKENS: u32 = 4_096;
/// Maximum context-fraction for prime injection (0.0..=1.0).
pub const MAX_CONTEXT_FRACTION: f32 = 1.0;

// ---------------------------------------------------------------------------
// Protocols / encoding
// ---------------------------------------------------------------------------

/// Wire protocol for embedding requests.
///
/// Only registered typed protocols are accepted; user-authored JSON templates
/// are never supported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingProtocol {
    /// OpenAI-compatible `/v1/embeddings` shape.
    #[default]
    OpenaiCompatible,
}

impl EmbeddingProtocol {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenaiCompatible => "openai_compatible",
        }
    }
}

/// Encoding format for embedding vectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingEncoding {
    #[default]
    Float,
    Base64,
}

impl EmbeddingEncoding {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Float => "float",
            Self::Base64 => "base64",
        }
    }
}

/// Wire protocol for reranker requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RerankerProtocol {
    /// OpenAI-compatible style rerank endpoint.
    #[default]
    OpenaiCompatible,
    /// Cohere-compatible rerank shape.
    CohereCompatible,
}

impl RerankerProtocol {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenaiCompatible => "openai_compatible",
            Self::CohereCompatible => "cohere_compatible",
        }
    }
}

/// Profile fallback strategy. v1 only supports deterministic ordered fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalFallbackStrategy {
    /// Try routes in declared order; never retarget to siblings or built-ins.
    #[default]
    Deterministic,
}

impl RetrievalFallbackStrategy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Deterministic => "deterministic",
        }
    }
}

// ---------------------------------------------------------------------------
// Embedding / reranker model configs
// ---------------------------------------------------------------------------

/// Named embedding model entry (`[embedding_models.<id>]`).
///
/// Credential-free: `provider` is an exact provider-instance id resolved
/// through the registry. No endpoints, keys, or headers live here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EmbeddingModelConfig {
    /// Exact provider instance id (`[model_providers.<id>]` or built-in).
    pub provider: String,
    /// Upstream model slug as the provider expects it.
    pub model: String,
    pub protocol: EmbeddingProtocol,
    /// Optional fixed dimensions (provider-specific).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<u32>,
    pub encoding: EmbeddingEncoding,
    pub batch_size: u32,
    pub max_input_tokens: u32,
}

impl Default for EmbeddingModelConfig {
    fn default() -> Self {
        Self {
            provider: String::new(),
            model: String::new(),
            protocol: EmbeddingProtocol::default(),
            dimensions: None,
            encoding: EmbeddingEncoding::default(),
            batch_size: DEFAULT_EMBEDDING_BATCH_SIZE,
            max_input_tokens: DEFAULT_MAX_INPUT_TOKENS,
        }
    }
}

/// Named reranker model entry (`[reranker_models.<id>]`).
///
/// Optional `endpoint` is a validated *relative* path only (no scheme, origin,
/// authority, `..`, query, fragment, backslash, or control characters).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RerankerModelConfig {
    /// Exact provider instance id.
    pub provider: String,
    /// Upstream model slug.
    pub model: String,
    pub protocol: RerankerProtocol,
    /// Optional relative path (e.g. `rerank` or `v1/rerank`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    pub batch_size: u32,
    pub max_input_tokens: u32,
}

impl Default for RerankerModelConfig {
    fn default() -> Self {
        Self {
            provider: String::new(),
            model: String::new(),
            protocol: RerankerProtocol::default(),
            endpoint: None,
            batch_size: DEFAULT_EMBEDDING_BATCH_SIZE,
            max_input_tokens: DEFAULT_MAX_INPUT_TOKENS,
        }
    }
}

// ---------------------------------------------------------------------------
// Retrieval profiles
// ---------------------------------------------------------------------------

/// Named retrieval profile (`[retrieval_profiles.<id>]`).
///
/// Routes reference ordered embedding/reranker model ids. Fallback is
/// deterministic only in v1. Budgets are profile-wide and strict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RetrievalProfileConfig {
    /// Ordered embedding model ids (primary first).
    pub embedding_models: Vec<String>,
    /// Ordered reranker model ids (may be empty).
    pub reranker_models: Vec<String>,
    pub fallback_strategy: RetrievalFallbackStrategy,
    /// Candidate pool size before rerank / final cut.
    pub max_candidates: u32,
    /// Final result limit returned to consumers.
    pub max_results: u32,
    /// Semantic minimum score (0.0..=1.0).
    pub min_score: f32,
    /// Strict profile-wide deadline in milliseconds.
    pub deadline_ms: u64,
    /// Maximum attempts across the ordered fallback chain.
    pub max_attempts: u32,
    /// Aggregate input token budget for the profile.
    pub max_input_tokens: u32,
    /// Aggregate output token budget for the profile.
    pub max_output_tokens: u32,
}

impl Default for RetrievalProfileConfig {
    fn default() -> Self {
        Self {
            embedding_models: Vec::new(),
            reranker_models: Vec::new(),
            fallback_strategy: RetrievalFallbackStrategy::default(),
            max_candidates: DEFAULT_MAX_CANDIDATES,
            max_results: DEFAULT_MAX_RESULTS,
            min_score: 0.0,
            deadline_ms: DEFAULT_DEADLINE_MS,
            max_attempts: DEFAULT_MAX_ATTEMPTS,
            max_input_tokens: DEFAULT_MAX_INPUT_TOKENS,
            max_output_tokens: DEFAULT_MAX_OUTPUT_TOKENS,
        }
    }
}

// ---------------------------------------------------------------------------
// Prime consumers
// ---------------------------------------------------------------------------

/// Skill-prime retrieval settings (`[prime.skills]`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SkillPrimeConfig {
    pub enabled: bool,
    /// Named retrieval profile id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieval_profile: Option<String>,
    pub max_results: u32,
    /// Max characters per skill body snippet.
    pub max_body_chars: u32,
    /// Max total characters across all injected skill bodies.
    pub max_total_chars: u32,
    /// Token budget for skill prime injection.
    pub max_tokens: u32,
    /// Fraction of context window reserved for skill prime (0.0..=1.0).
    pub max_context_fraction: f32,
    pub deadline_ms: u64,
    /// When true, degrade (omit) rather than fail the turn on retrieval errors.
    pub degrade_on_error: bool,
}

impl Default for SkillPrimeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            retrieval_profile: None,
            max_results: 3,
            max_body_chars: 2_000,
            max_total_chars: 6_000,
            max_tokens: 1_500,
            max_context_fraction: 0.05,
            deadline_ms: 3_000,
            degrade_on_error: true,
        }
    }
}

/// Agent-prime retrieval settings (`[prime.agents]`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentPrimeConfig {
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieval_profile: Option<String>,
    pub max_results: u32,
    pub max_body_chars: u32,
    pub max_total_chars: u32,
    pub max_tokens: u32,
    pub max_context_fraction: f32,
    pub deadline_ms: u64,
    pub degrade_on_error: bool,
}

impl Default for AgentPrimeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            retrieval_profile: None,
            max_results: 3,
            max_body_chars: 2_000,
            max_total_chars: 6_000,
            max_tokens: 1_500,
            max_context_fraction: 0.05,
            deadline_ms: 3_000,
            degrade_on_error: true,
        }
    }
}

/// Aggregate prime configuration (`[prime]`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PrimeConfig {
    pub skills: SkillPrimeConfig,
    pub agents: AgentPrimeConfig,
}

// ---------------------------------------------------------------------------
// Aggregate graph (parse/management convenience)
// ---------------------------------------------------------------------------

/// Complete named retrieval graph as loaded from config.
///
/// Maps preserve insertion order for reorder UX and comment-preserving writes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct RetrievalGraphConfig {
    pub embedding_models: indexmap::IndexMap<String, EmbeddingModelConfig>,
    pub reranker_models: indexmap::IndexMap<String, RerankerModelConfig>,
    pub retrieval_profiles: indexmap::IndexMap<String, RetrievalProfileConfig>,
    pub prime: PrimeConfig,
    /// Optional memory retrieval-profile selection (`[memory] retrieval_profile`).
    /// Additive: legacy `[memory.embedding]` / `[memory.search]` remain authoritative
    /// when this is absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_retrieval_profile: Option<String>,
}

// ---------------------------------------------------------------------------
// Relative endpoint validation (shared pure helper)
// ---------------------------------------------------------------------------

/// Validate a reranker relative endpoint path.
///
/// Rejects scheme/origin override, authority, `..`, query/fragment, backslash,
/// and control characters. Leading `/` is allowed (absolute-on-origin path).
pub fn validate_relative_endpoint(path: &str) -> Result<(), String> {
    let p = path.trim();
    if p.is_empty() {
        return Err("endpoint path must be non-empty".into());
    }
    if p.contains('\\') {
        return Err("endpoint path must not contain backslash".into());
    }
    if p.contains("://") || p.starts_with("//") {
        return Err("endpoint path must not include a scheme or authority".into());
    }
    if p.contains('?') || p.contains('#') {
        return Err("endpoint path must not include query or fragment".into());
    }
    if p.chars().any(|c| c.is_control() || c == '\0') {
        return Err("endpoint path must not contain control characters".into());
    }
    // Reject path traversal and empty segments that could escape.
    for seg in p.split('/') {
        if seg == ".." {
            return Err("endpoint path must not contain '..' segments".into());
        }
    }
    // Reject host-like prefixes (user@host or host:port without scheme).
    if let Some(first) = p.trim_start_matches('/').split('/').next() {
        if first.contains('@') {
            return Err("endpoint path must not include host or authority".into());
        }
        if first
            .split_once(':')
            .is_some_and(|(h, port)| !h.is_empty() && port.parse::<u16>().is_ok())
        {
            return Err("endpoint path must not include host or authority".into());
        }
    }
    Ok(())
}

/// Normalize a retrieval entity id: trim, lowercase ASCII, validate slug.
pub fn normalize_retrieval_id(raw: &str) -> Result<String, String> {
    let s = raw.trim().to_ascii_lowercase();
    if s.is_empty() {
        return Err("id must be non-empty".into());
    }
    if s.len() > MAX_RETRIEVAL_ID_LEN {
        return Err(format!(
            "id exceeds maximum length of {MAX_RETRIEVAL_ID_LEN}"
        ));
    }
    if !s
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
    {
        return Err("id must be a lowercase ASCII slug (letters, digits, '_', '-')".into());
    }
    if s.starts_with('-') || s.ends_with('-') {
        return Err("id must not start or end with '-'".into());
    }
    Ok(s)
}

/// Clamp score to [0.0, 1.0].
pub fn clamp_unit_score(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// Clamp context fraction to [0.0, MAX_CONTEXT_FRACTION].
pub fn clamp_context_fraction(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, MAX_CONTEXT_FRACTION)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedding_defaults_and_serde_snake_case() {
        let e = EmbeddingModelConfig {
            provider: "openai".into(),
            model: "text-embedding-3-small".into(),
            ..Default::default()
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["protocol"], "openai_compatible");
        assert_eq!(v["encoding"], "float");
        assert_eq!(v["batch_size"], DEFAULT_EMBEDDING_BATCH_SIZE);
        let back: EmbeddingModelConfig = serde_json::from_value(v).unwrap();
        assert_eq!(back, e);
    }

    #[test]
    fn reranker_protocol_roundtrip() {
        let r = RerankerModelConfig {
            provider: "cohere-lab".into(),
            model: "rerank-v3".into(),
            protocol: RerankerProtocol::CohereCompatible,
            endpoint: Some("v1/rerank".into()),
            ..Default::default()
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["protocol"], "cohere_compatible");
        let back: RerankerModelConfig = serde_json::from_value(v).unwrap();
        assert_eq!(back.protocol, RerankerProtocol::CohereCompatible);
    }

    #[test]
    fn fallback_strategy_only_deterministic() {
        let p: RetrievalProfileConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(
            p.fallback_strategy,
            RetrievalFallbackStrategy::Deterministic
        );
        assert!(serde_json::from_str::<RetrievalFallbackStrategy>(r#""random""#).is_err());
    }

    #[test]
    fn endpoint_rejects_attacks() {
        assert!(validate_relative_endpoint("rerank").is_ok());
        assert!(validate_relative_endpoint("/v1/rerank").is_ok());
        assert!(validate_relative_endpoint("v1/rerank").is_ok());
        assert!(validate_relative_endpoint("https://evil/x").is_err());
        assert!(validate_relative_endpoint("//evil/x").is_err());
        assert!(validate_relative_endpoint("../secret").is_err());
        assert!(validate_relative_endpoint("a/../b").is_err());
        assert!(validate_relative_endpoint("x?q=1").is_err());
        assert!(validate_relative_endpoint("x#frag").is_err());
        assert!(validate_relative_endpoint("a\\b").is_err());
        assert!(validate_relative_endpoint("host:443/path").is_err());
        assert!(validate_relative_endpoint("user@host/path").is_err());
        assert!(validate_relative_endpoint("").is_err());
        assert!(validate_relative_endpoint("a\nb").is_err());
    }

    #[test]
    fn normalize_id_lowercases_and_validates() {
        assert_eq!(normalize_retrieval_id("  Foo_Bar  ").unwrap(), "foo_bar");
        assert!(normalize_retrieval_id("").is_err());
        assert!(normalize_retrieval_id("Has Space").is_err());
        assert!(normalize_retrieval_id("Bad!").is_err());
        assert!(normalize_retrieval_id(&"a".repeat(MAX_RETRIEVAL_ID_LEN + 1)).is_err());
    }

    #[test]
    fn clamp_helpers() {
        assert_eq!(clamp_unit_score(1.5), 1.0);
        assert_eq!(clamp_unit_score(-0.1), 0.0);
        assert_eq!(clamp_context_fraction(2.0), 1.0);
        assert_eq!(clamp_unit_score(f32::NAN), 0.0);
    }

    #[test]
    fn prime_defaults_disabled() {
        let p = PrimeConfig::default();
        assert!(!p.skills.enabled);
        assert!(!p.agents.enabled);
        assert!(p.skills.degrade_on_error);
    }
}
