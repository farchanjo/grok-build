//! Secret-free catalog discovery types shared by OpenAI and OpenRouter adapters.

use crate::provider_registry::{
    ApiSurface, CredentialBindingId, CredentialRoute, ProviderId, ProviderIncarnation, ProviderKind,
};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// How a discovered catalog page set was produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogFetchSource {
    Live,
    Cache,
}

/// Why a catalog fetch stopped before "end of list" natural completion.
///
/// Bound exceedance and errors never publish a truncated account; callers keep
/// last-known-good. This enum is diagnostic-only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogTruncationReason {
    /// Natural completion (no more pages).
    Complete,
    /// Hit configured safety bound (pages/models/bytes/time).
    BoundExceeded { detail: String },
    /// Provider response malformed or pagination token invalid.
    MalformedPage { detail: String },
    /// Auth rejected (401/403).
    AuthFailure { status: u16 },
    /// Transport / non-auth HTTP failure.
    TransportFailure { detail: String },
    /// Caller cancelled the fetch.
    Cancelled,
    /// Pagination loop (cursor/URL/offset) detected.
    PaginationLoop,
    /// Next-page URL left the authorized origin.
    OriginEscape,
}

impl CatalogTruncationReason {
    pub fn is_complete(&self) -> bool {
        matches!(self, Self::Complete)
    }
}

/// One discovered upstream model after provider-specific projection.
///
/// `upstream_model_id` is the exact wire value. `canonical_selection_id` is the
/// selection key (`openai:<slug>` / `openrouter:<slug>` for built-ins;
/// `<instance>:<verbatim-upstream>` for additional configured accounts).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiscoveredModel {
    /// Canonical selection id used by pickers, config, and persistence.
    pub canonical_selection_id: String,
    /// Exact provider-wire model id (never silently normalized).
    pub upstream_model_id: String,
    /// Human-readable label when the provider advertises one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
    /// Per-request max completion tokens when the provider advertises a
    /// request budget, or when the user set a model override. OpenRouter
    /// discovery leaves this unset: its `top_provider.max_completion_tokens`
    /// is stored on [`Self::max_output_ceiling`] instead so context
    /// validation does not reserve the full route cap.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u32>,
    /// OpenRouter-routed completion-token ceiling (min of positive
    /// `top_provider.max_completion_tokens` and
    /// `per_request_limits.completion_tokens`). Never copied onto
    /// `ModelInfo.max_completion_tokens`. The sampler clamps the provider
    /// request default to this ceiling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_ceiling: Option<u32>,
    /// Projected agent/tool capabilities. Never guessed for embeddings/rerank.
    pub capabilities: ProjectedCapabilities,
    /// Provider instance that produced this row.
    pub provider_instance_id: String,
    pub provider_kind: ProviderKind,
    pub api_surface: ApiSurface,
    pub credential_route: CredentialRoute,
    /// Secret-free origin provenance (scheme+host[+port] only).
    pub endpoint_origin: String,
}

/// Capability projection from catalog metadata. Explicit manual overrides remain
/// authoritative over auto projection. Generic `/models` discovery never asserts
/// embeddings or rerank support.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProjectedCapabilities {
    /// Whether tools/function calling is advertised (OpenRouter) or unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_tools: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_reasoning_effort: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasoning_efforts: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_reasoning_effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_image_input: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_audio_input: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_video_input: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_file_input: Option<bool>,
    /// True when `architecture.output_modalities` includes `text`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_has_text: Option<bool>,
    /// Native structured outputs from `structured_outputs` / `response_format`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_native_schema: Option<bool>,
    /// OpenRouter zero-data-retention: `GET /models?zdr=true` membership or
    /// `GET /endpoints/zdr` slug intersection. Never inferred from xAI team ZDR.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_zdr: Option<bool>,
    /// Routed completion-token ceiling retained on capability snapshots.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_ceiling: Option<u32>,
    /// Explicit manual capability overrides from config (authoritative).
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub manual_overrides: IndexMap<String, bool>,
    /// Always false/absent from generic catalog auto-projection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_embeddings: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_rerank: Option<bool>,
}

/// Complete (or LKG) result for one provider instance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstanceCatalogResult {
    pub provider_instance_id: String,
    pub provider_kind: ProviderKind,
    pub api_surface: ApiSurface,
    pub credential_route: CredentialRoute,
    pub endpoint_origin: String,
    /// Route-affecting org/project fingerprint (empty when none). Matches PR7.
    #[serde(default)]
    pub org_project_fingerprint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incarnation: Option<ProviderIncarnation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_binding_id: Option<CredentialBindingId>,
    /// Live registry generation observed when this result was accepted.
    pub registry_generation: u64,
    /// Catalog generation assigned on successful live store (0 for pure LKG).
    pub catalog_generation: u64,
    /// ModelsManager publication generation this result was requested under.
    pub publication_generation: u64,
    pub source: CatalogFetchSource,
    pub truncation: CatalogTruncationReason,
    /// Deterministic order: first-seen upstream id wins; later duplicates dropped.
    pub models: Vec<DiscoveredModel>,
    /// Diagnostics only — never secrets, never full URLs with query secrets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<String>,
}

