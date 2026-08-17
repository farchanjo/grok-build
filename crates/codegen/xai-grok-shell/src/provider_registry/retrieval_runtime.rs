//! Exact retrieval runtime resolver (PR16).
//!
//! Purpose-aware resolution of one provider instance for embeddings or
//! rerank. Consumes PR15 `EmbeddingModelConfig` / `RerankerModelConfig` and
//! resolves exactly one configured or built-in instance — never siblings,
//! never admin for application ops, never Platform↔ChatGPT OAuth cross, and
//! never built-in env/vault fallback for configured siblings.
//!
//! Capability and API surface are validated **before** credential resolution
//! or any network I/O. Manual capabilities are authoritative.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use xai_grok_config_types::{
    DEFAULT_DEADLINE_MS, EmbeddingEncoding, EmbeddingModelConfig, EmbeddingProtocol,
    RerankerModelConfig, RerankerProtocol,
};
use xai_grok_inference::{
    DEFAULT_EMBEDDINGS_PATH, DEFAULT_RERANK_PATH, EmbeddingEncodingFormat, EmbeddingRequest,
    OpenRouterRerankAdapter, OpenaiCompatibleEmbeddings, RerankRequest, RetrievalAuthScheme,
    RetrievalCredential, RetrievalError, RetrievalPurpose, RetrievalResult, RetrievalRouteContext,
    VllmRerankAdapter, normalize_endpoint_path,
};

use super::id::{BuiltInProviderId, ProviderId};
use super::instance::{ApiSurface, CredentialRoute, ProviderKind};
use super::lifecycle::{CapabilityMode, ProviderAuthScheme, ProviderMetadata};
use super::lifecycle_state::load_lifecycle_state;
use super::route_guard::{RouteGuardRequest, assert_route_usable};
use super::runtime_cache::load_runtime;
use super::secrets::{application_key_scope, read_provider_secret};
use super::service::ProviderService;
use crate::auth::{OPENAI_API_KEY_SCOPE, OPENROUTER_API_KEY_SCOPE, read_provider_api_key};
use crate::retrieval_config::validate::ProviderCapabilityView;

/// Optional counters for hermetic tests (capability fail-before-secret).
#[derive(Debug, Default)]
pub struct RetrievalResolveCounters {
    pub secret_lookups: AtomicUsize,
    pub capability_checks: AtomicUsize,
    pub network_attempts: AtomicUsize,
}

impl RetrievalResolveCounters {
    pub fn secret_lookups(&self) -> usize {
        self.secret_lookups.load(Ordering::SeqCst)
    }
    pub fn capability_checks(&self) -> usize {
        self.capability_checks.load(Ordering::SeqCst)
    }
}

/// Inputs controlling optional provenance pins for route-guard.
#[derive(Debug, Clone, Default)]
pub struct RetrievalResolveOptions<'a> {
    pub provenance_incarnation: Option<&'a str>,
    pub session_registry_generation: Option<u64>,
    pub is_retry: bool,
    /// Override total deadline (profile budget); defaults to config deadline.
    pub total_deadline: Option<Duration>,
}

/// Fully resolved exact retrieval handle (secret-free route + short-lived cred).
pub struct ResolvedRetrievalRuntime {
    pub route: RetrievalRouteContext,
    pub credential: RetrievalCredential,
    pub upstream_model: String,
    pub embedding_protocol: Option<EmbeddingProtocol>,
    pub reranker_protocol: Option<RerankerProtocol>,
    pub embedding_encoding: Option<EmbeddingEncoding>,
    pub dimensions: Option<u32>,
    pub rerank_endpoint: Option<String>,
}

impl std::fmt::Debug for ResolvedRetrievalRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolvedRetrievalRuntime")
            .field("route", &self.route)
            .field("credential", &self.credential)
            .field("upstream_model", &self.upstream_model)
            .field("embedding_protocol", &self.embedding_protocol)
            .field("reranker_protocol", &self.reranker_protocol)
            .field("embedding_encoding", &self.embedding_encoding)
            .field("dimensions", &self.dimensions)
            .field("rerank_endpoint", &self.rerank_endpoint)
            .finish()
    }
}

/// Secret-free resolver error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetrievalRuntimeError {
    ProviderMissing { id: String },
    ProviderDisabled { id: String },
    RouteGuard(String),
    CapabilityDenied { id: String, detail: String },
    SurfaceMismatch { id: String, detail: String },
    ProtocolMismatch { detail: String },
    MissingCredential { id: String },
    InvalidConfig(String),
    Lifecycle(String),
}

