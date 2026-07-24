//! ChatGPT Codex Responses wire shaping (OpenCode / Codex CLI parity).
//!
//! The ChatGPT subscription endpoint
//! (`https://chatgpt.com/backend-api/codex/responses`) accepts a restricted
//! Responses dialect. Standard OpenAI Platform fields such as
//! `max_output_tokens`, `temperature`, and `top_p` are rejected with HTTP 400
//! (`{"detail":"Unsupported parameter: …"}`).
//!
//! OpenCode keeps those knobs off the wire via `chat.params` (`maxOutputTokens
//! = undefined`) and GPT-5 temperature capability flags. Grok Build applies
//! the same constraints at the HTTP body boundary so OAuth turns match
//! OpenCode.

use serde_json::{Value, json};

/// True when `base_url` targets the ChatGPT Codex Responses host (with or
/// without a trailing path segment).
pub fn is_chatgpt_codex_base_url(base_url: &str) -> bool {
    base_url.contains("chatgpt.com/backend-api/codex")
}

/// Fields the Codex backend rejects when present on the request body.
///
/// Derived from live probes against `chatgpt.com/backend-api/codex/responses`
/// and from OpenCode's recorded OAuth fixture bodies (which omit them).
const CODEX_UNSUPPORTED_BODY_KEYS: &[&str] = &[
    "max_output_tokens",
    "temperature",
    "top_p",
    "stream_tool_calls",
    "presence_penalty",
    "frequency_penalty",
    "service_tier",
    "truncation",
    "user",
    "metadata",
    "background",
    "top_logprobs",
    "max_tool_calls",
    "safety_identifier",
];

/// Clear typed fields that must not reach Codex before JSON serialization.
pub fn clear_chatgpt_codex_create_response_fields(req: &mut crate::rs::CreateResponse) {
    req.max_output_tokens = None;
    req.temperature = None;
    req.top_p = None;
    req.service_tier = None;
    req.truncation = None;
    req.metadata = None;
    req.background = None;
    req.top_logprobs = None;
    req.max_tool_calls = None;
    req.safety_identifier = None;
    // Codex requires store=false (ZDR / subscription).
    req.store = Some(false);
}

/// Shape a serialized Responses body for the ChatGPT Codex endpoint.
///
/// - Forces `store: false`
/// - Removes unsupported parameters (defense in depth after typed clear)
/// - Forces function tools to `strict: false` (OpenCode Codex parity)
/// - Lifts `role: "system"` / `role: "developer"` input items into the top-level
///   `instructions` field (ChatGPT Codex rejects system messages in `input`
///   with `invalid_request_error: System messages are not allowed`; OpenCode
///   OAuth does the same lift via `options.instructions = system.join("\n")`)
/// - Does **not** force `stream` here; streaming callers set it separately
pub fn shape_chatgpt_codex_responses_body(body: &mut Value) {
    let Some(obj) = body.as_object_mut() else {
        return;
    };

    obj.insert("store".to_owned(), json!(false));

    for key in CODEX_UNSUPPORTED_BODY_KEYS {
        obj.remove(*key);
    }

    if let Some(tools) = obj.get_mut("tools").and_then(|t| t.as_array_mut()) {
        for tool in tools.iter_mut() {
            let Some(tool_obj) = tool.as_object_mut() else {
                continue;
            };
            if tool_obj.get("type").and_then(|t| t.as_str()) == Some("function") {
                tool_obj.insert("strict".to_owned(), json!(false));
            }
        }
    }

    lift_system_messages_to_instructions(obj);
    // OpenCode strips Responses item IDs when store != true so the
    // signed/stateless body does not re-reference server-stored items
    // that Codex cannot look up (avoids "encrypted content for item
    // rs_… could not be verified").
    strip_input_item_ids(obj);
    // Drop reasoning items Codex cannot verify: missing/empty ciphertext, or
    // foreign blobs (e.g. xAI / prior-provider history). OpenCode only
    // re-sends reasoning with a string encrypted_content from the same
    // Codex endpoint; replaying anything else hard-400s the turn.
    filter_codex_reasoning_items(obj);
}

/// Remove `id` from input items. With `store: false`, Codex cannot resolve
/// prior response item IDs; replaying them triggers encrypted-content verify
/// failures. Encrypted reasoning blobs stay; only the lookup key is dropped.
fn strip_input_item_ids(obj: &mut serde_json::Map<String, Value>) {
    let Some(input) = obj.get_mut("input").and_then(|v| v.as_array_mut()) else {
        return;
    };
    for item in input.iter_mut() {
        if let Some(item_obj) = item.as_object_mut() {
            item_obj.remove("id");
        }
    }
}

