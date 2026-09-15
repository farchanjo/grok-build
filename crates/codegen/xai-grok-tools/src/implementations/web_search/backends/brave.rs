//! Brave Search backend — `GET {base}/res/v1/web/search` with an
//! `X-Subscription-Token` header.

use indexmap::IndexMap;
use reqwest::header::{ACCEPT, HeaderName};

use super::{
    SearchHit, SearchRequest, SearchResponse, WebSearchBackend, error_body, headers_from_extra,
    http_client_with_headers, join_url, missing_api_key_reason, missing_base_url_reason,
    web_search_tool_id,
};
use xai_tool_runtime::ToolError;

/// Number of results requested per query.
const MAX_RESULTS: usize = 5;

/// Brave's subscription header. Not a `reqwest::header::HeaderName` constant.
fn subscription_header() -> HeaderName {
    HeaderName::from_static("x-subscription-token")
}

#[derive(Clone)]
pub struct BraveBackend {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl BraveBackend {
    pub fn new(
        base_url: &str,
        api_key: Option<String>,
        extra_headers: &IndexMap<String, String>,
    ) -> Result<Self, ToolError> {
        let mut headers = headers_from_extra(extra_headers)?;
        headers.insert(
            ACCEPT,
            reqwest::header::HeaderValue::from_static("application/json"),
        );
        let api_key = api_key.unwrap_or_default();
        if !api_key.is_empty() {
            let value = reqwest::header::HeaderValue::from_str(&api_key).map_err(|error| {
                ToolError::execution(
                    web_search_tool_id(),
                    format!("Invalid Brave subscription token for header: {error}"),
                )
            })?;
            headers.insert(subscription_header(), value);
        }
        Ok(Self {
            http: http_client_with_headers(headers, "brave")?,
            base_url: base_url.to_string(),
            api_key,
        })
    }
}

impl WebSearchBackend for BraveBackend {
    fn name(&self) -> &str {
        "brave"
    }

    fn is_configured(&self) -> Result<(), String> {
        if self.base_url.trim().is_empty() {
            return Err(missing_base_url_reason(self.name()));
        }
        if self.api_key.trim().is_empty() {
            return Err(missing_api_key_reason(self.name()));
        }
        Ok(())
    }

    async fn search(&self, request: &SearchRequest<'_>) -> Result<SearchResponse, ToolError> {
        let url = join_url(&self.base_url, "/res/v1/web/search");
        let response = self
            .http
            .get(&url)
            .query(&[
                ("q", request.query_with_site_filter()),
                ("count", MAX_RESULTS.to_string()),
            ])
            .send()
            .await
            .map_err(|error| {
                ToolError::execution(
                    web_search_tool_id(),
                    format!("Brave Search request to {url} failed: {error}"),
                )
            })?;
        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            let body = error_body(response).await;
            return Err(ToolError::unauthorized(format!(
                "Brave Search returned {status}: {body}"
            )));
        }
        if !status.is_success() {
            let body = error_body(response).await;
            return Err(ToolError::execution(
                web_search_tool_id(),
                format!("Brave Search returned {status}: {body}"),
            ));
        }
        let bytes = response.bytes().await.map_err(|error| {
            ToolError::execution(
                web_search_tool_id(),
                format!("Failed to read Brave Search response body: {error}"),
            )
        })?;
        let payload: BraveResponse = serde_json::from_slice(&bytes).map_err(|error| {
            ToolError::execution(
                web_search_tool_id(),
                format!("Failed to parse Brave Search response: {error}"),
            )
        })?;
        Ok(SearchResponse::rendered(
            payload
                .web
                .map(|web| web.results)
                .unwrap_or_default()
                .into_iter()
                .take(MAX_RESULTS)
                .map(|result| SearchHit {
                    title: result.title,
                    url: result.url,
                    snippet: result.description,
                })
                .collect(),
        ))
    }
}

#[derive(serde::Deserialize)]
struct BraveResponse {
    #[serde(default)]
    web: Option<BraveWeb>,
}

#[derive(serde::Deserialize)]
struct BraveWeb {
    #[serde(default)]
    results: Vec<BraveResult>,
}

#[derive(serde::Deserialize)]
struct BraveResult {
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    description: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
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
    fn missing_key_and_base_url_are_reported_with_their_knobs() {
        let backend = BraveBackend::new("https://api.search.brave.com", None, &IndexMap::new())
            .expect("backend builds");
        let reason = backend.is_configured().unwrap_err();
        assert!(reason.contains("GROK_SEARCH_API_KEY"), "{reason}");

        let backend = BraveBackend::new("", Some("key".to_string()), &IndexMap::new())
            .expect("backend builds");
        let reason = backend.is_configured().unwrap_err();
        assert!(reason.contains("GROK_SEARCH_BASE_URL"), "{reason}");
    }

    #[tokio::test]
    async fn get_request_carries_subscription_token_and_query() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/res/v1/web/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "web": {
                    "results": [
                        {"title": "Rust", "url": "https://www.rust-lang.org/", "description": "Systems language"},
                        {"title": "Docs", "url": "https://docs.rs/", "description": "Crate docs"},
                    ]
                }
            })))
            .mount(&server)
            .await;

        let backend = BraveBackend::new(
            &server.uri(),
            Some("brave-subscription-token".to_string()),
            &IndexMap::new(),
        )
        .expect("backend builds");
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

        let requests = server.received_requests().await.expect("requests recorded");
        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert_eq!(request.url.path(), "/res/v1/web/search");
        assert_eq!(
            request
                .headers
                .get("x-subscription-token")
                .and_then(|value| value.to_str().ok()),
            Some("brave-subscription-token")
        );
        assert_eq!(
            request
                .headers
                .get("accept")
                .and_then(|value| value.to_str().ok()),
            Some("application/json")
        );
        assert!(request.headers.get("authorization").is_none());
        let params = query_pairs(request);
        assert_eq!(params.get("q").map(String::as_str), Some("rust async"));
        assert_eq!(params.get("count").map(String::as_str), Some("5"));
    }

    #[tokio::test]
    async fn allowed_domains_become_site_filters() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/res/v1/web/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;

        let domains = vec!["example.com".to_string()];
        let backend = BraveBackend::new(&server.uri(), Some("token".to_string()), &IndexMap::new())
            .expect("backend builds");
        let response = backend
            .search(&search_request("async", Some(&domains)))
            .await
            .expect("search succeeds");
        assert_eq!(response.content, super::super::NO_RESULTS);

        let requests = server.received_requests().await.expect("requests recorded");
        let params = query_pairs(&requests[0]);
        assert_eq!(
            params.get("q").map(String::as_str),
            Some("async site:example.com")
        );
    }

    #[tokio::test]
    async fn forbidden_is_reported_as_unauthorized() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/res/v1/web/search"))
            .respond_with(ResponseTemplate::new(403).set_body_string("quota exceeded"))
            .mount(&server)
            .await;

        let backend = BraveBackend::new(&server.uri(), Some("token".to_string()), &IndexMap::new())
            .expect("backend builds");
        let error = backend
            .search(&search_request("rust", None))
            .await
            .expect_err("403 must fail");
        assert!(error.to_string().contains("403"), "{error}");
    }
}
