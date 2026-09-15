//! HTTP backends for the `web_search` tool.
//!
//! The tool talks to exactly one type — the backend-erased
//! [`super::client::WebSearchClient`] — and never learns which wire shape is
//! underneath. Every backend implements [`WebSearchBackend`]:
//!
//! - [`xai`] — the historical `POST {base_url}/responses` Responses-API call
//!   with a server-side `web_search` tool, byte-identical to before.
//! - [`searxng`] — keyless `GET {base}/search?format=json`.
//! - [`tavily`] — `POST {base}/search` with an `Authorization: Bearer` header.
//! - [`brave`] — `GET {base}/res/v1/web/search` with `X-Subscription-Token`.
//!
//! A backend is chosen by [`super::factory::resolve_search_backend`]. A selected
//! backend that is missing its base URL or API key still constructs, and
//! [`WebSearchBackend::is_configured`] explains what to set, so the tool result
//! names the remedy instead of the tool degrading to `Disabled`.

use xai_tool_protocol::ToolId;
use xai_tool_runtime::ToolError;

use crate::attribution::SharedAttributionCallback;
use crate::types::SharedApiKeyProvider;

pub mod brave;
pub mod searxng;
pub mod tavily;
pub mod xai;

pub use brave::BraveBackend;
pub use searxng::SearxngBackend;
pub use tavily::TavilyBackend;
pub use xai::XaiBackend;

/// Text returned when a backend answers successfully but matched nothing.
pub const NO_RESULTS: &str = "No search results found.";

/// Tool id every backend error is attributed to.
pub(crate) fn web_search_tool_id() -> ToolId {
    ToolId::new("web_search").expect("static tool id is valid")
}

/// A backend id accepted by `GROK_SEARCH_PROVIDER` / `[search] provider`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SearchProvider {
    Xai,
    Searxng,
    Tavily,
    Brave,
}

impl SearchProvider {
    /// Every accepted id, in the order error messages list them.
    pub const ALL: [Self; 4] = [Self::Xai, Self::Searxng, Self::Tavily, Self::Brave];

    /// Canonical id, as written in config and in tool errors.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Xai => "xai",
            Self::Searxng => "searxng",
            Self::Tavily => "tavily",
            Self::Brave => "brave",
        }
    }

    /// Parse a provider id. Case-insensitive; `searx` and `brave-search` are
    /// accepted aliases.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "xai" => Some(Self::Xai),
            "searxng" | "searx" => Some(Self::Searxng),
            "tavily" => Some(Self::Tavily),
            "brave" | "brave-search" => Some(Self::Brave),
            _ => None,
        }
    }

    /// Whether the backend cannot run without an API key. SearXNG is keyless.
    pub const fn requires_api_key(self) -> bool {
        !matches!(self, Self::Searxng)
    }
}

/// One search query, backend-independent.
#[derive(Debug, Clone)]
pub struct SearchRequest<'a> {
    pub query: &'a str,
    pub allowed_domains: Option<&'a [String]>,
}

impl SearchRequest<'_> {
    /// `query` with `site:` clauses appended, for engines without a dedicated
    /// domain filter (SearXNG, Brave).
    pub fn query_with_site_filter(&self) -> String {
        let mut query = self.query.to_string();
        for domain in self.allowed_domains.unwrap_or_default() {
            let domain = domain.trim();
            if !domain.is_empty() {
                query.push_str(" site:");
                query.push_str(domain);
            }
        }
        query
    }

    /// Domains to send as an explicit filter (Tavily `include_domains`).
    pub fn domains(&self) -> Vec<String> {
        self.allowed_domains
            .unwrap_or_default()
            .iter()
            .map(|domain| domain.trim().to_string())
            .filter(|domain| !domain.is_empty())
            .collect()
    }
}

/// One search result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    pub title: String,
    pub url: String,
    /// Result text/description; empty when the backend supplies none.
    pub snippet: String,
}

/// What a backend returns: the text handed to the model plus the source list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResponse {
    /// Text handed to the model. For `xai` this is the sampler's synthesis;
    /// for the other backends it is a rendered list of [`Self::hits`].
    pub content: String,
    pub hits: Vec<SearchHit>,
}

