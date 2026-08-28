//! Canonical provider-instance service over a frozen snapshot.
//!
//! [`ProviderService`] owns one immutable [`ProviderRegistrySnapshot`] plus the
//! credential-free [`ProviderInstanceDescriptor`]s derived from it, and exposes
//! safe `get`/`list`/`enabled`/`generation` access.
//!
//! Product built-ins (`xai`, `openai`, `openrouter`, `anthropic`) are modeled
//! as canonical [`ProviderRef::BuiltIn`] descriptors in
//! [`BuiltInProviderId::ALL`] order. Each canonical descriptor selects a whole
//! source config with an explicit, tested precedence: the canonical id table
//! wins when present; otherwise the first legacy alias in documented order
//! (`chatgpt` before `codex` for OpenAI, `grok` for xAI) wins as a whole
//! config; otherwise the internal `grok_build_*` preset source; otherwise the
//! canonical built-in default. No fields are merged across sources. The
//! internal `grok_build_*` ids never become descriptors; selected reserved
//! tables keep their actual `kind`/routes/booleans/lists/headers/backend while
//! product identity stays [`ProviderRef::BuiltIn`]. Z.ai remains its
//! configured-style `zai-model-api` instance; other configured entries follow
//! in input order.
//!
//! The service is strictly additive: it does not replace
//! [`crate::agent::providers::ProviderManager`] or any composition root, never
//! publishes duplicate account models, and never touches caches or
//! user-visible catalogs. No credential material is read or copied from the
//! vault, environment, or an auth helper; explicit `api_key` values and inline
//! auth-helper commands are omitted. Preserved OpenRouter policy/plugin JSON
//! is user-authored local config and must be treated as potentially sensitive
//! (never logged or sent to external telemetry).

use super::id::{BuiltInProviderId, ProviderId, ProviderIdError, ProviderRef};
use super::instance::{
    ApiSurface, CredentialRoute, ProviderInstanceDescriptor, ProviderKind, ProviderRouteDescriptor,
};
use super::lifecycle::{
    CapabilityMode, CapabilityOverrides, ProviderAuthScheme, ProviderMetadata,
    ProviderRegistrySnapshot, validate_http_base_url,
};
use crate::agent::config::EnvKeys;
use crate::agent::model_providers::{
    ModelProviderConfig, ModelProviderKind, grok_build_anthropic_config, grok_build_openai_config,
    grok_build_openrouter_config, model_provider_auth_name,
};
use crate::agent::zai::{ZAI_PROVIDER_ID, zai_builtin_provider_config};
use crate::inference::ApiBackend;
use indexmap::IndexMap;
use std::sync::Arc;

/// Default snapshot generation for a freshly built service.
const DEFAULT_GENERATION: u64 = 0;

/// Internal `model_providers` alias ids that act as preset config sources for
/// canonical built-ins (never distinct account descriptors).
const INTERNAL_ALIAS_IDS: [&str; 3] = [
    "grok_build_openai",
    "grok_build_openrouter",
    "grok_build_anthropic",
];

/// Legacy reserved alias ids in documented precedence order (first wins). The
/// canonical id table takes precedence over all of these, and each list's
/// first entry wins over later ones.
fn legacy_alias_ids(built_in: BuiltInProviderId) -> &'static [&'static str] {
    match built_in {
        BuiltInProviderId::Xai => &["grok"],
        BuiltInProviderId::OpenAi => &["chatgpt", "codex"],
        BuiltInProviderId::OpenRouter => &[],
        BuiltInProviderId::Anthropic => &[],
    }
}

/// The internal `grok_build_*` preset source id for a built-in (xai has none).
fn internal_preset_id(built_in: BuiltInProviderId) -> Option<&'static str> {
    match built_in {
        BuiltInProviderId::Xai => None,
        BuiltInProviderId::OpenAi => Some(INTERNAL_ALIAS_IDS[0]),
        BuiltInProviderId::OpenRouter => Some(INTERNAL_ALIAS_IDS[1]),
        BuiltInProviderId::Anthropic => Some(INTERNAL_ALIAS_IDS[2]),
    }
}

/// Error building a [`ProviderService`] from config. There is no panic path:
/// invalid ids and embedded-credential URLs are reported as errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderServiceError {
    InvalidId { id: String, error: ProviderIdError },
    InvalidBaseUrl { id: String, error: String },
    InvalidAdminBaseUrl { id: String, error: String },
}

impl std::fmt::Display for ProviderServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidId { id, error } => write!(f, "invalid provider id `{id}`: {error}"),
            Self::InvalidBaseUrl { id, error } => {
                write!(f, "invalid base URL for provider `{id}`: {error}")
            }
            Self::InvalidAdminBaseUrl { id, error } => {
                write!(f, "invalid admin base URL for provider `{id}`: {error}")
            }
        }
    }
}

impl std::error::Error for ProviderServiceError {}

/// Cloneable provider service exposing safe instance metadata.
///
/// Clones are cheap and share the same immutable snapshot/descriptors.
#[derive(Clone, Debug)]
pub struct ProviderService {
    snapshot: Arc<ProviderRegistrySnapshot>,
    descriptors: Arc<IndexMap<String, ProviderInstanceDescriptor>>,
    generation: u64,
}

impl Default for ProviderService {
    fn default() -> Self {
        // Internally valid canonical constants; cannot fail.
        Self::from_model_providers(&IndexMap::new())
            .expect("canonical built-ins are internally valid")
    }
}

