//! Wire-codec integration tests (Phase 4c).
//!
//! Drives real `InferenceClient`s against a [`MockInferenceServer`] over SSE
//! and asserts the per-provider delta channels, usage, and echoed request
//! bodies (reasoning-echo / strip) are byte-exact. Six provider kinds are
//! covered, plus the `compatible + vLLM` echo/strip pair.

use std::sync::Arc;
use std::time::Duration;

use futures_util::Stream;
use futures_util::StreamExt;

use xai_grok_inference::config::ProviderIdentity;
use xai_grok_inference::provider::WireDialect;
use xai_grok_inference::{
    ApiBackend, InferenceChannel, InferenceClient, InferenceConfig, InferenceEvent, RequestId,
};
use xai_grok_inference_types::{
    AssistantItem, ContentPart, ConversationItem, ConversationRequest, UserItem,
    synthesized_reasoning_item,
};
use xai_grok_test_support::MockInferenceServer;

/// The fixed text streamed by the mock for every backend.
const TEXT: &str = "hello world";

fn config_for(
    server: &MockInferenceServer,
    identity: ProviderIdentity,
    backend: ApiBackend,
    dialect: Option<WireDialect>,
) -> InferenceConfig {
    InferenceConfig {
        api_key: Some("test-key".to_string()),
        base_url: server.url(),
        model: "test-model".to_string(),
        provider_identity: identity,
        api_backend: backend,
        wire_dialect: dialect,
        ..InferenceConfig::default()
    }
}

fn user_request(text: &str) -> ConversationRequest {
    ConversationRequest {
        items: vec![ConversationItem::User(UserItem {
            content: vec![ContentPart::Text {
                text: Arc::<str>::from(text),
            }],
            ..Default::default()
        })],
        ..Default::default()
    }
}

/// A conversation carrying a replayed reasoning sibling folded into the
/// following assistant message (`reasoning_content` on the wire).
fn conversation_with_reasoning(text: &str, reasoning: &str) -> ConversationRequest {
    ConversationRequest {
        items: vec![
            ConversationItem::User(UserItem {
                content: vec![ContentPart::Text {
                    text: Arc::<str>::from(text),
                }],
                ..Default::default()
            }),
            ConversationItem::Reasoning(synthesized_reasoning_item(reasoning.to_string())),
            ConversationItem::Assistant(AssistantItem {
                content: Arc::<str>::from("answer"),
                tool_calls: Vec::new(),
                model_id: Some("test-model".to_string()),
                model_fingerprint: None,
                reasoning_effort: None,
                reasoning_details: Vec::new(),
                provider_payload: None,
            }),
        ],
        ..Default::default()
    }
}

fn rid() -> RequestId {
    RequestId::from("wire-codec")
}

const TIMEOUT: Duration = Duration::from_secs(30);

async fn collect<S>(s: S) -> Vec<InferenceEvent>
where
    S: Stream<Item = InferenceEvent> + Send,
{
    let mut out = Vec::new();
    let mut s = Box::pin(s);
    while let Some(ev) = s.next().await {
        out.push(ev);
    }
    out
}

/// Drive a Chat Completions stream through the pump and return the events.
async fn drive_chat(client: &InferenceClient, req: ConversationRequest) -> Vec<InferenceEvent> {
    let (raw, meta) = client
        .conversation_stream(req)
        .await
        .expect("conversation_stream");
    collect(xai_grok_inference::stream_chat_completions(
        raw,
        meta,
        rid(),
        TIMEOUT,
        Some("test-model"),
        client.provider_adapter(),
    ))
    .await
}

/// Drive a Responses stream through the pump and return the events.
async fn drive_responses(client: &InferenceClient, req: ConversationRequest) -> Vec<InferenceEvent> {
    let (raw, meta, doom_loop) = client
        .conversation_stream_responses(req)
        .await
        .expect("conversation_stream_responses");
    collect(xai_grok_inference::stream_responses(
        raw,
        meta,
        rid(),
        TIMEOUT,
        doom_loop,
    ))
    .await
}

/// Drive a Messages stream through the pump and return the events.
async fn drive_messages(client: &InferenceClient, req: ConversationRequest) -> Vec<InferenceEvent> {
    let (raw, meta) = client
        .conversation_stream_messages(req)
        .await
        .expect("conversation_stream_messages");
    collect(xai_grok_inference::stream_messages(raw, meta, rid(), TIMEOUT)).await
}

