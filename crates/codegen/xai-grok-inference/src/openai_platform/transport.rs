//! Shared HTTP/SSE/multipart/binary transport for platform clients.

use super::error::{CredentialClass, ErrorCategory, PlatformError, PlatformResult, redact_preview};
use super::url_policy::NormalizedBaseUrl;
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::{HeaderName, HeaderValue};
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_util::sync::CancellationToken;

/// Which credential store entry the transport may inject.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialKind {
    Application,
    Admin,
}

impl From<CredentialKind> for CredentialClass {
    fn from(value: CredentialKind) -> Self {
        match value {
            CredentialKind::Application => CredentialClass::Application,
            CredentialKind::Admin => CredentialClass::Admin,
        }
    }
}

/// Bounds and retry policy for platform traffic.
#[derive(Debug, Clone)]
pub struct TransportPolicy {
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
    pub max_response_bytes: usize,
    pub max_error_preview_chars: usize,
    pub max_redirects: usize,
    pub max_pagination_pages: usize,
    pub max_retries: u32,
    pub user_agent: String,
}

impl Default for TransportPolicy {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(120),
            max_response_bytes: 32 * 1024 * 1024,
            max_error_preview_chars: 512,
            max_redirects: 3,
            max_pagination_pages: 100,
            max_retries: 2,
            user_agent: format!("grok-openai-platform/{}", env!("CARGO_PKG_VERSION")),
        }
    }
}

/// Fully described HTTP request (path is relative to the normalized base).
#[derive(Debug, Clone)]
pub struct HttpRequestSpec {
    pub method: &'static str,
    pub path: String,
    pub query: BTreeMap<String, String>,
    pub body: Option<Value>,
    pub credential: CredentialKind,
    pub expect_sse: bool,
    pub expect_binary: bool,
    pub multipart: bool,
    pub operation_id: &'static str,
    pub idempotent: bool,
}

/// Response body variants.
#[derive(Debug, Clone)]
pub enum ResponseBody {
    Json(Value),
    Bytes(Vec<u8>),
    /// SSE frames already split into data payloads (comments stripped).
    SseFrames(Vec<String>),
}

/// One Server-Sent Event with optional event name and raw data payload.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SseEvent {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    pub data: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

type RealtimeSocket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

/// Connected, bounded Realtime WebSocket session with typed JSON events.
pub struct RealtimeSession {
    socket: RealtimeSocket,
    max_event_bytes: usize,
    cancel: CancellationToken,
}

impl RealtimeSession {
    /// Send a pinned-schema OpenAI Realtime client event.
    pub async fn send_client_event(
        &mut self,
        event: &super::realtime::RealtimeClientEvent,
    ) -> PlatformResult<()> {
        self.send(event).await
    }

    /// Receive a pinned-schema OpenAI Realtime server event.
    pub async fn recv_server_event(
        &mut self,
    ) -> PlatformResult<Option<super::realtime::RealtimeServerEvent>> {
        self.recv().await
    }

    /// Send one typed client event as a bounded JSON text frame.
    pub async fn send<T: Serialize>(&mut self, event: &T) -> PlatformResult<()> {
        let payload = serde_json::to_string(event)
            .map_err(|error| PlatformError::InvalidRequest(error.to_string()))?;
        if payload.len() > self.max_event_bytes {
            return Err(PlatformError::OversizedResponse {
                limit_bytes: self.max_event_bytes,
            });
        }
        tokio::select! {
            _ = self.cancel.cancelled() => Err(PlatformError::Cancelled),
            result = self.socket.send(Message::Text(payload.into())) => {
                result.map_err(|error| PlatformError::Transport(error.to_string()))
            }
        }
    }