impl ProviderService {
    /// Build a service from `[model_providers.<id>]` entries plus the canonical
    /// built-in product descriptors.
    pub fn from_model_providers(
        entries: &IndexMap<String, ModelProviderConfig>,
    ) -> Result<Self, ProviderServiceError> {
        let mut descriptors: IndexMap<String, ProviderInstanceDescriptor> = IndexMap::new();
        let mut metadata: IndexMap<String, ProviderMetadata> = IndexMap::new();

        // 1. Canonical built-in product descriptors, `BuiltInProviderId::ALL`
        // order (xai, openai, openrouter, anthropic).
        for built_in in BuiltInProviderId::ALL {
            let (desc, meta) = Self::canonical_descriptor(built_in, entries)?;
            metadata.insert(built_in.as_str().to_owned(), meta);
            descriptors.insert(built_in.as_str().to_owned(), desc);
        }

        // 2. Z.ai: configured-style first-class instance.
        let zai_cfg = entries
            .get(ZAI_PROVIDER_ID)
            .cloned()
            .unwrap_or_else(zai_builtin_provider_config);
        let (desc, meta) = Self::build_from_config(ZAI_PROVIDER_ID, ZAI_PROVIDER_ID, &zai_cfg)?;
        metadata.insert(ZAI_PROVIDER_ID.to_owned(), meta);
        descriptors.insert(ZAI_PROVIDER_ID.to_owned(), desc);

        // 3. Remaining configured entries deterministically in input order.
        for (id, cfg) in entries {
            if BuiltInProviderId::parse(id).is_some()
                || is_internal_alias(id)
                || id == ZAI_PROVIDER_ID
            {
                continue;
            }
            let (desc, meta) = Self::build_from_config(id, id, cfg)?;
            metadata.insert(id.clone(), meta);
            descriptors.insert(id.clone(), desc);
        }

        Ok(Self::assemble(descriptors, metadata, DEFAULT_GENERATION))
    }

    /// Build a single-instance service from one already-parsed config entry
    /// (test/derivation helper). The entry is used exactly as configured: the
    /// id and its single explicit/default route are preserved even for
    /// reserved ids (for example `from_model_provider("openai", cfg)` keeps
    /// the `openai` id and yields one route, not the composite canonical
    /// product route set). Returns `Err` for invalid ids or embedded-credential
    /// URLs. Canonical composite product semantics require
    /// [`ProviderService::from_model_providers`].
    pub fn from_model_provider(
        id: &str,
        cfg: &ModelProviderConfig,
        generation: u64,
    ) -> Result<Self, ProviderServiceError> {
        let (desc, meta) = Self::build_from_config(id, id, cfg)?;
        let mut descriptors = IndexMap::new();
        descriptors.insert(id.to_owned(), desc);
        let mut metadata = IndexMap::new();
        metadata.insert(id.to_owned(), meta);
        Ok(Self::assemble(descriptors, metadata, generation))
    }

    /// Build a service with a new generation number (identity preserved).
    pub fn with_generation(mut self, generation: u64) -> Self {
        self.generation = generation;
        self
    }

    /// Attach Grok-owned incarnations from lifecycle state (secret-free).
    /// Built-ins receive stable compatibility incarnations. Configured
    /// instances receive their durable row when present.
    pub fn with_lifecycle_incarnations(mut self, home: &std::path::Path) -> Self {
        use super::lifecycle_state::{load_lifecycle_state, stable_builtin_incarnation};
        let state = load_lifecycle_state(home).unwrap_or_default();
        let mut descriptors = (*self.descriptors).clone();
        for (id, desc) in descriptors.iter_mut() {
            if desc.incarnation.is_some() {
                continue;
            }
            if let Some(inc) = state.incarnation_for(id) {
                desc.incarnation = Some(inc);
            } else if let Some(inc) = stable_builtin_incarnation(id) {
                desc.incarnation = Some(inc);
            }
        }
        self.descriptors = Arc::new(descriptors);
        self
    }

    /// Monotonic snapshot generation, stable for the life of this service.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// The owned immutable registry snapshot backing this service.
    pub fn snapshot(&self) -> &ProviderRegistrySnapshot {
        &self.snapshot
    }

    /// Look up a descriptor by its configured id.
    pub fn get(&self, id: &str) -> Option<&ProviderInstanceDescriptor> {
        self.descriptors.get(id)
    }

    /// Enumerate all descriptors in stable snapshot ordering.
    pub fn list(&self) -> Vec<&ProviderInstanceDescriptor> {
        self.descriptors.values().collect()
    }

    /// Iterate descriptors for enabled instances in stable ordering.
    pub fn enabled(&self) -> impl Iterator<Item = &ProviderInstanceDescriptor> {
        self.descriptors.values().filter(|d| d.enabled)
    }

    /// Whether any instance advertises a route using the given credential
    /// route, across all surfaces and descriptors. Credential-free.
    pub fn has_credential_route(&self, route: CredentialRoute) -> bool {
        self.descriptors
            .values()
            .any(|d| d.has_credential_route(route))
    }

    /// Number of registered instances (including built-ins).
    pub fn len(&self) -> usize {
        self.descriptors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.descriptors.is_empty()
    }

    fn canonical_descriptor(
        built_in: BuiltInProviderId,
        entries: &IndexMap<String, ModelProviderConfig>,
    ) -> Result<(ProviderInstanceDescriptor, ProviderMetadata), ProviderServiceError> {
        // Whole-config source selection with explicit, tested precedence:
        //   1. the canonical id table (`openai`, `xai`, `openrouter`,
        //      `anthropic`) wins when present;
        //   2. otherwise the first legacy alias in documented order
        //      (`chatgpt` before `codex` for OpenAI; `grok` for xAI) wins as a
        //      whole config;
        //   3. otherwise the internal `grok_build_*` preset source if present;
        //   4. otherwise the canonical built-in default.
        // No fields are merged across sources: a selected table is used
        // exactly as authored, so absent default-valued fields never clobber
        // another source.
        let canonical_id = built_in.as_str();
        let source_key: Option<&str> = if entries.contains_key(canonical_id) {
            Some(canonical_id)
        } else if let Some(alias) = legacy_alias_ids(built_in)
            .iter()
            .copied()
            .find(|&a| entries.contains_key(a))
        {
            Some(alias)
        } else {
            None
        };
        let explicit_source = source_key.is_some();
        let (config_source, auth_namespace) = match source_key {
            Some(key) => (
                entries
                    .get(key)
                    .cloned()
                    .expect("selection key verified present"),
                key,
            ),
            None => internal_preset_source(built_in, entries),
        };
        let (mut desc, meta) =
            Self::build_from_config(canonical_id, auth_namespace, &config_source)?;
        // Product identity is `ProviderRef::BuiltIn` (already parsed from the
        // canonical id); protocol kind stays the selected config's kind — it is
        // never forcibly overwritten here.
        desc.routes = if explicit_source {
            vec![Self::single_route(&config_source)]
        } else {
            canonical_routes(desc.kind)
        };
        Ok((desc, meta))
    }

