//! Direct Anthropic API client (`x-api-key` + pinned version header).

use std::fmt;

use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use futures_util::stream::BoxStream;
use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderValue};
use reqwest::multipart::{Form, Part};
use serde::Serialize;
use tokio_util::sync::CancellationToken;
use xai_grok_inference_types::anthropic::{
    AnthropicBetaSet, CountTokensRequest, CountTokensResponse, DeleteFileResponse, FileListPage,
    FileMetadata, FileUploadSource, ListFilesParams, ListModelsParams, ModelInfo, ModelListPage,
};
use xai_grok_inference_types::error::try_parse_stream_error;
use xai_grok_inference_types::messages::{MessageStreamEvent, MessagesRequest, MessagesResponse};

use super::error::{AnthropicClientError, AnthropicResult};
use super::headers::{AnthropicResponseMeta, build_request_headers};

/// Default Anthropic API origin (paths are `/v1/...`).
pub const DEFAULT_ANTHROPIC_BASE_URL: &str = "https://api.anthropic.com";

/// Maximum serialized request body size accepted before contacting the network.
pub const MAX_REQUEST_BYTES: usize = 32 * 1024 * 1024;

/// Effective request size limit. Tests may override via [`set_test_max_request_bytes`].
fn effective_max_request_bytes() -> usize {
    #[cfg(test)]
    {
        TEST_MAX_REQUEST_BYTES
            .with(|cell| cell.get())
            .unwrap_or(MAX_REQUEST_BYTES)
    }
    #[cfg(not(test))]
    {
        MAX_REQUEST_BYTES
    }
}

#[cfg(test)]
thread_local! {
    static TEST_MAX_REQUEST_BYTES: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

/// Override the preflight size limit for the current test thread (`None` restores default).
#[cfg(test)]
pub(super) fn set_test_max_request_bytes(limit: Option<usize>) {
    TEST_MAX_REQUEST_BYTES.with(|cell| cell.set(limit));
}

/// RAII guard that restores the default preflight limit when dropped.
#[cfg(test)]
pub(super) struct TestMaxRequestBytesGuard;

#[cfg(test)]
impl TestMaxRequestBytesGuard {
    pub fn new(limit: usize) -> Self {
        set_test_max_request_bytes(Some(limit));
        Self
    }
}

#[cfg(test)]
impl Drop for TestMaxRequestBytesGuard {
    fn drop(&mut self) {
        set_test_max_request_bytes(None);
    }
}

/// Configuration for [`AnthropicClient`].
#[derive(Clone)]
pub struct AnthropicClientConfig {
    /// API key sent as `x-api-key` (never as Bearer).
    pub api_key: String,
    /// Base URL; default [`DEFAULT_ANTHROPIC_BASE_URL`].
    pub base_url: String,
    /// Explicit beta set (empty by default).
    pub betas: AnthropicBetaSet,
    /// Optional cancellation token; when cancelled, in-flight ops return
    /// [`AnthropicClientError::Cancelled`].
    pub cancel: CancellationToken,
}

impl AnthropicClientConfig {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: DEFAULT_ANTHROPIC_BASE_URL.into(),
            betas: AnthropicBetaSet::new(),
            cancel: CancellationToken::new(),
        }
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    pub fn with_betas(mut self, betas: AnthropicBetaSet) -> Self {
        self.betas = betas;
        self
    }

    pub fn with_cancel(mut self, cancel: CancellationToken) -> Self {
        self.cancel = cancel;
        self
    }
}

impl fmt::Debug for AnthropicClientConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AnthropicClientConfig")
            .field("base_url", &self.base_url)
            .field("betas", &self.betas)
            .field("api_key", &"<redacted>")
            .finish()
    }
}

/// Outcome of a non-streaming Messages create call.
#[derive(Debug, Clone)]
pub struct AnthropicMessagesOutcome {
    pub response: MessagesResponse,
    pub meta: AnthropicResponseMeta,
}

/// Generic page wrapper preserving response metadata.
#[derive(Debug, Clone)]
pub struct AnthropicPage<T> {
    pub page: T,
    pub meta: AnthropicResponseMeta,
}

/// Repository-owned Anthropic client. Wraps `reqwest` with safe Debug omission
/// of the API key. Distinct from [`crate::InferenceClient`]'s Messages backend.
#[derive(Clone)]
pub struct AnthropicClient {
    http: reqwest::Client,
    api_key: String,
    base_url: String,
    betas: AnthropicBetaSet,
    cancel: CancellationToken,
}

