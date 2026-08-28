//! Recovery of leaked tool-integrated-reasoning (TIR) XML from prose.
//!
//! Some tool-integrated-reasoning models (GLM-family models served through
//! OpenRouter, for example) occasionally emit their tool calls as inline XML
//! inside the assistant `content` instead of structured `tool_calls` deltas.
//! This module recovers those calls when, and only when, the structured
//! channel produced nothing.
//!
//! Design constraints that keep the fallback safe:
//! - It fires only when there are zero native `tool_calls`, so well-behaved
//!   responses are never modified.
//! - It requires the distinctive `<function=` / `<arg_key>` markers before
//!   any parsing happens; plain prose is returned untouched.
//! - Anything ambiguous (unbalanced pairs, empty name, empty call list)
//!   falls back to today's behavior: the text stays as prose.

use xai_grok_inference_types::ToolCall;

/// Recover TIR-style tool calls from leaked inline XML in `content`.
///
/// Returns the prose with the XML markup removed plus the recovered calls.
/// When nothing can be recovered the original content is returned unchanged
/// with an empty call list.
pub fn recover_tool_calls(content: &str) -> (String, Vec<ToolCall>) {
    let trimmed = content.trim();
    if !trimmed.contains("<function=") || !trimmed.contains("<arg_key>") {
        return (content.to_string(), Vec::new());
    }

    let mut prose = String::new();
    let mut calls: Vec<ToolCall> = Vec::new();
    let mut rest = content;

    while let Some(start) = rest.find("<function=") {
        prose.push_str(&rest[..start]);

        let after_name = &rest[start + "<function=".len()..];
        let name_end = match after_name.find('>') {
            Some(idx) => idx,
            None => break,
        };
        let name = after_name[..name_end].trim().to_string();
        let mut body = &after_name[name_end + 1..];

        // The block stops at the next `<function=` marker when the model
        // omitted the optional close tag, so consecutive calls are both
        // recovered.
        let block = match body.find("<function=") {
            Some(next_start) => {
                let block = &body[..next_start];
                body = &body[next_start..];
                block
            }
            None => {
                let block = body;
                body = "";
                block
            }
        };

        let mut arguments = serde_json::Map::new();
        let mut cursor = block;
        while let Some(key_start) = cursor.find("<arg_key>") {
            let after_key = &cursor[key_start + "<arg_key>".len()..];
            let Some(key_end) = after_key.find("</arg_key>") else {
                break;
            };
            let key = after_key[..key_end].trim().to_string();
            let after_key_close = &after_key[key_end + "</arg_key>".len()..];

            let value = if let Some(val_start) = after_key_close.find("<arg_value>") {
                let after_val = &after_key_close[val_start + "<arg_value>".len()..];
                match after_val.find("</arg_value>") {
                    Some(val_end) => {
                        cursor = &after_val[val_end + "</arg_value>".len()..];
                        after_val[..val_end].to_string()
                    }
                    // Truncated value: keep what arrived. The JSON we emit
                    // below stays parseable, matching today's tolerance.
                    None => {
                        let v = after_val.to_string();
                        cursor = "";
                        v
                    }
                }
            } else {
                cursor = after_key_close;
                String::new()
            };

            if !key.is_empty() {
                arguments.insert(key, serde_json::Value::String(value));
            }
        }

        if !name.is_empty() && !arguments.is_empty() {
            calls.push(ToolCall {
                id: std::sync::Arc::<str>::from(format!("tir-{}", calls.len())),
                name,
                arguments: std::sync::Arc::<str>::from(
                    serde_json::Value::Object(arguments).to_string(),
                ),
            });
        }

        rest = body;
    }

    if calls.is_empty() {
        return (content.to_string(), Vec::new());
    }

    (prose, calls)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_prose_is_untouched() {
        let content = "Hello world, no markup here.";
        let (prose, calls) = recover_tool_calls(content);
        assert_eq!(prose, content);
        assert!(calls.is_empty());
    }

    #[test]
    fn leaked_xml_becomes_structured_calls() {
        let content = concat!(
            "Checking now.<function=bash>",
            "<arg_key>command</arg_key><arg_value>ls -la</arg_value>",
            "<arg_key>description</arg_key><arg_value>List files</arg_value>",
        );
        let (prose, calls) = recover_tool_calls(content);
        assert_eq!(prose, "Checking now.");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "bash");
        let parsed: serde_json::Value = serde_json::from_str(&calls[0].arguments).unwrap();
        assert_eq!(parsed["command"], "ls -la");
        assert_eq!(parsed["description"], "List files");
    }

    #[test]
    fn truncated_value_keeps_prose_and_partial_json() {
        let content = "<function=bash><arg_key>command</arg_key><arg_value>ls -la";
        let (_prose, calls) = recover_tool_calls(content);
        assert_eq!(calls.len(), 1);
        let parsed: serde_json::Value = serde_json::from_str(&calls[0].arguments).unwrap();
        assert_eq!(parsed["command"], "ls -la");
    }

    #[test]
    fn multiple_calls_are_all_recovered() {
        let content = concat!(
            "<function=a><arg_key>x</arg_key><arg_value>1</arg_value>",
            "<function=b><arg_key>y</arg_key><arg_value>2</arg_value>",
        );
        let (_prose, calls) = recover_tool_calls(content);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "a");
        assert_eq!(calls[1].name, "b");
    }
}
