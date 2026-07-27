//! Anthropic Models API wire types (`GET /v1/models`).

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;

/// Query parameters for listing models (cursor pagination).
#[derive(Debug, Clone, Default, Serialize)]
pub struct ListModelsParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// Whether a model capability is supported.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilitySupport {
    #[serde(default)]
    pub supported: bool,
    /// Forward-compatible fields.
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, JsonValue>,
}

/// Per-level support advertised by Anthropic's effort capability.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffortCapability {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low: Option<CapabilitySupport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub medium: Option<CapabilitySupport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high: Option<CapabilitySupport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xhigh: Option<CapabilitySupport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<CapabilitySupport>,
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, JsonValue>,
}

/// Documented model capabilities plus additive unknown keys.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelCapabilities {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch: Option<CapabilitySupport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub citations: Option<CapabilitySupport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_execution: Option<CapabilitySupport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_input: Option<CapabilitySupport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pdf_input: Option<CapabilitySupport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_outputs: Option<CapabilitySupport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<EffortCapability>,
    /// Other nested capability objects (thinking, context_management, …)
    /// retained as JSON for forward compatibility.
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, JsonValue>,
}

/// One model entry from `GET /v1/models` or `GET /v1/models/{id}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// Object type; typically `"model"`.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    /// Maximum input context window in tokens when reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_input_tokens: Option<u64>,
    /// Maximum value for `max_tokens` when reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<ModelCapabilities>,
    /// Forward-compatible unknown fields.
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, JsonValue>,
}

/// Cursor-paginated model list response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelListPage {
    pub data: Vec<ModelInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_id: Option<String>,
    #[serde(default)]
    pub has_more: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_info_round_trips_documented_fields() {
        let raw = r#"{
            "id": "claude-opus-4-6",
            "type": "model",
            "display_name": "Claude Opus 4.6",
            "created_at": "2026-02-04T00:00:00Z",
            "max_input_tokens": 200000,
            "max_tokens": 32000,
            "capabilities": {
                "batch": {"supported": true},
                "thinking": {"supported": true, "types": {"enabled": {"supported": true}}}
            },
            "future_field": 1
        }"#;
        let info: ModelInfo = serde_json::from_str(raw).unwrap();
        assert_eq!(info.id, "claude-opus-4-6");
        assert_eq!(info.max_input_tokens, Some(200_000));
        assert_eq!(info.max_tokens, Some(32_000));
        assert_eq!(
            info.capabilities
                .as_ref()
                .and_then(|c| c.batch.as_ref())
                .map(|b| b.supported),
            Some(true)
        );
        assert!(info.extra.contains_key("future_field"));
        // Thinking nested under capabilities.extra via flatten.
        assert!(
            info.capabilities
                .as_ref()
                .is_some_and(|c| c.extra.contains_key("thinking"))
        );
    }

    #[test]
    fn model_info_parses_typed_effort_capabilities() {
        let raw = r#"{
            "id": "claude-sonnet-test",
            "capabilities": {
                "effort": {
                    "low": {"supported": true},
                    "medium": {"supported": false},
                    "high": {"supported": true},
                    "xhigh": {"supported": true},
                    "max": {"supported": true},
                    "future": {"supported": true}
                }
            }
        }"#;
        let info: ModelInfo = serde_json::from_str(raw).unwrap();
        let effort = info.capabilities.unwrap().effort.unwrap();
        assert_eq!(effort.low.map(|level| level.supported), Some(true));
        assert_eq!(effort.medium.map(|level| level.supported), Some(false));
        assert_eq!(effort.high.map(|level| level.supported), Some(true));
        assert_eq!(effort.xhigh.map(|level| level.supported), Some(true));
        assert_eq!(effort.max.map(|level| level.supported), Some(true));
        assert!(effort.extra.contains_key("future"));
    }

    #[test]
    fn model_list_page_parses() {
        let raw = r#"{
            "data": [{"id": "m1", "type": "model"}],
            "first_id": "m1",
            "last_id": "m1",
            "has_more": false
        }"#;
        let page: ModelListPage = serde_json::from_str(raw).unwrap();
        assert_eq!(page.data.len(), 1);
        assert!(!page.has_more);
    }
}
