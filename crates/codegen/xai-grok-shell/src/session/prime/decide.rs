//! Jev decisions for skill prime: one gate `noul`, then one `choice`.
//!
//! The deterministic + retrieval pipeline answers "which candidates are
//! plausible"; a decision answers "is any skill needed at all" and "which one".
//! Measured on the shipped 172-skill roster (`FINDINGS-SKILLS.md` §9), gate then
//! one choice over the shortlist moved injection precision from 21.7% to 86.5%,
//! dropped harmful injection on no-skill requests from 12/12 to 1/12, and cut
//! injected context to a quarter (393 against 1,500 tokens per turn). Blind
//! top-3 injection cannot decline, which is where the 21.7% comes from.
//!
//! Two hard rules from the same measurement set, both encoded here:
//! - **One noul for the gate.** Averaging three questions compresses: 56 of 66
//!   cases landed in the 0.4–0.6 band where a skill exists 100% of the time, so
//!   the mean fired "no skill" on only 2 of 12 uncovered cases. Reverting to the
//!   mean costs 4 top-1 and correct silence drops 11/12 → 8/12.
//! - **A confidence floor on the variant answer.** Routing to a declared child
//!   wins +2 top-1 / +3 top-3, but it has a false-positive mode when the
//!   umbrella *is* the answer; the floor keeps the umbrella in that case.
//!
//! Everything here is fail-open: a missing route, a transport failure, a
//! malformed answer, or a missing field returns `None` and the caller keeps the
//! fused order it already had.

use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;
use xai_grok_config_types::RerankerProtocol;
use xai_grok_inference::{
    DEFAULT_RERANK_PATH, RetrievalCredential, RetrievalTransport, normalize_endpoint_path,
};

use crate::provider_registry::{RetrievalResolveOptions, resolve_reranker_runtime};
use crate::retrieval::RetrievalService;

/// Gate wording. Threshold and wording are tuned as a pair; do not edit one
/// without the other (added prose is unreliable — five attempts, one win).
pub const GATE_QUESTION: &str = "Would a careful expert answering this request consult a specific documented procedure or set of commands, rather than answering from general knowledge alone?";

/// Selection wording. The trailing hint is part of the measured shape; the
/// ablation found it neutral in the stacked pipeline, so it is not a lever.
pub const SELECT_QUESTION: &str = "Which of these skills is the right one to load for this request? When a broad umbrella skill and a specific one both cover the request, prefer the specific one.";

/// Second question, asked only over the umbrella's own declared children.
pub const VARIANT_QUESTION: &str = "This request matches the topic area, but the topic has several implementation variants. Which variant is the right one for this specific request?";

/// Gate threshold for the single `noul`. A "is this worth it" noul sits at
/// 0.20–0.40, never 0.50; 0.40 is the measured operating point.
pub const GATE_THRESHOLD: f64 = 0.40;

/// Floor on the variant answer's confidence. Unmeasured (the ablation ran
/// without one) and deliberately conservative: below it the umbrella stands,
/// which is exactly today's behaviour.
pub const VARIANT_CONFIDENCE_FLOOR: f64 = 0.60;

/// Options sent in one `choice` question. The measured shortlist is 10.
pub const MAX_DECISION_OPTIONS: usize = 10;

/// A answered `choice`: the chosen option key plus its confidence.
#[derive(Debug, Clone, PartialEq)]
pub struct Chosen {
    pub key: String,
    pub confidence: f64,
}

/// Build the gate request body. One question, no criteria — the wording is the
/// criterion.
pub fn gate_body(model: &str, query: &str) -> Value {
    json!({
        "model": model,
        "state": { "request": query },
        "questions": { "gate": { "type": "noul", "instructions": GATE_QUESTION } },
    })
}

/// Build a `choice` request body over `options` (key → index text).
///
/// Option keys are the option labels the answer echoes back. `serde_json`
/// objects are sorted by key, so the options reach the wire in name order, not
/// shortlist order (documented deviation from the measured harness).
pub fn choice_body(
    model: &str,
    query: &str,
    question: &str,
    options: &[(String, String)],
) -> Value {
    let criteria: serde_json::Map<String, Value> = options
        .iter()
        .map(|(key, text)| (key.clone(), Value::String(text.clone())))
        .collect();
    json!({
        "model": model,
        "state": { "request": query },
        "questions": { "which": { "type": "choice", "instructions": question, "criteria": criteria } },
    })
}

