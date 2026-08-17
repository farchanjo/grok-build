//! Mock-server failure-path tests for platform transport.

#[cfg(test)]
mod tests {
    use crate::openai_platform::error::PlatformError;
    use crate::openai_platform::transport::{
        CredentialKind, HttpRequestSpec, MultipartFiles, PlatformTransport, StaticCredentials,
        TransportPolicy, split_sse_data_frames,
    };
    use axum::Router;
    use axum::body::Body;
    use axum::http::{HeaderMap, StatusCode};
    use axum::response::Response;
    use axum::routing::get;
    use futures_util::{SinkExt, StreamExt};
    use serde::{Deserialize, Serialize};
    use std::collections::BTreeMap;
    use std::net::SocketAddr;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
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

    #[derive(Debug, Serialize)]
    struct TestClientEvent {
        r#type: &'static str,
        text: &'static str,
    }

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    struct TestServerEvent {
        r#type: String,
        text: String,
    }

    #[tokio::test]
    async fn multipart_realtime_call_preserves_fields_and_sdp_response() {
        let app = Router::new().route(
            "/v1/realtime/calls",
            axum::routing::post(|headers: HeaderMap, body: axum::body::Bytes| async move {
                assert_eq!(
                    headers
                        .get("authorization")
                        .and_then(|value| value.to_str().ok()),
                    Some("Bearer app-secret")
                );
                let content_type = headers
                    .get("content-type")
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or_default();
                assert!(content_type.starts_with("multipart/form-data; boundary="));
                let body = String::from_utf8(body.to_vec()).unwrap();
                let body_lower = body.to_ascii_lowercase();
                assert!(body.contains("name=\"sdp\""));
                assert!(body_lower.contains("content-type: application/sdp"));
                assert!(body.contains("v=0"));
                assert!(body.contains("name=\"session\""));
                assert!(body_lower.contains("content-type: application/json"));
                assert!(body.contains("gpt-realtime"));
                Response::builder()
                    .status(StatusCode::CREATED)
                    .header("content-type", "application/sdp")
                    .body(Body::from("v=0\r\na=setup:active\r\n"))
                    .unwrap()
            }),
        );
        let (addr, _handle) = spawn(app).await;
        let t = transport(&format!("http://{addr}/v1"));
        let value = t
            .execute_multipart(
                HttpRequestSpec {
                    method: "POST",
                    path: "/realtime/calls".into(),
                    query: BTreeMap::new(),
                    body: Some(serde_json::json!({
                        "sdp": "v=0",
                        "session": {"type": "realtime", "model": "gpt-realtime"}
                    })),
                    credential: CredentialKind::Application,
                    expect_sse: false,
                    expect_binary: false,
                    multipart: true,
                    operation_id: "create-realtime-call",
                    idempotent: false,
                },
                MultipartFiles::new()
                    .content_type("sdp", "application/sdp")
                    .content_type("session", "application/json"),
            )
            .await
            .unwrap();
        assert_eq!(value, serde_json::json!("v=0\r\na=setup:active\r\n"));
    }