    /// Receive and deserialize the next typed server event.
    ///
    /// Ping/Pong and binary frames are handled or rejected internally. A
    /// graceful close returns `Ok(None)`.
    pub async fn recv<T: DeserializeOwned>(&mut self) -> PlatformResult<Option<T>> {
        loop {
            let frame = tokio::select! {
                _ = self.cancel.cancelled() => return Err(PlatformError::Cancelled),
                frame = self.socket.next() => frame,
            };
            match frame {
                Some(Ok(Message::Text(text))) => {
                    if text.len() > self.max_event_bytes {
                        return Err(PlatformError::OversizedResponse {
                            limit_bytes: self.max_event_bytes,
                        });
                    }
                    return serde_json::from_str(&text)
                        .map(Some)
                        .map_err(|error| PlatformError::Decode(error.to_string()));
                }
                Some(Ok(Message::Binary(bytes))) => {
                    if bytes.len() > self.max_event_bytes {
                        return Err(PlatformError::OversizedResponse {
                            limit_bytes: self.max_event_bytes,
                        });
                    }
                    return Err(PlatformError::Decode(
                        "expected Realtime JSON text event, received binary frame".into(),
                    ));
                }
                Some(Ok(Message::Ping(payload))) => {
                    tokio::select! {
                        _ = self.cancel.cancelled() => return Err(PlatformError::Cancelled),
                        result = self.socket.send(Message::Pong(payload)) => {
                            result.map_err(|error| PlatformError::Transport(error.to_string()))?;
                        }
                    }
                }
                Some(Ok(Message::Pong(_))) | Some(Ok(Message::Frame(_))) => {}
                Some(Ok(Message::Close(_))) | None => return Ok(None),
                Some(Err(error)) => {
                    return Err(PlatformError::Transport(error.to_string()));
                }
            }
        }
    }

    /// Close the Realtime session cleanly.
    pub async fn close(mut self) -> PlatformResult<()> {
        tokio::select! {
            _ = self.cancel.cancelled() => Err(PlatformError::Cancelled),
            result = self.socket.close(None) => {
                result.map_err(|error| PlatformError::Transport(error.to_string()))
            }
        }
    }
}

/// Multipart form file parts for upload operations.
#[derive(Debug, Clone, Default)]
pub struct MultipartFiles {
    /// Field name → filesystem path to stream.
    pub files: Vec<(String, std::path::PathBuf)>,
    /// Additional text fields (field name → value).
    pub text_fields: Vec<(String, String)>,
    /// Optional per-field MIME types derived from the operation schema.
    pub content_types: BTreeMap<String, String>,
}

impl MultipartFiles {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn file(mut self, field: impl Into<String>, path: impl Into<std::path::PathBuf>) -> Self {
        self.files.push((field.into(), path.into()));
        self
    }

    pub fn text(mut self, field: impl Into<String>, value: impl Into<String>) -> Self {
        self.text_fields.push((field.into(), value.into()));
        self
    }

    pub fn content_type(mut self, field: impl Into<String>, mime: impl Into<String>) -> Self {
        self.content_types.insert(field.into(), mime.into());
        self
    }
}

/// Resolves a bearer token for a credential kind. Never logs the value.
pub trait CredentialResolver: Send + Sync {
    fn resolve(&self, kind: CredentialKind) -> PlatformResult<Option<String>>;
}

/// Static bearer pair used by tests and simple CLI wiring.
#[derive(Debug, Clone, Default)]
pub struct StaticCredentials {
    pub application: Option<String>,
    pub admin: Option<String>,
}

impl CredentialResolver for StaticCredentials {
    fn resolve(&self, kind: CredentialKind) -> PlatformResult<Option<String>> {
        Ok(match kind {
            CredentialKind::Application => self.application.clone(),
            CredentialKind::Admin => self.admin.clone(),
        })
    }
}

/// Extra headers configured on the provider (never include Authorization).
pub type ExtraHeaders = BTreeMap<String, String>;

/// Shared transport handle for application or admin clients.
#[derive(Clone)]
pub struct PlatformTransport {
    pub(crate) base: NormalizedBaseUrl,
    pub(crate) provider_id: String,
    pub(crate) provider_display_name: String,
    pub(crate) credentials: Arc<dyn CredentialResolver>,
    pub(crate) extra_headers: ExtraHeaders,
    pub(crate) policy: TransportPolicy,
    pub(crate) http: reqwest::Client,
    pub(crate) cancel: CancellationToken,
}

