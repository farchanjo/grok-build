use indexmap::IndexMap;

use super::config::{ConfigModelOverride, EnvKeys};
use super::config_model_override_parse::{ConfigWarning, ConfigWarningKind};
use crate::inference::ApiBackend;

/// Internal sampler-config marker used to retain native-agent routing identity
/// after a catalog model is reduced to `InferenceConfig`.
pub(crate) const NATIVE_AGENT_PROVIDER_HEADER: &str = "x-grok-native-agent-provider";

/// The upstream represented by a `[model_providers.<id>]` entry.
///
/// `openai_compatible` is the explicit custom kind. Legacy `kind = "custom"`
/// remains accepted as an alias and deserializes to the same variant. Named
/// kinds let the runtime apply provider-specific policy without inferring
/// identity from a mutable URL.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelProviderKind {
    /// OpenAI-compatible HTTP endpoint (vLLM, SGLang, Z.ai, gateways, …).
    ///
    /// Serialized as `openai_compatible`. Legacy TOML `custom` still loads.
    #[default]
    #[serde(rename = "openai_compatible", alias = "custom")]
    OpenAiCompatible,
    #[serde(rename = "xai")]
    Xai,
    /// OpenAI API key or ChatGPT subscription OAuth (single provider).
    ///
    /// Legacy configs may still say `kind = "codex"`; that deserializes here
    /// and is treated as OpenAI (HTTP Responses), never as an external agent.
    #[serde(rename = "openai", alias = "codex")]
    OpenAi,
    #[serde(rename = "openrouter")]
    OpenRouter,
    /// Direct Anthropic Messages API (`https://api.anthropic.com`).
    ///
    /// Distinct from OpenRouter Claude routes and from custom Messages
    /// backends. Never inherits OpenAI-compatible credential scopes.
    #[serde(rename = "anthropic")]
    Anthropic,
    /// First-class Z.ai Model API profile (`https://api.z.ai/api/paas/v4`).
    #[serde(rename = "zai")]
    Zai,
}

impl ModelProviderKind {
    /// Whether this kind is treated as an OpenAI-compatible HTTP backend for
    /// credential scopes, discovery, and the platform client.
    pub const fn is_openai_compatible_family(self) -> bool {
        matches!(
            self,
            Self::OpenAiCompatible | Self::OpenAi | Self::OpenRouter | Self::Zai
        )
    }

    pub const fn as_config_str(self) -> &'static str {
        match self {
            Self::OpenAiCompatible => "openai_compatible",
            Self::Xai => "xai",
            Self::OpenAi => "openai",
            Self::OpenRouter => "openrouter",
            Self::Anthropic => "anthropic",
            Self::Zai => "zai",
        }
    }
}

/// Sidecar [`crate::provider_registry::ProviderKind`] converts from the
/// existing sampler-side [`ModelProviderKind`] without replacing it. Both
/// kinds stay valid for their respective consumers.
impl From<ModelProviderKind> for crate::provider_registry::ProviderKind {
    fn from(kind: ModelProviderKind) -> Self {
        use crate::provider_registry::ProviderKind;
        match kind {
            ModelProviderKind::OpenAiCompatible => ProviderKind::OpenAiCompatible,
            ModelProviderKind::Xai => ProviderKind::Xai,
            ModelProviderKind::OpenAi => ProviderKind::OpenAi,
            ModelProviderKind::OpenRouter => ProviderKind::OpenRouter,
            ModelProviderKind::Anthropic => ProviderKind::Anthropic,
            ModelProviderKind::Zai => ProviderKind::Zai,
        }
    }
}

// Re-exported from the sampler crate so the shell TOML layer and the sampler
// wire layer share one definition (matching the `ProviderIdentity` precedent).
pub use xai_grok_inference::{OpenRouterMaxPrice, OpenRouterPlugin, OpenRouterProviderPreferences};

/// Direct Anthropic Messages base used by the built-in Anthropic preset
/// (`…/v1` + `/messages`). Shared between the provider CLI/status preset
/// installation and the provider service so neither specializes or drifts.
pub(crate) const ANTHROPIC_INFERENCE_BASE_URL: &str = "https://api.anthropic.com/v1";

/// Canonical `grok_build_openai` preset config. This is the single source of
/// truth for the built-in OpenAI model-provider entry; the provider service
/// reuses it as the preset source for the canonical `openai` product
/// descriptor rather than hand-rolling a divergent default.
pub(crate) fn grok_build_openai_config() -> ModelProviderConfig {
    ModelProviderConfig {
        kind: ModelProviderKind::OpenAi,
        base_url: Some("https://api.openai.com/v1".into()),
        api_backend: Some(ApiBackend::Responses),
        ..Default::default()
    }
}

/// Canonical `grok_build_openrouter` preset config (shared). Reused by the
/// provider service as the canonical `openrouter` descriptor preset source.
pub(crate) fn grok_build_openrouter_config() -> ModelProviderConfig {
    let mut extra_headers = indexmap::IndexMap::<String, String>::new();
    extra_headers
        .entry("X-OpenRouter-Title".to_owned())
        .or_insert("Grok Build".to_owned());
    ModelProviderConfig {
        kind: ModelProviderKind::OpenRouter,
        base_url: Some("https://openrouter.ai/api/v1".into()),
        api_backend: Some(ApiBackend::ChatCompletions),
        provider_preferences: Some(OpenRouterProviderPreferences {
            data_collection: Some("deny".to_owned()),
            require_parameters: Some(true),
            ..Default::default()
        }),
        extra_headers,
        ..Default::default()
    }
}

/// Canonical `grok_build_anthropic` preset config (shared). Reused by the
/// provider service as the canonical `anthropic` descriptor preset source.
pub(crate) fn grok_build_anthropic_config() -> ModelProviderConfig {
    let mut extra_headers = indexmap::IndexMap::<String, String>::new();
    extra_headers.insert(
        "anthropic-version".to_owned(),
        xai_grok_inference::ANTHROPIC_VERSION.to_owned(),
    );
    ModelProviderConfig {
        kind: ModelProviderKind::Anthropic,
        base_url: Some(ANTHROPIC_INFERENCE_BASE_URL.to_owned()),
        // snake_case AuthScheme wire form (`x_api_key`).
        auth_scheme: Some("x_api_key".to_owned()),
        api_backend: Some(ApiBackend::Messages),
        extra_headers,
        ..Default::default()
    }
}

/// Provider identity retained on a resolved [`super::config::ModelEntry`].
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ResolvedModelProvider {
    pub id: String,
    pub kind: ModelProviderKind,
    /// OpenRouter model fallbacks inherited from the provider or overridden
    /// by a single model. These are retained here rather than inferred from a
    /// URL, so only an explicit OpenRouter provider can emit the extension.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub openrouter_fallback_models: Vec<String>,
    /// OpenRouter native `provider` request-body preferences. Model-level
    /// override replaces the provider-level object for that model; absent
    /// override inherits the provider-level object. Only populated for
    /// `kind = "openrouter"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openrouter_provider_preferences: Option<OpenRouterProviderPreferences>,
    /// OpenRouter native `plugins` request-body array. Model-level override
    /// replaces the provider-level list for that model; absent override
    /// inherits the provider-level list. Only populated for
    /// `kind = "openrouter"`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub openrouter_plugins: Vec<OpenRouterPlugin>,
    /// Explicit OpenRouter request-pacing opt-in for OpenRouter-compatible
    /// proxies that keep a non-`openrouter` provider identity. Native
    /// `kind = "openrouter"` always paces regardless of this flag. Model-level
    /// override replaces the provider-level value; absent override inherits.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub openrouter_pacing: bool,
    /// Provider-wide request `max_tokens` used when a model does not set
    /// [`super::config::ConfigModelOverride::max_completion_tokens`].
    /// OpenRouter (`kind = "openrouter"`) treats an unset value as
    /// [`OPENROUTER_DEFAULT_MAX_COMPLETION_TOKENS`]. Other kinds leave the
    /// request unset unless this is explicit. Never copied onto
    /// `ModelInfo.max_completion_tokens`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u32>,
    /// Unused; retained for forward-compatible TOML round-trips only.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub command: Vec<String>,
}

