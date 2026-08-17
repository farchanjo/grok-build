//! Bounded same-origin HTTP transport for retrieval POSTs.
//!
//! Reuses platform URL policy and transport defaults without unsafe reqwest
//! redirect following. Supports bearer / x-api-key / custom-header / none auth.
//! Retrieval POSTs are retry-safe: bounded retries for 429 (Retry-After), 5xx,
//! and transient transport only. Total deadline bounds all attempts.

use std::str::FromStr;
use std::time::{Duration, Instant};

use serde_json::Value;
use tokio_util::sync::CancellationToken;

use super::types::{
    DEFAULT_CONNECT_TIMEOUT, DEFAULT_MAX_ERROR_PREVIEW_CHARS, DEFAULT_MAX_REDIRECTS,
    DEFAULT_MAX_RESPONSE_BYTES, DEFAULT_MAX_RETRIES, DEFAULT_REQUEST_TIMEOUT,
    DEFAULT_TOTAL_DEADLINE, RetrievalAuthScheme, RetrievalError, RetrievalErrorCategory,
    RetrievalResult, RetrievalRouteContext, normalize_endpoint_path, redact_error_preview,
    validate_relative_endpoint_path,
};
use crate::openai_platform::url_policy::NormalizedBaseUrl;

/// Short-lived application credential material. Never logged or Debug-printed.
pub struct RetrievalCredential {
    value: Option<String>,
}

impl RetrievalCredential {
    pub fn none() -> Self {
        Self { value: None }
    }

    pub fn new(value: Option<String>) -> Self {
        Self {
            value: value.filter(|s| !s.trim().is_empty()),
        }
    }

    pub fn is_present(&self) -> bool {
        self.value.is_some()
    }

    /// Borrow the credential value for wire injection. Callers must not log it.
    pub fn as_str(&self) -> Option<&str> {
        self.value.as_deref()
    }
}

impl std::fmt::Debug for RetrievalCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetrievalCredential")
            .field("present", &self.value.is_some())
            .finish()
    }
}

/// Policy bounds for one retrieval transport handle.
#[derive(Debug, Clone)]
pub struct RetrievalTransportPolicy {
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
    pub total_deadline: Duration,
    pub max_response_bytes: usize,
    pub max_error_preview_chars: usize,
    pub max_redirects: usize,
    pub max_retries: u32,
    pub user_agent: String,
}

impl Default for RetrievalTransportPolicy {
    fn default() -> Self {
        Self {
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
            total_deadline: DEFAULT_TOTAL_DEADLINE,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
            max_error_preview_chars: DEFAULT_MAX_ERROR_PREVIEW_CHARS,
            max_redirects: DEFAULT_MAX_REDIRECTS,
            max_retries: DEFAULT_MAX_RETRIES,
            user_agent: format!("grok-retrieval/{}", env!("CARGO_PKG_VERSION")),
        }
    }
}

impl RetrievalTransportPolicy {
    pub fn from_route(route: &RetrievalRouteContext) -> Self {
        Self {
            connect_timeout: route.connect_timeout,
            request_timeout: route.request_timeout,
            total_deadline: route.total_deadline,
            max_response_bytes: route.max_response_bytes,
            max_error_preview_chars: DEFAULT_MAX_ERROR_PREVIEW_CHARS,
            max_redirects: route.max_redirects,
            max_retries: route.max_retries,
            user_agent: format!("grok-retrieval/{}", env!("CARGO_PKG_VERSION")),
        }
    }
}

/// HTTP transport bound to one exact route origin and auth scheme.
#[derive(Clone)]
pub struct RetrievalTransport {
    base: NormalizedBaseUrl,
    provider_id: String,
    auth_scheme: RetrievalAuthScheme,
    extra_headers: Vec<(String, String)>,
    organization: Option<String>,
    project: Option<String>,
    policy: RetrievalTransportPolicy,
    http: reqwest::Client,
}

impl std::fmt::Debug for RetrievalTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetrievalTransport")
            .field("provider_id", &self.provider_id)
            .field("auth_scheme", &self.auth_scheme)
            .field("origin", &self.base.origin)
            .field("path_prefix", &self.base.path_prefix)
            .field("extra_headers_count", &self.extra_headers.len())
            .field("organization", &self.organization)
            .field("project", &self.project)
            .field("policy", &self.policy)
            .finish()
    }
}

