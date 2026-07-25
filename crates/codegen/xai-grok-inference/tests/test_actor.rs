//! Integration tests for the M4 actor + request_task layer.
//!
//! Tests are integration-style (in `tests/`) rather than unit tests
//! because they require a real `tokio::runtime` and a mock HTTP
//! server (axum) to talk to the `InferenceClient`. Happy-path SSE
//! payloads come from `xai_grok_test_support::sse`.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use axum::Router;
use axum::http::StatusCode;
use axum::response::sse::{Event, Sse};
use axum::routing::post;
use futures_util::stream::{self, StreamExt};
use indexmap::IndexMap;
use serde_json::json;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot};

use xai_grok_inference::{
    ApiBackend, InferenceActor, InferenceChannel, InferenceConfig, InferenceErrorKind,
    InferenceEvent, OpenRouterPlugin, OpenRouterProviderPreferences, RequestId, RetryPolicy,
};
use xai_grok_inference_types::{
    ConversationItem, ConversationRequest, ConversationToolChoice, DoomLoopRecoveryPolicy,
    ReasoningEffort, ToolSpec, UserItem,
};
use xai_grok_test_support::{SseEvent, sse};

// ---------------------------------------------------------------------------
// Mock server harness
// ---------------------------------------------------------------------------

struct MockServer {
    addr: SocketAddr,
    shutdown_tx: oneshot::Sender<()>,
}

impl MockServer {
    async fn spawn(app: Router) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.await;
                })
                .await;
        });
        // Give the server a moment to start.
        tokio::time::sleep(Duration::from_millis(20)).await;
        Self { addr, shutdown_tx }
    }

    fn base_url(&self) -> String {
        format!("http://{}/v1", self.addr)
    }

    fn shutdown(self) {
        let _ = self.shutdown_tx.send(());
    }
}

// ---------------------------------------------------------------------------
// Config + request helpers
// ---------------------------------------------------------------------------

fn test_config(base_url: String, model: &str) -> InferenceConfig {
    InferenceConfig {
        api_key: Some("test-key".into()),
        base_url,
        model: model.into(),
        max_completion_tokens: Some(1024),
        temperature: None,
        top_p: None,
        openrouter_fallback_models: Vec::new(),
        openrouter_provider_preferences: None,
        openrouter_plugins: Vec::new(),
        openrouter_pacing: false,
            zai_tool_stream: false,
            zai_thinking: None,
        api_backend: ApiBackend::ChatCompletions,
        include_message_model_id: true,
        auth_scheme: Default::default(),
        extra_headers: IndexMap::new(),
        context_window: 128_000,
        force_http1: false,
        // Keep retries minimal so tests don't take forever.
        max_retries: Some(2),
        stream_tool_calls: false,
        idle_timeout_secs: Some(30),
        reasoning_effort: None,
        origin_client: None,
        client_identifier: None,
        deployment_id: None,
        user_id: None,
        client_version: None,
        attribution_callback: None,
        bearer_resolver: None,
        supports_backend_search: false,
        compactions_remaining: None,
        compaction_at_tokens: None,
        doom_loop_recovery: None,
        header_injector: None,
        provider_identity: xai_grok_inference::config::ProviderIdentity::default(),
    }
}