impl PlatformTransport {
    pub fn new(
        base_url: &str,
        provider_id: impl Into<String>,
        provider_display_name: impl Into<String>,
        credentials: Arc<dyn CredentialResolver>,
        extra_headers: ExtraHeaders,
        policy: TransportPolicy,
        cancel: CancellationToken,
    ) -> PlatformResult<Self> {
        let base = NormalizedBaseUrl::parse(base_url)?;
        validate_extra_headers(&extra_headers)?;
        let http = reqwest::Client::builder()
            .connect_timeout(policy.connect_timeout)
            .timeout(policy.request_timeout)
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(policy.user_agent.clone())
            .build()
            .map_err(|e| PlatformError::Transport(e.to_string()))?;
        Ok(Self {
            base,
            provider_id: provider_id.into(),
            provider_display_name: provider_display_name.into(),
            credentials,
            extra_headers,
            policy,
            http,
            cancel,
        })
    }

    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    pub fn provider_display_name(&self) -> &str {
        &self.provider_display_name
    }

    pub fn base(&self) -> &NormalizedBaseUrl {
        &self.base
    }

    pub fn policy(&self) -> &TransportPolicy {
        &self.policy
    }

    pub fn with_cancel(mut self, cancel: CancellationToken) -> Self {
        self.cancel = cancel;
        self
    }

    /// Execute a JSON request with redirect, retry, and size bounds.
    pub async fn execute_json(&self, spec: HttpRequestSpec) -> PlatformResult<Value> {
        let mut spec = spec;
        spec.expect_sse = false;
        spec.expect_binary = false;
        spec.multipart = false;
        match self.execute(spec).await? {
            ResponseBody::Json(v) => Ok(v),
            ResponseBody::Bytes(_) => Err(PlatformError::Decode(
                "expected JSON response, received binary".into(),
            )),
            ResponseBody::SseFrames(_) => Err(PlatformError::Decode(
                "expected JSON response, received SSE stream".into(),
            )),
        }
    }

    /// Execute an SSE request and preserve **every** event frame (including
    /// comments-stripped data frames). Does not collapse to the last frame.
    pub async fn execute_sse(&self, spec: HttpRequestSpec) -> PlatformResult<Vec<SseEvent>> {
        let mut spec = spec;
        spec.expect_sse = true;
        spec.expect_binary = false;
        spec.multipart = false;
        match self.execute(spec).await? {
            ResponseBody::SseFrames(frames) => {
                // execute_once already split data lines; rebuild structured events.
                Ok(frames
                    .into_iter()
                    .map(|data| SseEvent {
                        event: None,
                        data,
                        id: None,
                    })
                    .collect())
            }
            ResponseBody::Json(v) => Ok(vec![SseEvent {
                event: None,
                data: v.to_string(),
                id: None,
            }]),
            ResponseBody::Bytes(b) => {
                let text =
                    String::from_utf8(b).map_err(|e| PlatformError::Decode(e.to_string()))?;
                Ok(parse_sse_events(&text))
            }
        }
    }

