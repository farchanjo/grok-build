//! NDJSON stream-json protocol parser for Claude Agent CLI.
//!
//! Normalizes bounded system/init, assistant text/thinking, stream partial
//! events, tool-use/result (display/audit only), API retry/status, subagent
//! text + parent_tool_use_id, final result/usage/cost/model/session ID/
//! capabilities. Unknown events are ignored/preserved bounded for diagnostics.
//!
//! Never dispatches Claude tool events to the Grok tool executor.

use serde_json::Value;

use crate::agent::external_runtime::{
    ExternalResultMetadata, ExternalRuntimeTurnEvent, ExternalUsageMetadata,
};

/// Max chars retained for a single diagnostic unknown-event snippet.
pub const MAX_UNKNOWN_EVENT_SNIPPET: usize = 256;
/// Max unknown events retained per turn for diagnostics.
pub const MAX_UNKNOWN_EVENTS: usize = 16;
/// Max chars of assistant text retained in normalized events aggregate checks.
pub const MAX_TEXT_EVENT_CHARS: usize = 512 * 1024;

/// Normalized parse of one turn's NDJSON stream.
#[derive(Debug, Clone, Default)]
pub struct ParsedTurnStream {
    pub events: Vec<ExternalRuntimeTurnEvent>,
    pub session_id: Option<String>,
    pub model: Option<String>,
    pub capabilities: Vec<String>,
    pub version: Option<String>,
    pub result: Option<ExternalResultMetadata>,
    pub usage: Option<ExternalUsageMetadata>,
    pub total_cost_usd: Option<f64>,
    pub result_text: Option<String>,
    pub saw_final_result: bool,
    pub unknown_events: Vec<String>,
    pub errors: Vec<String>,
    /// `true` when at least one `stream_event` text_delta was accepted.
    /// When set, full assistant text blocks and result text are **not**
    /// re-emitted (dedupe against `--include-partial-messages`).
    pub saw_stream_text: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    MalformedLine { detail: String },
    InvalidUtf8,
    Truncated { detail: String },
    NoFinalResult,
    Oversized { detail: String },
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MalformedLine { detail } => {
                write!(f, "malformed Claude CLI NDJSON: {detail}")
            }
            Self::InvalidUtf8 => write!(f, "invalid UTF-8 in Claude CLI stream"),
            Self::Truncated { detail } => write!(f, "truncated Claude CLI stream: {detail}"),
            Self::NoFinalResult => {
                write!(f, "Claude CLI stream ended without a final result event")
            }
            Self::Oversized { detail } => write!(f, "oversized Claude CLI payload: {detail}"),
        }
    }
}

impl std::error::Error for ProtocolError {}

/// Parse all NDJSON lines from a completed turn.
///
/// Returns `Err` for hard failures (no final result when stream ended cleanly
/// without cancellation, malformed required structure). Soft unknowns accumulate
/// in `unknown_events`.
pub fn parse_turn_lines(lines: &[String]) -> Result<ParsedTurnStream, ProtocolError> {
    let mut parsed = ParsedTurnStream::default();
    let mut malformed = 0usize;

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.len() > MAX_TEXT_EVENT_CHARS {
            return Err(ProtocolError::Oversized {
                detail: format!("line length {}", trimmed.len()),
            });
        }
        match serde_json::from_str::<Value>(trimmed) {
            Ok(value) => {
                if let Err(e) = ingest_event(&mut parsed, &value) {
                    malformed += 1;
                    parsed.errors.push(e);
                }
            }
            Err(e) => {
                malformed += 1;
                if parsed.unknown_events.len() < MAX_UNKNOWN_EVENTS {
                    parsed.unknown_events.push(truncate_snippet(
                        &format!("json_error:{e}:{}", truncate_snippet(trimmed, 80)),
                        MAX_UNKNOWN_EVENT_SNIPPET,
                    ));
                }
            }
        }
    }

    if !parsed.saw_final_result {
        if malformed > 0 && parsed.events.is_empty() {
            return Err(ProtocolError::MalformedLine {
                detail: format!("{malformed} unparseable line(s) and no final result"),
            });
        }
        return Err(ProtocolError::NoFinalResult);
    }

    Ok(parsed)
}

