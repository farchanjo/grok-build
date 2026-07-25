//! First-class Z.ai Model API integration (`zai-model-api` profile).
//!
//! Base URL: `https://api.z.ai/api/paas/v4` (general OpenAI-compatible API).
//! Coding Plan endpoint is intentionally a separate configured instance.
//!
//! Credentials are never stored in TOML fixtures or source. Use provider-scoped
//! secrets or `GROK_TEST_ZAI_API_KEY` for ignored conformance only.

use crate::agent::model_providers::{ModelProviderConfig, ModelProviderKind};
use crate::inference::ApiBackend;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub mod fixtures;

/// Stable config / CLI / TUI provider id for the built-in Z.ai profile.
pub const ZAI_PROVIDER_ID: &str = "zai-model-api";

/// Verified general Model API base URL (not the Coding Plan endpoint).
pub const ZAI_DEFAULT_BASE_URL: &str = "https://api.z.ai/api/paas/v4";

/// Optional env var name for application credentials (never auto-persisted).
pub const ZAI_ENV_KEY: &str = "ZAI_API_KEY";

/// Test-only credential injection (ignored harness). Never log this value.
pub const ZAI_TEST_ENV_KEY: &str = "GROK_TEST_ZAI_API_KEY";

/// Built-in provider profile for Z.ai Model API.
pub fn zai_builtin_provider_config() -> ModelProviderConfig {
    ModelProviderConfig {
        kind: ModelProviderKind::Zai,
        display_name: Some("Z.ai Model API".into()),
        base_url: Some(ZAI_DEFAULT_BASE_URL.into()),
        enabled: true,
        default_backend: Some("chat_completions".into()),
        auth_scheme: Some("bearer".into()),
        env_key: Some(crate::agent::config::EnvKeys::single(ZAI_ENV_KEY)),
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

/// Install the Z.ai built-in provider into a model_providers map when absent.
pub fn install_zai_provider(model_providers: &mut IndexMap<String, ModelProviderConfig>) {
    model_providers
        .entry(ZAI_PROVIDER_ID.to_owned())
        .or_insert_with(zai_builtin_provider_config);
}

/// Z.ai Chat Completions request extensions (serialized only for Z.ai profile).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ZaiChatExtensions {
    /// Enable thinking / reasoning stream (`thinking.type = enabled`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ZaiThinking>,
    /// Stream tool call argument fragments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_stream: Option<bool>,
    /// Request-id correlation when the client supplies one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    /// Preserve unknown additive fields safely.
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ZaiThinking {
    /// `enabled` / `disabled` per Z.ai docs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    /// Clear thinking continuity across tool rounds when true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clear_thinking: Option<bool>,
}

/// Partial assistant message fields unique to Z.ai streams.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ZaiAssistantExtensions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Map Z.ai HTTP/status/finish_reason into provider-scoped diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZaiErrorClass {
    InvalidKey,
    QuotaExhausted,
    RateLimited,
    Safety,
    UnsupportedModel,
    MalformedStream,
    Upstream,
    Unknown,
}

pub fn classify_zai_error(
    status: Option<u16>,
    finish_reason: Option<&str>,
    body_preview: &str,
) -> ZaiErrorClass {
    if status == Some(401) || body_preview.contains("invalid_api_key") {
        return ZaiErrorClass::InvalidKey;
    }
    if status == Some(429) {
        return ZaiErrorClass::RateLimited;
    }
    if status == Some(402) || body_preview.contains("insufficient") {
        return ZaiErrorClass::QuotaExhausted;
    }
    if matches!(finish_reason, Some("sensitive") | Some("network_error"))
        || body_preview.contains("content_filter")
    {
        return ZaiErrorClass::Safety;
    }
    if body_preview.contains("model_not_found") || status == Some(404) {
        return ZaiErrorClass::UnsupportedModel;
    }
    if body_preview.contains("malformed") {
        return ZaiErrorClass::MalformedStream;
    }
    if status.is_some_and(|s| (500..600).contains(&s)) {
        return ZaiErrorClass::Upstream;
    }
    ZaiErrorClass::Unknown
}

/// Provider-scoped 401 remediation (never mentions `/login`).
pub fn zai_credential_repair_message() -> &'static str {
    "Z.ai rejected its API key. Open /providers, select Z.ai Model API (zai-model-api), \
     and replace or test the key."
}

/// Whether native Z.ai web search should be advertised (off by default).
pub fn native_web_search_enabled(capabilities: &IndexMap<String, bool>) -> bool {
    capabilities
        .get("native_web_search")
        .copied()
        .unwrap_or(false)
}

/// Merge Z.ai extensions into a chat completion request body when identity is Z.ai.
pub fn apply_zai_extensions(body: &mut Value, extensions: &ZaiChatExtensions) {
    if let Some(obj) = body.as_object_mut() {
        if let Some(thinking) = &extensions.thinking {
            if let Ok(v) = serde_json::to_value(thinking) {
                obj.insert("thinking".into(), v);
            }
        }
        if let Some(tool_stream) = extensions.tool_stream {
            obj.insert("tool_stream".into(), Value::Bool(tool_stream));
        }
        if let Some(rid) = &extensions.request_id {
            obj.insert("request_id".into(), Value::String(rid.clone()));
        }
        for (k, v) in &extensions.extra {
            obj.entry(k.clone()).or_insert_with(|| v.clone());
        }
    }
}

/// Extract reasoning_content from a streamed choice delta if present.
pub fn extract_reasoning_content(delta: &Value) -> Option<String> {
    delta
        .get("reasoning_content")
        .and_then(|v| v.as_str())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn builtin_profile_points_at_paas_v4() {
        let cfg = zai_builtin_provider_config();
        assert_eq!(cfg.kind, ModelProviderKind::Zai);
        assert_eq!(cfg.base_url.as_deref(), Some(ZAI_DEFAULT_BASE_URL));
        assert_eq!(cfg.api_backend, Some(ApiBackend::ChatCompletions));
        assert_eq!(cfg.capabilities.get("chat_completions"), Some(&true));
        assert_eq!(cfg.capabilities.get("native_web_search"), Some(&false));
    }

    #[test]
    fn repair_message_never_mentions_login() {
        let msg = zai_credential_repair_message();
        assert!(!msg.contains("/login"));
        assert!(!msg.contains("grok login"));
        assert!(msg.contains("/providers"));
        assert!(msg.contains("Z.ai"));
    }

    #[test]
    fn classifies_401() {
        assert_eq!(
            classify_zai_error(Some(401), None, "invalid_api_key"),
            ZaiErrorClass::InvalidKey
        );
    }

    #[test]
    fn applies_thinking_extension() {
        let mut body = json!({"model": "glm-4.5", "messages": []});
        apply_zai_extensions(
            &mut body,
            &ZaiChatExtensions {
                thinking: Some(ZaiThinking {
                    r#type: Some("enabled".into()),
                    clear_thinking: Some(false),
                }),
                tool_stream: Some(true),
                request_id: Some("req_test".into()),
                ..Default::default()
            },
        );
        assert_eq!(body["tool_stream"], true);
        assert_eq!(body["thinking"]["type"], "enabled");
        assert_eq!(body["request_id"], "req_test");
    }
}