fn user_request(text: &str) -> ConversationRequest {
    ConversationRequest {
        items: vec![ConversationItem::User(UserItem {
            content: vec![xai_grok_inference_types::ContentPart::Text {
                text: std::sync::Arc::<str>::from(text),
            }],
            synthetic_reason: None,
            ..Default::default()
        })],
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// SSE generators
// ---------------------------------------------------------------------------

/// Render test-helper [`SseEvent`]s (optional `event:` name + `data:`) as
/// axum SSE events for this file's router-based harness.
fn sse_events_to_axum(events: Vec<SseEvent>) -> Vec<Event> {
    events
        .into_iter()
        .map(|e| {
            let ev = Event::default().data(e.data);
            match e.event {
                Some(name) => ev.event(name),
                None => ev,
            }
        })
        .collect()
}

fn text_chunk_event(content: &str, finish: bool) -> Event {
    let chunk = json!({
        "id": "chatcmpl-test",
        "object": "chat.completion.chunk",
        "created": 0,
        "model": "test-model",
        "choices": [{
            "index": 0,
            "delta": { "role": "assistant", "content": content },
            "finish_reason": if finish { json!("stop") } else { json!(null) }
        }]
    });
    Event::default().data(chunk.to_string())
}

// ---------------------------------------------------------------------------
// Actor lifecycle
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawn_then_active_count_zero_then_cancel_unknown_is_noop() {
    let (event_tx, _event_rx) = mpsc::unbounded_channel();
    let cfg = test_config("http://127.0.0.1:0/v1".into(), "test-model");
    let handle = InferenceActor::spawn(cfg, RetryPolicy::default(), event_tx);
    assert_eq!(handle.active_count().await, 0);
    handle.cancel(RequestId::from("nonexistent"));
    // Re-querying should still be 0 (cancel of unknown id is no-op).
    assert_eq!(handle.active_count().await, 0);
}

// ---------------------------------------------------------------------------
// Submit + event flow
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn submit_emits_started_first_token_channel_completed() {
    let app = Router::new().route(
        "/v1/chat/completions",
        post(|| async {
            let events = sse::chat_completion_events("hello world", "test-model");
            Sse::new(stream::iter(
                events.into_iter().map(Ok::<_, std::convert::Infallible>),
            ))
        }),
    );
    let server = MockServer::spawn(app).await;
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let cfg = test_config(server.base_url(), "test-model");
    let handle = InferenceActor::spawn(cfg, RetryPolicy::default(), event_tx);

    let rid = RequestId::from("req-1");
    handle.submit(rid.clone(), user_request("hi"));

    let events = drain_until_terminal(&mut event_rx, Duration::from_secs(5)).await;
    server.shutdown();

    assert!(matches!(events[0], InferenceEvent::StreamStarted { .. }));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, InferenceEvent::FirstToken { .. }))
    );

    let texts: Vec<&str> = events
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
    assert_eq!(texts.join(""), "hello world");

    match events.last().unwrap() {
        InferenceEvent::Completed {
            request_id,
            response,
            ..
        } => {
            assert_eq!(request_id, &rid);
            if let Some(a) = response.assistant() {
                assert_eq!(a.content.as_ref(), "hello world");
            } else {
                panic!("expected Assistant message");
            }
        }
        other => panic!("expected Completed, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// submit_and_collect
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn submit_and_collect_returns_response() {
    let app = Router::new().route(
        "/v1/chat/completions",
        post(|| async {
            let events = sse::chat_completion_events("collected response", "test-model");
            Sse::new(stream::iter(
                events.into_iter().map(Ok::<_, std::convert::Infallible>),
            ))
        }),
    );
    let server = MockServer::spawn(app).await;
    let (event_tx, _event_rx) = mpsc::unbounded_channel();
    let cfg = test_config(server.base_url(), "test-model");
    let handle = InferenceActor::spawn(cfg, RetryPolicy::default(), event_tx);

    let rid = RequestId::from("req-collect");
    let result = handle
        .submit_and_collect(rid, user_request("hi"))
        .await
        .expect("collected ok");
    server.shutdown();

    let (response, _metrics) = result;
    let a = response.assistant().expect("assistant item present");
    assert_eq!(a.content.as_ref(), "collected response");
}

// ---------------------------------------------------------------------------
// Cancellation
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_in_flight_request_terminates_task() {
    // Server that yields one chunk then hangs.
    let app = Router::new().route(
        "/v1/chat/completions",
        post(|| async {
            let stream = stream::iter(vec![Ok::<_, std::convert::Infallible>(text_chunk_event(
                "starting", false,
            ))])
            .chain(stream::pending());
            Sse::new(stream)
        }),
    );
    let server = MockServer::spawn(app).await;
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let cfg = test_config(server.base_url(), "test-model");
    let handle = InferenceActor::spawn(cfg, RetryPolicy::default(), event_tx);

    let rid = RequestId::from("req-cancel");
    handle.submit(rid.clone(), user_request("hi"));

    // Wait for the first token to arrive so we know the request is in flight.
    let _ = await_event_matching(
        &mut event_rx,
        |e| matches!(e, InferenceEvent::FirstToken { .. }),
        Duration::from_secs(5),
    )
    .await
    .expect("first token");

    handle.cancel(rid.clone());

    // Expect a Failed event with the cancellation message.
    let failed = await_event_matching(
        &mut event_rx,
        |e| matches!(e, InferenceEvent::Failed { .. }),
        Duration::from_secs(5),
    )
    .await
    .expect("Failed event after cancel");

    if let InferenceEvent::Failed { error, .. } = failed {
        assert!(error.message.contains("cancelled"));
    }

    // Wait briefly for the task to clean up.
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(handle.active_count().await, 0);
    server.shutdown();
}

// ---------------------------------------------------------------------------
// Concurrent requests
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_concurrent_requests_complete_with_correct_request_ids() {
    let counter = Arc::new(AtomicU32::new(0));
    let counter_handler = Arc::clone(&counter);
    let app = Router::new().route(
        "/v1/chat/completions",
        post(move || {
            let counter = Arc::clone(&counter_handler);
            async move {
                let n = counter.fetch_add(1, Ordering::SeqCst);
                let events = sse::chat_completion_events(&format!("response-{n}"), "test-model");
                Sse::new(stream::iter(
                    events.into_iter().map(Ok::<_, std::convert::Infallible>),
                ))
            }
        }),
    );
    let server = MockServer::spawn(app).await;
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let cfg = test_config(server.base_url(), "test-model");
    let handle = InferenceActor::spawn(cfg, RetryPolicy::default(), event_tx);

    let rid_a = RequestId::from("req-a");
    let rid_b = RequestId::from("req-b");
    handle.submit(rid_a.clone(), user_request("a"));
    handle.submit(rid_b.clone(), user_request("b"));

    // Drain until we see Completed for both.
    let mut completed_a = false;
    let mut completed_b = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while !(completed_a && completed_b) {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            panic!(
                "timed out waiting for both requests to complete: a={completed_a}, b={completed_b}"
            );
        }
        let remaining = deadline - now;
        match tokio::time::timeout(remaining, event_rx.recv()).await {
            Ok(Some(InferenceEvent::Completed { request_id, .. })) if request_id == rid_a => {
                completed_a = true;
            }
            Ok(Some(InferenceEvent::Completed { request_id, .. })) if request_id == rid_b => {
                completed_b = true;
            }
            Ok(Some(_)) => {}
            Ok(None) => panic!("event channel closed"),
            Err(_) => panic!("timeout"),
        }
    }
    server.shutdown();
}

// ---------------------------------------------------------------------------
// Retry on transient transport error
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retries_on_500_then_succeeds() {
    let counter = Arc::new(AtomicU32::new(0));
    let counter_handler = Arc::clone(&counter);
    let app = Router::new().route(
        "/v1/chat/completions",
        post(move || {
            let counter = Arc::clone(&counter_handler);
            async move {
                let n = counter.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    // First attempt: server error.
                    Err::<Sse<_>, (StatusCode, String)>((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        json!({ "error": { "message": "transient" } }).to_string(),
                    ))
                } else {
                    // Subsequent attempts: success.
                    let events = sse::chat_completion_events("ok", "test-model");
                    Ok(Sse::new(stream::iter(
                        events.into_iter().map(Ok::<_, std::convert::Infallible>),
                    )))
                }
            }
        }),
    );
    let server = MockServer::spawn(app).await;
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    // Lots of retries available; backoff is jittered around 2s on first
    // retry, so this test takes a bit to run.
    let cfg = test_config(server.base_url(), "test-model");
    let handle = InferenceActor::spawn(cfg, RetryPolicy::default(), event_tx);

    let rid = RequestId::from("req-retry");
    handle.submit(rid.clone(), user_request("hi"));

    let events = drain_until_terminal(&mut event_rx, Duration::from_secs(15)).await;
    server.shutdown();

    let saw_retrying = events
        .iter()
        .any(|e| matches!(e, InferenceEvent::Retrying { .. }));
    assert!(saw_retrying, "expected at least one Retrying event");

    match events.last().unwrap() {
        InferenceEvent::Completed { response, .. } => {
            if let Some(a) = response.assistant() {
                assert_eq!(a.content.as_ref(), "ok");
            }
        }
        other => panic!("expected Completed after retry, got {other:?}"),
    }

    assert!(
        counter.load(Ordering::SeqCst) >= 2,
        "server hit at least twice"
    );
}

// ---------------------------------------------------------------------------
// Rate limit exhausts threshold
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rate_limit_exhausts_at_threshold_and_yields_failed() {
    let counter = Arc::new(AtomicU32::new(0));
    let counter_handler = Arc::clone(&counter);
    let app = Router::new().route(
        "/v1/chat/completions",
        post(move || {
            let counter = Arc::clone(&counter_handler);
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Err::<
                    Sse<
                        futures_util::stream::Iter<
                            std::vec::IntoIter<Result<Event, std::convert::Infallible>>,
                        >,
                    >,
                    (StatusCode, String),
                >((
                    StatusCode::TOO_MANY_REQUESTS,
                    json!({ "error": { "message": "slow down" } }).to_string(),
                ))
            }
        }),
    );
    let server = MockServer::spawn(app).await;
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let cfg = test_config(server.base_url(), "test-model");
    let handle = InferenceActor::spawn(cfg, RetryPolicy::default(), event_tx);

    let rid = RequestId::from("req-429");
    handle.submit(rid.clone(), user_request("hi"));

    let events = drain_until_terminal(&mut event_rx, Duration::from_secs(60)).await;
    server.shutdown();

    match events.last().unwrap() {
        InferenceEvent::Failed { error, .. } => {
            assert_eq!(error.kind, InferenceErrorKind::RateLimited);
            assert_eq!(error.status_code, Some(429));
        }
        other => panic!("expected Failed(RateLimited), got {other:?}"),
    }

    let hits = counter.load(Ordering::SeqCst);
    // RATE_LIMIT_RETRY_THRESHOLD = 2, so the actor stops after two
    // attempts (the first attempt + one retry that also 429s = 2
    // hits). Allow a small slack in case scheduling fires a third
    // attempt before the threshold check.
    assert!((1..=3).contains(&hits), "expected 1-3 hits, got {hits}");
}

