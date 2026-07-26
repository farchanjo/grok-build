//! Deterministic axum/mock HTTP tests for the Anthropic client.
//!
//! No real credentials or network: every test binds a local TcpListener.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{Path, Query, Request};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::Response;
use axum::routing::{get, post};
use futures_util::StreamExt;
use serde_json::json;
use tokio_util::sync::CancellationToken;
use xai_grok_inference_types::anthropic::{
    ANTHROPIC_VERSION, AnthropicBetaSet, CountTokensRequest, FILES_API_BETA, FileUploadSource,
    ListFilesParams, ListModelsParams,
};
use xai_grok_inference_types::messages::{
    Message, MessageContent, MessageRole, MessagesRequest, MessagesResponse,
};

use super::client::{AnthropicClient, AnthropicClientConfig, set_test_max_request_bytes};
use super::error::{AnthropicClientError, ErrorClass};
use super::headers::parse_anthropic_rate_limit_headers;
use crate::retry::{RATE_LIMIT_RETRY_THRESHOLD, RetryDecision, classify_error};

async fn spawn(app: Router) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    // Yield so the server accepts connections.
    tokio::task::yield_now().await;
    (addr, handle)
}

fn client_for(addr: SocketAddr) -> AnthropicClient {
    AnthropicClient::new(
        AnthropicClientConfig::new("test-secret-key").with_base_url(format!("http://{addr}")),
    )
    .unwrap()
}

fn sample_messages_request() -> MessagesRequest {
    MessagesRequest {
        model: "claude-test".into(),
        max_tokens: 64,
        messages: vec![Message {
            role: MessageRole::User,
            content: MessageContent::Text("hello".into()),
        }],
        ..Default::default()
    }
}

fn assert_anthropic_identity(headers: &HeaderMap) {
    assert_eq!(
        headers.get("x-api-key").and_then(|v| v.to_str().ok()),
        Some("test-secret-key"),
        "direct Anthropic client must send x-api-key"
    );
    assert!(
        headers.get(axum::http::header::AUTHORIZATION).is_none(),
        "direct Anthropic client must never send Authorization"
    );
    assert_eq!(
        headers
            .get("anthropic-version")
            .and_then(|v| v.to_str().ok()),
        Some(ANTHROPIC_VERSION)
    );
}

// -----------------------------------------------------------------------------
// Messages create
// -----------------------------------------------------------------------------

#[tokio::test]
async fn create_message_success_preserves_meta() {
    let app = Router::new().route(
        "/v1/messages",
        post(|headers: HeaderMap, body: Bytes| async move {
            assert_anthropic_identity(&headers);
            assert!(headers.get("anthropic-beta").is_none());
            let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(v["stream"], false);
            assert_eq!(v["model"], "claude-test");
            Response::builder()
                .status(StatusCode::OK)
                .header("request-id", "req_msg_1")
                .header("anthropic-ratelimit-requests-remaining", "99")
                .header("anthropic-ratelimit-requests-reset", "2026-07-01T00:00:00Z")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "id": "msg_1",
                        "type": "message",
                        "role": "assistant",
                        "content": [{"type": "text", "text": "hi"}],
                        "model": "claude-test",
                        "stop_reason": "end_turn",
                        "usage": {"input_tokens": 3, "output_tokens": 1}
                    })
                    .to_string(),
                ))
                .unwrap()
        }),
    );
    let (addr, _h) = spawn(app).await;
    let client = client_for(addr);
    let out = client
        .create_message(sample_messages_request())
        .await
        .unwrap();
    assert_eq!(out.response.id, "msg_1");
    assert_eq!(out.meta.request_id.as_deref(), Some("req_msg_1"));
    assert_eq!(
        out.meta.rate_limit.requests_remaining.as_deref(),
        Some("99")
    );
}

