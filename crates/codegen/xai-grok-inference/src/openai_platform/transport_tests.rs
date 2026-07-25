//! Mock-server failure-path tests for platform transport.

#[cfg(test)]
mod tests {
    use crate::openai_platform::error::PlatformError;
    use crate::openai_platform::transport::{
        CredentialKind, HttpRequestSpec, PlatformTransport, StaticCredentials, TransportPolicy,
        split_sse_data_frames,
    };
    use axum::Router;
    use axum::body::Body;
    use axum::http::{HeaderMap, StatusCode};
    use axum::response::Response;
    use axum::routing::get;
    use std::collections::BTreeMap;
    use std::net::SocketAddr;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio_util::sync::CancellationToken;

    async fn spawn(app: Router) -> (SocketAddr, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (addr, handle)
    }

    fn transport(base: &str) -> PlatformTransport {
        PlatformTransport::new(
            base,
            "test",
            "Test",
            Arc::new(StaticCredentials {
                application: Some("app-secret".into()),
                admin: Some("admin-secret".into()),
            }),
            BTreeMap::new(),
            TransportPolicy {
                max_response_bytes: 1024,
                max_redirects: 2,
                max_retries: 0,
                ..TransportPolicy::default()
            },
            CancellationToken::new(),
        )
        .unwrap()
    }

    fn get_spec(path: &str) -> HttpRequestSpec {
        HttpRequestSpec {
            method: "GET",
            path: path.into(),
            query: BTreeMap::new(),
            body: None,
            credential: CredentialKind::Application,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "testOp",
            idempotent: true,
        }
    }

    #[tokio::test]
    async fn refuses_cross_origin_redirect_with_auth() {
        let app = Router::new().route(
            "/v1/models",
            get(|| async {
                Response::builder()
                    .status(StatusCode::FOUND)
                    .header("Location", "https://evil.example/steal")
                    .body(Body::empty())
                    .unwrap()
            }),
        );
        let (addr, _h) = spawn(app).await;
        let t = transport(&format!("http://{addr}/v1"));
        let err = t.execute_json(get_spec("/models")).await.unwrap_err();
        match err {
            PlatformError::RedirectPolicy(m) => {
                assert!(m.contains("cross-origin") || m.contains("refused"));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[tokio::test]
    async fn same_origin_redirect_bounded() {
        let hits = Arc::new(AtomicUsize::new(0));
        let hits2 = hits.clone();
        let app = Router::new().route(
            "/v1/models",
            get(move || {
                let hits = hits2.clone();
                async move {
                    let n = hits.fetch_add(1, Ordering::SeqCst);
                    if n < 5 {
                        Response::builder()
                            .status(StatusCode::FOUND)
                            .header("Location", "/v1/models")
                            .body(Body::empty())
                            .unwrap()
                    } else {
                        Response::builder()
                            .status(StatusCode::OK)
                            .header("content-type", "application/json")
                            .body(Body::from(r#"{"object":"list","data":[]}"#))
                            .unwrap()
                    }
                }
            }),
        );
        let (addr, _h) = spawn(app).await;
        let t = transport(&format!("http://{addr}/v1"));
        let err = t.execute_json(get_spec("/models")).await.unwrap_err();
        assert!(matches!(err, PlatformError::RedirectPolicy(_)));
    }

    #[tokio::test]
    async fn does_not_leak_auth_header_into_error_message() {
        let app = Router::new().route(
            "/v1/models",
            get(|headers: HeaderMap| async move {
                let auth = headers
                    .get("authorization")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_owned();
                Response::builder()
                    .status(StatusCode::UNAUTHORIZED)
                    .header("content-type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"error":{{"message":"bad {auth}"}}}}"#
                    )))
                    .unwrap()
            }),
        );
        let (addr, _h) = spawn(app).await;
        let t = transport(&format!("http://{addr}/v1"));
        let err = t.execute_json(get_spec("/models")).await.unwrap_err();
        let s = err.to_string();
        assert!(!s.contains("app-secret"));
        assert!(!s.contains("Bearer app"));
    }

    #[tokio::test]
    async fn oversized_response_fails_closed() {
        let big = "x".repeat(2048);
        let app = Router::new().route(
            "/v1/models",
            get(move || {
                let big = big.clone();
                async move {
                    Response::builder()
                        .status(StatusCode::OK)
                        .body(Body::from(big))
                        .unwrap()
                }
            }),
        );
        let (addr, _h) = spawn(app).await;
        let t = transport(&format!("http://{addr}/v1"));
        let err = t.execute_json(get_spec("/models")).await.unwrap_err();
        assert!(matches!(err, PlatformError::OversizedResponse { .. }));
    }

    #[tokio::test]
    async fn rate_limit_surfaces_retry_after() {
        let app = Router::new().route(
            "/v1/models",
            get(|| async {
                Response::builder()
                    .status(StatusCode::TOO_MANY_REQUESTS)
                    .header("Retry-After", "2")
                    .body(Body::empty())
                    .unwrap()
            }),
        );
        let (addr, _h) = spawn(app).await;
        let t = transport(&format!("http://{addr}/v1"));
        let err = t.execute_json(get_spec("/models")).await.unwrap_err();
        match err {
            PlatformError::RateLimited {
                retry_after_ms: Some(ms),
                ..
            } => assert_eq!(ms, 2000),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[tokio::test]
    async fn cancellation_aborts() {
        let app = Router::new().route(
            "/v1/models",
            get(|| async {
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                StatusCode::OK
            }),
        );
        let (addr, _h) = spawn(app).await;
        let cancel = CancellationToken::new();
        let t = PlatformTransport::new(
            &format!("http://{addr}/v1"),
            "test",
            "Test",
            Arc::new(StaticCredentials {
                application: Some("app-secret".into()),
                admin: None,
            }),
            BTreeMap::new(),
            TransportPolicy::default(),
            cancel.clone(),
        )
        .unwrap();
        cancel.cancel();
        let err = t.execute_json(get_spec("/models")).await.unwrap_err();
        assert!(matches!(err, PlatformError::Cancelled));
    }

    #[tokio::test]
    async fn malformed_json_is_decode_error() {
        let app = Router::new().route(
            "/v1/models",
            get(|| async {
                Response::builder()
                    .status(StatusCode::OK)
                    .body(Body::from("not-json"))
                    .unwrap()
            }),
        );
        let (addr, _h) = spawn(app).await;
        let t = transport(&format!("http://{addr}/v1"));
        let err = t.execute_json(get_spec("/models")).await.unwrap_err();
        assert!(matches!(err, PlatformError::Decode(_)));
    }

    #[tokio::test]
    async fn admin_credential_not_used_for_application_request() {
        let app = Router::new().route(
            "/v1/models",
            get(|headers: HeaderMap| async move {
                let auth = headers.get("authorization").unwrap().to_str().unwrap();
                assert!(auth.contains("app-secret"));
                assert!(!auth.contains("admin-secret"));
                Response::builder()
                    .status(StatusCode::OK)
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"object":"list","data":[]}"#))
                    .unwrap()
            }),
        );
        let (addr, _h) = spawn(app).await;
        let t = transport(&format!("http://{addr}/v1"));
        t.execute_json(get_spec("/models")).await.unwrap();
    }

    #[test]
    fn sse_split_drops_comments() {
        let frames = split_sse_data_frames(": comment\n\ndata: {\"a\":1}\n\n");
        assert_eq!(frames, vec![r#"{"a":1}"#]);
    }
}
