//! Secret-free retrieval request/result/error types and adapter traits.
//!
//! These interfaces are stable for PR17 clients. They never carry credentials,
//! Authorization headers, or raw request bodies in Debug / Display / errors.

use std::fmt;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

// ---------------------------------------------------------------------------
// Bounds (strict input/output limits for adapters)
// ---------------------------------------------------------------------------

/// Maximum texts in one embeddings request.
pub const MAX_EMBEDDING_INPUTS: usize = 512;
/// Maximum characters per embedding input string.
pub const MAX_EMBEDDING_INPUT_CHARS: usize = 1_000_000;
/// Maximum embedding dimensions accepted from config or response.
pub const MAX_EMBEDDING_DIMENSIONS: usize = 16_384;
/// Maximum documents in one rerank request.
pub const MAX_RERANK_DOCUMENTS: usize = 512;
/// Maximum characters per rerank query or document.
pub const MAX_RERANK_TEXT_CHARS: usize = 1_000_000;
/// Maximum top_n for rerank.
pub const MAX_RERANK_TOP_N: usize = 10_000;
/// Default max response body bytes for retrieval POSTs.
pub const DEFAULT_MAX_RESPONSE_BYTES: usize = 32 * 1024 * 1024;
/// Default max error body preview chars (redacted).
pub const DEFAULT_MAX_ERROR_PREVIEW_CHARS: usize = 512;
/// Default connect timeout.
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Default per-attempt request timeout.
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
/// Default total deadline covering all attempts/backoff.
pub const DEFAULT_TOTAL_DEADLINE: Duration = Duration::from_secs(120);
/// Default max retries for retry-safe retrieval POSTs.
pub const DEFAULT_MAX_RETRIES: u32 = 2;
/// Default same-origin redirect cap.
pub const DEFAULT_MAX_REDIRECTS: usize = 3;
/// Default relative embeddings path (joined onto provider base).
pub const DEFAULT_EMBEDDINGS_PATH: &str = "/embeddings";
/// Default relative rerank path (joined onto provider base).
pub const DEFAULT_RERANK_PATH: &str = "/rerank";

// ---------------------------------------------------------------------------
// Auth / encoding (wire schemes; never secret values)
// ---------------------------------------------------------------------------

/// How the resolved route injects application credentials on retrieval POSTs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetrievalAuthScheme {
    /// `Authorization: Bearer <token>`
    Bearer,
    /// `x-api-key: <token>`
    XApiKey,
    /// Named custom header with credential value. Name is non-secret config.
    CustomHeader { name: String },
    /// No Authorization or credential header is sent.
    None,
}

impl RetrievalAuthScheme {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Bearer => "bearer",
            Self::XApiKey => "x-api-key",
            Self::CustomHeader { .. } => "custom_header",
            Self::None => "none",
        }
    }
}

/// Embedding wire encoding requested of the upstream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EmbeddingEncodingFormat {
    #[default]
    Float,
    Base64,
}

impl EmbeddingEncodingFormat {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Float => "float",
            Self::Base64 => "base64",
        }
    }
}

// ---------------------------------------------------------------------------
// Route context (secret-free; suitable for Debug / DTOs)
// ---------------------------------------------------------------------------

/// Secret-free resolved route metadata for a retrieval call.
///
/// Never carries credential values. Built by the shell exact-route resolver.
#[derive(Clone, PartialEq, Eq)]
pub struct RetrievalRouteContext {
    pub provider_instance_id: String,
    pub provider_kind: String,
    pub api_surface: String,
    pub credential_route: String,
    pub auth_scheme: RetrievalAuthScheme,
    /// Normalized base URL string (no embedded credentials).
    pub base_url: String,
    pub display_name: String,
    pub organization: Option<String>,
    pub project: Option<String>,
    /// Validated non-secret extra headers (restricted names already stripped).
    pub extra_headers: Vec<(String, String)>,
    pub incarnation: Option<String>,
    pub registry_generation: u64,
    pub request_timeout: Duration,
    pub connect_timeout: Duration,
    pub total_deadline: Duration,
    pub max_retries: u32,
    pub max_redirects: usize,
    pub max_response_bytes: usize,
    pub purpose: RetrievalPurpose,
}