/// Parse lines for a cancelled turn — final result is optional.
pub fn parse_turn_lines_allow_incomplete(lines: &[String]) -> ParsedTurnStream {
    let mut parsed = ParsedTurnStream::default();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
            let _ = ingest_event(&mut parsed, &value);
        }
    }
    parsed
}

fn ingest_event(parsed: &mut ParsedTurnStream, value: &Value) -> Result<(), String> {
    let ty = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match ty {
        "system" => ingest_system(parsed, value),
        "assistant" => ingest_assistant(parsed, value),
        "user" => ingest_user(parsed, value),
        "stream_event" => ingest_stream_event(parsed, value),
        "result" => ingest_result(parsed, value),
        "error" => {
            let msg = value
                .get("error")
                .or_else(|| value.get("message"))
                .and_then(|v| v.as_str())
                .unwrap_or("claude error event")
                .to_owned();
            parsed.events.push(ExternalRuntimeTurnEvent::Error {
                message: bound_text(&msg, 2048),
            });
            Ok(())
        }
        "" => Err("missing type field".into()),
        other => {
            push_unknown(parsed, other, value);
            Ok(())
        }
    }
}

fn ingest_system(parsed: &mut ParsedTurnStream, value: &Value) -> Result<(), String> {
    let subtype = value.get("subtype").and_then(|v| v.as_str()).unwrap_or("");
    match subtype {
        "init" => {
            if let Some(sid) = value.get("session_id").and_then(|v| v.as_str()) {
                parsed.session_id = Some(bound_text(sid, 512));
            }
            if let Some(model) = value.get("model").and_then(|v| v.as_str()) {
                parsed.model = Some(bound_text(model, 256));
            }
            if let Some(ver) = value
                .get("claude_code_version")
                .or_else(|| value.get("version"))
                .and_then(|v| v.as_str())
            {
                parsed.version = Some(bound_text(ver, 128));
            }
            if let Some(caps) = value.get("capabilities").and_then(|v| v.as_array()) {
                for c in caps {
                    if let Some(s) = c.as_str() {
                        if parsed.capabilities.len() < 64 {
                            parsed.capabilities.push(bound_text(s, 128));
                        }
                    }
                }
            }
            parsed.events.push(ExternalRuntimeTurnEvent::Status {
                message: "Claude Agent CLI session initialized".into(),
            });
            Ok(())
        }
        "api_retry" => {
            let attempt = value.get("attempt").and_then(|v| v.as_u64()).unwrap_or(0);
            let max = value
                .get("max_retries")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let err = value
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("retry");
            parsed.events.push(ExternalRuntimeTurnEvent::Status {
                message: bound_text(&format!("API retry {attempt}/{max}: {err}"), 512),
            });
            Ok(())
        }
        other => {
            push_unknown(parsed, &format!("system/{other}"), value);
            Ok(())
        }
    }
}

fn ingest_assistant(parsed: &mut ParsedTurnStream, value: &Value) -> Result<(), String> {
    let parent = value
        .get("parent_tool_use_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty() && *s != "null");

    // message.content array (Anthropic-style)
    let content = value
        .pointer("/message/content")
        .or_else(|| value.get("content"));

    if let Some(Value::Array(blocks)) = content {
        for block in blocks {
            let bty = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
            match bty {
                "text" => {
                    // When stream partials already delivered the text, skip the
                    // full assistant text block to avoid duplicate transcript.
                    if parsed.saw_stream_text {
                        continue;
                    }
                    if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                        let text = annotate_subagent(text, parent);
                        if !text.is_empty() {
                            parsed.events.push(ExternalRuntimeTurnEvent::TextDelta {
                                text: bound_text(&text, MAX_TEXT_EVENT_CHARS),
                            });
                        }
                    }
                }
                "thinking" => {
                    if let Some(text) = block
                        .get("thinking")
                        .or_else(|| block.get("text"))
                        .and_then(|v| v.as_str())
                    {
                        // Map thinking to status for MVP (no separate thought channel
                        // on ExternalRuntimeTurnEvent yet).
                        let text = annotate_subagent(text, parent);
                        parsed.events.push(ExternalRuntimeTurnEvent::Status {
                            message: bound_text(&format!("thinking: {text}"), 2048),
                        });
                    }
                }
                "tool_use" => {
                    // Display/audit only — never dispatch to Grok tools.
                    let name = block.get("name").and_then(|v| v.as_str()).unwrap_or("tool");
                    let id = block.get("id").and_then(|v| v.as_str());
                    let summary = id.map(|i| format!("id={i}"));
                    parsed.events.push(ExternalRuntimeTurnEvent::ToolCall {
                        name: bound_text(name, 128),
                        summary: summary.map(|s| bound_text(&s, 256)),
                    });
                }
                _ => {}
            }
        }
    } else if let Some(text) = value.get("text").and_then(|v| v.as_str()) {
        if !parsed.saw_stream_text {
            let text = annotate_subagent(text, parent);
            parsed.events.push(ExternalRuntimeTurnEvent::TextDelta {
                text: bound_text(&text, MAX_TEXT_EVENT_CHARS),
            });
        }
    }
    Ok(())
}