/// Default OpenRouter request `max_tokens` when the model, provider, and
/// catalog ceiling are all unset. When the catalog advertises a ceiling, that
/// value is used instead (and still clamped).
///
/// Every sampler path that resolves a request max inherits this one value,
/// including compaction: a compaction call is built from the resolved route
/// config, so it never needs its own budget constant.
pub const OPENROUTER_DEFAULT_MAX_COMPLETION_TOKENS: u32 = 16_384;

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(default)]
pub struct ModelProviderConfig {
    pub kind: ModelProviderKind,
    /// Optional additive sidecar request-surface override. Absent => the
    /// backward-compatible default derived from `kind`. Never affects current
    /// inference routing; reserved for the multi-account platform.
    pub api_surface: Option<crate::provider_registry::ApiSurface>,
    /// Optional additive sidecar credential-route override. Absent => the
    /// backward-compatible default derived from `kind` and credential setup.
    /// Never affects current credential resolution; reserved for the future.
    pub credential_route: Option<crate::provider_registry::CredentialRoute>,
    /// Optional human-readable label for `/providers` and CLI output.
    pub display_name: Option<String>,
    pub base_url: Option<String>,
    pub api_base_url: Option<String>,
    /// Optional separate administration base URL.
    pub admin_base_url: Option<String>,
    /// When false, provider is retained in config but excluded from catalogs.
    pub enabled: bool,
    /// Default agent backend (`chat_completions` / `responses`).
    pub default_backend: Option<String>,
    /// Auth scheme: `bearer` (default), `none`, or `custom_header`.
    pub auth_scheme: Option<String>,
    pub env_key: Option<EnvKeys>,
    /// Environment variable name for an administration credential (never auto-persisted).
    pub admin_env_key: Option<String>,
    pub api_key: Option<String>,
    pub api_backend: Option<ApiBackend>,
    /// Discover models via authenticated `GET …/models` for this provider.
    pub catalog_enabled: bool,
    /// Capability discovery mode: `auto`, `manual`, or `off`.
    pub capability_mode: Option<String>,
    pub catalog_ttl_secs: Option<u64>,
    pub request_timeout_secs: Option<u64>,
    pub organization: Option<String>,
    pub project: Option<String>,
    /// Explicit capability overrides (`[model_providers.<id>.capabilities]`).
    #[serde(default)]
    pub capabilities: IndexMap<String, bool>,
    /// OpenRouter models to try after a model's primary `model` slug.
    /// Ignored unless `kind = "openrouter"`.
    #[serde(default)]
    pub openrouter_fallback_models: Vec<String>,
    /// OpenRouter native `provider` request-body preferences. Ignored unless
    /// `kind = "openrouter"`. A model-level override replaces this object for
    /// that model.
    #[serde(default)]
    pub provider_preferences: Option<OpenRouterProviderPreferences>,
    /// OpenRouter native `plugins` request-body array. Ignored unless
    /// `kind = "openrouter"`. A model-level override replaces this list for
    /// that model.
    #[serde(default)]
    pub plugins: Vec<OpenRouterPlugin>,
    /// Opt into OpenRouter request pacing for OpenRouter-compatible proxies
    /// that use a non-`openrouter` `kind`. Native OpenRouter identity always
    /// paces; hostname `openrouter.ai` remains a legacy fallback. A model-level
    /// override replaces this value for that model.
    #[serde(default)]
    pub openrouter_pacing: bool,
    /// Provider-wide default request `max_tokens`. Absent OpenRouter
    /// instances use [`OPENROUTER_DEFAULT_MAX_COMPLETION_TOKENS`].
    #[serde(default)]
    pub max_completion_tokens: Option<u32>,
    pub extra_headers: IndexMap<String, String>,
    pub auth_provider: Option<String>,
    pub auth: Option<crate::auth::AuthProviderConfig>,
    pub context_window: Option<u64>,
    /// Unused legacy field (previously Codex app-server command override).
    pub command: Vec<String>,
}

impl Default for ModelProviderConfig {
    fn default() -> Self {
        Self {
            kind: ModelProviderKind::default(),
            api_surface: None,
            credential_route: None,
            display_name: None,
            base_url: None,
            api_base_url: None,
            admin_base_url: None,
            enabled: true,
            default_backend: None,
            auth_scheme: None,
            env_key: None,
            admin_env_key: None,
            api_key: None,
            api_backend: None,
            catalog_enabled: true,
            capability_mode: None,
            catalog_ttl_secs: None,
            request_timeout_secs: None,
            organization: None,
            project: None,
            capabilities: IndexMap::new(),
            openrouter_fallback_models: Vec::new(),
            provider_preferences: None,
            plugins: Vec::new(),
            openrouter_pacing: false,
            max_completion_tokens: None,
            extra_headers: IndexMap::new(),
            auth_provider: None,
            auth: None,
            context_window: None,
            command: Vec::new(),
        }
    }
}

impl ModelProviderConfig {
    fn resolved(
        &self,
        id: &str,
        openrouter_fallback_models: Vec<String>,
        provider_preferences: Option<OpenRouterProviderPreferences>,
        plugins: Vec<OpenRouterPlugin>,
        openrouter_pacing: bool,
    ) -> ResolvedModelProvider {
        ResolvedModelProvider {
            id: id.to_string(),
            kind: self.kind,
            openrouter_fallback_models: if self.kind == ModelProviderKind::OpenRouter {
                openrouter_fallback_models
            } else {
                Vec::new()
            },
            openrouter_provider_preferences: if self.kind == ModelProviderKind::OpenRouter {
                provider_preferences
            } else {
                None
            },
            openrouter_plugins: if self.kind == ModelProviderKind::OpenRouter {
                plugins
            } else {
                Vec::new()
            },
            // Pacing opt-in is intentionally not identity-gated: proxies that
            // keep `kind = "custom"` (or another non-openrouter kind) still
            // need an explicit way to enable spacing without claiming native
            // OpenRouter request extensions.
            openrouter_pacing,
            max_completion_tokens: self.max_completion_tokens.filter(|&n| n > 0),
            command: self.command.clone(),
        }
    }
}

impl ResolvedModelProvider {
    /// Request budget when the model row has no `max_completion_tokens`.
    ///
    /// Order: explicit provider TOML > OpenRouter catalog ceiling >
    /// [`OPENROUTER_DEFAULT_MAX_COMPLETION_TOKENS`] (16384) for
    /// `kind = "openrouter"`. Other kinds stay unset unless the provider set
    /// an explicit positive value. The catalog ceiling is a capability cap
    /// and a fill-in; it is never stored on `ModelInfo.max_completion_tokens`.
    pub fn request_max_completion_tokens(&self, catalog_ceiling: Option<u32>) -> Option<u32> {
        self.max_completion_tokens
            .filter(|&n| n > 0)
            .or_else(|| catalog_ceiling.filter(|&n| n > 0))
            .or_else(|| {
                (self.kind == ModelProviderKind::OpenRouter)
                    .then_some(OPENROUTER_DEFAULT_MAX_COMPLETION_TOKENS)
            })
    }
}

pub(crate) fn model_provider_auth_name(provider_id: &str) -> String {
    format!("model_provider:{provider_id}")
}

pub(crate) fn auth_config_issues(
    config: &crate::auth::AuthProviderConfig,
) -> Vec<(&'static str, ConfigWarningKind, String)> {
    let mut issues = Vec::new();
    if !config.is_usable() {
        issues.push((
            "command",
            ConfigWarningKind::InvalidValue,
            "missing or empty command; models resolve with no credential".to_owned(),
        ));
    }
    let skew = crate::auth::PROVIDER_TOKEN_EXPIRY_SKEW_SECS;
    if config.token_ttl_secs.is_some_and(|ttl| ttl <= skew) {
        issues.push((
            "token_ttl_secs",
            ConfigWarningKind::InvalidValue,
            format!(
                "at or below the {skew}s refresh margin; the command will run before every turn"
            ),
        ));
    }
    if let Some(timeout) = config.timeout_secs
        && !(1..=crate::auth::PROVIDER_TIMEOUT_CEILING_SECS).contains(&timeout)
    {
        let ceiling = crate::auth::PROVIDER_TIMEOUT_CEILING_SECS;
        issues.push((
            "timeout_secs",
            ConfigWarningKind::InvalidValue,
            if timeout == 0 {
                "below the 1 second minimum; clamped to 1".to_owned()
            } else {
                format!("above the {ceiling}s maximum; clamped to {ceiling}")
            },
        ));
    }
    issues
}