    /// `auth_namespace` is the config table key the config actually lived
    /// under (used to synthesize `model_provider:<key>` auth-helper refs), so
    /// inline-auth naming matches runtime registration even for reserved/alias
    /// tables that map onto a canonical descriptor id.
    fn build_from_config(
        id: &str,
        auth_namespace: &str,
        cfg: &ModelProviderConfig,
    ) -> Result<(ProviderInstanceDescriptor, ProviderMetadata), ProviderServiceError> {
        let pid = ProviderId::new(id).map_err(|error| ProviderServiceError::InvalidId {
            id: id.to_owned(),
            error,
        })?;
        let provider_ref =
            ProviderRef::parse(id).map_err(|error| ProviderServiceError::InvalidId {
                id: id.to_owned(),
                error,
            })?;
        if let Some(url) = cfg.base_url.as_deref().or(cfg.api_base_url.as_deref())
            && let Err(e) = validate_http_base_url(url)
        {
            return Err(ProviderServiceError::InvalidBaseUrl {
                id: id.to_owned(),
                error: e.to_string(),
            });
        }
        if let Some(url) = cfg.admin_base_url.as_deref()
            && let Err(e) = validate_http_base_url(url)
        {
            return Err(ProviderServiceError::InvalidAdminBaseUrl {
                id: id.to_owned(),
                error: e.to_string(),
            });
        }
        let kind = ProviderKind::from(cfg.kind);
        let desc = ProviderInstanceDescriptor {
            id: pid.clone(),
            provider_ref: provider_ref.clone(),
            kind,
            routes: vec![Self::single_route(cfg)],
            display_name: cfg.display_name.clone(),
            enabled: cfg.enabled,
            base_url: cfg.base_url.clone().or_else(|| cfg.api_base_url.clone()),
            admin_base_url: cfg.admin_base_url.clone(),
            incarnation: None,
            env_keys: cfg
                .env_key
                .as_ref()
                .map(EnvKeys::names)
                .unwrap_or_default()
                .into_iter()
                .map(str::to_owned)
                .collect(),
            api_backend: cfg.api_backend.clone().map(backend_str),
            auth_scheme: cfg.auth_scheme.clone(),
            auth_provider: match (&cfg.auth_provider, &cfg.auth) {
                // An explicit auth_provider reference always wins.
                (Some(name), _) => Some(name.clone()),
                // Inline auth helper: synthesize the same registry key the
                // config path uses (`model_provider:<table key>`).
                (None, Some(_)) => Some(model_provider_auth_name(auth_namespace)),
                (None, None) => None,
            },
            openrouter_fallback_models: cfg.openrouter_fallback_models.clone(),
            openrouter_provider_preferences: cfg.provider_preferences.clone(),
            openrouter_plugins: cfg.plugins.clone(),
            openrouter_pacing: cfg.openrouter_pacing,
            max_completion_tokens: cfg.max_completion_tokens.filter(|&n| n > 0),
        };
        let meta = Self::metadata_from_config(&pid, cfg, provider_ref);
        Ok((desc, meta))
    }

    /// Derive the single explicit/default route for a configured instance.
    fn single_route(cfg: &ModelProviderConfig) -> ProviderRouteDescriptor {
        let kind = ProviderKind::from(cfg.kind);
        let surface = cfg.api_surface.unwrap_or_else(|| default_surface(kind));
        let route = cfg
            .credential_route
            .unwrap_or_else(|| default_route(kind, cfg));
        ProviderRouteDescriptor::new(surface, route)
    }

    /// Build [`ProviderMetadata`] from the source config so no representable
    /// safe field is lost. Secrets (`api_key`) are never carried.
    fn metadata_from_config(
        id: &ProviderId,
        cfg: &ModelProviderConfig,
        provider_ref: ProviderRef,
    ) -> ProviderMetadata {
        ProviderMetadata {
            id: id.clone(),
            provider_ref,
            display_name: cfg.display_name.clone(),
            kind: cfg.kind.as_config_str().to_owned(),
            base_url: cfg.base_url.clone().or_else(|| cfg.api_base_url.clone()),
            admin_base_url: cfg.admin_base_url.clone(),
            enabled: cfg.enabled,
            default_backend: cfg.default_backend.clone(),
            auth_scheme: auth_scheme_from_config(cfg.auth_scheme.as_deref()),
            env_key: cfg
                .env_key
                .as_ref()
                .and_then(EnvKeys::primary)
                .map(str::to_owned),
            admin_env_key: cfg.admin_env_key.clone(),
            catalog_enabled: cfg.catalog_enabled,
            capability_mode: capability_mode_from_config(cfg.capability_mode.as_deref()),
            catalog_ttl_secs: cfg.catalog_ttl_secs,
            request_timeout_secs: cfg.request_timeout_secs,
            organization: cfg.organization.clone(),
            project: cfg.project.clone(),
            extra_headers: cfg.extra_headers.clone(),
            capabilities: capabilities_from_config(&cfg.capabilities),
        }
    }

    fn assemble(
        descriptors: IndexMap<String, ProviderInstanceDescriptor>,
        metadata: IndexMap<String, ProviderMetadata>,
        generation: u64,
    ) -> Self {
        Self {
            snapshot: Arc::new(ProviderRegistrySnapshot {
                providers: metadata,
                warnings: Vec::new(),
            }),
            descriptors: Arc::new(descriptors),
            generation,
        }
    }
}

fn is_internal_alias(id: &str) -> bool {
    INTERNAL_ALIAS_IDS.contains(&id)
}

/// Source config + auth namespace for a built-in when no reserved table exists:
/// the internal `grok_build_*` preset entry if present (its table key becomes
/// the auth namespace so inline-auth naming matches runtime registration),
/// else the canonical built-in default (canonical id).
fn internal_preset_source(
    built_in: BuiltInProviderId,
    entries: &IndexMap<String, ModelProviderConfig>,
) -> (ModelProviderConfig, &'static str) {
    if let Some(preset) = internal_preset_id(built_in)
        && let Some(cfg) = entries.get(preset)
    {
        (cfg.clone(), preset)
    } else {
        (canonical_builtin_config(built_in), built_in.as_str())
    }
}

