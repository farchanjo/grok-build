//! HTTP client for the Jev decisions endpoint.
//!
//! The wire takes `{model, state, questions}` plus an optional `provider`
//! block and answers `{answers}`. Two transports serve it — see
//! [`super::transport`]; which one is live, and therefore the endpoint, the
//! model string and the credential chain, comes from the resolved policy and
//! never from the session route, so the Jev call works even when
//! `[compaction] models` points somewhere else entirely.
//!
//! The body carries a **derived** session key (`{session_id}:jev`), never the
//! session's own key: the Jev payload shape differs from a chat turn's, and
//! sharing the key would share `bootstrap_room` with a prefix family Jev does
//! not belong to.

use std::path::Path;
use std::time::Duration;

use serde_json::Value;

use super::transport::{CredentialSource, JevTransport, resolve_credential};
use super::types::{PruneError, ResolvedJevPrune, derived_session_key};

/// Environment variable consulted by the OpenRouter chain.
pub const JEV_API_KEY_ENV: &str = "GROK_JEV_API_KEY";

/// Longest response body echoed into a log line.
const BODY_SNIPPET_CHARS: usize = 200;

/// A reusable Jev decisions client: one `reqwest::Client`, one resolved key.
#[derive(Debug)]
pub struct JevClient {
    transport: JevTransport,
    endpoint: String,
    model: String,
    api_key: String,
    /// Where [`Self::api_key`] came from. Never the key itself; surfaced in
    /// `status` and in a 401 so a two-transport setup stays debuggable.
    credential_source: CredentialSource,
    /// Derived session key (`{session_id}:jev`), or `None` when the caller has
    /// no session. Sent as the body's `session_id`.
    session_key: Option<String>,
    provider: Option<Value>,
    client: reqwest::Client,
}

impl JevClient {
    /// Build a client from a resolved policy.
    ///
    /// The credential chain belongs to the transport: native resolves
    /// `TYPESAFE_API_KEY` / `TYPESAFE_AI_FABRICIO_KEY` from the environment or
    /// the key file, OpenRouter resolves `GROK_JEV_API_KEY` then the
    /// `auth.json` OpenRouter scope. A missing credential is
    /// [`PruneError::MissingCredential`] naming the whole chain; the caller
    /// falls back to the unpruned view.
    ///
    /// `session_id` is the session's own id; the client derives the Jev lane
    /// key from it. `None` sends no `session_id`.
    pub fn new(
        cfg: &ResolvedJevPrune,
        grok_home: &Path,
        session_id: Option<&str>,
    ) -> Result<Self, PruneError> {
        let credential = resolve_credential(cfg.transport, cfg.api_key_env.as_deref(), grok_home)?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(cfg.timeout_ms))
            .build()
            .map_err(|error| PruneError::Request(error.to_string()))?;
        Ok(Self {
            transport: cfg.transport,
            endpoint: cfg.endpoint.clone(),
            model: cfg.model.clone(),
            api_key: credential.value,
            credential_source: credential.source,
            session_key: session_id
                .filter(|id| !id.is_empty())
                .map(derived_session_key),
            provider: cfg.provider_block(),
            client,
        })
    }

    /// Endpoint this client posts to (for logs and tests).
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Transport this client speaks (for logs and tests).
    pub fn transport(&self) -> JevTransport {
        self.transport
    }

    /// Where the credential came from — `env:`, `file:` or `auth.json:`.
    pub fn credential_source(&self) -> &CredentialSource {
        &self.credential_source
    }

    /// Derived session key carried in the body, when the caller had a session.
    pub fn session_key(&self) -> Option<&str> {
        self.session_key.as_deref()
    }

    /// Ask one batch of questions about `state`; returns the `answers` object.
    pub async fn ask(&self, state: &Value, questions: &Value) -> Result<Value, PruneError> {
        let mut body = serde_json::Map::new();
        body.insert("model".to_owned(), Value::String(self.model.clone()));
        body.insert("state".to_owned(), state.clone());
        body.insert("questions".to_owned(), questions.clone());
        // The derived lane key. `bootstrap_room` is left to the transports
        // that define it (SGLang/vLLM on the chat path); neither Jev endpoint
        // is documented to act on it, so inventing one here would be a guess.
        if let Some(key) = &self.session_key {
            body.insert("session_id".to_owned(), Value::String(key.clone()));
        }
        if let Some(provider) = &self.provider {
            body.insert("provider".to_owned(), provider.clone());
        }
        let response = self
            .client
            .post(&self.endpoint)
            .bearer_auth(&self.api_key)
            .json(&Value::Object(body))
            .send()
            .await
            .map_err(|error| PruneError::Request(error.to_string()))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|error| PruneError::Request(error.to_string()))?;
        if !status.is_success() {
            // A 401/403 is a credential problem, so the message names the
            // chain link that produced the key — without it, a two-transport
            // setup is undebuggable.
            let body = if matches!(status.as_u16(), 401 | 403) {
                format!(
                    "{} (credential from {}; transport {})",
                    snippet(&text),
                    self.credential_source.label(),
                    self.transport.as_str(),
                )
            } else {
                snippet(&text)
            };
            return Err(PruneError::Http {
                status: status.as_u16(),
                body,
            });
        }
        let parsed: Value = serde_json::from_str(&text)
            .map_err(|error| PruneError::Malformed(format!("invalid JSON: {error}")))?;
        match parsed.get("answers") {
            Some(Value::Object(answers)) => Ok(Value::Object(answers.clone())),
            Some(other) => Err(PruneError::Malformed(format!(
                "`answers` is {}, not an object",
                kind_of(other)
            ))),
            None => Err(PruneError::Malformed("missing `answers`".to_owned())),
        }
    }
}

