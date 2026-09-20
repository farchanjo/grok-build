//! Jev (TypeSafe decisions) rerank adapter.
//!
//! Jev is neither OpenAI-compatible nor Cohere-compatible: it posts
//! `{model, state, questions}` and answers `{answers: {<question_id>: {...}}}`.
//! One `noul` question is rendered per candidate, keyed by the candidate's
//! **index** — never by candidate name, because two candidates can share a
//! name and a name-keyed answer would collide.
//!
//! The measured axis (`FINDINGS-SKILLS.md` §8, `FINDINGS-TOOLS.md` §3) found the
//! reranker neutral-to-negative six times, so the slot is wireable but expected
//! to stay empty. The adapter therefore inherits the profile deadline, ordered
//! fallback and attempt budget unchanged — a Jev failure degrades to the
//! pre-rerank order with no second fallback path.

use serde::Deserialize;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use super::transport::{RetrievalCredential, RetrievalTransport};
use super::types::{
    RerankAdapter, RerankHit, RerankRequest, RerankResult, RetrievalError, RetrievalResult,
    RetrievalRouteContext, normalize_endpoint_path, validate_rerank_request,
};

/// Question asked once per candidate, with [`JEV_CANDIDATE_PREFIX`] and the
/// candidate text appended.
///
/// Verbatim from the rerank measurement (`script-test/sim_memory_rerank.py`:
/// top-1 7/10 → 9/10, gold in top-3 10/10). The harness labelled each candidate
/// `Chunk (scope: …):`; this writes `Candidate:` — the one deviation, because the
/// same adapter also reranks skills and tools. The state carries
/// `workspace_path` whenever the caller knows the folder — the gate route
/// always does; the retrieval route only when it supplies one.
pub const JEV_RERANK_QUESTION: &str = "The user is working in the folder given in the state. Is this memory chunk one they would want surfaced for the query below?";
/// Candidate label appended after [`JEV_RERANK_QUESTION`].
pub const JEV_CANDIDATE_PREFIX: &str = "Candidate: ";

/// `true` criterion wording (the operating knob, not the threshold), verbatim
/// from the same measurement.
pub const JEV_TRUE_CRITERION: &str =
    "It answers the query, and it applies where the user is working right now.";
/// `false` criterion wording.
pub const JEV_FALSE_CRITERION: &str = "It is only topically near, or it is about a different project and the query is about this one.";

/// Typed Jev decisions client bound to one exact route.
#[derive(Debug, Clone)]
pub struct JevRerankAdapter {
    route: RetrievalRouteContext,
    transport: RetrievalTransport,
}

impl JevRerankAdapter {
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
        let body = build_jev_rerank_body(&request);
        let raw = self
            .transport
            .post_json(&endpoint, &body, credential, &cancel, "createRerank")
            .await?;
        parse_jev_rerank_response(&raw, request.documents.len(), request.top_n, &request.model)
    }
}

impl RerankAdapter for JevRerankAdapter {
    async fn rerank(
        &self,
        request: RerankRequest,
        credential: &RetrievalCredential,
        cancel: CancellationToken,
    ) -> RetrievalResult<RerankResult> {
        JevRerankAdapter::rerank(self, request, credential, cancel).await
    }

    fn route_context(&self) -> &RetrievalRouteContext {
        &self.route
    }
}

/// Render one `noul` question per candidate, keyed by document index.
pub fn build_jev_rerank_body(request: &RerankRequest) -> Value {
    let questions: serde_json::Map<String, Value> = request
        .documents
        .iter()
        .enumerate()
        .map(|(index, document)| {
            (
                index.to_string(),
                json!({
                    "type": "noul",
                    "instructions": format!(
                        "{JEV_RERANK_QUESTION}\n\n{JEV_CANDIDATE_PREFIX}{document}"
                    ),
                    "criteria": {
                        "true": JEV_TRUE_CRITERION,
                        "false": JEV_FALSE_CRITERION,
                    },
                }),
            )
        })
        .collect();
    json!({
        "model": request.model,
        "state": state_with_query_and_workspace(request),
        "questions": questions,
    })
}

/// Request state. `query` always travels; `workspace_path` travels when the
/// caller knows the folder (blank counts as unknown), because
/// [`JEV_RERANK_QUESTION`] reads the folder from the state.
fn state_with_query_and_workspace(request: &RerankRequest) -> Value {
    let mut state = serde_json::Map::new();
    state.insert("query".to_owned(), json!(request.query));
    if let Some(workspace_path) = request
        .workspace_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
    {
        state.insert("workspace_path".to_owned(), json!(workspace_path));
    }
    Value::Object(state)
}

#[derive(Debug, Deserialize)]
struct WireAnswer {
    #[serde(default)]
    noul: Option<f64>,
    #[serde(default)]
    score: Option<f64>,
}