impl RetrievalTransport {
    pub fn from_route(route: &RetrievalRouteContext) -> RetrievalResult<Self> {
        let base = NormalizedBaseUrl::parse(&route.base_url)
            .map_err(|e| RetrievalError::InvalidUrl(e.to_string()))?;
        validate_extra_headers(&route.extra_headers)?;
        // Custom header name must also pass restricted-name checks.
        if let RetrievalAuthScheme::CustomHeader { name } = &route.auth_scheme {
            validate_header_name(name)?;
        }
        let policy = RetrievalTransportPolicy::from_route(route);
        let http = reqwest::Client::builder()
            .connect_timeout(policy.connect_timeout)
            .timeout(policy.request_timeout)
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(policy.user_agent.clone())
            .build()
            .map_err(|e| RetrievalError::Transport(e.to_string()))?;
        Ok(Self {
            base,
            provider_id: route.provider_instance_id.clone(),
            auth_scheme: route.auth_scheme.clone(),
            extra_headers: route.extra_headers.clone(),
            organization: route.organization.clone(),
            project: route.project.clone(),
            policy,
            http,
        })
    }

    pub fn base(&self) -> &NormalizedBaseUrl {
        &self.base
    }

    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    pub fn policy(&self) -> &RetrievalTransportPolicy {
        &self.policy
    }

    /// Join a relative endpoint onto the configured base (same-origin only).
    pub fn join_endpoint(&self, relative: &str) -> RetrievalResult<reqwest::Url> {
        validate_relative_endpoint_path(relative)?;
        let path = normalize_endpoint_path(relative);
        self.base
            .join_path(&path)
            .map_err(|e| RetrievalError::InvalidUrl(e.to_string()))
    }