impl SearchResponse {
    /// Render a hit list into model-readable text, deduplicating by URL.
    pub fn rendered(hits: Vec<SearchHit>) -> Self {
        let hits = dedupe_hits(hits);
        let content = if hits.is_empty() {
            NO_RESULTS.to_string()
        } else {
            render_hits(&hits)
        };
        Self { content, hits }
    }

    /// Citation URLs, first-seen order, deduplicated.
    pub fn citations(&self) -> Vec<String> {
        self.hits.iter().map(|hit| hit.url.clone()).collect()
    }

    /// `(title, url)` pairs, first-seen order, deduplicated.
    pub fn titled_citations(&self) -> Vec<(String, String)> {
        self.hits
            .iter()
            .map(|hit| (hit.title.clone(), hit.url.clone()))
            .collect()
    }
}

fn dedupe_hits(hits: Vec<SearchHit>) -> Vec<SearchHit> {
    let mut seen = std::collections::HashSet::new();
    hits.into_iter()
        .filter(|hit| !hit.url.is_empty() && seen.insert(hit.url.clone()))
        .collect()
}

fn render_hits(hits: &[SearchHit]) -> String {
    let mut out = String::new();
    for (index, hit) in hits.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        out.push_str(&format!("{}. {}\n", index + 1, hit.title));
        out.push_str(&format!("   {}\n", hit.url));
        let snippet = hit.snippet.trim();
        if !snippet.is_empty() {
            out.push_str(&format!("   {snippet}\n"));
        }
    }
    out.trim_end().to_string()
}

/// Remedy text for a backend that has no base URL.
pub fn missing_base_url_reason(provider: &str) -> String {
    format!(
        "{provider} search backend has no base URL: set GROK_SEARCH_BASE_URL, \
         or `base_url` under `[search]` in config.toml"
    )
}

/// Remedy text for a backend that has no API key.
pub fn missing_api_key_reason(provider: &str) -> String {
    format!(
        "{provider} search backend has no API key: set GROK_SEARCH_API_KEY, \
         or `api_key_env` under `[search]` in config.toml naming the env var that holds it"
    )
}

/// A search wire shape.
///
/// The trait is used through the [`AnyBackend`] enum rather than `dyn`, so the
/// auto-trait bounds of the returned future stay visible to callers.
#[expect(
    async_fn_in_trait,
    reason = "backends are dispatched through AnyBackend, never as `dyn WebSearchBackend`"
)]
pub trait WebSearchBackend: Send + Sync + 'static {
    /// Stable backend id (`xai`, `searxng`, …), used in logs and errors.
    fn name(&self) -> &str;

    /// `Ok(())` when the backend has everything it needs; `Err(reason)` with an
    /// actionable remedy otherwise.
    fn is_configured(&self) -> Result<(), String>;

    /// Run one query.
    async fn search(&self, request: &SearchRequest<'_>) -> Result<SearchResponse, ToolError>;
}

/// A selected provider that cannot run yet: unknown id, or missing settings.
#[derive(Debug, Clone)]
pub struct UnconfiguredBackend {
    name: String,
    reason: String,
}

impl UnconfiguredBackend {
    pub fn new(name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            reason: reason.into(),
        }
    }

    /// `GROK_SEARCH_PROVIDER` (or `[search] provider`) named something unknown.
    pub fn unknown_provider(raw: &str) -> Self {
        let known = SearchProvider::ALL
            .iter()
            .map(|provider| provider.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        Self::new(
            raw.trim(),
            format!("unknown search provider {raw:?}: expected one of {known}"),
        )
    }
}

impl WebSearchBackend for UnconfiguredBackend {
    fn name(&self) -> &str {
        &self.name
    }

    fn is_configured(&self) -> Result<(), String> {
        Err(self.reason.clone())
    }

    async fn search(&self, _request: &SearchRequest<'_>) -> Result<SearchResponse, ToolError> {
        Err(ToolError::execution(
            web_search_tool_id(),
            self.reason.clone(),
        ))
    }
}

/// Concrete dispatch over the known backends.
#[derive(Clone)]
pub enum AnyBackend {
    Xai(XaiBackend),
    Searxng(SearxngBackend),
    Tavily(TavilyBackend),
    Brave(BraveBackend),
    Unconfigured(UnconfiguredBackend),
}

