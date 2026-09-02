//! Pinned tools — user-favorited tools surfaced to the model.
//!
//! Users pin tools from the pager's Settings → Tools section. Each pin is a
//! fully-qualified client-facing tool name (`server__tool` for MCP tools, the
//! bare client name for built-ins, matching `ToolDefinition.function.name`).
//!
//! Persistence lives under `[hints] pinned_tools = [...]` in `config.toml`
//! (same namespace as the tersify hints; written by the pager through
//! `config_toml_edit`). The model-facing surface is a single stable
//! `<pinned_tools>` block in the system prompt so descriptions are never
//! re-sent per turn. Fail-open: malformed entries are skipped, missing
//! entries degrade to "no pins".

use xai_grok_tools::types::definition::ToolDefinition;

/// `config.toml` `[hints]` key holding the pinned-tool name array.
pub const PINNED_TOOLS_HINT_KEY: &str = "pinned_tools";
/// System-prompt block tag; idempotent upsert/remove key.
pub const PINNED_TOOLS_TAG: &str = "pinned_tools";

/// Maximum `description` characters kept per pinned-tool line before an
/// ellipsis tail; bounds one chatty MCP server's share of the block.
const MAX_DESCRIPTION_CHARS: usize = 400;

/// Read the pinned tool names from `[hints] pinned_tools` in the effective
/// merged config. Deduplicated, order-preserving, empty-string entries
/// dropped. Never fails — malformed shapes read as "no pins".
#[must_use]
pub fn pinned_tools_from_disk() -> Vec<String> {
    let config = crate::config::load_effective_config().ok();
    let Some(hints) = config.as_ref().and_then(|root| root.get("hints")) else {
        return Vec::new();
    };
    let Some(list) = hints.get(PINNED_TOOLS_HINT_KEY) else {
        return Vec::new();
    };
    let Some(items) = list.as_array() else {
        return Vec::new();
    };
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for item in items {
        let Some(name) = item.as_str() else {
            continue;
        };
        if name.is_empty() || !seen.insert(name.to_owned()) {
            continue;
        }
        out.push(name.to_owned());
    }
    out
}

/// Build the `<pinned_tools>` block body from resolved tool definitions.
///
/// `definitions` is the session's full catalog (`ToolBridge::tool_definitions`);
/// `pinned` holds client-facing names. Unresolvable names are skipped with a
/// `tracing::debug!` so a stale pin never wedges the prompt. The block states
/// the count once and lists `name — description` pairs; descriptions are
/// truncated so a chatty MCP server cannot blow up the system prompt.
#[must_use]
pub fn pinned_tools_block(definitions: &[ToolDefinition], pinned: &[String]) -> Option<String> {
    if pinned.is_empty() {
        return None;
    }
    let mut lines = Vec::with_capacity(pinned.len());
    for name in pinned {
        let Some(definition) = definitions.iter().find(|d| d.function.name == *name) else {
            tracing::debug!(tool = %name, "pinned tool not in toolset; skipping");
            continue;
        };
        let raw = definition
            .function
            .description
            .as_deref()
            .unwrap_or("(no description available)")
            .trim();
        let description: std::borrow::Cow<'_, str> = if raw.is_empty() {
            std::borrow::Cow::Borrowed("(no description available)")
        } else if raw.chars().count() > MAX_DESCRIPTION_CHARS {
            std::borrow::Cow::Owned(truncate_with_ellipsis(raw))
        } else {
            std::borrow::Cow::Borrowed(raw)
        };
        lines.push(format!("- {name} — {description}"));
    }
    if lines.is_empty() {
        return None;
    }
    let mut body = String::new();
    body.push_str("The user pinned these tools for quick access. When a task ");
    body.push_str("matches one of them, prefer using it:\n");
    body.push_str(&lines.join("\n"));
    Some(body)
}

/// Truncate `text` to `MAX_DESCRIPTION_CHARS` (including the `...` tail).
/// Caller checks the length first; this always cuts.
fn truncate_with_ellipsis(text: &str) -> String {
    let mut out: String = text.chars().take(MAX_DESCRIPTION_CHARS - 3).collect();
    out.push_str("...");
    out
}

/// Wrap a block body in its `<pinned_tools>` tags.
#[must_use]
pub fn render_pinned_tools_block(body: &str) -> String {
    format!("<{PINNED_TOOLS_TAG}>\n{body}\n</{PINNED_TOOLS_TAG}>")
}

/// Insert or replace the `<pinned_tools>` block in `prompt`.
///
/// Mirrors the `tersify_style` upsert semantics: replace an existing block
/// in place, append (separated by a blank line) otherwise. Returns the
/// prompt unchanged when `block` is `None` and no block exists.
#[must_use]
pub fn upsert_pinned_tools_block(prompt: &str, block: &str) -> String {
    let open = format!("<{PINNED_TOOLS_TAG}>");
    let close = format!("</{PINNED_TOOLS_TAG}>");
    match prompt.find(&open) {
        Some(start) => {
            let Some(end_rel) = prompt[start..].find(&close) else {
                return format!("{prompt}\n\n{block}");
            };
            let end = start + end_rel + close.len();
            let mut out = String::with_capacity(prompt.len() + block.len() + 2);
            out.push_str(prompt[..start].trim_end());
            out.push('\n');
            out.push_str(block);
            out.push('\n');
            out.push_str(prompt[end..].trim_start());
            out.trim().to_string()
        }
        None => format!("{prompt}\n\n{block}"),
    }
}