/// Pure response parser for tests and the adapter.
///
/// An answer key that is not a document index is malformed; a missing answer
/// for an in-range index is skipped (Jev may answer a subset).
pub fn parse_jev_rerank_response(
    raw: &Value,
    document_count: usize,
    top_n: Option<u32>,
    fallback_model: &str,
) -> RetrievalResult<RerankResult> {
    let answers = raw
        .get("answers")
        .and_then(Value::as_object)
        .ok_or_else(|| RetrievalError::MalformedResponse("missing `answers` object".into()))?;
    if answers.is_empty() {
        return Err(RetrievalError::MalformedResponse(
            "`answers` object is empty".into(),
        ));
    }

    let mut hits: Vec<RerankHit> = Vec::with_capacity(answers.len());
    for (key, body) in answers {
        let index: usize = key.parse().map_err(|_| {
            RetrievalError::MalformedResponse(format!("answer key `{key}` is not a document index"))
        })?;
        if index >= document_count {
            return Err(RetrievalError::MalformedResponse(format!(
                "answer index {index} out of range 0..{document_count}"
            )));
        }
        let parsed: WireAnswer = serde_json::from_value(body.clone()).map_err(|e| {
            RetrievalError::MalformedResponse(format!("answer `{key}` envelope: {e}"))
        })?;
        let score = parsed.noul.or(parsed.score).ok_or_else(|| {
            RetrievalError::MalformedResponse(format!("answer `{key}` carries no noul/score"))
        })?;
        if !score.is_finite() {
            return Err(RetrievalError::MalformedResponse(format!(
                "answer `{key}` score is non-finite"
            )));
        }
        hits.push(RerankHit {
            index,
            score: score as f32,
            document: None,
        });
    }

    // Descending score; index ascending breaks ties deterministically.
    hits.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.index.cmp(&b.index))
    });
    if let Some(top_n) = top_n {
        hits.truncate(top_n as usize);
    }
    Ok(RerankResult {
        model: fallback_model.to_owned(),
        hits,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> RerankRequest {
        RerankRequest {
            model: "~typesafe/jev-latest".into(),
            query: "find the issue about login".into(),
            documents: vec!["list issues".into(), "search issues".into()],
            top_n: None,
            endpoint: "alpha/decisions".into(),
            return_documents: false,
            workspace_path: None,
        }
    }

    #[test]
    fn body_keys_questions_by_index_and_carries_the_query_in_state() {
        let body = build_jev_rerank_body(&request());
        assert_eq!(body["model"], json!("~typesafe/jev-latest"));
        assert_eq!(body["state"]["query"], json!("find the issue about login"));
        // Unknown folder: the key stays out rather than sending an empty one.
        assert!(body["state"].get("workspace_path").is_none());
        assert_eq!(body["questions"]["0"]["type"], json!("noul"));
        let instruction = body["questions"]["0"]["instructions"].as_str().unwrap();
        // The measured question leads; the candidate text follows the label.
        assert!(
            instruction.starts_with(JEV_RERANK_QUESTION),
            "{instruction}"
        );
        assert!(instruction.contains("list issues"), "{instruction}");
        assert_eq!(
            body["questions"]["1"]["criteria"]["true"],
            JEV_TRUE_CRITERION
        );
        assert_eq!(
            body["questions"]["1"]["criteria"]["false"],
            JEV_FALSE_CRITERION
        );
    }

    #[test]
    fn body_carries_the_workspace_path_in_the_state_when_known() {
        let mut req = request();
        req.workspace_path = Some("/w/project".into());
        let body = build_jev_rerank_body(&req);
        assert_eq!(body["state"]["workspace_path"], json!("/w/project"));
        assert_eq!(body["state"]["query"], json!("find the issue about login"));

        // Blank counts as unknown, mirroring the credential handling.
        req.workspace_path = Some("   ".into());
        let body = build_jev_rerank_body(&req);
        assert!(body["state"].get("workspace_path").is_none());
    }

    #[test]
    fn duplicate_candidate_names_do_not_collide() {
        let mut req = request();
        req.documents = vec!["fetch".into(), "fetch".into()];
        let body = build_jev_rerank_body(&req);
        assert!(body["questions"]["0"].is_object());
        assert!(body["questions"]["1"].is_object());
    }

    #[test]
    fn parser_orders_by_probability_and_truncates_to_top_n() {
        let raw = json!({
            "answers": {
                "1": {"type": "noul", "noul": 0.9},
                "0": {"type": "noul", "noul": 0.1},
            }
        });
        let res = parse_jev_rerank_response(&raw, 2, Some(1), "fallback").unwrap();
        assert_eq!(res.hits.len(), 1);
        assert_eq!(res.hits[0].index, 1);
        assert!((res.hits[0].score - 0.9).abs() < 1e-5);
        assert_eq!(res.model, "fallback");
    }

    #[test]
    fn parser_accepts_score_answers_and_skips_absent_indices() {
        let raw = json!({"answers": {"1": {"type": "score", "score": 0.4}}});
        let res = parse_jev_rerank_response(&raw, 3, None, "m").unwrap();
        assert_eq!(res.hits.len(), 1);
        assert_eq!(res.hits[0].index, 1);
    }

    #[test]
    fn parser_rejects_oob_non_index_and_empty() {
        let oob = json!({"answers": {"5": {"noul": 0.5}}});
        assert!(parse_jev_rerank_response(&oob, 2, None, "m").is_err());
        let named = json!({"answers": {"save_issue": {"noul": 0.5}}});
        assert!(parse_jev_rerank_response(&named, 2, None, "m").is_err());
        let empty = json!({"answers": {}});
        assert!(parse_jev_rerank_response(&empty, 2, None, "m").is_err());
        let missing = json!({});
        assert!(parse_jev_rerank_response(&missing, 2, None, "m").is_err());
    }

    #[test]
    fn parser_rejects_non_finite_and_answerless_bodies() {
        let nan = json!({"answers": {"0": {"noul": f64::NAN}}});
        assert!(parse_jev_rerank_response(&nan, 1, None, "m").is_err());
        let bare = json!({"answers": {"0": {"type": "noul"}}});
        assert!(parse_jev_rerank_response(&bare, 1, None, "m").is_err());
    }
}