// ---------------------------------------------------------------------------
// Auth error -> EmitToSession (immediate)
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auth_401_emits_failed_immediately_no_retry() {
    let counter = Arc::new(AtomicU32::new(0));
    let counter_handler = Arc::clone(&counter);
    let app = Router::new().route(
        "/v1/chat/completions",
        post(move || {
            let counter = Arc::clone(&counter_handler);
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Err::<
                    Sse<
                        futures_util::stream::Iter<
                            std::vec::IntoIter<Result<Event, std::convert::Infallible>>,
                        >,
                    >,
                    (StatusCode, String),
                >((StatusCode::UNAUTHORIZED, "unauthorized".to_string()))
            }
        }),
    );
    let server = MockServer::spawn(app).await;
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let cfg = test_config(server.base_url(), "test-model");
    let handle = InferenceActor::spawn(cfg, RetryPolicy::default(), event_tx);

    let rid = RequestId::from("req-auth");
    handle.submit(rid.clone(), user_request("hi"));

    let events = drain_until_terminal(&mut event_rx, Duration::from_secs(5)).await;
    server.shutdown();

    // Auth errors are session-owned -- `classify_error` returns
    // `EmitToSession` so the actor emits Failed immediately without
    // retrying.
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, InferenceEvent::Retrying { .. }))
    );
    match events.last().unwrap() {
        InferenceEvent::Failed { error, .. } => {
            assert_eq!(error.kind, InferenceErrorKind::Auth);
        }
        other => panic!("expected Failed(Auth), got {other:?}"),
    }
    assert_eq!(counter.load(Ordering::SeqCst), 1, "no retries on 401");
}

// ---------------------------------------------------------------------------
// Anthropic Messages API: refusal stop_reason + mid-stream parse failure
// ---------------------------------------------------------------------------

fn messages_config(base_url: String) -> InferenceConfig {
    let mut cfg = test_config(base_url, "messages-compatible-model");
    cfg.api_backend = ApiBackend::Messages;
    cfg
}

/// Regression for the refusal-stop_reason incident: a well-formed stream
/// terminated by `stop_reason: "refusal"` must produce a successful
/// completion from EXACTLY ONE request — no retry storm.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn messages_refusal_stream_completes_with_single_request() {
    let counter = Arc::new(AtomicU32::new(0));
    let counter_handler = Arc::clone(&counter);
    let app = Router::new().route(
        "/v1/messages",
        post(move || {
            let counter = Arc::clone(&counter_handler);
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                let events = sse::messages_api_events(
                    "I can't help with that.",
                    "messages-compatible-model",
                    "refusal",
                );
                Sse::new(stream::iter(
                    events.into_iter().map(Ok::<_, std::convert::Infallible>),
                ))
            }
        }),
    );
    let server = MockServer::spawn(app).await;
    let (event_tx, _event_rx) = mpsc::unbounded_channel();
    let handle = InferenceActor::spawn(
        messages_config(server.base_url()),
        RetryPolicy::default(),
        event_tx,
    );

    let result = handle
        .submit_and_collect(RequestId::from("req-refusal"), user_request("hi"))
        .await;
    server.shutdown();

    let (response, _metrics) = result.expect("refusal-terminated turn must complete");
    let a = response.assistant().expect("assistant item present");
    assert_eq!(a.content.as_ref(), "I can't help with that.");
    assert_eq!(
        counter.load(Ordering::SeqCst),
        1,
        "refusal must not trigger retries"
    );
}

/// Empty-bodied refusal: `message_start → message_delta(refusal) →
/// message_stop` with zero content blocks must complete from exactly one
/// request — the content-less response must not be classified as a retryable
/// EmptyResponse.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn messages_empty_refusal_completes_without_retry() {
    let counter = Arc::new(AtomicU32::new(0));
    let counter_handler = Arc::clone(&counter);
    let app = Router::new().route(
        "/v1/messages",
        post(move || {
            let counter = Arc::clone(&counter_handler);
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                let mut events =
                    sse::messages_api_events("", "messages-compatible-model", "refusal");
                // Drop the content block events; keep start/delta/stop only.
                events.drain(1..4);
                Sse::new(stream::iter(
                    events.into_iter().map(Ok::<_, std::convert::Infallible>),
                ))
            }
        }),
    );
    let server = MockServer::spawn(app).await;
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let handle = InferenceActor::spawn(
        messages_config(server.base_url()),
        RetryPolicy::default(),
        event_tx,
    );

    handle.submit(RequestId::from("req-empty-refusal"), user_request("hi"));
    let events = drain_until_terminal(&mut event_rx, Duration::from_secs(10)).await;
    server.shutdown();

    assert!(
        !events
            .iter()
            .any(|e| matches!(e, InferenceEvent::Retrying { .. })),
        "content-less refusal must not be retried"
    );
    match events.last().unwrap() {
        InferenceEvent::Completed { response, .. } => {
            assert_eq!(
                response.stop_reason,
                Some(xai_grok_inference_types::StopReason::ContentFilter)
            );
        }
        other => panic!("expected Completed, got {other:?}"),
    }
    assert_eq!(counter.load(Ordering::SeqCst), 1, "exactly one request");
}