pub(crate) fn parse_model_providers(
    raw_config: &toml::Value,
) -> (IndexMap<String, ModelProviderConfig>, Vec<ConfigWarning>) {
    let mut providers = IndexMap::new();
    let mut warnings = Vec::new();
    let Some(section) = raw_config.get("model_providers") else {
        return (providers, warnings);
    };
    let Some(table) = section.as_table() else {
        warnings.push(ConfigWarning::model_provider_section(
            ConfigWarningKind::NotATable,
            format!(
                "`model_providers` must be a table of [model_providers.<id>] entries, got {}; \
                 all model providers ignored",
                section.type_str()
            ),
        ));
        return (providers, warnings);
    };
    for (id, value) in table {
        let mut unknown = Vec::new();
        // Lenient additive parsing: an invalid `api_surface` / `credential_route`
        // value must warn+prune (field -> None/default) rather than drop the
        // otherwise valid provider. Valid typed values remain stored.
        let pruned = prune_invalid_sidecar_fields(id, value, &mut warnings);
        match serde_ignored::deserialize::<_, _, ModelProviderConfig>(pruned, |path| {
            unknown.push(path.to_string());
        }) {
            Ok(provider) => {
                for key in unknown {
                    warnings.push(ConfigWarning::model_provider(
                        id,
                        Some(key.as_str()),
                        ConfigWarningKind::UnknownField,
                        "unrecognized key; field ignored".to_owned(),
                    ));
                }
                if let Some(auth) = &provider.auth {
                    for (field, kind, reason) in auth_config_issues(auth) {
                        warnings.push(ConfigWarning::model_provider(
                            id,
                            Some(&format!("auth.{field}")),
                            kind,
                            reason,
                        ));
                    }
                }
                let has_helper = provider.auth.is_some() || provider.auth_provider.is_some();
                let has_static_api_key = provider
                    .api_key
                    .as_deref()
                    .map(str::trim)
                    .is_some_and(|k| !k.is_empty());
                // Validate provider id slug for configured instances.
                if let Err(err) = crate::provider_registry::validate_provider_id_str(id) {
                    warnings.push(ConfigWarning::model_provider(
                        id,
                        None,
                        ConfigWarningKind::InvalidValue,
                        format!("invalid provider id ({err}); provider skipped"),
                    ));
                    continue;
                }
                if let Some(url) = provider
                    .base_url
                    .as_deref()
                    .or(provider.api_base_url.as_deref())
                    && let Err(err) =
                        crate::provider_registry::lifecycle::validate_http_base_url(url)
                {
                    warnings.push(ConfigWarning::model_provider(
                        id,
                        Some("base_url"),
                        ConfigWarningKind::InvalidValue,
                        format!("{err}; provider skipped"),
                    ));
                    continue;
                }
                if let Some(url) = provider.admin_base_url.as_deref()
                    && let Err(err) =
                        crate::provider_registry::lifecycle::validate_http_base_url(url)
                {
                    warnings.push(ConfigWarning::model_provider(
                        id,
                        Some("admin_base_url"),
                        ConfigWarningKind::InvalidValue,
                        format!("{err}; provider skipped"),
                    ));
                    continue;
                }
                if let Err(err) = crate::provider_registry::lifecycle::validate_extra_headers(
                    &provider.extra_headers,
                ) {
                    warnings.push(ConfigWarning::model_provider(
                        id,
                        Some("extra_headers"),
                        ConfigWarningKind::InvalidValue,
                        format!("{err}; restricted headers ignored at runtime"),
                    ));
                }
                if has_helper && has_static_api_key {
                    warnings.push(ConfigWarning::model_provider(
                        id,
                        Some("api_key"),
                        ConfigWarningKind::ConflictingFields,
                        "api_key shadows this provider's auth helper; the static key always \
                         takes precedence, so the helper never runs for inheriting models"
                            .to_owned(),
                    ));
                } else if has_helper
                    && provider
                        .env_key
                        .as_ref()
                        .and_then(EnvKeys::primary)
                        .is_some()
                {
                    warnings.push(ConfigWarning::model_provider(
                        id,
                        Some("env_key"),
                        ConfigWarningKind::ConflictingFields,
                        "env_key may shadow this provider's auth helper; env_key takes precedence \
                         when its variable resolves, otherwise the helper runs"
                            .to_owned(),
                    ));
                }
                if provider.auth_provider.is_some() && provider.auth.is_some() {
                    warnings.push(ConfigWarning::model_provider(
                        id,
                        Some("auth"),
                        ConfigWarningKind::ConflictingFields,
                        "inline auth is shadowed by auth_provider on this provider; the referenced \
                         provider takes precedence, so the inline helper never runs"
                            .to_owned(),
                    ));
                }
                providers.insert(id.clone(), provider);
            }
            Err(error) => {
                warnings.push(ConfigWarning::model_provider(
                    id,
                    None,
                    ConfigWarningKind::InvalidValue,
                    format!(
                        "failed to parse ({error}); provider skipped, inheriting models \
                         resolve with defaults"
                    ),
                ));
            }
        }
    }
    (providers, warnings)
}

/// Prune an invalid additive sidecar field (`api_surface` / `credential_route`)
/// from an otherwise valid `[model_providers.<id>]` entry so the provider is
/// never dropped. The recognized field is validated in isolation: a value that
/// does not deserialize to the canonical sidecar type is removed (defaulting to
/// the backward-compatible value) and a structured `InvalidValue` warning is
/// pushed. A valid typed value is left in place and stored by the struct.
fn prune_invalid_sidecar_fields(
    id: &str,
    value: &toml::Value,
    warnings: &mut Vec<ConfigWarning>,
) -> toml::Value {
    let Some(table) = value.as_table() else {
        return value.clone();
    };
    let sidecar_ok = |key: &str| -> bool {
        let Some(field) = table.get(key) else {
            return true;
        };
        match key {
            "api_surface" => field
                .clone()
                .try_into::<crate::provider_registry::ApiSurface>()
                .is_ok(),
            "credential_route" => field
                .clone()
                .try_into::<crate::provider_registry::CredentialRoute>()
                .is_ok(),
            _ => true,
        }
    };
    let surface_ok = sidecar_ok("api_surface");
    let route_ok = sidecar_ok("credential_route");
    if surface_ok && route_ok {
        return value.clone();
    }
    let mut out = value.clone();
    if let Some(t) = out.as_table_mut() {
        if !surface_ok {
            t.remove("api_surface");
            warnings.push(ConfigWarning::model_provider(
                id,
                Some("api_surface"),
                ConfigWarningKind::InvalidValue,
                "unrecognized api_surface value; ignored and the backward-compatible \
                 default applies"
                    .to_owned(),
            ));
        }
        if !route_ok {
            t.remove("credential_route");
            warnings.push(ConfigWarning::model_provider(
                id,
                Some("credential_route"),
                ConfigWarningKind::InvalidValue,
                "unrecognized credential_route value; ignored and the backward-compatible \
                 default applies"
                    .to_owned(),
            ));
        }
    }
    out
}

impl ConfigModelOverride {
    pub(crate) fn with_provider_defaults(
        &self,
        provider: &ModelProviderConfig,
        provider_id: &str,
    ) -> Self {
        let ModelProviderConfig {
            kind: _,
            api_surface: _,
            credential_route: _,
            display_name,
            base_url,
            api_base_url,
            admin_base_url: _,
            enabled: _,
            default_backend: _,
            auth_scheme,
            env_key,
            admin_env_key: _,
            api_key,
            api_backend,
            catalog_enabled: _,
            capability_mode: _,
            catalog_ttl_secs: _,
            request_timeout_secs: _,
            organization: _,
            project: _,
            capabilities: _,
            openrouter_fallback_models,
            provider_preferences,
            plugins,
            openrouter_pacing,
            max_completion_tokens: _,
            extra_headers,
            auth_provider,
            auth,
            context_window,
            command: _,
        } = provider;

        let mut merged = self.clone();
        let effective_openrouter_fallback_models = self
            .openrouter_fallback_models
            .clone()
            .unwrap_or_else(|| openrouter_fallback_models.clone());
        // Model-level preferences replace the provider-level object for that
        // model; absent override inherits the provider-level object. Mirrors
        // the `openrouter_fallback_models` inheritance contract.
        let effective_provider_preferences = self
            .provider_preferences
            .clone()
            .or_else(|| provider_preferences.clone());
        // Model-level plugins replace the provider-level list for that model;
        // absent override inherits the provider-level list. Mirrors the
        // `provider_preferences` inheritance contract.
        let effective_plugins = self
            .plugins
            .clone()
            .or_else(|| Some(plugins.clone()))
            .unwrap_or_default();
        // Model-level openrouter_pacing replaces the provider-level flag;
        // absent override inherits the provider-level value.
        let effective_openrouter_pacing = self.openrouter_pacing.unwrap_or(*openrouter_pacing);
        merged.resolved_model_provider = Some(provider.resolved(
            provider_id,
            effective_openrouter_fallback_models,
            effective_provider_preferences,
            effective_plugins,
            effective_openrouter_pacing,
        ));
        if merged.provider_display_name.is_none() {
            merged.provider_display_name = display_name.clone();
        }
        merged.base_url = merged.base_url.or_else(|| base_url.clone());
        merged.api_base_url = merged.api_base_url.or_else(|| api_base_url.clone());
        merged.api_backend = merged.api_backend.or_else(|| api_backend.clone());
        if merged.auth_scheme.is_none() {
            merged.auth_scheme = auth_scheme.as_deref().and_then(|raw| match raw {
                "x_api_key" | "x-api-key" | "xapikey" => {
                    Some(xai_grok_inference::AuthScheme::XApiKey)
                }
                "bearer" => Some(xai_grok_inference::AuthScheme::Bearer),
                _ => None,
            });
        }
        merged.context_window = merged.context_window.or(*context_window);
        if !extra_headers.is_empty() {
            let mut headers = extra_headers.clone();
            headers.extend(merged.extra_headers);
            merged.extra_headers = headers;
        }
        let model_sets_own_api_key = self
            .api_key
            .as_deref()
            .is_some_and(|k| !k.trim().is_empty());
        let model_sets_own_env_key = self.env_key.as_ref().and_then(EnvKeys::primary).is_some();
        let model_has_own_auth =
            model_sets_own_api_key || model_sets_own_env_key || self.auth_provider.is_some();
        if !model_has_own_auth {
            // The provider-scoped vault is deliberately resolved per turn in
            // `resolve_credentials`, not copied here. That way a key saved or
            // removed by the separate TUI process takes effect immediately.
            merged.api_key = api_key.clone();
            merged.env_key = env_key.clone();
            merged.auth_provider = auth_provider
                .clone()
                .or_else(|| auth.as_ref().map(|_| model_provider_auth_name(provider_id)));
        }
        merged
    }

    pub(crate) fn with_missing_provider(&self) -> Self {
        let mut merged = self.clone();
        merged.resolved_model_provider = None;
        merged
    }
}

