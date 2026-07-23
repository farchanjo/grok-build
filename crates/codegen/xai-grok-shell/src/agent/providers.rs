//! Provider credentials and connection actions used by the interactive UI.
//!
//! API keys live in provider-scoped entries in the existing owner-only
//! `auth.json` vault rather than `config.toml`. This module deliberately never
//! serializes a key in a status value, error, or log message.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::process::Command;

use super::model_providers::ModelProviderKind;

const MAX_API_KEY_BYTES: usize = 16 * 1024;
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(10);
const CODEX_STATUS_TIMEOUT: Duration = Duration::from_secs(10);
const CODEX_LOGIN_TIMEOUT: Duration = Duration::from_secs(180);
const CODEX_CACHE_FILE: &str = "codex_models_cache.json";
const CODEX_CACHE_VERSION: u8 = 1;
const OPENROUTER_CACHE_FILE: &str = "openrouter_models_cache.json";
const OPENROUTER_CACHE_VERSION: u8 = 1;

/// A provider understood by the built-in provider screen.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderId {
    OpenAi,
    OpenRouter,
    Codex,
}

impl ProviderId {
    pub const ALL: [Self; 3] = [Self::OpenAi, Self::OpenRouter, Self::Codex];

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::OpenAi => "OpenAI API",
            Self::OpenRouter => "OpenRouter",
            Self::Codex => "Codex / ChatGPT",
        }
    }

    fn auth_scope(self) -> Result<&'static str, ProviderError> {
        match self {
            Self::OpenAi => Ok(crate::auth::OPENAI_API_KEY_SCOPE),
            Self::OpenRouter => Ok(crate::auth::OPENROUTER_API_KEY_SCOPE),
            Self::Codex => Err(ProviderError::ApiKeyUnsupported),
        }
    }

    fn environment_key(self) -> Option<&'static str> {
        match self {
            Self::OpenAi => Some("OPENAI_API_KEY"),
            Self::OpenRouter => Some("OPENROUTER_API_KEY"),
            Self::Codex => None,
        }
    }

    pub const fn model_provider_kind(self) -> Option<ModelProviderKind> {
        match self {
            Self::OpenAi => Some(ModelProviderKind::OpenAi),
            Self::OpenRouter => Some(ModelProviderKind::OpenRouter),
            Self::Codex => Some(ModelProviderKind::Codex),
        }
    }

    pub const fn missing_api_key_message(self) -> &'static str {
        match self {
            Self::OpenAi => {
                "OpenAI API key is not configured. Open /providers and connect OpenAI, or \
                 select a GPT model via OpenRouter. A ChatGPT subscription is available \
                 through the codex-subscription agent, not as an OpenAI API key."
            }
            Self::OpenRouter => {
                "OpenRouter API key is not configured. Open /providers and connect OpenRouter."
            }
            Self::Codex => {
                "Codex uses the official ChatGPT login. Open /providers and connect Codex."
            }
        }
    }
}

/// Where a configured API credential was found. It intentionally has no
/// variant carrying the credential itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCredentialSource {
    SecureStore,
    Environment,
}

/// Safe, displayable provider state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderConnectionState {
    Connected,
    Configured,
    NotConfigured,
    Unavailable,
    StoreUnavailable,
}

/// Status for one provider. This is safe to render and to send over IPC.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderStatus {
    pub provider: ProviderId,
    pub display_name: String,
    pub state: ProviderConnectionState,
    pub credential_source: Option<ProviderCredentialSource>,
    pub can_test_connection: bool,
    pub presets: Vec<ProviderModelPreset>,
}

/// A built-in model choice. Credentials are never part of a preset.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderModelPreset {
    pub id: String,
    pub provider: ProviderId,
    pub label: String,
    pub model: String,
    pub base_url: Option<String>,
    pub is_agent: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u32>,
    #[serde(default)]
    pub supports_tools: bool,
    #[serde(default)]
    pub supports_reasoning_effort: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasoning_efforts: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_reasoning_effort: Option<String>,
}

/// Source of an OpenRouter model catalog. A cache is only used after an
/// authenticated live request has populated it.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenRouterCatalogSource {
    Live,
    Cache,
}

/// Models discovered from the authenticated OpenRouter catalog. This contains
/// capabilities and metadata only—never credentials or response bodies.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OpenRouterCatalog {
    pub source: OpenRouterCatalogSource,
    pub models: Vec<ProviderModelPreset>,
}

/// A safe outcome of testing an API connection.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderConnectionTest {
    Connected,
    NotConfigured,
    Rejected,
    Unavailable,
}

/// Errors exposed to the UI. Never include command output, HTTP bodies, URLs
/// with credentials, or API keys.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("this provider does not accept an API key")]
    ApiKeyUnsupported,
    #[error("API key is empty or too large")]
    InvalidApiKey,
    #[error("provider credential store is unavailable")]
    CredentialStore,
    #[error("official Codex executable is unavailable")]
    CodexUnavailable,
    #[error("official Codex command timed out")]
    CodexTimedOut,
    #[error("official Codex command failed")]
    CodexFailed,
    #[error("Codex model catalog is unavailable")]
    CodexCatalogUnavailable,
    #[error("OpenRouter catalog is unavailable")]
    OpenRouterCatalogUnavailable,
}

/// Provider service used by the TUI. The default constructor stores secrets in
/// `$GROK_HOME/auth.json` scoped credential vault and uses the official
/// `codex` executable for subscription authentication.
#[derive(Clone, Debug)]
pub struct ProviderManager {
    grok_home: PathBuf,
    openai_models_url: String,
    openrouter_models_url: String,
    openrouter_catalog_url: String,
    codex_program: PathBuf,
}

impl Default for ProviderManager {
    fn default() -> Self {
        Self::new(crate::util::grok_home::grok_home())
    }
}

impl ProviderManager {
    pub fn new(grok_home: impl Into<PathBuf>) -> Self {
        Self {
            grok_home: grok_home.into(),
            openai_models_url: "https://api.openai.com/v1/models".to_owned(),
            // `/models` is public on OpenRouter and therefore cannot validate
            // a credential. This endpoint verifies the supplied key without
            // sending an inference request or incurring model charges.
            openrouter_models_url: "https://openrouter.ai/api/v1/key".to_owned(),
            openrouter_catalog_url: "https://openrouter.ai/api/v1/models".to_owned(),
            codex_program: PathBuf::from("codex"),
        }
    }

    /// Test-only/custom-deployment constructor. The endpoints are retained
    /// only in memory and are never persisted with credentials.
    pub fn with_endpoints(
        grok_home: impl Into<PathBuf>,
        openai_models_url: impl Into<String>,
        openrouter_models_url: impl Into<String>,
    ) -> Self {
        Self {
            openai_models_url: openai_models_url.into(),
            openrouter_models_url: openrouter_models_url.into(),
            ..Self::new(grok_home)
        }
    }