    #[tokio::test]
    async fn realtime_session_exchanges_typed_events_and_authenticates() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_hdr_async(
                stream,
                |request: &tokio_tungstenite::tungstenite::handshake::server::Request,
                 response: tokio_tungstenite::tungstenite::handshake::server::Response| {
                    assert_eq!(request.uri().path(), "/v1/realtime");
                    assert_eq!(request.uri().query(), Some("model=gpt-realtime"));
                    assert_eq!(
                        request
                            .headers()
                            .get("authorization")
                            .and_then(|value| value.to_str().ok()),
                        Some("Bearer app-secret")
                    );
                    Ok(response)
                },
            )
            .await
            .unwrap();
            let message = socket.next().await.unwrap().unwrap();
            let text = message.into_text().unwrap();
            let event: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(event["type"], "response.create");
            socket
                .send(tokio_tungstenite::tungstenite::Message::Text(
                    r#"{"type":"response.text.delta","text":"hello"}"#.into(),
                ))
                .await
                .unwrap();
            socket.close(None).await.unwrap();
        });
        let t = transport(&format!("http://{addr}/v1"));
        let mut query = BTreeMap::new();
        query.insert("model".into(), "gpt-realtime".into());
        let mut session = t
            .connect_realtime(HttpRequestSpec {
                method: "GET",
                path: "/realtime".into(),
                query,
                body: None,
                credential: CredentialKind::Application,
                expect_sse: false,
                expect_binary: false,
                multipart: false,
                operation_id: "connectRealtime",
                idempotent: false,
            })
            .await
            .unwrap();
        session
            .send(&TestClientEvent {
                r#type: "response.create",
                text: "start",
            })
            .await
            .unwrap();
        assert_eq!(
            session.recv::<TestServerEvent>().await.unwrap(),
            Some(TestServerEvent {
                r#type: "response.text.delta".into(),
                text: "hello".into(),
            })
        );
        assert!(session.recv::<TestServerEvent>().await.unwrap().is_none());
        server.await.unwrap();
    }

    #[tokio::test]
    async fn realtime_session_rejects_redirect_without_forwarding_credentials() {
        let redirect_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let redirect_addr = redirect_listener.local_addr().unwrap();
        let redirect_target = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let target_addr = redirect_target.local_addr().unwrap();
        let redirect_server = tokio::spawn(async move {
            let (mut stream, _) = redirect_listener.accept().await.unwrap();
            let mut request = vec![0_u8; 4096];
            let read = stream.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..read]).to_ascii_lowercase();
            assert!(request.contains("authorization: bearer app-secret"));
            let response = format!(
                "HTTP/1.1 302 Found\r\nLocation: ws://{target_addr}/steal\r\nContent-Length: 0\r\n\r\n"
            );
            stream.write_all(response.as_bytes()).await.unwrap();
        });
        let t = transport(&format!("http://{redirect_addr}/v1"));
        let error = match t
            .connect_realtime(HttpRequestSpec {
                method: "GET",
                path: "/realtime".into(),
                query: BTreeMap::new(),
                body: None,
                credential: CredentialKind::Application,
                expect_sse: false,
                expect_binary: false,
                multipart: false,
                operation_id: "connectRealtime",
                idempotent: false,
            })
            .await
        {
            Err(error) => error,
            Ok(_) => panic!("redirecting Realtime upgrade unexpectedly succeeded"),
        };
        assert!(matches!(
            error,
            PlatformError::Http {
                status: 302,
                provider_id: Some(ref provider_id),
                ..
            } if provider_id == "test"
        ));
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(100),
                redirect_target.accept()
            )
            .await
            .is_err(),
            "redirect target must not receive a credential-bearing connection"
        );
        redirect_server.await.unwrap();
    }

    #[tokio::test]
    async fn realtime_session_rejects_oversized_typed_event() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
            let _ = socket.next().await;
        });
        let t = transport(&format!("http://{addr}/v1"));
        let mut session = t
            .connect_realtime(HttpRequestSpec {
                method: "GET",
                path: "/realtime".into(),
                query: BTreeMap::new(),
                body: None,
                credential: CredentialKind::Application,
                expect_sse: false,
                expect_binary: false,
                multipart: false,
                operation_id: "connectRealtime",
                idempotent: false,
            })
            .await
            .unwrap();
        let oversized = serde_json::json!({"type": "session.update", "data": "x".repeat(2048)});
        assert!(matches!(
            session.send(&oversized).await.unwrap_err(),
            PlatformError::OversizedResponse { limit_bytes: 1024 }
        ));
        session.close().await.unwrap();
        server.await.unwrap();
    }

    #[test]
    fn sse_split_drops_comments() {
        let frames = split_sse_data_frames(": comment\n\ndata: {\"a\":1}\n\n");
        assert_eq!(frames, vec![r#"{"a":1}"#]);
    }

    #[tokio::test]
    async fn binary_sink_is_owner_only_durable() {
        let (addr, server) = spawn(Router::new().route(
            "/v1/audio",
            get(|| async {
                (
                    [(axum::http::header::CONTENT_TYPE, "audio/mpeg")],
                    vec![7u8, 8, 9, 10],
                )
            }),
        ))
        .await;
        let transport = transport(&format!("http://{addr}/v1"));
        let dir = std::env::temp_dir().join(format!(
            "grok-bin-sink-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let sink = dir.join("out.bin");
        let mut spec = get_spec("/audio");
        spec.expect_binary = true;
        let (bytes, _) = transport.execute_binary(spec, Some(&sink)).await.unwrap();
        assert_eq!(bytes, vec![7, 8, 9, 10]);
        assert_eq!(std::fs::read(&sink).unwrap(), vec![7, 8, 9, 10]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&sink).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "binary sink must be owner-only");
        }
        let _ = std::fs::remove_dir_all(dir);
        server.abort();
    }

    #[tokio::test]
    async fn binary_sink_refuses_symlink() {
        let (addr, server) =
            spawn(Router::new().route("/v1/audio", get(|| async { vec![1u8, 2, 3] }))).await;
        let transport = transport(&format!("http://{addr}/v1"));
        let dir = std::env::temp_dir().join(format!(
            "grok-bin-symlink-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let real = dir.join("real.bin");
        std::fs::write(&real, b"keep").unwrap();
        let link = dir.join("link.bin");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&real, &link).unwrap();
            let mut spec = get_spec("/audio");
            spec.expect_binary = true;
            let err = transport
                .execute_binary(spec, Some(&link))
                .await
                .unwrap_err();
            assert!(
                matches!(err, PlatformError::Transport(_)),
                "expected transport error for symlink sink, got {err:?}"
            );
            assert_eq!(std::fs::read(&real).unwrap(), b"keep");
        }
        let _ = std::fs::remove_dir_all(dir);
        server.abort();
    }
}