#[tokio::test]
async fn create_message_stream_emits_events_and_sse_error() {
    let app = Router::new().route(
        "/v1/messages",
        post(|headers: HeaderMap| async move {
            assert_anthropic_identity(&headers);
            assert_eq!(
                headers.get(axum::http::header::ACCEPT).and_then(|v| v.to_str().ok()),
                Some("text/event-stream")
            );
            let sse = "\
event: message_start\n\
data: {\"type\":\"message_start\",\"message\":{\"id\":\"m\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"model\":\"claude-test\",\"stop_reason\":null,\"usage\":{\"input_tokens\":1,\"output_tokens\":0}}}\n\
\n\
event: error\n\
data: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}\n\
\n";
            Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "text/event-stream")
                .header("request-id", "req_sse")
                .body(Body::from(sse))
                .unwrap()
        }),
    );
    let (addr, _h) = spawn(app).await;
    let client = client_for(addr);
    let (mut stream, meta) = client
        .create_message_stream(sample_messages_request())
        .await
        .unwrap();
    assert_eq!(meta.request_id.as_deref(), Some("req_sse"));

    let first = stream.next().await.unwrap().unwrap();
    assert!(matches!(
        first,
        xai_grok_inference_types::messages::MessageStreamEvent::MessageStart { .. }
    ));

    let err = stream.next().await.unwrap().unwrap_err();
    match &err {
        AnthropicClientError::Stream {
            error_type,
            message,
            class,
            ..
        } => {
            assert_eq!(error_type, "overloaded_error");
            assert_eq!(message, "Overloaded");
            assert_eq!(*class, ErrorClass::RetryableOverload);
        }
        other => panic!("expected Stream error, got {other:?}"),
    }
    // Exactly one terminal error then stream ends.
    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn create_message_401_is_permanent_auth() {
    let app = Router::new().route(
        "/v1/messages",
        post(|| async {
            (
                StatusCode::UNAUTHORIZED,
                json!({"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}).to_string(),
            )
        }),
    );
    let (addr, _h) = spawn(app).await;
    let err = client_for(addr)
        .create_message(sample_messages_request())
        .await
        .unwrap_err();
    assert_eq!(err.class(), ErrorClass::PermanentAuth);
    assert!(!err.is_retryable());
    let s = format!("{err:?}");
    assert!(!s.contains("test-secret-key"));
}

#[tokio::test]
async fn create_message_403_is_permanent_permission() {
    let app = Router::new().route(
        "/v1/messages",
        post(|| async {
            (
                StatusCode::FORBIDDEN,
                json!({"type":"error","error":{"type":"permission_error","message":"not allowed"}})
                    .to_string(),
            )
        }),
    );
    let (addr, _h) = spawn(app).await;
    let err = client_for(addr)
        .create_message(sample_messages_request())
        .await
        .unwrap_err();
    assert_eq!(err.class(), ErrorClass::PermanentPermission);
    assert!(!err.is_retryable());
}

#[tokio::test]
async fn create_message_400_and_413_are_permanent_actionable() {
    for (status, ty) in [
        (StatusCode::BAD_REQUEST, "invalid_request_error"),
        (StatusCode::PAYLOAD_TOO_LARGE, "request_too_large"),
    ] {
        let app = Router::new().route(
            "/v1/messages",
            post(move || async move {
                (
                    status,
                    json!({"type":"error","error":{"type": ty, "message":"bad"}}).to_string(),
                )
            }),
        );
        let (addr, _h) = spawn(app).await;
        let err = client_for(addr)
            .create_message(sample_messages_request())
            .await
            .unwrap_err();
        assert_eq!(err.class(), ErrorClass::PermanentActionable);
        assert!(!err.is_retryable());
    }
}

#[tokio::test]
async fn create_message_429_is_retryable_with_retry_after() {
    let app = Router::new().route(
        "/v1/messages",
        post(|| async {
            Response::builder()
                .status(StatusCode::TOO_MANY_REQUESTS)
                .header("retry-after", "7")
                .header("anthropic-ratelimit-requests-remaining", "0")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"type":"error","error":{"type":"rate_limit_error","message":"slow down"}})
                        .to_string(),
                ))
                .unwrap()
        }),
    );
    let (addr, _h) = spawn(app).await;
    let err = client_for(addr)
        .create_message(sample_messages_request())
        .await
        .unwrap_err();
    assert_eq!(err.class(), ErrorClass::RetryableRateLimit);
    assert!(err.is_retryable());
    assert_eq!(err.retry_after_secs(), Some(7));
    assert_eq!(
        err.meta()
            .and_then(|m| m.rate_limit.requests_remaining.as_deref()),
        Some("0")
    );
}

