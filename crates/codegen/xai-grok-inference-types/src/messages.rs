//! Anthropic Messages API (`/v1/messages`) wire types.
//!
//! These types represent the request/response format for the `/v1/messages` API.
//!
//! # Wire fidelity and unknown variants
//!
//! Known content-block, stream-delta, and stream-event variants stay typed.
//! Unknown variants retain their wire `type` discriminant and the original
//! JSON object so history can be re-serialized field-faithfully. Unknown
//! blocks and events are never promoted into executable Grok tool calls.

use serde::de::{self, Deserializer};
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value as JsonValue;

// ============================================================================
// Request Types
// ============================================================================

/// POST /v1/messages request body
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessagesRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<SystemParam>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolParam>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoiceParam>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_config: Option<OutputConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<OutputFormat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputFormat {
    JsonSchema { schema: serde_json::Value },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: MessageContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SystemParam {
    Text(String),
    Blocks(Vec<TextBlock>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextBlock {
    #[serde(rename = "type")]
    pub r#type: String, // always "text"
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
    /// Citation locations when the block is part of a cited response. Optional
    /// so ordinary text blocks serialize without the field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub citations: Option<Vec<JsonValue>>,
}

/// Prompt-cache control. Wire remains `{"type":"ephemeral"}` when `ttl` is unset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheControl {
    #[serde(rename = "type")]
    pub r#type: String, // "ephemeral"
    /// Optional cache TTL. Documented values: `"5m"`, `"1h"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl: Option<CacheControlTtl>,
}

/// Documented ephemeral cache TTL values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CacheControlTtl {
    #[serde(rename = "5m")]
    FiveMinutes,
    #[serde(rename = "1h")]
    OneHour,
    /// Preserve forward-compatible TTL strings without failing deserialize.
    #[serde(untagged)]
    Unknown(String),
}

/// Enable citations on a document or search-result block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CitationsConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

/// Content blocks used in both requests and responses.
///
/// Serialize/Deserialize are hand-written so known variants stay typed while
/// unknown `type` values keep the full original object for lossless replay.
#[derive(Debug, Clone, PartialEq)]
pub enum ContentBlock {
    Text {
        text: String,
        cache_control: Option<CacheControl>,
        /// Response-side citation locations supporting this text claim.
        citations: Option<Vec<JsonValue>>,
    },
    Image {
        source: ImageSource,
        cache_control: Option<CacheControl>,
    },
    Document {
        source: DocumentSource,
        title: Option<String>,
        context: Option<String>,
        citations: Option<CitationsConfig>,
        cache_control: Option<CacheControl>,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
        cache_control: Option<CacheControl>,
    },
    ToolResult {
        tool_use_id: String,
        content: ToolResultContent,
        is_error: Option<bool>,
        cache_control: Option<CacheControl>,
    },
    Thinking {
        thinking: String,
        signature: String,
    },
    /// Encrypted/redacted thinking blob. Must be echoed back for multi-turn
    /// thinking continuity when present in prior assistant turns.
    RedactedThinking {
        data: String,
    },
    /// RAG search-result block with optional citation enablement.
    SearchResult {
        source: String,
        title: String,
        content: Vec<TextBlock>,
        citations: Option<CitationsConfig>,
        cache_control: Option<CacheControl>,
    },
    /// Provider-executed server tool call (web search, code execution, etc.).
    /// Preserved for wire fidelity only; Grok never executes these locally.
    ServerToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
        cache_control: Option<CacheControl>,
    },
    /// Server-tool result / error / related result shapes.
    ///
    /// `type_name` is the wire discriminant (e.g. `web_search_tool_result`).
    /// `content` and `extra` keep the result body lossless without modeling
    /// every server-tool-specific schema. Never treated as a client tool call.
    ServerToolResult {
        type_name: String,
        tool_use_id: String,
        content: JsonValue,
        cache_control: Option<CacheControl>,
        /// Additional fields beyond the common ones (error codes, etc.).
        extra: serde_json::Map<String, JsonValue>,
    },
    /// Server-side context compaction block when present on the wire.
    Compaction {
        content: Option<String>,
    },
    /// Unknown content block. Discriminant + full raw object for faithful
    /// reserialization. Must never become an executable Grok tool call.
    Unknown {
        type_name: String,
        raw: JsonValue,
    },
}

/// Server-tool result wire type names recognized for typed preservation.
const SERVER_TOOL_RESULT_TYPES: &[&str] = &[
    "web_search_tool_result",
    "web_fetch_tool_result",
    "code_execution_tool_result",
    "bash_code_execution_tool_result",
    "text_editor_code_execution_tool_result",
    "tool_search_tool_result",
    "tool_search_tool_search_result",
    "mcp_tool_result",
    "mcp_tool_use",
];

impl ContentBlock {
    /// Wire `type` string for this block.
    pub fn type_name(&self) -> &str {
        match self {
            Self::Text { .. } => "text",
            Self::Image { .. } => "image",
            Self::Document { .. } => "document",
            Self::ToolUse { .. } => "tool_use",
            Self::ToolResult { .. } => "tool_result",
            Self::Thinking { .. } => "thinking",
            Self::RedactedThinking { .. } => "redacted_thinking",
            Self::SearchResult { .. } => "search_result",
            Self::ServerToolUse { .. } => "server_tool_use",
            Self::ServerToolResult { type_name, .. } => type_name.as_str(),
            Self::Compaction { .. } => "compaction",
            Self::Unknown { type_name, .. } => type_name.as_str(),
        }
    }