impl AnyBackend {
    /// Forward the 401-attribution hook to the backend that owns one (`xai`).
    /// Other backends never emit xAI 401 attribution.
    pub fn with_attribution_callback(self, callback: Option<SharedAttributionCallback>) -> Self {
        match self {
            Self::Xai(backend) => Self::Xai(backend.with_attribution_callback(callback)),
            other => other,
        }
    }

    /// Whether a 401-attribution callback is installed.
    pub fn has_attribution_callback(&self) -> bool {
        match self {
            Self::Xai(backend) => backend.has_attribution_callback(),
            _ => false,
        }
    }

    /// Build the backend named by `provider`, or an unconfigured placeholder
    /// that explains the unknown id.
    ///
    /// Only fails on an invalid header value; a missing base URL or key is
    /// deferred to [`WebSearchBackend::is_configured`].
    pub fn from_provider(
        provider: &str,
        base_url: &str,
        api_key: Option<String>,
        model: &str,
        extra_headers: indexmap::IndexMap<String, String>,
        api_key_provider: Option<SharedApiKeyProvider>,
    ) -> Result<Self, ToolError> {
        Ok(match SearchProvider::parse(provider) {
            Some(SearchProvider::Xai) => Self::Xai(XaiBackend::new(
                base_url,
                api_key.as_deref().unwrap_or_default(),
                model,
                &extra_headers,
                api_key_provider,
            )?),
            Some(SearchProvider::Searxng) => {
                Self::Searxng(SearxngBackend::new(base_url, &extra_headers)?)
            }
            Some(SearchProvider::Tavily) => {
                Self::Tavily(TavilyBackend::new(base_url, api_key, &extra_headers)?)
            }
            Some(SearchProvider::Brave) => {
                Self::Brave(BraveBackend::new(base_url, api_key, &extra_headers)?)
            }
            None => Self::Unconfigured(UnconfiguredBackend::unknown_provider(provider)),
        })
    }
}

impl WebSearchBackend for AnyBackend {
    fn name(&self) -> &str {
        match self {
            Self::Xai(backend) => backend.name(),
            Self::Searxng(backend) => backend.name(),
            Self::Tavily(backend) => backend.name(),
            Self::Brave(backend) => backend.name(),
            Self::Unconfigured(backend) => backend.name(),
        }
    }

    fn is_configured(&self) -> Result<(), String> {
        match self {
            Self::Xai(backend) => backend.is_configured(),
            Self::Searxng(backend) => backend.is_configured(),
            Self::Tavily(backend) => backend.is_configured(),
            Self::Brave(backend) => backend.is_configured(),
            Self::Unconfigured(backend) => backend.is_configured(),
        }
    }

    async fn search(&self, request: &SearchRequest<'_>) -> Result<SearchResponse, ToolError> {
        // Fully qualified: `XaiBackend` also has an inherent `search(query, …)`.
        match self {
            Self::Xai(backend) => WebSearchBackend::search(backend, request).await,
            Self::Searxng(backend) => WebSearchBackend::search(backend, request).await,
            Self::Tavily(backend) => WebSearchBackend::search(backend, request).await,
            Self::Brave(backend) => WebSearchBackend::search(backend, request).await,
            Self::Unconfigured(backend) => WebSearchBackend::search(backend, request).await,
        }
    }
}

/// Shared HTTP-client construction: the extra-CA bundle hook plus the caller's
/// static headers.
pub(crate) fn http_client_with_headers(
    headers: reqwest::header::HeaderMap,
    provider: &str,
) -> Result<reqwest::Client, ToolError> {
    crate::extra_ca::with_extra_root_certificates(reqwest::Client::builder())
        .default_headers(headers)
        .build()
        .map_err(|error| {
            ToolError::execution(
                web_search_tool_id(),
                format!("Failed to build {provider} HTTP client: {error}"),
            )
        })
}

/// Translate `extra_headers` into a header map, rejecting invalid names/values.
pub(crate) fn headers_from_extra(
    extra_headers: &indexmap::IndexMap<String, String>,
) -> Result<reqwest::header::HeaderMap, ToolError> {
    use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
    let mut headers = HeaderMap::new();
    for (key, value) in extra_headers {
        let header_name = HeaderName::from_bytes(key.as_bytes()).map_err(|error| {
            ToolError::execution(
                web_search_tool_id(),
                format!("Invalid header name '{key}': {error}"),
            )
        })?;
        let header_value = HeaderValue::from_str(value).map_err(|error| {
            ToolError::execution(
                web_search_tool_id(),
                format!("Invalid header value for '{key}': {error}"),
            )
        })?;
        headers.insert(header_name, header_value);
    }
    Ok(headers)
}