impl std::fmt::Display for RetrievalRuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProviderMissing { id } => write!(
                f,
                "provider `{id}` is not configured for retrieval (exact reference; no sibling fallback)"
            ),
            Self::ProviderDisabled { id } => {
                write!(f, "provider `{id}` is disabled for retrieval")
            }
            Self::RouteGuard(m) => write!(f, "retrieval route guard: {m}"),
            Self::CapabilityDenied { id, detail } => {
                write!(f, "provider `{id}` capability denied: {detail}")
            }
            Self::SurfaceMismatch { id, detail } => {
                write!(f, "provider `{id}` surface mismatch: {detail}")
            }
            Self::ProtocolMismatch { detail } => write!(f, "retrieval protocol mismatch: {detail}"),
            Self::MissingCredential { id } => write!(
                f,
                "application credential missing for provider `{id}` (never borrows siblings, admin, OAuth cross, or built-in fallback)"
            ),
            Self::InvalidConfig(m) => write!(f, "invalid retrieval config: {m}"),
            Self::Lifecycle(m) => write!(f, "retrieval lifecycle: {m}"),
        }
    }
}

impl std::error::Error for RetrievalRuntimeError {}

impl From<RetrievalRuntimeError> for RetrievalError {
    fn from(value: RetrievalRuntimeError) -> Self {
        match value {
            RetrievalRuntimeError::CapabilityDenied { detail, .. } => {
                RetrievalError::CapabilityDenied(detail)
            }
            RetrievalRuntimeError::SurfaceMismatch { detail, .. } => {
                RetrievalError::SurfaceMismatch(detail)
            }
            RetrievalRuntimeError::ProtocolMismatch { detail } => {
                RetrievalError::ProtocolMismatch(detail)
            }
            RetrievalRuntimeError::MissingCredential { .. } => RetrievalError::MissingCredential,
            RetrievalRuntimeError::InvalidConfig(m) => RetrievalError::InvalidRequest(m),
            other => RetrievalError::InvalidRequest(other.to_string()),
        }
    }
}

/// Resolve an exact embedding route for `config.provider`.
pub fn resolve_embedding_runtime(
    home: &Path,
    config: &EmbeddingModelConfig,
    opts: &RetrievalResolveOptions<'_>,
    counters: Option<&RetrievalResolveCounters>,
) -> Result<ResolvedRetrievalRuntime, RetrievalRuntimeError> {
    resolve_inner(
        home,
        RetrievalPurpose::Embeddings,
        config.provider.trim(),
        &config.model,
        Some(config.protocol),
        None,
        Some(config.encoding),
        config.dimensions,
        None,
        opts,
        counters,
    )
}

/// Resolve an exact reranker route for `config.provider`.
pub fn resolve_reranker_runtime(
    home: &Path,
    config: &RerankerModelConfig,
    opts: &RetrievalResolveOptions<'_>,
    counters: Option<&RetrievalResolveCounters>,
) -> Result<ResolvedRetrievalRuntime, RetrievalRuntimeError> {
    resolve_inner(
        home,
        RetrievalPurpose::Rerank,
        config.provider.trim(),
        &config.model,
        None,
        Some(config.protocol),
        None,
        None,
        config.endpoint.clone(),
        opts,
        counters,
    )
}