/// Probability that the gate answered yes. `None` on a missing/malformed answer.
pub fn parse_noul(answers: &Value, key: &str) -> Option<f64> {
    let value = answers.get(key)?.get("noul")?.as_f64()?;
    value.is_finite().then_some(value)
}

/// The chosen option key and its confidence. `None` when the answer is absent,
/// malformed, or names an option that was not offered.
pub fn parse_choice(answers: &Value, key: &str, offered: &[String]) -> Option<Chosen> {
    let answer = answers.get(key)?;
    let chosen = answer.get("choice")?.as_str()?.to_owned();
    if !offered.iter().any(|o| o == &chosen) {
        return None;
    }
    let confidence = answer
        .get("confidence")
        .and_then(Value::as_f64)
        .filter(|c| c.is_finite())
        .unwrap_or(0.0);
    Some(Chosen {
        key: chosen,
        confidence,
    })
}

/// Declared children of an umbrella skill (`metadata.variants`), in declaration
/// order. 27 of 172 shipped skills declare them; nothing read the field before.
pub fn declared_variants(
    skill: &xai_grok_tools::implementations::skills::types::SkillInfo,
) -> Vec<String> {
    let Some(raw) = skill.metadata.as_ref().and_then(|m| m.get("variants")) else {
        return Vec::new();
    };
    let mut seen = std::collections::HashSet::new();
    raw.split(',')
        .map(|v| v.trim().trim_matches('"').trim())
        .filter(|v| !v.is_empty() && seen.insert(v.to_ascii_lowercase()))
        .map(str::to_owned)
        .take(MAX_DECISION_OPTIONS)
        .collect()
}

/// A resolved Jev decisions route plus its short-lived credential.
pub struct PrimeDecider {
    transport: RetrievalTransport,
    credential: RetrievalCredential,
    model: String,
    endpoint: String,
}

impl PrimeDecider {
    /// Resolve the decisions route from the profile's reranker routes.
    ///
    /// A Jev protocol route is what makes decisions available; the same route
    /// must therefore not also be used as a cross-encoder rerank (see
    /// [`PrimeDecider::is_decisions_profile`]). `None` when the profile declares
    /// no Jev route or the exact route cannot be resolved — both fail open.
    pub fn from_profile(service: &RetrievalService, profile_id: &str, home: &Path) -> Option<Self> {
        let snapshot = service.load_snapshot();
        let profile = snapshot.profile(profile_id)?;
        let route_id = profile.reranker_route_ids.iter().find(|id| {
            snapshot
                .reranker_models
                .get(*id)
                .is_some_and(|route| route.config.protocol == RerankerProtocol::Jev)
        })?;
        let config = &snapshot.reranker_models.get(route_id)?.config;
        let opts = RetrievalResolveOptions {
            total_deadline: Some(Duration::from_millis(profile.config.deadline_ms.max(1))),
            ..RetrievalResolveOptions::default()
        };
        let runtime = resolve_reranker_runtime(home, config, &opts, None).ok()?;
        let transport = RetrievalTransport::from_route(&runtime.route).ok()?;
        let endpoint = runtime
            .rerank_endpoint
            .as_deref()
            .map(normalize_endpoint_path)
            .unwrap_or_else(|| DEFAULT_RERANK_PATH.to_owned());
        Some(Self {
            transport,
            credential: runtime.credential,
            model: runtime.upstream_model,
            endpoint,
        })
    }

    /// True when this profile's reranker slot is a decisions route rather than a
    /// cross-encoder. The caller then skips the noul-per-document rerank: the
    /// gate and the choice already ask the model about the shortlist, and
    /// stacking both measured negative.
    pub fn is_decisions_profile(service: &RetrievalService, profile_id: &str) -> bool {
        let snapshot = service.load_snapshot();
        snapshot.profile(profile_id).is_some_and(|profile| {
            profile.reranker_route_ids.iter().any(|id| {
                snapshot
                    .reranker_models
                    .get(id)
                    .is_some_and(|route| route.config.protocol == RerankerProtocol::Jev)
            })
        })
    }

