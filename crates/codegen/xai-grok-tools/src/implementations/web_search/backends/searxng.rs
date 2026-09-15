//! SearXNG backend — keyless `GET {base}/search?format=json`.
//!
//! SearXNG needs no API key, so this backend is configured by a base URL alone.
//! The instance must have the JSON output format enabled (`format: json` in
//! `searxng/settings.yml`); an HTML answer is reported with that hint instead of
//! a bare parse error.

use indexmap::IndexMap;

use super::{
    SearchHit, SearchRequest, SearchResponse, WebSearchBackend, error_body, headers_from_extra,
    http_client_with_headers, join_url, missing_base_url_reason, web_search_tool_id,
};
use xai_tool_runtime::ToolError;

/// Number of results requested per query.
const MAX_RESULTS: usize = 5;

#[derive(Clone)]
pub struct SearxngBackend {
    http: reqwest::Client,
    base_url: String,
}

impl SearxngBackend {
    pub fn new(
        base_url: &str,
        extra_headers: &IndexMap<String, String>,
    ) -> Result<Self, ToolError> {
        let headers = headers_from_extra(extra_headers)?;
        Ok(Self {
            http: http_client_with_headers(headers, "searxng")?,
            base_url: base_url.to_string(),
        })
    }
}

impl WebSearchBackend for SearxngBackend {
    fn name(&self) -> &str {
        "searxng"
    }

    fn is_configured(&self) -> Result<(), String> {
        if self.base_url.trim().is_empty() {
            return Err(missing_base_url_reason(self.name()));
        }
        Ok(())
    }

    async fn search(&self, request: &SearchRequest<'_>) -> Result<SearchResponse, ToolError> {
        let url = join_url(&self.base_url, "/search");
        let response = self
            .http
            .get(&url)
            .query(&[
                ("q", request.query_with_site_filter()),
                ("format", "json".to_string()),
            ])
            .send()
            .await
            .map_err(|error| {
                ToolError::execution(
                    web_search_tool_id(),
                    format!("SearXNG request to {url} failed: {error}"),
                )
            })?;
        let status = response.status();
        if !status.is_success() {
            let body = error_body(response).await;
            return Err(ToolError::execution(
                web_search_tool_id(),
                format!("SearXNG returned {status}: {body}"),
            ));
        }
        let bytes = response.bytes().await.map_err(|error| {
            ToolError::execution(
                web_search_tool_id(),
                format!("Failed to read SearXNG response body: {error}"),
            )
        })?;
        let payload: SearxngResponse = serde_json::from_slice(&bytes).map_err(|error| {
            ToolError::execution(
                web_search_tool_id(),
                format!(
                    "Failed to parse SearXNG response as JSON ({error}); \
                     make sure the instance has `format: json` enabled"
                ),
            )
        })?;
        Ok(SearchResponse::rendered(
            payload
                .results
                .into_iter()
                .take(MAX_RESULTS)
                .map(|result| SearchHit {
                    title: result.title,
                    url: result.url,
                    snippet: result.content,
                })
                .collect(),
        ))
    }
}

#[derive(serde::Deserialize)]
struct SearxngResponse {
    #[serde(default)]
    results: Vec<SearxngResult>,
}

#[derive(serde::Deserialize)]
struct SearxngResult {
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    content: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn search_request<'a>(query: &'a str, domains: Option<&'a [String]>) -> SearchRequest<'a> {
        SearchRequest {
            query,
            allowed_domains: domains,
        }
    }

    fn query_pairs(request: &wiremock::Request) -> std::collections::HashMap<String, String> {
        url::form_urlencoded::parse(request.url.query().unwrap_or_default().as_bytes())
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect()
    }

    #[test]
    fn missing_base_url_is_reported_with_the_knob_name() {
        let backend = SearxngBackend::new("", &IndexMap::new()).expect("backend builds");
        let reason = backend.is_configured().unwrap_err();
        assert!(reason.contains("GROK_SEARCH_BASE_URL"), "{reason}");
        assert!(reason.contains("[search]"), "{reason}");
    }

    #[tokio::test]
    async fn get_request_carries_q_and_format_and_no_auth_header() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/search"))
            .and(query_param("format", "json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "results": [
                    {"title": "Rust", "url": "https://www.rust-lang.org/", "content": "Systems language"},
                    {"title": "Docs", "url": "https://docs.rs/", "content": "Crate docs"},
                ]
            })))
            .mount(&server)
            .await;

        let backend = SearxngBackend::new(&server.uri(), &IndexMap::new()).expect("backend builds");
        assert!(backend.is_configured().is_ok());
        let response = backend
            .search(&search_request("rust async", None))
            .await
            .expect("search succeeds");

        assert_eq!(
            response.citations(),
            ["https://www.rust-lang.org/", "https://docs.rs/"]
        );
        assert!(response.content.contains("1. Rust"));
        assert!(response.content.contains("   Systems language"));

        let requests = server.received_requests().await.expect("requests recorded");
        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert_eq!(request.url.path(), "/search");
        let params = query_pairs(request);
        assert_eq!(params.get("q").map(String::as_str), Some("rust async"));
        assert_eq!(params.get("format").map(String::as_str), Some("json"));
        assert!(request.headers.get("authorization").is_none());
    }

    #[tokio::test]
    async fn allowed_domains_become_site_filters_in_the_query() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/search"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"results": []})),
            )
            .mount(&server)
            .await;

        let domains = vec!["example.com".to_string(), "rust-lang.org".to_string()];
        let backend = SearxngBackend::new(&server.uri(), &IndexMap::new()).expect("backend builds");
        let response = backend
            .search(&search_request("async", Some(&domains)))
            .await
            .expect("search succeeds");
        assert_eq!(response.content, super::super::NO_RESULTS);

        let requests = server.received_requests().await.expect("requests recorded");
        let params = query_pairs(&requests[0]);
        assert_eq!(
            params.get("q").map(String::as_str),
            Some("async site:example.com site:rust-lang.org")
        );
    }

    #[tokio::test]
    async fn html_answer_reports_the_format_hint() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/search"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_raw("<html></html>", "text/html; charset=utf-8"),
            )
            .mount(&server)
            .await;

        let backend = SearxngBackend::new(&server.uri(), &IndexMap::new()).expect("backend builds");
        let error = backend
            .search(&search_request("rust", None))
            .await
            .expect_err("html body must not parse as json");
        assert!(error.to_string().contains("format: json"), "{error}");
    }

    #[tokio::test]
    async fn error_status_is_surfaced_with_the_body() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(503).set_body_string("upstream is down"))
            .mount(&server)
            .await;

        let backend = SearxngBackend::new(&server.uri(), &IndexMap::new()).expect("backend builds");
        let error = backend
            .search(&search_request("rust", None))
            .await
            .expect_err("503 must fail");
        let message = error.to_string();
        assert!(message.contains("503"), "{message}");
        assert!(message.contains("upstream is down"), "{message}");
    }
}