    /// Override only the authenticated OpenRouter catalog endpoint. Intended
    /// for tests and a deliberately managed proxy; it is never persisted.
    pub fn with_openrouter_catalog_url(mut self, url: impl Into<String>) -> Self {
        self.openrouter_catalog_url = url.into();
        self
    }

    /// Override the official executable for tests or an explicitly managed
    /// install. The command is never interpreted by a shell.
    pub fn with_codex_program(mut self, program: impl Into<PathBuf>) -> Self {
        self.codex_program = program.into();
        self
    }

    pub fn presets() -> Vec<ProviderModelPreset> {
        vec![
            ProviderModelPreset {
                id: "openai-gpt-5.6-sol".to_owned(),
                provider: ProviderId::OpenAi,
                label: "GPT-5.6 Sol".to_owned(),
                model: "gpt-5.6-sol".to_owned(),
                base_url: Some("https://api.openai.com/v1".to_owned()),
                is_agent: false,
                description: None,
                context_window: Some(1_050_000),
                max_completion_tokens: Some(128_000),
                supports_tools: true,
                supports_reasoning_effort: true,
                reasoning_efforts: Vec::new(),
                default_reasoning_effort: None,
            },
            ProviderModelPreset {
                id: "openai-gpt-5.6-terra".to_owned(),
                provider: ProviderId::OpenAi,
                label: "GPT-5.6 Terra".to_owned(),
                model: "gpt-5.6-terra".to_owned(),
                base_url: Some("https://api.openai.com/v1".to_owned()),
                is_agent: false,
                description: None,
                context_window: Some(1_050_000),
                max_completion_tokens: Some(128_000),
                supports_tools: true,
                supports_reasoning_effort: true,
                reasoning_efforts: Vec::new(),
                default_reasoning_effort: None,
            },
            ProviderModelPreset {
                id: "openai-gpt-5.6-luna".to_owned(),
                provider: ProviderId::OpenAi,
                label: "GPT-5.6 Luna".to_owned(),
                model: "gpt-5.6-luna".to_owned(),
                base_url: Some("https://api.openai.com/v1".to_owned()),
                is_agent: false,
                description: None,
                context_window: Some(400_000),
                max_completion_tokens: Some(128_000),
                supports_tools: true,
                supports_reasoning_effort: true,
                reasoning_efforts: Vec::new(),
                default_reasoning_effort: None,
            },
            ProviderModelPreset {
                id: "openrouter-openai-gpt-5.6-sol".to_owned(),
                provider: ProviderId::OpenRouter,
                label: "GPT-5.6 Sol via OpenRouter".to_owned(),
                model: "openai/gpt-5.6-sol".to_owned(),
                base_url: Some("https://openrouter.ai/api/v1".to_owned()),
                is_agent: false,
                description: None,
                context_window: Some(1_050_000),
                max_completion_tokens: Some(128_000),
                supports_tools: true,
                supports_reasoning_effort: true,
                reasoning_efforts: Vec::new(),
                default_reasoning_effort: None,
            },
            ProviderModelPreset {
                id: "openrouter-openai-gpt-5.6-terra".to_owned(),
                provider: ProviderId::OpenRouter,
                label: "GPT-5.6 Terra via OpenRouter".to_owned(),
                model: "openai/gpt-5.6-terra".to_owned(),
                base_url: Some("https://openrouter.ai/api/v1".to_owned()),
                is_agent: false,
                description: None,
                context_window: Some(1_050_000),
                max_completion_tokens: Some(128_000),
                supports_tools: true,
                supports_reasoning_effort: true,
                reasoning_efforts: Vec::new(),
                default_reasoning_effort: None,
            },
            ProviderModelPreset {
                id: "codex-subscription".to_owned(),
                provider: ProviderId::Codex,
                label: "Codex (ChatGPT subscription)".to_owned(),
                model: "gpt-5.6-sol".to_owned(),
                base_url: None,
                is_agent: true,
                description: Some(
                    "Latest frontier agentic coding model via your ChatGPT subscription."
                        .to_owned(),
                ),
                context_window: Some(1_050_000),
                max_completion_tokens: Some(128_000),
                supports_tools: true,
                supports_reasoning_effort: true,
                reasoning_efforts: Vec::new(),
                default_reasoning_effort: Some("low".to_owned()),
            },
        ]
    }

    /// Materialize the built-in choices into an in-memory [`super::config::Config`].
    /// This deliberately does not write `config.toml`: keys and preset choices
    /// added in the TUI become immediately available to the current model
    /// catalog, while explicit user configuration keeps precedence.
    ///
    /// Credential lookup happens per turn, so saving or removing a key needs
    /// no catalog rebuild and no secret is retained in this API.
    pub fn install_model_presets(config: &mut super::config::Config) {
        Self::install_model_presets_into(&mut config.model_providers, &mut config.config_models);
    }

    pub(crate) fn install_model_presets_into(
        model_providers: &mut indexmap::IndexMap<
            String,
            super::model_providers::ModelProviderConfig,
        >,
        config_models: &mut indexmap::IndexMap<String, super::config::ConfigModelOverride>,
    ) {
        use super::config::ConfigModelOverride;
        use super::model_providers::ModelProviderConfig;
        use crate::sampling::ApiBackend;

        model_providers
            .entry("grok_build_openai".to_owned())
            .or_insert_with(|| ModelProviderConfig {
                kind: ModelProviderKind::OpenAi,
                base_url: Some("https://api.openai.com/v1".to_owned()),
                api_backend: Some(ApiBackend::Responses),
                ..Default::default()
            });
        model_providers
            .entry("grok_build_openrouter".to_owned())
            .or_insert_with(|| ModelProviderConfig {
                kind: ModelProviderKind::OpenRouter,
                base_url: Some("https://openrouter.ai/api/v1".to_owned()),
                api_backend: Some(ApiBackend::ChatCompletions),
                ..Default::default()
            });
        model_providers
            .entry("grok_build_codex".to_owned())
            .or_insert_with(|| ModelProviderConfig {
                kind: ModelProviderKind::Codex,
                ..Default::default()
            });

        let openrouter_configured = credential_lookup_manager()
            .api_key(ProviderId::OpenRouter)
            .ok()
            .flatten()
            .is_some();
        let mut presets = Self::presets();
        // Static OpenRouter choices remain useful in the providers modal, but
        // are not admitted to the model picker without a BYOK credential.
        if openrouter_configured {
            if let Ok(cached) =
                load_openrouter_catalog_cache(&credential_lookup_manager().grok_home)
            {
                presets.retain(|preset| preset.provider != ProviderId::OpenRouter);
                presets.extend(cached.models);
            }
        }
        presets.retain(|preset| preset.provider != ProviderId::Codex);
        if let Ok(cached) = load_codex_catalog_cache(&credential_lookup_manager().grok_home) {
            presets.extend(cached);
        }

        for preset in presets {
            if preset.provider == ProviderId::OpenRouter && !openrouter_configured {
                continue;
            }
            let provider = match preset.provider {
                ProviderId::OpenAi => "grok_build_openai",
                ProviderId::OpenRouter => "grok_build_openrouter",
                ProviderId::Codex => "grok_build_codex",
            };
            config_models
                .entry(preset.id)
                .or_insert_with(|| ConfigModelOverride {
                    model: Some(preset.model),
                    name: Some(preset.label),
                    description: preset.description,
                    model_provider: Some(provider.to_owned()),
                    context_window: preset.context_window,
                    // OpenRouter's `top_provider.max_completion_tokens` is a
                    // capability ceiling, not a safe per-request default.
                    // Some models advertise the full context window here
                    // (for example 131072/131072), so forwarding that value
                    // makes every non-empty prompt fail context validation.
                    // Keep the ceiling in the provider cache/status metadata
                    // and let OpenRouter choose its normal response budget.
                    max_completion_tokens: if preset.provider == ProviderId::OpenRouter {
                        None
                    } else {
                        preset.max_completion_tokens
                    },
                    reasoning_effort: preset
                        .default_reasoning_effort
                        .as_deref()
                        .and_then(|effort| effort.parse().ok()),
                    supports_reasoning_effort: Some(preset.supports_reasoning_effort),
                    reasoning_efforts: reasoning_effort_options(
                        &preset.reasoning_efforts,
                        preset.default_reasoning_effort.as_deref(),
                    ),
                    // Codex is a native app-server agent, but it is also a
                    // valid primary conversation route. Runtime routing keeps
                    // it away from the inference sampler.
                    hidden: None,
                    ..Default::default()
                });
        }
    }

