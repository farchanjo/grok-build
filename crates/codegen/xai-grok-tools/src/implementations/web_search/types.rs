use indexmap::IndexMap;

/// Configuration for the web search tool.
///
/// Use `Disabled` when no API key is available or web search should be turned off.
/// Use `Enabled { … }` to provide credentials and endpoint configuration for the
/// xAI Responses API.
///
/// `External` selects a non-xAI search backend (`searxng`, `tavily`, `brave`) or
/// an explicit `xai` override, resolved from `GROK_SEARCH_PROVIDER` /
/// `[search] provider` by
/// [`crate::implementations::web_search::factory`]. An `External` config never
/// needs an xAI credential or a `models.web_search` route, and it is still
/// "enabled" so the tool registers; a missing base URL or key is reported by the
/// backend instead of silently degrading to `Disabled`.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum WebSearchConfig {
    #[default]
    Disabled,
    Enabled {
        api_key: String,
        base_url: String,
        model: String,
        #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
        extra_headers: IndexMap<String, String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        alpha_test_key: Option<String>,
    },
    /// A backend selected by the search-provider factory.
    ///
    /// Additive on the ACP wire: an older client that only knows `disabled` /
    /// `enabled` still parses the payload, and every field is defaulted so a
    /// minimal `{"status": "external", "provider": "searxng"}` is valid.
    External {
        /// Backend id (`xai`, `searxng`, `tavily`, `brave`). Never empty.
        provider: String,
        /// Endpoint root. Empty when the user configured none; the backend then
        /// reports which knob to set.
        #[serde(default)]
        base_url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        api_key: Option<String>,
        /// Only the `xai` backend uses this.
        #[serde(default, skip_serializing_if = "String::is_empty")]
        model: String,
        #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
        extra_headers: IndexMap<String, String>,
    },
}

impl WebSearchConfig {
    /// Returns `true` when the config is not `Disabled`.
    ///
    /// `External` counts as enabled on purpose: the tool must register even when
    /// no xAI credential and no `models.web_search` route resolve, so a selected
    /// third-party backend can explain what it is missing at call time.
    pub fn is_enabled(&self) -> bool {
        !matches!(self, Self::Disabled)
    }

    /// Returns `true` when the config is the `Disabled` variant.
    pub fn is_disabled(&self) -> bool {
        matches!(self, Self::Disabled)
    }

    /// Backend id for `External`, `"xai"` for `Enabled`, `None` for `Disabled`.
    pub fn provider(&self) -> Option<&str> {
        match self {
            Self::Disabled => None,
            Self::Enabled { .. } => Some("xai"),
            Self::External { provider, .. } => Some(provider),
        }
    }