/// Remove the `<pinned_tools>` block from `prompt` entirely.
#[must_use]
pub fn remove_pinned_tools_block(prompt: &str) -> String {
    let open = format!("<{PINNED_TOOLS_TAG}>");
    let close = format!("</{PINNED_TOOLS_TAG}>");
    let Some(start) = prompt.find(&open) else {
        return prompt.to_string();
    };
    let Some(end_rel) = prompt[start..].find(&close) else {
        return prompt.to_string();
    };
    let end = start + end_rel + close.len();
    let mut out = String::with_capacity(prompt.len());
    out.push_str(prompt[..start].trim_end());
    out.push('\n');
    out.push_str(prompt[end..].trim_start());
    out.trim().to_string()
}

/// Convenience: rebuild `prompt` with the pinned-tools block for `pinned`
/// names resolved against `definitions`. A empty/fully-unresolvable pin list
/// removes any existing block.
#[must_use]
pub fn refresh_pinned_tools_block(
    prompt: &str,
    definitions: &[ToolDefinition],
    pinned: &[String],
) -> String {
    match pinned_tools_block(definitions, pinned) {
        Some(body) => upsert_pinned_tools_block(prompt, &render_pinned_tools_block(&body)),
        None => remove_pinned_tools_block(prompt),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xai_grok_tools::types::definition::FunctionTool;
    use xai_grok_tools::types::definition::ToolType;

    fn definition(name: &str, description: Option<&str>) -> ToolDefinition {
        ToolDefinition {
            kind: ToolType::Function,
            function: FunctionTool {
                name: name.to_owned(),
                description: description.map(str::to_owned),
                parameters: serde_json::json!({}),
            },
        }
    }

    #[test]
    fn empty_pins_yields_no_block() {
        assert!(pinned_tools_block(&[], &[]).is_none());
        let definitions = vec![definition("read_file", Some("Reads a file."))];
        assert!(pinned_tools_block(&definitions, &[]).is_none());
    }

    #[test]
    fn block_lists_name_and_description() {
        let definitions = vec![
            definition("read_file", Some("Reads a file from disk.")),
            definition("grep", Some("Searches file contents.")),
        ];
        let body = pinned_tools_block(&definitions, &["grep".to_owned()])
            .expect("block for resolvable pin");
        assert_eq!(body.lines().count(), 2);
        assert!(body.contains("- grep — Searches file contents."));
    }

    #[test]
    fn unresolvable_pins_are_skipped_and_empty_result_removes_block() {
        let empty: Vec<String> = vec!["ghost".to_owned()];
        assert!(pinned_tools_block(&[], &empty).is_none());
    }

    #[test]
    fn long_descriptions_are_truncated() {
        let long = "x".repeat(1000);
        let definitions = vec![definition("chatty", Some(&long))];
        let body = pinned_tools_block(&definitions, &["chatty".to_owned()]).expect("block");
        // Exactly one truncated description segment (with the `...` tail) —
        // the raw 1000-char description must not survive verbatim.
        assert!(
            !body.contains(&long),
            "full 1000-char description must be truncated"
        );
        assert!(body.contains("..."), "truncation tail must be present");
        assert!(
            body.chars().count() < 600,
            "truncated body must stay bounded, got {} chars",
            body.chars().count()
        );
    }

    #[test]
    fn missing_description_has_placeholder() {
        let definitions = vec![definition("bare", None)];
        let body = pinned_tools_block(&definitions, &["bare".to_owned()]).expect("block");
        assert!(body.contains("(no description available)"));
    }

    #[test]
    fn upsert_replaces_existing_block_in_place() {
        let base = "HEAD\n<pinned_tools>\nold\n</pinned_tools>\n\nTAIL";
        let rebuilt = upsert_pinned_tools_block(base, &render_pinned_tools_block("new body"));
        assert!(rebuilt.contains("new body"));
        assert!(!rebuilt.contains("old"));
        assert!(rebuilt.starts_with("HEAD"));
        assert!(rebuilt.ends_with("TAIL"));
        assert_eq!(rebuilt.matches("<pinned_tools>").count(), 1);
    }

    #[test]
    fn upsert_appends_when_absent() {
        let rebuilt = upsert_pinned_tools_block("HEAD", &render_pinned_tools_block("body"));
        assert_eq!(rebuilt.matches("<pinned_tools>").count(), 1);
        assert!(rebuilt.starts_with("HEAD\n\n<pinned_tools>"));
    }

    #[test]
    fn remove_strips_the_block() {
        let base = "HEAD\n<pinned_tools>\nold\n</pinned_tools>\n\nTAIL";
        let stripped = remove_pinned_tools_block(base);
        assert!(!stripped.contains("pinned_tools"));
        assert!(stripped.contains("HEAD"));
        assert!(stripped.contains("TAIL"));
    }

    #[test]
    fn refresh_round_trip() {
        let definitions = vec![definition("grep", Some("Searches."))];
        let pinned = ["grep".to_owned()];
        let with = refresh_pinned_tools_block("BASE", &definitions, &pinned);
        assert!(with.contains("<pinned_tools>"));
        let without = refresh_pinned_tools_block(&with, &definitions, &[]);
        assert!(!without.contains("<pinned_tools>"));
        assert!(without.contains("BASE"));
    }
}