/// A mid-stream event that fails serde (after a valid `message_start`) is a
/// deterministic response-parse failure: Fatal on the first attempt, surfaced
/// as a non-retryable Serialization error — never a retry storm.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn messages_unparseable_event_is_fatal_without_retry() {
    let counter = Arc::new(AtomicU32::new(0));
    let counter_handler = Arc::clone(&counter);
    let app =
        Router::new().route(
            "/v1/messages",
            post(move || {
                let counter = Arc::clone(&counter_handler);
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    let mut events =
                        sse::messages_api_events("hello", "messages-compatible-model", "end_turn");
                    // Replace the tail with a `message_delta` missing the
                    // required `delta` field — fails MessageStreamEvent serde.
                    events.truncate(4);
                    events.push(Event::default().data(
                        json!({"type":"message_delta","usage":{"output_tokens":1}}).to_string(),
                    ));
                    Sse::new(stream::iter(
                        events.into_iter().map(Ok::<_, std::convert::Infallible>),
                    ))
                }
            }),
        );
    let server = MockServer::spawn(app).await;
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let handle = InferenceActor::spawn(
        messages_config(server.base_url()),
        RetryPolicy::default(),
        event_tx,
    );

    handle.submit(RequestId::from("req-bad-event"), user_request("hi"));
    let events = drain_until_terminal(&mut event_rx, Duration::from_secs(10)).await;
    server.shutdown();

    assert!(
        !events
            .iter()
            .any(|e| matches!(e, InferenceEvent::Retrying { .. })),
        "serde failures must not be retried"
    );
    match events.last().unwrap() {
        InferenceEvent::Failed { error, .. } => {
            assert_eq!(error.kind, InferenceErrorKind::Serialization);
            assert!(!error.is_retryable, "surfaced info must be non-retryable");
        }
        other => panic!("expected Failed(Serialization), got {other:?}"),
    }
    assert_eq!(counter.load(Ordering::SeqCst), 1, "exactly one attempt");
}

// ---------------------------------------------------------------------------
// UpdateConfig invalidates cache + applies to subsequent requests
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn update_config_changes_subsequent_request_model() {
    use std::sync::Mutex;

    let captured_models: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let captured_handler = Arc::clone(&captured_models);
    let app = Router::new().route(
        "/v1/chat/completions",
        post(move |axum::Json(body): axum::Json<serde_json::Value>| {
            let captured = Arc::clone(&captured_handler);
            async move {
                let model = body
                    .get("model")
                    .and_then(|m| m.as_str())
                    .unwrap_or("")
                    .to_string();
                captured.lock().unwrap().push(model);
                let events = sse::chat_completion_events("ok", "test-model");
                Sse::new(stream::iter(
                    events.into_iter().map(Ok::<_, std::convert::Infallible>),
                ))
            }
        }),
    );
    let server = MockServer::spawn(app).await;
    let (event_tx, _event_rx) = mpsc::unbounded_channel();
    let cfg = test_config(server.base_url(), "model-A");
    let handle = InferenceActor::spawn(cfg, RetryPolicy::default(), event_tx);

    let _ = handle
        .submit_and_collect(RequestId::from("req-1"), user_request("hi"))
        .await
        .expect("first req ok");

    let mut new_cfg = test_config(server.base_url(), "model-B");
    new_cfg.api_key = Some("test-key".into());
    handle.update_config(new_cfg);

    let _ = handle
        .submit_and_collect(RequestId::from("req-2"), user_request("hi"))
        .await
        .expect("second req ok");

    server.shutdown();

    let models = captured_models.lock().unwrap();
    assert_eq!(
        models.as_slice(),
        &["model-A".to_string(), "model-B".to_string()]
    );
}

// ---------------------------------------------------------------------------
// Responses doom-loop check signals
// ---------------------------------------------------------------------------

fn responses_config(
    base_url: String,
    doom_loop: Option<DoomLoopRecoveryPolicy>,
) -> InferenceConfig {
    let mut cfg = test_config(base_url, "test-model");
    cfg.api_backend = ApiBackend::Responses;
    cfg.doom_loop_recovery = doom_loop;
    cfg
}

/// Server-reported doom-loop triggers flow through the actor rung onto the
/// completed response, without retries. The trigger is non-confident
/// (`@response` channel), so the recovery — which resamples only confident
/// signals — leaves it alone.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_doom_loop_signals_reach_completed_response() {
    let counter = Arc::new(AtomicU32::new(0));
    let counter_handler = Arc::clone(&counter);
    let app = Router::new().route(
        "/v1/responses",
        post(move || {
            let counter = Arc::clone(&counter_handler);
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                let events = sse_events_to_axum(sse::responses_api_doom_loop_terminal_only_events(
                    &["tail_repetition:4@response"],
                    "some thought",
                    "an answer",
                    "test-model",
                ));
                Sse::new(stream::iter(
                    events.into_iter().map(Ok::<_, std::convert::Infallible>),
                ))
            }
        }),
    );
    let server = MockServer::spawn(app).await;
    let (event_tx, _event_rx) = mpsc::unbounded_channel();
    let handle = InferenceActor::spawn(
        responses_config(server.base_url(), Some(DoomLoopRecoveryPolicy::default())),
        RetryPolicy::default(),
        event_tx,
    );

    let result = handle
        .submit_and_collect(RequestId::from("req-doom-signal"), user_request("hi"))
        .await;
    server.shutdown();

    let (response, _metrics) = result.expect("a signalled turn still completes");
    assert_eq!(counter.load(Ordering::SeqCst), 1, "warn-only: no resample");
    assert_eq!(response.doom_loop_signals.len(), 1);
    assert_eq!(
        response.doom_loop_signals[0].raw,
        "tail_repetition:4@response"
    );
    assert_eq!(response.assistant_text(), "an answer");
}

/// Acceptance spec for the recovery rung: a confident signal
/// (`tail_repetition:8@thinking` at the default threshold) is resampled once
/// and the clean second response is accepted, on its own budget.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_confident_doom_loop_signal_resamples_once() {
    let counter = Arc::new(AtomicU32::new(0));
    let counter_handler = Arc::clone(&counter);
    let app = Router::new().route(
        "/v1/responses",
        post(move || {
            let counter = Arc::clone(&counter_handler);
            async move {
                let attempt = counter.fetch_add(1, Ordering::SeqCst);
                let events = if attempt == 0 {
                    sse::responses_api_doom_loop_terminal_only_events(
                        &["tail_repetition:8@thinking"],
                        "loop loop loop",
                        "poisoned answer",
                        "test-model",
                    )
                } else {
                    sse::responses_api_reasoning_and_text_events(
                        "fresh thought",
                        "clean answer",
                        "test-model",
                    )
                };
                let events = sse_events_to_axum(events);
                Sse::new(stream::iter(
                    events.into_iter().map(Ok::<_, std::convert::Infallible>),
                ))
            }
        }),
    );
    let server = MockServer::spawn(app).await;
    let (event_tx, _event_rx) = mpsc::unbounded_channel();
    let handle = InferenceActor::spawn(
        responses_config(server.base_url(), Some(DoomLoopRecoveryPolicy::default())),
        RetryPolicy::default(),
        event_tx,
    );

    let result = handle
        .submit_and_collect(RequestId::from("req-doom-resample"), user_request("hi"))
        .await;
    server.shutdown();

    let (response, _metrics) = result.expect("recovery accepts the clean resample");
    assert_eq!(counter.load(Ordering::SeqCst), 2, "exactly one resample");
    assert_eq!(response.assistant_text(), "clean answer");
    assert!(
        response.doom_loop_signals.is_empty(),
        "the accepted response is the clean resample"
    );
}

// ---------------------------------------------------------------------------
// Responses transport keepalive (OpenAI OAuth / Codex)
// ---------------------------------------------------------------------------

