//! Hermetic mock-HTTP tests for retrieval transport + embeddings + rerank.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use serde_json::json;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use super::base64_f32::encode_standard_base64;
use super::embeddings::OpenaiCompatibleEmbeddings;
use super::transport::{RetrievalCredential, RetrievalTransport};
use super::types::{
    EmbeddingEncodingFormat, EmbeddingRequest, RerankRequest, RetrievalAuthScheme, RetrievalError,
    RetrievalPurpose, RetrievalRouteContext,
};
use super::vllm_rerank::VllmRerankAdapter;

#[derive(Clone, Default)]
struct Spy {
    requests: Arc<Mutex<Vec<Captured>>>,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
struct Captured {
    authorization: Option<String>,
    custom_auth: Option<String>,
    body: String,
}

fn route_for(base: &str, auth: RetrievalAuthScheme) -> RetrievalRouteContext {
    RetrievalRouteContext {
        provider_instance_id: "lab".into(),
        provider_kind: "openai_compatible".into(),
        api_surface: "openai_compatible_subset".into(),
        credential_route: "api_key".into(),
        auth_scheme: auth,
        base_url: base.to_owned(),
        display_name: "Lab".into(),
        organization: Some("org-1".into()),
        project: Some("proj-1".into()),
        extra_headers: vec![("X-Test".into(), "1".into())],
        incarnation: None,
        registry_generation: 1,
        request_timeout: Duration::from_secs(5),
        connect_timeout: Duration::from_secs(2),
        total_deadline: Duration::from_secs(15),
        max_retries: 2,
        max_redirects: 3,
        max_response_bytes: 1024 * 1024,
        purpose: RetrievalPurpose::Embeddings,
    }
}

async fn spawn_router(app: Router) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    // Tiny yield so accept loop is ready.
    tokio::task::yield_now().await;
    (addr, handle)
}

