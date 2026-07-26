//! NDJSON stream-json protocol parser for Claude Agent CLI.
//!
//! Normalizes bounded system/init, assistant text/thinking, stream partial
//! events, tool-use/result (display/audit only), API retry/status, subagent
//! text + parent_tool_use_id, final result/usage/cost/model/session ID/
//! capabilities. Unknown events are ignored/preserved bounded for diagnostics.
//!
//! Text dedupe is **segment/message-aware** (not turn-global): stream partials
//! are authoritative for a segment keyed by `parent_tool_use_id` (empty = main).
//! A matching full assistant echo for that segment is skipped once; later
//! post-tool assistant messages and unstreamed subagent text are retained.
//!
//! Never dispatches Claude tool events to the Grok tool executor.

use std::collections::HashMap;

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

/// Segment key: empty string = main conversation; otherwise `parent_tool_use_id`.
type SegmentKey = String;

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
    /// Accumulated stream text per segment (for echo matching).
    #[doc(hidden)]
    streamed_text: HashMap<SegmentKey, String>,
    /// Segment has open stream deltas not yet consumed by a full assistant echo.
    #[doc(hidden)]
    pending_stream_echo: HashMap<SegmentKey, bool>,
    /// Whether any main-segment TextDelta was emitted (for result dedupe).
    #[doc(hidden)]
    main_text_emitted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    MalformedLine {
        detail: String,
    },
    /// Stream ended cleanly without a final `result` event (truncated / incomplete).
    NoFinalResult,
    Oversized {
        detail: String,
    },
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MalformedLine { detail } => {
                write!(f, "malformed Claude CLI NDJSON: {detail}")
            }
            Self::NoFinalResult => {
                write!(
                    f,
                    "Claude CLI stream ended without a final result event (truncated or incomplete)"
                )
            }
            Self::Oversized { detail } => write!(f, "oversized Claude CLI payload: {detail}"),
        }
    }
}

impl std::error::Error for ProtocolError {}

/// Parse all NDJSON lines from a completed turn.
///
/// Returns `Err(NoFinalResult)` when the stream ends without a final `result`
/// event (process-layer truncation / incomplete). Soft unknowns accumulate
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

fn segment_key(parent: Option<&str>) -> SegmentKey {
    parent.unwrap_or("").to_owned()
}