/// Keepalive frames (`event: keepalive` and JSON `type: "keepalive"`) must not
/// abort the stream with a serialization error. The turn continues through
/// subsequent text deltas and `response.completed`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_keepalive_frames_do_not_abort_stream() {
    let app = Router::new().route(
        "/v1/responses",
        post(|| async move {
            let events = sse_events_to_axum(sse::responses_api_with_keepalive_frames(
                "hello after keepalive",
                "test-model",
            ));
            Sse::new(stream::iter(
                events.into_iter().map(Ok::<_, std::convert::Infallible>),
            ))
        }),
    );
    let server = MockServer::spawn(app).await;
    let (event_tx, _event_rx) = mpsc::unbounded_channel();
    let handle = InferenceActor::spawn(
        responses_config(server.base_url(), None),
        RetryPolicy::default(),
        event_tx,
    );

    let result = handle
        .submit_and_collect(RequestId::from("req-keepalive"), user_request("hi"))
        .await;
    server.shutdown();

    let (response, _metrics) = result.expect("keepalive must not surface as serialization error");
    assert!(
        response.assistant_text().contains("hello after keepalive"),
        "expected streamed text after keepalive frames, got {:?}",
        response.assistant_text()
    );
}

// ---------------------------------------------------------------------------
// Responses Codex metadata control frames (OpenAI OAuth / Codex)
// ---------------------------------------------------------------------------

/// Codex `response.metadata` frames (named SSE event and data-only JSON with
/// exact `type: "response.metadata"`, including realistic headers/metadata)
/// must not abort the stream. The turn continues through text and completed;
/// metadata is not treated as model output.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_metadata_frames_do_not_abort_stream() {
    let app = Router::new().route(
        "/v1/responses",
        post(|| async move {
            let events = sse_events_to_axum(sse::responses_api_with_metadata_frames(
                "hello after metadata",
                "test-model",
            ));
            Sse::new(stream::iter(
                events.into_iter().map(Ok::<_, std::convert::Infallible>),
            ))
        }),
    );
    let server = MockServer::spawn(app).await;
    let (event_tx, _event_rx) = mpsc::unbounded_channel();
    let handle = InferenceActor::spawn(
        responses_config(server.base_url(), None),
        RetryPolicy::default(),
        event_tx,
    );

    let result = handle
        .submit_and_collect(RequestId::from("req-metadata"), user_request("hi"))
        .await;
    server.shutdown();

    let (response, _metrics) = result.expect("metadata must not surface as serialization error");
    let text = response.assistant_text();
    assert!(
        text.contains("hello after metadata"),
        "expected streamed text after metadata frames, got {text:?}"
    );
    // Control-frame payload must not leak into assistant content.
    assert!(
        !text.contains("x-codex-turn-state")
            && !text.contains("turn_state_fixture")
            && !text.contains("response.metadata"),
        "metadata/headers must not become model output, got {text:?}"
    );
}

/// Keepalive and `response.metadata` can interleave on the same OAuth stream;
/// both control filters must absorb their frames without aborting the turn.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_keepalive_and_metadata_frames_do_not_abort_stream() {
    let app = Router::new().route(
        "/v1/responses",
        post(|| async move {
            let events = sse_events_to_axum(sse::responses_api_with_keepalive_and_metadata_frames(
                "hello after control frames",
                "test-model",
            ));
            Sse::new(stream::iter(
                events.into_iter().map(Ok::<_, std::convert::Infallible>),
            ))
        }),
    );
    let server = MockServer::spawn(app).await;
    let (event_tx, _event_rx) = mpsc::unbounded_channel();
    let handle = InferenceActor::spawn(
        responses_config(server.base_url(), None),
        RetryPolicy::default(),
        event_tx,
    );

    let result = handle
        .submit_and_collect(
            RequestId::from("req-keepalive-metadata"),
            user_request("hi"),
        )
        .await;
    server.shutdown();

    let (response, _metrics) = result.expect("control frames must not abort the stream");
    assert!(
        response
            .assistant_text()
            .contains("hello after control frames"),
        "expected streamed text after keepalive+metadata, got {:?}",
        response.assistant_text()
    );
}

/// An unknown semantic `response.*` event still fails closed (not silently
/// swallowed like transport keepalives or Codex `response.metadata`).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_unknown_semantic_event_fails_closed() {
    let app = Router::new().route(
        "/v1/responses",
        post(|| async move {
            let mut events = sse::responses_api_script_exact("should not complete", "test-model");
            // Inject an unknown semantic event after response.created.
            events.insert(
                1,
                SseEvent::data(
                    r#"{"type":"response.brand_new_semantic_event","sequence_number":1}"#,
                ),
            );
            let events = sse_events_to_axum(events);
            Sse::new(stream::iter(
                events.into_iter().map(Ok::<_, std::convert::Infallible>),
            ))
        }),
    );
    let server = MockServer::spawn(app).await;
    let (event_tx, _event_rx) = mpsc::unbounded_channel();
    let mut cfg = responses_config(server.base_url(), None);
    // Serialization is non-retryable; keep retries low so the test is fast.
    cfg.max_retries = Some(0);
    let handle = InferenceActor::spawn(
        cfg,
        RetryPolicy {
            max_retries: 0,
            rate_limit_retry_threshold: 0,
            retry_only_before_output: false,
        },
        event_tx,
    );

    let result = handle
        .submit_and_collect(RequestId::from("req-unknown-semantic"), user_request("hi"))
        .await;
    server.shutdown();

    let err = result.expect_err("unknown semantic response.* must fail the request");
    assert!(
        matches!(
            err,
            xai_grok_inference_types::InferenceError::Serialization(_)
        ),
        "expected InferenceError::Serialization, got: {err:?}"
    );
    let msg = err.to_string();
    assert!(
        msg.contains("serialization error:"),
        "expected serialization error display prefix, got: {msg}"
    );
    assert!(
        msg.contains("response.brand_new_semantic_event") || msg.contains("unknown variant"),
        "expected unknown-event detail, got: {msg}"
    );
}

// ---------------------------------------------------------------------------
// OpenRouter production tool-loop / cancel / backend-switch (actor path)
// ---------------------------------------------------------------------------
//
// These drive `InferenceActor` + `InferenceClient` against a local Axum mock
// (no network). They are not conversion-only unit tests: each turn goes
// through submit_and_collect / submit+events so stream decode, OpenRouter
// request extensions, and completion collection run for real.

const OR_MODEL: &str = "openai/gpt-oss-120b";
const OR_FALLBACK: &str = "openai/gpt-5-mini";
const TOOL_NAME: &str = "run_terminal_command";
const TOOL_CALL_ID: &str = "call_or_1";
const TOOL_ARGS: &str = r#"{"command":"echo hi"}"#;
const TOOL_RESULT: &str = "hi\n";