impl fmt::Debug for RetrievalRouteContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RetrievalRouteContext")
            .field("provider_instance_id", &self.provider_instance_id)
            .field("provider_kind", &self.provider_kind)
            .field("api_surface", &self.api_surface)
            .field("credential_route", &self.credential_route)
            .field("auth_scheme", &self.auth_scheme)
            .field("base_url", &self.base_url)
            .field("display_name", &self.display_name)
            .field("organization", &self.organization)
            .field("project", &self.project)
            .field("extra_headers_count", &self.extra_headers.len())
            .field("incarnation", &self.incarnation)
            .field("registry_generation", &self.registry_generation)
            .field("request_timeout", &self.request_timeout)
            .field("connect_timeout", &self.connect_timeout)
            .field("total_deadline", &self.total_deadline)
            .field("max_retries", &self.max_retries)
            .field("max_redirects", &self.max_redirects)
            .field("max_response_bytes", &self.max_response_bytes)
            .field("purpose", &self.purpose)
            .finish()
    }
}

/// Bounded retrieval purpose (telemetry / routing partition).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RetrievalPurpose {
    Embeddings,
    Rerank,
}

impl RetrievalPurpose {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Embeddings => "embeddings",
            Self::Rerank => "rerank",
        }
    }
}

// ---------------------------------------------------------------------------
// Requests / results
// ---------------------------------------------------------------------------

/// Embedding batch request (handwritten; model is free-form upstream slug).
#[derive(Clone, PartialEq)]
pub struct EmbeddingRequest {
    pub model: String,
    pub inputs: Vec<String>,
    pub dimensions: Option<u32>,
    pub encoding: EmbeddingEncodingFormat,
    /// Relative path (default `/embeddings`). Must pass same-origin join policy.
    pub endpoint: String,
}

impl fmt::Debug for EmbeddingRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EmbeddingRequest")
            .field("model", &self.model)
            .field("input_count", &self.inputs.len())
            .field(
                "input_chars",
                &self.inputs.iter().map(|s| s.len()).sum::<usize>(),
            )
            .field("dimensions", &self.dimensions)
            .field("encoding", &self.encoding)
            .field("endpoint", &self.endpoint)
            .finish()
    }
}

impl Default for EmbeddingRequest {
    fn default() -> Self {
        Self {
            model: String::new(),
            inputs: Vec::new(),
            dimensions: None,
            encoding: EmbeddingEncodingFormat::Float,
            endpoint: DEFAULT_EMBEDDINGS_PATH.to_owned(),
        }
    }
}

/// One embedding vector aligned to the original input index.
#[derive(Clone, PartialEq)]
pub struct EmbeddingVector {
    pub index: usize,
    pub values: Vec<f32>,
}

impl fmt::Debug for EmbeddingVector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EmbeddingVector")
            .field("index", &self.index)
            .field("dimensions", &self.values.len())
            .finish()
    }
}

/// Embedding batch result: vectors ordered by original input index.
#[derive(Clone, PartialEq)]
pub struct EmbeddingResult {
    pub model: String,
    pub vectors: Vec<EmbeddingVector>,
}

impl fmt::Debug for EmbeddingResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EmbeddingResult")
            .field("model", &self.model)
            .field("vector_count", &self.vectors.len())
            .field(
                "dimensions",
                &self.vectors.first().map(|v| v.values.len()).unwrap_or(0),
            )
            .finish()
    }
}

/// Rerank request (query + documents).
#[derive(Clone, PartialEq)]
pub struct RerankRequest {
    pub model: String,
    pub query: String,
    pub documents: Vec<String>,
    pub top_n: Option<u32>,
    /// Relative path (default `/rerank`).
    pub endpoint: String,
    /// When true, request optional returned document text (never trusted as
    /// authoritative for identity; original documents/index mapping wins).
    pub return_documents: bool,
}

impl fmt::Debug for RerankRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RerankRequest")
            .field("model", &self.model)
            .field("query_chars", &self.query.len())
            .field("document_count", &self.documents.len())
            .field("top_n", &self.top_n)
            .field("endpoint", &self.endpoint)
            .field("return_documents", &self.return_documents)
            .finish()
    }
}

impl Default for RerankRequest {
    fn default() -> Self {
        Self {
            model: String::new(),
            query: String::new(),
            documents: Vec::new(),
            top_n: None,
            endpoint: DEFAULT_RERANK_PATH.to_owned(),
            return_documents: false,
        }
    }
}

/// One rerank hit. `document` is optional echo from upstream (informational).
#[derive(Clone, PartialEq)]
pub struct RerankHit {
    /// Source index into the original request documents.
    pub index: usize,
    pub score: f32,
    /// Optional returned document text; never used as authoritative identity.
    pub document: Option<String>,
}

