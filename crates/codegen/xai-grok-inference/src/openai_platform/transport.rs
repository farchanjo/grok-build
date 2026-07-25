//! Shared HTTP/SSE/multipart/binary transport for platform clients.

use super::error::{
    CredentialClass, ErrorCategory, PlatformError, PlatformResult, redact_preview,
};
use super::url_policy::NormalizedBaseUrl;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
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
        match self.execute(spec).await? {
            ResponseBody::Json(v) => Ok(v),
            ResponseBody::Bytes(_) => Err(PlatformError::Decode(
                "expected JSON response, received binary".into(),
            )),
            ResponseBody::SseFrames(frames) => {
                // Last non-empty data frame that is not `[DONE]`.
                for frame in frames.iter().rev() {
                    let t = frame.trim();
                    if t.is_empty() || t == "[DONE]" {
                        continue;
                    }
                    return serde_json::from_str(t)
                        .map_err(|e| PlatformError::Decode(e.to_string()));
                }
                Err(PlatformError::Decode("empty SSE stream".into()))
            }
        }
    }

    pub async fn execute(&self, spec: HttpRequestSpec) -> PlatformResult<ResponseBody> {
        if self.cancel.is_cancelled() {
            return Err(PlatformError::Cancelled);
        }
        if spec.multipart {
            return Err(PlatformError::UnsupportedTransport(format!(
                "multipart for {} requires handwritten upload path",
                spec.operation_id
            )));
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
                }) if spec.idempotent && attempts <= self.policy.max_retries + 1 =>
                {
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
                    return Err(PlatformError::RedirectPolicy(
                        "too many redirects".into(),
                    ));
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
            let value: Value = serde_json::from_slice(&bytes)
                .map_err(|e| PlatformError::Decode(e.to_string()))?;
            return Ok(ResponseBody::Json(value));
        }
    }
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
    let mut frames = Vec::new();
    let mut current = String::new();
    for line in body.lines() {
        if line.starts_with(':') {
            continue;
        }
        if line.is_empty() {
            if !current.is_empty() {
                frames.push(std::mem::take(&mut current));
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("data:") {
            let data = rest.strip_prefix(' ').unwrap_or(rest);
            if !current.is_empty() {
                current.push('\n');
            }
            current.push_str(data);
        }
    }
    if !current.is_empty() {
        frames.push(current);
    }
    frames
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