#[cfg(test)]
mod tests {
    use crate::agent::config::{
        Config, inference_config_for_model, resolve_credentials, resolve_model_list,
    };
    #[test]
    fn model_inherits_provider_connection_defaults() {
        let raw_config: toml::Value = toml::from_str(
            r#"
            [model_providers.gateway]
            base_url = "https://gateway.example/v1"
            context_window = 123456

            [model_providers.gateway.extra_headers]
            X-Corp = "yes"

            [model.via-gateway]
            model = "m"
            model_provider = "gateway"
            "#,
        )
        .unwrap();

        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        assert!(cfg.model_providers.contains_key("gateway"));
        let resolved = resolve_model_list(&cfg, None);
        let model = resolved.get("via-gateway").expect("model should exist");
        assert_eq!(model.info.base_url, "https://gateway.example/v1");
        assert_eq!(model.info.context_window.get(), 123456);
        assert_eq!(
            model.info.extra_headers.get("X-Corp").map(String::as_str),
            Some("yes")
        );
        let provider = model
            .model_provider
            .as_ref()
            .expect("provider identity survives default expansion");
        assert_eq!(provider.id, "gateway");
        assert_eq!(provider.kind, super::ModelProviderKind::OpenAiCompatible);
        assert!(
            model.has_own_credentials(),
            "a custom endpoint without a credential is BYOK, not session-authed"
        );
        assert_eq!(
            resolve_credentials(model, Some("session-jwt")).api_key,
            None,
            "the session token must not leak to the provider's custom endpoint"
        );
    }

    #[test]
    fn model_fields_override_provider_defaults() {
        let raw_config: toml::Value = toml::from_str(
            r#"
            [model_providers.gateway]
            base_url = "https://gateway.example/v1"
            context_window = 100000

            [model.override-url]
            model = "m"
            model_provider = "gateway"
            base_url = "https://model-specific.example/v1"
            context_window = 200000
            "#,
        )
        .unwrap();

        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        let resolved = resolve_model_list(&cfg, None);
        let model = resolved.get("override-url").expect("model should exist");
        assert_eq!(model.info.base_url, "https://model-specific.example/v1");
        assert_eq!(model.info.context_window.get(), 200000);
    }

    #[test]
    fn model_provider_inline_auth_registers_synthetic_provider() {
        let raw_config: toml::Value = toml::from_str(
            r#"
            [model_providers.gateway]
            base_url = "https://gateway.example/v1"
            context_window = 200000

            [model_providers.gateway.auth]
            command = "printf gw-token"
            token_ttl_secs = 3600

            [model.byok-via-gateway]
            model = "m"
            model_provider = "gateway"
            "#,
        )
        .unwrap();

        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        assert_eq!(
            cfg.auth_providers
                .get("model_provider:gateway")
                .map(|c| c.command.as_str()),
            Some("printf gw-token"),
            "inline auth registers a synthetic provider keyed by the id"
        );
        let resolved = resolve_model_list(&cfg, None);
        let model = resolved
            .get("byok-via-gateway")
            .expect("model should exist");
        let provider = model
            .auth_provider
            .as_ref()
            .expect("the model inherits the provider's auth");
        assert_eq!(provider.name, "model_provider:gateway");
        assert_eq!(provider.config.command, "printf gw-token");
        assert!(
            model.has_own_credentials(),
            "a provider-backed model is BYOK (session token must not leak)"
        );
    }

    #[test]
    fn model_with_own_key_ignores_provider_auth() {
        let raw_config: toml::Value = toml::from_str(
            r#"
            [model_providers.gateway]
            base_url = "https://gateway.example/v1"
            context_window = 200000

            [model_providers.gateway.auth]
            command = "printf gw-token"

            [model.own-key]
            model = "m"
            model_provider = "gateway"
            api_key = "sk-model-own"
            "#,
        )
        .unwrap();

        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        let resolved = resolve_model_list(&cfg, None);
        let model = resolved.get("own-key").expect("model should exist");
        assert_eq!(
            model.info.base_url, "https://gateway.example/v1",
            "non-auth connection fields are still inherited"
        );
        assert_eq!(
            model.effective_auth_provider().map(|p| p.name.as_str()),
            None,
            "the model's own key shadows the provider's auth"
        );
        let creds = resolve_credentials(model, Some("session-jwt"));
        assert_eq!(creds.api_key.as_deref(), Some("sk-model-own"));
    }

    #[test]
    fn undefined_model_provider_fails_closed() {
        use super::super::config_model_override_parse::{ConfigWarningKind, WarningTarget};

        let raw_config: toml::Value = toml::from_str(
            r#"
            [model.dangling]
            model = "m"
            base_url = "https://third-party.example/v1"
            context_window = 200000
            model_provider = "ghost"
            "#,
        )
        .unwrap();

        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        assert!(
            cfg.config_warnings.iter().any(|w| {
                w.kind == ConfigWarningKind::InvalidValue
                    && matches!(
                        &w.target,
                        WarningTarget::Model { field, .. }
                            if field.as_deref() == Some("model_provider")
                    )
            }),
            "an undefined provider reference warns: {:?}",
            cfg.config_warnings
        );
        let resolved = resolve_model_list(&cfg, None);
        let model = resolved.get("dangling").expect("model should exist");
        assert_eq!(
            model.info.base_url, "https://third-party.example/v1",
            "the model keeps its own connection fields"
        );
        assert!(
            model.has_own_credentials(),
            "an undefined provider leaves the model BYOK, not session-authed"
        );
        let creds = resolve_credentials(model, Some("session-jwt"));
        assert_eq!(
            creds.api_key, None,
            "no credential resolves and the session token does not leak to the model's base_url"
        );
    }

    #[test]
    fn undefined_model_provider_keeps_model_own_key() {
        let raw_config: toml::Value = toml::from_str(
            r#"
            [model.own-key]
            model = "m"
            base_url = "https://third-party.example/v1"
            context_window = 200000
            api_key = "sk-model-own"
            model_provider = "ghost"
            "#,
        )
        .unwrap();

        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        let resolved = resolve_model_list(&cfg, None);
        let model = resolved.get("own-key").expect("model should exist");
        let creds = resolve_credentials(model, Some("session-jwt"));
        assert_eq!(creds.api_key.as_deref(), Some("sk-model-own"));
    }