fn parent_from_value(value: &Value) -> Option<&str> {
    value
        .get("parent_tool_use_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty() && *s != "null")
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

fn should_skip_full_text_echo(parsed: &mut ParsedTurnStream, key: &str, full_text: &str) -> bool {
    let pending = parsed
        .pending_stream_echo
        .get(key)
        .copied()
        .unwrap_or(false);
    if !pending {
        return false;
    }
    let streamed = parsed
        .streamed_text
        .get(key)
        .map(String::as_str)
        .unwrap_or("");
    // Match exact stream concat, or full text equals stream (common).
    // Also treat as echo when full starts with stream and stream is non-empty
    // (minor trailing whitespace differences stripped).
    let full_trim = full_text.trim();
    let stream_trim = streamed.trim();
    let is_echo = !stream_trim.is_empty()
        && (full_trim == stream_trim
            || full_trim == streamed
            || streamed == full_text
            || full_trim.starts_with(stream_trim));
    if is_echo {
        // Consume this stream segment so a later post-tool message can emit.
        parsed.pending_stream_echo.insert(key.to_owned(), false);
        parsed.streamed_text.insert(key.to_owned(), String::new());
        return true;
    }
    // Pending stream but text doesn't match — keep stream, still emit full
    // (defensive; rare protocol mismatch). Clear pending to avoid sticky skip.
    parsed.pending_stream_echo.insert(key.to_owned(), false);
    false
}

fn emit_text(parsed: &mut ParsedTurnStream, key: &str, text: String) {
    if text.is_empty() {
        return;
    }
    if key.is_empty() {
        parsed.main_text_emitted = true;
    }
    parsed.events.push(ExternalRuntimeTurnEvent::TextDelta {
        text: bound_text(&text, MAX_TEXT_EVENT_CHARS),
    });
}

fn ingest_assistant(parsed: &mut ParsedTurnStream, value: &Value) -> Result<(), String> {
    let parent = parent_from_value(value);
    let key = segment_key(parent);

    let content = value
        .pointer("/message/content")
        .or_else(|| value.get("content"));

    if let Some(Value::Array(blocks)) = content {
        // Collect full text first for echo matching (may be multi-block).
        let mut full_text_parts: Vec<String> = Vec::new();
        for block in blocks {
            if block.get("type").and_then(|v| v.as_str()) == Some("text") {
                if let Some(t) = block.get("text").and_then(|v| v.as_str()) {
                    full_text_parts.push(t.to_owned());
                }
            }
        }
        let full_joined = full_text_parts.join("");
        let skip_text = if full_joined.is_empty() {
            false
        } else {
            should_skip_full_text_echo(parsed, &key, &full_joined)
        };

        for block in blocks {
            let bty = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
            match bty {
                "text" => {
                    if skip_text {
                        continue;
                    }
                    if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                        let text = annotate_subagent(text, parent);
                        emit_text(parsed, &key, text);
                    }
                }
                "thinking" => {
                    if let Some(text) = block
                        .get("thinking")
                        .or_else(|| block.get("text"))
                        .and_then(|v| v.as_str())
                    {
                        let text = annotate_subagent(text, parent);
                        parsed.events.push(ExternalRuntimeTurnEvent::Status {
                            message: bound_text(&format!("thinking: {text}"), 2048),
                        });
                    }
                }
                "tool_use" => {
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
        if !should_skip_full_text_echo(parsed, &key, text) {
            let text = annotate_subagent(text, parent);
            emit_text(parsed, &key, text);
        }
    }
    Ok(())
}

fn ingest_user(parsed: &mut ParsedTurnStream, value: &Value) -> Result<(), String> {
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
    let parent = parent_from_value(value);
    let key = segment_key(parent);
    let event = value.get("event").unwrap_or(value);
    let delta = event.get("delta");
    if let Some(delta) = delta {
        let dty = delta.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if dty == "text_delta" || delta.get("text").is_some() {
            if let Some(text) = delta.get("text").and_then(|v| v.as_str()) {
                if !text.is_empty() {
                    let entry = parsed.streamed_text.entry(key.clone()).or_default();
                    entry.push_str(text);
                    parsed.pending_stream_echo.insert(key.clone(), true);
                    let display = if key.is_empty() {
                        text.to_owned()
                    } else {
                        // Subagent partials: prefix only on first chunk of segment.
                        // Keep raw delta for ordering; parent shown via later full
                        // if not streamed — for stream, annotate first delta only
                        // when segment just started.
                        text.to_owned()
                    };
                    if key.is_empty() {
                        parsed.main_text_emitted = true;
                    }
                    // For subagent, prefix the first character of a new pending segment.
                    let emit = if !key.is_empty() && entry.len() == text.len() {
                        annotate_subagent(&display, parent)
                    } else if !key.is_empty() {
                        display
                    } else {
                        display
                    };
                    parsed.events.push(ExternalRuntimeTurnEvent::TextDelta {
                        text: bound_text(&emit, MAX_TEXT_EVENT_CHARS),
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
        // Result text is the main-conversation summary — emit only when no
        // main-segment text was already delivered via stream or assistant.
        if !parsed.main_text_emitted && !text.is_empty() {
            emit_text(parsed, "", text.to_owned());
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

    fn text_deltas(p: &ParsedTurnStream) -> Vec<&str> {
        p.events
            .iter()
            .filter_map(|e| match e {
                ExternalRuntimeTurnEvent::TextDelta { text } => Some(text.as_str()),
                _ => None,
            })
            .collect()
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
        assert_eq!(text_deltas(&p), vec!["Hello"]); // no result re-echo
        assert_eq!(p.usage.as_ref().unwrap().input_tokens, Some(10));
    }

    #[test]
    fn segment_aware_partial_echo_tool_later_and_subagent() {
        let ls = lines(&[
            // Main stream partials
            r#"{"type":"stream_event","event":{"delta":{"type":"text_delta","text":"Hel"}}}"#,
            r#"{"type":"stream_event","event":{"delta":{"type":"text_delta","text":"lo"}}}"#,
            // Full assistant echo of main + tool
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Hello"},{"type":"tool_use","id":"t1","name":"Read"}]}}"#,
            // Later post-tool assistant (no stream) — must keep
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"After tool"}]}}"#,
            // Subagent full text (no stream) — must keep
            r#"{"type":"assistant","parent_tool_use_id":"t1","message":{"content":[{"type":"text","text":"sub answer"}]}}"#,
            r#"{"type":"result","subtype":"success","result":"Hello After tool","session_id":"s1"}"#,
        ]);
        let p = parse_turn_lines(&ls).unwrap();
        let texts = text_deltas(&p);
        // Stream Hel+lo, skip Hello echo, keep After tool, keep subagent, skip result
        assert_eq!(
            texts,
            vec!["Hel", "lo", "After tool", "[subagent:t1] sub answer"]
        );
        assert!(p.events.iter().any(
            |e| matches!(e, ExternalRuntimeTurnEvent::ToolCall { name, .. } if name == "Read")
        ));
    }

    #[test]
    fn subagent_stream_then_echo_deduped_main_later_kept() {
        let ls = lines(&[
            r#"{"type":"stream_event","parent_tool_use_id":"tool_9","event":{"delta":{"type":"text_delta","text":"sub"}}}"#,
            r#"{"type":"assistant","parent_tool_use_id":"tool_9","message":{"content":[{"type":"text","text":"sub"}]}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"main later"}]}}"#,
            r#"{"type":"result","subtype":"success","result":"done"}"#,
        ]);
        let p = parse_turn_lines(&ls).unwrap();
        let texts = text_deltas(&p);
        assert_eq!(texts, vec!["[subagent:tool_9] sub", "main later"]);
    }

    #[test]
    fn no_partials_uses_assistant_once() {
        let ls = lines(&[
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Only full"}]}}"#,
            r#"{"type":"result","subtype":"success","result":"Only full"}"#,
        ]);
        let p = parse_turn_lines(&ls).unwrap();
        assert_eq!(text_deltas(&p), vec!["Only full"]);
    }

    #[test]
    fn result_only_stream_emits_result_text() {
        let ls = lines(&[r#"{"type":"result","subtype":"success","result":"solo"}"#]);
        let p = parse_turn_lines(&ls).unwrap();
        assert_eq!(text_deltas(&p), vec!["solo"]);
    }

    #[test]
    fn no_final_result_is_explicit_incomplete() {
        let ls =
            lines(&[r#"{"type":"assistant","message":{"content":[{"type":"text","text":"x"}]}}"#]);
        let err = parse_turn_lines(&ls).unwrap_err();
        assert!(matches!(err, ProtocolError::NoFinalResult));
        assert!(
            err.to_string().contains("truncated")
                || err.to_string().contains("incomplete")
                || err.to_string().contains("final result")
        );
    }

    #[test]
    fn oversized_line_errors() {
        let huge = "x".repeat(MAX_TEXT_EVENT_CHARS + 10);
        assert!(matches!(
            parse_turn_lines(&[huge]),
            Err(ProtocolError::Oversized { .. })
        ));
    }

    #[test]
    fn tool_use_is_display_only() {
        let ls = lines(&[
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t1","name":"Bash"}]}}"#,
            r#"{"type":"result","subtype":"success","result":"done","session_id":"s"}"#,
        ]);
        let p = parse_turn_lines(&ls).unwrap();
        assert!(p.events.iter().any(
            |e| matches!(e, ExternalRuntimeTurnEvent::ToolCall { name, .. } if name == "Bash")
        ));
    }
}