    /// Multipart form upload with streaming file handles and typed text fields.
    pub async fn execute_multipart(
        &self,
        spec: HttpRequestSpec,
        files: MultipartFiles,
    ) -> PlatformResult<Value> {
        if self.cancel.is_cancelled() {
            return Err(PlatformError::Cancelled);
        }
        let token = self
            .credentials
            .resolve(spec.credential)?
            .filter(|t| !t.trim().is_empty())
            .ok_or_else(|| PlatformError::MissingCredential(spec.credential.into()))?;

        let mut url = self.base.join_path(&spec.path)?;
        {
            let mut pairs = url.query_pairs_mut();
            for (k, v) in &spec.query {
                pairs.append_pair(k, v);
            }
        }

        let mut form = reqwest::multipart::Form::new();
        // JSON body fields flattened into form text parts when present.
        if let Some(Value::Object(map)) = &spec.body {
            for (field, value) in map {
                let text = match value {
                    Value::String(text) => text.clone(),
                    other => other.to_string(),
                };
                let part = multipart_text_part(field, text, &files.content_types)?;
                form = form.part(field.clone(), part);
            }
        }
        for (field, value) in &files.text_fields {
            let part = multipart_text_part(field, value.clone(), &files.content_types)?;
            form = form.part(field.clone(), part);
        }
        for (field, path) in &files.files {
            let file_name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("upload.bin")
                .to_owned();
            let bytes = tokio::fs::read(path).await.map_err(|e| {
                PlatformError::InvalidRequest(format!(
                    "read multipart file {}: {e}",
                    path.display()
                ))
            })?;
            if bytes.len() > self.policy.max_response_bytes {
                return Err(PlatformError::OversizedResponse {
                    limit_bytes: self.policy.max_response_bytes,
                });
            }
            let part = reqwest::multipart::Part::bytes(bytes).file_name(file_name);
            let part = multipart_part_content_type(field, part, &files.content_types)?;
            form = form.part(field.clone(), part);
        }

        let mut builder = match spec.method {
            "POST" => self.http.post(url),
            "PUT" => self.http.put(url),
            "PATCH" => self.http.patch(url),
            other => {
                return Err(PlatformError::InvalidRequest(format!(
                    "multipart unsupported for method {other}"
                )));
            }
        };
        builder = builder
            .header("Authorization", format!("Bearer {token}"))
            .header("OpenAI-Request-Id", uuid::Uuid::new_v4().to_string())
            .multipart(form);
        for (k, v) in &self.extra_headers {
            builder = builder.header(k.as_str(), v.as_str());
        }

        let response = tokio::select! {
            _ = self.cancel.cancelled() => return Err(PlatformError::Cancelled),
            res = builder.send() => res.map_err(|e| PlatformError::Transport(e.to_string()))?,
        };
        let status = response.status();
        let request_id = response
            .headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let bytes = response
            .bytes()
            .await
            .map_err(|e| PlatformError::Transport(e.to_string()))?;
        if bytes.len() > self.policy.max_response_bytes {
            return Err(PlatformError::OversizedResponse {
                limit_bytes: self.policy.max_response_bytes,
            });
        }
        if !status.is_success() {
            let preview = redact_preview(
                std::str::from_utf8(&bytes).unwrap_or(""),
                self.policy.max_error_preview_chars,
            );
            return Err(PlatformError::Http {
                status: status.as_u16(),
                category: ErrorCategory::from_status(status.as_u16()),
                message: extract_error_message(&preview).unwrap_or(preview),
                request_id,
                operation_id: Some(spec.operation_id.to_string()),
                provider_id: Some(self.provider_id.clone()),
            });
        }
        if bytes.is_empty() {
            return Ok(Value::Object(Default::default()));
        }
        if let Ok(value) = serde_json::from_slice(&bytes) {
            return Ok(value);
        }
        String::from_utf8(bytes.to_vec())
            .map(Value::String)
            .map_err(|error| PlatformError::Decode(error.to_string()))
    }

    /// Bounded/streamed binary download. When `sink` is set, writes to the file
    /// and returns the bytes read (still bounded by policy).
    pub async fn execute_binary(
        &self,
        spec: HttpRequestSpec,
        sink: Option<&std::path::Path>,
    ) -> PlatformResult<(Vec<u8>, Option<String>)> {
        let mut spec = spec;
        spec.expect_binary = true;
        spec.expect_sse = false;
        spec.multipart = false;
        let (bytes, content_type) = match self.execute(spec).await? {
            ResponseBody::Bytes(bytes) => (bytes, None),
            ResponseBody::Json(v) => (
                serde_json::to_vec(&v).map_err(|e| PlatformError::Decode(e.to_string()))?,
                Some("application/json".to_owned()),
            ),
            ResponseBody::SseFrames(_) => {
                return Err(PlatformError::Decode(
                    "expected binary response, received SSE".into(),
                ));
            }
        };
        if let Some(path) = sink {
            tokio::fs::write(path, &bytes)
                .await
                .map_err(|e| PlatformError::Transport(format!("write binary sink: {e}")))?;
        }
        Ok((bytes, content_type))
    }