fn ingest_user(parsed: &mut ParsedTurnStream, value: &Value) -> Result<(), String> {
    // tool_result blocks — display/audit only.
    let content = value
        .pointer("/message/content")
        .or_else(|| value.get("content"));
    if let Some(Value::Array(blocks)) = content {
        for block in blocks {
            let bty = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if bty == "tool_result" {
                let tool_use_id = block
                    .get("tool_use_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                let is_error = block
                    .get("is_error")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let label = if is_error {
                    format!("tool_result error id={tool_use_id}")
                } else {
                    format!("tool_result id={tool_use_id}")
                };
                parsed.events.push(ExternalRuntimeTurnEvent::Status {
                    message: bound_text(&label, 256),
                });
            }
        }
    }
    Ok(())
}

fn ingest_stream_event(parsed: &mut ParsedTurnStream, value: &Value) -> Result<(), String> {
    // stream_event.event.delta.text for text_delta — authoritative channel when
    // --include-partial-messages is enabled.
    let event = value.get("event").unwrap_or(value);
    let delta = event.get("delta");
    if let Some(delta) = delta {
        let dty = delta.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if dty == "text_delta" || delta.get("text").is_some() {
            if let Some(text) = delta.get("text").and_then(|v| v.as_str()) {
                if !text.is_empty() {
                    parsed.saw_stream_text = true;
                    parsed.events.push(ExternalRuntimeTurnEvent::TextDelta {
                        text: bound_text(text, MAX_TEXT_EVENT_CHARS),
                    });
                }
            }
        }
        if dty == "thinking_delta" {
            if let Some(text) = delta
                .get("thinking")
                .or_else(|| delta.get("text"))
                .and_then(|v| v.as_str())
            {
                parsed.events.push(ExternalRuntimeTurnEvent::Status {
                    message: bound_text(&format!("thinking: {text}"), 1024),
                });
            }
        }
    }
    Ok(())
}

fn ingest_result(parsed: &mut ParsedTurnStream, value: &Value) -> Result<(), String> {
    parsed.saw_final_result = true;

    if let Some(sid) = value.get("session_id").and_then(|v| v.as_str()) {
        parsed.session_id = Some(bound_text(sid, 512));
    }
    if let Some(model) = value
        .get("model")
        .or_else(|| value.pointer("/usage/model"))
        .and_then(|v| v.as_str())
    {
        parsed.model = Some(bound_text(model, 256));
    }
    if let Some(cost) = value.get("total_cost_usd").and_then(|v| v.as_f64()) {
        parsed.total_cost_usd = Some(cost);
    }
    if let Some(text) = value.get("result").and_then(|v| v.as_str()) {
        parsed.result_text = Some(bound_text(text, MAX_TEXT_EVENT_CHARS));
        // Emit final result text only when no stream partials and no assistant
        // text deltas were already recorded (no-partial streams).
        let already_has_text = parsed
            .events
            .iter()
            .any(|e| matches!(e, ExternalRuntimeTurnEvent::TextDelta { .. }));
        if !parsed.saw_stream_text && !already_has_text && !text.is_empty() {
            parsed.events.push(ExternalRuntimeTurnEvent::TextDelta {
                text: bound_text(text, MAX_TEXT_EVENT_CHARS),
            });
        }
    }

    let is_error = value
        .get("is_error")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let subtype = value
        .get("subtype")
        .and_then(|v| v.as_str())
        .unwrap_or(if is_error { "error" } else { "success" });
    let stop = value
        .get("stop_reason")
        .and_then(|v| v.as_str())
        .map(|s| bound_text(s, 128));

    parsed.result = Some(ExternalResultMetadata {
        status: bound_text(subtype, 128),
        stop_reason: stop,
    });

    if let Some(usage) = value.get("usage") {
        parsed.usage = Some(ExternalUsageMetadata {
            input_tokens: usage
                .get("input_tokens")
                .or_else(|| usage.get("prompt_tokens"))
                .and_then(|v| v.as_u64()),
            output_tokens: usage
                .get("output_tokens")
                .or_else(|| usage.get("completion_tokens"))
                .and_then(|v| v.as_u64()),
            total_tokens: usage.get("total_tokens").and_then(|v| v.as_u64()),
        });
    }

    if is_error {
        let msg = value
            .get("result")
            .and_then(|v| v.as_str())
            .or_else(|| value.get("error").and_then(|v| v.as_str()))
            .unwrap_or("Claude CLI reported an error result");
        parsed.events.push(ExternalRuntimeTurnEvent::Error {
            message: bound_text(msg, 2048),
        });
    }

    Ok(())
}

fn annotate_subagent(text: &str, parent: Option<&str>) -> String {
    match parent {
        Some(id) => format!("[subagent:{id}] {text}"),
        None => text.to_owned(),
    }
}

fn push_unknown(parsed: &mut ParsedTurnStream, label: &str, value: &Value) {
    if parsed.unknown_events.len() >= MAX_UNKNOWN_EVENTS {
        return;
    }
    let snippet = truncate_snippet(&value.to_string(), MAX_UNKNOWN_EVENT_SNIPPET);
    parsed.unknown_events.push(format!("{label}:{snippet}"));
}

fn bound_text(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_owned()
    } else {
        let mut end = max;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &s[..end])
    }
}