impl fmt::Debug for RerankHit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RerankHit")
            .field("index", &self.index)
            .field("score", &self.score)
            .field("has_document", &self.document.is_some())
            .finish()
    }
}

/// Rerank result preserving original document/index mapping semantics.
#[derive(Clone, PartialEq)]
pub struct RerankResult {
    pub model: String,
    pub hits: Vec<RerankHit>,
}

impl fmt::Debug for RerankResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RerankResult")
            .field("model", &self.model)
            .field("hit_count", &self.hits.len())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Errors (secret-free)
// ---------------------------------------------------------------------------

/// Fail-closed retrieval error. Display/Debug never include credentials.
#[derive(Clone, PartialEq, Eq)]
pub enum RetrievalError {
    InvalidRequest(String),
    InvalidUrl(String),
    MissingCredential,
    CapabilityDenied(String),
    SurfaceMismatch(String),
    ProtocolMismatch(String),
    RedirectPolicy(String),
    Http {
        status: u16,
        category: RetrievalErrorCategory,
        message: String,
        request_id: Option<String>,
        provider_id: Option<String>,
    },
    Decode(String),
    MalformedResponse(String),
    Timeout,
    Cancelled,
    RateLimited {
        retry_after_ms: Option<u64>,
    },
    OversizedResponse {
        limit_bytes: usize,
    },
    Transport(String),
    DeadlineExceeded,
}

/// Safe HTTP category for telemetry / UI (never a raw body).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetrievalErrorCategory {
    Authentication,
    Authorization,
    Validation,
    RateLimit,
    Server,
    NotFound,
    Unknown,
}

impl RetrievalErrorCategory {
    pub fn from_status(status: u16) -> Self {
        match status {
            401 => Self::Authentication,
            403 => Self::Authorization,
            404 => Self::NotFound,
            400 | 422 => Self::Validation,
            429 => Self::RateLimit,
            // 408 Request Timeout is treated as transient server/transport-class
            // for retry classification (same as platform-adjacent fail-open retry).
            408 => Self::Server,
            s if (500..600).contains(&s) => Self::Server,
            _ => Self::Unknown,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Authentication => "authentication",
            Self::Authorization => "authorization",
            Self::Validation => "validation",
            Self::RateLimit => "rate_limit",
            Self::Server => "server",
            Self::NotFound => "not_found",
            Self::Unknown => "unknown",
        }
    }
}

impl fmt::Debug for RetrievalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest(_) => f.write_str("InvalidRequest"),
            Self::InvalidUrl(_) => f.write_str("InvalidUrl"),
            Self::MissingCredential => f.write_str("MissingCredential"),
            Self::CapabilityDenied(_) => f.write_str("CapabilityDenied"),
            Self::SurfaceMismatch(_) => f.write_str("SurfaceMismatch"),
            Self::ProtocolMismatch(_) => f.write_str("ProtocolMismatch"),
            Self::RedirectPolicy(_) => f.write_str("RedirectPolicy"),
            Self::Http {
                status, category, ..
            } => f
                .debug_struct("Http")
                .field("status", status)
                .field("category", category)
                .finish(),
            Self::Decode(_) => f.write_str("Decode"),
            Self::MalformedResponse(_) => f.write_str("MalformedResponse"),
            Self::Timeout => f.write_str("Timeout"),
            Self::Cancelled => f.write_str("Cancelled"),
            Self::RateLimited { retry_after_ms } => f
                .debug_struct("RateLimited")
                .field("retry_after_ms", retry_after_ms)
                .finish(),
            Self::OversizedResponse { limit_bytes } => f
                .debug_struct("OversizedResponse")
                .field("limit_bytes", limit_bytes)
                .finish(),
            Self::Transport(_) => f.write_str("Transport"),
            Self::DeadlineExceeded => f.write_str("DeadlineExceeded"),
        }
    }
}

