//! Deterministic Z.ai wire fixtures (no network).

use super::{ZaiChatExtensions, ZaiThinking, apply_zai_extensions, extract_reasoning_content};
use serde_json::{Value, json};

/// Non-streaming text response fixture.
pub fn text_completion_fixture() -> Value {
    json!({
        "id": "chatcmpl-zai-1",
        "object": "chat.completion",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "hello from zai",
                "reasoning_content": "brief thought"
            },
            "finish_reason": "stop"
        }],
        "usage": {"prompt_tokens": 3, "completion_tokens": 4, "total_tokens": 7}
    })
}

/// Fragmented tool_stream SSE-like deltas (as JSON chunks).
pub fn tool_stream_fragments() -> Vec<Value> {
    vec![
        json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_z1","type":"function","function":{"name":"run_terminal_command","arguments":"{\""}}]}}]}),
        json!({"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"command\":\"echo hi\""}}]}}]}),
        json!({"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"}"}}]}}]}),
        json!({"choices":[{"finish_reason":"tool_calls"}]}),
    ]
}

/// Accumulate fragmented tool arguments by index (deterministic).
pub fn accumulate_tool_args(frames: &[Value]) -> String {
    let mut args = String::new();
    for frame in frames {
        if let Some(delta) = frame
            .pointer("/choices/0/delta/tool_calls/0/function/arguments")
            .and_then(|v| v.as_str())
        {
            args.push_str(delta);
        }
    }
    args
}

/// Parallel tool call fragments for two tools.
pub fn parallel_tool_fragments() -> Vec<Value> {
    vec![
        json!({"choices":[{"delta":{"tool_calls":[
            {"index":0,"id":"c1","type":"function","function":{"name":"a","arguments":"{}"}},
            {"index":1,"id":"c2","type":"function","function":{"name":"b","arguments":"{"}}
        ]}}]}),
        json!({"choices":[{"delta":{"tool_calls":[
            {"index":1,"function":{"arguments":"}"}}
        ]}}]}),
        json!({"choices":[{"finish_reason":"tool_calls"}]}),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_stream_args_reconstruct() {
        let frames = tool_stream_fragments();
        let args = accumulate_tool_args(&frames);
        assert_eq!(args, r#"{"command":"echo hi"}"#);
    }

    #[test]
    fn reasoning_content_extracted() {
        let delta = json!({"reasoning_content": "think", "content": "hi"});
        assert_eq!(
            extract_reasoning_content(&delta).as_deref(),
            Some("think")
        );
    }

    #[test]
    fn extensions_serialize_only_when_applied() {
        let mut body = json!({"model": "glm-4.5", "messages": []});
        apply_zai_extensions(
            &mut body,
            &ZaiChatExtensions {
                thinking: Some(ZaiThinking {
                    r#type: Some("enabled".into()),
                    clear_thinking: Some(false),
                }),
                tool_stream: Some(true),
                request_id: Some("req_1".into()),
                ..Default::default()
            },
        );
        assert_eq!(body["tool_stream"], true);
        assert_eq!(body["thinking"]["type"], "enabled");
        assert_eq!(body["thinking"]["clear_thinking"], false);
        assert_eq!(body["request_id"], "req_1");
    }

    #[test]
    fn text_fixture_has_usage_and_reasoning() {
        let f = text_completion_fixture();
        assert_eq!(f["choices"][0]["message"]["content"], "hello from zai");
        assert!(f["choices"][0]["message"]["reasoning_content"].is_string());
        assert_eq!(f["usage"]["total_tokens"], 7);
    }

    #[test]
    fn parallel_tools_have_distinct_ids() {
        let frames = parallel_tool_fragments();
        let first = &frames[0]["choices"][0]["delta"]["tool_calls"];
        assert_eq!(first[0]["id"], "c1");
        assert_eq!(first[1]["id"], "c2");
    }
}