    /// Open a bounded Realtime WebSocket and return a typed event session.
    ///
    /// WebSocket redirects are disabled by tungstenite. The request is built
    /// only from the normalized configured origin, so credentials can never be
    /// forwarded to an unrelated host.
    pub async fn connect_realtime(&self, spec: HttpRequestSpec) -> PlatformResult<RealtimeSession> {
        if self.cancel.is_cancelled() {
            return Err(PlatformError::Cancelled);
        }
        let token = self
            .credentials
            .resolve(spec.credential)?
            .filter(|token| !token.trim().is_empty())
            .ok_or_else(|| PlatformError::MissingCredential(spec.credential.into()))?;
        let mut url = self.base.join_path(&spec.path)?;
        {
            let mut pairs = url.query_pairs_mut();
            for (key, value) in &spec.query {
                pairs.append_pair(key, value);
            }
        }
        match url.scheme() {
            "https" => url.set_scheme("wss").map_err(|()| {
                PlatformError::InvalidUrl("failed to select secure WebSocket scheme".into())
            })?,
            "http" => url.set_scheme("ws").map_err(|()| {
                PlatformError::InvalidUrl("failed to select WebSocket scheme".into())
            })?,
            scheme => {
                return Err(PlatformError::InvalidUrl(format!(
                    "unsupported Realtime base scheme {scheme}"
                )));
            }
        }
        let mut request = url
            .as_str()
            .into_client_request()
            .map_err(|error| PlatformError::InvalidUrl(error.to_string()))?;
        request.headers_mut().insert(
            "Authorization",
            format!("Bearer {token}")
                .parse()
                .map_err(|error| PlatformError::InvalidRequest(format!("auth header: {error}")))?,
        );
        request.headers_mut().insert(
            "OpenAI-Request-Id",
            uuid::Uuid::new_v4().to_string().parse().map_err(|error| {
                PlatformError::InvalidRequest(format!("request id header: {error}"))
            })?,
        );
        for (name, value) in &self.extra_headers {
            let header_name = HeaderName::from_bytes(name.as_bytes()).map_err(|error| {
                PlatformError::InvalidRequest(format!("header name `{name}`: {error}"))
            })?;
            let header_value = HeaderValue::try_from(value.as_str()).map_err(|error| {
                PlatformError::InvalidRequest(format!("header `{name}`: {error}"))
            })?;
            request.headers_mut().insert(header_name, header_value);
        }
        let limit = self.policy.max_response_bytes;
        let config = WebSocketConfig::default()
            .max_message_size(Some(limit))
            .max_frame_size(Some(limit));
        let connect = tokio_tungstenite::connect_async_with_config(request, Some(config), false);
        let (socket, _) = tokio::select! {
            _ = self.cancel.cancelled() => return Err(PlatformError::Cancelled),
            result = tokio::time::timeout(self.policy.connect_timeout, connect) => {
                result
                    .map_err(|_| PlatformError::Timeout {
                        operation_id: Some(spec.operation_id.to_owned()),
                    })?
                    .map_err(|error| map_websocket_error(error, &spec, &self.provider_id))?
            }
        };
        Ok(RealtimeSession {
            socket,
            max_event_bytes: limit,
            cancel: self.cancel.clone(),
        })
    }