    /// List statuses without a network request. Codex status is deliberately
    /// queried separately because it starts an external process.
    pub fn list(&self) -> Vec<ProviderStatus> {
        ProviderId::ALL
            .into_iter()
            .map(|provider| self.status(provider))
            .collect()
    }

    pub fn status(&self, provider: ProviderId) -> ProviderStatus {
        let mut presets = Self::presets()
            .into_iter()
            .filter(|preset| preset.provider == provider)
            .collect::<Vec<_>>();
        if provider == ProviderId::OpenRouter
            && self.api_key(provider).ok().flatten().is_some()
            && let Ok(cached) = load_openrouter_catalog_cache(&self.grok_home)
        {
            presets = cached.models;
        }
        if provider == ProviderId::Codex
            && let Ok(cached) = load_codex_catalog_cache(&self.grok_home)
        {
            presets = cached;
        }
        match provider {
            ProviderId::Codex => ProviderStatus {
                provider,
                display_name: provider.display_name().to_owned(),
                state: ProviderConnectionState::Unavailable,
                credential_source: None,
                can_test_connection: false,
                presets,
            },
            _ => match self.credential_source(provider) {
                Ok(Some(source)) => ProviderStatus {
                    provider,
                    display_name: provider.display_name().to_owned(),
                    state: ProviderConnectionState::Configured,
                    credential_source: Some(source),
                    can_test_connection: true,
                    presets,
                },
                Ok(None) => ProviderStatus {
                    provider,
                    display_name: provider.display_name().to_owned(),
                    state: ProviderConnectionState::NotConfigured,
                    credential_source: None,
                    can_test_connection: true,
                    presets,
                },
                Err(_) => ProviderStatus {
                    provider,
                    display_name: provider.display_name().to_owned(),
                    state: ProviderConnectionState::StoreUnavailable,
                    credential_source: None,
                    can_test_connection: false,
                    presets,
                },
            },
        }
    }

    /// The real Codex login state, queried from the official executable. No
    /// output is captured because it could contain account information.
    pub async fn codex_status(&self) -> Result<ProviderStatus, ProviderError> {
        let connected = self
            .run_codex_quiet(["login", "status"], CODEX_STATUS_TIMEOUT)
            .await?;
        let presets = if connected {
            self.refresh_codex_catalog().await.unwrap_or_else(|_| {
                load_codex_catalog_cache(&self.grok_home).unwrap_or_else(|_| {
                    let fallback = static_codex_presets();
                    let _ = save_codex_catalog_cache(&self.grok_home, &fallback);
                    fallback
                })
            })
        } else {
            let _ = clear_codex_catalog_cache(&self.grok_home);
            Vec::new()
        };
        Ok(ProviderStatus {
            provider: ProviderId::Codex,
            display_name: ProviderId::Codex.display_name().to_owned(),
            state: if connected {
                ProviderConnectionState::Connected
            } else {
                ProviderConnectionState::NotConfigured
            },
            credential_source: None,
            can_test_connection: false,
            presets,
        })
    }

    /// Delegate browser login to the official Codex CLI without attaching to
    /// the pager's raw-mode terminal. The CLI owns the browser flow; Grok
    /// Build never sees a password, device code, or auth token.
    pub async fn codex_login(&self) -> Result<(), ProviderError> {
        if self.run_codex_quiet(["login"], CODEX_LOGIN_TIMEOUT).await? {
            let _ = self.refresh_codex_catalog().await;
            Ok(())
        } else {
            Err(ProviderError::CodexFailed)
        }
    }

    /// Delegate logout to the official Codex CLI without inspecting its state.
    pub async fn codex_logout(&self) -> Result<(), ProviderError> {
        if self
            .run_codex_quiet(["logout"], CODEX_STATUS_TIMEOUT)
            .await?
        {
            clear_codex_catalog_cache(&self.grok_home)
                .map_err(|_| ProviderError::CredentialStore)?;
            Ok(())
        } else {
            Err(ProviderError::CodexFailed)
        }
    }

    pub fn set_api_key(&self, provider: ProviderId, api_key: &str) -> Result<(), ProviderError> {
        if provider == ProviderId::Codex {
            return Err(ProviderError::ApiKeyUnsupported);
        }
        let api_key = api_key.trim();
        if api_key.is_empty() || api_key.len() > MAX_API_KEY_BYTES {
            return Err(ProviderError::InvalidApiKey);
        }
        crate::auth::store_provider_api_key(&self.grok_home, provider.auth_scope()?, api_key)
            .map_err(|_| ProviderError::CredentialStore)
    }

    pub fn remove_api_key(&self, provider: ProviderId) -> Result<(), ProviderError> {
        if provider == ProviderId::Codex {
            return Err(ProviderError::ApiKeyUnsupported);
        }
        crate::auth::clear_provider_api_key(&self.grok_home, provider.auth_scope()?)
            .map_err(|_| ProviderError::CredentialStore)
    }

