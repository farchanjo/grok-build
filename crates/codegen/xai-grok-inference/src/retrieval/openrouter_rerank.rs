//! OpenRouter native rerank via generated `OpenRouterClient::create_rerank`.
//!
//! JSON primary only. Exact OpenRouterNative instance + application key —
//! never admin/OAuth/sibling fallback. Generated ops mark `idempotent: false`,
//! so this adapter owns the bounded 429/5xx/transient retry loop (without
//! editing generated files).

use std::collections::HashSet;
use std::time::{Duration, Instant};

use tokio_util::sync::CancellationToken;

use super::transport::RetrievalCredential;
use super::types::{
    RerankAdapter, RerankHit, RerankRequest, RerankResult, RetrievalAuthScheme, RetrievalError,
    RetrievalResult, RetrievalRouteContext, validate_rerank_request,
};
use crate::openai_platform::PlatformError;
use crate::openai_platform::client::{OpenRouterClient, PlatformClientConfig};
use crate::openai_platform::generated::openrouter_types::{
    CreateRerankBody, CreateRerankBodyDocumentsItemUnion, CreateRerankParams, CreateRerankResult,
};
use crate::openai_platform::transport::TransportPolicy;

/// OpenRouter-native rerank adapter (generated create_rerank + local retry).
#[derive(Debug, Clone)]
pub struct OpenRouterRerankAdapter {
    route: RetrievalRouteContext,
}

impl OpenRouterRerankAdapter {
    pub fn new(route: RetrievalRouteContext) -> RetrievalResult<Self> {
        if route.api_surface != "openrouter_native" && route.provider_kind != "openrouter" {
            return Err(RetrievalError::SurfaceMismatch(format!(
                "OpenRouter rerank requires openrouter_native surface, got {}",
                route.api_surface
            )));
        }
        Ok(Self { route })
    }