    pub async fn execute(&self, spec: HttpRequestSpec) -> PlatformResult<ResponseBody> {
        if self.cancel.is_cancelled() {
            return Err(PlatformError::Cancelled);
        }
        let mut attempts = 0u32;
        loop {
            attempts += 1;
            match self.execute_once(&spec).await {
                Ok(body) => return Ok(body),
                Err(PlatformError::RateLimited {
                    retry_after_ms,
                    request_id,
                    operation_id,
                }) if spec.idempotent && attempts <= self.policy.max_retries + 1 => {
                    let sleep_ms = retry_after_ms.unwrap_or(250 * u64::from(attempts));
                    tokio::select! {
                        _ = self.cancel.cancelled() => return Err(PlatformError::Cancelled),
                        _ = tokio::time::sleep(Duration::from_millis(sleep_ms.min(5_000))) => {}
                    }
                    let _ = (request_id, operation_id);
                    continue;
                }
                Err(PlatformError::Http {
                    status,
                    category: ErrorCategory::Server,
                    ..
                }) if spec.idempotent && attempts <= self.policy.max_retries + 1 => {
                    let _ = status;
                    tokio::select! {
                        _ = self.cancel.cancelled() => return Err(PlatformError::Cancelled),
                        _ = tokio::time::sleep(Duration::from_millis(200 * u64::from(attempts))) => {}
                    }
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
    }

    async fn execute_once(&self, spec: &HttpRequestSpec) -> PlatformResult<ResponseBody> {
        if self.cancel.is_cancelled() {
            return Err(PlatformError::Cancelled);
        }
        let mut url = self.base.join_path(&spec.path)?;
        {
            let mut pairs = url.query_pairs_mut();
            for (k, v) in &spec.query {
                pairs.append_pair(k, v);
            }
        }
        let token = self
            .credentials
            .resolve(spec.credential)?
            .filter(|t| !t.trim().is_empty());
        if token.is_none() {
            return Err(PlatformError::MissingCredential(spec.credential.into()));
        }
        let token = token.expect("checked");

        let mut current = url;
        let mut redirects = 0usize;
        loop {
            if self.cancel.is_cancelled() {
                return Err(PlatformError::Cancelled);
            }
            let mut builder = match spec.method {
                "GET" => self.http.get(current.clone()),
                "POST" => self.http.post(current.clone()),
                "PUT" => self.http.put(current.clone()),
                "PATCH" => self.http.patch(current.clone()),
                "DELETE" => self.http.delete(current.clone()),
                "HEAD" => self.http.head(current.clone()),
                other => {
                    return Err(PlatformError::InvalidRequest(format!(
                        "unsupported method {other}"
                    )));
                }
            };
            builder = builder.header("Authorization", format!("Bearer {token}"));
            builder = builder.header("OpenAI-Request-Id", uuid::Uuid::new_v4().to_string());
            for (k, v) in &self.extra_headers {
                builder = builder.header(k.as_str(), v.as_str());
            }
            if let Some(body) = &spec.body {
                builder = builder
                    .header("Content-Type", "application/json")
                    .json(body);
            }
            if spec.expect_sse {
                builder = builder.header("Accept", "text/event-stream");
            }

            let response = tokio::select! {
                _ = self.cancel.cancelled() => return Err(PlatformError::Cancelled),
                res = builder.send() => res.map_err(|e| {
                    if e.is_timeout() {
                        PlatformError::Timeout {
                            operation_id: Some(spec.operation_id.to_string()),
                        }
                    } else {
                        PlatformError::Transport(e.to_string())
                    }
                })?,
            };

            let status = response.status();
            if status.is_redirection() {
                redirects += 1;
                if redirects > self.policy.max_redirects {
                    return Err(PlatformError::RedirectPolicy("too many redirects".into()));
                }
                let loc = response
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|v| v.to_str().ok())
                    .ok_or_else(|| {
                        PlatformError::RedirectPolicy("redirect missing Location".into())
                    })?;
                let next = current
                    .join(loc)
                    .map_err(|e| PlatformError::RedirectPolicy(e.to_string()))?;
                if !self.base.same_origin(&next) {
                    return Err(PlatformError::RedirectPolicy(format!(
                        "cross-origin redirect to {} refused (credentials not forwarded)",
                        next.host_str().unwrap_or("unknown")
                    )));
                }
                current = next;
                continue;
            }

            let request_id = response
                .headers()
                .get("x-request-id")
                .or_else(|| response.headers().get("openai-request-id"))
                .or_else(|| response.headers().get("x-openai-request-id"))
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);
            let retry_after_ms = parse_retry_after(response.headers());

            if status.as_u16() == 429 {
                return Err(PlatformError::RateLimited {
                    retry_after_ms,
                    request_id,
                    operation_id: Some(spec.operation_id.to_string()),
                });
            }

            let bytes = response
                .bytes()
                .await
                .map_err(|e| PlatformError::Transport(e.to_string()))?;
            if bytes.len() > self.policy.max_response_bytes {
                return Err(PlatformError::OversizedResponse {
                    limit_bytes: self.policy.max_response_bytes,
                });
            }

            if !status.is_success() {
                let preview = redact_preview(
                    std::str::from_utf8(&bytes).unwrap_or(""),
                    self.policy.max_error_preview_chars,
                );
                let category = ErrorCategory::from_status(status.as_u16());
                let message = extract_error_message(&preview).unwrap_or(preview);
                return Err(PlatformError::Http {
                    status: status.as_u16(),
                    category,
                    message,
                    request_id,
                    operation_id: Some(spec.operation_id.to_string()),
                    provider_id: Some(self.provider_id.clone()),
                });
            }

            if spec.expect_binary {
                return Ok(ResponseBody::Bytes(bytes.to_vec()));
            }
            if spec.expect_sse {
                let text = String::from_utf8(bytes.to_vec())
                    .map_err(|e| PlatformError::Decode(e.to_string()))?;
                return Ok(ResponseBody::SseFrames(split_sse_data_frames(&text)));
            }
            if bytes.is_empty() {
                return Ok(ResponseBody::Json(Value::Object(Default::default())));
            }
            let value: Value =
                serde_json::from_slice(&bytes).map_err(|e| PlatformError::Decode(e.to_string()))?;
            return Ok(ResponseBody::Json(value));
        }
    }
}

fn multipart_text_part(
    field: &str,
    value: String,
    content_types: &BTreeMap<String, String>,
) -> PlatformResult<reqwest::multipart::Part> {
    multipart_part_content_type(field, reqwest::multipart::Part::text(value), content_types)
}

fn multipart_part_content_type(
    field: &str,
    part: reqwest::multipart::Part,
    content_types: &BTreeMap<String, String>,
) -> PlatformResult<reqwest::multipart::Part> {
    match content_types.get(field) {
        Some(content_type) => part.mime_str(content_type).map_err(|error| {
            PlatformError::InvalidRequest(format!(
                "multipart field `{field}` content type: {error}"
            ))
        }),
        None => Ok(part),
    }
}

fn map_websocket_error(
    error: tokio_tungstenite::tungstenite::Error,
    spec: &HttpRequestSpec,
    provider_id: &str,
) -> PlatformError {
    if let tokio_tungstenite::tungstenite::Error::Http(response) = &error {
        let status = response.status().as_u16();
        return PlatformError::Http {
            status,
            category: ErrorCategory::from_status(status),
            message: "Realtime WebSocket upgrade rejected".into(),
            request_id: response
                .headers()
                .get("x-request-id")
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned),
            operation_id: Some(spec.operation_id.to_owned()),
            provider_id: Some(provider_id.to_owned()),
        };
    }
    PlatformError::Transport(error.to_string())
}

