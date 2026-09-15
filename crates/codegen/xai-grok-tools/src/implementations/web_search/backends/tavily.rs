//! Tavily backend — `POST {base}/search` with `Authorization: Bearer <key>`.

use indexmap::IndexMap;
use reqwest::header::AUTHORIZATION;

use super::{
    SearchHit, SearchRequest, SearchResponse, WebSearchBackend, error_body, headers_from_extra,
    http_client_with_headers, join_url, missing_api_key_reason, missing_base_url_reason,
    web_search_tool_id,
};
use xai_tool_runtime::ToolError;

/// Number of results requested per query.
const MAX_RESULTS: usize = 5;

#[derive(Clone)]
pub struct TavilyBackend {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl TavilyBackend {
    pub fn new(
        base_url: &str,
        api_key: Option<String>,
        extra_headers: &IndexMap<String, String>,
    ) -> Result<Self, ToolError> {
        let mut headers = headers_from_extra(extra_headers)?;
        let api_key = api_key.unwrap_or_default();
        if !api_key.is_empty() {
            let value = reqwest::header::HeaderValue::from_str(&format!("Bearer {api_key}"))
                .map_err(|error| {
                    ToolError::execution(
                        web_search_tool_id(),
                        format!("Invalid Tavily API key for header: {error}"),
                    )
                })?;
            headers.insert(AUTHORIZATION, value);
        }
        Ok(Self {
            http: http_client_with_headers(headers, "tavily")?,
            base_url: base_url.to_string(),
            api_key,
        })
    }
}

impl WebSearchBackend for TavilyBackend {
    fn name(&self) -> &str {
        "tavily"
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
        let url = join_url(&self.base_url, "/search");
        let mut body = serde_json::json!({
            "query": request.query,
            "max_results": MAX_RESULTS,
            "include_answer": true,
        });
        let domains = request.domains();
        if !domains.is_empty() {
            body["include_domains"] = serde_json::json!(domains);
        }
        let response = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|error| {
                ToolError::execution(
                    web_search_tool_id(),
                    format!("Tavily request to {url} failed: {error}"),
                )
            })?;
        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            let body = error_body(response).await;
            return Err(ToolError::unauthorized(format!(
                "Tavily returned 401 Unauthorized: {body}"
            )));
        }
        if !status.is_success() {
            let body = error_body(response).await;
            return Err(ToolError::execution(
                web_search_tool_id(),
                format!("Tavily returned {status}: {body}"),
            ));
        }
        let bytes = response.bytes().await.map_err(|error| {
            ToolError::execution(
                web_search_tool_id(),
                format!("Failed to read Tavily response body: {error}"),
            )
        })?;
        let payload: TavilyResponse = serde_json::from_slice(&bytes).map_err(|error| {
            ToolError::execution(
                web_search_tool_id(),
                format!("Failed to parse Tavily response: {error}"),
            )
        })?;
        let mut response = SearchResponse::rendered(
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
        );
        // Tavily's synthesized answer is closer to what the model wants than the
        // raw result list, so prefer it when the instance returns one.
        if let Some(answer) = payload.answer.filter(|answer| !answer.trim().is_empty()) {
            response.content = answer;
        }
        Ok(response)
    }
}

#[derive(serde::Deserialize)]
struct TavilyResponse {
    #[serde(default)]
    answer: Option<String>,
    #[serde(default)]
    results: Vec<TavilyResult>,
}

#[derive(serde::Deserialize)]
struct TavilyResult {
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
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn search_request<'a>(query: &'a str, domains: Option<&'a [String]>) -> SearchRequest<'a> {
        SearchRequest {
            query,
            allowed_domains: domains,
        }
    }

    #[test]
    fn missing_key_and_base_url_are_reported_with_their_knobs() {
        let backend = TavilyBackend::new("https://api.tavily.com", None, &IndexMap::new())
            .expect("backend builds");
        let reason = backend.is_configured().unwrap_err();
        assert!(reason.contains("GROK_SEARCH_API_KEY"), "{reason}");

        let backend = TavilyBackend::new("", Some("key".to_string()), &IndexMap::new())
            .expect("backend builds");
        let reason = backend.is_configured().unwrap_err();
        assert!(reason.contains("GROK_SEARCH_BASE_URL"), "{reason}");
    }

    #[tokio::test]
    async fn post_request_carries_bearer_and_query_body() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/search"))
            .and(header("Authorization", "Bearer tvly-test-key"))
            .and(header("content-type", "application/json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "query": "rust async",
                "answer": "Rust async is a runtime-provided concurrency model.",
                "results": [
                    {"title": "Async book", "url": "https://rust-lang.github.io/async-book/", "content": "Guide"},
                ]
            })))
            .mount(&server)
            .await;

        let backend = TavilyBackend::new(
            &server.uri(),
            Some("tvly-test-key".to_string()),
            &IndexMap::new(),
        )
        .expect("backend builds");
        assert!(backend.is_configured().is_ok());
        let response = backend
            .search(&search_request("rust async", None))
            .await
            .expect("search succeeds");

        assert_eq!(
            response.content,
            "Rust async is a runtime-provided concurrency model."
        );
        assert_eq!(
            response.citations(),
            ["https://rust-lang.github.io/async-book/"]
        );

        let requests = server.received_requests().await.expect("requests recorded");
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].url.path(), "/search");
        assert!(requests[0].url.query().is_none());
    }

    #[tokio::test]
    async fn allowed_domains_become_include_domains() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/search"))
            .and(body_json(serde_json::json!({
                "query": "async",
                "max_results": 5,
                "include_answer": true,
                "include_domains": ["example.com", "rust-lang.org"],
            })))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"results": []})),
            )
            .mount(&server)
            .await;

        let domains = vec!["example.com".to_string(), "rust-lang.org".to_string()];
        let backend = TavilyBackend::new(&server.uri(), Some("key".to_string()), &IndexMap::new())
            .expect("backend builds");
        let response = backend
            .search(&search_request("async", Some(&domains)))
            .await
            .expect("search succeeds");
        // No answer and no results: the shared placeholder is used.
        assert_eq!(response.content, super::super::NO_RESULTS);
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn renders_the_result_list_when_there_is_no_answer() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "answer": "   ",
                "results": [{"title": "Rust", "url": "https://www.rust-lang.org/", "content": "Systems language"}]
            })))
            .mount(&server)
            .await;

        let backend = TavilyBackend::new(&server.uri(), Some("key".to_string()), &IndexMap::new())
            .expect("backend builds");
        let response = backend
            .search(&search_request("rust", None))
            .await
            .expect("search succeeds");
        assert!(response.content.contains("1. Rust"), "{}", response.content);
    }

    #[tokio::test]
    async fn unauthorized_is_reported_as_unauthorized() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(401).set_body_string("bad key"))
            .mount(&server)
            .await;

        let backend = TavilyBackend::new(&server.uri(), Some("key".to_string()), &IndexMap::new())
            .expect("backend builds");
        let error = backend
            .search(&search_request("rust", None))
            .await
            .expect_err("401 must fail");
        assert!(error.to_string().contains("401"), "{error}");
    }
}