    /// Test the native provider endpoint using the resolved credential. Only a
    /// coarse result is returned; response bodies are intentionally discarded.
    pub async fn test_connection(
        &self,
        provider: ProviderId,
    ) -> Result<ProviderConnectionTest, ProviderError> {
        if provider == ProviderId::Codex {
            return Err(ProviderError::ApiKeyUnsupported);
        }
        let Some(key) = self.api_key(provider)? else {
            return Ok(ProviderConnectionTest::NotConfigured);
        };
        let url = match provider {
            ProviderId::OpenAi => &self.openai_models_url,
            ProviderId::OpenRouter => &self.openrouter_models_url,
            ProviderId::Codex => unreachable!(),
        };
        let response = reqwest::Client::builder()
            .timeout(CONNECTION_TIMEOUT)
            .build()
            .map_err(|_| ProviderError::CredentialStore)?
            .get(url)
            .bearer_auth(key)
            .send()
            .await;
        match response {
            Ok(response) if response.status().is_success() => {
                // A successful providers-modal save/test populates the model
                // cache opportunistically. Catalog failure does not make a
                // verified key look invalid; the next explicit refresh can
                // retry and a prior cache remains usable.
                if provider == ProviderId::OpenRouter {
                    let _ = self.refresh_openrouter_catalog().await;
                }
                Ok(ProviderConnectionTest::Connected)
            }
            Ok(response)
                if response.status().as_u16() == 401 || response.status().as_u16() == 403 =>
            {
                Ok(ProviderConnectionTest::Rejected)
            }
            Ok(_) | Err(_) => Ok(ProviderConnectionTest::Unavailable),
        }
    }

    /// Fetch the complete authenticated OpenRouter catalog and update the
    /// local owner-only cache. On a transport/server/parse failure, return the
    /// last valid cache if present; with no cache, fail closed.
    pub async fn refresh_openrouter_catalog(&self) -> Result<OpenRouterCatalog, ProviderError> {
        let Some(key) = self.api_key(ProviderId::OpenRouter)? else {
            return Err(ProviderError::OpenRouterCatalogUnavailable);
        };
        let response = reqwest::Client::builder()
            .timeout(CONNECTION_TIMEOUT)
            .build()
            .map_err(|_| ProviderError::OpenRouterCatalogUnavailable)?
            .get(&self.openrouter_catalog_url)
            .bearer_auth(key)
            .send()
            .await;
        let live = match response {
            Ok(response) if response.status().is_success() => response
                .bytes()
                .await
                .ok()
                .and_then(|body| parse_openrouter_catalog(&body).ok()),
            _ => None,
        };
        if let Some(models) = live {
            let catalog = OpenRouterCatalog {
                source: OpenRouterCatalogSource::Live,
                models,
            };
            // A cache write failure is fail-closed for persistence but must
            // not discard a successfully authenticated response in memory.
            let _ = save_openrouter_catalog_cache(&self.grok_home, &catalog);
            return Ok(catalog);
        }
        load_openrouter_catalog_cache(&self.grok_home)
            .map_err(|_| ProviderError::OpenRouterCatalogUnavailable)
    }

    /// Return cached discovered models only when an OpenRouter credential is
    /// configured. This keeps catalog visibility aligned with BYOK state.
    pub fn cached_openrouter_catalog(&self) -> Result<OpenRouterCatalog, ProviderError> {
        if self.api_key(ProviderId::OpenRouter)?.is_none() {
            return Err(ProviderError::OpenRouterCatalogUnavailable);
        }
        load_openrouter_catalog_cache(&self.grok_home)
            .map_err(|_| ProviderError::OpenRouterCatalogUnavailable)
    }

    /// Refresh every catalog whose provider is currently configured. Used at
    /// startup so the synchronous config resolver sees the latest cached
    /// projection without delaying one provider behind another.
    pub async fn refresh_configured_catalogs(&self) {
        let refresh_openrouter = async {
            if self
                .api_key(ProviderId::OpenRouter)
                .ok()
                .flatten()
                .is_some()
            {
                match self.refresh_openrouter_catalog().await {
                    Ok(catalog) => {
                        tracing::info!(
                            model_count = catalog.models.len(),
                            source = ?catalog.source,
                            "OpenRouter model catalog refreshed"
                        );
                    }
                    Err(error) => {
                        tracing::warn!(%error, "OpenRouter model catalog refresh failed");
                    }
                }
            }
        };
        let refresh_codex = async {
            let connected = self
                .run_codex_quiet(["login", "status"], CODEX_STATUS_TIMEOUT)
                .await
                .unwrap_or(false);
            if connected {
                match self.refresh_codex_catalog().await {
                    Ok(models) => {
                        tracing::info!(
                            model_count = models.len(),
                            "Codex subscription model catalog refreshed"
                        );
                    }
                    Err(error) => {
                        tracing::warn!(%error, "Codex subscription model catalog refresh failed");
                        if load_codex_catalog_cache(&self.grok_home).is_err() {
                            let _ =
                                save_codex_catalog_cache(&self.grok_home, &static_codex_presets());
                        }
                    }
                }
            } else {
                let _ = clear_codex_catalog_cache(&self.grok_home);
            }
        };
        tokio::join!(refresh_openrouter, refresh_codex);
    }

    /// Query the authenticated Codex app-server catalog and persist a
    /// credential-free local projection for synchronous model resolution.
    pub async fn refresh_codex_catalog(&self) -> Result<Vec<ProviderModelPreset>, ProviderError> {
        let mut request = super::codex_app_server::CodexModelListRequest::default();
        request.command[0] = self.codex_program.to_string_lossy().into_owned();
        let models = super::codex_app_server::list_codex_models(
            request,
            tokio_util::sync::CancellationToken::new(),
        )
        .await
        .map_err(|_| ProviderError::CodexCatalogUnavailable)?;
        let presets = codex_models_to_presets(models);
        if presets.is_empty() {
            return Err(ProviderError::CodexCatalogUnavailable);
        }
        save_codex_catalog_cache(&self.grok_home, &presets)
            .map_err(|_| ProviderError::CodexCatalogUnavailable)?;
        Ok(presets)
    }

    fn credential_source(
        &self,
        provider: ProviderId,
    ) -> Result<Option<ProviderCredentialSource>, ProviderError> {
        if crate::auth::read_provider_api_key(&self.grok_home, provider.auth_scope()?)
            .map_err(|_| ProviderError::CredentialStore)?
            .is_some_and(|key| !key.trim().is_empty())
        {
            return Ok(Some(ProviderCredentialSource::SecureStore));
        }
        Ok(provider
            .environment_key()
            .filter(|name| {
                std::env::var(name)
                    .ok()
                    .is_some_and(|value| !value.trim().is_empty())
            })
            .map(|_| ProviderCredentialSource::Environment))
    }

    fn api_key(&self, provider: ProviderId) -> Result<Option<String>, ProviderError> {
        if let Some(key) =
            crate::auth::read_provider_api_key(&self.grok_home, provider.auth_scope()?)
                .map_err(|_| ProviderError::CredentialStore)?
                .filter(|key| !key.trim().is_empty())
        {
            return Ok(Some(key));
        }
        Ok(provider
            .environment_key()
            .and_then(|name| std::env::var(name).ok())
            .filter(|key| !key.trim().is_empty()))
    }

