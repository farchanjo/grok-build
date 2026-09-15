//! Search-backend selection.
//!
//! Resolves which backend `web_search` should talk to, from the environment and
//! the raw `config.toml` table (no typed-config fields, same approach as
//! `xai_grok_voice::VoiceConfig::from_config_table`):
//!
//! | Setting | Precedence |
//! | --- | --- |
//! | provider | `GROK_SEARCH_PROVIDER` > `[search] provider` > `xai` |
//! | base URL | `GROK_SEARCH_BASE_URL` > `[search] base_url` |
//! | API key | `GROK_SEARCH_API_KEY` > the env var named by `[search] api_key_env` |
//! | model (xai only) | `GROK_SEARCH_MODEL` > `[search] model` |
//!
//! With none of those set, [`SearchSelection::to_config`] returns `None` and the
//! caller keeps today's xAI model-route resolution byte-for-byte. As soon as a
//! non-xAI provider is named, the xAI credential and the `models.web_search`
//! route are ignored entirely.
//!
//! A selection is turned into a [`WebSearchConfig::External`] even when it is
//! incomplete: the backend then reports which knob is missing at call time.

use indexmap::IndexMap;

use super::backends::SearchProvider;
use super::types::WebSearchConfig;

/// Env var naming the backend (`xai`, `searxng`, `tavily`, `brave`).
pub const ENV_PROVIDER: &str = "GROK_SEARCH_PROVIDER";
/// Env var overriding the backend base URL.
pub const ENV_BASE_URL: &str = "GROK_SEARCH_BASE_URL";
/// Env var carrying the backend API key.
pub const ENV_API_KEY: &str = "GROK_SEARCH_API_KEY";
/// Env var overriding the `xai` backend model.
pub const ENV_MODEL: &str = "GROK_SEARCH_MODEL";

/// Config table holding the same keys.
pub const CONFIG_SECTION: &str = "search";
pub const CONFIG_PROVIDER: &str = "provider";
pub const CONFIG_BASE_URL: &str = "base_url";
/// Config key naming the *env var* that holds the key, so no secret is written
/// into `config.toml`.
pub const CONFIG_API_KEY_ENV: &str = "api_key_env";
pub const CONFIG_MODEL: &str = "model";

/// Environment lookup seam. Tests inject a map; production reads the process.
pub trait SearchEnv: Send + Sync {
    fn var(&self, name: &str) -> Option<String>;
}

/// The process environment.
pub struct ProcessEnv;

impl SearchEnv for ProcessEnv {
    fn var(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }
}

/// The caller's resolved `models.web_search` route, used only as a fallback when
/// the selected provider is `xai` (a non-xAI backend ignores xAI auth).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct XaiRouteFallback {
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub extra_headers: IndexMap<String, String>,
}

/// What the environment and config selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchSelection {
    /// Effective provider id, lowercased. `xai` unless env/config named another.
    provider: String,
    /// `true` when env or config named the provider.
    provider_explicit: bool,
    /// `true` when nothing at all was configured: keep today's xAI route path.
    default_xai_route: bool,
    base_url: Option<String>,
    api_key: Option<String>,
    model: Option<String>,
    extra_headers: IndexMap<String, String>,
}

impl SearchSelection {
    /// Effective provider id.
    pub fn provider(&self) -> &str {
        &self.provider
    }

    /// Whether the provider was named explicitly rather than defaulted.
    pub fn provider_is_explicit(&self) -> bool {
        self.provider_explicit
    }

    /// Base URL, if one was configured.
    pub fn base_url(&self) -> Option<&str> {
        self.base_url.as_deref()
    }