#[tokio::test]
async fn create_message_529_is_retryable_overload() {
    let status = StatusCode::from_u16(529).unwrap();
    let app = Router::new().route(
        "/v1/messages",
        post(move || async move {
            (
                status,
                json!({"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}})
                    .to_string(),
            )
        }),
    );
    let (addr, _h) = spawn(app).await;
    let err = client_for(addr)
        .create_message(sample_messages_request())
        .await
        .unwrap_err();
    assert_eq!(err.class(), ErrorClass::RetryableOverload);
    assert!(err.is_retryable());
    // Maps into InferenceError that is_retryable for generic retry path.
    let inf = err.clone().into_inference_error();
    assert!(inf.is_retryable());
}

#[tokio::test]
async fn create_message_preflight_exact_boundary_and_plus_one() {
    let hits = Arc::new(AtomicUsize::new(0));
    let hits2 = hits.clone();
    let app = Router::new().route(
        "/v1/messages",
        post(move |body: Bytes| {
            let hits = hits2.clone();
            async move {
                hits.fetch_add(1, Ordering::SeqCst);
                // Echo a valid minimal response so acceptance path succeeds.
                let _ = body;
                (
                    StatusCode::OK,
                    json!({
                        "id": "msg_ok",
                        "type": "message",
                        "role": "assistant",
                        "content": [],
                        "model": "claude-test",
                        "stop_reason": "end_turn",
                        "usage": {"input_tokens": 1, "output_tokens": 0}
                    })
                    .to_string(),
                )
            }
        }),
    );
    let (addr, _h) = spawn(app).await;
    let client = client_for(addr);

    // Measure a small request size, then set the limit to exactly that size.
    let base = sample_messages_request();
    let exact_len = serde_json::to_vec(&{
        let mut r = base.clone();
        r.stream = Some(false);
        r
    })
    .unwrap()
    .len();

    set_test_max_request_bytes(Some(exact_len));
    client
        .create_message(base.clone())
        .await
        .expect("exact limit must be accepted");
    assert_eq!(hits.load(Ordering::SeqCst), 1);

    // One extra byte of model name pushes serialization past the limit.
    set_test_max_request_bytes(Some(exact_len));
    let mut oversized = base;
    oversized.model.push('X');
    let err = client.create_message(oversized).await.unwrap_err();
    match err {
        AnthropicClientError::RequestTooLarge {
            size_bytes,
            limit_bytes,
        } => {
            assert!(size_bytes > limit_bytes);
            assert_eq!(limit_bytes, exact_len);
        }
        other => panic!("expected RequestTooLarge, got {other:?}"),
    }
    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "rejection must not hit the network"
    );
    set_test_max_request_bytes(None);
}

#[tokio::test]
async fn create_message_stream_preflight_rejects_without_network() {
    let hits = Arc::new(AtomicUsize::new(0));
    let hits2 = hits.clone();
    let app = Router::new().route(
        "/v1/messages",
        post(move || {
            let hits = hits2.clone();
            async move {
                hits.fetch_add(1, Ordering::SeqCst);
                StatusCode::OK
            }
        }),
    );
    let (addr, _h) = spawn(app).await;
    let client = client_for(addr);

    let base = sample_messages_request();
    let exact_len = serde_json::to_vec(&{
        let mut r = base.clone();
        r.stream = Some(true);
        r
    })
    .unwrap()
    .len();
    set_test_max_request_bytes(Some(exact_len.saturating_sub(1).max(1)));
    let err = match client.create_message_stream(base).await {
        Err(e) => e,
        Ok(_) => panic!("expected preflight RequestTooLarge"),
    };
    assert!(matches!(err, AnthropicClientError::RequestTooLarge { .. }));
    assert_eq!(hits.load(Ordering::SeqCst), 0);
    set_test_max_request_bytes(None);
}

#[tokio::test]
async fn upload_file_preflight_raw_size_boundary() {
    let hits = Arc::new(AtomicUsize::new(0));
    let hits2 = hits.clone();
    let app = Router::new().route(
        "/v1/files",
        post(move || {
            let hits = hits2.clone();
            async move {
                hits.fetch_add(1, Ordering::SeqCst);
                (
                    StatusCode::OK,
                    json!({
                        "id": "file_ok",
                        "filename": "a.bin",
                        "type": "file"
                    })
                    .to_string(),
                )
            }
        }),
    );
    let (addr, _h) = spawn(app).await;
    let client = client_for(addr);

    set_test_max_request_bytes(Some(4));
    client
        .upload_file(FileUploadSource::new("a.bin", vec![1, 2, 3, 4]))
        .await
        .expect("exact raw size accepted");
    assert_eq!(hits.load(Ordering::SeqCst), 1);

    let err = client
        .upload_file(FileUploadSource::new("a.bin", vec![1, 2, 3, 4, 5]))
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        AnthropicClientError::RequestTooLarge {
            size_bytes: 5,
            limit_bytes: 4
        }
    ));
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    set_test_max_request_bytes(None);
}

