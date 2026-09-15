//! Backend-erased web-search client.
//!
//! [`WebSearchClient`] is the single type the `web_search` tool (and its
//! registry seam) ever sees: it holds whichever [`AnyBackend`] the config
//! selected, keeps the historical `search(query, allowed_domains)` signature,
//! and turns a selected-but-unconfigured backend into an actionable tool error
//! instead of a missing-resource failure.
//!
//! The wire shapes live in [`super::backends`]; the selection logic lives in
//! [`super::factory`].

use crate::attribution::SharedAttributionCallback;
use crate::types::SharedApiKeyProvider;

use super::backends::{AnyBackend, SearchRequest, WebSearchBackend, web_search_tool_id};
use super::types::WebSearchConfig;

/// A web-search client bound to the backend named by [`WebSearchConfig`].
#[derive(Clone)]
pub struct WebSearchClient {
    backend: AnyBackend,
}

impl std::fmt::Debug for WebSearchClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebSearchClient")
            .field("backend", &self.backend.name())
            .finish()
    }
}

impl WebSearchClient {
    /// Create a client from a [`WebSearchConfig`].
    ///
    /// Returns `Err` only for `Disabled` or an invalid header value. An
    /// `External` config always builds, so the tool stays registered and can
    /// report a missing base URL or key at call time.
    pub fn new(
        config: &WebSearchConfig,
        api_key_provider: Option<SharedApiKeyProvider>,
    ) -> Result<Self, xai_tool_runtime::ToolError> {
        match config {
            WebSearchConfig::Disabled => Err(xai_tool_runtime::ToolError::execution(
                web_search_tool_id(),
                "Cannot create WebSearchClient from disabled config".to_string(),
            )),
            WebSearchConfig::Enabled { .. } => Ok(Self {
                backend: AnyBackend::Xai(super::backends::XaiBackend::from_config(
                    config,
                    api_key_provider,
                )?),
            }),
            WebSearchConfig::External {
                provider,
                base_url,
                api_key,
                model,
                extra_headers,
            } => Ok(Self {
                backend: AnyBackend::from_provider(
                    provider,
                    base_url,
                    api_key.clone(),
                    model,
                    extra_headers.clone(),
                    api_key_provider,
                )?,
            }),
        }
    }

    /// Backend id (`xai`, `searxng`, …) this client will call.
    pub fn backend_name(&self) -> &str {
        self.backend.name()
    }

    /// `Ok(())` when the selected backend has everything it needs.
    pub fn is_configured(&self) -> Result<(), String> {
        self.backend.is_configured()
    }

    /// Wire a 401-attribution callback into this client. Idempotent;
    /// safe to call before or after the first request.
    pub fn with_attribution_callback(self, callback: Option<SharedAttributionCallback>) -> Self {
        Self {
            backend: self.backend.with_attribution_callback(callback),
        }
    }

    /// Whether a 401-attribution callback is installed (production-boundary tests).
    ///
    /// Only the `xai` backend emits xAI 401 attribution; other backends report
    /// `false`.
    pub fn has_attribution_callback(&self) -> bool {
        self.backend.has_attribution_callback()
    }

    /// Perform a web search query.
    ///
    /// Returns `(content, citations)` where content is the text handed to the
    /// model and citations are unique URLs found in the results.
    pub async fn search(
        &self,
        query: &str,
        allowed_domains: Option<Vec<String>>,
    ) -> Result<(String, Vec<String>), xai_tool_runtime::ToolError> {
        let response = self.run(query, allowed_domains.as_deref()).await?;
        let citations = response.citations();
        Ok((response.content, citations))
    }

    /// Same as [`Self::search`] but also extracts per-result titles. Returns
    /// `(content, citations_with_titles)` where each citation is
    /// `(title, url)`. Empty `title` strings indicate the upstream didn't
    /// supply one for that URL.
    ///
    /// Used by the cursor-compat `WebSearch` adapter to render a
    /// `Links:\n1. [title](url)` list instead of the LLM synthesis text.
    pub async fn search_with_titles(
        &self,
        query: &str,
        allowed_domains: Option<Vec<String>>,
    ) -> Result<(String, Vec<(String, String)>), xai_tool_runtime::ToolError> {
        let response = self.run(query, allowed_domains.as_deref()).await?;
        let pairs = response.titled_citations();
        Ok((response.content, pairs))
    }