    /// API key, if one was configured.
    pub fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }

    /// Model override for the `xai` wire shape, if one was configured.
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    /// `true` when the caller must keep today's xAI model-route resolution.
    pub fn uses_default_xai_route(&self) -> bool {
        self.default_xai_route
    }

    /// `true` when the selected backend is the `xai` one, including the
    /// unset default.
    ///
    /// Only the `xai` wire shape shares the model route with server-side
    /// (backend) search. A session that selected anything else must keep the
    /// local `web_search` tool reachable, because backend search knows nothing
    /// about the selected backend: the hosted tool would win the turn and the
    /// request would never reach it.
    pub fn is_xai_backend(&self) -> bool {
        self.provider == SearchProvider::Xai.as_str()
    }

    /// The config to hand to the tool, or `None` for the default xAI route path.
    pub fn to_config(&self) -> Option<WebSearchConfig> {
        if self.default_xai_route {
            return None;
        }
        Some(WebSearchConfig::External {
            provider: self.provider.clone(),
            base_url: self.base_url.clone().unwrap_or_default(),
            api_key: self.api_key.clone(),
            model: self.model.clone().unwrap_or_default(),
            extra_headers: self.extra_headers.clone(),
        })
    }
}

fn non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn config_str<'a>(table: Option<&'a xai_grok_config::toml::Table>, key: &str) -> Option<&'a str> {
    table?.get(key)?.as_str()
}