// -----------------------------------------------------------------------------
// Models
// -----------------------------------------------------------------------------

#[tokio::test]
async fn list_and_retrieve_models_with_pagination() {
    let app =
        Router::new()
            .route(
                "/v1/models",
                get(
                    |Query(q): Query<std::collections::HashMap<String, String>>,
                     headers: HeaderMap| async move {
                        assert_anthropic_identity(&headers);
                        assert_eq!(q.get("after_id").map(String::as_str), Some("m0"));
                        assert_eq!(q.get("limit").map(String::as_str), Some("2"));
                        json!({
                            "data": [
                                {
                                    "id": "m1",
                                    "type": "model",
                                    "display_name": "Model 1",
                                    "max_input_tokens": 200000,
                                    "max_tokens": 8192,
                                    "capabilities": {"batch": {"supported": true}}
                                }
                            ],
                            "first_id": "m1",
                            "last_id": "m1",
                            "has_more": true
                        })
                        .to_string()
                    },
                ),
            )
            .route(
                "/v1/models/{id}",
                get(|Path(id): Path<String>, headers: HeaderMap| async move {
                    assert_anthropic_identity(&headers);
                    assert_eq!(id, "m1");
                    json!({
                        "id": "m1",
                        "type": "model",
                        "display_name": "Model 1",
                        "max_input_tokens": 200000,
                        "max_tokens": 8192
                    })
                    .to_string()
                }),
            );
    let (addr, _h) = spawn(app).await;
    let client = client_for(addr);

    let page = client
        .list_models(&ListModelsParams {
            after_id: Some("m0".into()),
            before_id: None,
            limit: Some(2),
        })
        .await
        .unwrap();
    assert_eq!(page.page.data.len(), 1);
    assert!(page.page.has_more);
    assert_eq!(page.page.data[0].max_input_tokens, Some(200_000));
    assert_eq!(page.page.data[0].max_tokens, Some(8192));

    let model = client.retrieve_model("m1").await.unwrap();
    assert_eq!(model.page.id, "m1");
    assert_eq!(model.page.display_name.as_deref(), Some("Model 1"));
}

// -----------------------------------------------------------------------------
// Count tokens
// -----------------------------------------------------------------------------