    async fn run(
        &self,
        query: &str,
        allowed_domains: Option<&[String]>,
    ) -> Result<super::backends::SearchResponse, xai_tool_runtime::ToolError> {
        self.backend.is_configured().map_err(|reason| {
            xai_tool_runtime::ToolError::execution(web_search_tool_id(), reason)
        })?;
        let request = SearchRequest {
            query,
            allowed_domains,
        };
        WebSearchBackend::search(&self.backend, &request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn external(provider: &str, base_url: &str, api_key: Option<&str>) -> WebSearchConfig {
        WebSearchConfig::External {
            provider: provider.to_string(),
            base_url: base_url.to_string(),
            api_key: api_key.map(str::to_string),
            model: String::new(),
            extra_headers: IndexMap::new(),
        }
    }

    #[test]
    fn disabled_config_is_rejected() {
        let error = WebSearchClient::new(&WebSearchConfig::Disabled, None)
            .expect_err("disabled config must not build");
        assert!(error.to_string().contains("disabled config"), "{error}");
    }

    /// A selected backend builds even with no settings at all, so the tool
    /// registers and can say what is missing.
    #[test]
    fn selected_backend_without_settings_still_builds() {
        let client = WebSearchClient::new(&external("searxng", "", None), None)
            .expect("external config must build");
        assert_eq!(client.backend_name(), "searxng");
        let reason = client.is_configured().unwrap_err();
        assert!(reason.contains("GROK_SEARCH_BASE_URL"), "{reason}");
    }

    #[test]
    fn unknown_provider_builds_and_reports_the_alternatives() {
        let client =
            WebSearchClient::new(&external("duckduckgo", "https://ddg.example", None), None)
                .expect("unknown provider must still build");
        let reason = client.is_configured().unwrap_err();
        assert!(reason.contains("unknown search provider"), "{reason}");
    }

    #[tokio::test]
    async fn unconfigured_backend_fails_with_the_reason_not_missing_resource() {
        let client =
            WebSearchClient::new(&external("tavily", "https://api.tavily.com", None), None)
                .expect("external config must build");
        let error = client
            .search("rust", None)
            .await
            .expect_err("missing key must fail");
        let message = error.to_string();
        assert!(message.contains("GROK_SEARCH_API_KEY"), "{message}");
        assert!(!message.contains("missing required resource"), "{message}");
    }

    /// Only the xai backend owns the 401-attribution hook; the seam that wires
    /// it must stay a no-op elsewhere rather than lying about it.
    #[test]
    fn attribution_callback_only_lands_on_the_xai_backend() {
        let xai = WebSearchConfig::Enabled {
            api_key: "key".to_string(),
            base_url: "https://api.x.ai/v1".to_string(),
            model: "test-model".to_string(),
            extra_headers: IndexMap::new(),
            alpha_test_key: None,
        };
        let callback: SharedAttributionCallback = std::sync::Arc::new(NoopCallback);
        let client = WebSearchClient::new(&xai, None)
            .expect("client builds")
            .with_attribution_callback(Some(callback.clone()));
        assert!(client.has_attribution_callback());

        let client =
            WebSearchClient::new(&external("searxng", "http://localhost:8888", None), None)
                .expect("client builds")
                .with_attribution_callback(Some(callback));
        assert!(!client.has_attribution_callback());
    }

    #[derive(Debug)]
    struct NoopCallback;

    impl crate::attribution::Auth401AttributionCallback for NoopCallback {
        fn record_401(&self, _consumer: crate::attribution::ToolConsumer, _tail: Option<&str>) {}
    }

    /// The `xai` wire shape still goes through the erased client unchanged.
    #[tokio::test]
    async fn xai_config_keeps_the_responses_wire_shape() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/responses"))
            .and(header("Authorization", "Bearer config-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "resp_test",
                "object": "response",
                "created_at": 1234567890,
                "status": "completed",
                "model": "test-model",
                "output": [{
                    "type": "message",
                    "id": "msg_1",
                    "status": "completed",
                    "role": "assistant",
                    "content": [{
                        "type": "output_text",
                        "text": "synthesized",
                        "annotations": [{
                            "type": "url_citation",
                            "url": "https://www.rust-lang.org/",
                            "title": "Rust",
                            "start_index": 0,
                            "end_index": 4
                        }]
                    }]
                }]
            })))
            .mount(&server)
            .await;

        let config = WebSearchConfig::Enabled {
            api_key: "config-key".to_string(),
            base_url: server.uri(),
            model: "test-model".to_string(),
            extra_headers: IndexMap::new(),
            alpha_test_key: None,
        };
        let client = WebSearchClient::new(&config, None).expect("client builds");
        assert_eq!(client.backend_name(), "xai");
        let (content, citations) = client.search("rust", None).await.expect("search succeeds");
        assert_eq!(content, "synthesized");
        assert_eq!(citations, ["https://www.rust-lang.org/"]);
        let (_, pairs) = client
            .search_with_titles("rust", None)
            .await
            .expect("search succeeds");
        assert_eq!(
            pairs,
            [("Rust".to_string(), "https://www.rust-lang.org/".to_string())]
        );
    }

    /// An `External` config naming `xai` (base URL override only) uses the same
    /// wire shape with the override applied.
    #[tokio::test]
    async fn external_xai_override_uses_the_responses_wire_shape() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/responses"))
            .and(header("Authorization", "Bearer override-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "resp_test",
                "object": "response",
                "created_at": 1234567890,
                "status": "completed",
                "model": "test-model",
                "output": [{
                    "type": "message",
                    "id": "msg_1",
                    "status": "completed",
                    "role": "assistant",
                    "content": [{"type": "output_text", "text": "override result", "annotations": []}]
                }]
            })))
            .mount(&server)
            .await;

        let mut config = external("xai", &server.uri(), Some("override-key"));
        if let WebSearchConfig::External { model, .. } = &mut config {
            *model = "test-model".to_string();
        }
        let client = WebSearchClient::new(&config, None).expect("client builds");
        assert_eq!(client.backend_name(), "xai");
        let (content, _) = client.search("rust", None).await.expect("search succeeds");
        assert_eq!(content, "override result");
    }

    /// A third-party backend selected through the same facade reaches its own
    /// wire shape and never touches `/responses`.
    #[tokio::test]
    async fn external_searxng_client_queries_the_search_endpoint() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/search"))
            .and(query_param("format", "json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "results": [{"title": "Rust", "url": "https://www.rust-lang.org/", "content": "Systems language"}]
            })))
            .mount(&server)
            .await;

        let client = WebSearchClient::new(&external("searxng", &server.uri(), None), None)
            .expect("client builds");
        assert_eq!(client.backend_name(), "searxng");
        let (content, citations) = client.search("rust", None).await.expect("search succeeds");
        assert!(content.contains("1. Rust"), "{content}");
        assert_eq!(citations, ["https://www.rust-lang.org/"]);
    }
}