fn validate_extra_headers(headers: &ExtraHeaders) -> PlatformResult<()> {
    for (k, v) in headers {
        let lower = k.to_ascii_lowercase();
        if lower == "authorization" || lower == "cookie" || lower == "proxy-authorization" {
            return Err(PlatformError::InvalidRequest(format!(
                "refusing to set restricted header `{k}` via extra_headers"
            )));
        }
        if v.chars().any(|c| c == '\r' || c == '\n') {
            return Err(PlatformError::InvalidRequest(format!(
                "header `{k}` contains invalid newline"
            )));
        }
        // Never allow first-party x-grok headers on generic platform clients.
        if lower.starts_with("x-grok-") {
            return Err(PlatformError::InvalidRequest(format!(
                "refusing first-party header `{k}` on third-party platform client"
            )));
        }
    }
    Ok(())
}

fn parse_retry_after(headers: &reqwest::header::HeaderMap) -> Option<u64> {
    let raw = headers.get(reqwest::header::RETRY_AFTER)?.to_str().ok()?;
    if let Ok(secs) = raw.parse::<u64>() {
        return Some(secs.saturating_mul(1000));
    }
    None
}

fn extract_error_message(preview: &str) -> Option<String> {
    let v: Value = serde_json::from_str(preview).ok()?;
    v.get("error")
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str())
        .map(|s| redact_preview(s, 256))
}

