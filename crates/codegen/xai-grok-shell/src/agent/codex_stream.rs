//! Live stream events from the Codex app-server JSONL protocol.
//!
//! The app-server already emits a rich event surface (`item/started`,
//! `item/agentMessage/delta`, `item/reasoning/*`, tool progress, plan updates,
//! etc.). This module classifies those notifications so the host can forward
//! them into ACP session updates while a turn is still running.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use agent_client_protocol as acp;
use serde_json::Value;

/// Live progress from one Codex app-server turn.
///
/// Events are best-effort: unknown fields are ignored so protocol additions do
/// not break the bridge. Consumers should treat `ItemCompleted` as the
/// authoritative final state for a tool item.
#[derive(Clone, Debug, PartialEq)]
pub enum CodexStreamEvent {
    /// `turn/started`
    TurnStarted { turn_id: Option<String> },
    /// Streamed assistant text (`item/agentMessage/delta`).
    AgentMessageDelta { item_id: Option<String>, text: String },
    /// Streamed reasoning summary or raw reasoning text.
    ReasoningDelta { item_id: Option<String>, text: String },
    /// Streamed plan-mode body (`item/plan/delta`).
    PlanDelta { item_id: Option<String>, text: String },
    /// A new item began (`item/started`).
    ItemStarted(CodexItemSnapshot),
    /// Incremental tool output (`item/commandExecution/outputDelta`, MCP progress).
    ItemOutputDelta {
        item_id: String,
        text: String,
    },
    /// File-change patch snapshot (`item/fileChange/patchUpdated`).
    FileChangePatch {
        item_id: Option<String>,
        changes: Vec<CodexFileChange>,
    },
    /// Item finished (`item/completed`).
    ItemCompleted(CodexItemSnapshot),
    /// Aggregated turn-level unified diff (`turn/diff/updated`).
    TurnDiffUpdated { diff: String },
    /// Structured plan steps (`turn/plan/updated`).
    TurnPlanUpdated {
        explanation: Option<String>,
        steps: Vec<CodexPlanStep>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexPlanStep {
    pub step: String,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexFileChange {
    pub path: PathBuf,
    pub kind: Option<String>,
    pub diff: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodexItemKind {
    AgentMessage,
    Reasoning,
    Plan,
    CommandExecution,
    FileChange,
    McpToolCall,
    DynamicToolCall,
    CollabToolCall,
    WebSearch,
    ImageView,
    Sleep,
    ContextCompaction,
    Review,
    Other,
}

impl CodexItemKind {
    pub fn from_type(item_type: &str) -> Self {
        match item_type {
            "agentMessage" => Self::AgentMessage,
            "reasoning" => Self::Reasoning,
            "plan" => Self::Plan,
            "commandExecution" => Self::CommandExecution,
            "fileChange" => Self::FileChange,
            "mcpToolCall" => Self::McpToolCall,
            "dynamicToolCall" => Self::DynamicToolCall,
            "collabToolCall" | "collabAgentToolCall" => Self::CollabToolCall,
            "webSearch" => Self::WebSearch,
            "imageView" => Self::ImageView,
            "sleep" => Self::Sleep,
            "contextCompaction" | "compacted" => Self::ContextCompaction,
            "enteredReviewMode" | "exitedReviewMode" => Self::Review,
            _ => Self::Other,
        }
    }

    /// Whether this item should surface as an ACP tool call card.
    pub fn is_tool_like(self) -> bool {
        matches!(
            self,
            Self::CommandExecution
                | Self::FileChange
                | Self::McpToolCall
                | Self::DynamicToolCall
                | Self::CollabToolCall
                | Self::WebSearch
                | Self::ImageView
                | Self::Sleep
                | Self::ContextCompaction
                | Self::Review
                | Self::Other
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CodexItemSnapshot {
    pub id: String,
    pub item_type: String,
    pub kind: CodexItemKind,
    pub title: String,
    pub status: Option<String>,
    pub raw: Value,
    pub text: Option<String>,
    pub output: Option<String>,
    pub locations: Vec<PathBuf>,
    pub changes: Vec<CodexFileChange>,
}

/// Classify one app-server notification into zero or more stream events.
pub fn classify_notification(message: &Value) -> Vec<CodexStreamEvent> {
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        return Vec::new();
    };
    let params = message.get("params").unwrap_or(&Value::Null);

    match method {
        "turn/started" => vec![CodexStreamEvent::TurnStarted {
            turn_id: params
                .pointer("/turn/id")
                .and_then(Value::as_str)
                .map(str::to_owned),
        }],
        "item/agentMessage/delta" => delta_event(params, |item_id, text| {
            CodexStreamEvent::AgentMessageDelta { item_id, text }
        }),
        "item/plan/delta" => delta_event(params, |item_id, text| CodexStreamEvent::PlanDelta {
            item_id,
            text,
        }),
        "item/reasoning/summaryTextDelta" | "item/reasoning/textDelta" => {
            delta_event(params, |item_id, text| CodexStreamEvent::ReasoningDelta {
                item_id,
                text,
            })
        }
        "item/reasoning/summaryPartAdded" => {
            // Section boundary — render a blank line so summaries stay readable.
            let item_id = string_field(params, &["itemId", "item_id"]);
            vec![CodexStreamEvent::ReasoningDelta {
                item_id,
                text: "\n".to_owned(),
            }]
        }
        "item/commandExecution/outputDelta" | "item/fileChange/outputDelta" => {
            let item_id = string_field(params, &["itemId", "item_id"])
                .unwrap_or_else(|| "unknown".to_owned());
            let text = string_field(params, &["delta", "output", "text"]).unwrap_or_default();
            if text.is_empty() {
                Vec::new()
            } else {
                vec![CodexStreamEvent::ItemOutputDelta { item_id, text }]
            }
        }
        "item/mcpToolCall/progress" => {
            let item_id = string_field(params, &["itemId", "item_id"])
                .unwrap_or_else(|| "unknown".to_owned());
            let text = string_field(params, &["message", "delta", "text", "progress"])
                .or_else(|| {
                    params
                        .get("progress")
                        .and_then(|p| serde_json::to_string(p).ok())
                })
                .unwrap_or_default();
            if text.is_empty() {
                Vec::new()
            } else {
                vec![CodexStreamEvent::ItemOutputDelta {
                    item_id,
                    text: format!("{text}\n"),
                }]
            }
        }
        "item/fileChange/patchUpdated" => {
            let item_id = string_field(params, &["itemId", "item_id"]);
            let changes = parse_file_changes(
                params
                    .get("changes")
                    .or_else(|| params.pointer("/item/changes"))
                    .unwrap_or(&Value::Null),
            );
            if changes.is_empty() {
                Vec::new()
            } else {
                vec![CodexStreamEvent::FileChangePatch { item_id, changes }]
            }
        }
        "item/started" => parse_item(params.get("item").unwrap_or(params))
            .map(|item| vec![CodexStreamEvent::ItemStarted(item)])
            .unwrap_or_default(),
        "item/completed" => parse_item(params.get("item").unwrap_or(params))
            .map(|item| vec![CodexStreamEvent::ItemCompleted(item)])
            .unwrap_or_default(),
        "turn/diff/updated" => string_field(params, &["diff"])
            .filter(|d| !d.is_empty())
            .map(|diff| vec![CodexStreamEvent::TurnDiffUpdated { diff }])
            .unwrap_or_default(),
        "turn/plan/updated" => {
            let explanation = string_field(params, &["explanation"]);
            let steps = params
                .get("plan")
                .and_then(Value::as_array)
                .map(|entries| {
                    entries
                        .iter()
                        .filter_map(|entry| {
                            let step = string_field(entry, &["step", "content", "text"])?;
                            let status =
                                string_field(entry, &["status"]).unwrap_or_else(|| "pending".into());
                            Some(CodexPlanStep { step, status })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if explanation.is_none() && steps.is_empty() {
                Vec::new()
            } else {
                vec![CodexStreamEvent::TurnPlanUpdated { explanation, steps }]
            }
        }
        _ => Vec::new(),
    }
}

fn delta_event(
    params: &Value,
    build: impl FnOnce(Option<String>, String) -> CodexStreamEvent,
) -> Vec<CodexStreamEvent> {
    let text = string_field(params, &["delta", "text"]).unwrap_or_default();
    if text.is_empty() {
        return Vec::new();
    }
    let item_id = string_field(params, &["itemId", "item_id"]);
    vec![build(item_id, text)]
}

fn parse_item(item: &Value) -> Option<CodexItemSnapshot> {
    if item.is_null() {
        return None;
    }
    let item_type = item
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_owned();
    let kind = CodexItemKind::from_type(&item_type);
    let id = string_field(item, &["id"]).unwrap_or_else(|| format!("{item_type}:anonymous"));
    let status = string_field(item, &["status"]);
    let text = string_field(item, &["text"])
        .or_else(|| string_field(item, &["review"]))
        .or_else(|| {
            // Reasoning summary array → join for fallback display.
            item.get("summary").and_then(|s| {
                if let Some(arr) = s.as_array() {
                    let joined = arr
                        .iter()
                        .filter_map(|v| v.as_str().or_else(|| v.get("text").and_then(Value::as_str)))
                        .collect::<Vec<_>>()
                        .join("\n");
                    if joined.is_empty() {
                        None
                    } else {
                        Some(joined)
                    }
                } else {
                    s.as_str().map(str::to_owned)
                }
            })
        });
    let output = string_field(item, &["aggregatedOutput", "aggregated_output", "output"])
        .or_else(|| {
            item.get("result")
                .and_then(|r| serde_json::to_string_pretty(r).ok())
        });
    let changes = parse_file_changes(item.get("changes").unwrap_or(&Value::Null));
    let mut locations = changes.iter().map(|c| c.path.clone()).collect::<Vec<_>>();
    if let Some(path) = string_field(item, &["path", "cwd"]) {
        let pb = PathBuf::from(&path);
        if !locations.iter().any(|p| p == &pb) {
            locations.push(pb);
        }
    }
    let title = item_title(kind, item, &item_type, text.as_deref(), &changes);
    Some(CodexItemSnapshot {
        id,
        item_type,
        kind,
        title,
        status,
        raw: item.clone(),
        text,
        output,
        locations,
        changes,
    })
}

fn item_title(
    kind: CodexItemKind,
    item: &Value,
    item_type: &str,
    text: Option<&str>,
    changes: &[CodexFileChange],
) -> String {
    match kind {
        CodexItemKind::CommandExecution => string_field(item, &["command"])
            .map(|cmd| format!("$ {cmd}"))
            .unwrap_or_else(|| "Command".to_owned()),
        CodexItemKind::FileChange => {
            if changes.is_empty() {
                "Edit files".to_owned()
            } else if changes.len() == 1 {
                format!("Edit `{}`", changes[0].path.display())
            } else {
                format!("Edit {} files", changes.len())
            }
        }
        CodexItemKind::McpToolCall => {
            let server = string_field(item, &["server"]).unwrap_or_else(|| "mcp".into());
            let tool = string_field(item, &["tool"]).unwrap_or_else(|| "tool".into());
            format!("{server}/{tool}")
        }
        CodexItemKind::DynamicToolCall => string_field(item, &["tool", "name"])
            .map(|name| format!("Host tool `{name}`"))
            .unwrap_or_else(|| "Host tool".to_owned()),
        CodexItemKind::CollabToolCall => string_field(item, &["tool"])
            .map(|tool| format!("Collab `{tool}`"))
            .unwrap_or_else(|| "Collaboration".to_owned()),
        CodexItemKind::WebSearch => string_field(item, &["query"])
            .map(|q| format!("Web search: \"{q}\""))
            .unwrap_or_else(|| "Web search".to_owned()),
        CodexItemKind::ImageView => string_field(item, &["path"])
            .map(|p| format!("View `{p}`"))
            .unwrap_or_else(|| "View image".to_owned()),
        CodexItemKind::Sleep => {
            let ms = item
                .get("durationMs")
                .or_else(|| item.get("duration_ms"))
                .and_then(Value::as_u64)
                .unwrap_or(0);
            format!("Sleep {ms}ms")
        }
        CodexItemKind::ContextCompaction => "Compacting context".to_owned(),
        CodexItemKind::Review => text
            .map(|t| {
                let first = t.lines().next().unwrap_or(t);
                if first.len() > 80 {
                    format!("Review: {}…", &first[..77])
                } else {
                    format!("Review: {first}")
                }
            })
            .unwrap_or_else(|| "Review".to_owned()),
        CodexItemKind::Plan => "Plan".to_owned(),
        CodexItemKind::Reasoning => "Reasoning".to_owned(),
        CodexItemKind::AgentMessage => "Message".to_owned(),
        CodexItemKind::Other => item_type.to_owned(),
    }
}

fn parse_file_changes(value: &Value) -> Vec<CodexFileChange> {
    let Some(entries) = value.as_array() else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| {
            let path = string_field(entry, &["path"])?;
            Some(CodexFileChange {
                path: PathBuf::from(path),
                kind: string_field(entry, &["kind"]),
                diff: string_field(entry, &["diff", "unifiedDiff", "unified_diff"]),
            })
        })
        .collect()
}

fn string_field(value: &Value, names: &[&str]) -> Option<String> {
    for name in names {
        if let Some(s) = value.get(*name).and_then(Value::as_str) {
            return Some(s.to_owned());
        }
    }
    None
}

// ── ACP projection ─────────────────────────────────────────────────────
//
// Shared by primary Codex turns and Codex subagents so both surfaces render
// the same native stream (text, reasoning, tools, plan).

/// Mutable projection state while a Codex turn streams into ACP.
#[derive(Default)]
pub struct CodexStreamUiState {
    pub chunk_index: u64,
    pub streamed_agent_text: bool,
    pub tool_outputs: HashMap<String, String>,
    pub tools_called: Vec<String>,
}

impl CodexStreamUiState {
    pub fn next_chunk_index(&mut self) -> u64 {
        let idx = self.chunk_index;
        self.chunk_index = self.chunk_index.saturating_add(1);
        idx
    }
}

/// One ACP update to emit for a stream event.
#[derive(Debug)]
pub struct CodexAcpChunk {
    pub update: acp::SessionUpdate,
    pub chunk_index: Option<u64>,
    /// True when this chunk is assistant text (for streaming capture / phase).
    pub is_agent_text: bool,
    /// True when this chunk is reasoning/thought text.
    pub is_reasoning: bool,
    /// True when this chunk opens or advances a tool-like item.
    pub is_tool: bool,
    /// Plain text payload when the chunk is message/thought text (for capture).
    pub text: Option<String>,
}

/// Project one stream event into zero or more ACP session updates.
pub fn stream_event_to_acp(
    event: CodexStreamEvent,
    ui: &mut CodexStreamUiState,
) -> Vec<CodexAcpChunk> {
    match event {
        CodexStreamEvent::TurnStarted { .. } => Vec::new(),
        CodexStreamEvent::AgentMessageDelta { text, .. } => {
            ui.streamed_agent_text = true;
            let chunk_index = ui.next_chunk_index();
            vec![CodexAcpChunk {
                update: acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
                    acp::ContentBlock::Text(acp::TextContent::new(text.clone())),
                )),
                chunk_index: Some(chunk_index),
                is_agent_text: true,
                is_reasoning: false,
                is_tool: false,
                text: Some(text.clone()),
            }]
        }
        CodexStreamEvent::ReasoningDelta { text, .. } => {
            let chunk_index = ui.next_chunk_index();
            vec![CodexAcpChunk {
                update: acp::SessionUpdate::AgentThoughtChunk(acp::ContentChunk::new(
                    acp::ContentBlock::Text(acp::TextContent::new(text.clone())),
                )),
                chunk_index: Some(chunk_index),
                is_agent_text: false,
                is_reasoning: true,
                is_tool: false,
                text: Some(text.clone()),
            }]
        }
        CodexStreamEvent::PlanDelta { text, .. } => {
            // Plan deltas render as thought so they stay out of the final
            // assistant bubble.
            let chunk_index = ui.next_chunk_index();
            vec![CodexAcpChunk {
                update: acp::SessionUpdate::AgentThoughtChunk(acp::ContentChunk::new(
                    acp::ContentBlock::Text(acp::TextContent::new(text.clone())),
                )),
                chunk_index: Some(chunk_index),
                is_agent_text: false,
                is_reasoning: false,
                is_tool: false,
                text: Some(text.clone()),
            }]
        }
        CodexStreamEvent::ItemStarted(item) => match item.kind {
            CodexItemKind::AgentMessage => Vec::new(),
            CodexItemKind::Reasoning | CodexItemKind::Plan => {
                let Some(text) = item.text.filter(|t| !t.is_empty()) else {
                    return Vec::new();
                };
                let chunk_index = ui.next_chunk_index();
                vec![CodexAcpChunk {
                    update: acp::SessionUpdate::AgentThoughtChunk(acp::ContentChunk::new(
                        acp::ContentBlock::Text(acp::TextContent::new(text.clone())),
                    )),
                    chunk_index: Some(chunk_index),
                    is_agent_text: false,
                    is_reasoning: item.kind == CodexItemKind::Reasoning,
                    is_tool: false,
                    text: Some(text.clone()),
                }]
            }
            kind if kind.is_tool_like() => {
                ui.tools_called.push(item.title.clone());
                ui.tool_outputs.entry(item.id.clone()).or_default();
                let tool_call_id =
                    acp::ToolCallId::new(Arc::<str>::from(format!("codex:{}", item.id)));
                let (kind_acp, content, locations) = item_presentation(&item);
                vec![CodexAcpChunk {
                    update: acp::SessionUpdate::ToolCall(
                        acp::ToolCall::new(tool_call_id, item.title)
                            .kind(kind_acp)
                            .status(acp::ToolCallStatus::InProgress)
                            .content(content)
                            .locations(locations)
                            .raw_input(Some(item.raw)),
                    ),
                    chunk_index: None,
                    is_agent_text: false,
                    is_reasoning: false,
                    is_tool: true,
                    text: None,
                }]
            }
            _ => Vec::new(),
        },
        CodexStreamEvent::ItemOutputDelta { item_id, text } => {
            let accumulated = ui.tool_outputs.entry(item_id.clone()).or_default();
            accumulated.push_str(&text);
            let tool_call_id =
                acp::ToolCallId::new(Arc::<str>::from(format!("codex:{item_id}")));
            vec![CodexAcpChunk {
                update: acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate::new(
                    tool_call_id,
                    acp::ToolCallUpdateFields::new()
                        .status(Some(acp::ToolCallStatus::InProgress))
                        .content(Some(vec![acp::ToolCallContent::from(
                            acp::ContentBlock::Text(acp::TextContent::new(accumulated.clone())),
                        )])),
                )),
                chunk_index: None,
                is_agent_text: false,
                is_reasoning: false,
                is_tool: true,
                text: None,
            }]
        }
        CodexStreamEvent::FileChangePatch { item_id, changes } => {
            let Some(item_id) = item_id else {
                return Vec::new();
            };
            let tool_call_id =
                acp::ToolCallId::new(Arc::<str>::from(format!("codex:{item_id}")));
            let content = file_change_content(&changes);
            let locations = changes
                .iter()
                .map(|c| acp::ToolCallLocation::new(c.path.clone()))
                .collect();
            vec![CodexAcpChunk {
                update: acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate::new(
                    tool_call_id,
                    acp::ToolCallUpdateFields::new()
                        .status(Some(acp::ToolCallStatus::InProgress))
                        .kind(Some(acp::ToolKind::Edit))
                        .content(Some(content))
                        .locations(Some(locations)),
                )),
                chunk_index: None,
                is_agent_text: false,
                is_reasoning: false,
                is_tool: true,
                text: None,
            }]
        }
        CodexStreamEvent::ItemCompleted(item) => match item.kind {
            CodexItemKind::AgentMessage => {
                if ui.streamed_agent_text {
                    return Vec::new();
                }
                let Some(text) = item.text.filter(|t| !t.trim().is_empty()) else {
                    return Vec::new();
                };
                ui.streamed_agent_text = true;
                let chunk_index = ui.next_chunk_index();
                vec![CodexAcpChunk {
                    update: acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
                        acp::ContentBlock::Text(acp::TextContent::new(text.clone())),
                    )),
                    chunk_index: Some(chunk_index),
                    is_agent_text: true,
                    is_reasoning: false,
                    is_tool: false,
                    text: Some(text),
                }]
            }
            CodexItemKind::Reasoning | CodexItemKind::Plan => Vec::new(),
            kind if kind.is_tool_like() => {
                let tool_call_id =
                    acp::ToolCallId::new(Arc::<str>::from(format!("codex:{}", item.id)));
                let status = item_status(item.status.as_deref());
                let mut content = item_presentation(&item).1;
                if content.is_empty() {
                    if let Some(output) = ui.tool_outputs.get(&item.id).cloned() {
                        if !output.is_empty() {
                            content.push(acp::ToolCallContent::from(acp::ContentBlock::Text(
                                acp::TextContent::new(output),
                            )));
                        }
                    } else if let Some(output) = item.output.clone() {
                        content.push(acp::ToolCallContent::from(acp::ContentBlock::Text(
                            acp::TextContent::new(output),
                        )));
                    } else if let Some(text) = item.text.clone() {
                        content.push(acp::ToolCallContent::from(acp::ContentBlock::Text(
                            acp::TextContent::new(text.clone()),
                        )));
                    }
                }
                let locations = item
                    .locations
                    .iter()
                    .map(|p| acp::ToolCallLocation::new(p.clone()))
                    .collect::<Vec<_>>();
                vec![CodexAcpChunk {
                    update: acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate::new(
                        tool_call_id,
                        acp::ToolCallUpdateFields::new()
                            .status(Some(status))
                            .content(Some(content))
                            .locations(Some(locations))
                            .raw_output(Some(item.raw)),
                    )),
                    chunk_index: None,
                    is_agent_text: false,
                    is_reasoning: false,
                    is_tool: true,
                    text: None,
                }]
            }
            _ => Vec::new(),
        },
        CodexStreamEvent::TurnDiffUpdated { .. } => Vec::new(),
        CodexStreamEvent::TurnPlanUpdated {
            explanation,
            steps,
        } => {
            let mut out = Vec::new();
            if let Some(explanation) = explanation.filter(|e| !e.is_empty()) {
                let chunk_index = ui.next_chunk_index();
                out.push(CodexAcpChunk {
                    update: acp::SessionUpdate::AgentThoughtChunk(acp::ContentChunk::new(
                        acp::ContentBlock::Text(acp::TextContent::new(explanation.clone())),
                    )),
                    chunk_index: Some(chunk_index),
                    is_agent_text: false,
                    is_reasoning: true,
                    is_tool: false,
                    text: Some(explanation.clone()),
                });
            }
            if !steps.is_empty() {
                let entries = steps
                    .into_iter()
                    .map(|step| {
                        acp::PlanEntry::new(
                            step.step,
                            acp::PlanEntryPriority::Medium,
                            plan_status(&step.status),
                        )
                    })
                    .collect();
                out.push(CodexAcpChunk {
                    update: acp::SessionUpdate::Plan(acp::Plan::new(entries)),
                    chunk_index: None,
                    is_agent_text: false,
                    is_reasoning: false,
                    is_tool: false,
                    text: None,
                });
            }
            out
        }
    }
}

fn item_status(status: Option<&str>) -> acp::ToolCallStatus {
    match status {
        Some("completed") => acp::ToolCallStatus::Completed,
        Some("failed") | Some("declined") | Some("interrupted") => acp::ToolCallStatus::Failed,
        Some("inProgress") | Some("in_progress") => acp::ToolCallStatus::InProgress,
        _ => acp::ToolCallStatus::Completed,
    }
}

fn plan_status(status: &str) -> acp::PlanEntryStatus {
    match status {
        "completed" => acp::PlanEntryStatus::Completed,
        "inProgress" | "in_progress" => acp::PlanEntryStatus::InProgress,
        _ => acp::PlanEntryStatus::Pending,
    }
}

fn file_change_content(changes: &[CodexFileChange]) -> Vec<acp::ToolCallContent> {
    changes
        .iter()
        .map(|change| {
            let body = change.diff.clone().unwrap_or_else(|| {
                format!(
                    "{} ({})",
                    change.path.display(),
                    change.kind.as_deref().unwrap_or("change")
                )
            });
            acp::ToolCallContent::from(acp::Diff::new(change.path.clone(), body))
        })
        .collect()
}

fn item_presentation(
    item: &CodexItemSnapshot,
) -> (
    acp::ToolKind,
    Vec<acp::ToolCallContent>,
    Vec<acp::ToolCallLocation>,
) {
    let kind = match item.kind {
        CodexItemKind::CommandExecution => acp::ToolKind::Execute,
        CodexItemKind::FileChange => acp::ToolKind::Edit,
        CodexItemKind::WebSearch => acp::ToolKind::Search,
        CodexItemKind::ImageView => acp::ToolKind::Read,
        CodexItemKind::ContextCompaction | CodexItemKind::Sleep | CodexItemKind::Review => {
            acp::ToolKind::Think
        }
        CodexItemKind::McpToolCall
        | CodexItemKind::DynamicToolCall
        | CodexItemKind::CollabToolCall
        | CodexItemKind::Other => acp::ToolKind::Other,
        CodexItemKind::AgentMessage | CodexItemKind::Reasoning | CodexItemKind::Plan => {
            acp::ToolKind::Think
        }
    };
    let mut content = if !item.changes.is_empty() {
        file_change_content(&item.changes)
    } else {
        Vec::new()
    };
    if content.is_empty() {
        if let Some(output) = item.output.as_ref().filter(|s| !s.is_empty()) {
            content.push(acp::ToolCallContent::from(acp::ContentBlock::Text(
                acp::TextContent::new(output.clone()),
            )));
        } else if let Some(text) = item.text.as_ref().filter(|s| !s.is_empty()) {
            content.push(acp::ToolCallContent::from(acp::ContentBlock::Text(
                acp::TextContent::new(text.clone()),
            )));
        }
    }
    let locations = item
        .locations
        .iter()
        .map(|p| acp::ToolCallLocation::new(p.clone()))
        .collect();
    (kind, content, locations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn classifies_agent_message_deltas() {
        let events = classify_notification(&json!({
            "method": "item/agentMessage/delta",
            "params": {"itemId": "m1", "delta": "hello"}
        }));
        assert_eq!(
            events,
            vec![CodexStreamEvent::AgentMessageDelta {
                item_id: Some("m1".into()),
                text: "hello".into(),
            }]
        );
    }

    #[test]
    fn classifies_command_lifecycle() {
        let started = classify_notification(&json!({
            "method": "item/started",
            "params": {
                "item": {
                    "id": "cmd-1",
                    "type": "commandExecution",
                    "command": "rg foo",
                    "status": "inProgress"
                }
            }
        }));
        assert!(matches!(
            &started[0],
            CodexStreamEvent::ItemStarted(item)
                if item.id == "cmd-1"
                    && item.kind == CodexItemKind::CommandExecution
                    && item.title == "$ rg foo"
        ));

        let delta = classify_notification(&json!({
            "method": "item/commandExecution/outputDelta",
            "params": {"itemId": "cmd-1", "delta": "match\n"}
        }));
        assert_eq!(
            delta,
            vec![CodexStreamEvent::ItemOutputDelta {
                item_id: "cmd-1".into(),
                text: "match\n".into(),
            }]
        );

        let completed = classify_notification(&json!({
            "method": "item/completed",
            "params": {
                "item": {
                    "id": "cmd-1",
                    "type": "commandExecution",
                    "command": "rg foo",
                    "status": "completed",
                    "aggregatedOutput": "match\n",
                    "exitCode": 0
                }
            }
        }));
        assert!(matches!(
            &completed[0],
            CodexStreamEvent::ItemCompleted(item)
                if item.status.as_deref() == Some("completed")
                    && item.output.as_deref() == Some("match\n")
        ));
    }

    #[test]
    fn classifies_plan_and_reasoning() {
        let reasoning = classify_notification(&json!({
            "method": "item/reasoning/summaryTextDelta",
            "params": {"itemId": "r1", "delta": "thinking"}
        }));
        assert!(matches!(
            &reasoning[0],
            CodexStreamEvent::ReasoningDelta { text, .. } if text == "thinking"
        ));

        let plan = classify_notification(&json!({
            "method": "turn/plan/updated",
            "params": {
                "explanation": "Approach",
                "plan": [
                    {"step": "Inspect", "status": "completed"},
                    {"step": "Fix", "status": "inProgress"}
                ]
            }
        }));
        assert!(matches!(
            &plan[0],
            CodexStreamEvent::TurnPlanUpdated { explanation, steps }
                if explanation.as_deref() == Some("Approach") && steps.len() == 2
        ));
    }

    #[test]
    fn classifies_file_change() {
        let events = classify_notification(&json!({
            "method": "item/started",
            "params": {
                "item": {
                    "id": "fc-1",
                    "type": "fileChange",
                    "status": "inProgress",
                    "changes": [{
                        "path": "src/main.rs",
                        "kind": "update",
                        "diff": "@@ -1 +1 @@\n-old\n+new\n"
                    }]
                }
            }
        }));
        match &events[0] {
            CodexStreamEvent::ItemStarted(item) => {
                assert_eq!(item.kind, CodexItemKind::FileChange);
                assert_eq!(item.changes.len(), 1);
                assert_eq!(item.changes[0].path, PathBuf::from("src/main.rs"));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn projects_native_stream_into_acp_updates() {
        let mut ui = CodexStreamUiState::default();
        let text = stream_event_to_acp(
            CodexStreamEvent::AgentMessageDelta {
                item_id: Some("m1".into()),
                text: "hello".into(),
            },
            &mut ui,
        );
        assert!(ui.streamed_agent_text);
        assert!(matches!(
            text[0].update,
            acp::SessionUpdate::AgentMessageChunk(_)
        ));

        let tool = stream_event_to_acp(
            CodexStreamEvent::ItemStarted(CodexItemSnapshot {
                id: "cmd-1".into(),
                item_type: "commandExecution".into(),
                kind: CodexItemKind::CommandExecution,
                title: "$ echo hi".into(),
                status: Some("inProgress".into()),
                raw: json!({"id":"cmd-1","type":"commandExecution","command":"echo hi"}),
                text: None,
                output: None,
                locations: Vec::new(),
                changes: Vec::new(),
            }),
            &mut ui,
        );
        assert_eq!(ui.tools_called, vec!["$ echo hi"]);
        assert!(matches!(tool[0].update, acp::SessionUpdate::ToolCall(_)));

        // After live text streamed, completed agentMessage must not re-emit.
        let completed = stream_event_to_acp(
            CodexStreamEvent::ItemCompleted(CodexItemSnapshot {
                id: "m1".into(),
                item_type: "agentMessage".into(),
                kind: CodexItemKind::AgentMessage,
                title: "Message".into(),
                status: None,
                raw: json!({}),
                text: Some("hello world".into()),
                output: None,
                locations: Vec::new(),
                changes: Vec::new(),
            }),
            &mut ui,
        );
        assert!(completed.is_empty());
    }
}