/// Canonical default config for a built-in (used when no alias preset exists).
fn canonical_builtin_config(built_in: BuiltInProviderId) -> ModelProviderConfig {
    match built_in {
        BuiltInProviderId::Xai => ModelProviderConfig {
            kind: ModelProviderKind::Xai,
            base_url: Some("https://api.x.ai/v1".into()),
            display_name: Some("xAI".into()),
            ..Default::default()
        },
        BuiltInProviderId::OpenAi => grok_build_openai_config(),
        BuiltInProviderId::OpenRouter => grok_build_openrouter_config(),
        BuiltInProviderId::Anthropic => grok_build_anthropic_config(),
    }
}

/// Canonical multi-route set for a first-party built-in product descriptor.
fn canonical_routes(kind: ProviderKind) -> Vec<ProviderRouteDescriptor> {
    match kind {
        // xAI serves its actual session and API-key compatibility routes.
        ProviderKind::Xai => vec![
            ProviderRouteDescriptor::new(
                ApiSurface::OpenAiCompatibleSubset,
                CredentialRoute::XaiSession,
            ),
            ProviderRouteDescriptor::new(
                ApiSurface::OpenAiCompatibleSubset,
                CredentialRoute::ApiKey,
            ),
        ],
        // OpenAI product serves both the Platform/API-key route and the
        // ChatGPT subscription/OAuth inference route.
        ProviderKind::OpenAi => vec![
            ProviderRouteDescriptor::new(ApiSurface::OpenAiPlatform, CredentialRoute::ApiKey),
            ProviderRouteDescriptor::new(
                ApiSurface::ChatGptInference,
                CredentialRoute::ChatGptOauth,
            ),
        ],
        ProviderKind::OpenRouter => vec![ProviderRouteDescriptor::new(
            ApiSurface::OpenRouterNative,
            CredentialRoute::ApiKey,
        )],
        ProviderKind::Anthropic => vec![ProviderRouteDescriptor::new(
            ApiSurface::AnthropicMessages,
            CredentialRoute::ApiKey,
        )],
        ProviderKind::OpenAiCompatible => vec![ProviderRouteDescriptor::new(
            ApiSurface::OpenAiCompatibleSubset,
            CredentialRoute::ApiKey,
        )],
        ProviderKind::Zai => vec![ProviderRouteDescriptor::new(
            ApiSurface::OpenAiCompatibleSubset,
            CredentialRoute::ApiKey,
        )],
    }
}

/// Backward-compatible default surface for a provider kind.
fn default_surface(kind: ProviderKind) -> ApiSurface {
    match kind {
        ProviderKind::Xai => ApiSurface::OpenAiCompatibleSubset,
        ProviderKind::OpenAi => ApiSurface::OpenAiPlatform,
        ProviderKind::OpenRouter => ApiSurface::OpenRouterNative,
        ProviderKind::Anthropic => ApiSurface::AnthropicMessages,
        ProviderKind::OpenAiCompatible => ApiSurface::OpenAiCompatibleSubset,
        ProviderKind::Zai => ApiSurface::OpenAiCompatibleSubset,
    }
}

/// Backward-compatible default credential route for a configured instance.
///
/// An explicit auth helper wins over static defaults. A configured `xai` id
/// never receives `XaiSession` by kind alone — only the canonical first-party
/// built-in advertises that route.
fn default_route(kind: ProviderKind, cfg: &ModelProviderConfig) -> CredentialRoute {
    if cfg.auth.is_some() || cfg.auth_provider.is_some() {
        return CredentialRoute::AuthHelper;
    }
    match kind {
        ProviderKind::Xai => CredentialRoute::ApiKey,
        ProviderKind::OpenAi => CredentialRoute::ApiKey,
        ProviderKind::OpenRouter => CredentialRoute::ApiKey,
        ProviderKind::Anthropic => CredentialRoute::ApiKey,
        ProviderKind::Zai => CredentialRoute::ApiKey,
        ProviderKind::OpenAiCompatible => {
            if cfg.api_key.is_some() || cfg.env_key.as_ref().and_then(EnvKeys::primary).is_some() {
                CredentialRoute::ApiKey
            } else {
                CredentialRoute::None
            }
        }
    }
}

fn auth_scheme_from_config(raw: Option<&str>) -> ProviderAuthScheme {
    match raw.map(str::trim).unwrap_or("") {
        "bearer" => ProviderAuthScheme::Bearer,
        "none" => ProviderAuthScheme::None,
        // Anthropic-style `x-api-key` auth is header-based, so CustomHeader is
        // the truthful representation and never claims a Bearer token. The
        // exact spelling is retained on the descriptor's `auth_scheme`.
        "custom_header" | "x_api_key" | "x-api-key" | "xapikey" => ProviderAuthScheme::CustomHeader,
        _ => ProviderAuthScheme::default(),
    }
}

fn capability_mode_from_config(raw: Option<&str>) -> CapabilityMode {
    match raw.map(str::trim).unwrap_or("") {
        "manual" => CapabilityMode::Manual,
        "off" => CapabilityMode::Off,
        _ => CapabilityMode::Auto,
    }
}

fn capabilities_from_config(map: &IndexMap<String, bool>) -> CapabilityOverrides {
    let mut cap = CapabilityOverrides::default();
    for (k, v) in map {
        match k.as_str() {
            "chat_completions" => cap.chat_completions = Some(*v),
            "responses" => cap.responses = Some(*v),
            "embeddings" => cap.embeddings = Some(*v),
            "images" => cap.images = Some(*v),
            "audio" => cap.audio = Some(*v),
            "files" => cap.files = Some(*v),
            "batches" => cap.batches = Some(*v),
            "fine_tuning" => cap.fine_tuning = Some(*v),
            "admin" => cap.admin = Some(*v),
            other => {
                cap.extra.insert(other.to_owned(), *v);
            }
        }
    }
    cap
}

