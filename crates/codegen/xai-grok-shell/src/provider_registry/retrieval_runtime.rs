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
use super::instance::{ApiSurface, CredentialRoute, ProviderInstanceDescriptor, ProviderKind};
use super::lifecycle::{CapabilityMode, ProviderAuthScheme, ProviderMetadata};
use super::lifecycle_state::{ProviderLifecycleState, load_lifecycle_state};
use super::route_guard::{RouteGuardError, RouteGuardRequest, assert_route_usable};
use super::runtime_cache::load_runtime;
use super::secrets::{application_key_scope, read_provider_secret};
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
///
/// Generation pin semantics (retrieval-strict, independent of soft chat guard):
/// - `session_registry_generation: None` — fresh resolve; live generation is
///   returned/pinned on the result and is **not** treated as a stale bound.
/// - `session_registry_generation: Some(g)` — bound route; any mismatch with
///   live generation fails closed on the **next** request (regardless of
///   `is_retry` or incarnation pin).
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

    let (service, lifecycle, generation) =
        load_runtime(home).map_err(RetrievalRuntimeError::Lifecycle)?;

    // Retrieval-strict generation: pinned Some(stale) always fails on mismatch.
    // None means fresh resolve — pin live generation on the result only.
    if let Some(expected) = opts.session_registry_generation {
        if generation != 0 && expected != 0 && expected != generation {
            return Err(RetrievalRuntimeError::RouteGuard(
                RouteGuardError::GenerationReplaced {
                    id: provider_id.to_owned(),
                    expected,
                    live: generation,
                }
                .to_string(),
            ));
        }
    }

    // Incarnation/tombstone/disabled/missing via shared guard. Do **not** inject
    // live generation when caller omitted a pin (fresh resolve).
    assert_route_usable(
        home,
        &service,
        &RouteGuardRequest {
            provider_instance_id: provider_id,
            provenance_incarnation: opts.provenance_incarnation,
            session_registry_generation: opts.session_registry_generation,
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

    // Auth scheme is needed to interpret CredentialRoute::None (vault-backed
    // vs intentional no-auth) before credential resolution.
    let auth_scheme = map_auth_scheme(meta, desc.auth_scheme.as_deref())?;
    let (surface, cred_route) = select_retrieval_route(desc, provider_id, &auth_scheme)?;

    // --- Capability + surface BEFORE credentials ---
    if let Some(c) = counters {
        c.capability_checks.fetch_add(1, Ordering::SeqCst);
    }
    let cap_view = capability_view_from_meta(meta, desc, &lifecycle);
    if cap_view.tombstoned {
        return Err(RetrievalRuntimeError::RouteGuard(format!(
            "provider `{provider_id}` is tombstoned"
        )));
    }
    match purpose {
        RetrievalPurpose::Embeddings => {
            if !cap_view.can_embed() {
                return Err(RetrievalRuntimeError::CapabilityDenied {
                    id: provider_id.to_owned(),
                    detail: "embeddings not permitted (manual capabilities authoritative; no catalog inference)".into(),
                });
            }
            let protocol = emb_protocol.unwrap_or(EmbeddingProtocol::OpenaiCompatible);
            validate_embedding_surface(desc.kind, surface, protocol, provider_id)?;
        }
        RetrievalPurpose::Rerank => {
            if !cap_view.can_rerank() {
                return Err(RetrievalRuntimeError::CapabilityDenied {
                    id: provider_id.to_owned(),
                    detail: "rerank not permitted (manual capabilities authoritative; no catalog inference)".into(),
                });
            }
            let protocol = rr_protocol.unwrap_or(RerankerProtocol::OpenaiCompatible);
            validate_rerank_surface(desc.kind, surface, protocol, provider_id)?;
        }
    }
    let base_url = meta
        .base_url
        .clone()
        .filter(|u| !u.trim().is_empty())
        .ok_or_else(|| {
            RetrievalRuntimeError::InvalidConfig(format!(
                "provider `{provider_id}` has no base_url"
            ))
        })?;

    let organization = validate_org_project_field("organization", meta.organization.as_deref())?;
    let project = validate_org_project_field("project", meta.project.as_deref())?;
    let extra_headers = validate_and_collect_headers(&meta.extra_headers)?;

    let request_timeout = Duration::from_secs(meta.request_timeout_secs.unwrap_or(60).max(1));
    let total_deadline = opts
        .total_deadline
        .unwrap_or_else(|| Duration::from_millis(DEFAULT_DEADLINE_MS));

    let route = RetrievalRouteContext {
        provider_instance_id: provider_id.to_owned(),
        provider_kind: desc.kind.as_str().to_owned(),
        api_surface: surface.as_str().to_owned(),
        credential_route: cred_route.as_str().to_owned(),
        auth_scheme: auth_scheme.clone(),
        base_url,
        display_name: meta
            .display_name
            .clone()
            .unwrap_or_else(|| provider_id.to_owned()),
        organization,
        project,
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
    let credential = match (&auth_scheme, cred_route) {
        (RetrievalAuthScheme::None, _) | (_, CredentialRoute::None) => RetrievalCredential::none(),
        _ => {
            if let Some(c) = counters {
                c.secret_lookups.fetch_add(1, Ordering::SeqCst);
            }
            let token =
                resolve_application_credential(home, provider_id, meta, &desc.env_keys, desc.kind)?;
            RetrievalCredential::new(Some(token))
        }
    };

    if !matches!(auth_scheme, RetrievalAuthScheme::None)
        && !matches!(cred_route, CredentialRoute::None)
        && !credential.is_present()
    {
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

fn capability_view_from_meta(
    meta: &ProviderMetadata,
    desc: &ProviderInstanceDescriptor,
    lifecycle: &ProviderLifecycleState,
) -> ProviderCapabilityView {
    let rerank = meta
        .capabilities
        .extra
        .get("rerank")
        .copied()
        .or_else(|| meta.capabilities.extra.get("reranking").copied());
    let tombstoned = desc
        .incarnation
        .as_ref()
        .map(|inc| lifecycle.is_tombstoned(meta.id.as_str(), inc))
        .unwrap_or(false)
        || lifecycle.has_blocking_tombstone_for_id(meta.id.as_str());
    ProviderCapabilityView {
        id: meta.id.as_str().to_owned(),
        enabled: desc.enabled,
        tombstoned,
        exists: true,
        embeddings: meta.capabilities.embeddings,
        rerank,
        capability_mode_manual: meta.capability_mode == CapabilityMode::Manual,
        api_surface: None,
    }
}

/// Select an application-capable retrieval surface + credential route.
///
/// Prefers `ApiKey` / `OpenAiPlatform` over session/OAuth/helper.
/// `CredentialRoute::None` is accepted only for intentional no-auth
/// (`RetrievalAuthScheme::None`); vault-backed providers without an env_key
/// are often classified as `None` by the descriptor — those promote to
/// `ApiKey` when the auth scheme expects a secret.
/// Explicitly rejects AuthHelper-only and XaiSession-only (no silent substitute).
fn select_retrieval_route(
    desc: &ProviderInstanceDescriptor,
    provider_id: &str,
    auth_scheme: &RetrievalAuthScheme,
) -> Result<(ApiSurface, CredentialRoute), RetrievalRuntimeError> {
    let mut none_candidate: Option<ApiSurface> = None;
    for r in &desc.routes {
        if r.api_surface == ApiSurface::ChatGptInference {
            continue;
        }
        match r.credential_route {
            CredentialRoute::ApiKey | CredentialRoute::OpenAiPlatform => {
                return Ok((r.api_surface, r.credential_route));
            }
            CredentialRoute::None => {
                if none_candidate.is_none() {
                    none_candidate = Some(r.api_surface);
                }
            }
            CredentialRoute::ChatGptOauth
            | CredentialRoute::AuthHelper
            | CredentialRoute::XaiSession => continue,
        }
    }
    if let Some(surface) = none_candidate {
        return match auth_scheme {
            RetrievalAuthScheme::None => Ok((surface, CredentialRoute::None)),
            _ => Ok((surface, CredentialRoute::ApiKey)),
        };
    }
    let primary = desc.primary_route();
    let cred = primary.map(|r| r.credential_route);
    match cred {
        Some(CredentialRoute::AuthHelper) => Err(RetrievalRuntimeError::SurfaceMismatch {
            id: provider_id.to_owned(),
            detail: "auth_helper credential route is not supported for retrieval (no silent API-key substitute; helpers are not executed)".into(),
        }),
        Some(CredentialRoute::XaiSession) => Err(RetrievalRuntimeError::SurfaceMismatch {
            id: provider_id.to_owned(),
            detail: "xai_session credential route is not supported for retrieval (no silent API-key or session borrow)".into(),
        }),
        Some(CredentialRoute::ChatGptOauth) | None
            if primary.map(|r| r.api_surface) == Some(ApiSurface::ChatGptInference) =>
        {
            Err(RetrievalRuntimeError::SurfaceMismatch {
                id: provider_id.to_owned(),
                detail: "ChatGPT OAuth / chatgpt_inference never serves retrieval application routes"
                    .into(),
            })
        }
        Some(CredentialRoute::ChatGptOauth) => Err(RetrievalRuntimeError::SurfaceMismatch {
            id: provider_id.to_owned(),
            detail: "ChatGPT OAuth never serves retrieval application routes".into(),
        }),
        _ => Err(RetrievalRuntimeError::SurfaceMismatch {
            id: provider_id.to_owned(),
            detail: "no application-capable credential route for retrieval".into(),
        }),
    }
}

fn validate_org_project_field(
    label: &str,
    value: Option<&str>,
) -> Result<Option<String>, RetrievalRuntimeError> {
    let Some(v) = value.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    if v.chars().any(|c| c == '\r' || c == '\n' || c.is_control()) {
        return Err(RetrievalRuntimeError::InvalidConfig(format!(
            "{label} contains invalid control characters"
        )));
    }
    Ok(Some(v.to_owned()))
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
        "bearer" => Ok(RetrievalAuthScheme::Bearer),
        "none" => Ok(RetrievalAuthScheme::None),
        "x_api_key" | "x-api-key" | "xapikey" => Ok(RetrievalAuthScheme::XApiKey),
        // Explicit default: `custom_header` alone wires the credential as
        // `x-api-key` (same header name as the XApiKey scheme). Callers that
        // need a different name must spell it as the auth_scheme value under
        // CustomHeader metadata (validated restricted-name rules apply).
        "custom_header" => Ok(RetrievalAuthScheme::CustomHeader {
            name: "x-api-key".into(),
        }),
        other => {
            // Only accept unknown spelling as a custom header **name** when
            // metadata already classifies the scheme as CustomHeader.
            // Typo/unknown strings otherwise fail closed (never become Bearer).
            if meta.auth_scheme == ProviderAuthScheme::CustomHeader {
                let name = other.to_owned();
                let lower = name.to_ascii_lowercase();
                if lower == "authorization"
                    || lower == "cookie"
                    || lower == "proxy-authorization"
                    || lower.starts_with("x-grok-")
                    || lower == "openai-organization"
                    || lower == "openai-project"
                {
                    return Err(RetrievalRuntimeError::InvalidConfig(format!(
                        "restricted custom auth header name `{name}`"
                    )));
                }
                Ok(RetrievalAuthScheme::CustomHeader { name })
            } else {
                Err(RetrievalRuntimeError::InvalidConfig(format!(
                    "unknown auth_scheme `{other}` (refusing silent Bearer fallback)"
                )))
            }
        }
    }
}

fn validate_and_collect_headers(
    headers: &indexmap::IndexMap<String, String>,
) -> Result<Vec<(String, String)>, RetrievalRuntimeError> {
    super::lifecycle::validate_extra_headers(headers)
        .map_err(|e| RetrievalRuntimeError::InvalidConfig(e.to_string()))?;
    let mut out = Vec::with_capacity(headers.len());
    for (k, v) in headers {
        let lower = k.to_ascii_lowercase();
        // Typed org/project and content/auth owners are not free-form extras.
        if lower == "openai-organization"
            || lower == "openai-project"
            || lower == "content-type"
            || lower == "accept"
            || lower == "authorization"
            || lower == "cookie"
            || lower == "proxy-authorization"
        {
            return Err(RetrievalRuntimeError::InvalidConfig(format!(
                "extra_headers must not set restricted header `{k}` (use typed fields / auth scheme)"
            )));
        }
        if v.chars().any(|c| c == '\r' || c == '\n' || c.is_control()) {
            return Err(RetrievalRuntimeError::InvalidConfig(format!(
                "header `{k}` contains invalid control characters"
            )));
        }
        out.push((k.clone(), v.clone()));
    }
    Ok(out)
}

/// Resolve application credential for the **exact** instance only.
///
/// Rules:
/// - Configured ids: ordered `env_keys` from the descriptor (full list), then
///   one vault read of `openai_compatible::<id>::api_key`. Never built-in
///   OPENAI/OPENROUTER/xAI env or vault for configured siblings.
/// - Built-in openai/openrouter: only their matching built-in scopes/env.
/// - Built-in xai: only when selected route is ApiKey (`XAI_API_KEY`); never
///   session material.
/// - Never admin key. Never ChatGPT OAuth.
fn resolve_application_credential(
    home: &Path,
    provider_id: &str,
    meta: &ProviderMetadata,
    env_keys: &[String],
    kind: ProviderKind,
) -> Result<String, RetrievalRuntimeError> {
    // Exact instance env keys in descriptor order (full list, not primary only).
    for env_name in env_keys {
        if env_name.trim().is_empty() {
            continue;
        }
        if let Ok(v) = std::env::var(env_name)
            && !v.trim().is_empty()
        {
            return Ok(v);
        }
    }
    // Fallback: metadata primary if descriptor list empty (legacy tables).
    if env_keys.is_empty() {
        if let Some(env_name) = meta.env_key.as_deref() {
            if let Ok(v) = std::env::var(env_name)
                && !v.trim().is_empty()
            {
                return Ok(v);
            }
        }
    }

    let is_builtin = BuiltInProviderId::parse(provider_id).is_some();
    if is_builtin {
        return resolve_builtin_application(home, provider_id);
    }

    // Configured instance: single namespaced vault read only.
    let pid = ProviderId::new(provider_id)
        .map_err(|e| RetrievalRuntimeError::InvalidConfig(format!("invalid provider id: {e}")))?;
    let scope = application_key_scope(&pid);
    if let Ok(Some(v)) = read_provider_secret(home, &scope)
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
            // Only when the selected retrieval route is ApiKey (select_retrieval_route
            // prefers ApiKey over XaiSession). Never reads session tokens.
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
        // Provide a vault key so MissingCredential cannot mask surface failure.
        unsafe {
            std::env::set_var("ANTHROPIC_API_KEY", "should-not-be-used-for-embed");
        }
        let cfg = EmbeddingModelConfig {
            provider: "anthropic".into(),
            model: "e".into(),
            ..Default::default()
        };
        let counters = RetrievalResolveCounters::default();
        let err = resolve_embedding_runtime(
            home,
            &cfg,
            &RetrievalResolveOptions::default(),
            Some(&counters),
        )
        .unwrap_err();
        assert!(
            matches!(err, RetrievalRuntimeError::SurfaceMismatch { .. })
                || matches!(err, RetrievalRuntimeError::CapabilityDenied { .. }),
            "expected surface/capability, got {err}"
        );
        assert_eq!(
            counters.secret_lookups(),
            0,
            "must not resolve secrets after surface/capability deny"
        );
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
        }
    }

    #[test]
    fn pinned_generation_mismatch_fails_closed() {
        let dir = TempDir::new().unwrap();
        let home = dir.path();
        write_two_siblings(home);
        let a = ProviderId::new("acct-a").unwrap();
        store_provider_secret(home, &application_key_scope(&a), "secret-A").unwrap();
        let cfg = EmbeddingModelConfig {
            provider: "acct-a".into(),
            model: "e".into(),
            ..Default::default()
        };
        let live = resolve_embedding_runtime(home, &cfg, &RetrievalResolveOptions::default(), None)
            .unwrap();
        let live_gen = live.route.registry_generation;
        // Stale pin fails even when is_retry=false and no incarnation.
        let err = resolve_embedding_runtime(
            home,
            &cfg,
            &RetrievalResolveOptions {
                session_registry_generation: Some(live_gen.saturating_add(99).max(1)),
                is_retry: false,
                ..Default::default()
            },
            None,
        )
        .unwrap_err();
        assert!(
            matches!(err, RetrievalRuntimeError::RouteGuard(ref m) if m.contains("generation")),
            "{err}"
        );
        // Fresh resolve (None) still succeeds and pins live gen.
        let again =
            resolve_embedding_runtime(home, &cfg, &RetrievalResolveOptions::default(), None)
                .unwrap();
        assert_eq!(again.route.registry_generation, live_gen);
    }

    #[test]
    fn incarnation_mismatch_and_tombstone_fail_closed() {
        use crate::provider_registry::lifecycle_state::{
            ProviderLifecycleState, store_lifecycle_state,
        };
        let dir = TempDir::new().unwrap();
        let home = dir.path();
        write_two_siblings(home);
        let a = ProviderId::new("acct-a").unwrap();
        store_provider_secret(home, &application_key_scope(&a), "secret-A").unwrap();
        // Load live incarnation then pin a fake one.
        let cfg = EmbeddingModelConfig {
            provider: "acct-a".into(),
            model: "e".into(),
            ..Default::default()
        };
        let live = resolve_embedding_runtime(home, &cfg, &RetrievalResolveOptions::default(), None)
            .unwrap();
        let fake = "00000000-0000-0000-0000-000000000099";
        let err = resolve_embedding_runtime(
            home,
            &cfg,
            &RetrievalResolveOptions {
                provenance_incarnation: Some(fake),
                ..Default::default()
            },
            None,
        )
        .unwrap_err();
        assert!(
            matches!(err, RetrievalRuntimeError::RouteGuard(_)),
            "incarnation mismatch: {err}"
        );
        // Tombstone live incarnation; subsequent provenance pin must fail.
        if let Some(inc) = live.route.incarnation.as_deref() {
            let mut state = ProviderLifecycleState::empty();
            let pid = ProviderId::new("acct-a").unwrap();
            if let Ok(parsed) = crate::provider_registry::instance::ProviderIncarnation::new(inc) {
                let _ = state.tombstone_remove(&pid, Some(&parsed));
                store_lifecycle_state(home, &state).unwrap();
                invalidate_for_home(home);
                let err2 = resolve_embedding_runtime(
                    home,
                    &cfg,
                    &RetrievalResolveOptions {
                        provenance_incarnation: Some(inc),
                        ..Default::default()
                    },
                    None,
                )
                .unwrap_err();
                assert!(
                    matches!(err2, RetrievalRuntimeError::RouteGuard(_)),
                    "tombstone: {err2}"
                );
            }
        }
    }

    #[test]
    fn unknown_auth_scheme_fails_closed() {
        let dir = TempDir::new().unwrap();
        let home = dir.path();
        std::fs::write(
            home.join("config.toml"),
            r#"
[model_providers.typo_auth]
kind = "openai_compatible"
base_url = "http://127.0.0.1:9/v1"
auth_scheme = "beare"
enabled = true
capability_mode = "manual"

[model_providers.typo_auth.capabilities]
embeddings = true
"#,
        )
        .unwrap();
        invalidate_for_home(home);
        let cfg = EmbeddingModelConfig {
            provider: "typo_auth".into(),
            model: "e".into(),
            ..Default::default()
        };
        let err = resolve_embedding_runtime(home, &cfg, &RetrievalResolveOptions::default(), None)
            .unwrap_err();
        assert!(
            matches!(err, RetrievalRuntimeError::InvalidConfig(ref m) if m.contains("unknown auth_scheme")),
            "{err}"
        );
    }

    #[test]
    fn multi_env_key_list_honored_and_isolated() {
        let dir = TempDir::new().unwrap();
        let home = dir.path();
        std::fs::write(
            home.join("config.toml"),
            r#"
[model_providers.envlist]
kind = "openai_compatible"
base_url = "http://127.0.0.1:9/v1"
env_key = ["GROK_TEST_RETR_PRIMARY_MISSING", "GROK_TEST_RETR_FALLBACK"]
enabled = true
capability_mode = "manual"

[model_providers.envlist.capabilities]
embeddings = true
"#,
        )
        .unwrap();
        invalidate_for_home(home);
        unsafe {
            std::env::remove_var("GROK_TEST_RETR_PRIMARY_MISSING");
            std::env::set_var("GROK_TEST_RETR_FALLBACK", "from-fallback-list");
            std::env::set_var("OPENAI_API_KEY", "builtin-must-not-win");
        }
        let cfg = EmbeddingModelConfig {
            provider: "envlist".into(),
            model: "e".into(),
            ..Default::default()
        };
        let rt = resolve_embedding_runtime(home, &cfg, &RetrievalResolveOptions::default(), None)
            .unwrap();
        assert_eq!(rt.credential.as_str(), Some("from-fallback-list"));
        unsafe {
            std::env::remove_var("GROK_TEST_RETR_FALLBACK");
            std::env::remove_var("OPENAI_API_KEY");
        }
    }

    #[test]
    fn admin_vault_never_authenticates_application() {
        use crate::provider_registry::secrets::{admin_key_scope, store_provider_secret};
        let dir = TempDir::new().unwrap();
        let home = dir.path();
        write_two_siblings(home);
        let a = ProviderId::new("acct-a").unwrap();
        store_provider_secret(home, &admin_key_scope(&a), "admin-only-secret").unwrap();
        // No application key.
        let cfg = EmbeddingModelConfig {
            provider: "acct-a".into(),
            model: "e".into(),
            ..Default::default()
        };
        let err = resolve_embedding_runtime(home, &cfg, &RetrievalResolveOptions::default(), None)
            .unwrap_err();
        assert!(matches!(
            err,
            RetrievalRuntimeError::MissingCredential { .. }
        ));
    }

    #[test]
    fn auth_helper_only_rejected_without_executing_helper() {
        let dir = TempDir::new().unwrap();
        let home = dir.path();
        std::fs::write(
            home.join("config.toml"),
            r#"
[model_providers.helper_only]
kind = "openai_compatible"
base_url = "http://127.0.0.1:9/v1"
credential_route = "auth_helper"
auth_provider = "model_provider:helper_only"
enabled = true
capability_mode = "manual"

[model_providers.helper_only.capabilities]
embeddings = true
"#,
        )
        .unwrap();
        invalidate_for_home(home);
        let cfg = EmbeddingModelConfig {
            provider: "helper_only".into(),
            model: "e".into(),
            ..Default::default()
        };
        let counters = RetrievalResolveCounters::default();
        let err = resolve_embedding_runtime(
            home,
            &cfg,
            &RetrievalResolveOptions::default(),
            Some(&counters),
        )
        .unwrap_err();
        assert!(
            matches!(err, RetrievalRuntimeError::SurfaceMismatch { .. }),
            "{err}"
        );
        assert_eq!(counters.secret_lookups(), 0);
    }

    #[test]
    fn org_project_extra_header_collision_rejected() {
        let dir = TempDir::new().unwrap();
        let home = dir.path();
        std::fs::write(
            home.join("config.toml"),
            r#"
[model_providers.org_collide]
kind = "openai_compatible"
base_url = "http://127.0.0.1:9/v1"
organization = "real-org"
auth_scheme = "none"
enabled = true
capability_mode = "manual"

[model_providers.org_collide.extra_headers]
OpenAI-Organization = "evil-org"

[model_providers.org_collide.capabilities]
embeddings = true
"#,
        )
        .unwrap();
        invalidate_for_home(home);
        let cfg = EmbeddingModelConfig {
            provider: "org_collide".into(),
            model: "e".into(),
            ..Default::default()
        };
        let err = resolve_embedding_runtime(home, &cfg, &RetrievalResolveOptions::default(), None)
            .unwrap_err();
        assert!(
            matches!(err, RetrievalRuntimeError::InvalidConfig(_)),
            "{err}"
        );
    }

    #[test]
    fn xai_session_only_path_not_silently_api_keyed_without_api_route() {
        // Built-in xai has ApiKey route in the set; selection prefers it.
        // When XAI_API_KEY missing, MissingCredential — never session.
        let dir = TempDir::new().unwrap();
        let home = dir.path();
        unsafe {
            std::env::remove_var("XAI_API_KEY");
        }
        let cfg = EmbeddingModelConfig {
            provider: "xai".into(),
            model: "e".into(),
            ..Default::default()
        };
        let err = resolve_embedding_runtime(home, &cfg, &RetrievalResolveOptions::default(), None)
            .unwrap_err();
        // Capability may allow; credential or surface depending on routes.
        assert!(
            matches!(err, RetrievalRuntimeError::MissingCredential { .. })
                || matches!(err, RetrievalRuntimeError::CapabilityDenied { .. })
                || matches!(err, RetrievalRuntimeError::SurfaceMismatch { .. }),
            "{err}"
        );
    }
}