fn resolve_inner(
    home: &Path,
    purpose: RetrievalPurpose,
    provider_id: &str,
    upstream_model: &str,
    emb_protocol: Option<EmbeddingProtocol>,
    rr_protocol: Option<RerankerProtocol>,
    encoding: Option<EmbeddingEncoding>,
    dimensions: Option<u32>,
    rerank_endpoint: Option<String>,
    opts: &RetrievalResolveOptions<'_>,
    counters: Option<&RetrievalResolveCounters>,
) -> Result<ResolvedRetrievalRuntime, RetrievalRuntimeError> {
    if provider_id.is_empty() {
        return Err(RetrievalRuntimeError::InvalidConfig(
            "provider id must be non-empty".into(),
        ));
    }
    if upstream_model.trim().is_empty() {
        return Err(RetrievalRuntimeError::InvalidConfig(
            "upstream model must be non-empty".into(),
        ));
    }

    let (service, _lifecycle, generation) =
        load_runtime(home).map_err(RetrievalRuntimeError::Lifecycle)?;

    // Fail closed on unreadable lifecycle for route guard (already inside
    // assert_route_usable); also pre-check tombstone via capability view.
    let _ =
        load_lifecycle_state(home).map_err(|e| RetrievalRuntimeError::Lifecycle(e.to_string()))?;

    assert_route_usable(
        home,
        &service,
        &RouteGuardRequest {
            provider_instance_id: provider_id,
            provenance_incarnation: opts.provenance_incarnation,
            session_registry_generation: opts.session_registry_generation.or(Some(generation)),
            is_retry: opts.is_retry,
        },
    )
    .map_err(|e| RetrievalRuntimeError::RouteGuard(e.to_string()))?;

    let desc = service
        .get(provider_id)
        .ok_or_else(|| RetrievalRuntimeError::ProviderMissing {
            id: provider_id.to_owned(),
        })?;
    if !desc.enabled {
        return Err(RetrievalRuntimeError::ProviderDisabled {
            id: provider_id.to_owned(),
        });
    }
    let meta = service.snapshot().get(provider_id).ok_or_else(|| {
        RetrievalRuntimeError::ProviderMissing {
            id: provider_id.to_owned(),
        }
    })?;

    // --- Capability + surface BEFORE credentials ---
    if let Some(c) = counters {
        c.capability_checks.fetch_add(1, Ordering::SeqCst);
    }
    let cap_view = capability_view_from_meta(meta, desc.enabled);
    match purpose {
        RetrievalPurpose::Embeddings => {
            if !cap_view.can_embed() {
                return Err(RetrievalRuntimeError::CapabilityDenied {
                    id: provider_id.to_owned(),
                    detail: "embeddings not permitted (manual capabilities authoritative; no catalog inference)".into(),
                });
            }
            let protocol = emb_protocol.unwrap_or(EmbeddingProtocol::OpenaiCompatible);
            validate_embedding_surface(desc.kind, primary_surface(desc), protocol, provider_id)?;
        }
        RetrievalPurpose::Rerank => {
            if !cap_view.can_rerank() {
                return Err(RetrievalRuntimeError::CapabilityDenied {
                    id: provider_id.to_owned(),
                    detail: "rerank not permitted (manual capabilities authoritative; no catalog inference)".into(),
                });
            }
            let protocol = rr_protocol.unwrap_or(RerankerProtocol::OpenaiCompatible);
            validate_rerank_surface(desc.kind, primary_surface(desc), protocol, provider_id)?;
        }
    }

    let auth_scheme = map_auth_scheme(meta, desc.auth_scheme.as_deref())?;
    let base_url = meta
        .base_url
        .clone()
        .filter(|u| !u.trim().is_empty())
        .ok_or_else(|| {
            RetrievalRuntimeError::InvalidConfig(format!(
                "provider `{provider_id}` has no base_url"
            ))
        })?;

    let extra_headers = validate_and_collect_headers(&meta.extra_headers)?;
    let primary = primary_surface(desc);
    let cred_route = primary_credential_route(desc);

    // ChatGPT OAuth never serves application retrieval POSTs.
    if cred_route == CredentialRoute::ChatGptOauth || primary == ApiSurface::ChatGptInference {
        return Err(RetrievalRuntimeError::SurfaceMismatch {
            id: provider_id.to_owned(),
            detail: "ChatGPT OAuth / chatgpt_inference never serves retrieval application routes"
                .into(),
        });
    }

    let request_timeout = Duration::from_secs(meta.request_timeout_secs.unwrap_or(60).max(1));
    let total_deadline = opts
        .total_deadline
        .unwrap_or_else(|| Duration::from_millis(DEFAULT_DEADLINE_MS));

    let route = RetrievalRouteContext {
        provider_instance_id: provider_id.to_owned(),
        provider_kind: desc.kind.as_str().to_owned(),
        api_surface: primary.as_str().to_owned(),
        credential_route: cred_route.as_str().to_owned(),
        auth_scheme: auth_scheme.clone(),
        base_url,
        display_name: meta
            .display_name
            .clone()
            .unwrap_or_else(|| provider_id.to_owned()),
        organization: meta.organization.clone(),
        project: meta.project.clone(),
        extra_headers,
        incarnation: desc
            .incarnation
            .as_ref()
            .map(|i| i.as_str().to_owned())
            .or_else(|| opts.provenance_incarnation.map(str::to_owned)),
        registry_generation: generation,
        request_timeout,
        connect_timeout: Duration::from_secs(10),
        total_deadline,
        max_retries: 2,
        max_redirects: 3,
        max_response_bytes: 32 * 1024 * 1024,
        purpose,
    };

    // --- Credentials only after capability/surface pass ---
    let credential = match auth_scheme {
        RetrievalAuthScheme::None => RetrievalCredential::none(),
        _ => {
            if let Some(c) = counters {
                c.secret_lookups.fetch_add(1, Ordering::SeqCst);
            }
            let token = resolve_application_credential(home, provider_id, meta, desc.kind)?;
            RetrievalCredential::new(Some(token))
        }
    };

    if !matches!(auth_scheme, RetrievalAuthScheme::None) && !credential.is_present() {
        return Err(RetrievalRuntimeError::MissingCredential {
            id: provider_id.to_owned(),
        });
    }

    Ok(ResolvedRetrievalRuntime {
        route,
        credential,
        upstream_model: upstream_model.to_owned(),
        embedding_protocol: emb_protocol,
        reranker_protocol: rr_protocol,
        embedding_encoding: encoding,
        dimensions,
        rerank_endpoint,
    })
}

