//! Token counting API (`POST /v1/messages/count_tokens`).

use crate::messages::{
    Message, OutputConfig, SystemParam, ThinkingConfig, ToolChoiceParam, ToolParam,
};
use serde::{Deserialize, Serialize};

/// Request body shared with Messages create except create-only fields
/// (`max_tokens`, `stream`, `temperature`, `top_p`, `top_k`, `stop_sequences`,
/// `metadata`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CountTokensRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<SystemParam>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolParam>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoiceParam>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_config: Option<OutputConfig>,
}

impl From<&crate::messages::MessagesRequest> for CountTokensRequest {
    fn from(req: &crate::messages::MessagesRequest) -> Self {
        Self {
            model: req.model.clone(),
            messages: req.messages.clone(),
            system: req.system.clone(),
            tools: req.tools.clone(),
            tool_choice: req.tool_choice.clone(),
            thinking: req.thinking.clone(),
            output_config: req.output_config.clone(),
        }
    }
}

impl From<crate::messages::MessagesRequest> for CountTokensRequest {
    fn from(req: crate::messages::MessagesRequest) -> Self {
        Self::from(&req)
    }
}

/// Token count response. Documented field is `input_tokens`; optional cache
/// buckets are retained additively when present.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CountTokensResponse {
    pub input_tokens: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::{Message, MessageContent, MessageRole, MessagesRequest};

    #[test]
    fn from_messages_request_drops_create_only_fields() {
        let mut msg = MessagesRequest {
            model: "claude-test".into(),
            messages: vec![Message {
                role: MessageRole::User,
                content: MessageContent::Text("hi".into()),
            }],
            max_tokens: 1024,
            stream: Some(true),
            temperature: Some(0.5),
            ..Default::default()
        };
        msg.system = Some(SystemParam::Text("sys".into()));
        let count = CountTokensRequest::from(&msg);
        let json = serde_json::to_value(&count).unwrap();
        assert!(json.get("max_tokens").is_none());
        assert!(json.get("stream").is_none());
        assert!(json.get("temperature").is_none());
        assert_eq!(json["model"], "claude-test");
        assert_eq!(json["system"], "sys");
    }

    #[test]
    fn response_parses_input_tokens_and_additive_usage() {
        let raw = r#"{"input_tokens":42,"cache_read_input_tokens":10}"#;
        let resp: CountTokensResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(resp.input_tokens, 42);
        assert_eq!(resp.cache_read_input_tokens, Some(10));
    }
}
