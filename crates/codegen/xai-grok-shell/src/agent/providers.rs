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
const MIN_CODEX_VERSION: (u64, u64, u64) = (0, 145, 0);
const CODEX_CACHE_FILE: &str = "codex_models_cache.json";
const CODEX_CACHE_VERSION: u8 = 1;
const OPENAI_CACHE_FILE: &str = "openai_models_cache.json";
const OPENAI_CACHE_VERSION: u8 = 1;
const OPENROUTER_CACHE_FILE: &str = "openrouter_models_cache.json";
const OPENROUTER_CACHE_VERSION: u8 = 1;
/// Default freshness window for the OpenRouter catalog cache. A stale cache is
/// revalidated in the background while the picker/session keeps using the
/// last-good models.
const OPENROUTER_CATALOG_DEFAULT_TTL_SECS: u64 = 6 * 60 * 60;
const OPENROUTER_CATALOG_TTL_ENV: &str = "GROK_OPENROUTER_CATALOG_TTL_SECS";

/// A provider understood by the built-in provider screen.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderId {
    Xai,
    OpenAi,
    OpenRouter,
    Codex,
}

impl ProviderId {
    pub const ALL: [Self; 4] = [Self::Xai, Self::OpenAi, Self::OpenRouter, Self::Codex];

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Xai => "xAI",
            Self::OpenAi => "OpenAI API",
            Self::OpenRouter => "OpenRouter",
            Self::Codex => "Codex / ChatGPT",
        }
    }

    fn auth_scope(self) -> Result<&'static str, ProviderError> {
        match self {
            Self::Xai => Err(ProviderError::ApiKeyUnsupported),
            Self::OpenAi => Ok(crate::auth::OPENAI_API_KEY_SCOPE),
            Self::OpenRouter => Ok(crate::auth::OPENROUTER_API_KEY_SCOPE),
            Self::Codex => Err(ProviderError::ApiKeyUnsupported),
        }
    }

    fn environment_key(self) -> Option<&'static str> {
        match self {
            // xAI accepts both the canonical and legacy compatibility names;
            // its lookup is handled by read_xai_api_key_env().
            Self::Xai => None,
            Self::OpenAi => Some("OPENAI_API_KEY"),
            Self::OpenRouter => Some("OPENROUTER_API_KEY"),
            Self::Codex => None,
        }
    }

    pub const fn model_provider_kind(self) -> Option<ModelProviderKind> {
        match self {
            Self::Xai => Some(ModelProviderKind::Xai),
            Self::OpenAi => Some(ModelProviderKind::OpenAi),
            Self::OpenRouter => Some(ModelProviderKind::OpenRouter),
            Self::Codex => Some(ModelProviderKind::Codex),
        }
    }

    pub const fn missing_api_key_message(self) -> &'static str {
        match self {
            Self::Xai => {
                "xAI is not configured. Open /providers and connect a Grok/xAI account or add an \
                 xAI API key."
            }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderAuthenticationKind {
    OAuth,
    ApiKey,
    ChatGpt,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderAuthenticationStatus {
    pub kind: ProviderAuthenticationKind,
    pub state: ProviderConnectionState,
    pub credential_source: Option<ProviderCredentialSource>,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authentication: Vec<ProviderAuthenticationStatus>,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenAiCatalogSource {
    Live,
    Cache,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OpenAiCatalog {
    pub source: OpenAiCatalogSource,
    pub models: Vec<ProviderModelPreset>,
}

/// Models discovered from the authenticated OpenRouter catalog. This contains
/// capabilities and metadata only—never credentials or response bodies.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OpenRouterCatalog {
    pub source: OpenRouterCatalogSource,
    pub models: Vec<ProviderModelPreset>,
    /// Epoch seconds when the live catalog was fetched and cached. Absent for
    /// legacy caches written before catalog freshness tracking, which are
    /// treated as stale. Not part of the provider picker contract.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fetched_at: Option<u64>,
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
    #[error("Codex CLI 0.145.0 or newer is required")]
    CodexTooOld,
    #[error("official Codex command timed out")]
    CodexTimedOut,
    #[error("official Codex command failed")]
    CodexFailed,
    #[error("Codex model catalog is unavailable")]
    CodexCatalogUnavailable,
    #[error("OpenRouter catalog is unavailable")]
    OpenRouterCatalogUnavailable,
    #[error("OpenAI catalog is unavailable")]
    OpenAiCatalogUnavailable,
}

/// Provider service used by the TUI. The default constructor stores secrets in
/// `$GROK_HOME/auth.json` scoped credential vault and uses the official
/// `codex` executable for subscription authentication.
#[derive(Clone, Debug)]
pub struct ProviderManager {
    grok_home: PathBuf,
    xai_models_url: String,
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
            xai_models_url: "https://api.x.ai/v1/models".to_owned(),
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
        let openai_configured = credential_lookup_manager()
            .api_key(ProviderId::OpenAi)
            .ok()
            .flatten()
            .is_some();
        let mut presets = Self::presets();
        if openai_configured
            && let Ok(cached) = load_openai_catalog_cache(&credential_lookup_manager().grok_home)
        {
            presets.retain(|preset| preset.provider != ProviderId::OpenAi);
            presets.extend(cached.models);
        }
        // Static OpenRouter choices remain useful in the providers modal, but
        // are not admitted to the model picker without a BYOK credential.
        if openrouter_configured {
            if let Ok(cached) =
                load_openrouter_catalog_cache(&credential_lookup_manager().grok_home)
            {
                presets.retain(|preset| preset.provider != ProviderId::OpenRouter);
                presets.extend(cached.models);
            }
            // Stale-while-revalidate: if the cached catalog is older than the
            // TTL, trigger a non-blocking background refresh so long-lived
            // sessions eventually see retired/new models. The picker/session
            // keeps using the cached models meanwhile. This is the single
            // auto-refresh entry point; the TTL check is the only trigger.
            maybe_spawn_openrouter_background_refresh(&credential_lookup_manager().grok_home);
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
                ProviderId::Xai => continue,
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
        if provider == ProviderId::OpenAi
            && self.api_key(provider).ok().flatten().is_some()
            && let Ok(cached) = load_openai_catalog_cache(&self.grok_home)
        {
            presets = cached.models;
        }
        if provider == ProviderId::Codex
            && let Ok(cached) = load_codex_catalog_cache(&self.grok_home)
        {
            presets = cached;
        }
        match provider {
            ProviderId::Xai => {
                let oauth_connected = self.xai_oauth_configured().unwrap_or(false);
                let api_key_source = self.credential_source(provider).ok().flatten();
                let api_key_configured = api_key_source.is_some();
                ProviderStatus {
                    provider,
                    display_name: provider.display_name().to_owned(),
                    state: if oauth_connected {
                        ProviderConnectionState::Connected
                    } else if api_key_configured {
                        ProviderConnectionState::Configured
                    } else {
                        ProviderConnectionState::NotConfigured
                    },
                    credential_source: api_key_source,
                    can_test_connection: oauth_connected || api_key_configured,
                    authentication: vec![
                        ProviderAuthenticationStatus {
                            kind: ProviderAuthenticationKind::OAuth,
                            state: if oauth_connected {
                                ProviderConnectionState::Connected
                            } else {
                                ProviderConnectionState::NotConfigured
                            },
                            credential_source: oauth_connected
                                .then_some(ProviderCredentialSource::SecureStore),
                        },
                        ProviderAuthenticationStatus {
                            kind: ProviderAuthenticationKind::ApiKey,
                            state: if api_key_configured {
                                ProviderConnectionState::Configured
                            } else {
                                ProviderConnectionState::NotConfigured
                            },
                            credential_source: api_key_source,
                        },
                    ],
                    presets,
                }
            }
            ProviderId::Codex => ProviderStatus {
                provider,
                display_name: provider.display_name().to_owned(),
                state: ProviderConnectionState::Unavailable,
                credential_source: None,
                can_test_connection: false,
                authentication: Vec::new(),
                presets,
            },
            _ => match self.credential_source(provider) {
                Ok(Some(source)) => ProviderStatus {
                    provider,
                    display_name: provider.display_name().to_owned(),
                    state: ProviderConnectionState::Configured,
                    credential_source: Some(source),
                    can_test_connection: true,
                    authentication: vec![ProviderAuthenticationStatus {
                        kind: ProviderAuthenticationKind::ApiKey,
                        state: ProviderConnectionState::Configured,
                        credential_source: Some(source),
                    }],
                    presets,
                },
                Ok(None) => ProviderStatus {
                    provider,
                    display_name: provider.display_name().to_owned(),
                    state: ProviderConnectionState::NotConfigured,
                    credential_source: None,
                    can_test_connection: true,
                    authentication: vec![ProviderAuthenticationStatus {
                        kind: ProviderAuthenticationKind::ApiKey,
                        state: ProviderConnectionState::NotConfigured,
                        credential_source: None,
                    }],
                    presets,
                },
                Err(_) => ProviderStatus {
                    provider,
                    display_name: provider.display_name().to_owned(),
                    state: ProviderConnectionState::StoreUnavailable,
                    credential_source: None,
                    can_test_connection: false,
                    authentication: Vec::new(),
                    presets,
                },
            },
        }
    }

    /// The real Codex login state, queried from the official executable. No
    /// output is captured because it could contain account information.
    pub async fn codex_status(&self) -> Result<ProviderStatus, ProviderError> {
        if !self.codex_version_supported().await? {
            return Err(ProviderError::CodexTooOld);
        }
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
            authentication: vec![ProviderAuthenticationStatus {
                kind: ProviderAuthenticationKind::ChatGpt,
                state: if connected {
                    ProviderConnectionState::Connected
                } else {
                    ProviderConnectionState::NotConfigured
                },
                credential_source: None,
            }],
            presets,
        })
    }

    /// Delegate browser login to the official Codex CLI without attaching to
    /// the pager's raw-mode terminal. The CLI owns the browser flow; Grok
    /// Build never sees a password, device code, or auth token.
    pub async fn codex_login(&self) -> Result<(), ProviderError> {
        if !self.codex_version_supported().await? {
            return Err(ProviderError::CodexTooOld);
        }
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
        if provider == ProviderId::Xai {
            return crate::auth::store_api_key(&self.grok_home, api_key)
                .map_err(|_| ProviderError::CredentialStore);
        }
        crate::auth::store_provider_api_key(&self.grok_home, provider.auth_scope()?, api_key)
            .map_err(|_| ProviderError::CredentialStore)
    }

    pub fn remove_api_key(&self, provider: ProviderId) -> Result<(), ProviderError> {
        if provider == ProviderId::Codex {
            return Err(ProviderError::ApiKeyUnsupported);
        }
        let result = if provider == ProviderId::Xai {
            crate::auth::clear_api_key(&self.grok_home)
        } else {
            crate::auth::clear_provider_api_key(&self.grok_home, provider.auth_scope()?)
        };
        result.map_err(|_| ProviderError::CredentialStore)?;
        match provider {
            ProviderId::OpenAi => {
                let _ = clear_openai_catalog_cache(&self.grok_home);
            }
            ProviderId::OpenRouter => {
                let _ = clear_openrouter_catalog_cache(&self.grok_home);
            }
            ProviderId::Xai | ProviderId::Codex => {}
        }
        Ok(())
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
        let key = if provider == ProviderId::Xai {
            self.xai_oauth_bearer()?.or(self.api_key(provider)?)
        } else {
            self.api_key(provider)?
        };
        let Some(key) = key else {
            return Ok(ProviderConnectionTest::NotConfigured);
        };
        let url = match provider {
            ProviderId::Xai => &self.xai_models_url,
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
                match provider {
                    ProviderId::OpenAi => {
                        let _ = self.refresh_openai_catalog().await;
                    }
                    ProviderId::OpenRouter => {
                        let _ = self.refresh_openrouter_catalog().await;
                    }
                    ProviderId::Xai | ProviderId::Codex => {}
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
                fetched_at: current_epoch_secs(),
            };
            // A cache write failure is fail-closed for persistence but must
            // not discard a successfully authenticated response in memory.
            let _ = save_openrouter_catalog_cache(&self.grok_home, &catalog);
            return Ok(catalog);
        }
        load_openrouter_catalog_cache(&self.grok_home)
            .map_err(|_| ProviderError::OpenRouterCatalogUnavailable)
    }

    /// Fetch the authenticated OpenAI model list, merge it with the curated
    /// agent-safe presets, and persist a credential-free owner-only cache.
    pub async fn refresh_openai_catalog(&self) -> Result<OpenAiCatalog, ProviderError> {
        let Some(key) = self.api_key(ProviderId::OpenAi)? else {
            return Err(ProviderError::OpenAiCatalogUnavailable);
        };
        let response = reqwest::Client::builder()
            .timeout(CONNECTION_TIMEOUT)
            .build()
            .map_err(|_| ProviderError::OpenAiCatalogUnavailable)?
            .get(&self.openai_models_url)
            .bearer_auth(key)
            .send()
            .await;
        let live = match response {
            Ok(response) if response.status().is_success() => response
                .bytes()
                .await
                .ok()
                .and_then(|body| parse_openai_catalog(&body).ok()),
            _ => None,
        };
        if let Some(models) = live {
            let catalog = OpenAiCatalog {
                source: OpenAiCatalogSource::Live,
                models,
            };
            let _ = save_openai_catalog_cache(&self.grok_home, &catalog);
            return Ok(catalog);
        }
        load_openai_catalog_cache(&self.grok_home)
            .map_err(|_| ProviderError::OpenAiCatalogUnavailable)
    }

    pub fn cached_openai_catalog(&self) -> Result<OpenAiCatalog, ProviderError> {
        if self.api_key(ProviderId::OpenAi)?.is_none() {
            return Err(ProviderError::OpenAiCatalogUnavailable);
        }
        load_openai_catalog_cache(&self.grok_home)
            .map_err(|_| ProviderError::OpenAiCatalogUnavailable)
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
        let refresh_openai = async {
            if self.api_key(ProviderId::OpenAi).ok().flatten().is_some() {
                match self.refresh_openai_catalog().await {
                    Ok(catalog) => {
                        tracing::info!(
                            model_count = catalog.models.len(),
                            source = ?catalog.source,
                            "OpenAI model catalog refreshed"
                        );
                    }
                    Err(error) => {
                        tracing::warn!(%error, "OpenAI model catalog refresh failed");
                    }
                }
            }
        };
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
            let supported = self.codex_version_supported().await.unwrap_or(false);
            if !supported {
                let _ = clear_codex_catalog_cache(&self.grok_home);
                return;
            }
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
        tokio::join!(refresh_openai, refresh_openrouter, refresh_codex);
    }

    /// Query the authenticated Codex app-server catalog and persist a
    /// credential-free local projection for synchronous model resolution.
    pub async fn refresh_codex_catalog(&self) -> Result<Vec<ProviderModelPreset>, ProviderError> {
        if !self.codex_version_supported().await? {
            return Err(ProviderError::CodexTooOld);
        }
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
        let stored = if provider == ProviderId::Xai {
            crate::auth::read_api_key(&self.grok_home)
        } else {
            crate::auth::read_provider_api_key(&self.grok_home, provider.auth_scope()?)
                .map_err(|_| ProviderError::CredentialStore)?
        };
        if stored.is_some_and(|key| !key.trim().is_empty()) {
            return Ok(Some(ProviderCredentialSource::SecureStore));
        }
        if provider == ProviderId::Xai
            && crate::agent::auth_method::read_xai_api_key_env()
                .ok()
                .is_some_and(|key| !key.trim().is_empty())
        {
            return Ok(Some(ProviderCredentialSource::Environment));
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
        let stored = if provider == ProviderId::Xai {
            crate::auth::read_api_key(&self.grok_home)
        } else {
            crate::auth::read_provider_api_key(&self.grok_home, provider.auth_scope()?)
                .map_err(|_| ProviderError::CredentialStore)?
        };
        if let Some(key) = stored.filter(|key| !key.trim().is_empty()) {
            return Ok(Some(key));
        }
        if provider == ProviderId::Xai {
            return Ok(crate::agent::auth_method::read_xai_api_key_env()
                .ok()
                .filter(|key| !key.trim().is_empty()));
        }
        Ok(provider
            .environment_key()
            .and_then(|name| std::env::var(name).ok())
            .filter(|key| !key.trim().is_empty()))
    }

    fn xai_oauth_configured(&self) -> Result<bool, ProviderError> {
        Ok(self.xai_oauth_bearer()?.is_some())
    }

    fn xai_oauth_bearer(&self) -> Result<Option<String>, ProviderError> {
        let path = self.grok_home.join("auth.json");
        let store = match crate::auth::read_auth_json(&path) {
            Ok(store) => store,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(ProviderError::CredentialStore),
        };
        Ok(store
            .values()
            .find(|auth| {
                !auth.key.trim().is_empty()
                    && matches!(
                        auth.auth_mode,
                        crate::auth::AuthMode::Oidc | crate::auth::AuthMode::External
                    )
            })
            .map(|auth| auth.key.clone()))
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

    async fn codex_version_supported(&self) -> Result<bool, ProviderError> {
        let output = tokio::time::timeout(
            CODEX_STATUS_TIMEOUT,
            Command::new(&self.codex_program)
                .arg("--version")
                .stdin(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .output(),
        )
        .await
        .map_err(|_| ProviderError::CodexTimedOut)?
        .map_err(|_| ProviderError::CodexUnavailable)?;
        if !output.status.success() {
            return Err(ProviderError::CodexFailed);
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let version = text
            .split_whitespace()
            .find_map(parse_version_triplet)
            .ok_or(ProviderError::CodexFailed)?;
        Ok(version >= MIN_CODEX_VERSION)
    }
}

fn parse_version_triplet(value: &str) -> Option<(u64, u64, u64)> {
    let value = value.trim_matches(|ch: char| !ch.is_ascii_digit() && ch != '.');
    let mut segments = value.split('.');
    let major = segments.next()?.parse().ok()?;
    let minor = segments.next()?.parse().ok()?;
    let patch = segments
        .next()?
        .split(|ch: char| !ch.is_ascii_digit())
        .next()?
        .parse()
        .ok()?;
    Some((major, minor, patch))
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
        ModelProviderKind::Xai => ProviderId::Xai,
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
struct OpenAiModelsResponse {
    data: Vec<OpenAiModel>,
}

#[derive(Debug, Deserialize)]
struct OpenAiModel {
    id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct OpenAiCatalogCache {
    version: u8,
    models: Vec<ProviderModelPreset>,
}

fn parse_openai_catalog(body: &[u8]) -> Result<Vec<ProviderModelPreset>, ()> {
    let response: OpenAiModelsResponse = serde_json::from_slice(body).map_err(|_| ())?;
    let mut curated = ProviderManager::presets()
        .into_iter()
        .filter(|preset| preset.provider == ProviderId::OpenAi)
        .collect::<Vec<_>>();
    let curated_models = curated
        .iter()
        .map(|preset| preset.model.as_str())
        .collect::<std::collections::HashSet<_>>();
    let mut experimental = response
        .data
        .into_iter()
        .filter_map(|model| {
            let id = model.id.trim();
            if id.is_empty() || curated_models.contains(id) {
                return None;
            }
            Some(ProviderModelPreset {
                id: format!("openai:{id}"),
                provider: ProviderId::OpenAi,
                label: format!("{id} (experimental)"),
                model: id.to_owned(),
                base_url: Some("https://api.openai.com/v1".to_owned()),
                is_agent: false,
                description: Some(
                    "Discovered from the OpenAI account; agent/tool capabilities are unverified."
                        .to_owned(),
                ),
                context_window: None,
                max_completion_tokens: None,
                supports_tools: false,
                supports_reasoning_effort: false,
                reasoning_efforts: Vec::new(),
                default_reasoning_effort: None,
            })
        })
        .collect::<Vec<_>>();
    experimental.sort_by(|left, right| left.id.cmp(&right.id));
    experimental.dedup_by(|left, right| left.id == right.id);
    curated.extend(experimental);
    if curated.is_empty() {
        return Err(());
    }
    Ok(curated)
}

fn openai_cache_path(grok_home: &Path) -> PathBuf {
    grok_home.join(OPENAI_CACHE_FILE)
}

fn load_openai_catalog_cache(grok_home: &Path) -> Result<OpenAiCatalog, ()> {
    let path = openai_cache_path(grok_home);
    let bytes = std::fs::read(&path).map_err(|_| ())?;
    xai_grok_shell_base::util::secure_file::ensure_owner_only_permissions(&path).map_err(|_| ())?;
    let cache: OpenAiCatalogCache = serde_json::from_slice(&bytes).map_err(|_| ())?;
    if cache.version != OPENAI_CACHE_VERSION || cache.models.is_empty() {
        return Err(());
    }
    Ok(OpenAiCatalog {
        source: OpenAiCatalogSource::Cache,
        models: cache.models,
    })
}

fn save_openai_catalog_cache(grok_home: &Path, catalog: &OpenAiCatalog) -> std::io::Result<()> {
    let path = openai_cache_path(grok_home);
    let cache = OpenAiCatalogCache {
        version: OPENAI_CACHE_VERSION,
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

fn clear_openai_catalog_cache(grok_home: &Path) -> std::io::Result<()> {
    remove_cache_file(&openai_cache_path(grok_home))
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
    /// Epoch seconds when the cache was written. Legacy caches predating
    /// catalog freshness tracking omit this field; tolerant deserialize
    /// yields `None` so a single background refresh is attempted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fetched_at: Option<u64>,
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

/// Return the current Unix epoch seconds, or `None` if the system clock is
/// before the epoch (which should never happen in practice).
fn current_epoch_secs() -> Option<u64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
}

/// Resolve the OpenRouter catalog freshness window. The default is
/// [`OPENROUTER_CATALOG_DEFAULT_TTL_SECS`]; `GROK_OPENROUTER_CATALOG_TTL_SECS`
/// overrides it. A value of `0` disables automatic background revalidation.
fn openrouter_catalog_ttl_secs() -> u64 {
    std::env::var(OPENROUTER_CATALOG_TTL_ENV)
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .unwrap_or(OPENROUTER_CATALOG_DEFAULT_TTL_SECS)
}

/// A cached catalog is stale when no `fetched_at` timestamp is recorded
/// (legacy cache) or the elapsed time since the last fetch exceeds the TTL.
/// A TTL of `0` disables auto-refresh, so the cache is never considered
/// stale by this check.
fn openrouter_cache_is_stale(fetched_at: Option<u64>, ttl: u64) -> bool {
    if ttl == 0 {
        return false;
    }
    let Some(fetched_at) = fetched_at else {
        return true;
    };
    let Some(now) = current_epoch_secs() else {
        // A non-monotonic clock cannot prove freshness; treat as stale so a
        // refresh is attempted once and the cache is rewritten on success.
        return true;
    };
    now.saturating_sub(fetched_at) >= ttl
}

/// Process-wide guard preventing concurrent OpenRouter catalog background
/// refreshes from stampeding. Tracks whether a refresh is in flight and the
/// epoch seconds of the most recent attempt. [`try_claim_openrouter_refresh`]
/// returns a guard that releases the claim on drop; a `None` result means a
/// refresh is already in flight and the caller should keep using the cache.
static OPENROUTER_REFRESH_GUARD: std::sync::Mutex<Option<OpenRouterRefreshState>> =
    std::sync::Mutex::new(None);

struct OpenRouterRefreshState {
    in_flight: bool,
    last_attempt_secs: u64,
}

/// Claim the process-wide OpenRouter background refresh slot. Returns a guard
/// when the cache is stale and no refresh is already running, or `None` when
/// the cache is fresh, a refresh is disabled, a refresh is in flight, or the
/// last attempt was too recent to retry (debounce within the TTL window).
fn try_claim_openrouter_refresh(
    fetched_at: Option<u64>,
    ttl: u64,
) -> Option<OpenRouterRefreshClaim> {
    if !openrouter_cache_is_stale(fetched_at, ttl) {
        return None;
    }
    let now = current_epoch_secs().unwrap_or(0);
    let mut guard = OPENROUTER_REFRESH_GUARD.lock().ok()?;
    let state = guard.get_or_insert(OpenRouterRefreshState {
        in_flight: false,
        last_attempt_secs: 0,
    });
    if state.in_flight {
        return None;
    }
    // Debounce: never retry within the TTL window after the last attempt,
    // even if the cache is still stale (for example, a refresh that failed
    // without rewriting the timestamp).
    if now.saturating_sub(state.last_attempt_secs) < ttl {
        return None;
    }
    state.in_flight = true;
    state.last_attempt_secs = now;
    Some(OpenRouterRefreshClaim)
}

/// RAII guard marking the process-wide OpenRouter refresh slot as no longer
/// in flight when dropped. The refresh attempt timestamp is retained for
/// debouncing.
struct OpenRouterRefreshClaim;

impl Drop for OpenRouterRefreshClaim {
    fn drop(&mut self) {
        if let Ok(mut guard) = OPENROUTER_REFRESH_GUARD.lock()
            && let Some(state) = guard.as_mut()
        {
            state.in_flight = false;
        }
    }
}

/// Reset the process-wide OpenRouter refresh guard. Test-only: serial tests
/// that exercise [`maybe_spawn_openrouter_background_refresh`] call this to
/// start from a known state so a prior test's debounce timestamp does not
/// suppress the refresh under test.
#[cfg(test)]
fn reset_openrouter_refresh_guard_for_tests() {
    if let Ok(mut guard) = OPENROUTER_REFRESH_GUARD.lock() {
        *guard = None;
    }
}

/// Trigger a non-blocking background refresh of the OpenRouter catalog when
/// the on-disk cache is stale and an OpenRouter credential is configured.
/// The picker/session continues with the cached models meanwhile; a failed
/// refresh keeps the last-good cache. This is the single auto-refresh entry
/// point: the TTL check is the only trigger, and there are no periodic loops.
fn maybe_spawn_openrouter_background_refresh(grok_home: &Path) {
    let ttl = openrouter_catalog_ttl_secs();
    if ttl == 0 {
        return;
    }
    let manager = credential_lookup_manager();
    // Only refresh when an OpenRouter key is configured; without one the
    // live request fails closed and there is nothing to revalidate.
    let Ok(Some(_)) = manager.api_key(ProviderId::OpenRouter) else {
        return;
    };
    // Reuse the manager's grok_home unless the caller (tests) supplied a
    // thread-local override; otherwise honor the explicit path so the cache
    // file and credential vault are consistent.
    let home = if grok_home == manager.grok_home {
        manager.grok_home.clone()
    } else {
        grok_home.to_path_buf()
    };
    let fetched_at = load_openrouter_catalog_cache(&home)
        .ok()
        .and_then(|catalog| catalog.fetched_at);
    let Some(_claim) = try_claim_openrouter_refresh(fetched_at, ttl) else {
        return;
    };
    // Spawn the refresh on a dedicated thread with its own tokio runtime so
    // this synchronous caller (install_model_presets/resolve_model_list) is
    // never blocked. The claim is moved into the thread and released on drop.
    #[cfg(test)]
    let catalog_url_override = OPENROUTER_CATALOG_URL_OVERRIDE.with(|value| value.borrow().clone());
    std::thread::spawn(move || {
        let _claim = _claim;
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                tracing::warn!(%error, "OpenRouter catalog background runtime failed");
                return;
            }
        };
        runtime.block_on(async move {
            let mut manager = ProviderManager::new(&home);
            #[cfg(test)]
            if let Some(url) = catalog_url_override {
                manager = manager.with_openrouter_catalog_url(url);
            }
            match manager.refresh_openrouter_catalog().await {
                Ok(catalog) => {
                    tracing::info!(
                        model_count = catalog.models.len(),
                        source = ?catalog.source,
                        "OpenRouter model catalog background refresh completed"
                    );
                }
                Err(error) => {
                    tracing::warn!(
                        %error,
                        "OpenRouter model catalog background refresh failed; last-good cache retained"
                    );
                }
            }
        });
    });
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
        fetched_at: cache.fetched_at,
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
        fetched_at: catalog.fetched_at.or_else(current_epoch_secs),
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

fn clear_openrouter_catalog_cache(grok_home: &Path) -> std::io::Result<()> {
    remove_cache_file(&openrouter_cache_path(grok_home))
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
    remove_cache_file(&codex_cache_path(grok_home))
}

fn remove_cache_file(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
thread_local! {
    static STORED_KEY_HOME_OVERRIDE: std::cell::RefCell<Option<std::path::PathBuf>> =
        const { std::cell::RefCell::new(None) };
    static OPENROUTER_CATALOG_URL_OVERRIDE: std::cell::RefCell<Option<String>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(crate) fn set_stored_key_home_for_tests(home: Option<std::path::PathBuf>) {
    STORED_KEY_HOME_OVERRIDE.with(|value| *value.borrow_mut() = home);
}

#[cfg(test)]
pub(crate) fn set_openrouter_catalog_url_for_tests(url: Option<String>) {
    OPENROUTER_CATALOG_URL_OVERRIDE.with(|value| *value.borrow_mut() = url);
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

    #[test]
    fn xai_oauth_and_api_key_are_reported_and_removed_independently() {
        let home = tempfile::tempdir().unwrap();
        let manager = ProviderManager::new(home.path());
        manager.set_api_key(ProviderId::Xai, "xai-api-key").unwrap();
        manager
            .set_api_key(ProviderId::OpenAi, "openai-key")
            .unwrap();

        let mut store = crate::auth::read_auth_json(&home.path().join("auth.json")).unwrap();
        store.insert(
            "https://auth.x.ai".to_owned(),
            crate::auth::GrokAuth {
                key: "oauth-token".to_owned(),
                auth_mode: crate::auth::AuthMode::Oidc,
                ..Default::default()
            },
        );
        let bytes = serde_json::to_vec(&store).unwrap();
        xai_grok_shell_base::util::secure_file::write_secure_file(
            &home.path().join("auth.json"),
            &bytes,
        )
        .unwrap();

        let status = manager.status(ProviderId::Xai);
        assert_eq!(status.state, ProviderConnectionState::Connected);
        assert!(status.authentication.iter().any(|entry| {
            entry.kind == ProviderAuthenticationKind::OAuth
                && entry.state == ProviderConnectionState::Connected
        }));
        assert!(status.authentication.iter().any(|entry| {
            entry.kind == ProviderAuthenticationKind::ApiKey
                && entry.state == ProviderConnectionState::Configured
        }));

        manager.remove_api_key(ProviderId::Xai).unwrap();
        let status = manager.status(ProviderId::Xai);
        assert_eq!(status.state, ProviderConnectionState::Connected);
        assert!(crate::auth::read_api_key(home.path()).is_none());
        assert!(
            crate::auth::read_provider_api_key(home.path(), crate::auth::OPENAI_API_KEY_SCOPE)
                .unwrap()
                .is_some()
        );
    }

    #[test]
    #[serial_test::serial]
    fn xai_provider_status_honors_the_legacy_environment_key() {
        let home = tempfile::tempdir().unwrap();
        let _canonical = EnvGuard::unset(crate::agent::auth_method::XAI_API_KEY_ENV_VAR);
        let _legacy = EnvGuard::set(
            crate::agent::auth_method::LEGACY_XAI_API_KEY_ENV_VAR,
            "legacy-xai-key",
        );
        let manager = ProviderManager::new(home.path());

        let status = manager.status(ProviderId::Xai);
        assert_eq!(status.state, ProviderConnectionState::Configured);
        assert_eq!(
            status.credential_source,
            Some(ProviderCredentialSource::Environment)
        );
        assert_eq!(
            manager.api_key(ProviderId::Xai).unwrap().as_deref(),
            Some("legacy-xai-key")
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
    fn presets_are_credential_free_and_cover_every_model_provider() {
        let presets = ProviderManager::presets();
        for provider in [
            ProviderId::OpenAi,
            ProviderId::OpenRouter,
            ProviderId::Codex,
        ] {
            assert!(presets.iter().any(|preset| preset.provider == provider));
        }
        assert!(manager_status_is_credential_free(
            &ProviderManager::new(tempfile::tempdir().unwrap().path()).status(ProviderId::Xai)
        ));
        assert!(
            serde_json::to_string(&presets)
                .unwrap()
                .contains("openai-gpt-5.6-sol")
        );
    }

    fn manager_status_is_credential_free(status: &ProviderStatus) -> bool {
        let rendered = serde_json::to_string(status).unwrap();
        !rendered.contains("api_key\":\"")
            && !rendered.contains("oauth-token")
            && !rendered.contains("xai-api-key")
    }

    #[test]
    fn openai_catalog_keeps_curated_models_first_and_marks_discovery_experimental() {
        let models = parse_openai_catalog(
            br#"{"data":[{"id":"gpt-5.6-sol"},{"id":"gpt-custom-preview"},{"id":"gpt-custom-preview"}]}"#,
        )
        .unwrap();
        assert_eq!(models[0].id, "openai-gpt-5.6-sol");
        let experimental = models
            .iter()
            .find(|model| model.id == "openai:gpt-custom-preview")
            .unwrap();
        assert!(experimental.label.contains("experimental"));
        assert!(!experimental.supports_tools);
        assert!(!experimental.supports_reasoning_effort);
        assert_eq!(
            models
                .iter()
                .filter(|model| model.id == "openai:gpt-custom-preview")
                .count(),
            1
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn authenticated_openai_fetch_caches_falls_back_and_disconnect_clears_catalog() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let home = tempfile::tempdir().unwrap();
        let _openai = EnvGuard::unset("OPENAI_API_KEY");
        set_stored_key_home_for_tests(Some(home.path().to_path_buf()));
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 2048];
            let read = stream.read(&mut request).unwrap();
            let request = std::str::from_utf8(&request[..read]).unwrap();
            assert!(request.contains("GET /models HTTP/1.1"));
            assert!(
                request.contains("authorization: Bearer openai-test-key")
                    || request.contains("Authorization: Bearer openai-test-key")
            );
            let body = r#"{"data":[{"id":"gpt-5.6-sol"},{"id":"gpt-private-preview"}]}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        });
        let manager = ProviderManager::with_endpoints(
            home.path(),
            format!("http://{address}/models"),
            "http://127.0.0.1:1/key",
        );
        manager
            .set_api_key(ProviderId::OpenAi, "openai-test-key")
            .unwrap();

        let live = manager.refresh_openai_catalog().await.unwrap();
        assert_eq!(live.source, OpenAiCatalogSource::Live);
        assert!(
            live.models
                .iter()
                .any(|model| model.id == "openai:gpt-private-preview")
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(home.path().join(OPENAI_CACHE_FILE))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        server.join().unwrap();

        let cached = ProviderManager::with_endpoints(
            home.path(),
            "http://127.0.0.1:1/models",
            "http://127.0.0.1:1/key",
        )
        .refresh_openai_catalog()
        .await
        .unwrap();
        assert_eq!(cached.source, OpenAiCatalogSource::Cache);
        assert_eq!(cached.models, live.models);
        assert!(
            super::super::config::resolve_model_list(
                &super::super::config::Config::default(),
                None,
            )
            .contains_key("openai:gpt-private-preview")
        );

        manager.remove_api_key(ProviderId::OpenAi).unwrap();
        assert!(!home.path().join(OPENAI_CACHE_FILE).exists());
        assert!(
            !super::super::config::resolve_model_list(
                &super::super::config::Config::default(),
                None,
            )
            .contains_key("openai:gpt-private-preview")
        );
        set_stored_key_home_for_tests(None);
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
            fetched_at: None,
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
        assert_eq!(
            model.info.supports_reasoning_effort,
            Some(true),
            "catalog model advertising reasoning should resolve to Some(true)",
        );
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
        assert_eq!(
            models["openai-gpt-5.6-sol"].info.supports_reasoning_effort,
            Some(true)
        );
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
            "#!/bin/sh\ncase \"$1:$2\" in --version:) echo 'codex-cli 0.145.0'; exit 0 ;; login:status|login:|logout:) exit 0 ;; *) exit 1 ;; esac\n",
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

    #[cfg(unix)]
    #[tokio::test]
    async fn unsupported_codex_version_cannot_leave_models_in_the_picker_cache() {
        use std::os::unix::fs::PermissionsExt;

        let home = tempfile::tempdir().unwrap();
        let script = home.path().join("old-codex");
        std::fs::write(
            &script,
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo 'codex-cli 0.144.0'; exit 0; fi\nexit 0\n",
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&script, permissions).unwrap();
        save_codex_catalog_cache(home.path(), &static_codex_presets()).unwrap();
        let manager = ProviderManager::new(home.path()).with_codex_program(&script);

        assert!(matches!(
            manager.codex_status().await,
            Err(ProviderError::CodexTooOld)
        ));
        manager.refresh_configured_catalogs().await;
        assert!(!home.path().join(CODEX_CACHE_FILE).exists());
    }

    /// Write an OpenRouter catalog cache with the given models and
    /// `fetched_at` timestamp directly to disk, bypassing the live fetch.
    fn write_openrouter_cache(
        home: &Path,
        models: &[ProviderModelPreset],
        fetched_at: Option<u64>,
    ) {
        let cache = OpenRouterCatalogCache {
            version: OPENROUTER_CACHE_VERSION,
            models: models.to_vec(),
            fetched_at,
        };
        let bytes = serde_json::to_vec(&cache).unwrap();
        let path = openrouter_cache_path(home);
        xai_grok_shell_base::util::secure_file::write_secure_file(&path, &bytes).unwrap();
    }

    fn openrouter_cache_fetched_at(home: &Path) -> Option<u64> {
        let bytes = std::fs::read(openrouter_cache_path(home)).unwrap();
        let cache: OpenRouterCatalogCache = serde_json::from_slice(&bytes).unwrap();
        cache.fetched_at
    }

    #[test]
    fn openrouter_cache_is_stale_when_missing_timestamp() {
        // A legacy cache without fetched_at is treated as stale so a single
        // background refresh is attempted, then the cache is rewritten with
        // a timestamp on success.
        assert!(openrouter_cache_is_stale(None, 6 * 60 * 60));
    }

    #[test]
    fn openrouter_cache_is_fresh_within_ttl() {
        let now = current_epoch_secs().unwrap();
        // Fetched one minute ago with a six-hour TTL is still fresh.
        assert!(!openrouter_cache_is_stale(Some(now - 60), 6 * 60 * 60));
    }

    #[test]
    fn openrouter_cache_is_stale_beyond_ttl() {
        let now = current_epoch_secs().unwrap();
        // Fetched twelve hours ago exceeds the six-hour default TTL.
        assert!(openrouter_cache_is_stale(
            Some(now - 12 * 60 * 60),
            6 * 60 * 60
        ));
    }

    #[test]
    fn openrouter_cache_is_never_stale_when_ttl_disables_auto_refresh() {
        let now = current_epoch_secs().unwrap();
        // TTL=0 disables auto-refresh entirely, regardless of age.
        assert!(!openrouter_cache_is_stale(
            Some(now - 365 * 24 * 60 * 60),
            0
        ));
        assert!(!openrouter_cache_is_stale(None, 0));
    }

    #[test]
    #[serial_test::serial]
    fn openrouter_fresh_cache_does_not_trigger_background_refresh() {
        let home = tempfile::tempdir().unwrap();
        let _openrouter = EnvGuard::unset("OPENROUTER_API_KEY");
        set_stored_key_home_for_tests(Some(home.path().to_path_buf()));
        set_openrouter_catalog_url_for_tests(None);
        reset_openrouter_refresh_guard_for_tests();

        ProviderManager::new(home.path())
            .set_api_key(ProviderId::OpenRouter, "router-key")
            .unwrap();
        let now = current_epoch_secs().unwrap();
        let models = parse_openrouter_catalog(
            br#"{"data":[{"id":"acme/fresh","context_length":32000,"supported_parameters":["tools"]}]}"#,
        )
        .unwrap();
        write_openrouter_cache(home.path(), &models, Some(now));

        // The cache is fresh: no background refresh is claimed.
        let fetched_at = openrouter_cache_fetched_at(home.path());
        assert_eq!(fetched_at, Some(now));
        let claim = try_claim_openrouter_refresh(Some(now), 6 * 60 * 60);
        assert!(claim.is_none(), "fresh cache must not trigger a refresh");

        // The picker is still populated from the cache.
        let mut config = super::super::config::Config::default();
        ProviderManager::install_model_presets(&mut config);
        let models = super::super::config::resolve_model_list(&config, None);
        assert!(models.contains_key("openrouter:acme/fresh"));
        assert_eq!(openrouter_cache_fetched_at(home.path()), Some(now));

        set_stored_key_home_for_tests(None);
    }

    #[test]
    #[serial_test::serial]
    fn openrouter_stale_legacy_cache_triggers_background_refresh_and_rewrites_timestamp() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let home = tempfile::tempdir().unwrap();
        let _openrouter = EnvGuard::unset("OPENROUTER_API_KEY");
        set_stored_key_home_for_tests(Some(home.path().to_path_buf()));
        reset_openrouter_refresh_guard_for_tests();

        // Legacy cache without fetched_at (None) and a stale model set.
        let stale_models = parse_openrouter_catalog(
            br#"{"data":[{"id":"acme/old","context_length":8000,"supported_parameters":["tools"]}]}"#,
        )
        .unwrap();
        write_openrouter_cache(home.path(), &stale_models, None);
        assert_eq!(openrouter_cache_fetched_at(home.path()), None);

        // Mock server returns a fresh catalog.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 2048];
            let _ = stream.read(&mut request);
            let body = r#"{"data":[{"id":"acme/new","context_length":128000,"supported_parameters":["tools"]}]}"#;
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
            .set_api_key(ProviderId::OpenRouter, "router-key")
            .unwrap();
        set_openrouter_catalog_url_for_tests(Some(format!("http://{address}/models")));

        // The picker continues with the stale cached models immediately.
        let mut config = super::super::config::Config::default();
        ProviderManager::install_model_presets(&mut config);
        let models = super::super::config::resolve_model_list(&config, None);
        assert!(
            models.contains_key("openrouter:acme/old"),
            "stale-while-revalidate must populate the picker from cache"
        );

        // Wait for the background refresh to complete and rewrite the cache.
        server.join().unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            if openrouter_cache_fetched_at(home.path()).is_some() {
                break;
            }
            if std::time::Instant::now() > deadline {
                panic!("background refresh did not rewrite the timestamp");
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        // The cache now has a fresh timestamp and the new model.
        let updated = load_openrouter_catalog_cache(home.path()).unwrap();
        assert!(updated.fetched_at.is_some(), "timestamp was rewritten");
        assert!(
            updated.models.iter().any(|m| m.id == "openrouter:acme/new"),
            "background refresh updated the cached models"
        );

        set_stored_key_home_for_tests(None);
        set_openrouter_catalog_url_for_tests(None);
    }

    #[test]
    #[serial_test::serial]
    fn openrouter_failed_background_refresh_keeps_last_good_cache() {
        let home = tempfile::tempdir().unwrap();
        let _openrouter = EnvGuard::unset("OPENROUTER_API_KEY");
        set_stored_key_home_for_tests(Some(home.path().to_path_buf()));
        reset_openrouter_refresh_guard_for_tests();

        // A stale last-good cache (old timestamp + valid models).
        let now = current_epoch_secs().unwrap();
        let last_good = parse_openrouter_catalog(
            br#"{"data":[{"id":"acme/lastgood","context_length":32000,"supported_parameters":["tools"]}]}"#,
        )
        .unwrap();
        write_openrouter_cache(home.path(), &last_good, Some(now - 365 * 24 * 60 * 60));

        ProviderManager::new(home.path())
            .set_api_key(ProviderId::OpenRouter, "router-key")
            .unwrap();
        // Point the background refresh at an unreachable endpoint so the live
        // fetch fails; the cached models must remain usable.
        set_openrouter_catalog_url_for_tests(Some("http://127.0.0.1:1/models".to_owned()));

        let mut config = super::super::config::Config::default();
        ProviderManager::install_model_presets(&mut config);
        let models = super::super::config::resolve_model_list(&config, None);
        assert!(
            models.contains_key("openrouter:acme/lastgood"),
            "failed refresh must keep last-good cache"
        );

        // Give the background refresh a moment to fail, then verify the cache
        // file still holds the last-good models and the stale timestamp.
        std::thread::sleep(std::time::Duration::from_millis(200));
        let retained = load_openrouter_catalog_cache(home.path()).unwrap();
        assert!(
            retained
                .models
                .iter()
                .any(|m| m.id == "openrouter:acme/lastgood"),
            "last-good models remain after a failed refresh"
        );

        set_stored_key_home_for_tests(None);
        set_openrouter_catalog_url_for_tests(None);
    }

    #[test]
    #[serial_test::serial]
    fn openrouter_ttl_zero_disables_background_refresh() {
        let home = tempfile::tempdir().unwrap();
        let _openrouter = EnvGuard::unset("OPENROUTER_API_KEY");
        let _ttl = EnvGuard::set(OPENROUTER_CATALOG_TTL_ENV, "0");
        set_stored_key_home_for_tests(Some(home.path().to_path_buf()));
        set_openrouter_catalog_url_for_tests(None);
        reset_openrouter_refresh_guard_for_tests();

        // A deeply stale cache that would trigger a refresh if auto-refresh
        // were enabled.
        let now = current_epoch_secs().unwrap();
        let stale_models = parse_openrouter_catalog(
            br#"{"data":[{"id":"acme/stale","context_length":8000,"supported_parameters":["tools"]}]}"#,
        )
        .unwrap();
        write_openrouter_cache(home.path(), &stale_models, Some(now - 365 * 24 * 60 * 60));
        let original_fetched_at = openrouter_cache_fetched_at(home.path());

        ProviderManager::new(home.path())
            .set_api_key(ProviderId::OpenRouter, "router-key")
            .unwrap();

        // The staleness check returns false when TTL=0, so no refresh is
        // claimed even for a year-old cache.
        let claim = try_claim_openrouter_refresh(
            Some(now - 365 * 24 * 60 * 60),
            openrouter_catalog_ttl_secs(),
        );
        assert!(claim.is_none(), "TTL=0 must disable auto-refresh");

        // install_model_presets must not rewrite the cache either.
        let mut config = super::super::config::Config::default();
        ProviderManager::install_model_presets(&mut config);
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert_eq!(
            openrouter_cache_fetched_at(home.path()),
            original_fetched_at,
            "TTL=0 must leave the cache untouched"
        );

        set_stored_key_home_for_tests(None);
    }

    #[test]
    #[serial_test::serial]
    fn openrouter_refresh_guard_debounces_concurrent_triggers() {
        reset_openrouter_refresh_guard_for_tests();

        // First claim succeeds for a stale cache.
        let now = current_epoch_secs().unwrap();
        let first = try_claim_openrouter_refresh(Some(now - 12 * 60 * 60), 6 * 60 * 60);
        assert!(first.is_some(), "first stale trigger should claim");

        // While the first refresh is in flight, a second concurrent trigger
        // must not stampede: no second claim is granted.
        let second = try_claim_openrouter_refresh(Some(now - 12 * 60 * 60), 6 * 60 * 60);
        assert!(second.is_none(), "in-flight refresh must suppress a second");

        // Dropping the first claim releases the slot; the last-attempt
        // timestamp is retained so a retry within the TTL is still debounced.
        drop(first);
        let third = try_claim_openrouter_refresh(Some(now - 12 * 60 * 60), 6 * 60 * 60);
        assert!(
            third.is_none(),
            "debounce within the TTL must suppress an immediate retry"
        );

        reset_openrouter_refresh_guard_for_tests();
    }
}