fn openrouter_config(base_url: String, backend: ApiBackend) -> InferenceConfig {
    let mut cfg = test_config(base_url, OR_MODEL);
    cfg.api_backend = backend;
    cfg.provider_identity = xai_grok_inference::config::ProviderIdentity::OpenRouter;
    cfg.openrouter_fallback_models = vec![OR_FALLBACK.into()];
    cfg.openrouter_provider_preferences = Some(OpenRouterProviderPreferences {
        require_parameters: Some(true),
        data_collection: Some("deny".into()),
        ..Default::default()
    });
    cfg.openrouter_plugins = vec![OpenRouterPlugin {
        id: "response-healing".into(),
        ..Default::default()
    }];
    cfg.reasoning_effort = Some(ReasoningEffort::Medium);
    // OpenRouter Chat rejects message model_id metadata.
    cfg.include_message_model_id = false;
    cfg
}

fn tool_round_request(items: Vec<ConversationItem>) -> ConversationRequest {
    ConversationRequest {
        model: Some(OR_MODEL.into()),
        items,
        tools: vec![ToolSpec {
            name: TOOL_NAME.into(),
            description: Some("run a shell command".into()),
            parameters: json!({
                "type": "object",
                "properties": { "command": { "type": "string" } }
            }),
        }],
        tool_choice: Some(ConversationToolChoice::Auto),
        reasoning_effort: Some(ReasoningEffort::Medium),
        ..ConversationRequest::default()
    }
}

fn assert_openrouter_chat_wire(body: &serde_json::Value) {
    assert_eq!(body["model"], OR_MODEL);
    assert_eq!(body["models"], json!([OR_FALLBACK]));
    assert_eq!(body["provider"]["require_parameters"], true);
    assert_eq!(body["provider"]["data_collection"], "deny");
    assert_eq!(body["plugins"][0]["id"], "response-healing");
    assert_eq!(body["reasoning"]["effort"], "medium");
    assert_eq!(body["reasoning_effort"], "medium");
    assert!(
        body["tools"].as_array().is_some_and(|t| !t.is_empty()),
        "tools must be present on the wire: {body}"
    );
}

fn assert_openrouter_responses_wire(body: &serde_json::Value) {
    assert_eq!(body["model"], OR_MODEL);
    assert_eq!(body["models"], json!([OR_FALLBACK]));
    assert_eq!(body["provider"]["require_parameters"], true);
    assert_eq!(body["provider"]["data_collection"], "deny");
    assert_eq!(body["plugins"][0]["id"], "response-healing");
    assert_eq!(body["store"], false);
    assert!(
        body.get("previous_response_id").is_none(),
        "stateless OpenRouter Responses must omit previous_response_id: {body}"
    );
    assert!(
        body["tools"].as_array().is_some_and(|t| !t.is_empty()),
        "tools must be present on the wire: {body}"
    );
}

/// Full Chat Completions tool round through the actor: streamed tool call,
/// local tool result continuation, final answer — with OpenRouter extensions
/// on both request bodies.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn openrouter_chat_tool_round_trip_through_actor() {
    use std::sync::Mutex;

    let captured: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let captured_h = Arc::clone(&captured);
    let turn = Arc::new(AtomicU32::new(0));
    let turn_h = Arc::clone(&turn);

    let app = Router::new().route(
        "/v1/chat/completions",
        post(move |axum::Json(body): axum::Json<serde_json::Value>| {
            let captured = Arc::clone(&captured_h);
            let turn = Arc::clone(&turn_h);
            async move {
                captured.lock().unwrap().push(body);
                let n = turn.fetch_add(1, Ordering::SeqCst);
                let events = if n == 0 {
                    sse_events_to_axum(sse::chat_completions_reasoning_then_tool_call_events(
                        "plan the call",
                        TOOL_CALL_ID,
                        TOOL_NAME,
                        TOOL_ARGS,
                        OR_MODEL,
                    ))
                } else {
                    // Final assistant answer after tool result (axum Events).
                    sse::chat_completion_events("tool said hi", OR_MODEL)
                };
                Sse::new(stream::iter(
                    events.into_iter().map(Ok::<_, std::convert::Infallible>),
                ))
            }
        }),
    );
    let server = MockServer::spawn(app).await;
    let (event_tx, _event_rx) = mpsc::unbounded_channel();
    let handle = InferenceActor::spawn(
        openrouter_config(server.base_url(), ApiBackend::ChatCompletions),
        RetryPolicy::default(),
        event_tx,
    );

    // Turn 1: user asks for a tool.
    let (resp1, _m1) = handle
        .submit_and_collect(
            RequestId::from("or-chat-tool-1"),
            tool_round_request(vec![ConversationItem::user("use the tool")]),
        )
        .await
        .expect("tool-call turn");
    let calls = resp1.tool_calls();
    assert_eq!(calls.len(), 1, "expected one tool call: {:?}", resp1.items);
    assert_eq!(calls[0].id.as_ref(), TOOL_CALL_ID);
    assert_eq!(calls[0].name, TOOL_NAME);
    assert_eq!(calls[0].arguments.as_ref(), TOOL_ARGS);
    let usage1 = resp1.usage.expect("usage on tool-call turn");
    assert!(usage1.total_tokens > 0);

    // Simulate local tool execution and continue the conversation.
    let mut history = vec![ConversationItem::user("use the tool")];
    history.extend(resp1.items.clone());
    history.push(ConversationItem::tool_result(TOOL_CALL_ID, TOOL_RESULT));

    let (resp2, _m2) = handle
        .submit_and_collect(
            RequestId::from("or-chat-tool-2"),
            tool_round_request(history),
        )
        .await
        .expect("tool-result turn");
    assert!(
        resp2.assistant_text().contains("tool said hi"),
        "final answer missing, got {:?}",
        resp2.assistant_text()
    );
    assert!(resp2.tool_calls().is_empty());
    let usage2 = resp2.usage.expect("usage on final turn");
    assert!(usage2.total_tokens > 0);

    server.shutdown();

    let bodies = captured.lock().unwrap().clone();
    assert_eq!(bodies.len(), 2, "exactly two HTTP requests");
    assert_openrouter_chat_wire(&bodies[0]);
    assert_openrouter_chat_wire(&bodies[1]);
    // Follow-up must re-send local history including the tool result correlation.
    let msgs = bodies[1]["messages"]
        .as_array()
        .expect("chat messages array");
    let joined = serde_json::to_string(msgs).unwrap();
    assert!(
        joined.contains(TOOL_CALL_ID) && joined.contains(TOOL_RESULT.trim()),
        "follow-up must carry call id and tool result: {joined}"
    );
}