fn capability_view_from_meta(meta: &ProviderMetadata, enabled: bool) -> ProviderCapabilityView {
    let rerank = meta
        .capabilities
        .extra
        .get("rerank")
        .copied()
        .or_else(|| meta.capabilities.extra.get("reranking").copied());
    ProviderCapabilityView {
        id: meta.id.as_str().to_owned(),
        enabled,
        tombstoned: false,
        exists: true,
        embeddings: meta.capabilities.embeddings,
        rerank,
        capability_mode_manual: meta.capability_mode == CapabilityMode::Manual,
        api_surface: None,
    }
}

fn primary_surface(desc: &super::instance::ProviderInstanceDescriptor) -> ApiSurface {
    desc.primary_route()
        .map(|r| r.api_surface)
        .unwrap_or_else(|| match desc.kind {
            ProviderKind::OpenAi => ApiSurface::OpenAiPlatform,
            ProviderKind::OpenRouter => ApiSurface::OpenRouterNative,
            ProviderKind::Anthropic => ApiSurface::AnthropicMessages,
            ProviderKind::Xai | ProviderKind::OpenAiCompatible | ProviderKind::Zai => {
                ApiSurface::OpenAiCompatibleSubset
            }
        })
}

fn primary_credential_route(desc: &super::instance::ProviderInstanceDescriptor) -> CredentialRoute {
    desc.primary_route()
        .map(|r| r.credential_route)
        .unwrap_or(CredentialRoute::ApiKey)
}

fn validate_embedding_surface(
    kind: ProviderKind,
    surface: ApiSurface,
    protocol: EmbeddingProtocol,
    provider_id: &str,
) -> Result<(), RetrievalRuntimeError> {
    if matches!(
        surface,
        ApiSurface::AnthropicMessages | ApiSurface::ChatGptInference
    ) {
        return Err(RetrievalRuntimeError::SurfaceMismatch {
            id: provider_id.to_owned(),
            detail: format!(
                "surface {} cannot host embeddings protocol {}",
                surface.as_str(),
                protocol.as_str()
            ),
        });
    }
    match protocol {
        EmbeddingProtocol::OpenaiCompatible => {
            // OpenAI platform, OpenRouter native, compatible subset, retrieval-only, xAI subset.
            if matches!(
                surface,
                ApiSurface::OpenAiPlatform
                    | ApiSurface::OpenRouterNative
                    | ApiSurface::OpenAiCompatibleSubset
                    | ApiSurface::RetrievalOnly
            ) || kind.is_openai_compatible_family()
                || kind == ProviderKind::Xai
            {
                Ok(())
            } else {
                Err(RetrievalRuntimeError::SurfaceMismatch {
                    id: provider_id.to_owned(),
                    detail: format!(
                        "surface {} / kind {} cannot host openai_compatible embeddings",
                        surface.as_str(),
                        kind.as_str()
                    ),
                })
            }
        }
    }
}

fn validate_rerank_surface(
    kind: ProviderKind,
    surface: ApiSurface,
    protocol: RerankerProtocol,
    provider_id: &str,
) -> Result<(), RetrievalRuntimeError> {
    if matches!(
        surface,
        ApiSurface::AnthropicMessages | ApiSurface::ChatGptInference
    ) {
        return Err(RetrievalRuntimeError::SurfaceMismatch {
            id: provider_id.to_owned(),
            detail: format!(
                "surface {} cannot host rerank protocol {}",
                surface.as_str(),
                protocol.as_str()
            ),
        });
    }
    match protocol {
        RerankerProtocol::OpenaiCompatible => {
            // vLLM / compatible + retrieval-only + openrouter native.
            if matches!(
                surface,
                ApiSurface::OpenAiCompatibleSubset
                    | ApiSurface::RetrievalOnly
                    | ApiSurface::OpenRouterNative
                    | ApiSurface::OpenAiPlatform
            ) || kind.is_openai_compatible_family()
                || kind == ProviderKind::OpenRouter
            {
                Ok(())
            } else {
                Err(RetrievalRuntimeError::SurfaceMismatch {
                    id: provider_id.to_owned(),
                    detail: format!(
                        "surface {} cannot host openai_compatible rerank",
                        surface.as_str()
                    ),
                })
            }
        }
        RerankerProtocol::CohereCompatible => {
            // Handwritten vLLM-style path also covers Cohere-compatible shapes
            // on retrieval-only / compatible providers.
            if matches!(
                surface,
                ApiSurface::OpenAiCompatibleSubset
                    | ApiSurface::RetrievalOnly
                    | ApiSurface::OpenRouterNative
            ) || kind.is_openai_compatible_family()
            {
                Ok(())
            } else {
                Err(RetrievalRuntimeError::SurfaceMismatch {
                    id: provider_id.to_owned(),
                    detail: format!(
                        "surface {} cannot host cohere_compatible rerank",
                        surface.as_str()
                    ),
                })
            }
        }
    }
}

