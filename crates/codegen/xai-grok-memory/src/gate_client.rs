//! HTTP transport for the Jev decisions endpoint.
//!
//! OpenRouter's `/api/alpha/decisions` is the supported surface: it takes
//! `{model, state, questions}` and answers `{answers}`. The credential comes
//! from `api_key_env`, then [`JEV_API_KEY_ENV`], then whatever the caller
//! resolved from its own store (the shell passes the `auth.json` OpenRouter
//! key). A missing credential is [`GateError::MissingCredential`] and the
//! caller falls back to the un-gated write.

use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;

use super::gate::{DecisionClient, GateConfig, GateError, JEV_API_KEY_ENV};

/// Longest response body echoed into a log line.
const BODY_SNIPPET_CHARS: usize = 200;

/// A decisions client: one `reqwest::Client`, one resolved key.
#[derive(Debug)]
pub struct JevDecisionsClient {
    endpoint: String,
    model: String,
    api_key: String,
    provider: Option<Value>,
    client: reqwest::Client,
}

impl JevDecisionsClient {
    /// Build a client from a resolved policy.
    ///
    /// Credential precedence: `config.api_key_env` → [`JEV_API_KEY_ENV`] →
    /// `stored_key`. A blank override at any step falls through to the next.
    pub fn new(config: &GateConfig, stored_key: Option<&str>) -> Result<Self, GateError> {
        let api_key = resolve_api_key(config, stored_key)?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .build()
            .map_err(|error| GateError::Request(error.to_string()))?;
        Ok(Self {
            endpoint: config.endpoint.clone(),
            model: config.model.clone(),
            api_key,
            provider: config.provider_block(),
            client,
        })
    }

    /// Endpoint this client posts to (for logs and tests).
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Ask one batch of questions about `state`; returns the `answers` object.
    pub async fn ask(&self, state: &Value, questions: &Value) -> Result<Value, GateError> {
        let mut body = serde_json::Map::new();
        body.insert("model".to_owned(), Value::String(self.model.clone()));
        body.insert("state".to_owned(), state.clone());
        body.insert("questions".to_owned(), questions.clone());
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
            .map_err(|error| GateError::Request(error.to_string()))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|error| GateError::Request(error.to_string()))?;
        if !status.is_success() {
            return Err(GateError::Http {
                status: status.as_u16(),
                body: snippet(&text),
            });
        }
        let parsed: Value = serde_json::from_str(&text)
            .map_err(|error| GateError::Malformed(format!("invalid JSON: {error}")))?;
        match parsed.get("answers") {
            Some(Value::Object(answers)) => Ok(Value::Object(answers.clone())),
            Some(other) => Err(GateError::Malformed(format!(
                "`answers` is {}, not an object",
                kind_of(other)
            ))),
            None => Err(GateError::Malformed("missing `answers`".to_owned())),
        }
    }
}

#[async_trait]
impl DecisionClient for JevDecisionsClient {
    async fn ask(&self, state: &Value, questions: &Value) -> Result<Value, GateError> {
        JevDecisionsClient::ask(self, state, questions).await
    }
}

/// First non-blank credential, in precedence order.
fn resolve_api_key(config: &GateConfig, stored_key: Option<&str>) -> Result<String, GateError> {
    let from_env = |name: &str| {
        std::env::var(name)
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    };
    config
        .api_key_env
        .as_deref()
        .and_then(from_env)
        .or_else(|| from_env(JEV_API_KEY_ENV))
        .or_else(|| {
            stored_key
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
        })
        .ok_or(GateError::MissingCredential)
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
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// One-shot HTTP server answering a fixed status and body.
    async fn spawn_server(status: u16, body: &'static str) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let mut buffer = vec![0_u8; 16 * 1024];
            let _ = stream.read(&mut buffer).await;
            let response = format!(
                "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\n\
                 Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.flush().await;
        });
        format!("http://{addr}/decisions")
    }

    /// Env var the client tests resolve their credential from; the caller has
    /// no `auth.json` to hand over.
    const TEST_KEY_ENV: &str = "GROK_JEV_GATE_TEST_KEY";

    fn config(endpoint: String) -> GateConfig {
        GateConfig {
            enabled: true,
            endpoint,
            api_key_env: Some(TEST_KEY_ENV.to_owned()),
            ..GateConfig::disabled()
        }
    }

    async fn client_for(status: u16, body: &'static str) -> JevDecisionsClient {
        let endpoint = spawn_server(status, body).await;
        unsafe { std::env::set_var(TEST_KEY_ENV, "test-key") };
        JevDecisionsClient::new(&config(endpoint), None).unwrap()
    }

    #[tokio::test]
    async fn success_returns_the_answers_object() {
        let client = client_for(200, r#"{"answers": {"worth": {"noul": 0.9}}}"#).await;
        let answers = client
            .ask(
                &json!({"candidate": "x"}),
                &json!({"worth": {"type": "noul"}}),
            )
            .await
            .unwrap();
        assert_eq!(answers["worth"]["noul"], json!(0.9));
    }

    #[tokio::test]
    async fn non_success_is_reported_with_the_body() {
        let client = client_for(400, r#"{"error": "decisions model"}"#).await;
        match client.ask(&json!({}), &json!({})).await.unwrap_err() {
            GateError::Http { status, body } => {
                assert_eq!(status, 400);
                assert!(body.contains("decisions model"), "{body}");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn malformed_body_is_reported() {
        let client = client_for(200, "not json").await;
        assert!(matches!(
            client.ask(&json!({}), &json!({})).await.unwrap_err(),
            GateError::Malformed(_)
        ));
    }

    #[tokio::test]
    async fn missing_answers_is_reported() {
        let client = client_for(200, r#"{"result": {}}"#).await;
        assert!(matches!(
            client.ask(&json!({}), &json!({})).await.unwrap_err(),
            GateError::Malformed(_)
        ));
    }

    #[tokio::test]
    async fn absent_credential_everywhere_is_reported() {
        let mut cfg = config("http://127.0.0.1:1/decisions".to_owned());
        cfg.api_key_env = Some("GROK_JEV_GATE_ABSENT_KEY".to_owned());
        assert_eq!(
            JevDecisionsClient::new(&cfg, None).unwrap_err(),
            GateError::MissingCredential
        );
    }

    #[tokio::test]
    async fn stored_key_is_used_when_the_env_is_absent() {
        let mut cfg = config("http://127.0.0.1:1/decisions".to_owned());
        cfg.api_key_env = Some("GROK_JEV_GATE_ABSENT_KEY".to_owned());
        let client = JevDecisionsClient::new(&cfg, Some("stored")).unwrap();
        assert_eq!(client.endpoint(), cfg.endpoint);
    }

    #[tokio::test]
    async fn blank_stored_key_falls_through_to_the_env() {
        unsafe { std::env::set_var(TEST_KEY_ENV, "from-env") };
        let cfg = config("http://127.0.0.1:1/decisions".to_owned());
        assert!(JevDecisionsClient::new(&cfg, Some("   ")).is_ok());
    }
}