/// Full Responses tool round through the actor with OpenRouter identity:
/// stateless store=false, no previous_response_id, provider/plugins/models
/// retained on both turns, tool correlation and final text/usage.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn openrouter_responses_tool_round_trip_through_actor() {
    use std::sync::Mutex;

    let captured: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let captured_h = Arc::clone(&captured);
    let turn = Arc::new(AtomicU32::new(0));
    let turn_h = Arc::clone(&turn);

    let app = Router::new().route(
        "/v1/responses",
        post(move |axum::Json(body): axum::Json<serde_json::Value>| {
            let captured = Arc::clone(&captured_h);
            let turn = Arc::clone(&turn_h);
            async move {
                captured.lock().unwrap().push(body);
                let n = turn.fetch_add(1, Ordering::SeqCst);
                let events = if n == 0 {
                    sse_events_to_axum(sse::responses_api_reasoning_then_tool_call_events(
                        "plan the call",
                        TOOL_CALL_ID,
                        TOOL_NAME,
                        TOOL_ARGS,
                        OR_MODEL,
                    ))
                } else {
                    sse_events_to_axum(sse::responses_api_script_exact("tool said hi", OR_MODEL))
                };
                Sse::new(stream::iter(
                    events.into_iter().map(Ok::<_, std::convert::Infallible>),
                ))
            }
        }),
    );
    let server = MockServer::spawn(app).await;
    let (event_tx, _event_rx) = mpsc::unbounded_channel();
    let handle = InferenceActor::spawn(
        openrouter_config(server.base_url(), ApiBackend::Responses),
        RetryPolicy::default(),
        event_tx,
    );

    let (resp1, _m1) = handle
        .submit_and_collect(
            RequestId::from("or-resp-tool-1"),
            tool_round_request(vec![ConversationItem::user("use the tool")]),
        )
        .await
        .expect("responses tool-call turn");
    let calls = resp1.tool_calls();
    assert_eq!(calls.len(), 1, "expected one tool call: {:?}", resp1.items);
    assert_eq!(calls[0].id.as_ref(), TOOL_CALL_ID);
    assert_eq!(calls[0].name, TOOL_NAME);
    assert_eq!(calls[0].arguments.as_ref(), TOOL_ARGS);
    // Reasoning continuity: tool-call turn should surface reasoning items.
    assert!(
        resp1.reasoning_items().next().is_some()
            || resp1
                .items
                .iter()
                .any(|i| matches!(i, ConversationItem::Reasoning(_))),
        "expected reasoning continuity on tool-call turn: {:?}",
        resp1.items
    );
    let usage1 = resp1.usage.expect("usage on tool-call turn");
    assert!(usage1.total_tokens > 0);

    let mut history = vec![ConversationItem::user("use the tool")];
    history.extend(resp1.items.clone());
    history.push(ConversationItem::tool_result(TOOL_CALL_ID, TOOL_RESULT));

    let (resp2, _m2) = handle
        .submit_and_collect(
            RequestId::from("or-resp-tool-2"),
            tool_round_request(history),
        )
        .await
        .expect("responses tool-result turn");
    assert!(
        resp2.assistant_text().contains("tool said hi"),
        "final answer missing, got {:?}",
        resp2.assistant_text()
    );
    assert!(resp2.usage.is_some());

    server.shutdown();

    let bodies = captured.lock().unwrap().clone();
    assert_eq!(bodies.len(), 2);
    assert_openrouter_responses_wire(&bodies[0]);
    assert_openrouter_responses_wire(&bodies[1]);
    // Full local history re-sent as input (stateless); call id must appear.
    let input = serde_json::to_string(&bodies[1]["input"]).unwrap();
    assert!(
        input.contains(TOOL_CALL_ID),
        "follow-up input must re-send tool call id: {input}"
    );
}

/// Cancel mid-stream under OpenRouter Chat identity: no Completed event and
/// active count returns to zero (no post-cancel tool execution completion).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn openrouter_chat_cancel_under_identity() {
    let app = Router::new().route(
        "/v1/chat/completions",
        post(|| async {
            let first = sse_events_to_axum(vec![SseEvent::data(
                json!({
                    "id": "chatcmpl-or",
                    "object": "chat.completion.chunk",
                    "created": 0,
                    "model": OR_MODEL,
                    "choices": [{
                        "index": 0,
                        "delta": { "role": "assistant", "content": "starting" },
                        "finish_reason": null
                    }]
                })
                .to_string(),
            )]);
            let stream = stream::iter(first.into_iter().map(Ok::<_, std::convert::Infallible>))
                .chain(stream::pending());
            Sse::new(stream)
        }),
    );
    let server = MockServer::spawn(app).await;
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let handle = InferenceActor::spawn(
        openrouter_config(server.base_url(), ApiBackend::ChatCompletions),
        RetryPolicy::default(),
        event_tx,
    );

    let rid = RequestId::from("or-chat-cancel");
    handle.submit(
        rid.clone(),
        tool_round_request(vec![ConversationItem::user("hi")]),
    );
    let _ = await_event_matching(
        &mut event_rx,
        |e| matches!(e, InferenceEvent::FirstToken { .. }),
        Duration::from_secs(5),
    )
    .await
    .expect("first token under OpenRouter chat");
    handle.cancel(rid);

    let failed = await_event_matching(
        &mut event_rx,
        |e| matches!(e, InferenceEvent::Failed { .. }),
        Duration::from_secs(5),
    )
    .await
    .expect("Failed after cancel");
    match failed {
        InferenceEvent::Failed { error, .. } => {
            assert!(error.message.contains("cancelled"));
        }
        InferenceEvent::Completed { .. } => panic!("cancel must not complete"),
        other => panic!("expected Failed after cancel, got {other:?}"),
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(handle.active_count().await, 0);
    server.shutdown();
}

/// Cancel mid-stream under OpenRouter Responses identity.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn openrouter_responses_cancel_under_identity() {
    let app = Router::new().route(
        "/v1/responses",
        post(|| async {
            let mut events = sse_events_to_axum(sse::responses_api_script_exact(
                "should not finish",
                OR_MODEL,
            ));
            // Keep only the first frames then hang so cancel is observable.
            events.truncate(2);
            let stream = stream::iter(events.into_iter().map(Ok::<_, std::convert::Infallible>))
                .chain(stream::pending());
            Sse::new(stream)
        }),
    );
    let server = MockServer::spawn(app).await;
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let handle = InferenceActor::spawn(
        openrouter_config(server.base_url(), ApiBackend::Responses),
        RetryPolicy::default(),
        event_tx,
    );

    let rid = RequestId::from("or-resp-cancel");
    handle.submit(
        rid.clone(),
        tool_round_request(vec![ConversationItem::user("hi")]),
    );
    let _ = await_event_matching(
        &mut event_rx,
        |e| matches!(e, InferenceEvent::StreamStarted { .. }),
        Duration::from_secs(5),
    )
    .await
    .expect("stream started");
    // Give the stream a moment to attach, then cancel.
    tokio::time::sleep(Duration::from_millis(50)).await;
    handle.cancel(rid);

    let failed = await_event_matching(
        &mut event_rx,
        |e| matches!(e, InferenceEvent::Failed { .. }),
        Duration::from_secs(5),
    )
    .await
    .expect("Failed after cancel");
    if let InferenceEvent::Failed { error, .. } = failed {
        assert!(error.message.contains("cancelled"));
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(handle.active_count().await, 0);
    server.shutdown();
}