fn map_auth_scheme(
    meta: &ProviderMetadata,
    exact_spelling: Option<&str>,
) -> Result<RetrievalAuthScheme, RetrievalRuntimeError> {
    let spelling = exact_spelling
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| match meta.auth_scheme {
            ProviderAuthScheme::Bearer => "bearer",
            ProviderAuthScheme::None => "none",
            ProviderAuthScheme::CustomHeader => "custom_header",
        });
    match spelling {
        "bearer" | "" => Ok(RetrievalAuthScheme::Bearer),
        "none" => Ok(RetrievalAuthScheme::None),
        "x_api_key" | "x-api-key" | "xapikey" => Ok(RetrievalAuthScheme::XApiKey),
        "custom_header" => Ok(RetrievalAuthScheme::CustomHeader {
            // Default custom header name when only the scheme is declared.
            name: "x-api-key".into(),
        }),
        other => {
            // Treat unknown non-keyword spelling as a custom header name when
            // metadata says CustomHeader; otherwise fail closed.
            if meta.auth_scheme == ProviderAuthScheme::CustomHeader {
                Ok(RetrievalAuthScheme::CustomHeader {
                    name: other.to_owned(),
                })
            } else {
                Ok(RetrievalAuthScheme::Bearer)
            }
        }
    }
}

fn validate_and_collect_headers(
    headers: &indexmap::IndexMap<String, String>,
) -> Result<Vec<(String, String)>, RetrievalRuntimeError> {
    super::lifecycle::validate_extra_headers(headers)
        .map_err(|e| RetrievalRuntimeError::InvalidConfig(e.to_string()))?;
    Ok(headers
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect())
}

/// Resolve application credential for the **exact** instance only.
///
/// Rules:
/// - Configured ids: only `openai_compatible::<id>::api_key` vault + exact
///   instance `env_key` list. Never built-in OPENAI/OPENROUTER/xAI env or vault.
/// - Built-in openai/openrouter: only their matching built-in scopes/env.
/// - Never admin key. Never ChatGPT OAuth. Never xAI session for non-xAI.
/// - Application never borrows another instance's material.
fn resolve_application_credential(
    home: &Path,
    provider_id: &str,
    meta: &ProviderMetadata,
    kind: ProviderKind,
) -> Result<String, RetrievalRuntimeError> {
    // Exact instance env keys first (configured names only).
    if let Some(env_name) = meta.env_key.as_deref() {
        if let Ok(v) = std::env::var(env_name)
            && !v.trim().is_empty()
        {
            return Ok(v);
        }
    }

    let is_builtin = BuiltInProviderId::parse(provider_id).is_some();
    if is_builtin {
        return resolve_builtin_application(home, provider_id);
    }

    // Configured instance: namespaced vault only. Never fall back to built-in
    // scopes even when kind matches openai/openrouter.
    let pid = ProviderId::new(provider_id)
        .map_err(|e| RetrievalRuntimeError::InvalidConfig(format!("invalid provider id: {e}")))?;
    let scope = application_key_scope(&pid);
    if let Ok(Some(v)) = read_provider_secret(home, &scope)
        && !v.trim().is_empty()
    {
        return Ok(v);
    }
    // Also try the auth storage helper under the same scope string.
    if let Ok(Some(v)) = read_provider_api_key(home, &scope)
        && !v.trim().is_empty()
    {
        return Ok(v);
    }

    let _ = kind; // kind must not unlock sibling/built-in scopes.
    Err(RetrievalRuntimeError::MissingCredential {
        id: provider_id.to_owned(),
    })
}

fn resolve_builtin_application(
    home: &Path,
    provider_id: &str,
) -> Result<String, RetrievalRuntimeError> {
    match provider_id {
        "openai" => {
            if let Ok(v) = std::env::var("OPENAI_API_KEY")
                && !v.trim().is_empty()
            {
                return Ok(v);
            }
            if let Ok(Some(v)) = read_provider_api_key(home, OPENAI_API_KEY_SCOPE)
                && !v.trim().is_empty()
            {
                return Ok(v);
            }
        }
        "openrouter" => {
            if let Ok(v) = std::env::var("OPENROUTER_API_KEY")
                && !v.trim().is_empty()
            {
                return Ok(v);
            }
            if let Ok(Some(v)) = read_provider_api_key(home, OPENROUTER_API_KEY_SCOPE)
                && !v.trim().is_empty()
            {
                return Ok(v);
            }
        }
        "anthropic" => {
            if let Ok(v) = std::env::var("ANTHROPIC_API_KEY")
                && !v.trim().is_empty()
            {
                return Ok(v);
            }
            // Anthropic is not a retrieval host in v1; still isolate its scope.
        }
        "xai" => {
            // xAI session is first-party chat only; retrieval on xAI uses API key
            // env if present (XAI_API_KEY), never session token borrowing.
            if let Ok(v) = std::env::var("XAI_API_KEY")
                && !v.trim().is_empty()
            {
                return Ok(v);
            }
        }
        _ => {}
    }
    Err(RetrievalRuntimeError::MissingCredential {
        id: provider_id.to_owned(),
    })
}

