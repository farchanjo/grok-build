//! Session identity on the wire, per provider identity.
//!
//! The session key must reach every provider that models a session, in the
//! carrier that provider actually understands, and must never reach the ones
//! that reject unknown parameters (OpenAI, Codex, Anthropic).
//!
//! These are wire-level assertions: a real `InferenceClient` talks to a mock
//! server and the captured request body/headers are inspected.

mod support;

use std::sync::Arc;

use xai_grok_inference::config::ProviderIdentity;
use xai_grok_inference::{ApiBackend, InferenceClient, InferenceConfig};
use xai_grok_inference_types::{ContentPart, ConversationItem, ConversationRequest, UserItem};
use xai_grok_test_support::MockInferenceServer;

const SESSION: &str = "01a0b75f-session";

fn config_for(
    server: &MockInferenceServer,
    identity: ProviderIdentity,
    backend: ApiBackend,
) -> InferenceConfig {
    InferenceConfig {
        api_key: Some("test-key".to_string()),
        base_url: server.url(),
        model: "test-model".to_string(),
        provider_identity: identity,
        api_backend: backend,
        session_id: Some(SESSION.to_string()),
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

/// Drive one request through the client for the configured backend and return
/// the captured request log entry for `path`.
fn body_at(server: &MockInferenceServer, path: &str) -> serde_json::Value {
    server
        .requests()
        .into_iter()
        .rev()
        .find(|e| e.path == path)
        .and_then(|e| e.body)
        .unwrap_or_else(|| panic!("a {path} request body"))
}

async fn drive(server: &MockInferenceServer, identity: ProviderIdentity, backend: ApiBackend) {
    server.set_response("hello");
    let client = InferenceClient::new(config_for(server, identity, backend)).expect("client");
    client
        .conversation_collect(user_request("hi"))
        .await
        .expect("collect");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn self_hosted_chat_carries_session_id_and_bootstrap_room() {
    let server = MockInferenceServer::start().await.expect("server");
    drive(
        &server,
        ProviderIdentity::Custom,
        ApiBackend::ChatCompletions,
    )
    .await;

    let body = body_at(&server, "/v1/chat/completions");
    assert_eq!(body["session_id"], serde_json::json!(SESSION));
    assert!(
        body["bootstrap_room"].is_u64(),
        "SGLang dispatches rank = bootstrap_room % dp_size: {body}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn self_hosted_responses_carries_session_id_and_bootstrap_room() {
    let server = MockInferenceServer::start().await.expect("server");
    drive(&server, ProviderIdentity::Custom, ApiBackend::Responses).await;

    let body = body_at(&server, "/v1/responses");
    assert_eq!(body["session_id"], serde_json::json!(SESSION));
    assert!(
        body["bootstrap_room"].is_u64(),
        "the typed Responses body has no session slot, so it is injected: {body}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn self_hosted_session_also_rides_the_x_session_id_header() {
    let server = MockInferenceServer::start().await.expect("server");
    drive(
        &server,
        ProviderIdentity::Custom,
        ApiBackend::ChatCompletions,
    )
    .await;

    let entry = server
        .requests()
        .into_iter()
        .rev()
        .find(|e| e.path == "/v1/chat/completions")
        .expect("chat request");
    assert_eq!(
        entry.header("x-session-id"),
        Some(SESSION),
        "vLLM reads X-Session-ID natively; a router can route on it without parsing the body"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn openrouter_responses_carries_native_session_id() {
    let server = MockInferenceServer::start().await.expect("server");
    drive(&server, ProviderIdentity::OpenRouter, ApiBackend::Responses).await;

    let body = body_at(&server, "/v1/responses");
    assert_eq!(
        body["session_id"],
        serde_json::json!(SESSION),
        "OpenRouter uses session_id as its sticky routing key"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn openrouter_chat_carries_session_id_and_prompt_cache_key() {
    let server = MockInferenceServer::start().await.expect("server");
    drive(
        &server,
        ProviderIdentity::OpenRouter,
        ApiBackend::ChatCompletions,
    )
    .await;

    let body = body_at(&server, "/v1/chat/completions");
    assert_eq!(body["session_id"], serde_json::json!(SESSION));
    assert_eq!(
        body["prompt_cache_key"],
        serde_json::json!(SESSION),
        "documented fallback sticky-routing key when session_id is absent"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn anthropic_messages_carries_metadata_user_id_and_no_session_id() {
    let server = MockInferenceServer::start().await.expect("server");
    drive(&server, ProviderIdentity::Anthropic, ApiBackend::Messages).await;

    let body = body_at(&server, "/v1/messages");
    assert_eq!(
        body["metadata"]["user_id"],
        serde_json::json!(SESSION),
        "metadata.user_id is Anthropic's only native session carrier"
    );
    assert!(
        body.get("session_id").is_none(),
        "Anthropic rejects unknown parameters: {body}"
    );
    assert!(
        body.get("bootstrap_room").is_none(),
        "Anthropic rejects unknown parameters: {body}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn openai_chat_keeps_prompt_cache_key_and_omits_session_fields() {
    let server = MockInferenceServer::start().await.expect("server");
    drive(
        &server,
        ProviderIdentity::OpenAi,
        ApiBackend::ChatCompletions,
    )
    .await;

    let body = body_at(&server, "/v1/chat/completions");
    assert_eq!(body["prompt_cache_key"], serde_json::json!(SESSION));
    assert!(body.get("session_id").is_none(), "{body}");
    assert!(body.get("bootstrap_room").is_none(), "{body}");

    // The X-Session-ID header is for the self-hosted family only.
    let entry = server
        .requests()
        .into_iter()
        .rev()
        .find(|e| e.path == "/v1/chat/completions")
        .expect("chat request");
    assert!(
        entry.header("x-session-id").is_none(),
        "OpenAI does not model it"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn openai_responses_keeps_prompt_cache_key_and_omits_session_fields() {
    let server = MockInferenceServer::start().await.expect("server");
    drive(&server, ProviderIdentity::OpenAi, ApiBackend::Responses).await;

    let body = body_at(&server, "/v1/responses");
    assert_eq!(
        body["prompt_cache_key"],
        serde_json::json!(SESSION),
        "prompt_cache_key is how OpenAI routes related requests to the same cache"
    );
    assert!(body.get("session_id").is_none(), "{body}");
    assert!(body.get("bootstrap_room").is_none(), "{body}");
}

/// The compaction path calls `chat_completion_stream` directly, bypassing the
/// `conversation_*` wrappers: the affinity must still be stamped.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn direct_chat_completion_stream_stamps_affinity() {
    let server = MockInferenceServer::start().await.expect("server");
    server.set_response("hello");
    let client = InferenceClient::new(config_for(
        &server,
        ProviderIdentity::Custom,
        ApiBackend::ChatCompletions,
    ))
    .expect("client");

    let mut request = xai_grok_inference_types::ChatCompletionRequest::new("test-model", vec![]);
    request.x_grok_session_id = Some(SESSION.to_string());
    let _ = client
        .chat_completion_stream(request)
        .await
        .expect("stream");

    let body = body_at(&server, "/v1/chat/completions");
    assert_eq!(body["session_id"], serde_json::json!(SESSION));
    assert!(body["bootstrap_room"].is_u64(), "{body}");
}

/// The rank pin is derived from the session key by a stable hash, so a resumed
/// session lands on the same rank after a client restart (and keeps its prefix
/// cache warm without the server-side session radix cache).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bootstrap_room_is_stable_across_client_restart() {
    let server = MockInferenceServer::start().await.expect("server");
    server.set_response("hello");
    let cfg = config_for(
        &server,
        ProviderIdentity::Custom,
        ApiBackend::ChatCompletions,
    );

    let mut rooms = Vec::new();
    for _ in 0..2 {
        let client = InferenceClient::new(cfg.clone()).expect("client");
        let mut request =
            xai_grok_inference_types::ChatCompletionRequest::new("test-model", vec![]);
        request.x_grok_session_id = Some(SESSION.to_string());
        let _ = client
            .chat_completion_stream(request)
            .await
            .expect("stream");
        let body = body_at(&server, "/v1/chat/completions");
        rooms.push(body["bootstrap_room"].as_u64().expect("room"));
    }
    assert_eq!(rooms[0], rooms[1], "the rank pin must survive a restart");
}

/// Explicit request value wins over the config default.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_request_session_id_wins_over_config_default() {
    let server = MockInferenceServer::start().await.expect("server");
    server.set_response("hello");
    let client = InferenceClient::new(config_for(
        &server,
        ProviderIdentity::Custom,
        ApiBackend::ChatCompletions,
    ))
    .expect("client");

    let mut request = xai_grok_inference_types::ChatCompletionRequest::new("test-model", vec![]);
    request.x_grok_session_id = Some("explicit-session".to_string());
    let _ = client
        .chat_completion_stream(request)
        .await
        .expect("stream");

    let body = body_at(&server, "/v1/chat/completions");
    assert_eq!(body["session_id"], serde_json::json!("explicit-session"));
}

/// A tiny stand-in for the SGLang root route, recording what it receives.
async fn spawn_close_session_server() -> (String, Arc<std::sync::Mutex<Vec<serde_json::Value>>>) {
    use axum::routing::post;

    let seen: Arc<std::sync::Mutex<Vec<serde_json::Value>>> =
        Arc::new(std::sync::Mutex::new(vec![]));
    let recorder = Arc::clone(&seen);
    let app = axum::Router::new().route(
        "/close_session",
        post(move |axum::Json(body): axum::Json<serde_json::Value>| {
            let recorder = Arc::clone(&recorder);
            async move {
                recorder.lock().unwrap().push(body);
                axum::http::StatusCode::OK
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (format!("http://{addr}"), seen)
}

/// SGLang serves the route at the app root, while `base_url` normally ends in
/// `/v1`: the root-stripped URL must be the one that is used.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn close_session_posts_to_the_root_route_with_the_session_id() {
    let (base, seen) = spawn_close_session_server().await;
    let cfg = InferenceConfig {
        api_key: Some("test-key".to_string()),
        base_url: format!("{base}/v1"),
        model: "test-model".to_string(),
        session_id: Some(SESSION.to_string()),
        ..InferenceConfig::default()
    };
    let client = InferenceClient::new(cfg).expect("client");

    client.close_session().await.expect("best-effort close");

    let bodies = seen.lock().unwrap().clone();
    assert_eq!(bodies.len(), 1, "exactly one attempt on the root route");
    assert_eq!(bodies[0]["session_id"], serde_json::json!(SESSION));
}

/// The endpoint is SGLang-only: a 404 elsewhere must stay silent and Ok.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn close_session_is_best_effort_when_the_endpoint_is_missing() {
    let server = MockInferenceServer::start().await.expect("server");
    let cfg = config_for(
        &server,
        ProviderIdentity::Custom,
        ApiBackend::ChatCompletions,
    );
    let client = InferenceClient::new(cfg).expect("client");

    client
        .close_session()
        .await
        .expect("a missing endpoint is not an error");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn close_session_is_a_noop_without_a_session_key() {
    let (base, seen) = spawn_close_session_server().await;
    let cfg = InferenceConfig {
        api_key: Some("test-key".to_string()),
        base_url: base,
        model: "test-model".to_string(),
        session_id: None,
        ..InferenceConfig::default()
    };
    let client = InferenceClient::new(cfg).expect("client");

    client.close_session().await.expect("no-op");
    assert!(
        seen.lock().unwrap().is_empty(),
        "no session key, nothing to close"
    );
}
