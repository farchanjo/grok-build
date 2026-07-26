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
}

impl ProviderId {
    pub const ALL: [Self; 3] = [Self::Xai, Self::OpenAi, Self::OpenRouter];

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Xai => "xAI",
            Self::OpenAi => "OpenAI",
            Self::OpenRouter => "OpenRouter",
        }
    }

    fn auth_scope(self) -> Result<&'static str, ProviderError> {
        match self {
            Self::Xai => Err(ProviderError::ApiKeyUnsupported),
            Self::OpenAi => Ok(crate::auth::OPENAI_API_KEY_SCOPE),
            Self::OpenRouter => Ok(crate::auth::OPENROUTER_API_KEY_SCOPE),
        }
    }

    fn environment_key(self) -> Option<&'static str> {
        match self {
            // xAI accepts both the canonical and legacy compatibility names;
            // its lookup is handled by read_xai_api_key_env().
            Self::Xai => None,
            Self::OpenAi => Some("OPENAI_API_KEY"),
            Self::OpenRouter => Some("OPENROUTER_API_KEY"),
        }
    }

    pub const fn model_provider_kind(self) -> Option<ModelProviderKind> {
        match self {
            Self::Xai => Some(ModelProviderKind::Xai),
            Self::OpenAi => Some(ModelProviderKind::OpenAi),
            Self::OpenRouter => Some(ModelProviderKind::OpenRouter),
        }
    }

    pub const fn missing_api_key_message(self) -> &'static str {
        match self {
            Self::Xai => {
                "xAI is not configured. Open /providers and connect a Grok/xAI account or add an \
                 xAI API key."
            }
            Self::OpenAi => {
                "OpenAI is not configured. Open /providers and connect with a ChatGPT login \
                 (subscription) or an OpenAI API key."
            }
            Self::OpenRouter => {
                "OpenRouter API key is not configured. Open /providers and connect OpenRouter."
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
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderConnectionTest {
    /// The key was accepted. For OpenRouter, `credits` carries a display-only
    /// credits-remaining string (e.g. "credits remaining: $4.21" or
    /// "unlimited") when the `/key` probe returned balance data. Never the
    /// raw body.
    Connected {
        credits: Option<String>,
    },
    NotConfigured,
    Rejected,
    Unavailable,
}

impl ProviderConnectionTest {
    /// Shorthand for a connected test with no credits data (xAI/OpenAI).
    pub fn connected() -> Self {
        Self::Connected { credits: None }
    }
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
    #[error("ChatGPT OAuth login failed")]
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
        use crate::inference::ApiBackend;

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
            .or_insert_with(|| {
                use super::model_providers::OpenRouterProviderPreferences;
                let mut extra_headers = indexmap::IndexMap::<String, String>::new();
                extra_headers
                    .entry("X-OpenRouter-Title".to_owned())
                    .or_insert("Grok Build".to_owned());
                ModelProviderConfig {
                    kind: ModelProviderKind::OpenRouter,
                    base_url: Some("https://openrouter.ai/api/v1".to_owned()),
                    api_backend: Some(ApiBackend::ChatCompletions),
                    provider_preferences: Some(OpenRouterProviderPreferences {
                        data_collection: Some("deny".to_owned()),
                        require_parameters: Some(true),
                        ..Default::default()
                    }),
                    extra_headers,
                    ..Default::default()
                }
            });
        // First-class Z.ai Model API profile (credentials never inlined).
        super::zai::install_zai_provider(model_providers);
        let openrouter_configured = credential_lookup_manager()
            .api_key(ProviderId::OpenRouter)
            .ok()
            .flatten()
            .is_some();
        let openai_api_key = credential_lookup_manager()
            .api_key(ProviderId::OpenAi)
            .ok()
            .flatten()
            .is_some();
        let openai_oauth =
            crate::auth::chatgpt_oauth::status(&credential_lookup_manager().grok_home)
                == crate::auth::chatgpt_oauth::ChatGptOAuthStatus::Connected;
        let openai_configured = openai_api_key || openai_oauth;
        let mut presets = Self::presets();
        // The OpenAI Platform and ChatGPT subscription catalogs have distinct
        // model IDs and can coexist. Each entry retains its own route.
        if openai_api_key
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
        if openai_oauth {
            presets.extend(static_chatgpt_oauth_presets());
        }

        for preset in presets {
            if preset.provider == ProviderId::OpenRouter && !openrouter_configured {
                continue;
            }
            let provider = match preset.provider {
                ProviderId::Xai => continue,
                ProviderId::OpenAi => "grok_build_openai",
                ProviderId::OpenRouter => "grok_build_openrouter",
            };
            config_models
                .entry(preset.id)
                .or_insert_with(|| ConfigModelOverride {
                    model: Some(preset.model),
                    base_url: preset.base_url,
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
                    hidden: None,
                    supports_tools: Some(preset.supports_tools),
                    ..Default::default()
                });
        }
    }

    /// List statuses without a network request.
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
        if provider == ProviderId::OpenAi
            && crate::auth::chatgpt_oauth::status(&self.grok_home)
                == crate::auth::chatgpt_oauth::ChatGptOAuthStatus::Connected
            && let Ok(cached) = load_codex_catalog_cache(&self.grok_home)
        {
            presets.extend(cached);
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
            ProviderId::OpenAi => {
                use crate::auth::chatgpt_oauth::{self, ChatGptOAuthStatus};
                let oauth = chatgpt_oauth::status(&self.grok_home);
                let oauth_connected = matches!(oauth, ChatGptOAuthStatus::Connected);
                let oauth_expired = matches!(oauth, ChatGptOAuthStatus::Expired);
                let api_key_source = self.credential_source(provider).ok().flatten();
                let api_key_configured = api_key_source.is_some();
                let mut openai_presets = presets;
                if oauth_connected {
                    openai_presets.extend(static_chatgpt_oauth_presets());
                }
                ProviderStatus {
                    provider,
                    display_name: provider.display_name().to_owned(),
                    state: if oauth_connected {
                        ProviderConnectionState::Connected
                    } else if oauth_expired || api_key_configured {
                        ProviderConnectionState::Configured
                    } else {
                        ProviderConnectionState::NotConfigured
                    },
                    credential_source: if api_key_configured {
                        api_key_source
                    } else if oauth_connected {
                        Some(ProviderCredentialSource::SecureStore)
                    } else {
                        None
                    },
                    can_test_connection: oauth_connected || api_key_configured,
                    authentication: vec![
                        ProviderAuthenticationStatus {
                            kind: ProviderAuthenticationKind::ChatGpt,
                            state: if oauth_connected {
                                ProviderConnectionState::Connected
                            } else if oauth_expired {
                                ProviderConnectionState::Configured
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
                    presets: openai_presets,
                }
            }
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

    /// Native ChatGPT OAuth status for the unified OpenAI provider.
    pub async fn chatgpt_oauth_status(&self) -> Result<ProviderStatus, ProviderError> {
        use crate::auth::chatgpt_oauth::{self, ChatGptOAuthStatus};
        let oauth = chatgpt_oauth::status(&self.grok_home);
        let connected = matches!(oauth, ChatGptOAuthStatus::Connected);
        let presets = if connected {
            static_chatgpt_oauth_presets()
        } else {
            Vec::new()
        };
        let state = match oauth {
            ChatGptOAuthStatus::Connected => ProviderConnectionState::Connected,
            ChatGptOAuthStatus::Expired => ProviderConnectionState::Configured,
            ChatGptOAuthStatus::NotConfigured => ProviderConnectionState::NotConfigured,
        };
        Ok(ProviderStatus {
            provider: ProviderId::OpenAi,
            display_name: ProviderId::OpenAi.display_name().to_owned(),
            state: state.clone(),
            credential_source: connected.then_some(ProviderCredentialSource::SecureStore),
            can_test_connection: false,
            authentication: vec![ProviderAuthenticationStatus {
                kind: ProviderAuthenticationKind::ChatGpt,
                state,
                credential_source: connected.then_some(ProviderCredentialSource::SecureStore),
            }],
            presets,
        })
    }

    /// Browser (or device) ChatGPT OAuth login for OpenAI.
    ///
    /// The separately scoped OpenAI Platform API key is preserved so each
    /// model route can select the credential it requires.
    ///
    /// Prefer browser PKCE by default. Device-code is used only when
    /// `GROK_CHATGPT_DEVICE_AUTH` is set, or on Linux without a graphical
    /// display (true headless). macOS/Windows never have `DISPLAY`, so the old
    /// "no DISPLAY" heuristic incorrectly forced device auth and hid the code.
    pub async fn chatgpt_oauth_login(&self) -> Result<(), ProviderError> {
        use crate::auth::chatgpt_oauth;
        let result = if prefer_chatgpt_device_auth() {
            chatgpt_oauth::login_device(&self.grok_home).await
        } else {
            chatgpt_oauth::login_browser(&self.grok_home).await
        };
        result.map_err(|e| {
            tracing::warn!(error = %e, "ChatGPT OAuth login failed");
            ProviderError::CodexFailed
        })?;
        let _ = save_codex_catalog_cache(&self.grok_home, &static_chatgpt_oauth_presets());
        Ok(())
    }

    /// Clear ChatGPT OAuth credentials for OpenAI.
    pub async fn chatgpt_oauth_logout(&self) -> Result<(), ProviderError> {
        crate::auth::chatgpt_oauth::clear_tokens(&self.grok_home)
            .map_err(|_| ProviderError::CredentialStore)?;
        let _ = clear_codex_catalog_cache(&self.grok_home);
        Ok(())
    }

    // Backward-compatible names used by existing TUI effects.
    pub async fn codex_status(&self) -> Result<ProviderStatus, ProviderError> {
        self.chatgpt_oauth_status().await
    }
    pub async fn codex_login(&self) -> Result<(), ProviderError> {
        self.chatgpt_oauth_login().await
    }
    pub async fn codex_logout(&self) -> Result<(), ProviderError> {
        self.chatgpt_oauth_logout().await
    }

    pub fn set_api_key(&self, provider: ProviderId, api_key: &str) -> Result<(), ProviderError> {
        let api_key = api_key.trim();
        if api_key.is_empty() || api_key.len() > MAX_API_KEY_BYTES {
            return Err(ProviderError::InvalidApiKey);
        }
        if provider == ProviderId::Xai {
            return crate::auth::store_api_key(&self.grok_home, api_key)
                .map_err(|_| ProviderError::CredentialStore);
        }
        if provider == ProviderId::OpenAi {
            return crate::auth::chatgpt_oauth::store_api_key(&self.grok_home, api_key)
                .map_err(|_| ProviderError::CredentialStore);
        }
        crate::auth::store_provider_api_key(&self.grok_home, provider.auth_scope()?, api_key)
            .map_err(|_| ProviderError::CredentialStore)
    }

    pub fn remove_api_key(&self, provider: ProviderId) -> Result<(), ProviderError> {
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
            ProviderId::Xai => {}
        }
        Ok(())
    }

    /// Test the native provider endpoint using the resolved credential. Only a
    /// coarse result is returned; response bodies are intentionally discarded.
    pub async fn test_connection(
        &self,
        provider: ProviderId,
    ) -> Result<ProviderConnectionTest, ProviderError> {
        let key = match provider {
            ProviderId::Xai => self.xai_oauth_bearer()?.or(self.api_key(provider)?),
            ProviderId::OpenAi => {
                if let Some(key) = self.api_key(provider)? {
                    Some(key)
                } else {
                    return Ok(
                        if crate::auth::chatgpt_oauth::status(&self.grok_home)
                            == crate::auth::chatgpt_oauth::ChatGptOAuthStatus::Connected
                        {
                            ProviderConnectionTest::connected()
                        } else {
                            ProviderConnectionTest::NotConfigured
                        },
                    );
                }
            }
            ProviderId::OpenRouter => self.api_key(provider)?,
        };
        let Some(key) = key else {
            return Ok(ProviderConnectionTest::NotConfigured);
        };
        let url = match provider {
            ProviderId::Xai => &self.xai_models_url,
            ProviderId::OpenAi => &self.openai_models_url,
            ProviderId::OpenRouter => &self.openrouter_models_url,
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
                // For OpenRouter, the same `/key` probe that validates the
                // credential also returns credits balance. Parse it
                // defensively — all fields optional; the raw body is never
                // retained or logged. A parse failure degrades to `None`
                // (Connected without a credits string).
                let credits = if provider == ProviderId::OpenRouter {
                    let info = response
                        .bytes()
                        .await
                        .ok()
                        .as_deref()
                        .and_then(parse_openrouter_credits);
                    // Emit the low-balance telemetry event when the remaining
                    // credits fall below the threshold. The balance is
                    // bucketed — never exact — and the event is product-only
                    // (not surfaced to the external OTEL stream unless the
                    // customer has enabled it).
                    if let Some(ref info) = info {
                        let threshold = openrouter_low_credit_threshold();
                        let below = info
                            .remaining_usd
                            .is_some_and(|remaining| remaining < threshold);
                        if below {
                            let bucket = openrouter_credits_bucket(info.remaining_usd);
                            xai_grok_telemetry::session_ctx::log_event(
                                xai_grok_telemetry::events::OpenrouterCredits {
                                    bucket: bucket.to_owned(),
                                },
                            );
                        }
                    }
                    info.map(|info| info.display)
                } else {
                    None
                };
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
                    ProviderId::Xai => {}
                }
                Ok(ProviderConnectionTest::Connected { credits })
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
            match self.refresh_codex_catalog().await {
                Ok(models) if !models.is_empty() => {
                    tracing::info!(
                        model_count = models.len(),
                        "ChatGPT OAuth model catalog refreshed"
                    );
                }
                Ok(_) => {}
                Err(error) => {
                    tracing::warn!(%error, "ChatGPT OAuth model catalog refresh failed");
                }
            }
        };
        tokio::join!(refresh_openai, refresh_openrouter, refresh_codex);
    }

    /// Persist the ChatGPT OAuth model allowlist when subscription auth is active.
    pub async fn refresh_codex_catalog(&self) -> Result<Vec<ProviderModelPreset>, ProviderError> {
        if crate::auth::chatgpt_oauth::status(&self.grok_home)
            != crate::auth::chatgpt_oauth::ChatGptOAuthStatus::Connected
        {
            let _ = clear_codex_catalog_cache(&self.grok_home);
            return Ok(Vec::new());
        }
        let presets = static_chatgpt_oauth_presets();
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
}

/// Whether ChatGPT OAuth should use the headless device-code path.
///
/// Device auth when:
/// - `GROK_CHATGPT_DEVICE_AUTH` is set (any value), or
/// - running on Linux with neither `DISPLAY` nor `WAYLAND_DISPLAY`.
///
/// Do **not** treat "no DISPLAY" as headless on macOS/Windows — those
/// platforms never set X11 display vars, and browser PKCE is the default.
pub(crate) fn prefer_chatgpt_device_auth() -> bool {
    if std::env::var_os("GROK_CHATGPT_DEVICE_AUTH").is_some() {
        return true;
    }
    cfg!(target_os = "linux")
        && std::env::var_os("DISPLAY").is_none()
        && std::env::var_os("WAYLAND_DISPLAY").is_none()
}

fn static_chatgpt_oauth_presets() -> Vec<ProviderModelPreset> {
    use crate::auth::chatgpt_oauth::CODEX_RESPONSES_BASE_URL;
    // Allowlisted ChatGPT subscription models (Codex OAuth backend).
    //
    // Context windows here are the **product** caps on
    // `chatgpt.com/backend-api/codex`, not the OpenAI Platform API specs.
    // API models advertise ~1.05M for GPT-5.6 Sol, but the subscription
    // catalog currently caps Sol/Terra at 372k raw (≈353.4k at 95%
    // effective) — see openai/codex#31860. OpenCode mirrors that with
    // limit.input=372_000 / limit.context=500_000 for 5.6 and
    // limit.input=272_000 / limit.context=400_000 for 5.5.
    //
    // Grok only has a single `context_window` (drives compaction). Use the
    // Codex raw catalog window so auto-compact fires before the product
    // truncates mid-turn.
    //
    // Tuple: (api_slug, label, context_window, max_completion_tokens)
    const MODELS: &[(&str, &str, u64, u32)] = &[
        // Codex catalog: context_window / max_context_window = 372_000
        ("gpt-5.6-sol", "GPT-5.6 Sol", 372_000, 128_000),
        ("gpt-5.6-terra", "GPT-5.6 Terra", 372_000, 128_000),
        ("gpt-5.6-luna", "GPT-5.6 Luna", 372_000, 128_000),
        // Codex / OpenCode: 400k product context, ~272k typical input budget
        ("gpt-5.5", "GPT-5.5", 400_000, 128_000),
        ("gpt-5.4", "GPT-5.4", 400_000, 128_000),
        ("gpt-5.4-mini", "GPT-5.4 Mini", 400_000, 128_000),
    ];
    MODELS
        .iter()
        .map(|(model, label, ctx, max_out)| ProviderModelPreset {
            id: format!("chatgpt-{model}"),
            provider: ProviderId::OpenAi,
            label: format!("{label} via ChatGPT"),
            model: (*model).to_owned(),
            base_url: Some(CODEX_RESPONSES_BASE_URL.to_owned()),
            is_agent: false,
            description: Some("ChatGPT subscription (OAuth)".to_owned()),
            context_window: Some(*ctx),
            max_completion_tokens: Some(*max_out),
            supports_tools: true,
            supports_reasoning_effort: true,
            reasoning_efforts: vec![
                "low".to_owned(),
                "medium".to_owned(),
                "high".to_owned(),
                "xhigh".to_owned(),
            ],
            default_reasoning_effort: Some("medium".to_owned()),
        })
        .collect()
}

/// Resolve a key saved by [`ProviderManager`] for the model resolver. This is
/// intentionally crate-private: callers should use the manager, which keeps
/// keys out of all UI DTOs.
pub(crate) fn stored_api_key(
    provider: &super::model_providers::ResolvedModelProvider,
) -> Option<String> {
    use crate::provider_registry::id::ProviderId as ConfiguredProviderId;
    use crate::provider_registry::secrets::{application_key_scope, read_provider_secret};

    let built_in = match (provider.kind, provider.id.as_str()) {
        (ModelProviderKind::Xai, "xai") => Some(ProviderId::Xai),
        (ModelProviderKind::OpenAi, "openai" | "grok_build_openai") => Some(ProviderId::OpenAi),
        (ModelProviderKind::OpenRouter, "openrouter" | "grok_build_openrouter") => {
            Some(ProviderId::OpenRouter)
        }
        _ => None,
    };
    if let Some(built_in) = built_in {
        return credential_lookup_manager().api_key(built_in).ok().flatten();
    }
    let id = ConfiguredProviderId::new(&provider.id).ok()?;
    read_provider_secret(
        &credential_lookup_manager().grok_home,
        &application_key_scope(&id),
    )
    .ok()
    .flatten()
}

/// Resolved OpenAI credential selected for the active model route.
pub(crate) struct StoredOpenAiCredentials {
    pub bearer: String,
    pub base_url: Option<String>,
    pub account_id: Option<String>,
}

pub(crate) fn stored_openai_credentials(
    provider: &super::model_providers::ResolvedModelProvider,
    model_base_url: &str,
) -> Option<StoredOpenAiCredentials> {
    if provider.kind != ModelProviderKind::OpenAi {
        return None;
    }
    let home = credential_lookup_manager().grok_home.clone();
    let is_builtin_openai = matches!(provider.id.as_str(), "openai" | "grok_build_openai");
    if is_builtin_openai && crate::auth::chatgpt_oauth::is_codex_base_url(model_base_url) {
        // Sync path: use the subscription token only for the ChatGPT Codex
        // route. Refresh is performed by the route-scoped pre-turn path.
        let tokens = crate::auth::chatgpt_oauth::read_tokens(&home)
            .ok()
            .flatten()?;
        return Some(StoredOpenAiCredentials {
            bearer: tokens.access_token,
            base_url: Some(crate::auth::chatgpt_oauth::CODEX_RESPONSES_BASE_URL.to_owned()),
            account_id: tokens.account_id,
        });
    }
    // Platform and custom OpenAI-kind endpoints use the separately stored API
    // key even when ChatGPT OAuth is connected.
    let key = stored_api_key(provider)?;
    Some(StoredOpenAiCredentials {
        bearer: key,
        base_url: None,
        account_id: None,
    })
}

pub(crate) fn stored_openai_oauth_account_id() -> Option<String> {
    let home = credential_lookup_manager().grok_home.clone();
    crate::auth::chatgpt_oauth::read_tokens(&home)
        .ok()
        .flatten()
        .and_then(|t| t.account_id)
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
        ModelProviderKind::OpenAiCompatible | ModelProviderKind::Zai => return None,
        ModelProviderKind::Xai => return None,
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

/// Home directory used for provider vault / ChatGPT OAuth token reads.
/// Respects the test-only stored-key home override so session pre-turn paths
/// and credential lookup share one store without touching production profiles.
pub(crate) fn provider_credential_home() -> PathBuf {
    credential_lookup_manager().grok_home.clone()
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
    /// Optional provider-advertised defaults (effort, temperature, …).
    #[serde(default)]
    default_parameters: Option<serde_json::Map<String, serde_json::Value>>,
    /// Optional advertised reasoning-effort levels when present.
    #[serde(default)]
    reasoning_effort_options: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterTopProvider {
    #[serde(default)]
    max_completion_tokens: Option<u64>,
}

/// Default effort ladder for OpenRouter models that advertise reasoning but
/// do not publish an explicit option list. Matches common OR tiers.
const OPENROUTER_DEFAULT_REASONING_EFFORTS: &[&str] = &["low", "medium", "high"];

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
            let (reasoning_efforts, default_reasoning_effort) =
                openrouter_effort_metadata(&model, supports_reasoning_effort);
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
                reasoning_efforts,
                default_reasoning_effort,
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

/// Extract advertised reasoning-effort options and default from an OpenRouter
/// catalog entry. Falls back to a standard low/medium/high ladder when the
/// model supports reasoning but omits an explicit list.
fn openrouter_effort_metadata(
    model: &OpenRouterModel,
    supports_reasoning: bool,
) -> (Vec<String>, Option<String>) {
    if !supports_reasoning {
        return (Vec::new(), None);
    }
    let mut efforts = model
        .reasoning_effort_options
        .clone()
        .unwrap_or_default()
        .into_iter()
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    if efforts.is_empty() {
        efforts = OPENROUTER_DEFAULT_REASONING_EFFORTS
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
    }
    let default = model
        .default_parameters
        .as_ref()
        .and_then(|params| {
            params
                .get("reasoning_effort")
                .or_else(|| params.get("reasoningEffort"))
        })
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            if efforts.iter().any(|e| e == "medium") {
                Some("medium".to_owned())
            } else {
                efforts.first().cloned()
            }
        });
    (efforts, default)
}

fn openrouter_cache_path(grok_home: &Path) -> PathBuf {
    grok_home.join(OPENROUTER_CACHE_FILE)
}

/// Low-credit threshold in USD. When the remaining OpenRouter credits fall
/// below this, the provider status flags it. Configurable via
/// `GROK_OPENROUTER_LOW_CREDIT_USD` for deployments that want a different
/// cutoff. A non-parseable override falls back to the default.
const OPENROUTER_LOW_CREDIT_USD_DEFAULT: f64 = 1.0;
const OPENROUTER_LOW_CREDIT_USD_ENV: &str = "GROK_OPENROUTER_LOW_CREDIT_USD";

fn openrouter_low_credit_threshold() -> f64 {
    std::env::var(OPENROUTER_LOW_CREDIT_USD_ENV)
        .ok()
        .and_then(|raw| raw.parse::<f64>().ok())
        .filter(|value| *value >= 0.0)
        .unwrap_or(OPENROUTER_LOW_CREDIT_USD_DEFAULT)
}

/// Bucketed credits balance for telemetry. Exact balances must never leave
/// the process; the buckets are the only value that reaches the external
/// stream. Kept generic so the credits fetch (Part B) and the telemetry event
/// (Part D) share one definition.
pub fn openrouter_credits_bucket(remaining_usd: Option<f64>) -> &'static str {
    match remaining_usd {
        None => "unknown",
        Some(value) if value < 1.0 => "lt_1",
        Some(value) if value < 10.0 => "1_to_10",
        Some(value) if value < 100.0 => "10_to_100",
        Some(_) => "gte_100",
    }
}

/// Parse OpenRouter's `GET /api/v1/key` response into a display-only credits
/// string and (when available) the numeric remaining balance for telemetry
/// bucketing. The response shape (defensive — all fields optional):
/// ```json
/// { "data": { "limit_remaining": 4.21, "limit": 10.0, "usage": 5.79 } }
/// ```
/// Returns `None` on parse failure or when no balance data is present; the
/// raw body is never retained or logged. When `limit` is missing or the
/// remaining balance is non-decreasing relative to a `null` limit, "unlimited"
/// is surfaced (OpenRouter reports `limit: null` for uncapped keys).
fn parse_openrouter_credits(body: &[u8]) -> Option<OpenRouterCreditsInfo> {
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    let data = value.get("data").unwrap_or(&value);
    let limit_remaining = data.get("limit_remaining").and_then(|v| v.as_f64());
    let limit = data.get("limit").and_then(|v| v.as_f64());
    let usage = data.get("usage").and_then(|v| v.as_f64());

    // No balance fields at all → nothing to display.
    if limit_remaining.is_none() && limit.is_none() && usage.is_none() {
        return None;
    }

    let threshold = openrouter_low_credit_threshold();

    // OpenRouter reports `limit: null` for uncapped (unlimited-credit) keys.
    // Treat an absent/null limit as unlimited regardless of usage.
    let is_unlimited = limit.is_none();

    let display = if is_unlimited {
        "unlimited".to_owned()
    } else {
        // Prefer `limit_remaining`; fall back to `limit - usage` when only
        // those are present. Clamp negatives to 0.
        let remaining = limit_remaining
            .or_else(|| limit.zip(usage).map(|(l, u)| (l - u).max(0.0)))
            .unwrap_or(0.0);
        format!("credits remaining: ${remaining:.2}")
    };

    // Low-credit flag only for capped keys with a concrete remaining balance.
    // Also capture the numeric balance for telemetry bucketing.
    let numeric_remaining = if is_unlimited {
        None
    } else {
        limit_remaining.or_else(|| limit.zip(usage).map(|(l, u)| (l - u).max(0.0)))
    };

    let display = if !is_unlimited
        && let Some(remaining) = numeric_remaining
        && remaining < threshold
    {
        format!("{display} ⚠ low balance")
    } else {
        display
    };

    Some(OpenRouterCreditsInfo {
        display,
        remaining_usd: numeric_remaining,
    })
}

/// Parsed OpenRouter credits info: a display-only string for the provider
/// status and an optional numeric balance for telemetry bucketing. The
/// numeric balance never leaves the process except as a bucket label.
struct OpenRouterCreditsInfo {
    display: String,
    remaining_usd: Option<f64>,
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
) -> Vec<xai_grok_inference_types::ReasoningEffortOption> {
    efforts
        .iter()
        .filter_map(|id| {
            let value = id.parse().ok()?;
            Some(xai_grok_inference_types::ReasoningEffortOption {
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
    #[serial_test::serial]
    fn prefer_chatgpt_device_auth_only_when_forced_or_headless_linux() {
        // Clear both vars so the platform default is observable.
        let _device = EnvGuard::unset("GROK_CHATGPT_DEVICE_AUTH");
        let _display = EnvGuard::unset("DISPLAY");
        let _wayland = EnvGuard::unset("WAYLAND_DISPLAY");

        // Without the force env var: device only on headless Linux.
        if cfg!(target_os = "linux") {
            assert!(
                prefer_chatgpt_device_auth(),
                "Linux with no display must prefer device auth"
            );
            let _d = EnvGuard::set("DISPLAY", ":0");
            assert!(
                !prefer_chatgpt_device_auth(),
                "Linux with DISPLAY must prefer browser PKCE"
            );
        } else {
            assert!(
                !prefer_chatgpt_device_auth(),
                "non-Linux must prefer browser PKCE when force env is unset"
            );
        }

        let _force = EnvGuard::set("GROK_CHATGPT_DEVICE_AUTH", "1");
        assert!(
            prefer_chatgpt_device_auth(),
            "GROK_CHATGPT_DEVICE_AUTH must force device auth"
        );
    }

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

    #[tokio::test]
    async fn openai_oauth_and_api_key_are_reported_together_and_removed_independently() {
        let home = tempfile::tempdir().unwrap();
        let manager = ProviderManager::new(home.path());
        manager
            .set_api_key(ProviderId::OpenAi, "openai-api-key")
            .unwrap();
        crate::auth::chatgpt_oauth::store_tokens(
            home.path(),
            &crate::auth::chatgpt_oauth::ChatGptOAuthTokens {
                access_token: "chatgpt-access".into(),
                refresh_token: "chatgpt-refresh".into(),
                expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
                account_id: Some("account".into()),
                email: None,
            },
        )
        .unwrap();

        let status = manager.status(ProviderId::OpenAi);
        assert_eq!(status.state, ProviderConnectionState::Connected);
        assert!(status.authentication.iter().any(|entry| {
            entry.kind == ProviderAuthenticationKind::ChatGpt
                && entry.state == ProviderConnectionState::Connected
        }));
        assert!(status.authentication.iter().any(|entry| {
            entry.kind == ProviderAuthenticationKind::ApiKey
                && entry.state == ProviderConnectionState::Configured
        }));

        manager.remove_api_key(ProviderId::OpenAi).unwrap();
        let status = manager.status(ProviderId::OpenAi);
        assert_eq!(status.state, ProviderConnectionState::Connected);
        assert_eq!(
            crate::auth::chatgpt_oauth::status(home.path()),
            crate::auth::chatgpt_oauth::ChatGptOAuthStatus::Connected
        );

        manager
            .set_api_key(ProviderId::OpenAi, "replacement-api-key")
            .unwrap();
        manager.chatgpt_oauth_logout().await.unwrap();
        let status = manager.status(ProviderId::OpenAi);
        assert_eq!(status.state, ProviderConnectionState::Configured);
        assert_eq!(
            manager.api_key(ProviderId::OpenAi).unwrap().as_deref(),
            Some("replacement-api-key")
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
    fn openai_accepts_nonblank_api_keys_and_rejects_blank_keys() {
        let home = tempfile::tempdir().unwrap();
        let manager = ProviderManager::new(home.path());
        manager.set_api_key(ProviderId::OpenAi, "x").unwrap();
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
            !models.contains_key("chatgpt-gpt-5.6-sol"),
            "ChatGPT models must not enter the catalog without OAuth"
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
            ProviderId::OpenAi,
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
        assert_eq!(
            reasoner.reasoning_efforts,
            ["low", "medium", "high"],
            "reasoning models without explicit options get the standard ladder"
        );
        assert_eq!(reasoner.default_reasoning_effort.as_deref(), Some("medium"));
        let basic = models
            .iter()
            .find(|model| model.id == "openrouter:acme/basic")
            .unwrap();
        assert!(basic.reasoning_efforts.is_empty());
        assert!(basic.default_reasoning_effort.is_none());
    }

    #[test]
    fn openrouter_catalog_preserves_explicit_effort_options_and_defaults() {
        let fixture = br#"{
          "data": [
            {
              "id": "acme/explicit",
              "supported_parameters": ["reasoning"],
              "reasoning_effort_options": ["minimal", "low", "high"],
              "default_parameters": { "reasoning_effort": "low" }
            }
          ]
        }"#;
        let models = parse_openrouter_catalog(fixture).unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].reasoning_efforts, ["minimal", "low", "high"]);
        assert_eq!(models[0].default_reasoning_effort.as_deref(), Some("low"));
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

    #[tokio::test]
    #[serial_test::serial]
    async fn openrouter_test_connection_surfaces_credits() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let home = tempfile::tempdir().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        // The /key probe and /models catalog refresh both hit the mock.
        let server = std::thread::spawn(move || {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0_u8; 2048];
                let read = stream.read(&mut request).unwrap();
                let request = std::str::from_utf8(&request[..read]).unwrap();
                let body = if request.contains("GET /key") {
                    r#"{"data":{"label":"default","limit_remaining":4.21,"limit":10.0,"usage":5.79}}"#
                } else {
                    r#"{"data":[{"id":"acme/test","context_length":65536,"supported_parameters":["tools"]}]}"#
                };
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .unwrap();
            }
        });
        let manager = ProviderManager::with_endpoints(
            home.path(),
            "https://api.openai.com/v1/models",
            format!("http://{address}/key"),
        )
        .with_openrouter_catalog_url(format!("http://{address}/models"));
        manager
            .set_api_key(ProviderId::OpenRouter, "router-test-key")
            .unwrap();

        let result = manager
            .test_connection(ProviderId::OpenRouter)
            .await
            .unwrap();
        match result {
            ProviderConnectionTest::Connected { credits } => {
                let credits = credits.expect("credits should be surfaced");
                assert!(
                    credits.contains("credits remaining: $4.21"),
                    "got: {credits}"
                );
            }
            other => panic!("expected Connected with credits, got {other:?}"),
        }
        server.join().unwrap();
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn openrouter_test_connection_credits_failure_degrades_gracefully() {
        // /key returns invalid JSON; the connection must still be Connected
        // with credits = None (the failure must not affect Connected state).
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let home = tempfile::tempdir().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0_u8; 2048];
                let _ = stream.read(&mut request).unwrap();
                let body = if request.starts_with(b"GET /key") {
                    "not valid json"
                } else {
                    r#"{"data":[{"id":"acme/test","context_length":65536,"supported_parameters":["tools"]}]}"#
                };
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .unwrap();
            }
        });
        let manager = ProviderManager::with_endpoints(
            home.path(),
            "https://api.openai.com/v1/models",
            format!("http://{address}/key"),
        )
        .with_openrouter_catalog_url(format!("http://{address}/models"));
        manager
            .set_api_key(ProviderId::OpenRouter, "router-test-key")
            .unwrap();

        let result = manager
            .test_connection(ProviderId::OpenRouter)
            .await
            .unwrap();
        match result {
            ProviderConnectionTest::Connected { credits } => {
                assert_eq!(credits, None, "credits parse failure must degrade to None");
            }
            other => panic!("expected Connected, got {other:?}"),
        }
        server.join().unwrap();
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
            crate::inference::ApiBackend::ChatCompletions
        );
        let sampling = super::super::config::inference_config_for_model(
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
    fn openai_oauth_and_api_key_select_credentials_by_model_route() {
        let home = tempfile::tempdir().unwrap();
        let _openai = EnvGuard::unset("OPENAI_API_KEY");
        set_stored_key_home_for_tests(Some(home.path().to_path_buf()));
        let manager = ProviderManager::new(home.path());
        manager
            .set_api_key(ProviderId::OpenAi, "platform-api-key")
            .unwrap();
        crate::auth::chatgpt_oauth::store_tokens(
            home.path(),
            &crate::auth::chatgpt_oauth::ChatGptOAuthTokens {
                access_token: "subscription-oauth-token".into(),
                refresh_token: "refresh".into(),
                expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
                account_id: Some("account".into()),
                email: None,
            },
        )
        .unwrap();

        let raw: toml::Value = toml::from_str("").unwrap();
        let config = super::super::config::Config::new_from_toml_cfg(&raw).unwrap();
        let models = super::super::config::resolve_model_list(&config, None);
        let oauth_model = &models["chatgpt-gpt-5.6-sol"];
        assert!(crate::auth::chatgpt_oauth::is_codex_base_url(
            &oauth_model.info.base_url
        ));
        let oauth = super::super::config::resolve_credentials(oauth_model, None);
        assert_eq!(oauth.api_key.as_deref(), Some("subscription-oauth-token"));
        assert!(crate::auth::chatgpt_oauth::is_codex_base_url(
            &oauth.base_url
        ));

        let platform_entry = super::super::config::ModelEntry {
            info: super::super::config::ModelInfo::fallback("gpt-platform"),
            model_provider: Some(super::super::model_providers::ResolvedModelProvider {
                id: "grok_build_openai".into(),
                kind: ModelProviderKind::OpenAi,
                openrouter_fallback_models: Vec::new(),
                openrouter_provider_preferences: None,
                openrouter_plugins: Vec::new(),
                openrouter_pacing: false,
                command: Vec::new(),
            }),
            api_key: None,
            env_key: None,
            auth_provider: None,
            api_base_url: None,
        };
        let mut platform_entry = platform_entry;
        platform_entry.info.base_url = "https://api.openai.com/v1".into();
        let platform = super::super::config::resolve_credentials(&platform_entry, None);
        assert_eq!(platform.api_key.as_deref(), Some("platform-api-key"));
        assert_eq!(platform.base_url, "https://api.openai.com/v1");
        set_stored_key_home_for_tests(None);
    }

    #[test]
    #[serial_test::serial]
    fn configured_provider_vault_keys_are_isolated_and_used_per_turn() {
        use crate::provider_registry::id::ProviderId as ConfiguredProviderId;
        use crate::provider_registry::secrets::{application_key_scope, store_provider_secret};

        let home = tempfile::tempdir().unwrap();
        set_stored_key_home_for_tests(Some(home.path().to_path_buf()));
        let a = ConfiguredProviderId::new("local_a").unwrap();
        let b = ConfiguredProviderId::new("local_b").unwrap();
        store_provider_secret(home.path(), &application_key_scope(&a), "key-a").unwrap();
        store_provider_secret(home.path(), &application_key_scope(&b), "key-b").unwrap();

        let raw: toml::Value = toml::from_str(
            r#"
            [model_providers.local_a]
            base_url = "https://a.example/v1"
            [model.a]
            model = "shared-model"
            model_provider = "local_a"

            [model_providers.local_b]
            base_url = "https://b.example/v1"
            [model.b]
            model = "shared-model"
            model_provider = "local_b"
            "#,
        )
        .unwrap();
        let config = super::super::config::Config::new_from_toml_cfg(&raw).unwrap();
        let models = super::super::config::resolve_model_list(&config, None);
        let a_creds = super::super::config::resolve_credentials(&models["a"], Some("xai-session"));
        let b_creds = super::super::config::resolve_credentials(&models["b"], Some("xai-session"));
        assert_eq!(a_creds.api_key.as_deref(), Some("key-a"));
        assert_eq!(a_creds.base_url, "https://a.example/v1");
        assert_eq!(b_creds.api_key.as_deref(), Some("key-b"));
        assert_eq!(b_creds.base_url, "https://b.example/v1");
        assert!(models["a"].is_provider_scoped_byok());
        assert!(models["b"].is_provider_scoped_byok());
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
        // OpenAI API-key path: Platform catalog windows (~1.05M for Sol).
        ProviderManager::new(home.path())
            .set_api_key(ProviderId::OpenAi, "sk-test-openai")
            .unwrap();
        let mut config = super::super::config::Config::default();
        ProviderManager::install_model_presets(&mut config);
        let models = super::super::config::resolve_model_list(&config, None);
        assert_eq!(models["openai-gpt-5.6-sol"].info.model, "gpt-5.6-sol");
        assert_eq!(
            models["openai-gpt-5.6-sol"].info.context_window.get(),
            1_050_000,
            "API-key OpenAI presets keep Platform context windows"
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
        set_stored_key_home_for_tests(None);
    }

    #[test]
    fn chatgpt_oauth_presets_use_subscription_context_caps() {
        let presets = static_chatgpt_oauth_presets();
        let by_id: std::collections::HashMap<_, _> =
            presets.into_iter().map(|p| (p.id.clone(), p)).collect();
        assert_eq!(
            by_id["chatgpt-gpt-5.6-sol"].context_window,
            Some(372_000),
            "Codex product catalog caps GPT-5.6 Sol at 372k (not API 1.05M)"
        );
        assert_eq!(by_id["chatgpt-gpt-5.6-terra"].context_window, Some(372_000));
        assert_eq!(by_id["chatgpt-gpt-5.6-luna"].context_window, Some(372_000));
        assert_eq!(by_id["chatgpt-gpt-5.5"].context_window, Some(400_000));
        assert_eq!(by_id["chatgpt-gpt-5.4"].context_window, Some(400_000));
        assert_eq!(by_id["chatgpt-gpt-5.4-mini"].context_window, Some(400_000));
    }

    // removed obsolete codex app-server test

    #[test]
    #[serial_test::serial]
    fn chatgpt_models_enter_and_leave_picker_with_oauth_tokens() {
        let home = tempfile::tempdir().unwrap();
        set_stored_key_home_for_tests(Some(home.path().to_path_buf()));
        let config = super::super::config::Config::default();

        assert!(
            !super::super::config::resolve_model_list(&config, None)
                .contains_key("chatgpt-gpt-5.6-sol")
        );
        crate::auth::chatgpt_oauth::store_tokens(
            home.path(),
            &crate::auth::chatgpt_oauth::ChatGptOAuthTokens {
                access_token: "access".into(),
                refresh_token: "refresh".into(),
                expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
                account_id: None,
                email: None,
            },
        )
        .unwrap();
        assert!(
            super::super::config::resolve_model_list(&config, None)
                .contains_key("chatgpt-gpt-5.6-sol")
        );

        crate::auth::chatgpt_oauth::clear_tokens(home.path()).unwrap();
        assert!(
            !super::super::config::resolve_model_list(&config, None)
                .contains_key("chatgpt-gpt-5.6-sol")
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

    #[tokio::test]
    async fn chatgpt_oauth_status_and_logout_use_native_store() {
        let home = tempfile::tempdir().unwrap();
        let manager = ProviderManager::new(home.path());
        crate::auth::chatgpt_oauth::store_tokens(
            home.path(),
            &crate::auth::chatgpt_oauth::ChatGptOAuthTokens {
                access_token: "access".into(),
                refresh_token: "refresh".into(),
                expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
                account_id: None,
                email: None,
            },
        )
        .unwrap();

        assert_eq!(
            manager.codex_status().await.unwrap().state,
            ProviderConnectionState::Connected
        );
        manager.codex_logout().await.unwrap();
        assert_eq!(
            manager.codex_status().await.unwrap().state,
            ProviderConnectionState::NotConfigured
        );
    }

    #[tokio::test]
    async fn disconnected_chatgpt_removes_stale_oauth_catalog_cache() {
        let home = tempfile::tempdir().unwrap();
        save_codex_catalog_cache(home.path(), &static_chatgpt_oauth_presets()).unwrap();
        let manager = ProviderManager::new(home.path());

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

    #[test]
    fn parse_openrouter_credits_remaining_with_limit() {
        let body = br#"{"data":{"label":"default","limit_remaining":4.21,"limit":10.0,"usage":5.79,"is_free_tier":false}}"#;
        let credits = parse_openrouter_credits(body).expect("should parse");
        assert!(
            credits.display.contains("credits remaining: $4.21"),
            "got: {}",
            credits.display
        );
        assert!(
            !credits.display.contains("low"),
            "not below threshold: {}",
            credits.display
        );
        assert_eq!(credits.remaining_usd, Some(4.21));
    }

    #[test]
    fn parse_openrouter_credits_low_balance_flagged() {
        // Default threshold is $1.0; $0.50 is below it.
        let body = br#"{"data":{"limit_remaining":0.5,"limit":10.0,"usage":9.5}}"#;
        let credits = parse_openrouter_credits(body).expect("should parse");
        assert!(
            credits.display.contains("low"),
            "low-balance flag expected: {}",
            credits.display
        );
        assert_eq!(credits.remaining_usd, Some(0.5));
    }

    #[test]
    fn parse_openrouter_credits_unlimited_when_limit_null() {
        // OpenRouter reports `limit: null` for uncapped keys.
        let body =
            br#"{"data":{"label":"default","limit_remaining":null,"limit":null,"usage":123.45}}"#;
        let credits = parse_openrouter_credits(body).expect("should parse");
        assert_eq!(credits.display, "unlimited");
        assert_eq!(
            credits.remaining_usd, None,
            "unlimited keys have no balance"
        );
    }

    #[test]
    fn parse_openrouter_credits_falls_back_to_limit_minus_usage() {
        let body = br#"{"data":{"limit":10.0,"usage":7.75}}"#;
        let credits = parse_openrouter_credits(body).expect("should parse");
        assert!(
            credits.display.contains("credits remaining: $2.25"),
            "got: {}",
            credits.display
        );
        assert_eq!(credits.remaining_usd, Some(2.25));
    }

    #[test]
    fn parse_openrouter_credits_returns_none_on_missing_data() {
        let body = br#"{"data":{"label":"default"}}"#;
        assert!(parse_openrouter_credits(body).is_none());
    }

    #[test]
    fn parse_openrouter_credits_returns_none_on_invalid_json() {
        let body = b"not json at all";
        assert!(parse_openrouter_credits(body).is_none());
    }

    #[test]
    fn parse_openrouter_credits_returns_none_on_empty_body() {
        assert!(parse_openrouter_credits(b"").is_none());
    }

    #[test]
    fn parse_openrouter_credits_never_logs_raw_body() {
        // A key-shaped string in the body must not appear in the output.
        let body = br#"{"data":{"limit_remaining":4.21,"limit":10.0,"usage":5.79,"sk-secret":"sk-CANARYabcdef123456"}}"#;
        let credits = parse_openrouter_credits(body).expect("should parse");
        assert!(
            !credits.display.contains("sk-CANARY"),
            "raw body leaked into credits: {}",
            credits.display
        );
    }

    #[test]
    fn openrouter_credits_bucket_boundaries() {
        assert_eq!(openrouter_credits_bucket(None), "unknown");
        assert_eq!(openrouter_credits_bucket(Some(0.5)), "lt_1");
        assert_eq!(openrouter_credits_bucket(Some(5.0)), "1_to_10");
        assert_eq!(openrouter_credits_bucket(Some(50.0)), "10_to_100");
        assert_eq!(openrouter_credits_bucket(Some(100.0)), "gte_100");
        assert_eq!(openrouter_credits_bucket(Some(999.0)), "gte_100");
    }

    /// Phase 1 capability plumbing: the catalog `supports_tools` flag must
    /// survive `install_model_presets` into the resolved `ModelInfo` entries.
    /// A cached OpenRouter catalog with a tools-capable and a tools-incapable
    /// model must preserve both values (`Some(true)` / `Some(false)`) so
    /// later phases can gate agent-safe OpenRouter models.
    #[test]
    #[serial_test::serial]
    fn install_presets_preserves_openrouter_supports_tools_flag() {
        let home = tempfile::tempdir().unwrap();
        let _openrouter = EnvGuard::unset("OPENROUTER_API_KEY");
        set_stored_key_home_for_tests(Some(home.path().to_path_buf()));
        reset_openrouter_refresh_guard_for_tests();

        // One catalog model advertises tool calling (`tools`), one does not.
        let cached_models = parse_openrouter_catalog(
            br#"{"data":[
                {"id":"acme/tools","context_length":128000,"supported_parameters":["tools"]},
                {"id":"acme/notools","context_length":32000,"supported_parameters":["reasoning"]}
            ]}"#,
        )
        .unwrap();
        assert!(
            cached_models.iter().any(|m| m.supports_tools),
            "fixture must include a tools-capable model"
        );
        assert!(
            cached_models.iter().any(|m| !m.supports_tools),
            "fixture must include a tools-incapable model"
        );
        write_openrouter_cache(
            home.path(),
            &cached_models,
            Some(current_epoch_secs().unwrap()),
        );

        // Storing an OpenRouter key makes the catalog visible to install.
        ProviderManager::new(home.path())
            .set_api_key(ProviderId::OpenRouter, "router-key")
            .unwrap();

        let mut config = super::super::config::Config::default();
        ProviderManager::install_model_presets(&mut config);
        let models = super::super::config::resolve_model_list(&config, None);

        let tools_model = &models["openrouter:acme/tools"].info;
        assert_eq!(
            tools_model.supports_tools,
            Some(true),
            "tools-capable catalog entry must preserve supports_tools = Some(true)"
        );
        let notools_model = &models["openrouter:acme/notools"].info;
        assert_eq!(
            notools_model.supports_tools,
            Some(false),
            "tools-incapable catalog entry must preserve supports_tools = Some(false)"
        );

        set_stored_key_home_for_tests(None);
    }
}