/// Assert the chat stream carried exactly one terminal `Completed` with the
/// assembled text, and that `usage` carries the mock's cumulative tokens.
fn assert_chat_completed(events: &[InferenceEvent], expected_text: &str) {
    let completed = events
        .iter()
        .find_map(|e| match e {
            InferenceEvent::Completed { response, .. } => Some(response.as_ref()),
            _ => None,
        })
        .expect("terminal Completed");
    assert_eq!(completed.assistant_text(), expected_text);
    let usage = completed.usage.as_ref().expect("usage reported");
    assert_eq!(usage.prompt_tokens, 10);
    assert_eq!(usage.completion_tokens, 2, "two deltas for 'hello world'");
    assert_eq!(usage.total_tokens, 12);
}

/// Assert the chat stream emitted a Reasoning channel token (a delta whose
/// `reasoning_content` was routed through the reasoning channel).
fn assert_reasoning_channel_present(events: &[InferenceEvent]) {
    assert!(
        events
            .iter()
            .any(|e| matches!(e, InferenceEvent::ChannelToken { channel: InferenceChannel::Reasoning, .. })),
        "expected a reasoning channel token"
    );
}

// ---------------------------------------------------------------------------
// Six provider kinds — each drives the backend its policy prefers.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn xai_chat_streams_text_and_usage() {
    let server = MockInferenceServer::start().await.expect("start mock");
    server.set_response(TEXT);
    let client =
        InferenceClient::new(config_for(&server, ProviderIdentity::Xai, ApiBackend::ChatCompletions, None))
            .expect("client");
    let events = drive_chat(&client, user_request("hi")).await;
    assert_chat_completed(&events, TEXT);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn openai_responses_streams_text_and_usage() {
    let server = MockInferenceServer::start().await.expect("start mock");
    server.set_response(TEXT);
    let client =
        InferenceClient::new(config_for(&server, ProviderIdentity::OpenAi, ApiBackend::Responses, None))
            .expect("client");
    let events = drive_responses(&client, user_request("hi")).await;
    let completed = events
        .iter()
        .find_map(|e| match e {
            InferenceEvent::Completed { response, .. } => Some(response.as_ref()),
            _ => None,
        })
        .expect("terminal Completed");
    assert_eq!(completed.assistant_text(), TEXT);
    let usage = completed.usage.as_ref().expect("usage reported");
    assert_eq!(usage.prompt_tokens, 10);
    assert_eq!(usage.completion_tokens, 5);
    assert_eq!(usage.total_tokens, 15);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn compatible_standard_chat_streams_text_and_usage() {
    let server = MockInferenceServer::start().await.expect("start mock");
    server.set_response(TEXT);
    let client = InferenceClient::new(config_for(
        &server,
        ProviderIdentity::Custom,
        ApiBackend::ChatCompletions,
        Some(WireDialect::Standard),
    ))
    .expect("client");
    let events = drive_chat(&client, user_request("hi")).await;
    assert_chat_completed(&events, TEXT);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn anthropic_messages_streams_text_and_usage() {
    let server = MockInferenceServer::start().await.expect("start mock");
    server.set_response(TEXT);
    let client = InferenceClient::new(config_for(
        &server,
        ProviderIdentity::Anthropic,
        ApiBackend::Messages,
        None,
    ))
    .expect("client");
    let events = drive_messages(&client, user_request("hi")).await;
    let completed = events
        .iter()
        .find_map(|e| match e {
            InferenceEvent::Completed { response, .. } => Some(response.as_ref()),
            _ => None,
        })
        .expect("terminal Completed");
    assert_eq!(completed.assistant_text(), TEXT);
    let usage = completed.usage.as_ref().expect("usage reported");
    assert_eq!(usage.prompt_tokens, 10);
    assert_eq!(usage.completion_tokens, 5);
    assert_eq!(usage.total_tokens, 15);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn openrouter_chat_streams_text_and_usage() {
    let server = MockInferenceServer::start().await.expect("start mock");
    server.set_response(TEXT);
    let client = InferenceClient::new(config_for(
        &server,
        ProviderIdentity::OpenRouter,
        ApiBackend::ChatCompletions,
        None,
    ))
    .expect("client");
    let events = drive_chat(&client, user_request("hi")).await;
    assert_chat_completed(&events, TEXT);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn zai_chat_streams_text_and_usage() {
    let server = MockInferenceServer::start().await.expect("start mock");
    server.set_response(TEXT);
    let client =
        InferenceClient::new(config_for(&server, ProviderIdentity::Zai, ApiBackend::ChatCompletions, None))
            .expect("client");
    let events = drive_chat(&client, user_request("hi")).await;
    assert_chat_completed(&events, TEXT);
}

// ---------------------------------------------------------------------------
// Echo / strip pair for compatible (+ vLLM dialect) — context-budget
// protection. Standard dialect echoes replayed reasoning_content; vLLM
// (Strip) drops it from the wire request body.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn compatible_standard_echoes_reasoning_content_on_replay() {
    let server = MockInferenceServer::start().await.expect("start mock");
    server.set_response(TEXT);
    let client = InferenceClient::new(config_for(
        &server,
        ProviderIdentity::Custom,
        ApiBackend::ChatCompletions,
        Some(WireDialect::Standard),
    ))
    .expect("client");
    // A reasoning sibling followed by an assistant; the reasoning folds into
    // `reasoning_content` on the assistant wire message.
    drive_chat(&client, conversation_with_reasoning("hi", "internal step")).await;

    let body = chat_request_body(&server);
    let assistant = find_assistant_message(&body).expect("assistant message");
    assert_eq!(
        assistant.get("reasoning_content"),
        Some(&serde_json::json!("internal step")),
        "standard compatible dialect must echo reasoning_content verbatim"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn compatible_vllm_strips_reasoning_content_on_replay() {
    let server = MockInferenceServer::start().await.expect("start mock");
    server.set_response(TEXT);
    let client = InferenceClient::new(config_for(
        &server,
        ProviderIdentity::Custom,
        ApiBackend::ChatCompletions,
        Some(WireDialect::Vllm),
    ))
    .expect("client");
    drive_chat(&client, conversation_with_reasoning("hi", "internal step")).await;

    let body = chat_request_body(&server);
    let assistant = find_assistant_message(&body).expect("assistant message");
    assert!(
        assistant.get("reasoning_content").is_none(),
        "vLLM dialect must strip replayed reasoning_content: {assistant}"
    );
    assert!(
        assistant.get("reasoning_details").is_none(),
        "vLLM dialect must also strip OpenRouter-only reasoning_details: {assistant}"
    );
}

/// The vLLM dialect also surfaces reasoning on the reasoning channel when a
/// delta carries a `reasoning` field, via the adapter's `shape_delta` hoist.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn compatible_vllm_hoists_reasoning_into_reasoning_channel() {
    let server = MockInferenceServer::start().await.expect("start mock");
    // A vLLM-style delta with both `content` and `reasoning_content`: the
    // adapter hoists the prose into reasoning so it never becomes assistant
    // text.
    let sse = vec![
        xai_grok_test_support::SseEvent::data(
            serde_json::json!({
                "id": "chatcmpl-test",
                "object": "chat.completion.chunk",
                "created": 1234567890,
                "model": "test-model",
                "choices": [{
                    "index": 0,
                    "delta": { "role": "assistant", "content": " note", "reasoning_content": "step 1" },
                    "finish_reason": null
                }]
            })
            .to_string(),
        ),
        xai_grok_test_support::SseEvent::data(
            serde_json::json!({
                "id": "chatcmpl-test",
                "object": "chat.completion.chunk",
                "created": 1234567890,
                "model": "test-model",
                "choices": [{
                    "index": 0,
                    "delta": {},
                    "finish_reason": "stop"
                }],
                "usage": { "prompt_tokens": 10, "completion_tokens": 1, "total_tokens": 11 }
            })
            .to_string(),
        ),
        xai_grok_test_support::SseEvent::data("[DONE]"),
    ];
    server.enqueue_response(
        "/v1/chat/completions",
        xai_grok_test_support::ScriptedResponse::sse(sse),
    );

    let client = InferenceClient::new(config_for(
        &server,
        ProviderIdentity::Custom,
        ApiBackend::ChatCompletions,
        Some(WireDialect::Vllm),
    ))
    .expect("client");
    let events = drive_chat(&client, user_request("hi")).await;

    // The content " note" was hoisted into reasoning: the reasoning channel
    // carries "step 1 note" and no text channel token is emitted.
    assert_reasoning_channel_present(&events);
    let text_tokens: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            InferenceEvent::ChannelToken {
                channel: InferenceChannel::Text,
                text,
                ..
            } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        text_tokens.is_empty(),
        "hoisted prose must not surface as assistant text: {text_tokens:?}"
    );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Fetch the most recent Chat Completions request body logged by the mock.
fn chat_request_body(server: &MockInferenceServer) -> serde_json::Value {
    server
        .requests()
        .into_iter()
        .rev()
        .find(|e| e.path == "/v1/chat/completions")
        .and_then(|e| e.body)
        .expect("a chat/completions request body")
}

fn find_assistant_message(body: &serde_json::Value) -> Option<&serde_json::Value> {
    body.get("messages")
        .and_then(|m| m.as_array())
        .and_then(|msgs| {
            msgs.iter().find(|msg| {
                msg.get("role").and_then(serde_json::Value::as_str) == Some("assistant")
            })
        })
}