impl fmt::Display for RetrievalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest(_) => write!(f, "invalid retrieval request"),
            Self::InvalidUrl(_) => write!(f, "invalid retrieval URL"),
            Self::MissingCredential => write!(
                f,
                "application credential missing for exact retrieval route (never borrows siblings or admin)"
            ),
            Self::CapabilityDenied(_) => write!(f, "retrieval capability denied"),
            Self::SurfaceMismatch(_) => write!(f, "retrieval API surface mismatch"),
            Self::ProtocolMismatch(_) => write!(f, "retrieval protocol mismatch"),
            Self::RedirectPolicy(_) => write!(f, "retrieval redirect refused"),
            Self::Http {
                status, category, ..
            } => {
                write!(f, "retrieval HTTP {status} ({})", category.as_str())
            }
            Self::Decode(_) => write!(f, "retrieval decode error"),
            Self::MalformedResponse(_) => write!(f, "malformed retrieval response"),
            Self::Timeout => write!(f, "retrieval request timed out"),
            Self::Cancelled => write!(f, "retrieval request cancelled"),
            Self::RateLimited { .. } => write!(f, "retrieval rate limited"),
            Self::OversizedResponse { limit_bytes } => {
                write!(f, "retrieval response exceeded {limit_bytes} bytes")
            }
            Self::Transport(_) => write!(f, "retrieval transport error"),
            Self::DeadlineExceeded => write!(f, "retrieval total deadline exceeded"),
        }
    }
}

impl std::error::Error for RetrievalError {}

impl RetrievalError {
    /// Internal diagnostic payload. Never used on user-facing Display paths.
    pub fn internal_message(&self) -> Option<&str> {
        match self {
            Self::InvalidRequest(m)
            | Self::InvalidUrl(m)
            | Self::CapabilityDenied(m)
            | Self::SurfaceMismatch(m)
            | Self::ProtocolMismatch(m)
            | Self::RedirectPolicy(m)
            | Self::Decode(m)
            | Self::MalformedResponse(m)
            | Self::Transport(m) => Some(m),
            Self::Http { message, .. } => Some(message),
            _ => None,
        }
    }

    /// Whether a failed POST may be retried (request construction is retry-safe).
    ///
    /// `DeadlineExceeded` is **not** retryable: the total call budget is already
    /// exhausted and another attempt would only multiply latency.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::RateLimited { .. } | Self::Timeout | Self::Transport(_) => true,
            Self::Http { category, .. } => matches!(
                category,
                RetrievalErrorCategory::RateLimit | RetrievalErrorCategory::Server
            ),
            // Never retry missing credentials, 400/schema/validation, 401/403,
            // cross-origin redirect, cancellation, malformed response, or a
            // total-deadline terminal gate.
            Self::MissingCredential
            | Self::InvalidRequest(_)
            | Self::InvalidUrl(_)
            | Self::CapabilityDenied(_)
            | Self::SurfaceMismatch(_)
            | Self::ProtocolMismatch(_)
            | Self::RedirectPolicy(_)
            | Self::Decode(_)
            | Self::MalformedResponse(_)
            | Self::Cancelled
            | Self::OversizedResponse { .. }
            | Self::DeadlineExceeded => false,
        }
    }
}

pub type RetrievalResult<T> = Result<T, RetrievalError>;

// ---------------------------------------------------------------------------
// Adapter traits (PR17-facing)
// ---------------------------------------------------------------------------

/// Async embeddings adapter bound to one exact resolved route.
///
/// Credentials are passed per call (short-lived, secret-bearing, redacted
/// Debug). Config/profile DTOs never carry secrets.
pub trait EmbeddingAdapter: Send + Sync {
    fn embed(
        &self,
        request: EmbeddingRequest,
        credential: &crate::retrieval::transport::RetrievalCredential,
        cancel: CancellationToken,
    ) -> impl std::future::Future<Output = RetrievalResult<EmbeddingResult>> + Send;

    fn route_context(&self) -> &RetrievalRouteContext;
}

/// Async reranker adapter bound to one exact resolved route.
///
/// Credentials are passed per call (short-lived, secret-bearing, redacted
/// Debug). Config/profile DTOs never carry secrets.
pub trait RerankAdapter: Send + Sync {
    fn rerank(
        &self,
        request: RerankRequest,
        credential: &crate::retrieval::transport::RetrievalCredential,
        cancel: CancellationToken,
    ) -> impl std::future::Future<Output = RetrievalResult<RerankResult>> + Send;