/// `{base}{path}` with exactly one separator, tolerating a trailing slash.
pub(crate) fn join_url(base_url: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base_url.trim().trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

/// Read a response body for an error message, never failing the error path.
pub(crate) async fn error_body(response: reqwest::Response) -> String {
    response
        .text()
        .await
        .unwrap_or_else(|_| "Failed to read error body".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(title: &str, url: &str, snippet: &str) -> SearchHit {
        SearchHit {
            title: title.to_string(),
            url: url.to_string(),
            snippet: snippet.to_string(),
        }
    }

    #[test]
    fn provider_ids_round_trip_and_parse_aliases() {
        for provider in SearchProvider::ALL {
            assert_eq!(SearchProvider::parse(provider.as_str()), Some(provider));
        }
        assert_eq!(
            SearchProvider::parse("  SearXNG "),
            Some(SearchProvider::Searxng)
        );
        assert_eq!(
            SearchProvider::parse("searx"),
            Some(SearchProvider::Searxng)
        );
        assert_eq!(SearchProvider::parse("BRAVE"), Some(SearchProvider::Brave));
        assert_eq!(
            SearchProvider::parse("brave-search"),
            Some(SearchProvider::Brave)
        );
        assert_eq!(SearchProvider::parse("duckduckgo"), None);
        assert!(!SearchProvider::Searxng.requires_api_key());
        assert!(SearchProvider::Tavily.requires_api_key());
    }

    #[test]
    fn unknown_provider_names_the_alternatives() {
        let backend = UnconfiguredBackend::unknown_provider("duckduckgo");
        assert_eq!(backend.name(), "duckduckgo");
        let reason = backend.is_configured().unwrap_err();
        assert!(reason.contains("unknown search provider"), "{reason}");
        assert!(reason.contains("searxng, tavily, brave"), "{reason}");
    }

    #[test]
    fn rendered_content_dedupes_and_keeps_order() {
        let response = SearchResponse::rendered(vec![
            hit("First", "https://a.example/", "alpha"),
            hit("Second", "https://b.example/", ""),
            hit("First again", "https://a.example/", "alpha"),
            hit("Empty url", "", "dropped"),
        ]);
        assert_eq!(
            response.citations(),
            ["https://a.example/", "https://b.example/"]
        );
        assert_eq!(
            response.titled_citations(),
            [
                ("First".to_string(), "https://a.example/".to_string()),
                ("Second".to_string(), "https://b.example/".to_string()),
            ]
        );
        assert_eq!(
            response.content,
            "1. First\n   https://a.example/\n   alpha\n\n2. Second\n   https://b.example/"
        );
    }

    #[test]
    fn rendered_content_for_no_hits_is_the_shared_placeholder() {
        assert_eq!(SearchResponse::rendered(vec![]).content, NO_RESULTS);
    }

    #[test]
    fn site_filter_appends_clauses_and_skips_blanks() {
        let domains = vec![
            "example.com".to_string(),
            " ".to_string(),
            "rust-lang.org".into(),
        ];
        let request = SearchRequest {
            query: "async runtime",
            allowed_domains: Some(&domains),
        };
        assert_eq!(
            request.query_with_site_filter(),
            "async runtime site:example.com site:rust-lang.org"
        );
        assert_eq!(request.domains(), ["example.com", "rust-lang.org"]);
    }

    #[test]
    fn site_filter_is_a_no_op_without_domains() {
        let request = SearchRequest {
            query: "async runtime",
            allowed_domains: None,
        };
        assert_eq!(request.query_with_site_filter(), "async runtime");
        assert!(request.domains().is_empty());
    }

    #[test]
    fn join_url_tolerates_trailing_and_leading_slashes() {
        assert_eq!(
            join_url("http://localhost:8888/", "/search"),
            "http://localhost:8888/search"
        );
        assert_eq!(
            join_url("http://localhost:8888", "search"),
            "http://localhost:8888/search"
        );
    }
}