/// Keep only Codex-replayable reasoning items in `input`.
///
/// Codex `store: false` multi-turn needs `encrypted_content` produced by this
/// same endpoint. History imported from xAI (or partial/corrupt turns) carries
/// blobs Codex rejects with `invalid_encrypted_content`. Drop those items so
/// the turn still runs (assistant text remains).
fn filter_codex_reasoning_items(obj: &mut serde_json::Map<String, Value>) {
    let Some(input) = obj.get_mut("input").and_then(|v| v.as_array_mut()) else {
        return;
    };
    input.retain(|item| {
        if item.get("type").and_then(|t| t.as_str()) != Some("reasoning") {
            return true;
        }
        match item.get("encrypted_content").and_then(|v| v.as_str()) {
            // OpenAI/Codex encrypted reasoning payloads are URL-safe base64 and
            // commonly start with the Fernet-like `gAAAAA` prefix used across
            // OpenAI Responses `store:false` ciphertext.
            Some(enc) if enc.len() >= 16 && enc.starts_with("gAAAAA") => true,
            _ => false,
        }
    });
}

/// Move system/developer EasyMessages out of `input` into `instructions`.
///
/// ChatGPT Codex (`chatgpt.com/backend-api/codex/responses`) rejects input
/// items with `role: "system"` (and developer is treated the same way by
/// OpenCode's OAuth path). The system prompt must travel on the top-level
/// `instructions` field instead.
fn lift_system_messages_to_instructions(obj: &mut serde_json::Map<String, Value>) {
    let Some(input) = obj.get_mut("input").and_then(|v| v.as_array_mut()) else {
        return;
    };

    let mut lifted: Vec<String> = Vec::new();
    let mut kept: Vec<Value> = Vec::with_capacity(input.len());

    for item in input.drain(..) {
        if is_system_or_developer_message(&item) {
            if let Some(text) = extract_message_text(&item) {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    lifted.push(trimmed.to_owned());
                }
            }
            continue;
        }
        kept.push(item);
    }

    *input = kept;

    if lifted.is_empty() {
        return;
    }

    let joined = lifted.join("\n");
    match obj.get("instructions") {
        Some(Value::String(existing)) if !existing.trim().is_empty() => {
            let merged = format!("{}\n{}", existing.trim_end(), joined);
            obj.insert("instructions".to_owned(), json!(merged));
        }
        _ => {
            obj.insert("instructions".to_owned(), json!(joined));
        }
    }
}

fn is_system_or_developer_message(item: &Value) -> bool {
    matches!(
        item.get("role").and_then(|r| r.as_str()),
        Some("system" | "developer")
    )
}