    fn route_context(&self) -> &RetrievalRouteContext;
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

/// Validate embedding request bounds before any network I/O.
pub fn validate_embedding_request(req: &EmbeddingRequest) -> RetrievalResult<()> {
    if req.model.trim().is_empty() {
        return Err(RetrievalError::InvalidRequest(
            "embedding model must be non-empty".into(),
        ));
    }
    if req.inputs.is_empty() {
        return Err(RetrievalError::InvalidRequest(
            "embedding inputs must be non-empty".into(),
        ));
    }
    if req.inputs.len() > MAX_EMBEDDING_INPUTS {
        return Err(RetrievalError::InvalidRequest(format!(
            "embedding input count {} exceeds max {MAX_EMBEDDING_INPUTS}",
            req.inputs.len()
        )));
    }
    for (i, text) in req.inputs.iter().enumerate() {
        if text.len() > MAX_EMBEDDING_INPUT_CHARS {
            return Err(RetrievalError::InvalidRequest(format!(
                "embedding input[{i}] exceeds max chars {MAX_EMBEDDING_INPUT_CHARS}"
            )));
        }
    }
    if let Some(dims) = req.dimensions {
        if dims == 0 || dims as usize > MAX_EMBEDDING_DIMENSIONS {
            return Err(RetrievalError::InvalidRequest(format!(
                "embedding dimensions {dims} out of range 1..={MAX_EMBEDDING_DIMENSIONS}"
            )));
        }
    }
    validate_relative_endpoint_path(&req.endpoint)?;
    Ok(())
}

/// Validate rerank request bounds before any network I/O.
pub fn validate_rerank_request(req: &RerankRequest) -> RetrievalResult<()> {
    if req.model.trim().is_empty() {
        return Err(RetrievalError::InvalidRequest(
            "rerank model must be non-empty".into(),
        ));
    }
    if req.query.is_empty() {
        return Err(RetrievalError::InvalidRequest(
            "rerank query must be non-empty".into(),
        ));
    }
    if req.query.len() > MAX_RERANK_TEXT_CHARS {
        return Err(RetrievalError::InvalidRequest(format!(
            "rerank query exceeds max chars {MAX_RERANK_TEXT_CHARS}"
        )));
    }
    if req.documents.is_empty() {
        return Err(RetrievalError::InvalidRequest(
            "rerank documents must be non-empty".into(),
        ));
    }
    if req.documents.len() > MAX_RERANK_DOCUMENTS {
        return Err(RetrievalError::InvalidRequest(format!(
            "rerank document count {} exceeds max {MAX_RERANK_DOCUMENTS}",
            req.documents.len()
        )));
    }
    for (i, doc) in req.documents.iter().enumerate() {
        if doc.len() > MAX_RERANK_TEXT_CHARS {
            return Err(RetrievalError::InvalidRequest(format!(
                "rerank document[{i}] exceeds max chars {MAX_RERANK_TEXT_CHARS}"
            )));
        }
    }
    if let Some(top_n) = req.top_n {
        if top_n == 0 || top_n as usize > MAX_RERANK_TOP_N {
            return Err(RetrievalError::InvalidRequest(format!(
                "rerank top_n {top_n} out of range 1..={MAX_RERANK_TOP_N}"
            )));
        }
    }
    validate_relative_endpoint_path(&req.endpoint)?;
    Ok(())
}

/// Normalize and validate a relative endpoint path for same-origin join.
pub fn validate_relative_endpoint_path(path: &str) -> RetrievalResult<()> {
    let p = path.trim();
    if p.is_empty() {
        return Err(RetrievalError::InvalidUrl(
            "endpoint path must be non-empty".into(),
        ));
    }
    if p.contains('\\') {
        return Err(RetrievalError::InvalidUrl(
            "endpoint path must not contain backslash".into(),
        ));
    }
    if p.contains("://") || p.starts_with("//") {
        return Err(RetrievalError::InvalidUrl(
            "endpoint path must not include a scheme or authority".into(),
        ));
    }
    if p.contains('?') || p.contains('#') {
        return Err(RetrievalError::InvalidUrl(
            "endpoint path must not include query or fragment".into(),
        ));
    }
    if p.chars().any(|c| c.is_control() || c == '\0') {
        return Err(RetrievalError::InvalidUrl(
            "endpoint path must not contain control characters".into(),
        ));
    }
    for seg in p.split('/') {
        if seg == ".." {
            return Err(RetrievalError::InvalidUrl(
                "endpoint path must not contain '..' segments".into(),
            ));
        }
    }
    if let Some(first) = p.trim_start_matches('/').split('/').next() {
        if first.contains('@') {
            return Err(RetrievalError::InvalidUrl(
                "endpoint path must not include host or authority".into(),
            ));
        }
        if first
            .split_once(':')
            .is_some_and(|(h, port)| !h.is_empty() && port.parse::<u16>().is_ok())
        {
            return Err(RetrievalError::InvalidUrl(
                "endpoint path must not include host or authority".into(),
            ));
        }
    }
    Ok(())
}

/// Ensure a relative path starts with `/` for join_path.
pub fn normalize_endpoint_path(path: &str) -> String {
    let p = path.trim();
    if p.starts_with('/') {
        p.to_owned()
    } else {
        format!("/{p}")
    }
}

/// Bounded, case-insensitive token/window redaction for error previews.
///
/// Stronger than needle-only replace: when a secret-bearing marker is found
/// (case-insensitive), the marker **and** the following token (or a fixed
/// window of up to 64 bytes) are replaced with `[redacted]` so Bearer tokens,
/// API keys, JWTs, and query material cannot survive in Display/Debug.
pub fn redact_error_preview(raw: &str, max_chars: usize) -> String {
    let markers = [
        "authorization:",
        "authorization=",
        "bearer ",
        "api-key:",
        "api_key=",
        "api-key=",
        "x-api-key:",
        "x-api-key=",
        "sk-",
        "xai-",
        "eyj", // JWT header prefix (base64url of `{"`)
        "token=",
        "access_token=",
        "refresh_token=",
        "password=",
        "secret=",
    ];
    let mut out = raw.to_string();
    // Iterate until no more markers (bounded passes).
    for _ in 0..16 {
        let lower = out.to_ascii_lowercase();
        let mut best: Option<(usize, usize)> = None;
        for m in markers {
            if let Some(idx) = lower.find(m) {
                let start = idx;
                // Consume marker + following non-whitespace token, min 8 bytes
                // after marker, max 64 after marker start of secret material.
                let after_marker = start + m.len();
                let rest = out.get(after_marker..).unwrap_or("");
                let token_len = rest
                    .chars()
                    .take_while(|c| {
                        !c.is_whitespace() && *c != '"' && *c != '\'' && *c != ',' && *c != '}'
                    })
                    .map(|c| c.len_utf8())
                    .sum::<usize>()
                    .max(8)
                    .min(64);
                let end = (after_marker + token_len).min(out.len());
                let cand = (start, end);
                best = Some(match best {
                    None => cand,
                    Some((bs, _be)) if start < bs => cand,
                    Some(prev) => prev,
                });
            }
        }
        let Some((start, end)) = best else {
            break;
        };
        if start >= end || end > out.len() {
            break;
        }
        out.replace_range(start..end, "[redacted]");
    }
    if out.chars().count() > max_chars {
        let trimmed: String = out.chars().take(max_chars).collect();
        format!("{trimmed}…")
    } else {
        out
    }
}

#[cfg(test)]
mod redact_tests {
    use super::*;