    #[test]
    fn model_provider_parse_warnings_are_lenient_and_specific() {
        use super::super::config_model_override_parse::{ConfigWarningKind, WarningTarget};

        let raw_config: toml::Value = toml::from_str(
            r#"
            [model_providers.good]
            base_url = "https://good.example/v1"

            [model_providers.bad-type]
            context_window = "not-a-number"

            [model_providers.typo]
            base_url = "https://typo.example/v1"
            unknown_field = 5

            [model.on-broken-provider]
            model = "m"
            base_url = "https://x.example/v1"
            context_window = 200000
            model_provider = "bad-type"
            "#,
        )
        .unwrap();

        let cfg = Config::new_from_toml_cfg(&raw_config)
            .expect("one bad provider must not fail the config");
        assert!(cfg.model_providers.contains_key("good"));
        assert!(
            !cfg.model_providers.contains_key("bad-type"),
            "a malformed provider is skipped"
        );

        let has_provider = |id: &str, field: Option<&str>, kind: ConfigWarningKind| {
            cfg.config_warnings.iter().any(|w| {
                w.kind == kind
                    && matches!(
                        &w.target,
                        WarningTarget::ModelProvider { id: i, field: f }
                            if i == id && f.as_deref() == field
                    )
            })
        };
        assert!(has_provider(
            "bad-type",
            None,
            ConfigWarningKind::InvalidValue
        ));
        assert!(has_provider(
            "typo",
            Some("unknown_field"),
            ConfigWarningKind::UnknownField
        ));
        assert!(
            !cfg.config_warnings.iter().any(|w| {
                matches!(
                    &w.target,
                    WarningTarget::Model { field, .. }
                        if field.as_deref() == Some("model_provider")
                )
            }),
            "a declared-but-malformed provider must not also warn as undefined: {:?}",
            cfg.config_warnings
        );

        let raw_config: toml::Value = toml::from_str(r#"model_providers = "oops""#).unwrap();
        let cfg = Config::new_from_toml_cfg(&raw_config)
            .expect("a non-table model_providers must not fail the config");
        assert!(cfg.model_providers.is_empty());
        assert!(
            cfg.config_warnings.iter().any(|w| {
                matches!(w.target, WarningTarget::ModelProviderSection)
                    && w.kind == ConfigWarningKind::NotATable
            }),
            "non-table section warns: {:?}",
            cfg.config_warnings
        );
    }

    #[test]
    fn invalid_additive_sidecar_fields_warn_and_prune_not_drop() {
        use super::super::config_model_override_parse::{ConfigWarningKind, WarningTarget};

        // Invalid recognized sidecar string values.
        let raw_config: toml::Value = toml::from_str(
            r#"
            [model_providers.bad]
            base_url = "https://bad.example/v1"
            api_surface = "bogus_surface"
            credential_route = "not-a-route"
            "#,
        )
        .unwrap();
        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        assert!(
            cfg.model_providers.contains_key("bad"),
            "an invalid sidecar value must not drop the provider"
        );
        let provider = &cfg.model_providers["bad"];
        assert!(
            provider.api_surface.is_none(),
            "invalid api_surface pruned to None"
        );
        assert!(
            provider.credential_route.is_none(),
            "invalid credential_route pruned to None"
        );
        let has_field = |field: &str| {
            cfg.config_warnings.iter().any(|w| {
                w.kind == ConfigWarningKind::InvalidValue
                    && matches!(
                        &w.target,
                        WarningTarget::ModelProvider { id, field: f }
                            if id == "bad" && f.as_deref() == Some(field)
                    )
            })
        };
        assert!(
            has_field("api_surface"),
            "api_surface warns: {:?}",
            cfg.config_warnings
        );
        assert!(
            has_field("credential_route"),
            "credential_route warns: {:?}",
            cfg.config_warnings
        );

        // Wrong type also warns and prunes.
        let raw_type: toml::Value = toml::from_str(
            r#"
            [model_providers.t]
            base_url = "https://t.example/v1"
            api_surface = 42
            "#,
        )
        .unwrap();
        let cfg2 = Config::new_from_toml_cfg(&raw_type).expect("config should parse");
        assert!(
            cfg2.model_providers.contains_key("t"),
            "wrong-type provider retained"
        );
        assert!(cfg2.model_providers["t"].api_surface.is_none());
        assert!(
            cfg2.config_warnings.iter().any(|w| {
                w.kind == ConfigWarningKind::InvalidValue
                    && matches!(
                        &w.target,
                        WarningTarget::ModelProvider { id, field: f }
                            if id == "t" && f.as_deref() == Some("api_surface")
                    )
            }),
            "wrong-type api_surface warns: {:?}",
            cfg2.config_warnings
        );
    }

    #[test]
    fn model_provider_conflicting_credentials_warn() {
        use super::super::config_model_override_parse::{ConfigWarningKind, WarningTarget};

        let raw_config: toml::Value = toml::from_str(
            r#"
            [model_providers.static-shadows]
            base_url = "https://a.example/v1"
            api_key = "sk-static"
            [model_providers.static-shadows.auth]
            command = "printf tok"

            [model_providers.env-shadows]
            base_url = "https://b.example/v1"
            env_key = "SOME_VAR"
            [model_providers.env-shadows.auth]
            command = "printf tok"

            [model_providers.two-helpers]
            base_url = "https://c.example/v1"
            auth_provider = "corp"
            [model_providers.two-helpers.auth]
            command = "printf tok"
            "#,
        )
        .unwrap();

        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        let has = |id: &str, field: &str| {
            cfg.config_warnings.iter().any(|w| {
                w.kind == ConfigWarningKind::ConflictingFields
                    && matches!(
                        &w.target,
                        WarningTarget::ModelProvider { id: i, field: f }
                            if i == id && f.as_deref() == Some(field)
                    )
            })
        };
        assert!(
            has("static-shadows", "api_key"),
            "a static api_key shadows the helper: {:?}",
            cfg.config_warnings
        );
        assert!(
            has("env-shadows", "env_key"),
            "an env_key may shadow the helper: {:?}",
            cfg.config_warnings
        );
        assert!(
            has("two-helpers", "auth"),
            "auth_provider shadows the inline auth helper: {:?}",
            cfg.config_warnings
        );
    }

    #[test]
    fn model_provider_undefined_auth_provider_warns() {
        use super::super::config_model_override_parse::{ConfigWarningKind, WarningTarget};

        let raw_config: toml::Value = toml::from_str(
            r#"
            [model_providers.gateway]
            base_url = "https://gateway.example/v1"
            auth_provider = "nonexistent"
            "#,
        )
        .unwrap();

        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        assert!(
            cfg.config_warnings.iter().any(|w| {
                w.kind == ConfigWarningKind::InvalidValue
                    && matches!(
                        &w.target,
                        WarningTarget::ModelProvider { id, field }
                            if id == "gateway" && field.as_deref() == Some("auth_provider")
                    )
            }),
            "an undefined provider auth_provider reference warns: {:?}",
            cfg.config_warnings
        );
    }

    #[test]
    fn model_provider_inline_auth_namespace_collision_warns() {
        use super::super::config_model_override_parse::{ConfigWarningKind, WarningTarget};

        let raw_config: toml::Value = toml::from_str(
            r#"
            [auth_provider."model_provider:gateway"]
            command = "printf hand-written"

            [model_providers.gateway]
            base_url = "https://gateway.example/v1"

            [model_providers.gateway.auth]
            command = "printf inline"
            "#,
        )
        .unwrap();

        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        assert!(
            cfg.config_warnings.iter().any(|w| {
                w.kind == ConfigWarningKind::ConflictingFields
                    && matches!(
                        &w.target,
                        WarningTarget::ModelProvider { id, field }
                            if id == "gateway" && field.as_deref() == Some("auth")
                    )
            }),
            "a reserved-namespace collision warns: {:?}",
            cfg.config_warnings
        );
        assert_eq!(
            cfg.auth_providers
                .get("model_provider:gateway")
                .map(|c| c.command.as_str()),
            Some("printf inline"),
            "inline auth wins the reserved name"
        );
    }

    #[test]
    fn model_inherits_provider_named_auth_provider() {
        let raw_config: toml::Value = toml::from_str(
            r#"
            [auth_provider.corp]
            command = "printf corp-token"
            token_ttl_secs = 3600

            [model_providers.gateway]
            base_url = "https://gateway.example/v1"
            auth_provider = "corp"

            [model.via-gateway]
            model = "m"
            model_provider = "gateway"
            "#,
        )
        .unwrap();

        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        let resolved = resolve_model_list(&cfg, None);
        let model = resolved.get("via-gateway").expect("model should exist");
        let provider = model
            .auth_provider
            .as_ref()
            .expect("the model inherits the provider's named auth_provider");
        assert_eq!(provider.name, "corp");
        assert_eq!(provider.config.command, "printf corp-token");
        assert!(model.has_own_credentials());
    }

    #[test]
    fn model_inherits_provider_static_key() {
        let raw_config: toml::Value = toml::from_str(
            r#"
            [model_providers.gateway]
            base_url = "https://gateway.example/v1"
            api_key = "sk-provider"

            [model.via-gateway]
            model = "m"
            model_provider = "gateway"
            "#,
        )
        .unwrap();

        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        let resolved = resolve_model_list(&cfg, None);
        let model = resolved.get("via-gateway").expect("model should exist");
        assert_eq!(
            resolve_credentials(model, Some("session-jwt"))
                .api_key
                .as_deref(),
            Some("sk-provider"),
            "the provider's static key resolves for the inheriting model"
        );
    }

    #[test]
    fn declared_unresolved_credential_fails_closed_on_provider_endpoint() {
        let raw_config: toml::Value = toml::from_str(
            r#"
            [model_providers.gateway]
            base_url = "https://gateway.example/v1"

            [model.via-gateway]
            model = "m"
            model_provider = "gateway"
            env_key = "DEFINITELY_UNSET_MODEL_PROVIDER_TEST_VAR"
            "#,
        )
        .unwrap();

        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        let resolved = resolve_model_list(&cfg, None);
        let model = resolved.get("via-gateway").expect("model should exist");
        assert_eq!(
            resolve_credentials(model, Some("session-jwt")).api_key,
            None,
            "an unresolved declared credential must not fall back to the session token"
        );
    }

    #[test]
    fn model_inherits_provider_api_backend_and_base_url() {
        let raw_config: toml::Value = toml::from_str(
            r#"
            [model_providers.gateway]
            base_url = "https://gateway.example/v1"
            api_base_url = "https://gateway.example/api"
            api_backend = "responses"
            api_key = "sk-provider"

            [model.via-gateway]
            model = "m"
            model_provider = "gateway"
            "#,
        )
        .unwrap();

        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        let resolved = resolve_model_list(&cfg, None);
        let model = resolved.get("via-gateway").expect("model should exist");
        assert_eq!(
            model.info.api_backend,
            crate::inference::ApiBackend::Responses
        );
        assert_eq!(
            model.api_base_url.as_deref(),
            Some("https://gateway.example/api")
        );
    }

    #[test]
    fn model_own_unresolved_key_ignores_provider_inline_auth() {
        let raw_config: toml::Value = toml::from_str(
            r#"
            [model_providers.gateway]
            base_url = "https://gateway.example/v1"

            [model_providers.gateway.auth]
            command = "printf gw-token"

            [model.own-env]
            model = "m"
            model_provider = "gateway"
            env_key = "DEFINITELY_UNSET_MODEL_PROVIDER_INLINE_VAR"
            "#,
        )
        .unwrap();

        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        let resolved = resolve_model_list(&cfg, None);
        let model = resolved.get("own-env").expect("model should exist");
        let effective = model
            .effective_auth_provider()
            .expect("an unresolved own credential fails closed via a provider ref");
        assert!(
            effective.name.contains("fail-closed"),
            "must pin the unusable fail-closed ref, not the live inline auth: {}",
            effective.name
        );
        assert!(
            effective.config.command.is_empty(),
            "the fail-closed ref is unusable"
        );
        assert_eq!(
            resolve_credentials(model, Some("session-jwt")).api_key,
            None,
            "must not fall back to the session token"
        );
    }

    #[test]
    fn fail_closed_ref_ignores_a_colliding_auth_provider_table() {
        let raw_config: toml::Value = toml::from_str(
            r#"
            [auth_provider."model_provider:gateway (fail-closed)"]
            command = "printf sneaky-token"

            [model_providers.gateway]
            base_url = "https://gateway.example/v1"

            [model.via-gateway]
            model = "m"
            context_window = 200000
            model_provider = "gateway"
            "#,
        )
        .unwrap();

        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        let resolved = resolve_model_list(&cfg, None);
        let model = resolved.get("via-gateway").expect("model should exist");
        assert_eq!(
            resolve_credentials(model, Some("session-jwt")).api_key,
            None,
            "a fail-closed ref must never resolve a colliding auth_provider table"
        );
        let effective = model
            .effective_auth_provider()
            .expect("fails closed via a provider ref");
        assert!(
            effective.config.command.is_empty(),
            "the fail-closed ref stays unusable despite the name collision"
        );
    }

    #[test]
    fn model_headers_merge_over_provider_headers() {
        let raw_config: toml::Value = toml::from_str(
            r#"
            [model_providers.gateway]
            base_url = "https://gateway.example/v1"
            api_key = "sk-provider"

            [model_providers.gateway.extra_headers]
            X-Corp = "yes"

            [model.via-gateway]
            model = "m"
            context_window = 200000
            model_provider = "gateway"

            [model.via-gateway.extra_headers]
            X-Model = "own"
            "#,
        )
        .unwrap();

        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        let resolved = resolve_model_list(&cfg, None);
        let model = resolved.get("via-gateway").expect("model should exist");
        assert_eq!(
            model.info.extra_headers.get("X-Model").map(String::as_str),
            Some("own")
        );
        assert_eq!(
            model.info.extra_headers.get("X-Corp").map(String::as_str),
            Some("yes"),
            "provider attribution/security headers survive model additions"
        );
    }

    #[test]
    fn named_provider_kinds_and_codex_command_survive_resolution() {
        let raw_config: toml::Value = toml::from_str(
            r#"
            [model_providers.openai]
            kind = "openai"
            base_url = "https://api.openai.com/v1"
            env_key = "OPENAI_API_KEY"
            api_backend = "responses"

            [model.openai-sol]
            model = "gpt-5.6-sol"
            model_provider = "openai"
            context_window = 1050000

            [model_providers.openrouter]
            kind = "openrouter"
            base_url = "https://openrouter.ai/api/v1"
            env_key = "OPENROUTER_API_KEY"
            api_backend = "chat_completions"

            [model.openrouter-sol]
            model = "openai/gpt-5.6-sol"
            model_provider = "openrouter"
            context_window = 1050000

            [model_providers.codex]
            kind = "codex"
            api_key = "must-not-reach-an-inference-endpoint"

            [model.codex-subscription]
            model = "gpt-5.6"
            model_provider = "codex"
            context_window = 1050000
            "#,
        )
        .unwrap();

        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        let resolved = resolve_model_list(&cfg, None);
        assert_eq!(
            resolved["openai-sol"]
                .model_provider
                .as_ref()
                .map(|provider| provider.kind),
            Some(super::ModelProviderKind::OpenAi)
        );
        assert_eq!(
            resolved["openrouter-sol"]
                .model_provider
                .as_ref()
                .map(|provider| provider.kind),
            Some(super::ModelProviderKind::OpenRouter)
        );
        let codex = &resolved["codex-subscription"];
        let codex_provider = codex
            .model_provider
            .as_ref()
            .expect("legacy kind=codex provider is retained as OpenAI");
        assert_eq!(
            codex_provider.kind,
            super::ModelProviderKind::OpenAi,
            "kind = \"codex\" deserializes as OpenAi (HTTP), not an external agent"
        );
        assert!(
            codex_provider.command.is_empty(),
            "no app-server command default"
        );
        assert!(!codex.info.hidden);
    }

    #[test]
    fn model_provider_inline_auth_ttl_and_timeout_warn() {
        use super::super::config_model_override_parse::{ConfigWarningKind, WarningTarget};

        let raw_config: toml::Value = toml::from_str(
            r#"
            [model_providers.gateway]
            base_url = "https://gateway.example/v1"

            [model_providers.gateway.auth]
            command = "printf tok"
            token_ttl_secs = 5
            timeout_secs = 0
            "#,
        )
        .unwrap();

        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        let has = |field: &str| {
            cfg.config_warnings.iter().any(|w| {
                w.kind == ConfigWarningKind::InvalidValue
                    && matches!(
                        &w.target,
                        WarningTarget::ModelProvider { id, field: f }
                            if id == "gateway" && f.as_deref() == Some(field)
                    )
            })
        };
        assert!(
            has("auth.token_ttl_secs"),
            "inline auth ttl below the refresh margin warns: {:?}",
            cfg.config_warnings
        );
        assert!(
            has("auth.timeout_secs"),
            "inline auth timeout out of range warns: {:?}",
            cfg.config_warnings
        );
    }

    #[test]
    fn blank_api_key_does_not_shadow_provider_auth() {
        let raw_config: toml::Value = toml::from_str(
            r#"
            [model_providers.gateway]
            base_url = "https://gateway.example/v1"

            [model_providers.gateway.auth]
            command = "printf tok"

            [model.m]
            model = "m"
            model_provider = "gateway"
            api_key = "   "
            "#,
        )
        .unwrap();
        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        let resolved = resolve_model_list(&cfg, None);
        let provider = resolved["m"]
            .auth_provider
            .as_ref()
            .expect("blank api_key must not fail-close a working gateway");
        assert_eq!(provider.name.as_str(), "model_provider:gateway");
        assert!(!provider.is_fail_closed());
    }

    #[test]
    fn openrouter_fallback_models_inherit_and_allow_model_override() {
        let raw_config: toml::Value = toml::from_str(
            r#"
            [model_providers.router]
            kind = "openrouter"
            base_url = "https://openrouter.ai/api/v1"
            openrouter_fallback_models = ["openai/gpt-5-mini", "google/gemini-2.5-flash"]

            [model.inherited]
            model = "openai/gpt-oss-120b"
            model_provider = "router"

            [model.overridden]
            model = "openai/gpt-oss-120b"
            model_provider = "router"
            openrouter_fallback_models = ["meta-llama/llama-3.3-70b-instruct"]

            [model.disabled]
            model = "openai/gpt-oss-120b"
            model_provider = "router"
            openrouter_fallback_models = []
            "#,
        )
        .unwrap();

        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        let resolved = resolve_model_list(&cfg, None);
        let fallbacks = |id: &str| {
            resolved[id]
                .model_provider
                .as_ref()
                .expect("OpenRouter provider should be retained")
                .openrouter_fallback_models
                .clone()
        };

        assert_eq!(
            fallbacks("inherited"),
            ["openai/gpt-5-mini", "google/gemini-2.5-flash"]
        );
        assert_eq!(
            fallbacks("overridden"),
            ["meta-llama/llama-3.3-70b-instruct"]
        );
        assert!(
            fallbacks("disabled").is_empty(),
            "an explicit empty model list disables the provider default"
        );

        let model = &resolved["overridden"];
        let sampling = inference_config_for_model(
            model,
            resolve_credentials(model, None),
            None,
            None,
            None,
            None,
        );
        assert_eq!(
            sampling.openrouter_fallback_models,
            ["meta-llama/llama-3.3-70b-instruct"],
            "only a resolved OpenRouter provider propagates the extension to sampling"
        );
    }

    #[test]
    fn provider_preferences_parse_and_resolve_for_openrouter() {
        let raw_config: toml::Value = toml::from_str(
            r#"
            [model_providers.openrouter]
            kind = "openrouter"
            base_url = "https://openrouter.ai/api/v1"

            [model_providers.openrouter.provider_preferences]
            sort = "latency"
            order = ["deepinfra/turbo"]
            only = []
            ignore = []
            allow_fallbacks = true
            require_parameters = true
            data_collection = "deny"
            zdr = true
            quantizations = ["int8"]
            max_price = { prompt = 0.5, completion = 2.0 }

            [model.inherited]
            model = "openai/gpt-5.6-sol"
            model_provider = "openrouter"
            context_window = 1050000
            "#,
        )
        .unwrap();

        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        let resolved = resolve_model_list(&cfg, None);
        let provider = resolved["inherited"]
            .model_provider
            .as_ref()
            .expect("provider retained");
        assert_eq!(provider.kind, super::ModelProviderKind::OpenRouter);
        let prefs = provider
            .openrouter_provider_preferences
            .as_ref()
            .expect("preferences should be retained");
        assert_eq!(
            prefs.sort.as_ref().and_then(|s| s.as_name()),
            Some("latency")
        );
        assert_eq!(prefs.order, ["deepinfra/turbo"]);
        assert!(prefs.only.is_empty());
        assert!(prefs.ignore.is_empty());
        assert_eq!(prefs.allow_fallbacks, Some(true));
        assert_eq!(prefs.require_parameters, Some(true));
        assert_eq!(prefs.data_collection.as_deref(), Some("deny"));
        assert_eq!(prefs.zdr, Some(true));
        assert_eq!(prefs.quantizations, ["int8"]);
        let max_price = prefs.max_price.as_ref().expect("max_price retained");
        assert_eq!(max_price.prompt, Some(0.5));
        assert_eq!(max_price.completion, Some(2.0));

        let sampling = inference_config_for_model(
            &resolved["inherited"],
            resolve_credentials(&resolved["inherited"], None),
            None,
            None,
            None,
            None,
        );
        let wire_prefs = sampling
            .openrouter_provider_preferences
            .as_ref()
            .expect("preferences thread to InferenceConfig");
        assert_eq!(
            wire_prefs.sort.as_ref().and_then(|s| s.as_name()),
            Some("latency")
        );
        assert_eq!(wire_prefs.data_collection.as_deref(), Some("deny"));
    }

    #[test]
    fn provider_preferences_model_override_replaces_provider_level() {
        let raw_config: toml::Value = toml::from_str(
            r#"
            [model_providers.openrouter]
            kind = "openrouter"
            base_url = "https://openrouter.ai/api/v1"

            [model_providers.openrouter.provider_preferences]
            sort = "price"
            data_collection = "allow"

            [model.inherited]
            model = "openai/gpt-oss-120b"
            model_provider = "openrouter"

            [model.overridden]
            model = "openai/gpt-oss-120b"
            model_provider = "openrouter"

            [model.overridden.provider_preferences]
            sort = "throughput"
            data_collection = "deny"
            "#,
        )
        .unwrap();

        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        let resolved = resolve_model_list(&cfg, None);
        let inherited = resolved["inherited"]
            .model_provider
            .as_ref()
            .expect("provider retained");
        let inherited_prefs = inherited
            .openrouter_provider_preferences
            .as_ref()
            .expect("inherited preferences");
        assert_eq!(
            inherited_prefs.sort.as_ref().and_then(|s| s.as_name()),
            Some("price")
        );
        assert_eq!(inherited_prefs.data_collection.as_deref(), Some("allow"));

        let overridden = resolved["overridden"]
            .model_provider
            .as_ref()
            .expect("provider retained");
        let overridden_prefs = overridden
            .openrouter_provider_preferences
            .as_ref()
            .expect("model override replaces provider-level");
        assert_eq!(
            overridden_prefs.sort.as_ref().and_then(|s| s.as_name()),
            Some("throughput")
        );
        assert_eq!(
            overridden_prefs.data_collection.as_deref(),
            Some("deny"),
            "the model-level object replaces the provider-level object entirely"
        );
        // Fields not set on the model override are not inherited from the provider
        // level — the override replaces the entire object.
        assert!(overridden_prefs.order.is_empty());
    }

    #[test]
    fn provider_preferences_absent_override_inherits_provider_level() {
        let raw_config: toml::Value = toml::from_str(
            r#"
            [model_providers.openrouter]
            kind = "openrouter"
            base_url = "https://openrouter.ai/api/v1"

            [model_providers.openrouter.provider_preferences]
            data_collection = "deny"
            require_parameters = true

            [model.no-override]
            model = "openai/gpt-oss-120b"
            model_provider = "openrouter"
            "#,
        )
        .unwrap();

        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        let resolved = resolve_model_list(&cfg, None);
        let prefs = resolved["no-override"]
            .model_provider
            .as_ref()
            .expect("provider retained")
            .openrouter_provider_preferences
            .as_ref()
            .expect("provider-level preferences inherited");
        assert_eq!(prefs.data_collection.as_deref(), Some("deny"));
        assert_eq!(prefs.require_parameters, Some(true));
    }

    #[test]
    fn provider_preferences_not_emitted_for_non_openrouter() {
        let raw_config: toml::Value = toml::from_str(
            r#"
            [model_providers.openai]
            kind = "openai"
            base_url = "https://api.openai.com/v1"

            [model_providers.openai.provider_preferences]
            sort = "latency"

            [model.via-openai]
            model = "gpt-5.6"
            model_provider = "openai"
            "#,
        )
        .unwrap();

        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        let resolved = resolve_model_list(&cfg, None);
        let provider = resolved["via-openai"]
            .model_provider
            .as_ref()
            .expect("provider retained");
        assert_eq!(provider.kind, super::ModelProviderKind::OpenAi);
        assert!(
            provider.openrouter_provider_preferences.is_none(),
            "non-OpenRouter providers must never carry preferences"
        );
    }

    #[test]
    fn built_in_openrouter_provider_has_privacy_defaults_and_title_header() {
        use super::super::providers::ProviderManager;
        let mut model_providers = indexmap::IndexMap::new();
        let mut config_models = indexmap::IndexMap::new();
        ProviderManager::install_model_presets_into(&mut model_providers, &mut config_models);

        let provider = model_providers
            .get("grok_build_openrouter")
            .expect("built-in openrouter provider should exist");
        assert_eq!(provider.kind, super::ModelProviderKind::OpenRouter);
        let prefs = provider
            .provider_preferences
            .as_ref()
            .expect("built-in openrouter has default preferences");
        assert_eq!(
            prefs.data_collection.as_deref(),
            Some("deny"),
            "built-in defaults deny data collection"
        );
        assert_eq!(
            prefs.require_parameters,
            Some(true),
            "built-in defaults require parameters"
        );
        assert!(
            prefs.zdr.is_none(),
            "zdr stays opt-in for the built-in default"
        );
        assert_eq!(
            provider
                .extra_headers
                .get("X-OpenRouter-Title")
                .map(String::as_str),
            Some("Grok Build"),
            "built-in default adds the X-OpenRouter-Title header"
        );
    }

    #[test]
    fn plugins_parse_and_resolve_for_openrouter() {
        let raw_config: toml::Value = toml::from_str(
            r#"
            [model_providers.openrouter]
            kind = "openrouter"
            base_url = "https://openrouter.ai/api/v1"

            [[model_providers.openrouter.plugins]]
            id = "response-healing"

            [[model_providers.openrouter.plugins]]
            id = "web"
            max_results = 3

            [model.inherited]
            model = "openai/gpt-5.6-sol"
            model_provider = "openrouter"
            context_window = 1050000
            "#,
        )
        .unwrap();

        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        let resolved = resolve_model_list(&cfg, None);
        let provider = resolved["inherited"]
            .model_provider
            .as_ref()
            .expect("provider retained");
        assert_eq!(provider.kind, super::ModelProviderKind::OpenRouter);
        let plugins = &provider.openrouter_plugins;
        assert_eq!(plugins.len(), 2);
        assert_eq!(plugins[0].id, "response-healing");
        assert_eq!(plugins[1].id, "web");
        assert_eq!(
            plugins[1].extra.get("max_results"),
            Some(&serde_json::json!(3))
        );

        let sampling = inference_config_for_model(
            &resolved["inherited"],
            resolve_credentials(&resolved["inherited"], None),
            None,
            None,
            None,
            None,
        );
        assert_eq!(
            sampling.openrouter_plugins.len(),
            2,
            "plugins thread to InferenceConfig"
        );
    }

    #[test]
    fn plugins_model_override_replaces_provider_level() {
        let raw_config: toml::Value = toml::from_str(
            r#"
            [model_providers.openrouter]
            kind = "openrouter"
            base_url = "https://openrouter.ai/api/v1"

            [[model_providers.openrouter.plugins]]
            id = "response-healing"

            [model.inherited]
            model = "openai/gpt-oss-120b"
            model_provider = "openrouter"

            [model.overridden]
            model = "openai/gpt-oss-120b"
            model_provider = "openrouter"

            [[model.overridden.plugins]]
            id = "web"
            "#,
        )
        .unwrap();

        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        let resolved = resolve_model_list(&cfg, None);
        let inherited = resolved["inherited"]
            .model_provider
            .as_ref()
            .expect("provider retained");
        assert_eq!(inherited.openrouter_plugins.len(), 1);
        assert_eq!(inherited.openrouter_plugins[0].id, "response-healing");

        let overridden = resolved["overridden"]
            .model_provider
            .as_ref()
            .expect("provider retained");
        assert_eq!(
            overridden.openrouter_plugins.len(),
            1,
            "model override replaces provider-level plugins"
        );
        assert_eq!(overridden.openrouter_plugins[0].id, "web");
    }

    #[test]
    fn plugins_not_emitted_for_non_openrouter() {
        let raw_config: toml::Value = toml::from_str(
            r#"
            [model_providers.openai]
            kind = "openai"
            base_url = "https://api.openai.com/v1"

            [[model_providers.openai.plugins]]
            id = "web"

            [model.via-openai]
            model = "gpt-5.6"
            model_provider = "openai"
            "#,
        )
        .unwrap();

        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        let resolved = resolve_model_list(&cfg, None);
        let provider = resolved["via-openai"]
            .model_provider
            .as_ref()
            .expect("provider retained");
        assert_eq!(provider.kind, super::ModelProviderKind::OpenAi);
        assert!(
            provider.openrouter_plugins.is_empty(),
            "non-OpenRouter providers must never carry plugins"
        );
    }

    #[test]
    fn openrouter_provider_defaults_max_completion_tokens_to_16k() {
        use super::OPENROUTER_DEFAULT_MAX_COMPLETION_TOKENS;
        let raw_config: toml::Value = toml::from_str(
            r#"
            [model_providers.zdr]
            kind = "openrouter"
            base_url = "https://openrouter.ai/api/v1"

            [model."zdr:acme/reasoner"]
            model = "acme/reasoner"
            model_provider = "zdr"
            "#,
        )
        .unwrap();
        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        let resolved = resolve_model_list(&cfg, None);
        let model = &resolved["zdr:acme/reasoner"];
        assert_eq!(
            model.info.max_completion_tokens, None,
            "provider default must not reserve context via ModelInfo"
        );
        let sampling = inference_config_for_model(
            model,
            resolve_credentials(model, None),
            None,
            None,
            None,
            None,
        );
        assert_eq!(
            sampling.max_completion_tokens,
            Some(OPENROUTER_DEFAULT_MAX_COMPLETION_TOKENS)
        );
        assert_eq!(sampling.max_output_ceiling, None);
    }

    #[test]
    fn openrouter_uses_catalog_ceiling_when_provider_max_is_unset() {
        let raw_config: toml::Value = toml::from_str(
            r#"
            [model_providers.zdr]
            kind = "openrouter"
            base_url = "https://openrouter.ai/api/v1"

            [model."zdr:z-ai/glm-5.3-flash"]
            model = "z-ai/glm-5.3-flash"
            model_provider = "zdr"
            max_output_ceiling = 131072
            "#,
        )
        .unwrap();
        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        let resolved = resolve_model_list(&cfg, None);
        let model = &resolved["zdr:z-ai/glm-5.3-flash"];
        assert_eq!(model.info.max_completion_tokens, None);
        assert_eq!(model.info.max_output_ceiling, Some(131_072));
        let sampling = inference_config_for_model(
            model,
            resolve_credentials(model, None),
            None,
            None,
            None,
            None,
        );
        assert_eq!(
            sampling.max_completion_tokens,
            Some(131_072),
            "catalog ceiling must become the request max when nothing is set"
        );
    }

    #[test]
    fn openrouter_provider_max_completion_tokens_is_configurable() {
        let raw_config: toml::Value = toml::from_str(
            r#"
            [model_providers.zdr]
            kind = "openrouter"
            base_url = "https://openrouter.ai/api/v1"
            max_completion_tokens = 4096

            [model."zdr:acme/reasoner"]
            model = "acme/reasoner"
            model_provider = "zdr"
            "#,
        )
        .unwrap();
        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        let resolved = resolve_model_list(&cfg, None);
        let model = &resolved["zdr:acme/reasoner"];
        assert_eq!(
            model
                .model_provider
                .as_ref()
                .and_then(|p| p.max_completion_tokens),
            Some(4096)
        );
        let sampling = inference_config_for_model(
            model,
            resolve_credentials(model, None),
            None,
            None,
            None,
            None,
        );
        assert_eq!(sampling.max_completion_tokens, Some(4096));
    }

    #[test]
    fn model_max_completion_tokens_wins_over_openrouter_provider_default() {
        let raw_config: toml::Value = toml::from_str(
            r#"
            [model_providers.zdr]
            kind = "openrouter"
            max_completion_tokens = 8192

            [model."zdr:acme/reasoner"]
            model = "acme/reasoner"
            model_provider = "zdr"
            max_completion_tokens = 2048
            "#,
        )
        .unwrap();
        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        let resolved = resolve_model_list(&cfg, None);
        let model = &resolved["zdr:acme/reasoner"];
        assert_eq!(model.info.max_completion_tokens, Some(2048));
        let sampling = inference_config_for_model(
            model,
            resolve_credentials(model, None),
            None,
            None,
            None,
            None,
        );
        assert_eq!(sampling.max_completion_tokens, Some(2048));
    }

    #[test]
    fn xai_and_openai_compatible_do_not_inherit_openrouter_16k_default() {
        let raw_config: toml::Value = toml::from_str(
            r#"
            [model_providers.xai]
            kind = "xai"

            [model_providers.lab]
            kind = "openai_compatible"
            base_url = "http://127.0.0.1:8000/v1"

            [model.grok-local]
            model = "grok-4.5"
            model_provider = "xai"

            [model.lab-model]
            model = "local-model"
            model_provider = "lab"
            "#,
        )
        .unwrap();
        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        let resolved = resolve_model_list(&cfg, None);
        for key in ["grok-local", "lab-model"] {
            let model = &resolved[key];
            let sampling = inference_config_for_model(
                model,
                resolve_credentials(model, None),
                None,
                None,
                None,
                None,
            );
            assert_eq!(
                sampling.max_completion_tokens, None,
                "{key} must not receive the OpenRouter 16k default"
            );
        }
    }

    #[test]
    fn openrouter_pacing_parse_and_threads_to_inference_config() {
        let raw_config: toml::Value = toml::from_str(
            r#"
            [model_providers.or-proxy]
            kind = "custom"
            base_url = "https://or-proxy.example/v1"
            openrouter_pacing = true

            [model.proxied]
            model = "openai/gpt-oss-120b"
            model_provider = "or-proxy"
            context_window = 128000
            "#,
        )
        .unwrap();

        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        let resolved = resolve_model_list(&cfg, None);
        let provider = resolved["proxied"]
            .model_provider
            .as_ref()
            .expect("provider retained");
        assert_eq!(provider.kind, super::ModelProviderKind::OpenAiCompatible);
        assert!(
            provider.openrouter_pacing,
            "custom proxy may opt into pacing without openrouter kind"
        );
        assert!(
            provider.openrouter_plugins.is_empty(),
            "pacing opt-in must not enable identity-gated plugins"
        );
        assert!(
            provider.openrouter_provider_preferences.is_none(),
            "pacing opt-in must not enable identity-gated provider prefs"
        );

        let sampling = inference_config_for_model(
            &resolved["proxied"],
            resolve_credentials(&resolved["proxied"], None),
            None,
            None,
            None,
            None,
        );
        assert!(
            sampling.openrouter_pacing,
            "openrouter_pacing must thread to InferenceConfig"
        );
        assert_eq!(
            sampling.provider_identity,
            xai_grok_inference::config::ProviderIdentity::Custom
        );
    }

    #[test]
    fn openrouter_pacing_model_override_replaces_provider_level() {
        let raw_config: toml::Value = toml::from_str(
            r#"
            [model_providers.or-proxy]
            kind = "custom"
            base_url = "https://or-proxy.example/v1"
            openrouter_pacing = true

            [model.inherited]
            model = "openai/gpt-oss-120b"
            model_provider = "or-proxy"
            context_window = 128000

            [model.overridden]
            model = "openai/gpt-oss-120b"
            model_provider = "or-proxy"
            context_window = 128000
            openrouter_pacing = false
            "#,
        )
        .unwrap();

        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        let resolved = resolve_model_list(&cfg, None);
        assert!(
            resolved["inherited"]
                .model_provider
                .as_ref()
                .expect("provider retained")
                .openrouter_pacing,
            "absent model override inherits provider-level true"
        );
        assert!(
            !resolved["overridden"]
                .model_provider
                .as_ref()
                .expect("provider retained")
                .openrouter_pacing,
            "model-level false replaces provider-level true"
        );

        let inherited = inference_config_for_model(
            &resolved["inherited"],
            resolve_credentials(&resolved["inherited"], None),
            None,
            None,
            None,
            None,
        );
        let overridden = inference_config_for_model(
            &resolved["overridden"],
            resolve_credentials(&resolved["overridden"], None),
            None,
            None,
            None,
            None,
        );
        assert!(inherited.openrouter_pacing);
        assert!(!overridden.openrouter_pacing);
    }

    #[test]
    fn openrouter_pacing_defaults_false_when_absent() {
        let raw_config: toml::Value = toml::from_str(
            r#"
            [model_providers.openai]
            kind = "openai"
            base_url = "https://api.openai.com/v1"

            [model.plain]
            model = "gpt-5.6"
            model_provider = "openai"
            "#,
        )
        .unwrap();

        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        let resolved = resolve_model_list(&cfg, None);
        let provider = resolved["plain"]
            .model_provider
            .as_ref()
            .expect("provider retained");
        assert!(!provider.openrouter_pacing);
        let sampling = inference_config_for_model(
            &resolved["plain"],
            resolve_credentials(&resolved["plain"], None),
            None,
            None,
            None,
            None,
        );
        assert!(!sampling.openrouter_pacing);
    }

    #[test]
    fn openai_compatible_kind_alias_accepts_custom_and_serializes_clear_name() {
        let raw_config: toml::Value = toml::from_str(
            r#"
            [model_providers.legacy]
            kind = "custom"
            base_url = "http://127.0.0.1:9/v1"

            [model_providers.modern]
            kind = "openai_compatible"
            base_url = "http://127.0.0.1:10/v1"
            display_name = "Modern"
            enabled = true
            catalog_enabled = true

            [model.a]
            model = "m"
            model_provider = "legacy"

            [model.b]
            model = "m"
            model_provider = "modern"
            "#,
        )
        .unwrap();
        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        assert_eq!(
            cfg.model_providers["legacy"].kind,
            super::ModelProviderKind::OpenAiCompatible
        );
        assert_eq!(
            cfg.model_providers["modern"].kind,
            super::ModelProviderKind::OpenAiCompatible
        );
        let json = serde_json::to_string(&super::ModelProviderKind::OpenAiCompatible).unwrap();
        assert_eq!(json, "\"openai_compatible\"");
    }

    #[test]
    fn invalid_base_url_skips_provider() {
        let raw_config: toml::Value = toml::from_str(
            r#"
            [model_providers.bad]
            kind = "openai_compatible"
            base_url = "https://user:pass@evil/v1"

            [model.x]
            model = "m"
            model_provider = "bad"
            "#,
        )
        .unwrap();
        let cfg = Config::new_from_toml_cfg(&raw_config).expect("config should parse");
        assert!(
            !cfg.model_providers.contains_key("bad"),
            "credential-embedding URL must skip the provider"
        );
    }

    #[test]
    fn zai_profile_installs_with_paas_v4_base() {
        use super::super::providers::ProviderManager;
        let mut model_providers = indexmap::IndexMap::new();
        let mut config_models = indexmap::IndexMap::new();
        ProviderManager::install_model_presets_into(&mut model_providers, &mut config_models);
        let zai = model_providers
            .get(crate::agent::zai::ZAI_PROVIDER_ID)
            .expect("zai profile installed");
        assert_eq!(zai.kind, super::ModelProviderKind::Zai);
        assert_eq!(
            zai.base_url.as_deref(),
            Some(crate::agent::zai::ZAI_DEFAULT_BASE_URL)
        );
        assert!(zai.api_key.is_none(), "never inline a Z.ai key");
    }
}