fn backend_str(backend: ApiBackend) -> String {
    match backend {
        ApiBackend::ChatCompletions => "chat_completions".into(),
        ApiBackend::Responses => "responses".into(),
        ApiBackend::Messages => "messages".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::model_providers::parse_model_providers;

    /// Parse a `[model_providers.<id>]` entry, failing loudly if the provider
    /// is dropped rather than silently falling back to a default.
    fn parse_provider(toml: &str, id: &str) -> (String, ModelProviderConfig) {
        let raw: toml::Value = toml::from_str(toml).expect("valid toml shell");
        let (parsed, _warnings) = parse_model_providers(&raw);
        let cfg = parsed
            .get(id)
            .cloned()
            .expect("provider must be parsed and retained (not dropped)");
        (id.to_owned(), cfg)
    }

    fn entries(toml: &str) -> IndexMap<String, ModelProviderConfig> {
        let raw: toml::Value = toml::from_str(toml).expect("valid toml shell");
        parse_model_providers(&raw).0
    }

    #[test]
    fn canonical_builtins_in_all_order_with_builtin_refs() {
        let svc = ProviderService::default();
        let ids: Vec<&str> = svc.list().iter().map(|d| d.id.as_str()).collect();
        assert_eq!(
            ids,
            ["xai", "openai", "openrouter", "anthropic", "zai-model-api"]
        );
        for id in ["xai", "openai", "openrouter", "anthropic"] {
            let d = svc.get(id).unwrap();
            assert!(
                matches!(d.provider_ref, ProviderRef::BuiltIn(_)),
                "{id} must be a BuiltIn ref"
            );
        }
        let z = svc.get("zai-model-api").unwrap();
        assert!(matches!(z.provider_ref, ProviderRef::Configured(_)));
        // Default is infallible through internally valid constants.
        assert_eq!(svc.generation(), 0);
    }

    #[test]
    fn internal_aliases_do_not_duplicate_descriptors() {
        // A config that went through preset install carries the grok_build_*
        // aliases. The service must not expose them as accounts.
        let mut m = IndexMap::new();
        m.insert("grok_build_openai".into(), grok_build_openai_config());
        m.insert(
            "grok_build_openrouter".into(),
            grok_build_openrouter_config(),
        );
        m.insert("grok_build_anthropic".into(), grok_build_anthropic_config());
        let svc = ProviderService::from_model_providers(&m).unwrap();
        assert!(
            svc.get("grok_build_openai").is_none(),
            "alias is not a descriptor"
        );
        assert!(svc.get("grok_build_openrouter").is_none());
        assert!(svc.get("grok_build_anthropic").is_none());
        // Canonical descriptors exist and are sourced from the aliases.
        assert_eq!(svc.get("openai").unwrap().kind, ProviderKind::OpenAi);
        assert_eq!(svc.get("anthropic").unwrap().kind, ProviderKind::Anthropic);
        // No duplicate "openai" accounts.
        assert_eq!(
            svc.list()
                .iter()
                .filter(|d| d.id.as_str() == "openai")
                .count(),
            1
        );
    }

    #[test]
    fn reserved_override_merges_into_canonical_builtin() {
        let mut m = IndexMap::new();
        m.insert(
            "openai".into(),
            ModelProviderConfig {
                kind: ModelProviderKind::OpenAi,
                base_url: Some("https://openai.example/v1".into()),
                display_name: Some("Corporate OpenAI".into()),
                enabled: false,
                ..Default::default()
            },
        );
        let svc = ProviderService::from_model_providers(&m).unwrap();
        let d = svc.get("openai").unwrap();
        // Stays canonical built-in, not configured.
        assert!(matches!(
            d.provider_ref,
            ProviderRef::BuiltIn(BuiltInProviderId::OpenAi)
        ));
        assert_eq!(d.kind, ProviderKind::OpenAi);
        assert_eq!(d.base_url.as_deref(), Some("https://openai.example/v1"));
        assert_eq!(d.display_name.as_deref(), Some("Corporate OpenAI"));
        assert!(!d.enabled, "override table disabled the built-in");
        // An explicit reserved source table derives its single route truthfully.
        assert_eq!(d.routes.len(), 1);
        assert_eq!(d.routes[0].api_surface, ApiSurface::OpenAiPlatform);
        assert_eq!(d.routes[0].credential_route, CredentialRoute::ApiKey);
        // Single instance, not two.
        assert_eq!(
            svc.list()
                .iter()
                .filter(|d| d.id.as_str() == "openai")
                .count(),
            1
        );
        // Metadata preserved from the whole selected table (no merge).
        assert_eq!(
            svc.snapshot().get("openai").unwrap().base_url.as_deref(),
            Some("https://openai.example/v1")
        );
    }

    #[test]
    fn composite_route_sets_are_truthful() {
        let svc = ProviderService::default();
        let openai = svc.get("openai").unwrap();
        assert!(openai.has_route(ApiSurface::OpenAiPlatform, CredentialRoute::ApiKey));
        assert!(openai.has_route(ApiSurface::ChatGptInference, CredentialRoute::ChatGptOauth));

        let xai = svc.get("xai").unwrap();
        // Only the canonical first-party built-in advertises the session route.
        assert!(xai.has_route(
            ApiSurface::OpenAiCompatibleSubset,
            CredentialRoute::XaiSession
        ));
        assert!(xai.has_route(ApiSurface::OpenAiCompatibleSubset, CredentialRoute::ApiKey));

        let anthropic = svc.get("anthropic").unwrap();
        assert!(anthropic.has_route(ApiSurface::AnthropicMessages, CredentialRoute::ApiKey));
    }

    #[test]
    fn configured_instance_gets_single_default_route() {
        let (id, cfg) = parse_provider(
            r#"[model_providers.my-gw]
            kind = "custom"
            base_url = "http://127.0.0.1:8126/v1"
            "#,
            "my-gw",
        );
        let svc = ProviderService::from_model_provider(&id, &cfg, 0).unwrap();
        let d = svc.get("my-gw").unwrap();
        assert_eq!(d.routes.len(), 1);
        assert_eq!(d.routes[0].api_surface, ApiSurface::OpenAiCompatibleSubset);
        assert_eq!(d.routes[0].credential_route, CredentialRoute::None);
    }

    #[test]
    fn configured_xai_never_gets_session_route() {
        let (id, cfg) = parse_provider(
            r#"[model_providers.xai-proxy]
            kind = "xai"
            base_url = "https://proxy.example/v1"
            "#,
            "xai-proxy",
        );
        let svc = ProviderService::from_model_provider(&id, &cfg, 0).unwrap();
        let d = svc.get("xai-proxy").unwrap();
        assert_eq!(d.routes.len(), 1);
        assert_eq!(d.routes[0].credential_route, CredentialRoute::ApiKey);
        assert!(!d.has_credential_route(CredentialRoute::XaiSession));
    }

    #[test]
    fn old_config_parse_and_defaults_are_backward_compatible() {
        let (id, cfg) = parse_provider(
            r#"[model_providers.openai]
            kind = "openai"
            base_url = "https://api.openai.com/v1"
            display_name = "OpenAI"
            env_key = "OPENAI_API_KEY"
            "#,
            "openai",
        );
        let svc = ProviderService::from_model_provider(&id, &cfg, 0).unwrap();
        let d = svc.get("openai").unwrap();
        assert_eq!(d.kind, ProviderKind::OpenAi);
        assert!(d.has_route(ApiSurface::OpenAiPlatform, CredentialRoute::ApiKey));
        assert_eq!(d.env_keys, ["OPENAI_API_KEY"]);
        assert!(d.enabled);
    }

    #[test]
    fn enabled_filtering_and_has_credential_route() {
        let mut m = IndexMap::new();
        for (id, kind, enabled) in [
            ("a", ModelProviderKind::OpenAiCompatible, true),
            ("b", ModelProviderKind::OpenAiCompatible, false),
        ] {
            m.insert(
                id.into(),
                ModelProviderConfig {
                    kind,
                    enabled,
                    base_url: Some("http://127.0.0.1:8127/v1".into()),
                    ..Default::default()
                },
            );
        }
        let svc = ProviderService::from_model_providers(&m).unwrap();
        let enabled_ids: Vec<&str> = svc.enabled().map(|d| d.id.as_str()).collect();
        assert!(enabled_ids.contains(&"a"));
        assert!(
            !enabled_ids.contains(&"b"),
            "disabled instance excluded from enabled()"
        );
        assert!(svc.get("b").is_some() && !svc.get("b").unwrap().enabled);
        // has_credential_route inspects the whole route set (incl. built-ins).
        assert!(svc.has_credential_route(CredentialRoute::ApiKey));
        assert!(svc.has_credential_route(CredentialRoute::XaiSession));
        assert!(svc.has_credential_route(CredentialRoute::ChatGptOauth));
    }

    #[test]
    fn metadata_preserves_safe_config_fields() {
        let toml = r#"[model_providers.rich]
            kind = "custom"
            base_url = "https://rich.example/v1"
            admin_base_url = "https://admin.example/v1"
            display_name = "Rich"
            enabled = true
            default_backend = "chat_completions"
            auth_scheme = "bearer"
            env_key = "RICH_API_KEY"
            admin_env_key = "RICH_ADMIN_KEY"
            catalog_enabled = true
            capability_mode = "manual"
            catalog_ttl_secs = 120
            request_timeout_secs = 45
            organization = "acme"
            project = "proj"
            api_backend = "responses"

            [model_providers.rich.capabilities]
            chat_completions = true
            native_web_search = false

            [model_providers.rich.extra_headers]
            X-Corp = "yes"
        "#;
        let (id, cfg) = parse_provider(toml, "rich");
        let svc = ProviderService::from_model_provider(&id, &cfg, 0).unwrap();
        let meta = svc.snapshot().get("rich").expect("descriptor present");
        assert_eq!(meta.display_name.as_deref(), Some("Rich"));
        assert_eq!(meta.default_backend.as_deref(), Some("chat_completions"));
        assert_eq!(
            meta.admin_base_url.as_deref(),
            Some("https://admin.example/v1")
        );
        assert_eq!(meta.env_key.as_deref(), Some("RICH_API_KEY"));
        assert_eq!(meta.admin_env_key.as_deref(), Some("RICH_ADMIN_KEY"));
        assert_eq!(meta.catalog_enabled, true);
        assert_eq!(meta.capability_mode, CapabilityMode::Manual);
        assert_eq!(meta.catalog_ttl_secs, Some(120));
        assert_eq!(meta.request_timeout_secs, Some(45));
        assert_eq!(meta.organization.as_deref(), Some("acme"));
        assert_eq!(meta.project.as_deref(), Some("proj"));
        assert_eq!(
            meta.extra_headers.get("X-Corp").map(String::as_str),
            Some("yes")
        );
        assert_eq!(meta.capabilities.chat_completions, Some(true));
        assert_eq!(
            meta.capabilities.extra.get("native_web_search"),
            Some(&false)
        );

        // Config state not representable in ProviderMetadata stays on the descriptor.
        let d = svc.get("rich").unwrap();
        assert_eq!(d.env_keys, ["RICH_API_KEY"]);
        assert_eq!(
            d.admin_base_url.as_deref(),
            Some("https://admin.example/v1")
        );
        assert_eq!(d.api_backend.as_deref(), Some("responses"));
        assert_eq!(d.auth_scheme.as_deref(), Some("bearer"));
        assert!(!d.has_credential_route(CredentialRoute::XaiSession));
    }

    #[test]
    fn anthropic_x_api_key_maps_to_custom_header_and_keeps_spelling() {
        let toml = r#"[model_providers.anthropic]
            kind = "anthropic"
            base_url = "https://api.anthropic.com/v1"
            auth_scheme = "x_api_key"
        "#;
        let (id, cfg) = parse_provider(toml, "anthropic");
        let svc = ProviderService::from_model_provider(&id, &cfg, 0).unwrap();
        let meta = svc.snapshot().get("anthropic").unwrap();
        // Header-based auth: CustomHeader is the truthful mapping (never Bearer).
        assert_eq!(meta.auth_scheme, ProviderAuthScheme::CustomHeader);
        // Exact spelling retained on the descriptor for later consumers.
        assert_eq!(
            svc.get("anthropic").unwrap().auth_scheme.as_deref(),
            Some("x_api_key")
        );
        assert!(
            svc.get("anthropic")
                .unwrap()
                .has_route(ApiSurface::AnthropicMessages, CredentialRoute::ApiKey)
        );
    }

    #[test]
    fn invalid_builder_inputs_error_instead_of_panicking() {
        let mut m = IndexMap::new();
        // Invalid id (uppercase) must error, not panic.
        m.insert(
            "Bad Id".into(),
            ModelProviderConfig {
                kind: ModelProviderKind::OpenAiCompatible,
                ..Default::default()
            },
        );
        assert!(matches!(
            ProviderService::from_model_providers(&m),
            Err(ProviderServiceError::InvalidId { .. })
        ));

        let mut m2 = IndexMap::new();
        // Embedded userinfo base URL must be rejected.
        m2.insert(
            "evil".into(),
            ModelProviderConfig {
                kind: ModelProviderKind::OpenAiCompatible,
                base_url: Some("https://user:pass@evil/v1".into()),
                ..Default::default()
            },
        );
        assert!(matches!(
            ProviderService::from_model_providers(&m2),
            Err(ProviderServiceError::InvalidBaseUrl { .. })
        ));

        // Non-http admin URL.
        let mut m3 = IndexMap::new();
        m3.insert(
            "b".into(),
            ModelProviderConfig {
                kind: ModelProviderKind::OpenAiCompatible,
                admin_base_url: Some("ftp://admin.example".into()),
                ..Default::default()
            },
        );
        assert!(matches!(
            ProviderService::from_model_providers(&m3),
            Err(ProviderServiceError::InvalidAdminBaseUrl { .. })
        ));
    }

    #[test]
    fn additive_fields_parse_and_override_defaults() {
        let (id, cfg) = parse_provider(
            r#"[model_providers.myprov]
            kind = "openai_compatible"
            base_url = "http://127.0.0.1:8128/v1"
            api_surface = "openai_compatible_subset"
            credential_route = "auth_helper"
            unknown_future_field = 42
            "#,
            "myprov",
        );
        let svc = ProviderService::from_model_provider(&id, &cfg, 7).unwrap();
        let d = svc.get("myprov").unwrap();
        assert!(d.has_route(
            ApiSurface::OpenAiCompatibleSubset,
            CredentialRoute::AuthHelper
        ));
        assert_eq!(svc.generation(), 7);
    }

    #[test]
    fn canonical_table_beats_legacy_aliases_when_all_present() {
        // Precedence 1: the canonical `openai` table wins over chatgpt/codex.
        let mut m = IndexMap::new();
        for (id, display, base) in [
            ("openai", "Canonical", "https://canonical.example/v1"),
            ("chatgpt", "ChatGptTable", "https://chatgpt.example/v1"),
            ("codex", "CodexTable", "https://codex.example/v1"),
        ] {
            m.insert(
                id.into(),
                ModelProviderConfig {
                    kind: ModelProviderKind::OpenAi,
                    base_url: Some(base.into()),
                    display_name: Some(display.into()),
                    ..Default::default()
                },
            );
        }
        let svc = ProviderService::from_model_providers(&m).unwrap();
        let d = svc.get("openai").unwrap();
        assert_eq!(d.display_name.as_deref(), Some("Canonical"));
        assert_eq!(d.base_url.as_deref(), Some("https://canonical.example/v1"));
        // Legacy aliases never become descriptors.
        assert!(svc.get("chatgpt").is_none());
        assert!(svc.get("codex").is_none());
        // Explicit source => its single route, one canonical instance.
        assert_eq!(d.routes.len(), 1);
        assert_eq!(
            svc.list()
                .iter()
                .filter(|d| d.id.as_str() == "openai")
                .count(),
            1
        );
    }

    #[test]
    fn first_legacy_alias_wins_when_canonical_absent() {
        // `chatgpt` precedes `codex` in documented order.
        let mut m = IndexMap::new();
        for (id, display, base) in [
            ("chatgpt", "ChatGptTable", "https://chatgpt.example/v1"),
            ("codex", "CodexTable", "https://codex.example/v1"),
        ] {
            m.insert(
                id.into(),
                ModelProviderConfig {
                    kind: ModelProviderKind::OpenAi,
                    base_url: Some(base.into()),
                    display_name: Some(display.into()),
                    ..Default::default()
                },
            );
        }
        let svc = ProviderService::from_model_providers(&m).unwrap();
        let d = svc.get("openai").unwrap();
        assert_eq!(d.display_name.as_deref(), Some("ChatGptTable"));
        assert_eq!(d.base_url.as_deref(), Some("https://chatgpt.example/v1"));
        assert!(matches!(
            d.provider_ref,
            ProviderRef::BuiltIn(BuiltInProviderId::OpenAi)
        ));
        assert!(svc.get("chatgpt").is_none());
        assert!(svc.get("codex").is_none());
    }

    #[test]
    fn legacy_source_retains_complete_config_no_hidden_merge() {
        // A `codex` legacy table is selected whole with its own fields; the
        // canonical preset's backend/env must NOT leak in (no field merging).
        let mut m = IndexMap::new();
        m.insert(
            "codex".into(),
            ModelProviderConfig {
                kind: ModelProviderKind::OpenAi,
                base_url: Some("https://codex.example/v1".into()),
                display_name: Some("CodexTable".into()),
                catalog_enabled: false,
                auth_scheme: Some("bearer".into()),
                ..Default::default()
            },
        );
        let svc = ProviderService::from_model_providers(&m).unwrap();
        let d = svc.get("openai").unwrap();
        assert!(matches!(
            d.provider_ref,
            ProviderRef::BuiltIn(BuiltInProviderId::OpenAi)
        ));
        assert_eq!(d.kind, ProviderKind::OpenAi);
        assert_eq!(d.display_name.as_deref(), Some("CodexTable"));
        // Whole-table fields preserved (no defaults injected from the preset).
        assert_eq!(
            d.api_backend, None,
            "no hidden merge of the preset Responses backend"
        );
        assert_eq!(d.env_keys, Vec::<String>::new());
        assert!(!svc.snapshot().get("openai").unwrap().catalog_enabled);
        // Descriptor and metadata kinds agree.
        assert_eq!(d.kind.as_str(), svc.snapshot().get("openai").unwrap().kind);
    }

    #[test]
    fn internal_preset_used_only_when_no_reserved_source() {
        let mut m = IndexMap::new();
        m.insert("grok_build_openai".into(), grok_build_openai_config());
        let svc = ProviderService::from_model_providers(&m).unwrap();
        let d = svc.get("openai").unwrap();
        // Preset source => canonical composite routes, preset fields retained.
        assert_eq!(d.routes.len(), 2);
        assert_eq!(d.api_backend.as_deref(), Some("responses"));
        // Adding the canonical table switches to whole-table explicit source.
        m.insert(
            "openai".into(),
            ModelProviderConfig {
                kind: ModelProviderKind::OpenAi,
                base_url: Some("https://other.example/v1".into()),
                ..Default::default()
            },
        );
        let svc2 = ProviderService::from_model_providers(&m).unwrap();
        let d2 = svc2.get("openai").unwrap();
        assert_eq!(d2.routes.len(), 1);
        assert_eq!(d2.base_url.as_deref(), Some("https://other.example/v1"));
        assert_eq!(
            d2.api_backend, None,
            "preset backend not merged into the canonical table"
        );
    }

    #[test]
    fn internal_preset_inline_auth_namespaced_by_preset_key() {
        // An existing `grok_build_openai` preset table with an inline auth
        // helper and no reserved source: the synthesized auth-provider ref
        // must use the actual table key, not the canonical `openai` id.
        let mut m = entries(
            r#"[model_providers.grok_build_openai]
            kind = "openai"
            base_url = "https://api.openai.com/v1"
            catalog_enabled = false

            [model_providers.grok_build_openai.auth]
            command = "printf token"
            token_ttl_secs = 3600
            "#,
        );
        let svc = ProviderService::from_model_providers(&m).unwrap();
        let d = svc.get("openai").unwrap();
        assert_eq!(
            d.auth_provider.as_deref(),
            Some("model_provider:grok_build_openai"),
            "inline auth namespace is the actual preset table key"
        );
        // The internal preset never becomes a descriptor; canonical identity
        // stays BuiltIn; whole-table fields preserved (no merge).
        assert!(svc.get("grok_build_openai").is_none());
        assert!(matches!(
            d.provider_ref,
            ProviderRef::BuiltIn(BuiltInProviderId::OpenAi)
        ));
        assert!(!svc.snapshot().get("openai").unwrap().catalog_enabled);
    }

    #[test]
    fn descriptor_and_metadata_kinds_agree() {
        let mut m = IndexMap::new();
        m.insert(
            "custom-gw".into(),
            ModelProviderConfig {
                kind: ModelProviderKind::OpenAiCompatible,
                base_url: Some("http://127.0.0.1:8200/v1".into()),
                ..Default::default()
            },
        );
        m.insert(
            "codex".into(),
            ModelProviderConfig {
                kind: ModelProviderKind::OpenAi,
                base_url: Some("https://codex.example/v1".into()),
                ..Default::default()
            },
        );
        let svc = ProviderService::from_model_providers(&m).unwrap();
        for d in svc.list() {
            let meta = svc.snapshot().get(d.id.as_str()).unwrap();
            assert_eq!(d.kind.as_str(), meta.kind, "kind agreement for `{}`", d.id);
        }
    }

    #[test]
    fn secrets_never_appear_in_descriptor_output() {
        let toml = r#"[model_providers.keyed]
            kind = "custom"
            base_url = "http://127.0.0.1:8201/v1"
            api_key = "sk-super-secret-value"

            [model_providers.keyed.auth]
            command = "printf top-secret-token"
            token_ttl_secs = 3600
        "#;
        let (id, cfg) = parse_provider(toml, "keyed");
        let svc = ProviderService::from_model_provider(&id, &cfg, 0).unwrap();
        let d = svc.get("keyed").unwrap();
        let debug = format!("{d:?}");
        let json = serde_json::to_string(d).unwrap();
        assert!(
            !debug.contains("sk-super-secret-value"),
            "api_key value leaked in Debug"
        );
        assert!(
            !json.contains("sk-super-secret-value"),
            "api_key value leaked in serde"
        );
        assert!(
            !debug.contains("printf top-secret-token"),
            "auth command leaked in Debug"
        );
        assert!(
            !json.contains("printf top-secret-token"),
            "auth command leaked in serde"
        );
        // The synthesized helper ref is carried instead (N-6).
        assert_eq!(d.auth_provider.as_deref(), Some("model_provider:keyed"));
        assert_eq!(d.routes[0].credential_route, CredentialRoute::AuthHelper);
    }

    #[test]
    fn inline_auth_synthesizes_model_provider_ref_and_explicit_wins() {
        let toml = r#"[model_providers.gw]
            kind = "custom"
            base_url = "http://127.0.0.1:8202/v1"

            [model_providers.gw.auth]
            command = "printf token"
            token_ttl_secs = 3600
        "#;
        let (id, cfg) = parse_provider(toml, "gw");
        let svc = ProviderService::from_model_provider(&id, &cfg, 0).unwrap();
        assert_eq!(
            svc.get("gw").unwrap().auth_provider.as_deref(),
            Some("model_provider:gw")
        );

        // Explicit auth_provider wins over inline auth.
        let toml2 = r#"[model_providers.wins]
            kind = "custom"
            base_url = "http://127.0.0.1:8203/v1"
            auth_provider = "corp"

            [model_providers.wins.auth]
            command = "printf other"
        "#;
        let (id2, cfg2) = parse_provider(toml2, "wins");
        let svc2 = ProviderService::from_model_provider(&id2, &cfg2, 0).unwrap();
        assert_eq!(
            svc2.get("wins").unwrap().auth_provider.as_deref(),
            Some("corp")
        );
    }

    #[test]
    fn from_model_provider_is_a_single_route_helper_even_for_reserved_ids() {
        let (id, cfg) = parse_provider(
            r#"[model_providers.openai]
            kind = "openai"
            base_url = "https://api.openai.com/v1"
            "#,
            "openai",
        );
        // Helper: a reserved id keeps its identity but gets exactly one route —
        // documented behavior, not context-dependent.
        let single = ProviderService::from_model_provider(&id, &cfg, 0).unwrap();
        let d = single.get("openai").unwrap();
        assert!(matches!(
            d.provider_ref,
            ProviderRef::BuiltIn(BuiltInProviderId::OpenAi)
        ));
        assert_eq!(d.routes.len(), 1);

        // Canonical composite product routes require `from_model_providers`.
        let composite = ProviderService::default();
        let cd = composite.get("openai").unwrap();
        assert_eq!(cd.routes.len(), 2);
        assert!(cd.has_route(ApiSurface::ChatGptInference, CredentialRoute::ChatGptOauth));
    }
}