impl InstanceCatalogResult {
    /// True when this result is a complete live fetch suitable for cache store.
    pub fn is_complete_live(&self) -> bool {
        self.source == CatalogFetchSource::Live && self.truncation.is_complete()
    }

    /// True when this is a complete (non-truncated) catalog suitable for
    /// publication as an account row (live or real disk LKG).
    pub fn is_complete_publishable(&self) -> bool {
        self.truncation.is_complete()
    }
}

/// Identity material required to bind a catalog fetch to PR7 cache storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogAccountIdentity {
    pub instance_id: ProviderId,
    pub kind: ProviderKind,
    pub api_surface: ApiSurface,
    pub credential_route: CredentialRoute,
    pub endpoint_origin: String,
    pub org_project_fingerprint: String,
    pub incarnation: ProviderIncarnation,
    pub credential_binding_id: CredentialBindingId,
    /// When true, use built-in compatibility selection ids
    /// (`openai:<slug>` / `openrouter:<slug>`). Additional accounts use
    /// `<instance>:<verbatim-upstream>`.
    pub is_built_in_compatibility: bool,
}

/// Error from one account catalog adapter. Secret-free and size-bounded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogAdapterError {
    Bound(super::bounds::CatalogBoundError),
    AuthFailure { status: u16 },
    Transport { detail: String },
    Malformed { detail: String },
    OriginEscape,
    Cancelled,
    MissingCredential,
    InvalidOrigin { detail: String },
}

impl std::fmt::Display for CatalogAdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bound(e) => write!(f, "{e}"),
            Self::AuthFailure { status } => {
                write!(f, "catalog authentication failed (HTTP {status})")
            }
            Self::Transport { detail } => write!(f, "catalog transport failure: {detail}"),
            Self::Malformed { detail } => write!(f, "malformed catalog page: {detail}"),
            Self::OriginEscape => write!(f, "catalog next-page URL left authorized origin"),
            Self::Cancelled => write!(f, "catalog fetch cancelled"),
            Self::MissingCredential => write!(f, "missing application credential for catalog"),
            Self::InvalidOrigin { detail } => write!(f, "invalid catalog origin: {detail}"),
        }
    }
}

impl std::error::Error for CatalogAdapterError {}

impl From<super::bounds::CatalogBoundError> for CatalogAdapterError {
    fn from(value: super::bounds::CatalogBoundError) -> Self {
        Self::Bound(value)
    }
}

impl CatalogAdapterError {
    pub fn to_truncation(&self) -> CatalogTruncationReason {
        match self {
            Self::Bound(e) => CatalogTruncationReason::BoundExceeded {
                detail: e.to_string(),
            },
            Self::AuthFailure { status } => {
                CatalogTruncationReason::AuthFailure { status: *status }
            }
            Self::Transport { detail } => CatalogTruncationReason::TransportFailure {
                detail: detail.clone(),
            },
            Self::Malformed { detail } => CatalogTruncationReason::MalformedPage {
                detail: detail.clone(),
            },
            Self::OriginEscape => CatalogTruncationReason::OriginEscape,
            Self::Cancelled => CatalogTruncationReason::Cancelled,
            Self::MissingCredential => CatalogTruncationReason::AuthFailure { status: 401 },
            Self::InvalidOrigin { detail } => CatalogTruncationReason::MalformedPage {
                detail: detail.clone(),
            },
        }
    }

    /// Redact accidental credential-like substrings from transport diagnostics.
    pub fn sanitize_detail(raw: &str) -> String {
        // Never retain Authorization headers, bearer tokens, or secret query keys.
        let lower = raw.to_ascii_lowercase();
        if lower.contains("authorization")
            || lower.contains("bearer ")
            || lower.contains("api_key")
            || lower.contains("api-key")
            || lower.contains("x-api-key")
            || lower.contains("access_token")
            || lower.contains("?token=")
            || lower.contains("&token=")
            || lower.contains("?key=")
            || lower.contains("&key=")
            || lower.contains("?secret=")
            || lower.contains("&secret=")
        {
            return "redacted transport error".to_owned();
        }
        // Cap length so huge bodies never reach logs.
        const MAX: usize = 200;
        if raw.chars().count() > MAX {
            let truncated: String = raw.chars().take(MAX).collect();
            format!("{truncated}…")
        } else {
            raw.to_owned()
        }
    }
}