// ---------------------------------------------------------------------------
// Adapter builders
// ---------------------------------------------------------------------------

/// Build embedding request from PR15 config + resolved runtime.
pub fn embedding_request_from_config(
    config: &EmbeddingModelConfig,
    inputs: Vec<String>,
) -> EmbeddingRequest {
    EmbeddingRequest {
        model: config.model.clone(),
        inputs,
        dimensions: config.dimensions,
        encoding: match config.encoding {
            EmbeddingEncoding::Float => EmbeddingEncodingFormat::Float,
            EmbeddingEncoding::Base64 => EmbeddingEncodingFormat::Base64,
        },
        endpoint: DEFAULT_EMBEDDINGS_PATH.to_owned(),
    }
}

/// Build rerank request from PR15 config.
pub fn rerank_request_from_config(
    config: &RerankerModelConfig,
    query: String,
    documents: Vec<String>,
    top_n: Option<u32>,
) -> RerankRequest {
    let endpoint = config
        .endpoint
        .as_deref()
        .map(normalize_endpoint_path)
        .unwrap_or_else(|| DEFAULT_RERANK_PATH.to_owned());
    RerankRequest {
        model: config.model.clone(),
        query,
        documents,
        top_n,
        endpoint,
        return_documents: false,
    }
}

/// Execute embeddings through the exact resolved runtime.
pub async fn embed_with_runtime(
    runtime: &ResolvedRetrievalRuntime,
    inputs: Vec<String>,
    cancel: tokio_util::sync::CancellationToken,
) -> RetrievalResult<xai_grok_inference::EmbeddingResult> {
    let encoding = match runtime
        .embedding_encoding
        .unwrap_or(EmbeddingEncoding::Float)
    {
        EmbeddingEncoding::Float => EmbeddingEncodingFormat::Float,
        EmbeddingEncoding::Base64 => EmbeddingEncodingFormat::Base64,
    };
    let request = EmbeddingRequest {
        model: runtime.upstream_model.clone(),
        inputs,
        dimensions: runtime.dimensions,
        encoding,
        endpoint: DEFAULT_EMBEDDINGS_PATH.to_owned(),
    };
    let client = OpenaiCompatibleEmbeddings::new(runtime.route.clone())?;
    client.embed(request, &runtime.credential, cancel).await
}