#[tokio::test]
async fn embeddings_float_happy_and_auth_headers() {
    let spy = Spy::default();
    let spy2 = spy.clone();
    let app = Router::new()
        .route(
            "/v1/embeddings",
            post(move |State(s): State<Spy>, req: Request<Body>| {
                let s = s.clone();
                async move {
                    let headers = req.headers().clone();
                    let bytes = axum::body::to_bytes(req.into_body(), 1024 * 1024)
                        .await
                        .unwrap();
                    let body = String::from_utf8_lossy(&bytes).into_owned();
                    s.requests.lock().await.push(Captured {
                        authorization: headers
                            .get("authorization")
                            .and_then(|v| v.to_str().ok())
                            .map(str::to_owned),
                        custom_auth: headers
                            .get("x-custom-key")
                            .and_then(|v| v.to_str().ok())
                            .map(str::to_owned),
                        body,
                    });
                    (
                        StatusCode::OK,
                        axum::Json(json!({
                            "model": "emb-1",
                            "data": [
                                {"index": 1, "embedding": [0.0, 1.0]},
                                {"index": 0, "embedding": [1.0, 0.0]}
                            ]
                        })),
                    )
                        .into_response()
                }
            }),
        )
        .with_state(spy2);
    let (addr, _h) = spawn_router(app).await;
    let base = format!("http://{addr}/v1");
    let route = route_for(&base, RetrievalAuthScheme::Bearer);
    let client = OpenaiCompatibleEmbeddings::new(route).unwrap();
    let cred = RetrievalCredential::new(Some("secret-a".into()));
    let res = client
        .embed(
            EmbeddingRequest {
                model: "emb-1".into(),
                inputs: vec!["hello".into(), "world".into()],
                dimensions: Some(2),
                encoding: EmbeddingEncodingFormat::Float,
                endpoint: "/embeddings".into(),
            },
            &cred,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(res.vectors[0].values, vec![1.0, 0.0]);
    assert_eq!(res.vectors[1].values, vec![0.0, 1.0]);
    let caps = spy.requests.lock().await;
    assert_eq!(caps.len(), 1);
    assert_eq!(caps[0].authorization.as_deref(), Some("Bearer secret-a"));
    assert!(caps[0].body.contains("\"dimensions\":2"));
    assert!(caps[0].body.contains("encoding_format"));
}

#[tokio::test]
async fn embeddings_base64_happy() {
    let floats = [0.25f32, -0.5, 1.0];
    let mut bytes = Vec::new();
    for f in floats {
        bytes.extend_from_slice(&f.to_le_bytes());
    }
    let b64 = encode_standard_base64(&bytes);
    let app = Router::new().route(
        "/v1/embeddings",
        post(move || {
            let b64 = b64.clone();
            async move {
                axum::Json(json!({
                    "data": [{"index": 0, "embedding": b64}]
                }))
            }
        }),
    );
    let (addr, _h) = spawn_router(app).await;
    let base = format!("http://{addr}/v1");
    let client =
        OpenaiCompatibleEmbeddings::new(route_for(&base, RetrievalAuthScheme::None)).unwrap();
    let res = client
        .embed(
            EmbeddingRequest {
                model: "m".into(),
                inputs: vec!["x".into()],
                dimensions: Some(3),
                encoding: EmbeddingEncodingFormat::Base64,
                endpoint: "embeddings".into(),
            },
            &RetrievalCredential::none(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(res.vectors[0].values, floats);
}

#[tokio::test]
async fn no_auth_sends_no_authorization() {
    let spy = Spy::default();
    let spy2 = spy.clone();
    let app = Router::new()
        .route(
            "/v1/embeddings",
            post(move |State(s): State<Spy>, req: Request<Body>| {
                let s = s.clone();
                async move {
                    let headers = req.headers().clone();
                    s.requests.lock().await.push(Captured {
                        authorization: headers
                            .get("authorization")
                            .and_then(|v| v.to_str().ok())
                            .map(str::to_owned),
                        custom_auth: None,
                        body: String::new(),
                    });
                    axum::Json(json!({"data":[{"index":0,"embedding":[1.0]}]}))
                }
            }),
        )
        .with_state(spy2);
    let (addr, _h) = spawn_router(app).await;
    let base = format!("http://{addr}/v1");
    let client =
        OpenaiCompatibleEmbeddings::new(route_for(&base, RetrievalAuthScheme::None)).unwrap();
    let _ = client
        .embed(
            EmbeddingRequest {
                model: "m".into(),
                inputs: vec!["x".into()],
                dimensions: None,
                encoding: EmbeddingEncodingFormat::Float,
                endpoint: "/embeddings".into(),
            },
            &RetrievalCredential::none(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    let caps = spy.requests.lock().await;
    assert!(caps[0].authorization.is_none());
}

#[tokio::test]
async fn custom_header_auth_exact() {
    let spy = Spy::default();
    let spy2 = spy.clone();
    let app = Router::new()
        .route(
            "/v1/embeddings",
            post(move |State(s): State<Spy>, req: Request<Body>| {
                let s = s.clone();
                async move {
                    let headers = req.headers().clone();
                    s.requests.lock().await.push(Captured {
                        authorization: headers
                            .get("authorization")
                            .and_then(|v| v.to_str().ok())
                            .map(str::to_owned),
                        custom_auth: headers
                            .get("x-custom-key")
                            .and_then(|v| v.to_str().ok())
                            .map(str::to_owned),
                        body: String::new(),
                    });
                    axum::Json(json!({"data":[{"index":0,"embedding":[1.0]}]}))
                }
            }),
        )
        .with_state(spy2);
    let (addr, _h) = spawn_router(app).await;
    let base = format!("http://{addr}/v1");
    let mut route = route_for(
        &base,
        RetrievalAuthScheme::CustomHeader {
            name: "X-Custom-Key".into(),
        },
    );
    route.extra_headers.clear();
    let client = OpenaiCompatibleEmbeddings::new(route).unwrap();
    let _ = client
        .embed(
            EmbeddingRequest {
                model: "m".into(),
                inputs: vec!["x".into()],
                dimensions: None,
                encoding: EmbeddingEncodingFormat::Float,
                endpoint: "/embeddings".into(),
            },
            &RetrievalCredential::new(Some("only-a".into())),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    let caps = spy.requests.lock().await;
    assert_eq!(caps[0].custom_auth.as_deref(), Some("only-a"));
    assert!(caps[0].authorization.is_none());
}

#[tokio::test]
async fn retry_429_then_success() {
    let hits = Arc::new(AtomicUsize::new(0));
    let hits2 = hits.clone();
    let app = Router::new().route(
        "/v1/embeddings",
        post(move || {
            let hits = hits2.clone();
            async move {
                let n = hits.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    return Response::builder()
                        .status(StatusCode::TOO_MANY_REQUESTS)
                        .header("Retry-After", "0")
                        .body(Body::from("rate"))
                        .unwrap();
                }
                (
                    StatusCode::OK,
                    axum::Json(json!({"data":[{"index":0,"embedding":[1.0,2.0]}]})),
                )
                    .into_response()
            }
        }),
    );
    let (addr, _h) = spawn_router(app).await;
    let base = format!("http://{addr}/v1");
    let client =
        OpenaiCompatibleEmbeddings::new(route_for(&base, RetrievalAuthScheme::None)).unwrap();
    let res = client
        .embed(
            EmbeddingRequest {
                model: "m".into(),
                inputs: vec!["x".into()],
                dimensions: Some(2),
                encoding: EmbeddingEncodingFormat::Float,
                endpoint: "/embeddings".into(),
            },
            &RetrievalCredential::none(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(res.vectors[0].values.len(), 2);
    assert!(hits.load(Ordering::SeqCst) >= 2);
}

#[tokio::test]
async fn no_retry_on_400() {
    let hits = Arc::new(AtomicUsize::new(0));
    let hits2 = hits.clone();
    let app = Router::new().route(
        "/v1/embeddings",
        post(move || {
            let hits = hits2.clone();
            async move {
                hits.fetch_add(1, Ordering::SeqCst);
                (StatusCode::BAD_REQUEST, "bad schema").into_response()
            }
        }),
    );
    let (addr, _h) = spawn_router(app).await;
    let base = format!("http://{addr}/v1");
    let client =
        OpenaiCompatibleEmbeddings::new(route_for(&base, RetrievalAuthScheme::None)).unwrap();
    let err = client
        .embed(
            EmbeddingRequest {
                model: "m".into(),
                inputs: vec!["x".into()],
                dimensions: None,
                encoding: EmbeddingEncodingFormat::Float,
                endpoint: "/embeddings".into(),
            },
            &RetrievalCredential::none(),
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, RetrievalError::Http { status: 400, .. }));
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn cancellation_during_request() {
    let app = Router::new().route(
        "/v1/embeddings",
        post(|| async {
            tokio::time::sleep(Duration::from_secs(30)).await;
            axum::Json(json!({"data":[{"index":0,"embedding":[1.0]}]}))
        }),
    );
    let (addr, _h) = spawn_router(app).await;
    let base = format!("http://{addr}/v1");
    let client =
        OpenaiCompatibleEmbeddings::new(route_for(&base, RetrievalAuthScheme::None)).unwrap();
    let cancel = CancellationToken::new();
    let cancel2 = cancel.clone();
    let task = tokio::spawn(async move {
        client
            .embed(
                EmbeddingRequest {
                    model: "m".into(),
                    inputs: vec!["x".into()],
                    dimensions: None,
                    encoding: EmbeddingEncodingFormat::Float,
                    endpoint: "/embeddings".into(),
                },
                &RetrievalCredential::none(),
                cancel2,
            )
            .await
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    cancel.cancel();
    let err = task.await.unwrap().unwrap_err();
    assert!(matches!(err, RetrievalError::Cancelled));
}

#[tokio::test]
async fn cross_origin_redirect_refused() {
    let app = Router::new().route(
        "/v1/embeddings",
        post(|| async {
            Response::builder()
                .status(StatusCode::FOUND)
                .header("Location", "http://evil.example/steal")
                .body(Body::empty())
                .unwrap()
        }),
    );
    let (addr, _h) = spawn_router(app).await;
    let base = format!("http://{addr}/v1");
    let client =
        OpenaiCompatibleEmbeddings::new(route_for(&base, RetrievalAuthScheme::None)).unwrap();
    let err = client
        .embed(
            EmbeddingRequest {
                model: "m".into(),
                inputs: vec!["x".into()],
                dimensions: None,
                encoding: EmbeddingEncodingFormat::Float,
                endpoint: "/embeddings".into(),
            },
            &RetrievalCredential::none(),
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, RetrievalError::RedirectPolicy(_)));
}

#[tokio::test]
async fn vllm_rerank_happy() {
    let app = Router::new().route(
        "/v1/rerank",
        post(|req: Request<Body>| async move {
            let bytes = axum::body::to_bytes(req.into_body(), 1024 * 1024)
                .await
                .unwrap();
            let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(v["query"], "q");
            assert!(v["documents"].as_array().unwrap().len() == 3);
            axum::Json(json!({
                "model": "rr",
                "results": [
                    {"index": 2, "relevance_score": 0.9},
                    {"index": 0, "score": 0.2}
                ]
            }))
        }),
    );
    let (addr, _h) = spawn_router(app).await;
    let base = format!("http://{addr}/v1");
    let mut route = route_for(&base, RetrievalAuthScheme::Bearer);
    route.purpose = RetrievalPurpose::Rerank;
    let client = VllmRerankAdapter::new(route).unwrap();
    let res = client
        .rerank(
            RerankRequest {
                model: "rr".into(),
                query: "q".into(),
                documents: vec!["a".into(), "b".into(), "c".into()],
                top_n: Some(2),
                endpoint: "/rerank".into(),
                return_documents: false,
            },
            &RetrievalCredential::new(Some("k".into())),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(res.hits.len(), 2);
    assert_eq!(res.hits[0].index, 2);
}

#[tokio::test]
async fn missing_credential_before_network() {
    let hits = Arc::new(AtomicUsize::new(0));
    let hits2 = hits.clone();
    let app = Router::new().route(
        "/v1/embeddings",
        post(move || {
            let hits = hits2.clone();
            async move {
                hits.fetch_add(1, Ordering::SeqCst);
                StatusCode::OK
            }
        }),
    );
    let (addr, _h) = spawn_router(app).await;
    let base = format!("http://{addr}/v1");
    let client =
        OpenaiCompatibleEmbeddings::new(route_for(&base, RetrievalAuthScheme::Bearer)).unwrap();
    let err = client
        .embed(
            EmbeddingRequest {
                model: "m".into(),
                inputs: vec!["x".into()],
                dimensions: None,
                encoding: EmbeddingEncodingFormat::Float,
                endpoint: "/embeddings".into(),
            },
            &RetrievalCredential::none(),
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, RetrievalError::MissingCredential));
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}

#[test]
fn debug_redaction_canaries() {
    let route = route_for("http://127.0.0.1:9/v1", RetrievalAuthScheme::Bearer);
    let dbg = format!("{route:?}");
    assert!(!dbg.contains("sk-"));
    let cred = RetrievalCredential::new(Some("sk-super-secret".into()));
    assert!(!format!("{cred:?}").contains("sk-super"));
    let err = RetrievalError::Http {
        status: 401,
        category: super::types::RetrievalErrorCategory::Authentication,
        message: "denied".into(),
        request_id: None,
        provider_id: Some("lab".into()),
    };
    assert!(!format!("{err:?}").contains("sk-"));
    let req = EmbeddingRequest {
        model: "m".into(),
        inputs: vec!["secret user text that should not dump fully".into()],
        dimensions: None,
        encoding: EmbeddingEncodingFormat::Float,
        endpoint: "/embeddings".into(),
    };
    let d = format!("{req:?}");
    assert!(d.contains("input_count"));
    assert!(!d.contains("secret user text"));
}

#[test]
fn transport_join_relative_endpoint_attacks() {
    let t = RetrievalTransport::from_route(&route_for(
        "http://127.0.0.1:9/v1",
        RetrievalAuthScheme::None,
    ))
    .unwrap();
    assert!(t.join_endpoint("https://evil").is_err());
    assert!(t.join_endpoint("//evil").is_err());
    assert!(t.join_endpoint("../x").is_err());
    assert!(t.join_endpoint("a?q=1").is_err());
}