impl fmt::Debug for AnthropicClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AnthropicClient")
            .field("base_url", &self.base_url)
            .field("betas", &self.betas)
            .field("api_key", &"<redacted>")
            .finish()
    }
}

impl AnthropicClient {
    pub fn new(config: AnthropicClientConfig) -> AnthropicResult<Self> {
        if config.api_key.trim().is_empty() {
            return Err(AnthropicClientError::InvalidConfig(
                "api_key must not be empty".into(),
            ));
        }
        let base_url = config.base_url.trim_end_matches('/').to_string();
        if base_url.is_empty() {
            return Err(AnthropicClientError::InvalidConfig(
                "base_url must not be empty".into(),
            ));
        }
        let http = crate::extra_ca::with_extra_root_certificates(reqwest::Client::builder())
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| AnthropicClientError::Transport(e.to_string()))?;
        Ok(Self {
            http,
            api_key: config.api_key,
            base_url,
            betas: config.betas,
            cancel: config.cancel,
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn betas(&self) -> &AnthropicBetaSet {
        &self.betas
    }

    fn url(&self, path: &str) -> String {
        let path = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{path}")
        };
        format!("{}{path}", self.base_url)
    }

    fn check_cancelled(&self) -> AnthropicResult<()> {
        if self.cancel.is_cancelled() {
            Err(AnthropicClientError::Cancelled)
        } else {
            Ok(())
        }
    }

    fn headers_for(&self, betas: &AnthropicBetaSet) -> AnthropicResult<reqwest::header::HeaderMap> {
        build_request_headers(&self.api_key, betas)
    }

    /// Reject raw payload sizes that exceed the preflight limit (no network).
    fn preflight_size(&self, size_bytes: usize) -> AnthropicResult<()> {
        let limit = effective_max_request_bytes();
        if size_bytes > limit {
            return Err(AnthropicClientError::RequestTooLarge {
                size_bytes,
                limit_bytes: limit,
            });
        }
        Ok(())
    }

    /// Serialize `body` and reject if over the size limit before any network I/O.
    fn preflight_json<T: Serialize>(&self, body: &T) -> AnthropicResult<Vec<u8>> {
        let bytes = serde_json::to_vec(body)
            .map_err(|e| AnthropicClientError::InvalidConfig(format!("serialize: {e}")))?;
        self.preflight_size(bytes.len())?;
        Ok(bytes)
    }

    async fn send_json<T: serde::de::DeserializeOwned>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<Vec<u8>>,
        betas: &AnthropicBetaSet,
    ) -> AnthropicResult<(T, AnthropicResponseMeta)> {
        self.check_cancelled()?;
        let mut headers = self.headers_for(betas)?;
        // JSON content-type only when a body is present (GET/DELETE stay bare).
        if body.is_some() {
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        }

        let mut builder = self.http.request(method, self.url(path)).headers(headers);
        if let Some(bytes) = body {
            builder = builder.body(bytes);
        }

        let response = tokio::select! {
            _ = self.cancel.cancelled() => return Err(AnthropicClientError::Cancelled),
            result = builder.send() => {
                result.map_err(|e| AnthropicClientError::Transport(e.to_string()))?
            }
        };

        let status = response.status();
        let header_map = response.headers().clone();
        let meta = AnthropicResponseMeta::from_headers(&header_map);
        let bytes = tokio::select! {
            _ = self.cancel.cancelled() => return Err(AnthropicClientError::Cancelled),
            result = response.bytes() => {
                result.map_err(|e| AnthropicClientError::Transport(e.to_string()))?
            }
        };

        if !status.is_success() {
            return Err(AnthropicClientError::from_status(
                status.as_u16(),
                bytes.as_ref(),
                meta,
            ));
        }

        let parsed = serde_json::from_slice::<T>(&bytes)
            .map_err(|e| AnthropicClientError::Decode(e.to_string()))?;
        Ok((parsed, meta))
    }

    // -------------------------------------------------------------------------
    // Messages
    // -------------------------------------------------------------------------