    /// One decision request. `None` on any failure (never an error to the caller).
    async fn ask(&self, body: Value, cancel: &CancellationToken) -> Option<Value> {
        let raw = self
            .transport
            .post_json(
                &self.endpoint,
                &body,
                &self.credential,
                cancel,
                "createDecision",
            )
            .await
            .ok()?;
        raw.get("answers").cloned()
    }

    /// Gate: should any skill be loaded for this request?
    pub async fn gate(&self, query: &str, cancel: &CancellationToken) -> Option<f64> {
        let answers = self.ask(gate_body(&self.model, query), cancel).await?;
        parse_noul(&answers, "gate")
    }

    /// One choice over `options` (key → index text). `None` fails open.
    pub async fn choose(
        &self,
        query: &str,
        question: &str,
        options: &[(String, String)],
        cancel: &CancellationToken,
    ) -> Option<Chosen> {
        if options.len() < 2 {
            return None;
        }
        let keys: Vec<String> = options.iter().map(|(k, _)| k.clone()).collect();
        let answers = self
            .ask(choice_body(&self.model, query, question, options), cancel)
            .await?;
        parse_choice(&answers, "which", &keys)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xai_grok_tools::implementations::skills::types::SkillInfo;

    fn options() -> Vec<(String, String)> {
        vec![
            ("dns".to_owned(), "dns: hub".to_owned()),
            ("bind9-dns".to_owned(), "bind9-dns: BIND".to_owned()),
        ]
    }

    #[test]
    fn gate_body_carries_one_question_and_the_request_as_state() {
        let body = gate_body("jev-latest", "set up bind");
        assert_eq!(body["questions"]["gate"]["type"], json!("noul"));
        assert_eq!(
            body["questions"]["gate"]["instructions"],
            json!(GATE_QUESTION)
        );
        assert!(body["questions"]["gate"].get("criteria").is_none());
        assert_eq!(body["state"]["request"], json!("set up bind"));
    }

    #[test]
    fn choice_body_keys_options_by_label() {
        let body = choice_body("m", "q", SELECT_QUESTION, &options());
        assert_eq!(body["questions"]["which"]["type"], json!("choice"));
        assert_eq!(
            body["questions"]["which"]["criteria"]["dns"],
            json!("dns: hub")
        );
        assert_eq!(
            body["questions"]["which"]["instructions"],
            json!(SELECT_QUESTION)
        );
    }

    #[test]
    fn parse_noul_rejects_missing_and_non_finite() {
        assert_eq!(
            parse_noul(&json!({"gate": {"noul": 0.41}}), "gate"),
            Some(0.41)
        );
        assert_eq!(parse_noul(&json!({"gate": {"type": "noul"}}), "gate"), None);
        assert_eq!(parse_noul(&json!({"gate": {"noul": "x"}}), "gate"), None);
        assert_eq!(
            parse_noul(&json!({"gate": {"noul": f64::NAN}}), "gate"),
            None
        );
        assert_eq!(parse_noul(&json!({}), "gate"), None);
    }

    #[test]
    fn parse_choice_requires_an_offered_key_and_reads_confidence() {
        let keys: Vec<String> = options().into_iter().map(|(k, _)| k).collect();
        let ok = json!({"which": {"type": "choice", "choice": "bind9-dns", "confidence": 0.8}});
        assert_eq!(
            parse_choice(&ok, "which", &keys),
            Some(Chosen {
                key: "bind9-dns".to_owned(),
                confidence: 0.8
            })
        );
        let off_menu = json!({"which": {"choice": "coredns"}});
        assert_eq!(parse_choice(&off_menu, "which", &keys), None);
        // Absent confidence reads as 0.0, which is below the variant floor.
        let bare = json!({"which": {"choice": "dns"}});
        assert_eq!(parse_choice(&bare, "which", &keys).unwrap().confidence, 0.0);
    }

    #[test]
    fn declared_variants_split_trim_dedupe_and_cap() {
        let mut skill = SkillInfo::default();
        assert!(declared_variants(&skill).is_empty());
        skill.metadata = Some(std::collections::HashMap::from([(
            "variants".to_owned(),
            "\"bind9-dns, coredns-dns ,, BIND9-DNS\"".to_owned(),
        )]));
        assert_eq!(declared_variants(&skill), vec!["bind9-dns", "coredns-dns"]);
    }
}