    pub async fn rerank(
        &self,
        request: RerankRequest,
        credential: &RetrievalCredential,
        cancel: CancellationToken,
    ) -> RetrievalResult<RerankResult> {
        validate_rerank_request(&request)?;
        let token = match &self.route.auth_scheme {
            RetrievalAuthScheme::None => None,
            _ => Some(
                credential
                    .as_str()
                    .ok_or(RetrievalError::MissingCredential)?
                    .to_owned(),
            ),
        };
        if self.route.auth_scheme != RetrievalAuthScheme::None && token.is_none() {
            return Err(RetrievalError::MissingCredential);
        }

        // Typed org/project only — never via free-form extra_headers collision.
        let mut extra = std::collections::BTreeMap::new();
        for (k, v) in &self.route.extra_headers {
            let lower = k.to_ascii_lowercase();
            if lower == "openai-organization"
                || lower == "openai-project"
                || lower == "authorization"
                || lower == "content-type"
                || lower == "accept"
            {
                return Err(RetrievalError::InvalidRequest(format!(
                    "refusing restricted header `{k}` on OpenRouter rerank extra_headers"
                )));
            }
            extra.insert(k.clone(), v.clone());
        }
        if let Some(org) = &self.route.organization {
            validate_org_project_value("organization", org)?;
            extra.insert("OpenAI-Organization".into(), org.clone());
        }
        if let Some(proj) = &self.route.project {
            validate_org_project_value("project", proj)?;
            extra.insert("OpenAI-Project".into(), proj.clone());
        }

        let body = CreateRerankBody {
            model: request.model.clone(),
            query: request.query.clone(),
            documents: request
                .documents
                .iter()
                .map(|d| CreateRerankBodyDocumentsItemUnion::Variant0(d.clone()))
                .collect(),
            top_n: request.top_n.map(|n| n as i64),
            provider: None,
            extra: Default::default(),
        };
        let params = CreateRerankParams { body };

        let started = Instant::now();
        let mut attempts = 0u32;
        let max_retries = self.route.max_retries;
        let total_deadline = self.route.total_deadline;

        loop {
            attempts += 1;
            if started.elapsed() >= total_deadline {
                return Err(RetrievalError::DeadlineExceeded);
            }
            if cancel.is_cancelled() {
                return Err(RetrievalError::Cancelled);
            }

            let remaining = total_deadline.saturating_sub(started.elapsed());
            let attempt_timeout = self.route.request_timeout.min(remaining);
            let attempt_cancel = cancel.child_token();

            // Platform max_retries=0: adapter owns all retries (generated op is
            // non-idempotent so platform would not retry anyway).
            let policy = TransportPolicy {
                connect_timeout: self.route.connect_timeout.min(attempt_timeout),
                request_timeout: attempt_timeout,
                max_response_bytes: self.route.max_response_bytes,
                max_error_preview_chars: 512,
                max_redirects: self.route.max_redirects,
                max_pagination_pages: 1,
                max_retries: 0,
                user_agent: format!("grok-retrieval/{}", env!("CARGO_PKG_VERSION")),
            };
            let cfg = PlatformClientConfig {
                provider_id: self.route.provider_instance_id.clone(),
                display_name: self.route.display_name.clone(),
                base_url: self.route.base_url.clone(),
                admin_base_url: None,
                application_token: token.clone(),
                admin_token: None,
                extra_headers: extra.clone(),
                policy,
            };

            let client = OpenRouterClient::from_config(cfg, attempt_cancel.clone())
                .map_err(map_platform_error)?;

            // Child cancel is wired into PlatformTransport; winning deadline/cancel
            // branches cancel the child then drop the in-flight future so the
            // HTTP attempt cannot outlive the total deadline unobserved.
            let mapped = tokio::select! {
                biased;
                _ = cancel.cancelled() => {
                    attempt_cancel.cancel();
                    return Err(RetrievalError::Cancelled);
                }
                _ = tokio::time::sleep(remaining) => {
                    attempt_cancel.cancel();
                    return Err(RetrievalError::DeadlineExceeded);
                }
                res = client.create_rerank(params.clone()) => match res {
                    Ok(result) => {
                        return map_openrouter_result(
                            result,
                            request.documents.len(),
                            request.top_n,
                            &request.model,
                        );
                    }
                    Err(e) => map_platform_error(e),
                },
            };

            if mapped.is_retryable() && attempts < 1 + max_retries {
                let sleep_ms = match &mapped {
                    RetrievalError::RateLimited {
                        retry_after_ms: Some(ms),
                    } => (*ms).min(5_000),
                    RetrievalError::RateLimited { .. } => 250 * u64::from(attempts),
                    _ => 200 * u64::from(attempts),
                }
                .min(5_000);
                let sleep_dur = Duration::from_millis(sleep_ms);
                if started.elapsed() + sleep_dur >= total_deadline {
                    return Err(RetrievalError::DeadlineExceeded);
                }
                tokio::select! {
                    _ = cancel.cancelled() => return Err(RetrievalError::Cancelled),
                    _ = tokio::time::sleep(sleep_dur) => {}
                }
                continue;
            }
            return Err(mapped);
        }
    }
}

fn validate_org_project_value(label: &str, value: &str) -> RetrievalResult<()> {
    if value
        .chars()
        .any(|c| c == '\r' || c == '\n' || c.is_control())
    {
        return Err(RetrievalError::InvalidRequest(format!(
            "OpenAI-{label} contains invalid control characters"
        )));
    }
    Ok(())
}

impl RerankAdapter for OpenRouterRerankAdapter {
    async fn rerank(
        &self,
        request: RerankRequest,
        cancel: CancellationToken,
    ) -> RetrievalResult<RerankResult> {
        let _ = (request, cancel);
        Err(RetrievalError::MissingCredential)
    }

    fn route_context(&self) -> &RetrievalRouteContext {
        &self.route
    }
}