    /// Return a copy safe for returning to clients.
    ///
    /// The `api_key` is replaced with `"***REDACTED***"` and the optional
    /// extra access key field is stripped.
    pub fn redacted(&self) -> Self {
        match self {
            Self::Disabled => Self::Disabled,
            Self::Enabled {
                base_url,
                model,
                extra_headers,
                ..
            } => Self::Enabled {
                api_key: "***REDACTED***".to_string(),
                base_url: base_url.clone(),
                model: model.clone(),
                extra_headers: extra_headers.clone(),
                alpha_test_key: None,
            },
            Self::External {
                provider,
                base_url,
                api_key,
                model,
                extra_headers,
            } => Self::External {
                provider: provider.clone(),
                base_url: base_url.clone(),
                api_key: api_key.as_ref().map(|_| "***REDACTED***".to_string()),
                model: model.clone(),
                extra_headers: extra_headers.clone(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default_is_disabled() {
        let config = WebSearchConfig::default();
        assert!(!config.is_enabled());
    }

    #[test]
    fn test_config_enabled() {
        let config = WebSearchConfig::Enabled {
            api_key: "test-key".to_string(),
            base_url: "https://api.x.ai/v1".to_string(),
            model: "test-web-search-model".to_string(),
            extra_headers: IndexMap::new(),
            alpha_test_key: None,
        };
        assert!(config.is_enabled());
    }

    #[test]
    fn test_config_redacted() {
        let mut headers = IndexMap::new();
        headers.insert("X-Custom".to_string(), "value".to_string());
        let config = WebSearchConfig::Enabled {
            api_key: "secret-key-12345".to_string(),
            base_url: "https://api.x.ai/v1".to_string(),
            model: "test-web-search-model".to_string(),
            extra_headers: headers,
            alpha_test_key: Some("alpha-secret".to_string()),
        };
        let redacted = config.redacted();
        match redacted {
            WebSearchConfig::Enabled {
                api_key,
                base_url,
                model,
                extra_headers,
                alpha_test_key,
            } => {
                assert_eq!(api_key, "***REDACTED***");
                assert_eq!(base_url, "https://api.x.ai/v1");
                assert_eq!(model, "test-web-search-model");
                assert_eq!(extra_headers.get("X-Custom").unwrap(), "value");
                assert!(alpha_test_key.is_none());
            }
            _ => panic!("Expected Enabled variant"),
        }
    }

    #[test]
    fn test_config_serde_roundtrip() {
        let config = WebSearchConfig::Enabled {
            api_key: "key".to_string(),
            base_url: "https://api.x.ai/v1".to_string(),
            model: "test-web-search-model".to_string(),
            extra_headers: IndexMap::new(),
            alpha_test_key: None,
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: WebSearchConfig = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_enabled());
    }

    /// `External` is a registered tool: `is_enabled()` is what the builder and
    /// the registry gate on, and the whole point is that a non-xAI backend
    /// registers without an xAI credential.
    #[test]
    fn test_config_external_is_enabled() {
        let config = WebSearchConfig::External {
            provider: "searxng".to_string(),
            base_url: "http://localhost:8888".to_string(),
            api_key: None,
            model: String::new(),
            extra_headers: IndexMap::new(),
        };
        assert!(config.is_enabled());
        assert!(!config.is_disabled());
        assert_eq!(config.provider(), Some("searxng"));
    }

    #[test]
    fn test_config_enabled_provider_is_xai() {
        let config = WebSearchConfig::Enabled {
            api_key: "key".to_string(),
            base_url: "https://api.x.ai/v1".to_string(),
            model: "test-web-search-model".to_string(),
            extra_headers: IndexMap::new(),
            alpha_test_key: None,
        };
        assert_eq!(config.provider(), Some("xai"));
        assert_eq!(WebSearchConfig::Disabled.provider(), None);
    }

    /// The wire shape is additive: a minimal payload with only a provider
    /// deserializes, so an external backend selected with no settings at all
    /// still reports *why* instead of vanishing into `Disabled`.
    #[test]
    fn test_config_external_deserializes_from_minimal_payload() {
        let json = r#"{"status": "external", "provider": "brave"}"#;
        let config: WebSearchConfig = serde_json::from_str(json).unwrap();
        match config {
            WebSearchConfig::External {
                provider,
                base_url,
                api_key,
                model,
                extra_headers,
            } => {
                assert_eq!(provider, "brave");
                assert!(base_url.is_empty());
                assert!(api_key.is_none());
                assert!(model.is_empty());
                assert!(extra_headers.is_empty());
            }
            other => panic!("expected External, got {other:?}"),
        }
    }

    #[test]
    fn test_config_external_serde_roundtrip_and_redaction() {
        let config = WebSearchConfig::External {
            provider: "tavily".to_string(),
            base_url: "https://api.tavily.com".to_string(),
            api_key: Some("tvly-secret".to_string()),
            model: String::new(),
            extra_headers: IndexMap::new(),
        };
        let json = serde_json::to_string(&config).unwrap();
        // The wire payload keeps the real key; `redacted()` is what strips it.
        assert!(json.contains("tvly-secret"));
        let parsed: WebSearchConfig = serde_json::from_str(&json).unwrap();
        match parsed {
            WebSearchConfig::External {
                provider, api_key, ..
            } => {
                assert_eq!(provider, "tavily");
                assert_eq!(api_key.as_deref(), Some("tvly-secret"));
            }
            other => panic!("expected External, got {other:?}"),
        }
        match config.redacted() {
            WebSearchConfig::External { api_key, .. } => {
                assert_eq!(api_key.as_deref(), Some("***REDACTED***"));
            }
            other => panic!("expected External, got {other:?}"),
        }
    }

    /// A keyless backend keeps `api_key: None` through redaction instead of
    /// inventing a redacted placeholder.
    #[test]
    fn test_config_external_redaction_keeps_absent_key_absent() {
        let config = WebSearchConfig::External {
            provider: "searxng".to_string(),
            base_url: "http://localhost:8888".to_string(),
            api_key: None,
            model: String::new(),
            extra_headers: IndexMap::new(),
        };
        match config.redacted() {
            WebSearchConfig::External { api_key, .. } => assert!(api_key.is_none()),
            other => panic!("expected External, got {other:?}"),
        }
    }

    #[test]
    fn test_config_deserialize_from_set_options_payload() {
        let json = r#"{
            "status": "enabled",
            "api_key": "xai-abc123",
            "base_url": "https://api.x.ai/v1",
            "model": "test-web-search-model"
        }"#;
        let config: WebSearchConfig = serde_json::from_str(json).unwrap();
        assert!(config.is_enabled());
    }
}