/// Resolve the selected search backend.
///
/// `raw_config` is the merged `config.toml` root; `xai_route` is the caller's
/// resolved `models.web_search` route, if it has one.
pub fn resolve_search_backend(
    env: &dyn SearchEnv,
    raw_config: Option<&xai_grok_config::toml::Table>,
    xai_route: Option<&XaiRouteFallback>,
) -> SearchSelection {
    let section = raw_config
        .and_then(|root| root.get(CONFIG_SECTION))
        .and_then(|value| value.as_table());

    let env_provider = non_empty(env.var(ENV_PROVIDER).as_deref());
    let config_provider = non_empty(config_str(section, CONFIG_PROVIDER));
    let provider_explicit = env_provider.is_some() || config_provider.is_some();
    let provider = env_provider
        .clone()
        .or(config_provider.clone())
        .map(|provider| provider.to_ascii_lowercase())
        .unwrap_or_else(|| SearchProvider::Xai.as_str().to_string());
    if provider_explicit && SearchProvider::parse(&provider).is_none() {
        tracing::warn!(
            provider = %provider,
            "unknown search provider; web_search will report it at call time"
        );
    }

    let base_url = non_empty(env.var(ENV_BASE_URL).as_deref())
        .or_else(|| non_empty(config_str(section, CONFIG_BASE_URL)));
    let api_key = non_empty(env.var(ENV_API_KEY).as_deref()).or_else(|| {
        let key_env = non_empty(config_str(section, CONFIG_API_KEY_ENV))?;
        let value = non_empty(env.var(&key_env).as_deref());
        if value.is_none() {
            tracing::warn!(
                api_key_env = %key_env,
                "`[search] api_key_env` names an unset environment variable"
            );
        }
        value
    });
    let model = non_empty(env.var(ENV_MODEL).as_deref())
        .or_else(|| non_empty(config_str(section, CONFIG_MODEL)));

    let default_xai_route =
        !provider_explicit && base_url.is_none() && api_key.is_none() && model.is_none();

    // The xAI route is a fallback for the xai wire shape only: a non-xAI backend
    // must not inherit an xAI credential or endpoint.
    let route = if provider == SearchProvider::Xai.as_str() {
        xai_route
    } else {
        None
    };
    SearchSelection {
        provider,
        provider_explicit,
        default_xai_route,
        base_url: base_url.or_else(|| route.and_then(|route| route.base_url.clone())),
        api_key: api_key.or_else(|| route.and_then(|route| route.api_key.clone())),
        model: model.or_else(|| route.and_then(|route| route.model.clone())),
        extra_headers: route
            .map(|route| route.extra_headers.clone())
            .unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Deterministic environment for the precedence tests.
    struct MapEnv(HashMap<String, String>);

    impl MapEnv {
        fn new(pairs: &[(&str, &str)]) -> Self {
            Self(
                pairs
                    .iter()
                    .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
                    .collect(),
            )
        }
    }

    impl SearchEnv for MapEnv {
        fn var(&self, name: &str) -> Option<String> {
            self.0.get(name).cloned()
        }
    }

    fn table(toml_source: &str) -> xai_grok_config::toml::Table {
        toml_source.parse().expect("test config parses")
    }

    #[test]
    fn nothing_configured_keeps_the_default_xai_route() {
        let env = MapEnv::new(&[]);
        let selection = resolve_search_backend(&env, None, None);
        assert!(selection.uses_default_xai_route());
        assert_eq!(selection.provider(), "xai");
        assert!(!selection.provider_is_explicit());
        assert_eq!(selection.to_config(), None);
    }

    /// Only the `xai` selection (explicit or defaulted) shares the model route, so
    /// only it may be shadowed by server-side search. An unknown provider is
    /// reported at call time, but it must still keep the local tool reachable.
    #[test]
    fn only_the_xai_selection_is_the_xai_backend() {
        assert!(resolve_search_backend(&MapEnv::new(&[]), None, None).is_xai_backend());
        assert!(
            resolve_search_backend(&MapEnv::new(&[(ENV_PROVIDER, "xai")]), None, None)
                .is_xai_backend()
        );
        for provider in ["searxng", "tavily", "brave", "duckduckgo"] {
            assert!(
                !resolve_search_backend(&MapEnv::new(&[(ENV_PROVIDER, provider)]), None, None)
                    .is_xai_backend(),
                "{provider} must keep the local web_search tool reachable"
            );
        }
    }

    /// The default route is only preserved when *nothing* is set; a lone base
    /// URL must not silently fall back to the route endpoint.
    #[test]
    fn a_lone_env_knob_is_enough_to_leave_the_default_route() {
        let env = MapEnv::new(&[(ENV_BASE_URL, "https://gateway.example/v1")]);
        let selection = resolve_search_backend(&env, None, None);
        assert!(!selection.uses_default_xai_route());
        assert_eq!(selection.provider(), "xai");
        let config = selection.to_config().expect("external config");
        assert!(matches!(
            config,
            WebSearchConfig::External { ref provider, .. } if provider == "xai"
        ));
    }

    #[test]
    fn env_provider_beats_config_provider_beats_default() {
        let config = table("[search]\nprovider = \"brave\"\n");
        let selection = resolve_search_backend(&MapEnv::new(&[]), Some(&config), None);
        assert_eq!(selection.provider(), "brave");
        assert!(selection.provider_is_explicit());

        let env = MapEnv::new(&[(ENV_PROVIDER, "searxng")]);
        let selection = resolve_search_backend(&env, Some(&config), None);
        assert_eq!(selection.provider(), "searxng");

        let selection = resolve_search_backend(&MapEnv::new(&[]), None, None);
        assert_eq!(selection.provider(), "xai");
    }

    #[test]
    fn env_base_url_and_key_beat_their_config_twins() {
        let config = table(
            "[search]\nprovider = \"tavily\"\nbase_url = \"https://config.example\"\napi_key_env = \"MY_TAVILY_KEY\"\n",
        );
        let env = MapEnv::new(&[
            (ENV_BASE_URL, "https://env.example"),
            (ENV_API_KEY, "env-key"),
            ("MY_TAVILY_KEY", "config-key"),
        ]);
        let selection = resolve_search_backend(&env, Some(&config), None);
        assert_eq!(selection.base_url(), Some("https://env.example"));
        assert_eq!(selection.api_key(), Some("env-key"));

        // Without the env tier, the config values apply (including the
        // `api_key_env` indirection).
        let env = MapEnv::new(&[("MY_TAVILY_KEY", "config-key")]);
        let selection = resolve_search_backend(&env, Some(&config), None);
        assert_eq!(selection.base_url(), Some("https://config.example"));
        assert_eq!(selection.api_key(), Some("config-key"));
    }

    /// `api_key_env` names a variable; a missing one must not fabricate a key.
    #[test]
    fn api_key_env_naming_an_unset_variable_yields_no_key() {
        let config = table("[search]\nprovider = \"tavily\"\napi_key_env = \"MISSING_KEY\"\n");
        let selection = resolve_search_backend(&MapEnv::new(&[]), Some(&config), None);
        assert_eq!(selection.api_key(), None);
        let config = selection.to_config().expect("external config");
        assert!(matches!(
            config,
            WebSearchConfig::External { api_key: None, .. }
        ));
    }

    /// SearXNG is keyless: a base URL alone is a complete selection.
    #[test]
    fn searxng_selection_needs_no_key() {
        let env = MapEnv::new(&[
            (ENV_PROVIDER, "searxng"),
            (ENV_BASE_URL, "http://localhost:8888"),
        ]);
        let selection = resolve_search_backend(&env, None, None);
        assert_eq!(selection.api_key(), None);
        let config = selection.to_config().expect("external config");
        match config {
            WebSearchConfig::External {
                provider,
                base_url,
                api_key,
                ..
            } => {
                assert_eq!(provider, "searxng");
                assert_eq!(base_url, "http://localhost:8888");
                assert!(api_key.is_none());
            }
            other => panic!("expected External, got {other:?}"),
        }
    }

    /// A non-xAI provider ignores the xAI route entirely — that is what makes
    /// the tool usable with no xAI credential.
    #[test]
    fn non_xai_provider_ignores_the_xai_route() {
        let route = XaiRouteFallback {
            base_url: Some("https://api.x.ai/v1".to_string()),
            api_key: Some("xai-key".to_string()),
            model: Some("grok-4-fast".to_string()),
            extra_headers: IndexMap::new(),
        };
        let env = MapEnv::new(&[(ENV_PROVIDER, "brave")]);
        let selection = resolve_search_backend(&env, None, Some(&route));
        assert_eq!(selection.base_url(), None);
        assert_eq!(selection.api_key(), None);
        assert_eq!(selection.model(), None);
    }

    /// An `xai` provider *does* fall back to the route, so a bare
    /// `GROK_SEARCH_BASE_URL` still authenticates like today.
    #[test]
    fn xai_provider_falls_back_to_the_route_for_unset_settings() {
        let route = XaiRouteFallback {
            base_url: Some("https://api.x.ai/v1".to_string()),
            api_key: Some("xai-key".to_string()),
            model: Some("grok-4-fast".to_string()),
            extra_headers: IndexMap::from([("x-grok-client".to_string(), "1".to_string())]),
        };
        let env = MapEnv::new(&[(ENV_BASE_URL, "https://gateway.example/v1")]);
        let selection = resolve_search_backend(&env, None, Some(&route));
        assert_eq!(selection.base_url(), Some("https://gateway.example/v1"));
        assert_eq!(selection.api_key(), Some("xai-key"));
        assert_eq!(selection.model(), Some("grok-4-fast"));
    }

    /// Blank values fall through to the next tier instead of counting as set.
    #[test]
    fn blank_values_fall_through() {
        let config = table("[search]\nprovider = \"  \"\nbase_url = \"\"\n");
        let env = MapEnv::new(&[(ENV_PROVIDER, "   ")]);
        let selection = resolve_search_backend(&env, Some(&config), None);
        assert_eq!(selection.provider(), "xai");
        assert!(selection.uses_default_xai_route());
    }

    #[test]
    fn provider_ids_are_normalized() {
        let env = MapEnv::new(&[(ENV_PROVIDER, "  SearXNG ")]);
        let selection = resolve_search_backend(&env, None, None);
        assert_eq!(selection.provider(), "searxng");
    }

    /// An unknown provider still produces a config, so the tool registers and
    /// reports the bad id instead of disappearing.
    #[test]
    fn unknown_provider_still_produces_a_config() {
        let env = MapEnv::new(&[(ENV_PROVIDER, "duckduckgo")]);
        let selection = resolve_search_backend(&env, None, None);
        let config = selection.to_config().expect("external config");
        assert!(matches!(
            config,
            WebSearchConfig::External { ref provider, .. } if provider == "duckduckgo"
        ));
        assert!(config.is_enabled());
    }

    #[test]
    fn env_model_beats_config_model() {
        let config = table("[search]\nmodel = \"config-model\"\n");
        let env = MapEnv::new(&[(ENV_MODEL, "env-model")]);
        assert_eq!(
            resolve_search_backend(&env, Some(&config), None).model(),
            Some("env-model")
        );
        assert_eq!(
            resolve_search_backend(&MapEnv::new(&[]), Some(&config), None).model(),
            Some("config-model")
        );
    }
}