fn snippet(text: &str) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= BODY_SNIPPET_CHARS {
        collapsed
    } else {
        collapsed.chars().take(BODY_SNIPPET_CHARS).collect()
    }
}

fn kind_of(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn cfg(endpoint: String) -> ResolvedJevPrune {
        ResolvedJevPrune {
            enabled: true,
            endpoint,
            ..ResolvedJevPrune::disabled()
        }
    }

    /// Minimal `axum` server returning one canned response.
    async fn spawn_server(handler: axum::Router) -> (String, tokio::sync::oneshot::Sender<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        tokio::spawn(async move {
            axum::serve(listener, handler)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .unwrap();
        });
        (format!("http://{addr}"), shutdown_tx)
    }

    /// Env var the client tests resolve their credential from; the temp grok
    /// home has no `auth.json`.
    const TEST_KEY_ENV: &str = "GROK_JEV_CLIENT_TEST_KEY";

    async fn client_for(handler: axum::Router) -> (JevClient, tokio::sync::oneshot::Sender<()>) {
        let (base, shutdown) = spawn_server(handler).await;
        let dir = tempfile::tempdir().unwrap();
        let mut resolved = cfg(format!("{base}/decisions"));
        resolved.api_key_env = Some(TEST_KEY_ENV.to_owned());
        unsafe { std::env::set_var(TEST_KEY_ENV, "test-key") };
        let client = JevClient::new(&resolved, dir.path(), None).unwrap();
        (client, shutdown)
    }

    #[tokio::test]
    async fn success_returns_the_answers_object() {
        let app = axum::Router::new().route(
            "/decisions",
            axum::routing::post(|| async {
                axum::Json(json!({ "answers": { "call_t1": { "noul": 0.9 } } }))
            }),
        );
        let (client, shutdown) = client_for(app).await;
        let answers = client
            .ask(
                &json!({"history": []}),
                &json!({"call_t1": {"type": "noul"}}),
            )
            .await
            .unwrap();
        assert_eq!(answers["call_t1"]["noul"], json!(0.9));
        let _ = shutdown.send(());
    }

    #[tokio::test]
    async fn request_carries_model_state_questions_and_provider_block() {
        use std::sync::{Arc, Mutex};
        let seen: Arc<Mutex<Option<Value>>> = Arc::new(Mutex::new(None));
        let captured = Arc::clone(&seen);
        let app = axum::Router::new().route(
            "/decisions",
            axum::routing::post(move |axum::Json(body): axum::Json<Value>| {
                let captured = Arc::clone(&captured);
                async move {
                    *captured.lock().unwrap() = Some(body);
                    axum::Json(json!({ "answers": {} }))
                }
            }),
        );
        let (base, shutdown) = spawn_server(app).await;
        let dir = tempfile::tempdir().unwrap();
        let mut resolved = cfg(format!("{base}/decisions"));
        resolved.api_key_env = Some(TEST_KEY_ENV.to_owned());
        resolved.model = "~typesafe/jev-latest".to_owned();
        resolved.zdr = Some(true);
        resolved.data_collection = Some("deny".to_owned());
        resolved.require_parameters = Some(true);
        unsafe { std::env::set_var(TEST_KEY_ENV, "test-key") };
        let client = JevClient::new(&resolved, dir.path(), None).unwrap();
        client
            .ask(
                &json!({"history": []}),
                &json!({"call_t1": {"type": "noul"}}),
            )
            .await
            .unwrap();
        let body = seen.lock().unwrap().clone().expect("body captured");
        assert_eq!(body["model"], json!("~typesafe/jev-latest"));
        assert_eq!(body["state"]["history"], json!([]));
        assert!(body["questions"]["call_t1"].is_object());
        assert_eq!(body["provider"]["zdr"], json!(true));
        assert_eq!(body["provider"]["data_collection"], json!("deny"));
        assert_eq!(body["provider"]["require_parameters"], json!(true));
        let _ = shutdown.send(());
    }

    /// The derived lane key rides the body; the session's own key never does.
    #[tokio::test]
    async fn body_carries_the_derived_session_key() {
        use std::sync::{Arc, Mutex};
        let seen: Arc<Mutex<Option<Value>>> = Arc::new(Mutex::new(None));
        let captured = Arc::clone(&seen);
        let app = axum::Router::new().route(
            "/decisions",
            axum::routing::post(move |axum::Json(body): axum::Json<Value>| {
                let captured = Arc::clone(&captured);
                async move {
                    *captured.lock().unwrap() = Some(body);
                    axum::Json(json!({ "answers": {} }))
                }
            }),
        );
        let (base, shutdown) = spawn_server(app).await;
        let dir = tempfile::tempdir().unwrap();
        let mut resolved = cfg(format!("{base}/decisions"));
        resolved.api_key_env = Some(TEST_KEY_ENV.to_owned());
        unsafe { std::env::set_var(TEST_KEY_ENV, "test-key") };
        let client = JevClient::new(&resolved, dir.path(), Some("01a0b71b")).unwrap();
        assert_eq!(client.session_key(), Some("01a0b71b:jev"));
        client.ask(&json!({}), &json!({})).await.unwrap();
        let body = seen.lock().unwrap().clone().expect("body captured");
        assert_eq!(body["session_id"], json!("01a0b71b:jev"));
        // Jev defines no room of its own; the chat path keeps its own.
        assert!(body.get("bootstrap_room").is_none());
        let _ = shutdown.send(());
    }

    /// No session ⇒ no `session_id`, rather than an empty one.
    #[tokio::test]
    async fn no_session_means_no_session_id_field() {
        use std::sync::{Arc, Mutex};
        let seen: Arc<Mutex<Option<Value>>> = Arc::new(Mutex::new(None));
        let captured = Arc::clone(&seen);
        let app = axum::Router::new().route(
            "/decisions",
            axum::routing::post(move |axum::Json(body): axum::Json<Value>| {
                let captured = Arc::clone(&captured);
                async move {
                    *captured.lock().unwrap() = Some(body);
                    axum::Json(json!({ "answers": {} }))
                }
            }),
        );
        let (base, shutdown) = spawn_server(app).await;
        let dir = tempfile::tempdir().unwrap();
        let mut resolved = cfg(format!("{base}/decisions"));
        resolved.api_key_env = Some(TEST_KEY_ENV.to_owned());
        unsafe { std::env::set_var(TEST_KEY_ENV, "test-key") };
        let client = JevClient::new(&resolved, dir.path(), None).unwrap();
        client.ask(&json!({}), &json!({})).await.unwrap();
        let body = seen.lock().unwrap().clone().expect("body captured");
        assert!(body.get("session_id").is_none(), "{body}");
        let _ = shutdown.send(());
    }

    /// The native transport never sends the OpenRouter-only `provider` block.
    #[tokio::test]
    async fn native_transport_omits_the_provider_block() {
        use std::sync::{Arc, Mutex};
        let seen: Arc<Mutex<Option<Value>>> = Arc::new(Mutex::new(None));
        let captured = Arc::clone(&seen);
        let app = axum::Router::new().route(
            "/decisions",
            axum::routing::post(move |axum::Json(body): axum::Json<Value>| {
                let captured = Arc::clone(&captured);
                async move {
                    *captured.lock().unwrap() = Some(body);
                    axum::Json(json!({ "answers": {} }))
                }
            }),
        );
        let (base, shutdown) = spawn_server(app).await;
        let dir = tempfile::tempdir().unwrap();
        let mut resolved = cfg(format!("{base}/decisions"));
        resolved.transport = super::super::transport::JevTransport::Native;
        resolved.model = super::super::transport::JevTransport::Native
            .model()
            .to_owned();
        resolved.zdr = Some(true);
        resolved.api_key_env = Some(TEST_KEY_ENV.to_owned());
        unsafe { std::env::set_var(TEST_KEY_ENV, "test-key") };
        let client = JevClient::new(&resolved, dir.path(), None).unwrap();
        client.ask(&json!({}), &json!({})).await.unwrap();
        let body = seen.lock().unwrap().clone().expect("body captured");
        assert_eq!(body["model"], json!("jev-latest"));
        assert!(body.get("provider").is_none(), "{body}");
        let _ = shutdown.send(());
    }

    /// A 401 names the chain link that produced the key, so a two-transport
    /// setup is diagnosable from the log line alone.
    #[tokio::test]
    async fn unauthorized_names_the_credential_source() {
        let app = axum::Router::new().route(
            "/decisions",
            axum::routing::post(|| async {
                (
                    reqwest::StatusCode::UNAUTHORIZED,
                    axum::Json(json!({ "error": "invalid api key" })),
                )
            }),
        );
        let (client, shutdown) = client_for(app).await;
        let error = client.ask(&json!({}), &json!({})).await.unwrap_err();
        match error {
            PruneError::Http { status, body } => {
                assert_eq!(status, 401);
                assert!(body.contains(TEST_KEY_ENV), "{body}");
                assert!(body.contains("transport openrouter"), "{body}");
            }
            other => panic!("unexpected error: {other:?}"),
        }
        let _ = shutdown.send(());
    }

    #[tokio::test]
    async fn four_hundred_is_reported_with_the_body() {
        let app = axum::Router::new().route(
            "/decisions",
            axum::routing::post(|| async {
                (
                    reqwest::StatusCode::BAD_REQUEST,
                    axum::Json(json!({
                        "error": "is a decisions model and cannot be used with the chat/completions endpoint"
                    })),
                )
            }),
        );
        let (client, shutdown) = client_for(app).await;
        let error = client.ask(&json!({}), &json!({})).await.unwrap_err();
        match error {
            PruneError::Http { status, body } => {
                assert_eq!(status, 400);
                assert!(body.contains("decisions model"), "{body}");
            }
            other => panic!("unexpected error: {other:?}"),
        }
        let _ = shutdown.send(());
    }

    #[tokio::test]
    async fn not_found_is_reported() {
        let app = axum::Router::new();
        let (client, shutdown) = client_for(app).await;
        let error = client.ask(&json!({}), &json!({})).await.unwrap_err();
        assert!(matches!(error, PruneError::Http { status: 404, .. }));
        let _ = shutdown.send(());
    }

    #[tokio::test]
    async fn malformed_body_is_reported() {
        let app = axum::Router::new().route(
            "/decisions",
            axum::routing::post(|| async { "not json at all" }),
        );
        let (client, shutdown) = client_for(app).await;
        let error = client.ask(&json!({}), &json!({})).await.unwrap_err();
        assert!(matches!(error, PruneError::Malformed(_)));
        let _ = shutdown.send(());
    }

    #[tokio::test]
    async fn answers_must_be_an_object() {
        let app = axum::Router::new().route(
            "/decisions",
            axum::routing::post(|| async { axum::Json(json!({ "answers": 3 })) }),
        );
        let (client, shutdown) = client_for(app).await;
        let error = client.ask(&json!({}), &json!({})).await.unwrap_err();
        assert!(matches!(error, PruneError::Malformed(_)));
        let _ = shutdown.send(());
    }

    #[tokio::test]
    async fn timeout_is_a_request_error() {
        let app = axum::Router::new().route(
            "/decisions",
            axum::routing::post(|| async {
                tokio::time::sleep(std::time::Duration::from_millis(400)).await;
                axum::Json(json!({ "answers": {} }))
            }),
        );
        let (base, shutdown) = spawn_server(app).await;
        let dir = tempfile::tempdir().unwrap();
        let mut resolved = cfg(format!("{base}/decisions"));
        resolved.api_key_env = Some(TEST_KEY_ENV.to_owned());
        resolved.timeout_ms = 50;
        unsafe { std::env::set_var(TEST_KEY_ENV, "test-key") };
        let client = JevClient::new(&resolved, dir.path(), None).unwrap();
        let error = client.ask(&json!({}), &json!({})).await.unwrap_err();
        assert!(matches!(error, PruneError::Request(_)), "{error:?}");
        let _ = shutdown.send(());
    }

    #[tokio::test]
    async fn missing_credential_is_reported() {
        // A blank env override removes the only guaranteed source; the temp
        // grok home has no auth.json.
        let dir = tempfile::tempdir().unwrap();
        let mut resolved = cfg("http://127.0.0.1:1/decisions".to_owned());
        resolved.api_key_env = Some("GROK_JEV_TEST_ABSENT_KEY".to_owned());
        let error = JevClient::new(&resolved, dir.path(), None).unwrap_err();
        match error {
            PruneError::MissingCredential { chain } => {
                assert!(chain.contains("GROK_JEV_TEST_ABSENT_KEY"), "{chain}")
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn api_key_env_wins_over_the_profile() {
        // SAFETY: the test runtime is single-threaded per test process.
        unsafe { std::env::set_var("GROK_JEV_TEST_PRESENT_KEY", "from-env") };
        let dir = tempfile::tempdir().unwrap();
        let mut resolved = cfg("http://127.0.0.1:1/decisions".to_owned());
        resolved.api_key_env = Some("GROK_JEV_TEST_PRESENT_KEY".to_owned());
        let client = JevClient::new(&resolved, dir.path(), None).unwrap();
        assert_eq!(client.api_key, "from-env");
        unsafe { std::env::remove_var("GROK_JEV_TEST_PRESENT_KEY") };
    }
}
