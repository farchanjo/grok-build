//! vLLM / OpenAI-compatible handwritten rerank adapter.
//!
//! Targets PR15 relative endpoints (default `/rerank`) on compatible and
//! retrieval-only providers with explicit rerank capability. Does not trust
//! optional returned document text as authoritative identity.

use std::collections::HashSet;

use serde::Deserialize;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use super::transport::{RetrievalCredential, RetrievalTransport};
use super::types::{
    RerankAdapter, RerankHit, RerankRequest, RerankResult, RetrievalError, RetrievalResult,
    RetrievalRouteContext, normalize_endpoint_path, validate_rerank_request,
};

/// Typed vLLM-style rerank client bound to one exact route.
#[derive(Debug, Clone)]
pub struct VllmRerankAdapter {
    route: RetrievalRouteContext,
    transport: RetrievalTransport,
}

impl VllmRerankAdapter {
    pub fn new(route: RetrievalRouteContext) -> RetrievalResult<Self> {
        let transport = RetrievalTransport::from_route(&route)?;
        Ok(Self { route, transport })
    }

    pub async fn rerank(
        &self,
        request: RerankRequest,
        credential: &RetrievalCredential,
        cancel: CancellationToken,
    ) -> RetrievalResult<RerankResult> {
        validate_rerank_request(&request)?;
        let endpoint = normalize_endpoint_path(&request.endpoint);
        let mut body = json!({
            "model": request.model,
            "query": request.query,
            "documents": request.documents,
        });
        if let Some(top_n) = request.top_n {
            body.as_object_mut()
                .expect("object")
                .insert("top_n".into(), json!(top_n));
        }
        // Some vLLM builds accept `return_documents` / `return_text`.
        if request.return_documents {
            body.as_object_mut()
                .expect("object")
                .insert("return_documents".into(), json!(true));
        }

        let raw = self
            .transport
            .post_json(&endpoint, &body, credential, &cancel, "createRerank")
            .await?;
        parse_vllm_rerank_response(&raw, request.documents.len(), request.top_n, &request.model)
    }
}

impl RerankAdapter for VllmRerankAdapter {
    async fn rerank(
        &self,
        request: RerankRequest,
        credential: &RetrievalCredential,
        cancel: CancellationToken,
    ) -> RetrievalResult<RerankResult> {
        VllmRerankAdapter::rerank(self, request, credential, cancel).await
    }

    fn route_context(&self) -> &RetrievalRouteContext {
        &self.route
    }
}

#[derive(Debug, Deserialize)]
struct WireRerankResponse {
    #[serde(default)]
    results: Vec<WireRerankItem>,
    #[serde(default)]
    model: Option<String>,
    /// Some servers return a flat `id` + results without nesting.
    #[serde(default)]
    data: Option<Vec<WireRerankItem>>,
}

#[derive(Debug, Deserialize)]
struct WireRerankItem {
    index: i64,
    #[serde(alias = "relevance_score")]
    score: f64,
    #[serde(default)]
    document: Option<WireDocument>,
}

#[derive(Debug, Deserialize)]
struct WireDocument {
    #[serde(default)]
    text: Option<String>,
}

/// Pure response parser for tests and adapters.
pub fn parse_vllm_rerank_response(
    raw: &Value,
    document_count: usize,
    top_n: Option<u32>,
    fallback_model: &str,
) -> RetrievalResult<RerankResult> {
    let parsed: WireRerankResponse = serde_json::from_value(raw.clone())
        .map_err(|e| RetrievalError::MalformedResponse(format!("rerank envelope: {e}")))?;
    let items = if !parsed.results.is_empty() {
        parsed.results
    } else {
        parsed.data.unwrap_or_default()
    };
    if items.is_empty() {
        return Err(RetrievalError::MalformedResponse(
            "rerank response results are empty".into(),
        ));
    }
    let max_results = top_n
        .map(|n| n as usize)
        .unwrap_or(document_count)
        .min(document_count);
    if items.len() > max_results && top_n.is_some() {
        // Allow servers to return more; we clamp below. Hard-fail only on
        // absurd overshoot beyond document_count.
    }
    if items.len() > document_count {
        return Err(RetrievalError::MalformedResponse(format!(
            "rerank result count {} exceeds document count {document_count}",
            items.len()
        )));
    }

    let mut seen = HashSet::new();
    let mut hits = Vec::with_capacity(items.len());
    for item in items {
        if item.index < 0 || item.index as usize >= document_count {
            return Err(RetrievalError::MalformedResponse(format!(
                "rerank index {} out of range 0..{document_count}",
                item.index
            )));
        }
        let idx = item.index as usize;
        if !seen.insert(idx) {
            return Err(RetrievalError::MalformedResponse(format!(
                "duplicate rerank index {idx}"
            )));
        }
        if !item.score.is_finite() {
            return Err(RetrievalError::MalformedResponse(format!(
                "rerank score at index {idx} is non-finite"
            )));
        }
        // Optional document is informational only; never trusted as identity.
        let document = item.document.and_then(|d| d.text);
        hits.push(RerankHit {
            index: idx,
            score: item.score as f32,
            document,
        });
    }

    if let Some(top_n) = top_n {
        hits.truncate(top_n as usize);
    }

    Ok(RerankResult {
        model: parsed
            .model
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| fallback_model.to_owned()),
        hits,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_unique_indices() {
        let raw = json!({
            "model": "bge-reranker",
            "results": [
                {"index": 2, "relevance_score": 0.9, "document": {"text": "c"}},
                {"index": 0, "score": 0.1}
            ]
        });
        let res = parse_vllm_rerank_response(&raw, 3, Some(2), "fallback").unwrap();
        assert_eq!(res.hits.len(), 2);
        assert_eq!(res.hits[0].index, 2);
        assert!((res.hits[0].score - 0.9).abs() < 1e-5);
        assert_eq!(res.hits[0].document.as_deref(), Some("c"));
        assert_eq!(res.model, "bge-reranker");
    }

    #[test]
    fn rejects_oob_duplicate_nonfinite() {
        let oob = json!({"results": [{"index": 9, "score": 0.5}]});
        assert!(parse_vllm_rerank_response(&oob, 2, None, "m").is_err());
        let dup = json!({"results": [
            {"index": 0, "score": 0.5},
            {"index": 0, "score": 0.1}
        ]});
        assert!(parse_vllm_rerank_response(&dup, 2, None, "m").is_err());
        let nan = json!({"results": [{"index": 0, "score": f64::NAN}]});
        assert!(parse_vllm_rerank_response(&nan, 1, None, "m").is_err());
    }
}