/// Pull plain text out of an EasyMessage-shaped input item.
///
/// Supports:
/// - `"content": "…"`
/// - `"content": [{"type":"input_text"|"text"|"output_text", "text":"…"}, …]`
fn extract_message_text(item: &Value) -> Option<String> {
    let content = item.get("content")?;
    match content {
        Value::String(s) => Some(s.clone()),
        Value::Array(parts) => {
            let mut out = String::new();
            for part in parts {
                let text = part
                    .get("text")
                    .and_then(|t| t.as_str())
                    .or_else(|| part.as_str());
                if let Some(t) = text {
                    if !out.is_empty() {
                        out.push('\n');
                    }
                    out.push_str(t);
                }
            }
            if out.is_empty() { None } else { Some(out) }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detects_codex_base_url() {
        assert!(is_chatgpt_codex_base_url(
            "https://chatgpt.com/backend-api/codex"
        ));
        assert!(is_chatgpt_codex_base_url(
            "https://chatgpt.com/backend-api/codex/"
        ));
        assert!(!is_chatgpt_codex_base_url("https://api.openai.com/v1"));
        assert!(!is_chatgpt_codex_base_url("https://api.x.ai/v1"));
    }

    #[test]
    fn strips_unsupported_fields_and_forces_store_false() {
        let mut body = json!({
            "model": "gpt-5.6-sol",
            "input": [{"role": "user", "content": "hi"}],
            "stream": true,
            "store": true,
            "max_output_tokens": 128000,
            "temperature": 1.0,
            "top_p": 1.0,
            "stream_tool_calls": true,
            "reasoning": {"effort": "high"},
            "prompt_cache_key": "sess-1",
            "tools": [{
                "type": "function",
                "name": "list_dir",
                "parameters": {"type": "object"},
                "strict": true
            }]
        });
        shape_chatgpt_codex_responses_body(&mut body);

        let obj = body.as_object().unwrap();
        assert_eq!(obj.get("store"), Some(&json!(false)));
        assert_eq!(obj.get("stream"), Some(&json!(true)));
        assert_eq!(obj.get("reasoning"), Some(&json!({"effort": "high"})));
        assert_eq!(obj.get("prompt_cache_key"), Some(&json!("sess-1")));
        for key in [
            "max_output_tokens",
            "temperature",
            "top_p",
            "stream_tool_calls",
        ] {
            assert!(!obj.contains_key(key), "expected {key} stripped");
        }
        assert_eq!(
            obj["tools"][0]["strict"],
            json!(false),
            "function tools must be strict:false for Codex parity"
        );
    }

    #[test]
    fn lifts_system_messages_into_instructions() {
        let mut body = json!({
            "model": "gpt-5.6-sol",
            "input": [
                {"role": "system", "content": "You are a coding agent."},
                {"role": "user", "content": "teste"}
            ],
            "stream": true,
            "store": false
        });
        shape_chatgpt_codex_responses_body(&mut body);

        let obj = body.as_object().unwrap();
        assert_eq!(
            obj.get("instructions"),
            Some(&json!("You are a coding agent.")),
            "system text must move to instructions"
        );
        assert_eq!(
            obj.get("input"),
            Some(&json!([{"role": "user", "content": "teste"}])),
            "system items must leave input"
        );
    }

    #[test]
    fn lifts_multiple_system_and_developer_messages() {
        let mut body = json!({
            "model": "gpt-5.6-sol",
            "input": [
                {"type": "message", "role": "system", "content": "Base prompt."},
                {
                    "role": "developer",
                    "content": [
                        {"type": "input_text", "text": "Project rules."}
                    ]
                },
                {"role": "user", "content": "hi"},
                {"role": "assistant", "content": "hello"}
            ]
        });
        shape_chatgpt_codex_responses_body(&mut body);

        let obj = body.as_object().unwrap();
        assert_eq!(
            obj.get("instructions"),
            Some(&json!("Base prompt.\nProject rules.")),
        );
        assert_eq!(
            obj.get("input"),
            Some(&json!([
                {"role": "user", "content": "hi"},
                {"role": "assistant", "content": "hello"}
            ])),
        );
    }

    #[test]
    fn merges_lifted_system_with_existing_instructions() {
        let mut body = json!({
            "model": "gpt-5.6-sol",
            "instructions": "Existing preamble.",
            "input": [
                {"role": "system", "content": "Extra system."},
                {"role": "user", "content": "go"}
            ]
        });
        shape_chatgpt_codex_responses_body(&mut body);

        assert_eq!(
            body.get("instructions"),
            Some(&json!("Existing preamble.\nExtra system.")),
        );
        assert_eq!(
            body.get("input"),
            Some(&json!([{"role": "user", "content": "go"}])),
        );
    }

    #[test]
    fn leaves_input_untouched_when_no_system_messages() {
        let mut body = json!({
            "model": "gpt-5.6-sol",
            "input": [{"role": "user", "content": "only user"}]
        });
        shape_chatgpt_codex_responses_body(&mut body);

        assert!(body.get("instructions").is_none());
        assert_eq!(
            body.get("input"),
            Some(&json!([{"role": "user", "content": "only user"}])),
        );
    }

    #[test]
    fn strips_ids_from_input_items_including_reasoning() {
        let mut body = json!({
            "model": "gpt-5.6-sol",
            "input": [
                {
                    "type": "reasoning",
                    "id": "rs_abc123",
                    "encrypted_content": "gAAAAABqEhTUCQT4XELlBu6r5VHqqtu5Il5WdX4m1upE8li0mPmIwg",
                    "summary": []
                },
                {"role": "user", "content": "hi", "id": "msg_1"}
            ]
        });
        shape_chatgpt_codex_responses_body(&mut body);

        let input = body.get("input").and_then(|v| v.as_array()).unwrap();
        assert!(input[0].get("id").is_none(), "reasoning id must be stripped");
        assert!(
            input[0]
                .get("encrypted_content")
                .and_then(|v| v.as_str())
                .is_some_and(|s| s.starts_with("gAAAAA")),
        );
        assert!(input[1].get("id").is_none(), "message id must be stripped");
        assert_eq!(input[1].get("role"), Some(&json!("user")));
    }

    #[test]
    fn drops_foreign_or_empty_reasoning_encrypted_content() {
        let mut body = json!({
            "model": "gpt-5.6-sol",
            "input": [
                {"role": "user", "content": "u1"},
                {
                    "type": "reasoning",
                    "encrypted_content": "LM4x-from-xai-or-corrupt-Pc8P",
                    "summary": []
                },
                {"role": "assistant", "content": "a1"},
                {
                    "type": "reasoning",
                    "summary": [{"type": "summary_text", "text": "think"}]
                },
                {"role": "user", "content": "u2"}
            ]
        });
        shape_chatgpt_codex_responses_body(&mut body);

        assert_eq!(
            body.get("input"),
            Some(&json!([
                {"role": "user", "content": "u1"},
                {"role": "assistant", "content": "a1"},
                {"role": "user", "content": "u2"}
            ])),
            "non-Codex reasoning items must be dropped so the turn can proceed"
        );
    }
}