/// Backend switch Chat → Responses at the inference config/conversation layer:
/// reconstruct a tool-round conversation, switch `ApiBackend` via
/// `update_config`, and assert OpenRouter identity extensions + tool-call
/// correlation survive on the next wire request.
///
/// Token usage is per-response only (not re-sent as request state); each
/// completed turn carries its own `usage` and is not persisted into the next
/// request body by design.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn openrouter_backend_switch_preserves_tool_conversation_and_identity() {
    use std::sync::Mutex;

    let captured: Arc<Mutex<Vec<(String, serde_json::Value)>>> = Arc::new(Mutex::new(Vec::new()));
    let captured_h = Arc::clone(&captured);

    // Dual-route mock: Chat then Responses after config switch.
    let app = Router::new()
        .route(
            "/v1/chat/completions",
            post({
                let captured = Arc::clone(&captured_h);
                move |axum::Json(body): axum::Json<serde_json::Value>| {
                    let captured = Arc::clone(&captured);
                    async move {
                        captured.lock().unwrap().push(("chat".into(), body));
                        let events = sse_events_to_axum(
                            sse::chat_completions_reasoning_then_tool_call_events(
                                "plan",
                                TOOL_CALL_ID,
                                TOOL_NAME,
                                TOOL_ARGS,
                                OR_MODEL,
                            ),
                        );
                        Sse::new(stream::iter(
                            events.into_iter().map(Ok::<_, std::convert::Infallible>),
                        ))
                    }
                }
            }),
        )
        .route(
            "/v1/responses",
            post({
                let captured = Arc::clone(&captured_h);
                move |axum::Json(body): axum::Json<serde_json::Value>| {
                    let captured = Arc::clone(&captured);
                    async move {
                        captured.lock().unwrap().push(("responses".into(), body));
                        let events = sse_events_to_axum(sse::responses_api_script_exact(
                            "switched", OR_MODEL,
                        ));
                        Sse::new(stream::iter(
                            events.into_iter().map(Ok::<_, std::convert::Infallible>),
                        ))
                    }
                }
            }),
        );

    let server = MockServer::spawn(app).await;
    let (event_tx, _event_rx) = mpsc::unbounded_channel();
    let chat_cfg = openrouter_config(server.base_url(), ApiBackend::ChatCompletions);
    let handle = InferenceActor::spawn(chat_cfg, RetryPolicy::default(), event_tx);

    let (resp_chat, _) = handle
        .submit_and_collect(
            RequestId::from("or-switch-1"),
            tool_round_request(vec![ConversationItem::user("use the tool")]),
        )
        .await
        .expect("chat tool turn");
    assert_eq!(resp_chat.tool_calls()[0].id.as_ref(), TOOL_CALL_ID);
    // Usage is turn-local, not a session field.
    assert!(resp_chat.usage.is_some());

    // Reconstruct the conversation the session would keep after a tool round.
    let mut conversation = vec![ConversationItem::user("use the tool")];
    conversation.extend(resp_chat.items.clone());
    conversation.push(ConversationItem::tool_result(TOOL_CALL_ID, TOOL_RESULT));

    // Switch backend while keeping the same OpenRouter identity/extensions.
    let mut resp_cfg = openrouter_config(server.base_url(), ApiBackend::Responses);
    resp_cfg.api_key = Some("test-key".into());
    handle.update_config(resp_cfg);

    let (resp_switched, _) = handle
        .submit_and_collect(
            RequestId::from("or-switch-2"),
            tool_round_request(conversation.clone()),
        )
        .await
        .expect("responses follow-up after switch");
    assert!(
        resp_switched.assistant_text().contains("switched"),
        "got {:?}",
        resp_switched.assistant_text()
    );
    // New turn has its own usage; prior usage is not rehydrated into the request.
    assert!(resp_switched.usage.is_some());

    server.shutdown();

    let bodies = captured.lock().unwrap().clone();
    assert_eq!(bodies.len(), 2);
    assert_eq!(bodies[0].0, "chat");
    assert_eq!(bodies[1].0, "responses");
    assert_openrouter_chat_wire(&bodies[0].1);
    assert_openrouter_responses_wire(&bodies[1].1);

    // Conversation tool IDs and result content survive into the switched backend body.
    let wire = serde_json::to_string(&bodies[1].1).unwrap();
    assert!(
        wire.contains(TOOL_CALL_ID),
        "tool call id must survive backend switch: {wire}"
    );
    // Usage is not a request-body field (documented: per-response only).
    assert!(
        bodies[1].1.get("usage").is_none(),
        "usage must not be re-sent as request state"
    );
    // Identity-gated extensions still present after switch.
    assert_eq!(bodies[1].1["provider"]["data_collection"], "deny");
    assert_eq!(bodies[1].1["plugins"][0]["id"], "response-healing");
}

// ---------------------------------------------------------------------------
// Helpers for draining the event channel
// ---------------------------------------------------------------------------

/// Drain the event channel until a terminal event (`Completed` or
/// `Failed`) is received, or until `deadline` elapses.
async fn drain_until_terminal(
    rx: &mut mpsc::UnboundedReceiver<InferenceEvent>,
    timeout: Duration,
) -> Vec<InferenceEvent> {
    let mut out = Vec::new();
    let start = tokio::time::Instant::now();
    loop {
        let elapsed = start.elapsed();
        if elapsed >= timeout {
            panic!(
                "drain_until_terminal timed out after {:?}; got {} events",
                timeout,
                out.len()
            );
        }
        let remaining = timeout - elapsed;
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(ev)) => {
                let terminal = matches!(
                    ev,
                    InferenceEvent::Completed { .. } | InferenceEvent::Failed { .. }
                );
                out.push(ev);
                if terminal {
                    return out;
                }
            }
            Ok(None) => panic!("event channel closed before terminal event"),
            Err(_) => panic!(
                "drain_until_terminal timed out after {:?}; got {} events",
                timeout,
                out.len()
            ),
        }
    }
}

/// Wait for the next event matching `pred`, or return `None` on
/// timeout.
async fn await_event_matching(
    rx: &mut mpsc::UnboundedReceiver<InferenceEvent>,
    mut pred: impl FnMut(&InferenceEvent) -> bool,
    timeout: Duration,
) -> Option<InferenceEvent> {
    let start = tokio::time::Instant::now();
    loop {
        let elapsed = start.elapsed();
        if elapsed >= timeout {
            return None;
        }
        let remaining = timeout - elapsed;
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(ev)) => {
                if pred(&ev) {
                    return Some(ev);
                }
            }
            Ok(None) => return None,
            Err(_) => return None,
        }
    }
}