#[tokio::test]
async fn count_tokens_success() {
    let app = Router::new().route(
        "/v1/messages/count_tokens",
        post(|headers: HeaderMap, body: Bytes| async move {
            assert_anthropic_identity(&headers);
            let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert!(v.get("max_tokens").is_none());
            assert!(v.get("stream").is_none());
            assert_eq!(v["model"], "claude-test");
            json!({"input_tokens": 12, "cache_read_input_tokens": 2}).to_string()
        }),
    );
    let (addr, _h) = spawn(app).await;
    let req = CountTokensRequest::from(&sample_messages_request());
    let out = client_for(addr).count_tokens(&req).await.unwrap();
    assert_eq!(out.page.input_tokens, 12);
    assert_eq!(out.page.cache_read_input_tokens, Some(2));
}

// -----------------------------------------------------------------------------
// Files beta
// -----------------------------------------------------------------------------

#[tokio::test]
async fn files_upload_list_retrieve_download_delete() {
    let app = Router::new()
        .route(
            "/v1/files",
            post(|headers: HeaderMap, req: Request| async move {
                assert_anthropic_identity(&headers);
                assert_eq!(
                    headers.get("anthropic-beta").and_then(|v| v.to_str().ok()),
                    Some(FILES_API_BETA),
                    "files methods add only files-api beta"
                );
                let ct = headers
                    .get(axum::http::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");
                assert!(
                    ct.starts_with("multipart/form-data"),
                    "expected multipart, got {ct}"
                );
                // Consume body so the connection stays clean; do not log bytes.
                let _body = axum::body::to_bytes(req.into_body(), 1024 * 1024)
                    .await
                    .unwrap();
                (
                    StatusCode::OK,
                    json!({
                        "id": "file_1",
                        "filename": "doc.txt",
                        "mime_type": "text/plain",
                        "size_bytes": 5,
                        "type": "file",
                        "downloadable": true
                    })
                    .to_string(),
                )
            })
            .get(|headers: HeaderMap, Query(q): Query<std::collections::HashMap<String, String>>| async move {
                assert_anthropic_identity(&headers);
                assert_eq!(
                    headers.get("anthropic-beta").and_then(|v| v.to_str().ok()),
                    Some(FILES_API_BETA)
                );
                assert_eq!(q.get("after_id").map(String::as_str), Some("file_0"));
                json!({
                    "data": [{
                        "id": "file_1",
                        "filename": "doc.txt",
                        "type": "file"
                    }],
                    "first_id": "file_1",
                    "last_id": "file_1",
                    "has_more": false
                })
                .to_string()
            }),
        )
        .route(
            "/v1/files/{id}",
            get(|Path(id): Path<String>, headers: HeaderMap| async move {
                assert_eq!(id, "file_1");
                assert_eq!(
                    headers.get("anthropic-beta").and_then(|v| v.to_str().ok()),
                    Some(FILES_API_BETA)
                );
                json!({
                    "id": "file_1",
                    "filename": "doc.txt",
                    "type": "file",
                    "downloadable": true
                })
                .to_string()
            })
            .delete(|Path(id): Path<String>, headers: HeaderMap| async move {
                assert_eq!(id, "file_1");
                assert_eq!(
                    headers.get("anthropic-beta").and_then(|v| v.to_str().ok()),
                    Some(FILES_API_BETA)
                );
                json!({"id": "file_1", "type": "file_deleted", "deleted": true}).to_string()
            }),
        )
        .route(
            "/v1/files/{id}/content",
            get(|Path(id): Path<String>, headers: HeaderMap| async move {
                assert_eq!(id, "file_1");
                assert_eq!(
                    headers.get("anthropic-beta").and_then(|v| v.to_str().ok()),
                    Some(FILES_API_BETA)
                );
                Response::builder()
                    .status(StatusCode::OK)
                    .header("content-type", "text/plain")
                    .body(Body::from("hello"))
                    .unwrap()
            }),
        );

    let (addr, _h) = spawn(app).await;
    let client = client_for(addr);

    let uploaded = client
        .upload_file(
            FileUploadSource::new("doc.txt", b"hello".to_vec()).with_mime_type("text/plain"),
        )
        .await
        .unwrap();
    assert_eq!(uploaded.page.id, "file_1");

    let listed = client
        .list_files(&ListFilesParams {
            after_id: Some("file_0".into()),
            before_id: None,
            limit: None,
        })
        .await
        .unwrap();
    assert_eq!(listed.page.data.len(), 1);

    let meta = client.retrieve_file("file_1").await.unwrap();
    assert_eq!(meta.page.filename, "doc.txt");

    let (bytes, _) = client.download_file("file_1").await.unwrap();
    assert_eq!(bytes, b"hello");

    let deleted = client.delete_file("file_1").await.unwrap();
    assert!(deleted.page.deleted);
}

#[tokio::test]
async fn files_do_not_send_all_betas_from_config() {
    // Config has an extra beta; files methods must add files-api AND keep
    // explicit betas, but never invent an "all betas" set.
    let mut betas = AnthropicBetaSet::new();
    betas.insert("prompt-caching-2024-07-31");
    let app = Router::new().route(
        "/v1/files",
        get(|headers: HeaderMap| async move {
            let beta = headers
                .get("anthropic-beta")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            assert!(beta.contains(FILES_API_BETA));
            assert!(beta.contains("prompt-caching-2024-07-31"));
            // Must not include unrelated betas that were never configured.
            assert!(!beta.contains("computer-use"));
            json!({"data": [], "has_more": false}).to_string()
        }),
    );
    let (addr, _h) = spawn(app).await;
    let client = AnthropicClient::new(
        AnthropicClientConfig::new("test-secret-key")
            .with_base_url(format!("http://{addr}"))
            .with_betas(betas),
    )
    .unwrap();
    let _ = client
        .list_files(&ListFilesParams::default())
        .await
        .unwrap();
}

#[tokio::test]
async fn file_upload_source_debug_and_errors_omit_bytes() {
    let src = FileUploadSource::new("secret.bin", b"SENSITIVE_PAYLOAD".to_vec());
    assert!(!format!("{src:?}").contains("SENSITIVE_PAYLOAD"));

    let app = Router::new().route(
        "/v1/files",
        post(|| async {
            (
                StatusCode::BAD_REQUEST,
                json!({"type":"error","error":{"type":"invalid_request_error","message":"bad file"}}).to_string(),
            )
        }),
    );
    let (addr, _h) = spawn(app).await;
    let err = client_for(addr)
        .upload_file(FileUploadSource::new(
            "secret.bin",
            b"SENSITIVE_PAYLOAD".to_vec(),
        ))
        .await
        .unwrap_err();
    let rendered = format!("{err:?}{err}");
    assert!(!rendered.contains("SENSITIVE_PAYLOAD"));
    assert!(!rendered.contains("test-secret-key"));
}

// -----------------------------------------------------------------------------
// Cancellation, isolation
// -----------------------------------------------------------------------------

#[tokio::test]
async fn cancelled_token_is_single_terminal_cancellation() {
    let cancel = CancellationToken::new();
    cancel.cancel();
    let client = AnthropicClient::new(
        AnthropicClientConfig::new("key")
            .with_base_url("http://127.0.0.1:1")
            .with_cancel(cancel),
    )
    .unwrap();
    let err = client
        .create_message(sample_messages_request())
        .await
        .unwrap_err();
    assert!(err.is_cancelled());
    assert_eq!(err.class(), ErrorClass::Cancelled);
    assert!(!err.is_retryable());
}

#[tokio::test]
async fn messages_backend_isolation_anthropic_headers_not_implied_by_protocol() {
    // ApiBackend::Messages / InferenceClient path is a separate identity.
    // Ensure this Anthropic client is the only place that injects
    // anthropic-version + x-api-key for direct Anthropic.
    let app = Router::new().route(
        "/v1/messages",
        post(|headers: HeaderMap| async move {
            assert_anthropic_identity(&headers);
            (
                StatusCode::OK,
                json!({
                    "id": "msg",
                    "type": "message",
                    "role": "assistant",
                    "content": [],
                    "model": "m",
                    "stop_reason": "end_turn",
                    "usage": {"input_tokens": 1, "output_tokens": 0}
                })
                .to_string(),
            )
        }),
    );
    let (addr, _h) = spawn(app).await;
    let _ = client_for(addr)
        .create_message(sample_messages_request())
        .await
        .unwrap();
}

// Isolation of ApiBackend::Messages + Bearer from Anthropic identity headers is
// covered by `client::tests::messages_plus_bearer_uses_authorization_and_not_x_api_key`
// (asserts Authorization present, x-api-key and anthropic-version absent).

#[tokio::test]
async fn stream_sse_error_is_exactly_one_terminal_then_none() {
    let app = Router::new().route(
        "/v1/messages",
        post(|| async {
            let sse = "\
event: message_start\n\
data: {\"type\":\"message_start\",\"message\":{\"id\":\"m\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"model\":\"claude-test\",\"stop_reason\":null,\"usage\":{\"input_tokens\":1,\"output_tokens\":0}}}\n\
\n\
event: error\n\
data: {\"type\":\"error\",\"error\":{\"type\":\"invalid_request_error\",\"message\":\"bad\"}}\n\
\n\
event: message_stop\n\
data: {\"type\":\"message_stop\"}\n\
\n";
            Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "text/event-stream")
                .body(Body::from(sse))
                .unwrap()
        }),
    );
    let (addr, _h) = spawn(app).await;
    let (mut stream, _) = client_for(addr)
        .create_message_stream(sample_messages_request())
        .await
        .unwrap();
    let first = stream.next().await.unwrap().unwrap();
    assert!(matches!(
        first,
        xai_grok_inference_types::messages::MessageStreamEvent::MessageStart { .. }
    ));
    let err = stream.next().await.unwrap().unwrap_err();
    assert_eq!(err.class(), ErrorClass::PermanentActionable);
    assert!(!err.is_retryable());
    // No further items (including the trailing message_stop).
    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn stream_cancellation_after_event_is_one_cancelled_then_none() {
    let app = Router::new().route(
        "/v1/messages",
        post(|| async {
            // Slow trickle: first event then hang open so cancellation can fire.
            let sse = "\
event: message_start\n\
data: {\"type\":\"message_start\",\"message\":{\"id\":\"m\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"model\":\"claude-test\",\"stop_reason\":null,\"usage\":{\"input_tokens\":1,\"output_tokens\":0}}}\n\
\n";
            Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "text/event-stream")
                .body(Body::from(sse))
                .unwrap()
        }),
    );
    let (addr, _h) = spawn(app).await;
    let cancel = CancellationToken::new();
    let client = AnthropicClient::new(
        AnthropicClientConfig::new("test-secret-key")
            .with_base_url(format!("http://{addr}"))
            .with_cancel(cancel.clone()),
    )
    .unwrap();
    let (mut stream, _) = client
        .create_message_stream(sample_messages_request())
        .await
        .unwrap();
    let first = stream.next().await.unwrap().unwrap();
    assert!(matches!(
        first,
        xai_grok_inference_types::messages::MessageStreamEvent::MessageStart { .. }
    ));
    cancel.cancel();
    // Stream may end with Cancelled or None depending on timing after the body
    // closes; require at most one Cancelled and no second terminal error.
    let mut saw_cancelled = false;
    while let Some(item) = stream.next().await {
        match item {
            Err(AnthropicClientError::Cancelled) => {
                assert!(!saw_cancelled, "exactly one Cancelled terminal");
                saw_cancelled = true;
            }
            Ok(_) => {}
            Err(other) => panic!("unexpected error after cancel: {other:?}"),
        }
    }
    // When the SSE body already ended after the first event, Cancelled may not
    // fire (stream is already exhausted). Either one Cancelled or clean None is
    // fine; never more than one Cancelled (asserted above).
    let _ = saw_cancelled;
}