/// Split an SSE body into data payloads, dropping comments and empty frames.
pub fn split_sse_data_frames(body: &str) -> Vec<String> {
    parse_sse_events(body).into_iter().map(|e| e.data).collect()
}

/// Parse a full SSE body into structured events, preserving every non-comment frame.
pub fn parse_sse_events(body: &str) -> Vec<SseEvent> {
    let mut events = Vec::new();
    let mut event_name: Option<String> = None;
    let mut data_lines: Vec<String> = Vec::new();
    let mut id: Option<String> = None;
    for line in body.lines() {
        if line.starts_with(':') {
            continue;
        }
        if line.is_empty() {
            if !data_lines.is_empty() || event_name.is_some() {
                events.push(SseEvent {
                    event: event_name.take(),
                    data: data_lines.join("\n"),
                    id: id.take(),
                });
                data_lines.clear();
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("event:") {
            event_name = Some(rest.strip_prefix(' ').unwrap_or(rest).to_owned());
        } else if let Some(rest) = line.strip_prefix("data:") {
            data_lines.push(rest.strip_prefix(' ').unwrap_or(rest).to_owned());
        } else if let Some(rest) = line.strip_prefix("id:") {
            id = Some(rest.strip_prefix(' ').unwrap_or(rest).to_owned());
        }
    }
    if !data_lines.is_empty() || event_name.is_some() {
        events.push(SseEvent {
            event: event_name,
            data: data_lines.join("\n"),
            id,
        });
    }
    events
}

/// Walk a paginated list with loop protection.
pub async fn paginate<F, Fut, T>(
    policy: &TransportPolicy,
    mut fetch_page: F,
) -> PlatformResult<Vec<T>>
where
    F: FnMut(Option<String>) -> Fut,
    Fut: std::future::Future<Output = PlatformResult<(Vec<T>, Option<String>, bool)>>,
{
    let mut out = Vec::new();
    let mut after = None;
    for _ in 0..policy.max_pagination_pages {
        let (page, next, has_more) = fetch_page(after).await?;
        out.extend(page);
        if !has_more {
            return Ok(out);
        }
        after = next;
        if after.is_none() {
            return Ok(out);
        }
    }
    Err(PlatformError::PaginationLimit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_authorization_extra_header() {
        let mut headers = BTreeMap::new();
        headers.insert("Authorization".into(), "Bearer x".into());
        assert!(validate_extra_headers(&headers).is_err());
    }

    #[test]
    fn rejects_x_grok_headers() {
        let mut headers = BTreeMap::new();
        headers.insert("x-grok-session".into(), "1".into());
        assert!(validate_extra_headers(&headers).is_err());
    }

    #[test]
    fn splits_sse_comments_and_frames() {
        let body = ": keep-alive\n\ndata: {\"a\":1}\n\ndata: [DONE]\n\n";
        let frames = split_sse_data_frames(body);
        assert_eq!(frames, vec!["{\"a\":1}".to_string(), "[DONE]".to_string()]);
    }
}