fn truncate_snippet(s: &str, max: usize) -> String {
    bound_text(s, max)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn parses_init_text_result() {
        let ls = lines(&[
            r#"{"type":"system","subtype":"init","session_id":"sess-1","model":"claude-sonnet","capabilities":["interrupt_receipt_v1"]}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Hello"}]}}"#,
            r#"{"type":"result","session_id":"sess-1","result":"Hello","subtype":"success","usage":{"input_tokens":10,"output_tokens":2},"total_cost_usd":0.001}"#,
        ]);
        let p = parse_turn_lines(&ls).unwrap();
        assert!(p.saw_final_result);
        assert_eq!(p.session_id.as_deref(), Some("sess-1"));
        assert_eq!(p.capabilities, vec!["interrupt_receipt_v1".to_owned()]);
        assert!(
            p.events.iter().any(
                |e| matches!(e, ExternalRuntimeTurnEvent::TextDelta { text } if text == "Hello")
            )
        );
        assert_eq!(p.usage.as_ref().unwrap().input_tokens, Some(10));
    }

    #[test]
    fn tool_use_is_display_only() {
        let ls = lines(&[
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t1","name":"Bash","input":{"command":"ls"}}]}}"#,
            r#"{"type":"result","subtype":"success","result":"done","session_id":"s"}"#,
        ]);
        let p = parse_turn_lines(&ls).unwrap();
        assert!(p.events.iter().any(
            |e| matches!(e, ExternalRuntimeTurnEvent::ToolCall { name, .. } if name == "Bash")
        ));
    }

    #[test]
    fn api_retry_status() {
        let ls = lines(&[
            r#"{"type":"system","subtype":"api_retry","attempt":1,"max_retries":3,"error":"rate_limit"}"#,
            r#"{"type":"result","subtype":"success","result":"ok"}"#,
        ]);
        let p = parse_turn_lines(&ls).unwrap();
        assert!(
            p.events
                .iter()
                .any(|e| matches!(e, ExternalRuntimeTurnEvent::Status { message } if message.contains("rate_limit")))
        );
    }

    #[test]
    fn subagent_parent_tool_use_id() {
        let ls = lines(&[
            r#"{"type":"assistant","parent_tool_use_id":"tool_99","message":{"content":[{"type":"text","text":"from sub"}]}}"#,
            r#"{"type":"result","subtype":"success","result":"ok"}"#,
        ]);
        let p = parse_turn_lines(&ls).unwrap();
        assert!(p.events.iter().any(|e| matches!(
            e,
            ExternalRuntimeTurnEvent::TextDelta { text } if text.contains("subagent:tool_99")
        )));
    }

    #[test]
    fn no_final_result_errors() {
        let ls =
            lines(&[r#"{"type":"assistant","message":{"content":[{"type":"text","text":"x"}]}}"#]);
        let err = parse_turn_lines(&ls).unwrap_err();
        assert!(matches!(err, ProtocolError::NoFinalResult));
    }

    #[test]
    fn malformed_only_errors() {
        let ls = lines(&["not-json{{{", "also bad"]);
        let err = parse_turn_lines(&ls).unwrap_err();
        assert!(matches!(
            err,
            ProtocolError::NoFinalResult | ProtocolError::MalformedLine { .. }
        ));
    }

    #[test]
    fn unknown_events_bounded() {
        let mut ls = Vec::new();
        for i in 0..30 {
            ls.push(format!(r#"{{"type":"future_event_{i}","x":1}}"#));
        }
        ls.push(r#"{"type":"result","subtype":"success","result":"ok"}"#.into());
        let p = parse_turn_lines(&ls).unwrap();
        assert!(p.unknown_events.len() <= MAX_UNKNOWN_EVENTS);
    }

    #[test]
    fn stream_event_text_delta() {
        let ls = lines(&[
            r#"{"type":"stream_event","event":{"delta":{"type":"text_delta","text":"Hi"}}}"#,
            r#"{"type":"result","subtype":"success","result":"Hi"}"#,
        ]);
        let p = parse_turn_lines(&ls).unwrap();
        assert!(
            p.events
                .iter()
                .any(|e| matches!(e, ExternalRuntimeTurnEvent::TextDelta { text } if text == "Hi"))
        );
    }

    #[test]
    fn partial_plus_assistant_plus_result_emits_single_logical_transcript() {
        let ls = lines(&[
            r#"{"type":"stream_event","event":{"delta":{"type":"text_delta","text":"Hel"}}}"#,
            r#"{"type":"stream_event","event":{"delta":{"type":"text_delta","text":"lo"}}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Hello"},{"type":"tool_use","id":"t1","name":"Read"}]}}"#,
            r#"{"type":"result","subtype":"success","result":"Hello","session_id":"s1"}"#,
        ]);
        let p = parse_turn_lines(&ls).unwrap();
        assert!(p.saw_stream_text);
        let texts: Vec<&str> = p
            .events
            .iter()
            .filter_map(|e| match e {
                ExternalRuntimeTurnEvent::TextDelta { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        // Stream partials only — no duplicate full assistant/result text.
        assert_eq!(texts, vec!["Hel", "lo"]);
        // Tools from assistant still recorded.
        assert!(p.events.iter().any(
            |e| matches!(e, ExternalRuntimeTurnEvent::ToolCall { name, .. } if name == "Read")
        ));
    }

    #[test]
    fn no_partials_uses_assistant_or_result_text() {
        let ls = lines(&[
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Only full"}]}}"#,
            r#"{"type":"result","subtype":"success","result":"Only full"}"#,
        ]);
        let p = parse_turn_lines(&ls).unwrap();
        assert!(!p.saw_stream_text);
        let texts: Vec<&str> = p
            .events
            .iter()
            .filter_map(|e| match e {
                ExternalRuntimeTurnEvent::TextDelta { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        // Assistant text once; result does not re-emit because text already present.
        assert_eq!(texts, vec!["Only full"]);
    }

    #[test]
    fn result_only_stream_emits_result_text() {
        let ls = lines(&[r#"{"type":"result","subtype":"success","result":"solo"}"#]);
        let p = parse_turn_lines(&ls).unwrap();
        assert!(
            p.events.iter().any(
                |e| matches!(e, ExternalRuntimeTurnEvent::TextDelta { text } if text == "solo")
            )
        );
    }
}