    /// Execute a JSON POST with bounded retries, same-origin redirects, and
    /// total deadline. `credential` is never logged.
    pub async fn post_json(
        &self,
        path: &str,
        body: &Value,
        credential: &RetrievalCredential,
        cancel: &CancellationToken,
        operation_id: &'static str,
    ) -> RetrievalResult<Value> {
        let _ = operation_id;
        if cancel.is_cancelled() {
            return Err(RetrievalError::Cancelled);
        }
        // Capability/auth scheme already resolved; missing credential fails
        // before network when a scheme requires one.
        match &self.auth_scheme {
            RetrievalAuthScheme::None => {}
            RetrievalAuthScheme::Bearer
            | RetrievalAuthScheme::XApiKey
            | RetrievalAuthScheme::CustomHeader { .. } => {
                if !credential.is_present() {
                    return Err(RetrievalError::MissingCredential);
                }
            }
        }

        let started = Instant::now();
        let mut attempts = 0u32;
        loop {
            attempts += 1;
            if started.elapsed() >= self.policy.total_deadline {
                return Err(RetrievalError::DeadlineExceeded);
            }
            if cancel.is_cancelled() {
                return Err(RetrievalError::Cancelled);
            }

            let remaining = self.policy.total_deadline.saturating_sub(started.elapsed());
            let attempt_timeout = self.policy.request_timeout.min(remaining);

            match self
                .post_json_once(path, body, credential, cancel, attempt_timeout)
                .await
            {
                Ok(v) => return Ok(v),
                // Total attempts = 1 + max_retries. Retry only while attempts
                // already spent is still strictly less than that budget.
                Err(e) if e.is_retryable() && attempts < 1 + self.policy.max_retries => {
                    let sleep_ms = match &e {
                        RetrievalError::RateLimited {
                            retry_after_ms: Some(ms),
                        } => (*ms).min(5_000),
                        RetrievalError::RateLimited { .. } => 250 * u64::from(attempts),
                        RetrievalError::Http {
                            category: RetrievalErrorCategory::Server,
                            ..
                        } => 200 * u64::from(attempts),
                        RetrievalError::Transport(_) | RetrievalError::Timeout => {
                            200 * u64::from(attempts)
                        }
                        _ => 200 * u64::from(attempts),
                    };
                    let sleep_ms = sleep_ms.min(5_000);
                    let sleep_dur = Duration::from_millis(sleep_ms);
                    if started.elapsed() + sleep_dur >= self.policy.total_deadline {
                        return Err(RetrievalError::DeadlineExceeded);
                    }
                    tokio::select! {
                        _ = cancel.cancelled() => return Err(RetrievalError::Cancelled),
                        _ = tokio::time::sleep(sleep_dur) => {}
                    }
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
    }

    async fn post_json_once(
        &self,
        path: &str,
        body: &Value,
        credential: &RetrievalCredential,
        cancel: &CancellationToken,
        attempt_timeout: Duration,
    ) -> RetrievalResult<Value> {
        let mut current = self.join_endpoint(path)?;
        let mut redirects = 0usize;
        loop {
            if cancel.is_cancelled() {
                return Err(RetrievalError::Cancelled);
            }
            let mut builder = self.http.post(current.clone());
            // Extra headers first, then force Content-Type/Accept/auth/org/project
            // so extras cannot override typed owners.
            for (k, v) in &self.extra_headers {
                builder = builder.header(k.as_str(), v.as_str());
            }
            builder = apply_auth(builder, &self.auth_scheme, credential)?;
            builder = builder
                .header("Content-Type", "application/json")
                .header("Accept", "application/json")
                .json(body)
                .timeout(attempt_timeout);
            if let Some(org) = &self.organization {
                builder = builder.header("OpenAI-Organization", org.as_str());
            }
            if let Some(proj) = &self.project {
                builder = builder.header("OpenAI-Project", proj.as_str());
            }

            let response = tokio::select! {
                _ = cancel.cancelled() => return Err(RetrievalError::Cancelled),
                res = builder.send() => res.map_err(|e| {
                    if e.is_timeout() {
                        RetrievalError::Timeout
                    } else if e.is_connect() {
                        RetrievalError::Transport(format!("connect: {e}"))
                    } else {
                        RetrievalError::Transport(e.to_string())
                    }
                })?,
            };

            let status = response.status();
            if status.is_redirection() {
                redirects += 1;
                if redirects > self.policy.max_redirects {
                    return Err(RetrievalError::RedirectPolicy("too many redirects".into()));
                }
                let loc = response
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|v| v.to_str().ok())
                    .ok_or_else(|| {
                        RetrievalError::RedirectPolicy("redirect missing Location".into())
                    })?;
                let next = current
                    .join(loc)
                    .map_err(|e| RetrievalError::RedirectPolicy(e.to_string()))?;
                if !self.base.same_origin(&next) {
                    return Err(RetrievalError::RedirectPolicy(format!(
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
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);
            let retry_after_ms = parse_retry_after(response.headers());

            if status.as_u16() == 429 {
                // Drain a small error preview only; do not buffer the full body.
                let _ =
                    read_body_bounded(response, self.policy.max_error_preview_chars, cancel).await;
                return Err(RetrievalError::RateLimited { retry_after_ms });
            }

            let bytes = read_body_bounded(response, self.policy.max_response_bytes, cancel).await?;

            if !status.is_success() {
                let preview = redact_error_preview(
                    std::str::from_utf8(&bytes).unwrap_or(""),
                    self.policy.max_error_preview_chars,
                );
                let category = RetrievalErrorCategory::from_status(status.as_u16());
                let message = extract_error_message(&preview).unwrap_or(preview);
                return Err(RetrievalError::Http {
                    status: status.as_u16(),
                    category,
                    message,
                    request_id,
                    provider_id: Some(self.provider_id.clone()),
                });
            }

            if bytes.is_empty() {
                return Ok(Value::Object(Default::default()));
            }
            let value: Value = serde_json::from_slice(&bytes)
                .map_err(|e| RetrievalError::Decode(e.to_string()))?;
            return Ok(value);
        }
    }
}

/// Stream response body chunks until complete or hard byte cap exceeded.
async fn read_body_bounded(
    mut response: reqwest::Response,
    max_bytes: usize,
    cancel: &CancellationToken,
) -> RetrievalResult<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        if cancel.is_cancelled() {
            return Err(RetrievalError::Cancelled);
        }
        let chunk = tokio::select! {
            _ = cancel.cancelled() => return Err(RetrievalError::Cancelled),
            c = response.chunk() => c.map_err(|e| RetrievalError::Transport(e.to_string()))?,
        };
        match chunk {
            Some(bytes) => {
                if out.len().saturating_add(bytes.len()) > max_bytes {
                    return Err(RetrievalError::OversizedResponse {
                        limit_bytes: max_bytes,
                    });
                }
                out.extend_from_slice(&bytes);
            }
            None => break,
        }
    }
    Ok(out)
}

fn apply_auth(
    builder: reqwest::RequestBuilder,
    scheme: &RetrievalAuthScheme,
    credential: &RetrievalCredential,
) -> RetrievalResult<reqwest::RequestBuilder> {
    match scheme {
        RetrievalAuthScheme::None => Ok(builder),
        RetrievalAuthScheme::Bearer => {
            let token = credential
                .as_str()
                .ok_or(RetrievalError::MissingCredential)?;
            Ok(builder.header("Authorization", format!("Bearer {token}")))
        }
        RetrievalAuthScheme::XApiKey => {
            let token = credential
                .as_str()
                .ok_or(RetrievalError::MissingCredential)?;
            Ok(builder.header("x-api-key", token))
        }
        RetrievalAuthScheme::CustomHeader { name } => {
            let token = credential
                .as_str()
                .ok_or(RetrievalError::MissingCredential)?;
            Ok(builder.header(name.as_str(), token))
        }
    }
}

fn validate_header_name(name: &str) -> RetrievalResult<()> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(RetrievalError::InvalidRequest(
            "custom auth header name must be non-empty".into(),
        ));
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower == "authorization"
        || lower == "cookie"
        || lower == "proxy-authorization"
        || lower.starts_with("x-grok-")
    {
        return Err(RetrievalError::InvalidRequest(format!(
            "refusing restricted or first-party auth header `{trimmed}`"
        )));
    }
    if trimmed
        .chars()
        .any(|c| c.is_control() || c == '\r' || c == '\n' || c == ':')
    {
        return Err(RetrievalError::InvalidRequest(format!(
            "invalid auth header name `{trimmed}`"
        )));
    }
    // Ensure reqwest can parse it.
    reqwest::header::HeaderName::from_str(trimmed).map_err(|_| {
        RetrievalError::InvalidRequest(format!("invalid auth header name `{trimmed}`"))
    })?;
    Ok(())
}

fn validate_extra_headers(headers: &[(String, String)]) -> RetrievalResult<()> {
    for (k, v) in headers {
        let lower = k.to_ascii_lowercase();
        // Auth / cookie / first-party / typed org-project / content negotiation
        // owners must not be set via free-form extra_headers.
        if lower == "authorization"
            || lower == "cookie"
            || lower == "proxy-authorization"
            || lower == "content-type"
            || lower == "accept"
            || lower == "openai-organization"
            || lower == "openai-project"
        {
            return Err(RetrievalError::InvalidRequest(format!(
                "refusing to set restricted header `{k}` via extra_headers"
            )));
        }
        if lower.starts_with("x-grok-") {
            return Err(RetrievalError::InvalidRequest(format!(
                "refusing first-party header `{k}` on retrieval client"
            )));
        }
        if v.chars().any(|c| c == '\r' || c == '\n' || c.is_control()) {
            return Err(RetrievalError::InvalidRequest(format!(
                "header `{k}` contains invalid control character"
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
        .map(|s| redact_error_preview(s, 256))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval::types::RetrievalPurpose;

    fn sample_route(auth: RetrievalAuthScheme) -> RetrievalRouteContext {
        RetrievalRouteContext {
            provider_instance_id: "lab".into(),
            provider_kind: "openai_compatible".into(),
            api_surface: "openai_compatible_subset".into(),
            credential_route: "api_key".into(),
            auth_scheme: auth,
            base_url: "http://127.0.0.1:9/v1".into(),
            display_name: "Lab".into(),
            organization: None,
            project: None,
            extra_headers: Vec::new(),
            incarnation: None,
            registry_generation: 1,
            request_timeout: Duration::from_secs(5),
            connect_timeout: Duration::from_secs(2),
            total_deadline: Duration::from_secs(10),
            max_retries: 1,
            max_redirects: 2,
            max_response_bytes: 1024 * 1024,
            purpose: RetrievalPurpose::Embeddings,
        }
    }

    #[test]
    fn transport_debug_redacts_nothing_secret_and_builds() {
        let t = RetrievalTransport::from_route(&sample_route(RetrievalAuthScheme::Bearer)).unwrap();
        let dbg = format!("{t:?}");
        assert!(dbg.contains("provider_id"));
        assert!(!dbg.contains("sk-"));
    }

    #[test]
    fn rejects_restricted_extra_headers() {
        let mut route = sample_route(RetrievalAuthScheme::Bearer);
        route
            .extra_headers
            .push(("Authorization".into(), "Bearer x".into()));
        let err = RetrievalTransport::from_route(&route).unwrap_err();
        assert!(matches!(err, RetrievalError::InvalidRequest(_)));
        let mut route2 = sample_route(RetrievalAuthScheme::Bearer);
        route2
            .extra_headers
            .push(("OpenAI-Organization".into(), "org".into()));
        assert!(RetrievalTransport::from_route(&route2).is_err());
        let mut route3 = sample_route(RetrievalAuthScheme::Bearer);
        route3
            .extra_headers
            .push(("Content-Type".into(), "text/plain".into()));
        assert!(RetrievalTransport::from_route(&route3).is_err());
    }

    #[test]
    fn rejects_restricted_custom_auth_header_name() {
        let route = sample_route(RetrievalAuthScheme::CustomHeader {
            name: "Authorization".into(),
        });
        assert!(RetrievalTransport::from_route(&route).is_err());
    }

    #[test]
    fn join_endpoint_normalizes_and_blocks_attacks() {
        let t = RetrievalTransport::from_route(&sample_route(RetrievalAuthScheme::None)).unwrap();
        let u = t.join_endpoint("embeddings").unwrap();
        assert_eq!(u.as_str(), "http://127.0.0.1:9/v1/embeddings");
        assert!(t.join_endpoint("https://evil/x").is_err());
        assert!(t.join_endpoint("../secret").is_err());
        assert!(t.join_endpoint("//evil/x").is_err());
    }

    #[test]
    fn credential_debug_hides_value() {
        let c = RetrievalCredential::new(Some("super-secret-key".into()));
        let dbg = format!("{c:?}");
        assert!(!dbg.contains("super-secret"));
        assert!(dbg.contains("present: true"));
    }
}