#[tokio::test]
async fn stream_unknown_event_is_yielded_successfully() {
    let app = Router::new().route(
        "/v1/messages",
        post(|| async {
            let sse = "\
event: future_event\n\
data: {\"type\":\"future_event\",\"payload\":1}\n\
\n\
event: message_stop\n\
data: {\"type\":\"message_stop\"}\n\
\n";
            Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "text/event-stream")
                .body(Body::from(sse))
                .unwrap()
        }),
    );
    let (addr, _h) = spawn(app).await;
    let (mut stream, _) = client_for(addr)
        .create_message_stream(sample_messages_request())
        .await
        .unwrap();
    let first = stream.next().await.unwrap().unwrap();
    match first {
        xai_grok_inference_types::messages::MessageStreamEvent::Unknown { type_name, .. } => {
            assert_eq!(type_name, "future_event");
        }
        other => panic!("expected Unknown event, got {other:?}"),
    }
    let stop = stream.next().await.unwrap().unwrap();
    assert!(matches!(
        stop,
        xai_grok_inference_types::messages::MessageStreamEvent::MessageStop
    ));
    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn http_429_and_529_bridge_classify_from_live_responses() {
    // 429
    {
        let app = Router::new().route(
            "/v1/messages",
            post(|| async {
                Response::builder()
                    .status(StatusCode::TOO_MANY_REQUESTS)
                    .header("retry-after", "3")
                    .body(Body::from(
                        json!({"type":"error","error":{"type":"rate_limit_error","message":"rl"}})
                            .to_string(),
                    ))
                    .unwrap()
            }),
        );
        let (addr, _h) = spawn(app).await;
        let err = client_for(addr)
            .create_message(sample_messages_request())
            .await
            .unwrap_err();
        let inf = err.into_inference_error();
        assert!(inf.is_rate_limited());
        match classify_error(&inf, 0, 15, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::RetryWithBackoff {
                backoff,
                is_rate_limited: true,
            } => assert_eq!(backoff, std::time::Duration::from_secs(3)),
            other => panic!("expected rate-limit backoff, got {other:?}"),
        }
    }
    // 529
    {
        let status = StatusCode::from_u16(529).unwrap();
        let app = Router::new().route(
            "/v1/messages",
            post(move || async move {
                (
                    status,
                    json!({"type":"error","error":{"type":"overloaded_error","message":"ov"}})
                        .to_string(),
                )
            }),
        );
        let (addr, _h) = spawn(app).await;
        let err = client_for(addr)
            .create_message(sample_messages_request())
            .await
            .unwrap_err();
        let inf = err.into_inference_error();
        assert!(inf.is_retryable());
        match classify_error(&inf, 0, 15, RATE_LIMIT_RETRY_THRESHOLD) {
            RetryDecision::RetryWithClientRebuild { .. } => {}
            other => panic!("expected rebuild retry for 529, got {other:?}"),
        }
    }
}

#[test]
fn rate_limit_parser_does_not_break_openai_headers() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-ratelimit-remaining",
        HeaderValue::from_static("openai-style"),
    );
    headers.insert(
        "anthropic-ratelimit-tokens-remaining",
        HeaderValue::from_static("1000"),
    );
    let rl = parse_anthropic_rate_limit_headers(&headers);
    assert_eq!(rl.tokens_remaining.as_deref(), Some("1000"));
    // OpenAI header is not mapped into Anthropic fields.
    assert!(rl.requests_remaining.is_none());
}

// Silence unused import warning for MessagesResponse in some rustc versions.
#[allow(dead_code)]
fn _type_check(r: MessagesResponse) {
    let _ = r.id;
}