    async fn run_codex_quiet<const N: usize>(
        &self,
        args: [&str; N],
        timeout: Duration,
    ) -> Result<bool, ProviderError> {
        let mut command = Command::new(&self.codex_program);
        command
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        let status = tokio::time::timeout(timeout, command.status())
            .await
            .map_err(|_| ProviderError::CodexTimedOut)?
            .map_err(|_| ProviderError::CodexUnavailable)?;
        Ok(status.success())
    }
}

fn codex_models_to_presets(
    models: Vec<super::codex_app_server::CodexCatalogModel>,
) -> Vec<ProviderModelPreset> {
    let mut presets = models
        .into_iter()
        .map(|model| {
            let id = if model.is_default {
                "codex-subscription".to_owned()
            } else {
                format!("codex:{}", model.model)
            };
            let context_window =
                if model.model.contains("5.6-sol") || model.model.contains("5.6-terra") {
                    1_050_000
                } else if model.model.contains("5.6-luna") {
                    400_000
                } else {
                    272_000
                };
            ProviderModelPreset {
                id,
                provider: ProviderId::Codex,
                label: format!("{} (Codex / ChatGPT)", model.display_name),
                model: model.model,
                base_url: None,
                is_agent: true,
                description: Some(model.description),
                context_window: Some(context_window),
                max_completion_tokens: Some(128_000),
                supports_tools: true,
                supports_reasoning_effort: !model.supported_reasoning_efforts.is_empty(),
                reasoning_efforts: model.supported_reasoning_efforts,
                default_reasoning_effort: Some(model.default_reasoning_effort),
            }
        })
        .collect::<Vec<_>>();
    presets.sort_by(|left, right| {
        let left_default = left.id == "codex-subscription";
        let right_default = right.id == "codex-subscription";
        right_default
            .cmp(&left_default)
            .then_with(|| left.label.cmp(&right.label))
    });
    presets
}

fn static_codex_presets() -> Vec<ProviderModelPreset> {
    ProviderManager::presets()
        .into_iter()
        .filter(|preset| preset.provider == ProviderId::Codex)
        .collect()
}

/// Resolve a key saved by [`ProviderManager`] for the model resolver. This is
/// intentionally crate-private: callers should use the manager, which keeps
/// keys out of all UI DTOs.
pub(crate) fn stored_api_key(kind: ModelProviderKind) -> Option<String> {
    let provider = match kind {
        ModelProviderKind::OpenAi => ProviderId::OpenAi,
        ModelProviderKind::OpenRouter => ProviderId::OpenRouter,
        _ => return None,
    };
    let manager = credential_lookup_manager();
    manager.api_key(provider).ok().flatten()
}

/// Return the named API provider whose credential is missing for this model.
///
/// This is used at model-selection and prompt boundaries so a missing BYOK
/// key never becomes a misleading upstream 401 that tells the user to refresh
/// their unrelated xAI session.
pub(crate) fn missing_api_key_provider(model: &super::config::ModelEntry) -> Option<ProviderId> {
    let provider = match model.model_provider.as_ref()?.kind {
        ModelProviderKind::OpenAi => ProviderId::OpenAi,
        ModelProviderKind::OpenRouter => ProviderId::OpenRouter,
        ModelProviderKind::Custom | ModelProviderKind::Xai | ModelProviderKind::Codex => {
            return None;
        }
    };
    crate::agent::config::resolve_credentials(model, None)
        .api_key
        .as_deref()
        .is_none_or(|key| key.trim().is_empty())
        .then_some(provider)
}

fn credential_lookup_manager() -> ProviderManager {
    #[cfg(test)]
    if let Some(home) = STORED_KEY_HOME_OVERRIDE.with(|value| value.borrow().clone()) {
        return ProviderManager::new(home);
    }
    ProviderManager::default()
}