    #[test]
    fn redacts_bearer_and_sk_tokens_case_insensitive() {
        let cases = [
            "Bearer sk-secret-value-here rejected",
            "bearer SK-SECRET-VALUE-HERE rejected",
            "BEARER sk-secret-value-here rejected",
            "Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.payload.sig",
            "api_key=super-secret-key-material",
            "x-api-key: abcdefghijklmnop",
            "?access_token=tok_abc123xyz&x=1",
        ];
        for c in cases {
            let r = redact_error_preview(c, 256);
            let lower = r.to_ascii_lowercase();
            assert!(
                !lower.contains("sk-secret")
                    && !lower.contains("super-secret")
                    && !lower.contains("abcdefghijklmnop")
                    && !lower.contains("tok_abc123")
                    && !lower.contains("eyjhbGci"),
                "leaked in redaction of `{c}` → `{r}`"
            );
            assert!(r.contains("[redacted]"), "expected marker in `{r}`");
        }
    }

    #[test]
    fn retrieval_error_display_and_debug_are_classification_only() {
        let err = RetrievalError::Http {
            status: 401,
            category: RetrievalErrorCategory::Authentication,
            message: "https://embed.example/v1 leaked-token sk-secret".into(),
            request_id: None,
            provider_id: Some("prov".into()),
        };
        let display = err.to_string();
        let debug = format!("{err:?}");
        assert!(!display.contains("sk-secret"));
        assert!(!display.contains("https://"));
        assert!(!debug.contains("sk-secret"));
        assert!(!debug.contains("https://"));
        assert!(display.contains("401"));
        let url = RetrievalError::InvalidUrl("https://embed.example/v1".into());
        assert!(!url.to_string().contains("https://"));
        assert!(!format!("{url:?}").contains("https://"));
        assert_eq!(err.internal_message().unwrap().contains("sk-secret"), true);
    }
}