    /// True for client-executable `tool_use` only. Server tools and unknown
    /// blocks are never executable Grok tool calls.
    pub fn is_client_tool_use(&self) -> bool {
        matches!(self, Self::ToolUse { .. })
    }

    /// Bounded diagnostic preview of an unknown/raw payload. Full `raw` is
    /// retained for serde; this helper is only for logs.
    pub fn raw_diagnostic_preview(raw: &JsonValue, max_chars: usize) -> String {
        let s = raw.to_string();
        if s.len() <= max_chars {
            return s;
        }
        let mut end = max_chars.min(s.len());
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…({} bytes total)", &s[..end], s.len())
    }
}

impl Serialize for ContentBlock {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Unknown { raw, .. } => raw.serialize(serializer),
            Self::ServerToolResult {
                type_name,
                tool_use_id,
                content,
                cache_control,
                extra,
            } => {
                let mut map = serde_json::Map::new();
                map.insert("type".into(), JsonValue::String(type_name.clone()));
                map.insert("tool_use_id".into(), JsonValue::String(tool_use_id.clone()));
                map.insert("content".into(), content.clone());
                if let Some(cc) = cache_control {
                    map.insert(
                        "cache_control".into(),
                        serde_json::to_value(cc).map_err(serde::ser::Error::custom)?,
                    );
                }
                for (k, v) in extra {
                    map.insert(k.clone(), v.clone());
                }
                JsonValue::Object(map).serialize(serializer)
            }
            other => {
                // Serialize known variants via an intermediate JSON object so
                // optional fields use the same skip rules as request builders.
                let mut map = serde_json::Map::new();
                map.insert(
                    "type".into(),
                    JsonValue::String(other.type_name().to_string()),
                );
                match other {
                    Self::Text {
                        text,
                        cache_control,
                        citations,
                    } => {
                        map.insert("text".into(), JsonValue::String(text.clone()));
                        insert_opt_json(&mut map, "cache_control", cache_control)?;
                        insert_opt_json(&mut map, "citations", citations)?;
                    }
                    Self::Image {
                        source,
                        cache_control,
                    } => {
                        map.insert(
                            "source".into(),
                            serde_json::to_value(source).map_err(serde::ser::Error::custom)?,
                        );
                        insert_opt_json(&mut map, "cache_control", cache_control)?;
                    }
                    Self::Document {
                        source,
                        title,
                        context,
                        citations,
                        cache_control,
                    } => {
                        map.insert(
                            "source".into(),
                            serde_json::to_value(source).map_err(serde::ser::Error::custom)?,
                        );
                        insert_opt_json(&mut map, "title", title)?;
                        insert_opt_json(&mut map, "context", context)?;
                        insert_opt_json(&mut map, "citations", citations)?;
                        insert_opt_json(&mut map, "cache_control", cache_control)?;
                    }
                    Self::ToolUse {
                        id,
                        name,
                        input,
                        cache_control,
                    } => {
                        map.insert("id".into(), JsonValue::String(id.clone()));
                        map.insert("name".into(), JsonValue::String(name.clone()));
                        map.insert("input".into(), input.clone());
                        insert_opt_json(&mut map, "cache_control", cache_control)?;
                    }
                    Self::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                        cache_control,
                    } => {
                        map.insert("tool_use_id".into(), JsonValue::String(tool_use_id.clone()));
                        map.insert(
                            "content".into(),
                            serde_json::to_value(content).map_err(serde::ser::Error::custom)?,
                        );
                        insert_opt_json(&mut map, "is_error", is_error)?;
                        insert_opt_json(&mut map, "cache_control", cache_control)?;
                    }
                    Self::Thinking {
                        thinking,
                        signature,
                    } => {
                        map.insert("thinking".into(), JsonValue::String(thinking.clone()));
                        map.insert("signature".into(), JsonValue::String(signature.clone()));
                    }
                    Self::RedactedThinking { data } => {
                        map.insert("data".into(), JsonValue::String(data.clone()));
                    }
                    Self::SearchResult {
                        source,
                        title,
                        content,
                        citations,
                        cache_control,
                    } => {
                        map.insert("source".into(), JsonValue::String(source.clone()));
                        map.insert("title".into(), JsonValue::String(title.clone()));
                        map.insert(
                            "content".into(),
                            serde_json::to_value(content).map_err(serde::ser::Error::custom)?,
                        );
                        insert_opt_json(&mut map, "citations", citations)?;
                        insert_opt_json(&mut map, "cache_control", cache_control)?;
                    }
                    Self::ServerToolUse {
                        id,
                        name,
                        input,
                        cache_control,
                    } => {
                        map.insert("id".into(), JsonValue::String(id.clone()));
                        map.insert("name".into(), JsonValue::String(name.clone()));
                        map.insert("input".into(), input.clone());
                        insert_opt_json(&mut map, "cache_control", cache_control)?;
                    }
                    Self::Compaction { content } => {
                        insert_opt_json(&mut map, "content", content)?;
                    }
                    Self::ServerToolResult { .. } | Self::Unknown { .. } => unreachable!(),
                }
                JsonValue::Object(map).serialize(serializer)
            }
        }
    }
}