#[derive(Debug, Deserialize)]
struct OpenRouterModelsResponse {
    data: Vec<OpenRouterModel>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterModel {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    context_length: Option<u64>,
    #[serde(default)]
    top_provider: Option<OpenRouterTopProvider>,
    #[serde(default)]
    supported_parameters: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterTopProvider {
    #[serde(default)]
    max_completion_tokens: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct OpenRouterCatalogCache {
    version: u8,
    models: Vec<ProviderModelPreset>,
}

fn parse_openrouter_catalog(body: &[u8]) -> Result<Vec<ProviderModelPreset>, ()> {
    let response: OpenRouterModelsResponse = serde_json::from_slice(body).map_err(|_| ())?;
    let mut models = response
        .data
        .into_iter()
        .filter_map(|model| {
            let id = model.id.trim();
            if id.is_empty() {
                return None;
            }
            let supported = model
                .supported_parameters
                .iter()
                .map(|parameter| parameter.to_ascii_lowercase())
                .collect::<Vec<_>>();
            let supports_tools = supported.iter().any(|parameter| {
                matches!(
                    parameter.as_str(),
                    "tools" | "tool_choice" | "function_calling"
                )
            });
            let supports_reasoning_effort = supported.iter().any(|parameter| {
                matches!(
                    parameter.as_str(),
                    "reasoning" | "reasoning_effort" | "include_reasoning"
                )
            });
            Some(ProviderModelPreset {
                // The upstream id is preserved verbatim after a stable local
                // namespace; it is the value sent to OpenRouter.
                id: format!("openrouter:{id}"),
                provider: ProviderId::OpenRouter,
                label: model.name.unwrap_or_else(|| id.to_owned()),
                model: id.to_owned(),
                base_url: Some("https://openrouter.ai/api/v1".to_owned()),
                is_agent: false,
                description: model.description,
                context_window: model.context_length.filter(|length| *length > 0),
                max_completion_tokens: model
                    .top_provider
                    .and_then(|provider| provider.max_completion_tokens)
                    .and_then(|tokens| u32::try_from(tokens).ok()),
                supports_tools,
                supports_reasoning_effort,
                reasoning_efforts: Vec::new(),
                default_reasoning_effort: None,
            })
        })
        .collect::<Vec<_>>();
    // Duplicate model ids make model selection ambiguous. Keep the first
    // response entry (OpenRouter's ordering) and make the local catalog
    // deterministic for tests and caches.
    models.sort_by(|left, right| left.id.cmp(&right.id));
    models.dedup_by(|left, right| left.id == right.id);
    if models.is_empty() {
        return Err(());
    }
    Ok(models)
}

fn openrouter_cache_path(grok_home: &Path) -> PathBuf {
    grok_home.join(OPENROUTER_CACHE_FILE)
}

fn load_openrouter_catalog_cache(grok_home: &Path) -> Result<OpenRouterCatalog, ()> {
    let path = openrouter_cache_path(grok_home);
    let bytes = std::fs::read(&path).map_err(|_| ())?;
    // Cache contents have no secret, but are owner-only so model/account
    // metadata is not published accidentally alongside auth.json.
    xai_grok_shell_base::util::secure_file::ensure_owner_only_permissions(&path).map_err(|_| ())?;
    let cache: OpenRouterCatalogCache = serde_json::from_slice(&bytes).map_err(|_| ())?;
    if cache.version != OPENROUTER_CACHE_VERSION || cache.models.is_empty() {
        return Err(());
    }
    Ok(OpenRouterCatalog {
        source: OpenRouterCatalogSource::Cache,
        models: cache.models,
    })
}

fn save_openrouter_catalog_cache(
    grok_home: &Path,
    catalog: &OpenRouterCatalog,
) -> std::io::Result<()> {
    let path = openrouter_cache_path(grok_home);
    let cache = OpenRouterCatalogCache {
        version: OPENROUTER_CACHE_VERSION,
        models: catalog.models.clone(),
    };
    let bytes = serde_json::to_vec(&cache).map_err(std::io::Error::other)?;
    let temporary = path.with_extension(format!("json.{}.tmp", std::process::id()));
    xai_grok_shell_base::util::secure_file::write_secure_file(&temporary, &bytes)?;
    #[cfg(windows)]
    {
        let _ = std::fs::remove_file(&path);
    }
    std::fs::rename(&temporary, &path)?;
    xai_grok_shell_base::util::secure_file::ensure_owner_only_permissions(&path)
}

fn reasoning_effort_options(
    efforts: &[String],
    default: Option<&str>,
) -> Vec<xai_grok_sampling_types::ReasoningEffortOption> {
    efforts
        .iter()
        .filter_map(|id| {
            let value = id.parse().ok()?;
            Some(xai_grok_sampling_types::ReasoningEffortOption {
                id: id.clone(),
                value,
                label: match id.as_str() {
                    "xhigh" => "Extra high".to_owned(),
                    other => {
                        let mut chars = other.chars();
                        chars
                            .next()
                            .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                            .unwrap_or_default()
                    }
                },
                description: None,
                default: Some(id.as_str()) == default,
            })
        })
        .collect()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CodexCatalogCache {
    version: u8,
    models: Vec<ProviderModelPreset>,
}

fn codex_cache_path(grok_home: &Path) -> PathBuf {
    grok_home.join(CODEX_CACHE_FILE)
}

fn load_codex_catalog_cache(grok_home: &Path) -> Result<Vec<ProviderModelPreset>, ()> {
    let path = codex_cache_path(grok_home);
    let bytes = std::fs::read(&path).map_err(|_| ())?;
    xai_grok_shell_base::util::secure_file::ensure_owner_only_permissions(&path).map_err(|_| ())?;
    let cache: CodexCatalogCache = serde_json::from_slice(&bytes).map_err(|_| ())?;
    if cache.version != CODEX_CACHE_VERSION || cache.models.is_empty() {
        return Err(());
    }
    Ok(cache.models)
}

fn save_codex_catalog_cache(
    grok_home: &Path,
    models: &[ProviderModelPreset],
) -> std::io::Result<()> {
    let path = codex_cache_path(grok_home);
    let cache = CodexCatalogCache {
        version: CODEX_CACHE_VERSION,
        models: models.to_vec(),
    };
    let bytes = serde_json::to_vec(&cache).map_err(std::io::Error::other)?;
    let temporary = path.with_extension(format!("json.{}.tmp", std::process::id()));
    xai_grok_shell_base::util::secure_file::write_secure_file(&temporary, &bytes)?;
    #[cfg(windows)]
    {
        let _ = std::fs::remove_file(&path);
    }
    std::fs::rename(&temporary, &path)?;
    xai_grok_shell_base::util::secure_file::ensure_owner_only_permissions(&path)
}

fn clear_codex_catalog_cache(grok_home: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(codex_cache_path(grok_home)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
thread_local! {
    static STORED_KEY_HOME_OVERRIDE: std::cell::RefCell<Option<std::path::PathBuf>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(crate) fn set_stored_key_home_for_tests(home: Option<std::path::PathBuf>) {
    STORED_KEY_HOME_OVERRIDE.with(|value| *value.borrow_mut() = home);
}

#[cfg(test)]
mod tests {
    use super::*;
    use xai_grok_test_support::EnvGuard;

    #[test]
    fn stores_provider_keys_without_exposing_them_in_status_or_debug() {
        let home = tempfile::tempdir().unwrap();
        let manager = ProviderManager::new(home.path());
        manager
            .set_api_key(ProviderId::OpenAi, "sk-secret-value")
            .unwrap();
        let status = manager.status(ProviderId::OpenAi);
        assert_eq!(status.state, ProviderConnectionState::Configured);
        assert_eq!(
            status.credential_source,
            Some(ProviderCredentialSource::SecureStore)
        );
        let json = serde_json::to_string(&status).unwrap();
        assert!(!json.contains("sk-secret-value"));
        assert!(!format!("{status:?}").contains("sk-secret-value"));
        let stored = std::fs::read_to_string(home.path().join("auth.json")).unwrap();
        assert!(stored.contains("sk-secret-value"));
    }

    #[test]
    fn provider_keys_are_scoped_and_removable() {
        let home = tempfile::tempdir().unwrap();
        let manager = ProviderManager::new(home.path());
        manager
            .set_api_key(ProviderId::OpenAi, "sk-openai")
            .unwrap();
        manager
            .set_api_key(ProviderId::OpenRouter, "sk-router")
            .unwrap();
        manager.remove_api_key(ProviderId::OpenAi).unwrap();
        assert_eq!(
            manager.status(ProviderId::OpenAi).state,
            ProviderConnectionState::NotConfigured
        );
        assert_eq!(
            manager.status(ProviderId::OpenRouter).state,
            ProviderConnectionState::Configured
        );
    }

    #[cfg(unix)]
    #[test]
    fn provider_store_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let home = tempfile::tempdir().unwrap();
        let manager = ProviderManager::new(home.path());
        manager
            .set_api_key(ProviderId::OpenAi, "sk-openai")
            .unwrap();
        let mode = std::fs::metadata(home.path().join("auth.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn rejects_codex_api_keys_and_blank_keys() {
        let home = tempfile::tempdir().unwrap();
        let manager = ProviderManager::new(home.path());
        assert!(matches!(
            manager.set_api_key(ProviderId::Codex, "x"),
            Err(ProviderError::ApiKeyUnsupported)
        ));
        assert!(matches!(
            manager.set_api_key(ProviderId::OpenAi, " "),
            Err(ProviderError::InvalidApiKey)
        ));
    }

    #[test]
    #[serial_test::serial]
    fn ignores_blank_credentials_from_storage_and_environment() {
        let home = tempfile::tempdir().unwrap();
        let _openai = EnvGuard::unset("OPENAI_API_KEY");
        let _openrouter = EnvGuard::set("OPENROUTER_API_KEY", "   ");
        crate::auth::store_provider_api_key(home.path(), crate::auth::OPENAI_API_KEY_SCOPE, "   ")
            .unwrap();
        let manager = ProviderManager::new(home.path());

        assert_eq!(
            manager.status(ProviderId::OpenAi).state,
            ProviderConnectionState::NotConfigured
        );
        assert_eq!(
            manager.status(ProviderId::OpenRouter).state,
            ProviderConnectionState::NotConfigured
        );
        assert_eq!(manager.api_key(ProviderId::OpenAi).unwrap(), None);
        assert_eq!(manager.api_key(ProviderId::OpenRouter).unwrap(), None);
    }

    #[test]
    #[serial_test::serial]
    fn missing_provider_key_is_detected_before_sampling() {
        let home = tempfile::tempdir().unwrap();
        let _openai = EnvGuard::unset("OPENAI_API_KEY");
        let _openrouter = EnvGuard::unset("OPENROUTER_API_KEY");
        set_stored_key_home_for_tests(Some(home.path().to_path_buf()));
        let manager = ProviderManager::new(home.path());
        let config = super::super::config::Config::default();
        let models = super::super::config::resolve_model_list(&config, None);

        assert_eq!(
            missing_api_key_provider(&models["openai-gpt-5.6-sol"]),
            Some(ProviderId::OpenAi)
        );
        assert!(
            !models.contains_key("openrouter-openai-gpt-5.6-terra"),
            "OpenRouter models must not enter the catalog before a key exists"
        );
        assert!(
            !models.contains_key("codex-subscription"),
            "Codex models must not enter the catalog without a verified login cache"
        );

        manager
            .set_api_key(ProviderId::OpenRouter, "openrouter-key")
            .unwrap();
        let models = super::super::config::resolve_model_list(&config, None);
        assert_eq!(
            missing_api_key_provider(&models["openrouter-openai-gpt-5.6-terra"]),
            None
        );
        set_stored_key_home_for_tests(None);
    }

    #[test]
    fn presets_are_credential_free_and_cover_every_provider() {
        let presets = ProviderManager::presets();
        for provider in ProviderId::ALL {
            assert!(presets.iter().any(|preset| preset.provider == provider));
        }
        assert!(
            serde_json::to_string(&presets)
                .unwrap()
                .contains("openai-gpt-5.6-sol")
        );
    }

    #[test]
    fn parses_openrouter_fixture_into_stable_capability_presets() {
        let fixture = br#"{
          "data": [
            {
              "id": "acme/reasoner",
              "name": "Acme Reasoner",
              "context_length": 262144,
              "top_provider": { "max_completion_tokens": 8192 },
              "supported_parameters": ["tools", "reasoning_effort"]
            },
            {
              "id": "acme/basic",
              "context_length": 32768,
              "supported_parameters": []
            }
          ]
        }"#;
        let models = parse_openrouter_catalog(fixture).unwrap();
        assert_eq!(models[0].id, "openrouter:acme/basic");
        let reasoner = models
            .iter()
            .find(|model| model.id == "openrouter:acme/reasoner")
            .unwrap();
        assert_eq!(reasoner.model, "acme/reasoner");
        assert_eq!(reasoner.context_window, Some(262144));
        assert_eq!(reasoner.max_completion_tokens, Some(8192));
        assert!(reasoner.supports_tools);
        assert!(reasoner.supports_reasoning_effort);
    }

    #[tokio::test]
    async fn authenticated_openrouter_fetch_caches_and_falls_back_to_last_catalog() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let home = tempfile::tempdir().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 2048];
            let read = stream.read(&mut request).unwrap();
            let request = std::str::from_utf8(&request[..read]).unwrap();
            assert!(request.contains("GET /models HTTP/1.1"));
            assert!(
                request.contains("authorization: Bearer router-test-key")
                    || request.contains("Authorization: Bearer router-test-key")
            );
            let body = r#"{"data":[{"id":"acme/test","context_length":65536,"top_provider":{"max_completion_tokens":4096},"supported_parameters":["tools"]}]}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        });
        let manager = ProviderManager::new(home.path())
            .with_openrouter_catalog_url(format!("http://{address}/models"));
        manager
            .set_api_key(ProviderId::OpenRouter, "router-test-key")
            .unwrap();

        let live = manager.refresh_openrouter_catalog().await.unwrap();
        assert_eq!(live.source, OpenRouterCatalogSource::Live);
        assert_eq!(live.models[0].id, "openrouter:acme/test");
        assert_eq!(
            manager.status(ProviderId::OpenRouter).presets,
            live.models,
            "the provider UI receives the discovered catalog after refresh"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(home.path().join(OPENROUTER_CACHE_FILE))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        server.join().unwrap();

        let cached = manager
            .with_openrouter_catalog_url("http://127.0.0.1:1/models")
            .refresh_openrouter_catalog()
            .await
            .unwrap();
        assert_eq!(cached.source, OpenRouterCatalogSource::Cache);
        assert_eq!(cached.models, live.models);
    }

    #[test]
    #[serial_test::serial]
    fn cached_openrouter_models_enter_picker_only_with_a_key() {
        let home = tempfile::tempdir().unwrap();
        let _openrouter = EnvGuard::unset("OPENROUTER_API_KEY");
        set_stored_key_home_for_tests(Some(home.path().to_path_buf()));
        let cached = OpenRouterCatalog {
            source: OpenRouterCatalogSource::Live,
            models: parse_openrouter_catalog(
                br#"{"data":[{"id":"acme/private","context_length":64000,"top_provider":{"max_completion_tokens":64000},"supported_parameters":["tools","reasoning"]}]}"#,
            )
            .unwrap(),
        };
        assert_eq!(cached.models[0].max_completion_tokens, Some(64000));
        save_openrouter_catalog_cache(home.path(), &cached).unwrap();
        let config = super::super::config::Config::default();
        assert!(
            !super::super::config::resolve_model_list(&config, None)
                .contains_key("openrouter:acme/private")
        );

        ProviderManager::new(home.path())
            .set_api_key(ProviderId::OpenRouter, "router-key")
            .unwrap();
        let models = super::super::config::resolve_model_list(&config, None);
        let model = &models["openrouter:acme/private"];
        assert_eq!(model.info.context_window.get(), 64000);
        assert_eq!(
            model.info.max_completion_tokens, None,
            "the provider capability ceiling must not become a per-turn output request"
        );
        assert!(model.info.supports_reasoning_effort);
        assert_eq!(
            model.info.api_backend,
            crate::sampling::ApiBackend::ChatCompletions
        );
        let sampling = super::super::config::sampling_config_for_model(
            model,
            super::super::config::resolve_credentials(model, None),
            None,
            None,
            None,
            None,
        );
        assert!(
            !sampling.include_message_model_id,
            "OpenRouter must receive only standard messages[] fields"
        );
        set_stored_key_home_for_tests(None);
    }

    #[test]
    #[serial_test::serial]
    fn installs_presets_in_memory_without_config_toml() {
        let home = tempfile::tempdir().unwrap();
        set_stored_key_home_for_tests(Some(home.path().to_path_buf()));
        ProviderManager::new(home.path())
            .set_api_key(ProviderId::OpenRouter, "router-key")
            .unwrap();
        save_codex_catalog_cache(home.path(), &static_codex_presets()).unwrap();
        let mut config = super::super::config::Config::default();
        ProviderManager::install_model_presets(&mut config);
        let models = super::super::config::resolve_model_list(&config, None);
        assert_eq!(models["openai-gpt-5.6-sol"].info.model, "gpt-5.6-sol");
        assert_eq!(
            models["openai-gpt-5.6-sol"].info.context_window.get(),
            1_050_000
        );
        assert_eq!(
            models["openai-gpt-5.6-sol"].info.max_completion_tokens,
            Some(128_000)
        );
        assert!(models["openai-gpt-5.6-sol"].info.supports_reasoning_effort);
        assert!(models.contains_key("openrouter-openai-gpt-5.6-terra"));
        assert!(!models["codex-subscription"].info.hidden);
        assert_eq!(models["codex-subscription"].info.model, "gpt-5.6-sol");
        let codex = &models["codex-subscription"];
        let sampling = super::super::config::sampling_config_for_model(
            codex,
            super::super::config::resolve_credentials(codex, None),
            None,
            None,
            None,
            None,
        );
        assert_eq!(
            sampling
                .extra_headers
                .get(super::super::model_providers::NATIVE_AGENT_PROVIDER_HEADER)
                .map(String::as_str),
            Some("grok_build_codex"),
            "Codex routing identity must survive conversion to sampler config"
        );
        set_stored_key_home_for_tests(None);
    }

    #[test]
    #[serial_test::serial]
    fn cached_codex_catalog_exposes_each_subscription_model_with_exact_efforts() {
        use super::super::codex_app_server::CodexCatalogModel;

        let home = tempfile::tempdir().unwrap();
        set_stored_key_home_for_tests(Some(home.path().to_path_buf()));
        let presets = codex_models_to_presets(vec![
            CodexCatalogModel {
                id: "sol".to_owned(),
                model: "gpt-5.6-sol".to_owned(),
                display_name: "GPT-5.6-Sol".to_owned(),
                description: "frontier".to_owned(),
                supported_reasoning_efforts: vec![
                    "low".to_owned(),
                    "max".to_owned(),
                    "ultra".to_owned(),
                ],
                default_reasoning_effort: "low".to_owned(),
                is_default: true,
            },
            CodexCatalogModel {
                id: "terra".to_owned(),
                model: "gpt-5.6-terra".to_owned(),
                display_name: "GPT-5.6-Terra".to_owned(),
                description: "balanced".to_owned(),
                supported_reasoning_efforts: vec!["medium".to_owned(), "high".to_owned()],
                default_reasoning_effort: "medium".to_owned(),
                is_default: false,
            },
        ]);
        save_codex_catalog_cache(home.path(), &presets).unwrap();

        let models = super::super::config::resolve_model_list(
            &super::super::config::Config::default(),
            None,
        );
        assert_eq!(models["codex-subscription"].info.model, "gpt-5.6-sol");
        let terra = &models["codex:gpt-5.6-terra"].info;
        assert_eq!(
            terra.name.as_deref(),
            Some("GPT-5.6-Terra (Codex / ChatGPT)")
        );
        assert_eq!(
            terra.reasoning_effort,
            Some(xai_grok_sampling_types::ReasoningEffort::Medium)
        );
        assert_eq!(terra.reasoning_efforts.len(), 2);
        // Ultra is advertised by Codex but is not yet a scalar effort in the
        // Grok sampling protocol, so it is not exposed as a broken choice.
        assert_eq!(models["codex-subscription"].info.reasoning_efforts.len(), 2);
        set_stored_key_home_for_tests(None);
    }

    #[test]
    #[serial_test::serial]
    fn codex_models_enter_and_leave_picker_with_verified_login_cache() {
        let home = tempfile::tempdir().unwrap();
        set_stored_key_home_for_tests(Some(home.path().to_path_buf()));
        let config = super::super::config::Config::default();

        assert!(
            !super::super::config::resolve_model_list(&config, None)
                .contains_key("codex-subscription")
        );

        save_codex_catalog_cache(home.path(), &static_codex_presets()).unwrap();
        assert!(
            super::super::config::resolve_model_list(&config, None)
                .contains_key("codex-subscription")
        );

        clear_codex_catalog_cache(home.path()).unwrap();
        assert!(
            !super::super::config::resolve_model_list(&config, None)
                .contains_key("codex-subscription")
        );
        set_stored_key_home_for_tests(None);
    }

    #[test]
    #[serial_test::serial]
    fn scoped_key_is_used_per_turn_then_removal_fails_closed_without_xai_fallback() {
        let home = tempfile::tempdir().unwrap();
        let _home = EnvGuard::set("GROK_HOME", home.path());
        let _openai = EnvGuard::unset("OPENAI_API_KEY");
        set_stored_key_home_for_tests(Some(home.path().to_path_buf()));
        let manager = ProviderManager::new(home.path());
        manager
            .set_api_key(ProviderId::OpenAi, "scoped-openai-key")
            .unwrap();

        let raw: toml::Value = toml::from_str("").unwrap();
        let config = super::super::config::Config::new_from_toml_cfg(&raw).unwrap();
        let models = super::super::config::resolve_model_list(&config, None);
        let openai = &models["openai-gpt-5.6-sol"];
        let credentials = super::super::config::resolve_credentials(openai, Some("xai-session"));
        assert_eq!(credentials.api_key.as_deref(), Some("scoped-openai-key"));
        assert_eq!(credentials.auth_type, xai_chat_state::AuthType::ApiKey);

        manager.remove_api_key(ProviderId::OpenAi).unwrap();
        let credentials = super::super::config::resolve_credentials(openai, Some("xai-session"));
        assert_eq!(credentials.api_key, None);
        assert_eq!(credentials.auth_type, xai_chat_state::AuthType::ApiKey);
        set_stored_key_home_for_tests(None);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn codex_actions_use_official_subcommands_without_tui_stdio() {
        use std::os::unix::fs::PermissionsExt;

        let home = tempfile::tempdir().unwrap();
        let script = home.path().join("fake-codex");
        std::fs::write(
            &script,
            "#!/bin/sh\ncase \"$1:$2\" in login:status|login:|logout:) exit 0 ;; *) exit 1 ;; esac\n",
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&script, permissions).unwrap();
        let manager = ProviderManager::new(home.path()).with_codex_program(&script);

        assert_eq!(
            manager.codex_status().await.unwrap().state,
            ProviderConnectionState::Connected
        );
        manager.codex_login().await.unwrap();
        manager.codex_logout().await.unwrap();
    }
}