/// Map generated OpenRouter result into secret-free domain hits.
pub fn map_openrouter_result(
    result: CreateRerankResult,
    document_count: usize,
    top_n: Option<u32>,
    fallback_model: &str,
) -> RetrievalResult<RerankResult> {
    let items = &result.body.results;
    if items.is_empty() {
        return Err(RetrievalError::MalformedResponse(
            "OpenRouter rerank results are empty".into(),
        ));
    }
    if items.len() > document_count {
        return Err(RetrievalError::MalformedResponse(format!(
            "OpenRouter rerank result count {} exceeds document count {document_count}",
            items.len()
        )));
    }
    let mut seen = HashSet::new();
    let mut hits = Vec::with_capacity(items.len());
    for item in items {
        if item.index < 0 || item.index as usize >= document_count {
            return Err(RetrievalError::MalformedResponse(format!(
                "OpenRouter rerank index {} out of range 0..{document_count}",
                item.index
            )));
        }
        let idx = item.index as usize;
        if !seen.insert(idx) {
            return Err(RetrievalError::MalformedResponse(format!(
                "duplicate OpenRouter rerank index {idx}"
            )));
        }
        if !item.relevance_score.is_finite() {
            return Err(RetrievalError::MalformedResponse(format!(
                "OpenRouter rerank score at index {idx} is non-finite"
            )));
        }
        let document = item.document.text.clone();
        hits.push(RerankHit {
            index: idx,
            score: item.relevance_score as f32,
            document,
        });
    }
    if let Some(top_n) = top_n {
        hits.truncate(top_n as usize);
    }
    Ok(RerankResult {
        model: if result.body.model.is_empty() {
            fallback_model.to_owned()
        } else {
            result.body.model
        },
        hits,
    })
}

pub(crate) fn map_platform_error(err: PlatformError) -> RetrievalError {
    match err {
        PlatformError::InvalidRequest(m) => RetrievalError::InvalidRequest(m),
        PlatformError::InvalidUrl(m) => RetrievalError::InvalidUrl(m),
        PlatformError::MissingCredential(_) => RetrievalError::MissingCredential,
        PlatformError::RedirectPolicy(m) => RetrievalError::RedirectPolicy(m),
        PlatformError::Http {
            status,
            category: _,
            message,
            request_id,
            provider_id,
            ..
        } => RetrievalError::Http {
            status,
            category: super::types::RetrievalErrorCategory::from_status(status),
            message,
            request_id,
            provider_id,
        },
        PlatformError::Decode(m) => RetrievalError::Decode(m),
        PlatformError::Timeout { .. } => RetrievalError::Timeout,
        PlatformError::Cancelled => RetrievalError::Cancelled,
        PlatformError::RateLimited { retry_after_ms, .. } => {
            RetrievalError::RateLimited { retry_after_ms }
        }
        PlatformError::OversizedResponse { limit_bytes } => {
            RetrievalError::OversizedResponse { limit_bytes }
        }
        PlatformError::PaginationLimit => RetrievalError::Transport("pagination limit".into()),
        PlatformError::Transport(m) => RetrievalError::Transport(m),
        PlatformError::UnsupportedTransport(m) => RetrievalError::Transport(m),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openai_platform::generated::openrouter_types::{
        CreateRerankResultBody, CreateRerankResultBodyResultsItem,
        CreateRerankResultBodyResultsItemDocument,
    };

    #[test]
    fn maps_happy_and_rejects_malformed() {
        let ok = CreateRerankResult {
            body: CreateRerankResultBody {
                id: None,
                model: "or-rerank".into(),
                provider: None,
                results: vec![CreateRerankResultBodyResultsItem {
                    document: CreateRerankResultBodyResultsItemDocument {
                        image: None,
                        text: Some("doc".into()),
                        extra: Default::default(),
                    },
                    index: 1,
                    relevance_score: 0.8,
                    extra: Default::default(),
                }],
                usage: None,
                extra: Default::default(),
            },
        };
        let res = map_openrouter_result(ok, 3, None, "fallback").unwrap();
        assert_eq!(res.hits[0].index, 1);
        assert_eq!(res.model, "or-rerank");

        let bad = CreateRerankResult {
            body: CreateRerankResultBody {
                id: None,
                model: "x".into(),
                provider: None,
                results: vec![CreateRerankResultBodyResultsItem {
                    document: CreateRerankResultBodyResultsItemDocument {
                        image: None,
                        text: None,
                        extra: Default::default(),
                    },
                    index: 99,
                    relevance_score: 0.1,
                    extra: Default::default(),
                }],
                usage: None,
                extra: Default::default(),
            },
        };
        assert!(map_openrouter_result(bad, 2, None, "m").is_err());
    }
}