fn insert_opt_json<T: Serialize, E: serde::ser::Error>(
    map: &mut serde_json::Map<String, JsonValue>,
    key: &str,
    value: &Option<T>,
) -> Result<(), E> {
    if let Some(v) = value {
        map.insert(key.to_string(), serde_json::to_value(v).map_err(E::custom)?);
    }
    Ok(())
}

impl<'de> Deserialize<'de> for ContentBlock {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = JsonValue::deserialize(deserializer)?;
        let type_name = value
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        match type_name.as_str() {
            "text" => {
                #[derive(Deserialize)]
                struct TextDe {
                    text: String,
                    #[serde(default)]
                    cache_control: Option<CacheControl>,
                    #[serde(default)]
                    citations: Option<Vec<JsonValue>>,
                }
                let t: TextDe = serde_json::from_value(value).map_err(de::Error::custom)?;
                Ok(Self::Text {
                    text: t.text,
                    cache_control: t.cache_control,
                    citations: t.citations,
                })
            }
            "image" => {
                #[derive(Deserialize)]
                struct ImageDe {
                    source: ImageSource,
                    #[serde(default)]
                    cache_control: Option<CacheControl>,
                }
                let t: ImageDe = serde_json::from_value(value).map_err(de::Error::custom)?;
                Ok(Self::Image {
                    source: t.source,
                    cache_control: t.cache_control,
                })
            }
            "document" => {
                #[derive(Deserialize)]
                struct DocumentDe {
                    source: DocumentSource,
                    #[serde(default)]
                    title: Option<String>,
                    #[serde(default)]
                    context: Option<String>,
                    #[serde(default)]
                    citations: Option<CitationsConfig>,
                    #[serde(default)]
                    cache_control: Option<CacheControl>,
                }
                let t: DocumentDe = serde_json::from_value(value).map_err(de::Error::custom)?;
                Ok(Self::Document {
                    source: t.source,
                    title: t.title,
                    context: t.context,
                    citations: t.citations,
                    cache_control: t.cache_control,
                })
            }
            "tool_use" => {
                #[derive(Deserialize)]
                struct ToolUseDe {
                    id: String,
                    name: String,
                    #[serde(default)]
                    input: serde_json::Value,
                    #[serde(default)]
                    cache_control: Option<CacheControl>,
                }
                let t: ToolUseDe = serde_json::from_value(value).map_err(de::Error::custom)?;
                Ok(Self::ToolUse {
                    id: t.id,
                    name: t.name,
                    input: t.input,
                    cache_control: t.cache_control,
                })
            }
            "tool_result" => {
                #[derive(Deserialize)]
                struct ToolResultDe {
                    tool_use_id: String,
                    #[serde(default)]
                    content: ToolResultContent,
                    #[serde(default)]
                    is_error: Option<bool>,
                    #[serde(default)]
                    cache_control: Option<CacheControl>,
                }
                let t: ToolResultDe = serde_json::from_value(value).map_err(de::Error::custom)?;
                Ok(Self::ToolResult {
                    tool_use_id: t.tool_use_id,
                    content: t.content,
                    is_error: t.is_error,
                    cache_control: t.cache_control,
                })
            }
            "thinking" => {
                #[derive(Deserialize)]
                struct ThinkingDe {
                    #[serde(default)]
                    thinking: String,
                    #[serde(default)]
                    signature: String,
                }
                let t: ThinkingDe = serde_json::from_value(value).map_err(de::Error::custom)?;
                Ok(Self::Thinking {
                    thinking: t.thinking,
                    signature: t.signature,
                })
            }
            "redacted_thinking" => {
                #[derive(Deserialize)]
                struct RedactedDe {
                    data: String,
                }
                let t: RedactedDe = serde_json::from_value(value).map_err(de::Error::custom)?;
                Ok(Self::RedactedThinking { data: t.data })
            }
            "search_result" => {
                #[derive(Deserialize)]
                struct SearchDe {
                    source: String,
                    title: String,
                    #[serde(default)]
                    content: Vec<TextBlock>,
                    #[serde(default)]
                    citations: Option<CitationsConfig>,
                    #[serde(default)]
                    cache_control: Option<CacheControl>,
                }
                let t: SearchDe = serde_json::from_value(value).map_err(de::Error::custom)?;
                Ok(Self::SearchResult {
                    source: t.source,
                    title: t.title,
                    content: t.content,
                    citations: t.citations,
                    cache_control: t.cache_control,
                })
            }
            "server_tool_use" => {
                #[derive(Deserialize)]
                struct ServerToolUseDe {
                    id: String,
                    name: String,
                    #[serde(default)]
                    input: serde_json::Value,
                    #[serde(default)]
                    cache_control: Option<CacheControl>,
                }
                let t: ServerToolUseDe =
                    serde_json::from_value(value).map_err(de::Error::custom)?;
                Ok(Self::ServerToolUse {
                    id: t.id,
                    name: t.name,
                    input: t.input,
                    cache_control: t.cache_control,
                })
            }
            "compaction" => {
                #[derive(Deserialize)]
                struct CompactionDe {
                    #[serde(default)]
                    content: Option<String>,
                }
                let t: CompactionDe = serde_json::from_value(value).map_err(de::Error::custom)?;
                Ok(Self::Compaction { content: t.content })
            }
            other if SERVER_TOOL_RESULT_TYPES.contains(&other) => {
                let mut obj = match value {
                    JsonValue::Object(m) => m,
                    other => {
                        return Ok(Self::Unknown {
                            type_name: type_name.clone(),
                            raw: other,
                        });
                    }
                };
                let tool_use_id = obj
                    .remove("tool_use_id")
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default();
                let content = obj.remove("content").unwrap_or(JsonValue::Null);
                let cache_control = obj
                    .remove("cache_control")
                    .and_then(|v| serde_json::from_value(v).ok());
                obj.remove("type");
                Ok(Self::ServerToolResult {
                    type_name,
                    tool_use_id,
                    content,
                    cache_control,
                    extra: obj,
                })
            }
            other => Ok(Self::Unknown {
                type_name: other.to_string(),
                raw: value,
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImageSource {
    Base64 { media_type: String, data: String },
    Url { url: String },
    File { file_id: String },
}

/// Document content sources (PDF / plain text / URL / file id / custom content).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DocumentSource {
    Base64 {
        media_type: String,
        data: String,
    },
    Text {
        media_type: String,
        data: String,
    },
    Url {
        url: String,
    },
    File {
        file_id: String,
    },
    /// Custom content document: caller-defined chunks, no further chunking.
    Content {
        content: DocumentContentBlocks,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DocumentContentBlocks {
    Text(String),
    Blocks(Vec<JsonValue>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolResultContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

impl Default for ToolResultContent {
    fn default() -> Self {
        Self::Text(String::new())
    }
}

/// Tool definition (Anthropic Messages API format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParam {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: serde_json::Value,
    /// When true, the model must produce schema-conformant tool input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
}

/// Tool choice (Anthropic Messages API format)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolChoiceParam {
    Auto {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        disable_parallel_tool_use: Option<bool>,
    },
    Any {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        disable_parallel_tool_use: Option<bool>,
    },
    Tool {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        disable_parallel_tool_use: Option<bool>,
    },
    None,
}

impl ToolChoiceParam {
    pub fn auto() -> Self {
        Self::Auto {
            disable_parallel_tool_use: None,
        }
    }

    pub fn any() -> Self {
        Self::Any {
            disable_parallel_tool_use: None,
        }
    }
}

/// Extended thinking configuration
///
/// Three modes per the Anthropic Messages API:
/// - Adaptive: 4.6+ models, API decides budget
/// - Enabled: 4.0-4.5 models, explicit budget_tokens
/// - Disabled: pre-thinking models or thinking_budget=0
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThinkingDisplay {
    Omitted,
    Summarized,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ThinkingConfig {
    Enabled {
        budget_tokens: u32,
    },
    Adaptive {
        // Newer thinking-capable models omit thinking content unless display = "summarized".
        // Older models ignore this field. Skip when None to stay back-compat.
        #[serde(skip_serializing_if = "Option::is_none")]
        display: Option<ThinkingDisplay>,
    },
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

// ============================================================================
// Response Types
// ============================================================================

/// Non-streaming response from POST /v1/messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagesResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub r#type: String, // "message"
    pub role: String, // "assistant"
    pub content: Vec<ContentBlock>,
    pub model: String,
    pub stop_reason: Option<StopReason>,
    /// Present on some responses when a stop sequence matched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_sequence: Option<String>,
    pub usage: MessagesUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    EndTurn,
    MaxTokens,
    ToolUse,
    StopSequence,
    Refusal,
    PauseTurn,
    ModelContextWindowExceeded,
    /// Catch-all for stop reasons this client does not know yet, so a new
    /// server-side value can never fail the terminal `message_delta` parse
    /// and discard an already-streamed response. Preserves the wire string
    /// for logging and faithful re-serialization; must stay the LAST variant
    /// (serde tries the tagged variants above first).
    #[serde(untagged)]
    Unknown(String),
}

/// Cache creation token breakdown by TTL when the provider reports it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheCreationUsage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ephemeral_5m_input_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ephemeral_1h_input_tokens: Option<u32>,
}

/// Server-tool token accounting when present on usage.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerToolUsage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_search_requests: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_fetch_requests: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessagesUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    #[serde(default)]
    pub cache_creation_input_tokens: u32,
    #[serde(default)]
    pub cache_read_input_tokens: u32,
    /// Optional breakdown of cache writes by TTL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_creation: Option<CacheCreationUsage>,
    /// Optional server-tool request counts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_tool_use: Option<ServerToolUsage>,
    /// Service tier label when the provider echoes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
}

// ============================================================================
// Streaming Event Types
// ============================================================================

/// Top-level streaming event (SSE `type` field determines variant).
///
/// Unknown event types keep discriminant + raw JSON and never drive tool
/// execution.
#[derive(Debug, Clone)]
pub enum MessageStreamEvent {
    MessageStart {
        message: MessagesResponse,
    },
    MessageDelta {
        delta: MessageDeltaBody,
        usage: MessageDeltaUsage,
    },
    MessageStop,
    ContentBlockStart {
        index: u32,
        content_block: ContentBlock,
    },
    ContentBlockDelta {
        index: u32,
        delta: StreamDelta,
    },
    ContentBlockStop {
        index: u32,
    },
    Ping,
    Error {
        error: StreamError,
    },
    /// Unknown stream event. Preserved for diagnostics / future handling;
    /// never executed as a tool call.
    Unknown {
        type_name: String,
        raw: JsonValue,
    },
}

impl Serialize for MessageStreamEvent {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Unknown { raw, .. } => raw.serialize(serializer),
            other => {
                let mut map = serde_json::Map::new();
                let type_name = match other {
                    Self::MessageStart { .. } => "message_start",
                    Self::MessageDelta { .. } => "message_delta",
                    Self::MessageStop => "message_stop",
                    Self::ContentBlockStart { .. } => "content_block_start",
                    Self::ContentBlockDelta { .. } => "content_block_delta",
                    Self::ContentBlockStop { .. } => "content_block_stop",
                    Self::Ping => "ping",
                    Self::Error { .. } => "error",
                    Self::Unknown { .. } => unreachable!(),
                };
                map.insert("type".into(), JsonValue::String(type_name.into()));
                match other {
                    Self::MessageStart { message } => {
                        map.insert(
                            "message".into(),
                            serde_json::to_value(message).map_err(serde::ser::Error::custom)?,
                        );
                    }
                    Self::MessageDelta { delta, usage } => {
                        map.insert(
                            "delta".into(),
                            serde_json::to_value(delta).map_err(serde::ser::Error::custom)?,
                        );
                        map.insert(
                            "usage".into(),
                            serde_json::to_value(usage).map_err(serde::ser::Error::custom)?,
                        );
                    }
                    Self::MessageStop | Self::Ping => {}
                    Self::ContentBlockStart {
                        index,
                        content_block,
                    } => {
                        map.insert("index".into(), JsonValue::from(*index));
                        map.insert(
                            "content_block".into(),
                            serde_json::to_value(content_block)
                                .map_err(serde::ser::Error::custom)?,
                        );
                    }
                    Self::ContentBlockDelta { index, delta } => {
                        map.insert("index".into(), JsonValue::from(*index));
                        map.insert(
                            "delta".into(),
                            serde_json::to_value(delta).map_err(serde::ser::Error::custom)?,
                        );
                    }
                    Self::ContentBlockStop { index } => {
                        map.insert("index".into(), JsonValue::from(*index));
                    }
                    Self::Error { error } => {
                        map.insert(
                            "error".into(),
                            serde_json::to_value(error).map_err(serde::ser::Error::custom)?,
                        );
                    }
                    Self::Unknown { .. } => unreachable!(),
                }
                JsonValue::Object(map).serialize(serializer)
            }
        }
    }
}

impl<'de> Deserialize<'de> for MessageStreamEvent {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = JsonValue::deserialize(deserializer)?;
        let type_name = value
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        match type_name.as_str() {
            "message_start" => {
                #[derive(Deserialize)]
                struct De {
                    message: MessagesResponse,
                }
                let d: De = serde_json::from_value(value).map_err(de::Error::custom)?;
                Ok(Self::MessageStart { message: d.message })
            }
            "message_delta" => {
                #[derive(Deserialize)]
                struct De {
                    delta: MessageDeltaBody,
                    #[serde(default)]
                    usage: MessageDeltaUsage,
                }
                let d: De = serde_json::from_value(value).map_err(de::Error::custom)?;
                Ok(Self::MessageDelta {
                    delta: d.delta,
                    usage: d.usage,
                })
            }
            "message_stop" => Ok(Self::MessageStop),
            "content_block_start" => {
                #[derive(Deserialize)]
                struct De {
                    index: u32,
                    content_block: ContentBlock,
                }
                let d: De = serde_json::from_value(value).map_err(de::Error::custom)?;
                Ok(Self::ContentBlockStart {
                    index: d.index,
                    content_block: d.content_block,
                })
            }
            "content_block_delta" => {
                #[derive(Deserialize)]
                struct De {
                    index: u32,
                    delta: StreamDelta,
                }
                let d: De = serde_json::from_value(value).map_err(de::Error::custom)?;
                Ok(Self::ContentBlockDelta {
                    index: d.index,
                    delta: d.delta,
                })
            }
            "content_block_stop" => {
                #[derive(Deserialize)]
                struct De {
                    index: u32,
                }
                let d: De = serde_json::from_value(value).map_err(de::Error::custom)?;
                Ok(Self::ContentBlockStop { index: d.index })
            }
            "ping" => Ok(Self::Ping),
            "error" => {
                #[derive(Deserialize)]
                struct De {
                    error: StreamError,
                }
                let d: De = serde_json::from_value(value).map_err(de::Error::custom)?;
                Ok(Self::Error { error: d.error })
            }
            other => Ok(Self::Unknown {
                type_name: other.to_string(),
                raw: value,
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDeltaBody {
    pub stop_reason: Option<StopReason>,
    /// Matched stop sequence when `stop_reason` is `stop_sequence`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_sequence: Option<String>,
    /// Provider detail for the stop; on `refusal`, `explanation` carries the
    /// reason the request was blocked (e.g. an Anthropic ToS auto-refusal).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_details: Option<StopDetails>,
}

/// Detail for a terminal `message_delta`, e.g.
/// `{"type":"refusal","category":"frontier_llm","explanation":"..."}`.
/// All fields optional so an unknown shape never fails the terminal parse.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StopDetails {
    #[serde(rename = "type", default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub explanation: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageDeltaUsage {
    pub output_tokens: u32,
    #[serde(default)]
    pub input_tokens: Option<u32>,
    #[serde(default)]
    pub cache_read_input_tokens: Option<u32>,
    #[serde(default)]
    pub cache_creation_input_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_creation: Option<CacheCreationUsage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_tool_use: Option<ServerToolUsage>,
}

/// Content delta within a content_block_delta event.
#[derive(Debug, Clone, PartialEq)]
pub enum StreamDelta {
    TextDelta {
        text: String,
    },
    InputJsonDelta {
        partial_json: String,
    },
    ThinkingDelta {
        thinking: String,
    },
    SignatureDelta {
        signature: String,
    },
    /// Unknown delta type: preserve discriminant + raw object.
    Unknown {
        type_name: String,
        raw: JsonValue,
    },
}

impl Serialize for StreamDelta {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Unknown { raw, .. } => raw.serialize(serializer),
            other => {
                let mut map = serializer.serialize_map(None)?;
                match other {
                    Self::TextDelta { text } => {
                        map.serialize_entry("type", "text_delta")?;
                        map.serialize_entry("text", text)?;
                    }
                    Self::InputJsonDelta { partial_json } => {
                        map.serialize_entry("type", "input_json_delta")?;
                        map.serialize_entry("partial_json", partial_json)?;
                    }
                    Self::ThinkingDelta { thinking } => {
                        map.serialize_entry("type", "thinking_delta")?;
                        map.serialize_entry("thinking", thinking)?;
                    }
                    Self::SignatureDelta { signature } => {
                        map.serialize_entry("type", "signature_delta")?;
                        map.serialize_entry("signature", signature)?;
                    }
                    Self::Unknown { .. } => unreachable!(),
                }
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for StreamDelta {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = JsonValue::deserialize(deserializer)?;
        let type_name = value
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        match type_name.as_str() {
            "text_delta" => {
                let text = value
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                Ok(Self::TextDelta { text })
            }
            "input_json_delta" => {
                let partial_json = value
                    .get("partial_json")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                Ok(Self::InputJsonDelta { partial_json })
            }
            "thinking_delta" => {
                let thinking = value
                    .get("thinking")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                Ok(Self::ThinkingDelta { thinking })
            }
            "signature_delta" => {
                let signature = value
                    .get("signature")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                Ok(Self::SignatureDelta { signature })
            }
            other => Ok(Self::Unknown {
                type_name: other.to_string(),
                raw: value,
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamError {
    #[serde(rename = "type")]
    pub r#type: String,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_reason_deserializes_all_known_values_and_catches_unknown() {
        let parse = |raw: &str| -> StopReason {
            serde_json::from_str(&format!("\"{raw}\""))
                .unwrap_or_else(|e| panic!("stop_reason {raw:?} must parse: {e}"))
        };
        assert!(matches!(parse("end_turn"), StopReason::EndTurn));
        assert!(matches!(parse("max_tokens"), StopReason::MaxTokens));
        assert!(matches!(parse("tool_use"), StopReason::ToolUse));
        assert!(matches!(parse("stop_sequence"), StopReason::StopSequence));
        assert!(matches!(parse("refusal"), StopReason::Refusal));
        assert!(matches!(parse("pause_turn"), StopReason::PauseTurn));
        assert!(matches!(
            parse("model_context_window_exceeded"),
            StopReason::ModelContextWindowExceeded
        ));
        match parse("some_future_stop_reason") {
            StopReason::Unknown(s) => assert_eq!(s, "some_future_stop_reason"),
            other => panic!("unknown value must preserve the wire string, got {other:?}"),
        }
        assert_eq!(
            serde_json::to_string(&StopReason::Unknown("some_future_stop_reason".into())).unwrap(),
            "\"some_future_stop_reason\"",
            "catch-all must re-serialize the wire string faithfully"
        );
        // The catch-all must also work through the Option<StopReason> field
        // it is parsed from in production.
        let delta: MessageDeltaBody =
            serde_json::from_str(r#"{"stop_reason":"mystery_reason"}"#).unwrap();
        match delta.stop_reason {
            Some(StopReason::Unknown(s)) => assert_eq!(s, "mystery_reason"),
            other => panic!("expected Unknown through Option, got {other:?}"),
        }
    }

    /// The terminal `message_delta` of a refusal-terminated stream must parse
    /// (the internally-tagged `MessageStreamEvent` wrapper is the actual
    /// production parse site, hence the full-event fixture).
    #[test]
    fn message_delta_with_refusal_stop_reason_parses() {
        let event: MessageStreamEvent = serde_json::from_str(
            r#"{"type":"message_delta","delta":{"stop_reason":"refusal"},"usage":{"output_tokens":5,"input_tokens":10}}"#,
        )
        .expect("refusal message_delta must deserialize");
        match event {
            MessageStreamEvent::MessageDelta { delta, usage } => {
                assert!(matches!(delta.stop_reason, Some(StopReason::Refusal)));
                assert!(delta.stop_details.is_none(), "no stop_details on the wire");
                assert_eq!(usage.output_tokens, 5);
            }
            other => panic!("expected MessageDelta, got {other:?}"),
        }
    }

    /// A refusal `message_delta` carrying `stop_details` (as emitted by
    /// Anthropic ToS auto-refusals) must parse and preserve the explanation,
    /// and unknown keys inside `stop_details` must not fail the parse.
    #[test]
    fn message_delta_with_refusal_stop_details_parses() {
        let event: MessageStreamEvent = serde_json::from_str(
            r#"{"type":"message_delta","delta":{"stop_reason":"refusal","stop_sequence":null,"stop_details":{"type":"refusal","category":"frontier_llm","explanation":"This request was blocked.","future_key":42}},"usage":{"output_tokens":0}}"#,
        )
        .expect("refusal message_delta with stop_details must deserialize");
        match event {
            MessageStreamEvent::MessageDelta { delta, .. } => {
                assert!(matches!(delta.stop_reason, Some(StopReason::Refusal)));
                let details = delta.stop_details.expect("stop_details must be captured");
                assert_eq!(details.r#type.as_deref(), Some("refusal"));
                assert_eq!(details.category.as_deref(), Some("frontier_llm"));
                assert_eq!(
                    details.explanation.as_deref(),
                    Some("This request was blocked.")
                );
            }
            other => panic!("expected MessageDelta, got {other:?}"),
        }
    }

    #[test]
    fn output_format_json_schema_wire_shape() {
        let fmt = OutputFormat::JsonSchema {
            schema: serde_json::json!({"type": "object", "properties": {"x": {"type": "string"}}}),
        };
        let json = serde_json::to_value(&fmt).unwrap();
        assert_eq!(json["type"], "json_schema");
        assert_eq!(json["schema"]["type"], "object");
        assert!(json.get("name").is_none());

        let config = OutputConfig {
            effort: None,
            format: Some(fmt),
        };
        let json = serde_json::to_value(&config).unwrap();
        assert!(json.get("effort").is_none(), "effort omitted when None");
        assert_eq!(json["format"]["type"], "json_schema");
    }

    #[test]
    fn content_block_unknown_round_trips_field_faithfully() {
        let raw = serde_json::json!({
            "type": "future_block",
            "payload": {"nested": true},
            "extra_flag": 1
        });
        let block: ContentBlock = serde_json::from_value(raw.clone()).unwrap();
        match &block {
            ContentBlock::Unknown { type_name, raw: r } => {
                assert_eq!(type_name, "future_block");
                assert_eq!(r, &raw);
                assert!(!block.is_client_tool_use());
            }
            other => panic!("expected Unknown, got {other:?}"),
        }
        let out = serde_json::to_value(&block).unwrap();
        assert_eq!(out, raw);
    }

    #[test]
    fn content_block_redacted_thinking_and_document_sources_parse() {
        let redacted: ContentBlock =
            serde_json::from_str(r#"{"type":"redacted_thinking","data":"enc-blob"}"#).unwrap();
        assert!(matches!(
            redacted,
            ContentBlock::RedactedThinking { ref data } if data == "enc-blob"
        ));

        let doc: ContentBlock = serde_json::from_str(
            r#"{"type":"document","source":{"type":"file","file_id":"file_123"},"title":"Spec","citations":{"enabled":true}}"#,
        )
        .unwrap();
        match doc {
            ContentBlock::Document {
                source: DocumentSource::File { file_id },
                title,
                citations,
                ..
            } => {
                assert_eq!(file_id, "file_123");
                assert_eq!(title.as_deref(), Some("Spec"));
                assert_eq!(citations.and_then(|c| c.enabled), Some(true));
            }
            other => panic!("expected Document, got {other:?}"),
        }

        let text_src: ContentBlock = serde_json::from_str(
            r#"{"type":"document","source":{"type":"text","media_type":"text/plain","data":"hello"}}"#,
        )
        .unwrap();
        assert!(matches!(
            text_src,
            ContentBlock::Document {
                source: DocumentSource::Text { .. },
                ..
            }
        ));
    }

    #[test]
    fn tool_result_is_error_and_tool_strict_and_cache_ttl_serialize_only_when_set() {
        let tr = ContentBlock::ToolResult {
            tool_use_id: "tu_1".into(),
            content: ToolResultContent::Text("boom".into()),
            is_error: Some(true),
            cache_control: None,
        };
        let v = serde_json::to_value(&tr).unwrap();
        assert_eq!(v["is_error"], true);
        assert!(v.get("cache_control").is_none());

        let tool = ToolParam {
            name: "read".into(),
            description: None,
            input_schema: serde_json::json!({"type": "object"}),
            strict: Some(true),
        };
        let tv = serde_json::to_value(&tool).unwrap();
        assert_eq!(tv["strict"], true);

        let tool_plain = ToolParam {
            name: "read".into(),
            description: None,
            input_schema: serde_json::json!({"type": "object"}),
            strict: None,
        };
        let tp = serde_json::to_value(&tool_plain).unwrap();
        assert!(tp.get("strict").is_none());

        let cc = CacheControl {
            r#type: "ephemeral".into(),
            ttl: Some(CacheControlTtl::OneHour),
        };
        assert_eq!(
            serde_json::to_value(&cc).unwrap(),
            serde_json::json!({"type":"ephemeral","ttl":"1h"})
        );
        let cc_default = CacheControl {
            r#type: "ephemeral".into(),
            ttl: None,
        };
        assert_eq!(
            serde_json::to_value(&cc_default).unwrap(),
            serde_json::json!({"type":"ephemeral"})
        );
    }

    #[test]
    fn tool_choice_disable_parallel_is_optional() {
        let auto = ToolChoiceParam::auto();
        assert_eq!(
            serde_json::to_value(&auto).unwrap(),
            serde_json::json!({"type":"auto"})
        );
        let auto_parallel = ToolChoiceParam::Auto {
            disable_parallel_tool_use: Some(true),
        };
        assert_eq!(
            serde_json::to_value(&auto_parallel).unwrap(),
            serde_json::json!({"type":"auto","disable_parallel_tool_use":true})
        );
    }

    #[test]
    fn server_tool_use_and_result_never_client_tool_use() {
        let stu: ContentBlock = serde_json::from_str(
            r#"{"type":"server_tool_use","id":"srv_1","name":"web_search","input":{"q":"x"}}"#,
        )
        .unwrap();
        assert!(!stu.is_client_tool_use());
        let result: ContentBlock = serde_json::from_str(
            r#"{"type":"web_search_tool_result","tool_use_id":"srv_1","content":[{"type":"web_search_result","url":"https://x"}]}"#,
        )
        .unwrap();
        match &result {
            ContentBlock::ServerToolResult { type_name, .. } => {
                assert_eq!(type_name, "web_search_tool_result");
            }
            other => panic!("expected ServerToolResult, got {other:?}"),
        }
        assert!(!result.is_client_tool_use());
        // Round-trip preserves type + tool_use_id + content.
        let out = serde_json::to_value(&result).unwrap();
        assert_eq!(out["type"], "web_search_tool_result");
        assert_eq!(out["tool_use_id"], "srv_1");
    }

    #[test]
    fn stream_event_and_delta_unknown_round_trip() {
        let event_raw = serde_json::json!({
            "type": "future_event",
            "payload": 42
        });
        let event: MessageStreamEvent = serde_json::from_value(event_raw.clone()).unwrap();
        match &event {
            MessageStreamEvent::Unknown { type_name, raw } => {
                assert_eq!(type_name, "future_event");
                assert_eq!(raw, &event_raw);
            }
            other => panic!("expected Unknown event, got {other:?}"),
        }
        assert_eq!(serde_json::to_value(&event).unwrap(), event_raw);

        let delta_raw = serde_json::json!({"type":"citations_delta","citation":{"x":1}});
        let delta: StreamDelta = serde_json::from_value(delta_raw.clone()).unwrap();
        match &delta {
            StreamDelta::Unknown { type_name, raw } => {
                assert_eq!(type_name, "citations_delta");
                assert_eq!(raw, &delta_raw);
            }
            other => panic!("expected Unknown delta, got {other:?}"),
        }
        assert_eq!(serde_json::to_value(&delta).unwrap(), delta_raw);
    }

    #[test]
    fn known_text_block_omits_new_optional_fields_by_default() {
        let block = ContentBlock::Text {
            text: "hi".into(),
            cache_control: None,
            citations: None,
        };
        assert_eq!(
            serde_json::to_value(&block).unwrap(),
            serde_json::json!({"type":"text","text":"hi"})
        );
    }

    #[test]
    fn raw_diagnostic_preview_is_bounded() {
        let raw = serde_json::json!({"type":"x","blob":"a".repeat(200)});
        let preview = ContentBlock::raw_diagnostic_preview(&raw, 40);
        assert!(preview.len() < raw.to_string().len());
        assert!(preview.contains("bytes total"));
    }

    #[test]
    fn usage_additive_fields_optional() {
        let usage: MessagesUsage = serde_json::from_str(
            r#"{"input_tokens":1,"output_tokens":2,"cache_creation":{"ephemeral_1h_input_tokens":3},"server_tool_use":{"web_search_requests":1},"service_tier":"standard"}"#,
        )
        .unwrap();
        assert_eq!(usage.input_tokens, 1);
        assert_eq!(
            usage
                .cache_creation
                .as_ref()
                .and_then(|c| c.ephemeral_1h_input_tokens),
            Some(3)
        );
        let plain = MessagesUsage {
            input_tokens: 1,
            output_tokens: 2,
            ..Default::default()
        };
        let v = serde_json::to_value(&plain).unwrap();
        assert!(v.get("cache_creation").is_none());
        assert!(v.get("server_tool_use").is_none());
        assert!(v.get("service_tier").is_none());
    }
}