    /// POST `/v1/messages` (non-streaming).
    pub async fn create_message(
        &self,
        mut request: MessagesRequest,
    ) -> AnthropicResult<AnthropicMessagesOutcome> {
        request.stream = Some(false);
        let body = self.preflight_json(&request)?;
        let (response, meta) = self
            .send_json::<MessagesResponse>(
                reqwest::Method::POST,
                "/v1/messages",
                Some(body),
                &self.betas,
            )
            .await?;
        Ok(AnthropicMessagesOutcome { response, meta })
    }

    /// POST `/v1/messages` with `stream: true`. Returns typed SSE events.
    pub async fn create_message_stream(
        &self,
        mut request: MessagesRequest,
    ) -> AnthropicResult<(
        BoxStream<'static, AnthropicResult<MessageStreamEvent>>,
        AnthropicResponseMeta,
    )> {
        self.check_cancelled()?;
        request.stream = Some(true);
        let body = self.preflight_json(&request)?;
        let mut headers = self.headers_for(&self.betas)?;
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(ACCEPT, HeaderValue::from_static("text/event-stream"));

        let builder = self
            .http
            .post(self.url("/v1/messages"))
            .headers(headers)
            .body(body);

        let response = tokio::select! {
            _ = self.cancel.cancelled() => return Err(AnthropicClientError::Cancelled),
            result = builder.send() => {
                result.map_err(|e| AnthropicClientError::Transport(e.to_string()))?
            }
        };

        let status = response.status();
        let header_map = response.headers().clone();
        let meta = AnthropicResponseMeta::from_headers(&header_map);

        if !status.is_success() {
            let bytes = response
                .bytes()
                .await
                .map_err(|e| AnthropicClientError::Transport(e.to_string()))?;
            return Err(AnthropicClientError::from_status(
                status.as_u16(),
                bytes.as_ref(),
                meta,
            ));
        }

        let meta_for_stream = meta.clone();
        let cancel = self.cancel.clone();
        let byte_stream = response.bytes_stream();
        let event_stream = byte_stream.eventsource();

        let events = async_stream::stream! {
            tokio::pin!(event_stream);
            loop {
                if cancel.is_cancelled() {
                    yield Err(AnthropicClientError::Cancelled);
                    break;
                }
                let next = tokio::select! {
                    _ = cancel.cancelled() => {
                        yield Err(AnthropicClientError::Cancelled);
                        break;
                    }
                    item = event_stream.next() => item,
                };
                let Some(event_res) = next else { break };
                match event_res {
                    Ok(event) => {
                        let data = event.data;
                        if data == "[DONE]" {
                            break;
                        }
                        if let Some(stream_err) = try_parse_stream_error(&data) {
                            let (error_type, message) = match stream_err {
                                xai_grok_inference_types::InferenceError::StreamError {
                                    error_type,
                                    message,
                                    ..
                                } => (error_type, message),
                                other => ("stream_error".into(), other.to_string()),
                            };
                            yield Err(AnthropicClientError::stream_error(
                                error_type,
                                message,
                                meta_for_stream.clone(),
                            ));
                            break;
                        }
                        match serde_json::from_str::<MessageStreamEvent>(&data) {
                            Ok(MessageStreamEvent::Error { error }) => {
                                yield Err(AnthropicClientError::stream_error(
                                    error.r#type,
                                    error.message,
                                    meta_for_stream.clone(),
                                ));
                                break;
                            }
                            Ok(ev) => yield Ok(ev),
                            Err(e) => {
                                yield Err(AnthropicClientError::Decode(e.to_string()));
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        yield Err(AnthropicClientError::Transport(e.to_string()));
                        break;
                    }
                }
            }
        };

        Ok((events.boxed(), meta))
    }

    // -------------------------------------------------------------------------
    // Models
    // -------------------------------------------------------------------------

    /// GET `/v1/models` with cursor pagination.
    pub async fn list_models(
        &self,
        params: &ListModelsParams,
    ) -> AnthropicResult<AnthropicPage<ModelListPage>> {
        let mut path = "/v1/models".to_string();
        let qs = query_string(params);
        if !qs.is_empty() {
            path.push('?');
            path.push_str(&qs);
        }
        let (page, meta) = self
            .send_json::<ModelListPage>(reqwest::Method::GET, &path, None, &self.betas)
            .await?;
        Ok(AnthropicPage { page, meta })
    }

    /// GET `/v1/models/{model_id}`.
    pub async fn retrieve_model(
        &self,
        model_id: &str,
    ) -> AnthropicResult<AnthropicPage<ModelInfo>> {
        if model_id.trim().is_empty() {
            return Err(AnthropicClientError::InvalidConfig(
                "model_id must not be empty".into(),
            ));
        }
        let path = format!("/v1/models/{}", urlencoding_path(model_id));
        let (info, meta) = self
            .send_json::<ModelInfo>(reqwest::Method::GET, &path, None, &self.betas)
            .await?;
        Ok(AnthropicPage { page: info, meta })
    }

    // -------------------------------------------------------------------------
    // Count tokens
    // -------------------------------------------------------------------------

    /// POST `/v1/messages/count_tokens`.
    pub async fn count_tokens(
        &self,
        request: &CountTokensRequest,
    ) -> AnthropicResult<AnthropicPage<CountTokensResponse>> {
        let body = self.preflight_json(request)?;
        let (resp, meta) = self
            .send_json::<CountTokensResponse>(
                reqwest::Method::POST,
                "/v1/messages/count_tokens",
                Some(body),
                &self.betas,
            )
            .await?;
        Ok(AnthropicPage { page: resp, meta })
    }

    // -------------------------------------------------------------------------
    // Files beta
    // -------------------------------------------------------------------------

    fn files_betas(&self) -> AnthropicBetaSet {
        self.betas.with_files_api()
    }

    /// POST `/v1/files` multipart upload. Caller supplies in-memory bytes;
    /// this method never reads the local filesystem.
    pub async fn upload_file(
        &self,
        source: FileUploadSource,
    ) -> AnthropicResult<AnthropicPage<FileMetadata>> {
        self.check_cancelled()?;
        if source.filename.trim().is_empty() {
            return Err(AnthropicClientError::InvalidConfig(
                "filename must not be empty".into(),
            ));
        }
        self.preflight_size(source.bytes.len())?;

        let headers = self.headers_for(&self.files_betas())?;
        // reqwest sets multipart Content-Type with boundary.

        let mut part = Part::bytes(source.bytes).file_name(source.filename.clone());
        if let Some(mime) = source.mime_type.as_deref() {
            part = part
                .mime_str(mime)
                .map_err(|e| AnthropicClientError::InvalidConfig(e.to_string()))?;
        }
        let form = Form::new().part("file", part);

        let builder = self
            .http
            .post(self.url("/v1/files"))
            .headers(headers)
            .multipart(form);

        let response = tokio::select! {
            _ = self.cancel.cancelled() => return Err(AnthropicClientError::Cancelled),
            result = builder.send() => {
                result.map_err(|e| AnthropicClientError::Transport(e.to_string()))?
            }
        };

        let status = response.status();
        let meta = AnthropicResponseMeta::from_headers(response.headers());
        let bytes = response
            .bytes()
            .await
            .map_err(|e| AnthropicClientError::Transport(e.to_string()))?;
        if !status.is_success() {
            return Err(AnthropicClientError::from_status(
                status.as_u16(),
                bytes.as_ref(),
                meta,
            ));
        }
        let meta_parsed = serde_json::from_slice::<FileMetadata>(&bytes)
            .map_err(|e| AnthropicClientError::Decode(e.to_string()))?;
        Ok(AnthropicPage {
            page: meta_parsed,
            meta,
        })
    }

    /// GET `/v1/files` with cursor pagination.
    pub async fn list_files(
        &self,
        params: &ListFilesParams,
    ) -> AnthropicResult<AnthropicPage<FileListPage>> {
        let mut path = "/v1/files".to_string();
        let qs = query_string(params);
        if !qs.is_empty() {
            path.push('?');
            path.push_str(&qs);
        }
        let (page, meta) = self
            .send_json::<FileListPage>(reqwest::Method::GET, &path, None, &self.files_betas())
            .await?;
        Ok(AnthropicPage { page, meta })
    }

    /// GET `/v1/files/{file_id}` metadata.
    pub async fn retrieve_file(
        &self,
        file_id: &str,
    ) -> AnthropicResult<AnthropicPage<FileMetadata>> {
        if file_id.trim().is_empty() {
            return Err(AnthropicClientError::InvalidConfig(
                "file_id must not be empty".into(),
            ));
        }
        let path = format!("/v1/files/{}", urlencoding_path(file_id));
        let (info, meta) = self
            .send_json::<FileMetadata>(reqwest::Method::GET, &path, None, &self.files_betas())
            .await?;
        Ok(AnthropicPage { page: info, meta })
    }

    /// GET `/v1/files/{file_id}/content` — download full bytes.
    ///
    /// Bytes are returned to the caller; they are never logged by this client.
    pub async fn download_file(
        &self,
        file_id: &str,
    ) -> AnthropicResult<(Vec<u8>, AnthropicResponseMeta)> {
        self.check_cancelled()?;
        if file_id.trim().is_empty() {
            return Err(AnthropicClientError::InvalidConfig(
                "file_id must not be empty".into(),
            ));
        }
        let headers = self.headers_for(&self.files_betas())?;
        let path = format!("/v1/files/{}/content", urlencoding_path(file_id));
        let builder = self.http.get(self.url(&path)).headers(headers);

        let response = tokio::select! {
            _ = self.cancel.cancelled() => return Err(AnthropicClientError::Cancelled),
            result = builder.send() => {
                result.map_err(|e| AnthropicClientError::Transport(e.to_string()))?
            }
        };

        let status = response.status();
        let meta = AnthropicResponseMeta::from_headers(response.headers());
        let bytes = response
            .bytes()
            .await
            .map_err(|e| AnthropicClientError::Transport(e.to_string()))?;
        if !status.is_success() {
            return Err(AnthropicClientError::from_status(
                status.as_u16(),
                bytes.as_ref(),
                meta,
            ));
        }
        Ok((bytes.to_vec(), meta))
    }

    /// DELETE `/v1/files/{file_id}`.
    pub async fn delete_file(
        &self,
        file_id: &str,
    ) -> AnthropicResult<AnthropicPage<DeleteFileResponse>> {
        if file_id.trim().is_empty() {
            return Err(AnthropicClientError::InvalidConfig(
                "file_id must not be empty".into(),
            ));
        }
        let path = format!("/v1/files/{}", urlencoding_path(file_id));
        let (resp, meta) = self
            .send_json::<DeleteFileResponse>(
                reqwest::Method::DELETE,
                &path,
                None,
                &self.files_betas(),
            )
            .await?;
        Ok(AnthropicPage { page: resp, meta })
    }
}

fn query_string<T: Serialize>(params: &T) -> String {
    // Manual small query builder: only include present Option fields.
    let value = serde_json::to_value(params).unwrap_or(serde_json::Value::Null);
    let Some(obj) = value.as_object() else {
        return String::new();
    };
    let mut parts = Vec::new();
    for (k, v) in obj {
        if v.is_null() {
            continue;
        }
        let s = match v {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            other => other.to_string(),
        };
        parts.push(format!(
            "{}={}",
            encode_query_component(k),
            encode_query_component(&s)
        ));
    }
    parts.join("&")
}

fn encode_query_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn urlencoding_path(s: &str) -> String {
    encode_query_component(s)
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn config_and_client_debug_redact_api_key() {
        let cfg = AnthropicClientConfig::new("sk-ant-secret-value");
        let dbg = format!("{cfg:?}");
        assert!(!dbg.contains("sk-ant-secret-value"));
        assert!(dbg.contains("<redacted>"));

        let client = AnthropicClient::new(cfg).unwrap();
        let dbg = format!("{client:?}");
        assert!(!dbg.contains("sk-ant-secret-value"));
        assert!(dbg.contains("<redacted>"));
    }

    #[test]
    fn empty_api_key_rejected() {
        let err = AnthropicClient::new(AnthropicClientConfig::new("  ")).unwrap_err();
        assert!(matches!(err, AnthropicClientError::InvalidConfig(_)));
    }

    #[test]
    fn preflight_size_accepts_exact_limit_rejects_plus_one() {
        let client = AnthropicClient::new(AnthropicClientConfig::new("key")).unwrap();
        let _guard = TestMaxRequestBytesGuard::new(64);
        assert!(client.preflight_size(64).is_ok());
        let err = client.preflight_size(65).unwrap_err();
        match err {
            AnthropicClientError::RequestTooLarge {
                size_bytes,
                limit_bytes,
            } => {
                assert_eq!(size_bytes, 65);
                assert_eq!(limit_bytes, 64);
            }
            other => panic!("expected RequestTooLarge, got {other:?}"),
        }
    }
}