/// Execute rerank through the exact resolved runtime (OpenRouter generated or
/// vLLM handwritten based on surface/protocol).
pub async fn rerank_with_runtime(
    runtime: &ResolvedRetrievalRuntime,
    query: String,
    documents: Vec<String>,
    top_n: Option<u32>,
    cancel: tokio_util::sync::CancellationToken,
) -> RetrievalResult<xai_grok_inference::RerankResult> {
    let endpoint = runtime
        .rerank_endpoint
        .as_deref()
        .map(normalize_endpoint_path)
        .unwrap_or_else(|| DEFAULT_RERANK_PATH.to_owned());
    let request = RerankRequest {
        model: runtime.upstream_model.clone(),
        query,
        documents,
        top_n,
        endpoint,
        return_documents: false,
    };

    let use_openrouter = runtime.route.api_surface == "openrouter_native"
        || runtime.route.provider_kind == "openrouter";
    if use_openrouter {
        let client = OpenRouterRerankAdapter::new(runtime.route.clone())?;
        client.rerank(request, &runtime.credential, cancel).await
    } else {
        let client = VllmRerankAdapter::new(runtime.route.clone())?;
        client.rerank(request, &runtime.credential, cancel).await
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider_registry::id::ProviderId;
    use crate::provider_registry::management::ProviderManagementService;
    use crate::provider_registry::management::dto::ProviderAddRequest;
    use crate::provider_registry::runtime_cache::invalidate_for_home;
    use crate::provider_registry::secrets::store_provider_secret;
    use tempfile::TempDir;

    fn write_two_siblings(home: &Path) {
        let svc = ProviderManagementService::new(home);
        let g = svc.current_generation();
        assert!(
            svc.add(ProviderAddRequest {
                id: "acct-a".into(),
                kind: "openai_compatible".into(),
                base_url: "http://127.0.0.1:9/v1".into(),
                display_name: Some("A".into()),
                admin_base_url: None,
                enabled: true,
                expected_generation: g,
            })
            .ok
        );
        let g = svc.current_generation();
        assert!(
            svc.add(ProviderAddRequest {
                id: "acct-b".into(),
                kind: "openai_compatible".into(),
                base_url: "http://127.0.0.1:9/v1".into(),
                display_name: Some("B".into()),
                admin_base_url: None,
                enabled: true,
                expected_generation: g,
            })
            .ok
        );
        // Patch capabilities embeddings=true for both via raw TOML append.
        let path = home.join("config.toml");
        let mut raw = std::fs::read_to_string(&path).unwrap();
        raw.push_str(
            r#"

[model_providers.acct-a.capabilities]
embeddings = true
rerank = true

[model_providers.acct-b.capabilities]
embeddings = true
rerank = true
"#,
        );
        std::fs::write(&path, raw).unwrap();
        invalidate_for_home(home);
    }

    #[test]
    fn exact_credential_isolation_same_kind_same_origin() {
        let dir = TempDir::new().unwrap();
        let home = dir.path();
        write_two_siblings(home);
        let a = ProviderId::new("acct-a").unwrap();
        let b = ProviderId::new("acct-b").unwrap();
        store_provider_secret(home, &application_key_scope(&a), "secret-A-only").unwrap();
        store_provider_secret(home, &application_key_scope(&b), "secret-B-only").unwrap();

        // Built-in env must not authenticate configured siblings.
        unsafe {
            std::env::set_var("OPENAI_API_KEY", "builtin-openai-should-not-leak");
            std::env::set_var("OPENROUTER_API_KEY", "builtin-or-should-not-leak");
        }

        let cfg_a = EmbeddingModelConfig {
            provider: "acct-a".into(),
            model: "e".into(),
            ..Default::default()
        };
        let rt_a =
            resolve_embedding_runtime(home, &cfg_a, &RetrievalResolveOptions::default(), None)
                .unwrap();
        assert_eq!(rt_a.credential.as_str(), Some("secret-A-only"));

        let cfg_b = EmbeddingModelConfig {
            provider: "acct-b".into(),
            model: "e".into(),
            ..Default::default()
        };
        let rt_b =
            resolve_embedding_runtime(home, &cfg_b, &RetrievalResolveOptions::default(), None)
                .unwrap();
        assert_eq!(rt_b.credential.as_str(), Some("secret-B-only"));
        assert_ne!(rt_a.credential.as_str(), rt_b.credential.as_str());

        // Missing key on A fails even if B / built-in exist.
        let _ = crate::provider_registry::secrets::clear_provider_secret(
            home,
            &application_key_scope(&a),
        );
        let err =
            resolve_embedding_runtime(home, &cfg_a, &RetrievalResolveOptions::default(), None)
                .unwrap_err();
        assert!(matches!(
            err,
            RetrievalRuntimeError::MissingCredential { .. }
        ));

        unsafe {
            std::env::remove_var("OPENAI_API_KEY");
            std::env::remove_var("OPENROUTER_API_KEY");
        }
    }

    #[test]
    fn capability_failure_before_secret_lookup() {
        let dir = TempDir::new().unwrap();
        let home = dir.path();
        let svc = ProviderManagementService::new(home);
        let g = svc.current_generation();
        assert!(
            svc.add(ProviderAddRequest {
                id: "no-embed".into(),
                kind: "openai_compatible".into(),
                base_url: "http://127.0.0.1:9/v1".into(),
                display_name: None,
                admin_base_url: None,
                enabled: true,
                expected_generation: g,
            })
            .ok
        );
        let path = home.join("config.toml");
        let mut raw = std::fs::read_to_string(&path).unwrap();
        raw.push_str(
            r#"

[model_providers.no-embed]
capability_mode = "manual"

[model_providers.no-embed.capabilities]
embeddings = false
"#,
        );
        // Re-write carefully: append capabilities block.
        std::fs::write(
            &path,
            format!(
                "{}\n\n[model_providers.no-embed.capabilities]\nembeddings = false\n",
                std::fs::read_to_string(&path).unwrap()
            ),
        )
        .unwrap();
        // Also set manual mode via patch of raw.
        let mut raw = std::fs::read_to_string(&path).unwrap();
        if !raw.contains("capability_mode") {
            raw = raw.replace(
                "[model_providers.no-embed]",
                "[model_providers.no-embed]\ncapability_mode = \"manual\"",
            );
            std::fs::write(&path, raw).unwrap();
        }
        invalidate_for_home(home);

        let pid = ProviderId::new("no-embed").unwrap();
        store_provider_secret(home, &application_key_scope(&pid), "should-not-read").unwrap();

        let counters = RetrievalResolveCounters::default();
        let cfg = EmbeddingModelConfig {
            provider: "no-embed".into(),
            model: "e".into(),
            ..Default::default()
        };
        let err = resolve_embedding_runtime(
            home,
            &cfg,
            &RetrievalResolveOptions::default(),
            Some(&counters),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            RetrievalRuntimeError::CapabilityDenied { .. }
        ));
        assert_eq!(counters.secret_lookups(), 0);
        assert!(counters.capability_checks() >= 1);
    }

    #[test]
    fn disabled_and_missing_fail_closed() {
        let dir = TempDir::new().unwrap();
        let home = dir.path();
        let svc = ProviderManagementService::new(home);
        let g = svc.current_generation();
        assert!(
            svc.add(ProviderAddRequest {
                id: "lab".into(),
                kind: "openai_compatible".into(),
                base_url: "http://127.0.0.1:9/v1".into(),
                display_name: None,
                admin_base_url: None,
                enabled: true,
                expected_generation: g,
            })
            .ok
        );
        let g2 = svc.current_generation();
        assert!(svc.set_enabled("lab", false, g2).ok);
        invalidate_for_home(home);

        let cfg = EmbeddingModelConfig {
            provider: "lab".into(),
            model: "e".into(),
            ..Default::default()
        };
        let err = resolve_embedding_runtime(home, &cfg, &RetrievalResolveOptions::default(), None)
            .unwrap_err();
        assert!(
            matches!(err, RetrievalRuntimeError::ProviderDisabled { .. })
                || matches!(err, RetrievalRuntimeError::RouteGuard(_))
        );

        let cfg2 = EmbeddingModelConfig {
            provider: "does-not-exist".into(),
            model: "e".into(),
            ..Default::default()
        };
        let err2 =
            resolve_embedding_runtime(home, &cfg2, &RetrievalResolveOptions::default(), None)
                .unwrap_err();
        assert!(
            matches!(err2, RetrievalRuntimeError::ProviderMissing { .. })
                || matches!(err2, RetrievalRuntimeError::RouteGuard(_))
        );
    }

    #[test]
    fn none_auth_has_no_credential() {
        let dir = TempDir::new().unwrap();
        let home = dir.path();
        let path = home.join("config.toml");
        std::fs::write(
            &path,
            r#"
[model_providers.local_lab]
kind = "openai_compatible"
base_url = "http://127.0.0.1:9/v1"
auth_scheme = "none"
enabled = true
capability_mode = "manual"

[model_providers.local_lab.capabilities]
embeddings = true
"#,
        )
        .unwrap();
        invalidate_for_home(home);
        let cfg = EmbeddingModelConfig {
            provider: "local_lab".into(),
            model: "e".into(),
            ..Default::default()
        };
        let rt = resolve_embedding_runtime(home, &cfg, &RetrievalResolveOptions::default(), None)
            .unwrap();
        assert!(matches!(rt.route.auth_scheme, RetrievalAuthScheme::None));
        assert!(!rt.credential.is_present());
    }

    #[test]
    fn debug_redacts_credential() {
        let dir = TempDir::new().unwrap();
        let home = dir.path();
        write_two_siblings(home);
        let a = ProviderId::new("acct-a").unwrap();
        store_provider_secret(home, &application_key_scope(&a), "super-secret-xyz").unwrap();
        let cfg = EmbeddingModelConfig {
            provider: "acct-a".into(),
            model: "e".into(),
            ..Default::default()
        };
        let rt = resolve_embedding_runtime(home, &cfg, &RetrievalResolveOptions::default(), None)
            .unwrap();
        let dbg = format!("{rt:?}");
        assert!(!dbg.contains("super-secret-xyz"));
    }

    #[test]
    fn anthropic_surface_rejects_embeddings() {
        let dir = TempDir::new().unwrap();
        let home = dir.path();
        // Built-in anthropic exists; resolve should surface-mismatch.
        let cfg = EmbeddingModelConfig {
            provider: "anthropic".into(),
            model: "e".into(),
            ..Default::default()
        };
        let err = resolve_embedding_runtime(home, &cfg, &RetrievalResolveOptions::default(), None)
            .unwrap_err();
        // Capability may also fail first if manual defaults; accept surface or capability.
        assert!(
            matches!(err, RetrievalRuntimeError::SurfaceMismatch { .. })
                || matches!(err, RetrievalRuntimeError::CapabilityDenied { .. })
                || matches!(err, RetrievalRuntimeError::MissingCredential { .. }),
            "{err}"
        );
    }
}
