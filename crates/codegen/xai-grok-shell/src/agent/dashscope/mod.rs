//! First-class Alibaba DashScope / Model Studio integration (`dashscope` profile).
//!
//! Base URL: `https://dashscope-intl.aliyuncs.com/compatible-mode/v1`
//! (international OpenAI-compatible endpoint). A dedicated MaaS workspace
//! (e.g. `https://ws-<id>.<region>.maas.aliyuncs.com/compatible-mode/v1`) is
//! used by pointing `base_url` at it; nothing else changes.
//!
//! The compatible-mode wire is OpenAI-standard: reasoning streams in
//! `delta.reasoning_content` and tool calls use canonical shapes. This module
//! owns the Qwen3 hybrid-thinking request-body extensions (`enable_thinking` /
//! `thinking_budget`) and the built-in provider profile. Credentials use a
//! plain bearer key (`DASHSCOPE_API_KEY`); they are never stored in TOML.

use crate::agent::model_providers::{ModelProviderConfig, ModelProviderKind};
use crate::inference::ApiBackend;
use indexmap::IndexMap;

/// Stable config / CLI / TUI provider id for the built-in DashScope profile.
pub const DASHSCOPE_PROVIDER_ID: &str = "dashscope";

/// Verified general compatible-mode base URL (international region).
pub const DASHSCOPE_DEFAULT_BASE_URL: &str =
    "https://dashscope-intl.aliyuncs.com/compatible-mode/v1";

/// Optional env var name for application credentials (never auto-persisted).
pub const DASHSCOPE_ENV_KEY: &str = "DASHSCOPE_API_KEY";

/// Test-only credential injection (ignored harness). Never log this value.
pub const DASHSCOPE_TEST_ENV_KEY: &str = "GROK_TEST_DASHSCOPE_API_KEY";

/// Built-in provider profile for Alibaba DashScope / Model Studio.
pub fn dashscope_builtin_provider_config() -> ModelProviderConfig {
    ModelProviderConfig {
        kind: ModelProviderKind::DashScope,
        display_name: Some("Alibaba Model Studio".into()),
        base_url: Some(DASHSCOPE_DEFAULT_BASE_URL.into()),
        enabled: true,
        default_backend: Some("chat_completions".into()),
        auth_scheme: Some("bearer".into()),
        env_key: Some(crate::agent::config::EnvKeys::single(DASHSCOPE_ENV_KEY)),
        api_backend: Some(ApiBackend::ChatCompletions),
        catalog_enabled: true,
        capability_mode: Some("auto".into()),
        capabilities: {
            let mut c = IndexMap::new();
            c.insert("chat_completions".into(), true);
            c.insert("responses".into(), false);
            c.insert("embeddings".into(), false);
            c.insert("native_web_search".into(), false);
            c.insert("native_mcp".into(), false);
            c
        },
        ..Default::default()
    }
}

/// Install the DashScope built-in provider into a model_providers map when absent.
pub fn install_dashscope_provider(model_providers: &mut IndexMap<String, ModelProviderConfig>) {
    model_providers
        .entry(DASHSCOPE_PROVIDER_ID.to_owned())
        .or_insert_with(dashscope_builtin_provider_config);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_config_shape() {
        let cfg = dashscope_builtin_provider_config();
        assert_eq!(cfg.kind, ModelProviderKind::DashScope);
        assert_eq!(cfg.base_url.as_deref(), Some(DASHSCOPE_DEFAULT_BASE_URL));
        assert_eq!(cfg.api_backend, Some(ApiBackend::ChatCompletions));
        assert!(cfg.catalog_enabled);
        assert_eq!(cfg.capabilities.get("chat_completions"), Some(&true));
        assert_eq!(cfg.capabilities.get("responses"), Some(&false));
        assert_eq!(
            cfg.env_key,
            Some(crate::agent::config::EnvKeys::single(DASHSCOPE_ENV_KEY))
        );
    }

    #[test]
    fn install_is_idempotent_and_preserves_user_override() {
        let mut providers = IndexMap::new();
        install_dashscope_provider(&mut providers);
        assert!(providers.contains_key(DASHSCOPE_PROVIDER_ID));
        let mut cfg = providers[DASHSCOPE_PROVIDER_ID].clone();
        cfg.base_url = Some("https://example.test/compatible-mode/v1".into());
        providers.insert(DASHSCOPE_PROVIDER_ID.to_owned(), cfg);
        install_dashscope_provider(&mut providers);
        assert_eq!(
            providers[DASHSCOPE_PROVIDER_ID]
                .base_url
                .as_deref()
                .unwrap(),
            "https://example.test/compatible-mode/v1",
            "a user-defined entry must not be replaced"
        );
    }
}
