//! Write-side admission gate: worth, coverage and scope routing.
//!
//! Every note that reaches the append seam is asked three atomic questions
//! before it is written: whether it is `worth` recalling on its own merits,
//! whether memory already `covered` it, and which `bucket` it belongs to
//! (`this_folder`, `global` or `discard`). One question per candidate, never a
//! combined "does this fit" question — a conflated question lands mid-scale.
//!
//! Measured on the 24-candidate sample (`script-test/FINDINGS-MEMORY.md` §4.2):
//! two atomic questions with the store in the state reach 100% precision and
//! 100% recall, and scope routing reaches 82.4% with every error leaning local.
//! **The store must be in the state** — without it the same gate re-stores a
//! restatement 6 times out of 7; with it, zero.
//!
//! Everything here is fail-open: a missing credential, an HTTP failure, a
//! malformed answer or a missing field reproduces today's behaviour exactly —
//! store the note, in the historical scope.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{Value, json};

use xai_grok_inference::retrieval::jev_rerank::{
    JEV_CANDIDATE_PREFIX, JEV_FALSE_CRITERION, JEV_RERANK_QUESTION, JEV_TRUE_CRITERION,
};

/// Log target, matching `xai_grok_telemetry::memory_log::TARGET`.
const LOG: &str = "xai_memory";

/// Threshold on the `worth` answer.
///
/// The band is 0.20–0.22: noise scores 0.05–0.23 and valuable notes 0.20–0.86,
/// so the overlap is three points wide. Sweeping the threshold over the
/// 48-event sample: 0.10 admits 7 noise items, 0.20 loses none of the 36
/// valuable notes, 0.25 loses one, and the intuitive coin flip 0.50 loses
/// seven. Do not "round" this to 0.5.
pub const WORTH_THRESHOLD: f64 = 0.20;

/// Threshold on the `covered` answer; at or above it the candidate is a
/// restatement and is dropped.
///
/// 0.50 separates a restatement from a note that adds something: the
/// restatements score above it and the notes that carry new information score
/// below it, on the same 24-candidate sample.
pub const COVERED_THRESHOLD: f64 = 0.50;

/// Default decisions endpoint (the OpenRouter alpha surface).
pub const DEFAULT_ENDPOINT: &str = "https://openrouter.ai/api/alpha/decisions";
/// Default model reference. A plain string: the Jev model is not a catalog entry.
pub const DEFAULT_MODEL: &str = "~typesafe/jev-latest";
/// Per-request timeout in milliseconds. A decision measures ~300 ms.
pub const DEFAULT_TIMEOUT_MS: u64 = 8_000;
/// Entries carried in the state before the store degrades to a summary.
pub const DEFAULT_STORE_MAX_ENTRIES: usize = 40;
/// Characters carried in the state before the store degrades to a summary.
pub const DEFAULT_STORE_MAX_CHARS: usize = 6_000;
/// Characters kept per store entry.
pub const DEFAULT_ENTRY_CHARS: usize = 240;

/// Environment variable consulted between `api_key_env` and the caller's key.
pub const JEV_API_KEY_ENV: &str = "GROK_JEV_API_KEY";

/// State context sent with every admission request. The field names are this
/// implementation's (`candidate`, `store`), not the harness's (`candidate_note`,
/// `already_in_memory`), so the line doubles as the field explainer; the
/// questions and their criteria are the measured wording verbatim.
pub const GATE_CONTEXT: &str = "A coding assistant is deciding whether to save one note to \
     long-term memory. `workspace_path` is the folder the session runs in, `candidate` is the \
     note, and `store` holds what memory already says. The note is written only when every \
     question agrees.";

/// `worth` question, verbatim as measured.
pub const WORTH_QUESTION: &str =
    "Would this note still be worth recalling in a future session, on its own merits?";
/// `worth` true criterion, verbatim from the same measurement
/// (`script-test/sim_memory_scope.py`, decomposed arm: 83.3% accuracy, 100%
/// precision, 100% recall, 0/7 restatements stored).
pub const WORTH_TRUE: &str = "Durable fact, rule, preference, or verified result";
/// `worth` false criterion.
pub const WORTH_FALSE: &str = "Momentary state or trivially rediscoverable";

/// `covered` question, verbatim as measured.
pub const COVERED_QUESTION: &str = "Do the entries already in memory already say this, or \
     something close enough that storing it again adds nothing?";
/// `covered` true criterion, verbatim from the same arm.
pub const COVERED_TRUE: &str = "Already said, or near enough that a second copy adds nothing";
/// `covered` false criterion.
pub const COVERED_FALSE: &str = "New: nothing above covers it";

/// Routing question and options, verbatim from the same script's routing arm
/// (82.4% over 34 candidates, 10/10 local correct). Deliberately no tie-break
/// sentence: adding one measured worse (82.4% → 73.5%). A `choice` carries its
/// options in `criteria`, the key the harness sent.
pub const BUCKET_QUESTION: &str = "A note is being filed into long-term memory. Which bucket does it belong in? The agent is working in the folder given in the state.";
/// Routing option: the current folder's `MEMORY.md`.
pub const BUCKET_THIS_FOLDER: &str = "True only of this repository: its layout, its conventions, a decision taken here, a command that works here and would not transfer to another project.";
/// Routing option: the global `MEMORY.md`.
pub const BUCKET_GLOBAL: &str = "True of the user or the machine everywhere: a working preference, a tool they always use, a rule about how they want to be talked to, a credential location. Also a fact about a different project that this folder has no claim on.";
/// Routing option: no write.
pub const BUCKET_DISCARD: &str =
    "Momentary state, trivially rediscoverable, or already implied by something else.";

/// Scope a kept note is written to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteScope {
    /// The workspace `MEMORY.md` of the session's folder.
    ThisFolder,
    /// The global `MEMORY.md`.
    Global,
}

/// Why a candidate was not written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropReason {
    /// `worth` scored below the threshold.
    NotWorth,
    /// `covered` scored at or above the threshold.
    Covered,
    /// The routing question answered `discard`.
    Discarded,
}

impl DropReason {
    /// Stable telemetry label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotWorth => "not_worth",
            Self::Covered => "covered",
            Self::Discarded => "discarded",
        }
    }
}

/// Terminal state of one admission decision.
#[derive(Debug, Clone, PartialEq)]
pub enum GateOutcome {
    /// The gate is off; the caller writes exactly as it did before.
    Disabled,
    /// Write the candidate to `scope`.
    Stored {
        scope: WriteScope,
        worth: f64,
        covered: f64,
    },
    /// Do not write; the reason is for the log line only.
    Dropped {
        reason: DropReason,
        worth: f64,
        covered: f64,
    },
    /// Any failure. The caller writes exactly as it did before.
    Failed { error: String },
}

impl GateOutcome {
    /// Stable telemetry label.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Stored { .. } => "stored",
            Self::Dropped { .. } => "dropped",
            Self::Failed { .. } => "failed_fallback",
        }
    }

    /// Whether the caller should write the candidate.
    pub const fn is_stored(&self) -> bool {
        matches!(self, Self::Stored { .. })
    }
}

/// One note offered to the gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Candidate<'a> {
    /// The note text, as the user wrote it.
    pub text: &'a str,
    /// Folder the session runs in; the routing question is relative to it.
    pub workspace_path: &'a str,
}

/// Resolved gate policy. [`Self::default`] is disabled — with the gate off
/// nothing changes on the write path.
#[derive(Debug, Clone, PartialEq)]
pub struct GateConfig {
    /// Master switch. `false` makes zero requests.
    pub enabled: bool,
    /// Route memory search reranking through this gate's decisions client
    /// instead of the retrieval protocol slot. Works even with `enabled =
    /// false`: this picks the owner of the search call, not the append gate.
    /// Both routes ask the same questions, so the switch is a choice of owner,
    /// not of order.
    pub rerank: bool,
    /// Decisions endpoint.
    pub endpoint: String,
    /// Model reference sent in the request body.
    pub model: String,
    /// Environment variable holding the credential, when the caller wants one.
    pub api_key_env: Option<String>,
    /// OpenRouter `provider.zdr`.
    pub zdr: Option<bool>,
    /// OpenRouter `provider.data_collection`.
    pub data_collection: Option<String>,
    /// OpenRouter `provider.require_parameters`.
    pub require_parameters: Option<bool>,
    /// Minimum `worth` for a note to be kept.
    pub worth_threshold: f64,
    /// Minimum `covered` for a note to be dropped as a restatement.
    pub covered_threshold: f64,
    /// Per-request timeout in milliseconds.
    pub timeout_ms: u64,
    /// Entries in the state before it degrades to a summary.
    pub store_max_entries: usize,
    /// Characters in the state before it degrades to a summary.
    pub store_max_chars: usize,
    /// Characters kept per store entry.
    pub entry_chars: usize,
}

impl Default for GateConfig {
    fn default() -> Self {
        Self::disabled()
    }
}

impl GateConfig {
    /// Disabled policy carrying the measured defaults.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            rerank: false,
            endpoint: DEFAULT_ENDPOINT.to_owned(),
            model: DEFAULT_MODEL.to_owned(),
            api_key_env: None,
            zdr: None,
            data_collection: None,
            require_parameters: None,
            worth_threshold: WORTH_THRESHOLD,
            covered_threshold: COVERED_THRESHOLD,
            timeout_ms: DEFAULT_TIMEOUT_MS,
            store_max_entries: DEFAULT_STORE_MAX_ENTRIES,
            store_max_chars: DEFAULT_STORE_MAX_CHARS,
            entry_chars: DEFAULT_ENTRY_CHARS,
        }
    }

    /// Whether the gate should run at all.
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// OpenRouter `provider` block, or `None` when no routing knob is set.
    pub fn provider_block(&self) -> Option<Value> {
        let mut block = serde_json::Map::new();
        if let Some(zdr) = self.zdr {
            block.insert("zdr".to_owned(), Value::Bool(zdr));
        }
        if let Some(data_collection) = &self.data_collection {
            block.insert(
                "data_collection".to_owned(),
                Value::String(data_collection.clone()),
            );
        }
        if let Some(require_parameters) = self.require_parameters {
            block.insert(
                "require_parameters".to_owned(),
                Value::Bool(require_parameters),
            );
        }
        (!block.is_empty()).then_some(Value::Object(block))
    }
}

/// Every gate failure mode. All of them are fail-open at the call site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateError {
    /// No credential resolved from `api_key_env`, `GROK_JEV_API_KEY` or the caller.
    MissingCredential,
    /// Non-2xx response; `body` is a short snippet for the log line.
    Http { status: u16, body: String },
    /// Transport failure (DNS, connect, timeout, body read).
    Request(String),
    /// Body was not JSON, or lacked an `answers` object.
    Malformed(String),
    /// An answer was missing, non-numeric, non-finite or outside `0.0..=1.0`.
    InvalidAnswer { name: String, value: String },
}

impl std::fmt::Display for GateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingCredential => write!(
                f,
                "no Jev credential: set api_key_env, {JEV_API_KEY_ENV}, or store the OpenRouter key"
            ),
            Self::Http { status, body } => write!(f, "jev request failed ({status}): {body}"),
            Self::Request(message) => write!(f, "jev request error: {message}"),
            Self::Malformed(message) => write!(f, "jev returned a malformed response: {message}"),
            Self::InvalidAnswer { name, value } => {
                write!(f, "invalid jev answer for {name}: {value}")
            }
        }
    }
}

impl std::error::Error for GateError {}

/// One decisions transport. Implemented by
/// [`JevDecisionsClient`](crate::gate_client::JevDecisionsClient) and by fakes
/// in tests.
#[async_trait]
pub trait DecisionClient: Send + Sync {
    /// Ask one batch of questions about `state`; returns the `answers` object.
    async fn ask(&self, state: &Value, questions: &Value) -> Result<Value, GateError>;
}

/// What memory already says, carried in the state on every candidate.
///
/// Capped by [`GateConfig::store_max_entries`] and
/// [`GateConfig::store_max_chars`]; past either bound the store degrades to a
/// one-line-per-entry summary so the state stays bounded however the memory
/// files grow. The summary keeps the newest entries — a restatement is almost
/// always a restatement of something recent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Store {
    body: StoreBody,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
enum StoreBody {
    #[default]
    Empty,
    Entries(Vec<String>),
    Summary(String),
}

impl Store {
    /// Build a store from existing entries, oldest first.
    pub fn build<I>(entries: I, config: &GateConfig) -> Self
    where
        I: IntoIterator<Item = String>,
    {
        let cleaned: Vec<String> = entries
            .into_iter()
            .map(|entry| collapse(&entry, config.entry_chars))
            .filter(|entry| !entry.is_empty())
            .collect();
        if cleaned.is_empty() {
            return Self::default();
        }
        let chars: usize = cleaned.iter().map(|entry| entry.len() + 1).sum();
        if cleaned.len() <= config.store_max_entries && chars <= config.store_max_chars {
            return Self {
                body: StoreBody::Entries(cleaned),
            };
        }
        let tail = &cleaned[cleaned.len().saturating_sub(config.store_max_entries)..];
        let mut summary = String::with_capacity(config.store_max_chars);
        for entry in tail {
            let line = first_line(entry);
            if summary.len() + line.len() + 3 > config.store_max_chars {
                break;
            }
            summary.push_str("- ");
            summary.push_str(&line);
            summary.push('\n');
        }
        Self {
            body: StoreBody::Summary(summary.trim_end().to_owned()),
        }
    }

    /// Whether the store carries anything.
    pub fn is_empty(&self) -> bool {
        matches!(self.body, StoreBody::Empty)
    }

    /// Number of entries when the store is a full list, `0` for a summary.
    pub fn len(&self) -> usize {
        match &self.body {
            StoreBody::Entries(entries) => entries.len(),
            _ => 0,
        }
    }

    /// State fragment sent to the decisions endpoint.
    fn to_state(&self) -> Value {
        match &self.body {
            StoreBody::Empty => json!({ "entries": [], "summary": Value::Null }),
            StoreBody::Entries(entries) => json!({ "entries": entries, "summary": Value::Null }),
            StoreBody::Summary(summary) => json!({ "entries": [], "summary": summary }),
        }
    }
}

/// Collapse whitespace and truncate to `limit` characters.
fn collapse(text: &str, limit: usize) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= limit {
        collapsed
    } else {
        collapsed.chars().take(limit).collect()
    }
}

/// First line of an entry, truncated for the summary form.
fn first_line(entry: &str) -> String {
    collapse(entry.split('\n').next().unwrap_or(entry), 120)
}

/// A note plus its three answers.
struct Verdict {
    worth: f64,
    covered: f64,
    bucket: Bucket,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Bucket {
    ThisFolder,
    Global,
    Discard,
}

/// The gate: one transport, one policy.
pub struct MemoryGate {
    client: Arc<dyn DecisionClient>,
    config: GateConfig,
}

impl MemoryGate {
    /// Build a gate over an explicit transport.
    pub fn new(client: Arc<dyn DecisionClient>, config: GateConfig) -> Self {
        Self { client, config }
    }

    /// Policy this gate resolved.
    pub fn config(&self) -> &GateConfig {
        &self.config
    }

    /// Decide whether one candidate is written, and where.
    ///
    /// Never fails: any transport or parsing error comes back as
    /// [`GateOutcome::Failed`], which the caller treats as "write as before".
    pub async fn decide(&self, candidate: &Candidate<'_>, store: &Store) -> GateOutcome {
        if !self.config.enabled {
            return GateOutcome::Disabled;
        }
        let state = build_state(candidate, store);
        let answers = match self.client.ask(&state, &build_questions()).await {
            Ok(answers) => answers,
            Err(error) => {
                tracing::info!(target: LOG, error = %error, "MEMORY_GATE: decision failed, storing");
                return GateOutcome::Failed {
                    error: error.to_string(),
                };
            }
        };
        match read_verdict(&answers) {
            Ok(verdict) => self.finish(verdict),
            Err(error) => {
                tracing::info!(target: LOG, error = %error, "MEMORY_GATE: bad answer, storing");
                GateOutcome::Failed {
                    error: error.to_string(),
                }
            }
        }
    }

    /// Turn validated answers into an outcome.
    fn finish(&self, verdict: Verdict) -> GateOutcome {
        let outcome = if verdict.worth < self.config.worth_threshold {
            GateOutcome::Dropped {
                reason: DropReason::NotWorth,
                worth: verdict.worth,
                covered: verdict.covered,
            }
        } else if verdict.covered >= self.config.covered_threshold {
            GateOutcome::Dropped {
                reason: DropReason::Covered,
                worth: verdict.worth,
                covered: verdict.covered,
            }
        } else if verdict.bucket == Bucket::Discard {
            GateOutcome::Dropped {
                reason: DropReason::Discarded,
                worth: verdict.worth,
                covered: verdict.covered,
            }
        } else {
            GateOutcome::Stored {
                scope: match verdict.bucket {
                    Bucket::ThisFolder => WriteScope::ThisFolder,
                    _ => WriteScope::Global,
                },
                worth: verdict.worth,
                covered: verdict.covered,
            }
        };
        tracing::info!(
            target: LOG,
            outcome = outcome.as_str(),
            worth = verdict.worth,
            covered = verdict.covered,
            "MEMORY_GATE: decision"
        );
        outcome
    }

    /// Rerank a shortlist against `query`, returning a permutation of indices.
    ///
    /// One `noul` question per candidate, each judged in isolation — a single
    /// "does the best fit" question conflates them. Measured 7/10 → 9/10 on the
    /// memory retrieval sample, which is also what removes the global leak
    /// without a scope filter on search. `None` (keep the caller's order) on
    /// any failure.
    ///
    /// Runs when either switch is on: `rerank = true` selects this route for
    /// search even with the append gate `enabled = false`.
    pub async fn rerank(
        &self,
        query: &str,
        workspace_path: &str,
        documents: &[String],
    ) -> Option<Vec<usize>> {
        if documents.is_empty() || !(self.config.enabled || self.config.rerank) {
            return None;
        }
        let state = json!({
            "context": "One candidate per question; each is judged alone against `query`.",
            "workspace_path": workspace_path,
            "query": query,
        });
        let answers = self
            .client
            .ask(&state, &rerank_questions(documents))
            .await
            .ok()?;
        let mut scored: Vec<(usize, f64)> = documents
            .iter()
            .enumerate()
            .map(|(index, _)| noul_answer(&answers, &index.to_string()).map(|s| (index, s)))
            .collect::<Result<Vec<_>, _>>()
            .ok()?;
        // Stable: equal scores keep their incoming order.
        scored.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        Some(scored.into_iter().map(|(index, _)| index).collect())
    }

    /// Drop the candidates memory already covers, reusing the `covered`
    /// question. Used by dream so a consolidation does not re-add what the
    /// curated region already says. `None` keeps every candidate.
    pub async fn covered_only(&self, candidates: &[String], store: &Store) -> Option<Vec<String>> {
        if !self.config.enabled || candidates.is_empty() {
            return None;
        }
        let state = json!({
            "context": "One candidate per question; `store` holds what memory already says.",
            "store": store.to_state(),
        });
        let answers = self
            .client
            .ask(&state, &covered_questions(candidates))
            .await
            .ok()?;
        let mut kept = Vec::with_capacity(candidates.len());
        for (index, candidate) in candidates.iter().enumerate() {
            let covered = noul_answer(&answers, &index.to_string()).ok()?;
            if covered < self.config.covered_threshold {
                kept.push(candidate.clone());
            }
        }
        Some(kept)
    }
}

/// State for one admission request.
fn build_state(candidate: &Candidate<'_>, store: &Store) -> Value {
    json!({
        "context": GATE_CONTEXT,
        "workspace_path": candidate.workspace_path,
        "candidate": candidate.text,
        "store": store.to_state(),
    })
}

/// The three atomic questions asked about every candidate.
fn build_questions() -> Value {
    json!({
        "worth": {
            "type": "noul",
            "instructions": WORTH_QUESTION,
            "criteria": { "true": WORTH_TRUE, "false": WORTH_FALSE },
        },
        "covered": {
            "type": "noul",
            "instructions": COVERED_QUESTION,
            "criteria": { "true": COVERED_TRUE, "false": COVERED_FALSE },
        },
        "bucket": {
            "type": "choice",
            "instructions": BUCKET_QUESTION,
            "criteria": {
                "this_folder": BUCKET_THIS_FOLDER,
                "global": BUCKET_GLOBAL,
                "discard": BUCKET_DISCARD,
            },
        },
    })
}

/// One `covered` question per candidate, keyed by index.
fn covered_questions(candidates: &[String]) -> Value {
    let questions: serde_json::Map<String, Value> = candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            (
                index.to_string(),
                json!({
                    "type": "noul",
                    "instructions": format!("Candidate: {candidate}"),
                    "criteria": { "true": COVERED_TRUE, "false": COVERED_FALSE },
                }),
            )
        })
        .collect();
    Value::Object(questions)
}

/// One `noul` question per document, keyed by index — never by text, because
/// two documents can share a text and a text-keyed answer would collide.
/// Wording mirrors the retrieval rerank adapter so both paths ask the same.
fn rerank_questions(documents: &[String]) -> Value {
    let questions: serde_json::Map<String, Value> = documents
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
                    "criteria": { "true": JEV_TRUE_CRITERION, "false": JEV_FALSE_CRITERION },
                }),
            )
        })
        .collect();
    Value::Object(questions)
}

/// Validate the three answers. Every field is required: a partial answer is a
/// failure, and the caller stores, which is what it did before the gate.
fn read_verdict(answers: &Value) -> Result<Verdict, GateError> {
    Ok(Verdict {
        worth: noul_answer(answers, "worth")?,
        covered: noul_answer(answers, "covered")?,
        bucket: bucket_answer(answers)?,
    })
}

/// Read one `noul` (or legacy `score`) answer in `0.0..=1.0`.
fn noul_answer(answers: &Value, name: &str) -> Result<f64, GateError> {
    let answer = answers.get(name).ok_or_else(|| GateError::InvalidAnswer {
        name: name.to_owned(),
        value: "missing".to_owned(),
    })?;
    let value = answer
        .get("noul")
        .or_else(|| answer.get("score"))
        .and_then(Value::as_f64)
        .ok_or_else(|| GateError::InvalidAnswer {
            name: name.to_owned(),
            value: answer.to_string(),
        })?;
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(GateError::InvalidAnswer {
            name: name.to_owned(),
            value: value.to_string(),
        });
    }
    Ok(value)
}

/// Read the routing answer. An unrecognised option is a failure, never a
/// silent guess: the fallback scope is the historical one.
fn bucket_answer(answers: &Value) -> Result<Bucket, GateError> {
    let answer = answers
        .get("bucket")
        .ok_or_else(|| GateError::InvalidAnswer {
            name: "bucket".to_owned(),
            value: "missing".to_owned(),
        })?;
    let raw = answer
        .get("choice")
        .or_else(|| answer.get("answer"))
        .or_else(|| answer.get("value"))
        .and_then(Value::as_str)
        .ok_or_else(|| GateError::InvalidAnswer {
            name: "bucket".to_owned(),
            value: answer.to_string(),
        })?;
    match raw.trim().to_ascii_lowercase().as_str() {
        "this_folder" | "workspace" | "project" | "local" => Ok(Bucket::ThisFolder),
        "global" | "user" => Ok(Bucket::Global),
        "discard" | "drop" | "none" => Ok(Bucket::Discard),
        other => Err(GateError::InvalidAnswer {
            name: "bucket".to_owned(),
            value: other.to_owned(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Every question carries its options under the wire key the harness sent
    /// (`script-test/jev/primitives.py`): `criteria`, for `choice` as well as
    /// `noul`. A `choice` whose options ride an unknown key is asked blind.
    #[test]
    fn every_question_carries_its_criteria_under_the_wire_key() {
        let questions = build_questions();
        for key in ["worth", "covered"] {
            assert!(questions[key]["criteria"]["true"].is_string(), "{key}");
            assert!(questions[key]["criteria"]["false"].is_string(), "{key}");
        }
        assert_eq!(
            questions["bucket"]["criteria"]["this_folder"],
            BUCKET_THIS_FOLDER
        );
        assert_eq!(questions["bucket"]["criteria"]["global"], BUCKET_GLOBAL);
        assert_eq!(questions["bucket"]["criteria"]["discard"], BUCKET_DISCARD);
    }

    /// Answers by question name; `fail` makes the transport fail.
    struct FakeClient {
        answers: Value,
        fail: bool,
        seen: Mutex<Vec<Value>>,
    }

    impl FakeClient {
        fn new(worth: f64, covered: f64, bucket: &str) -> Arc<Self> {
            Arc::new(Self {
                answers: json!({
                    "worth": { "noul": worth },
                    "covered": { "noul": covered },
                    "bucket": { "choice": bucket },
                }),
                fail: false,
                seen: Mutex::new(Vec::new()),
            })
        }

        fn failing() -> Arc<Self> {
            Arc::new(Self {
                answers: Value::Null,
                fail: true,
                seen: Mutex::new(Vec::new()),
            })
        }

        fn states(&self) -> Vec<Value> {
            self.seen.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl DecisionClient for FakeClient {
        async fn ask(&self, state: &Value, _questions: &Value) -> Result<Value, GateError> {
            self.seen.lock().unwrap().push(state.clone());
            if self.fail {
                return Err(GateError::Request("boom".to_owned()));
            }
            Ok(self.answers.clone())
        }
    }

    fn enabled() -> GateConfig {
        GateConfig {
            enabled: true,
            ..GateConfig::disabled()
        }
    }

    fn candidate() -> Candidate<'static> {
        Candidate {
            text: "Always run the formatter before finishing.",
            workspace_path: "/w/project",
        }
    }

    fn store() -> Store {
        Store::build(
            vec!["## Short sleeps\n\npoll state directly".to_owned()],
            &enabled(),
        )
    }

    #[tokio::test]
    async fn disabled_gate_makes_no_request() {
        let client = FakeClient::new(0.9, 0.0, "global");
        let gate = MemoryGate::new(client.clone(), GateConfig::disabled());
        assert_eq!(
            gate.decide(&candidate(), &store()).await,
            GateOutcome::Disabled
        );
        assert!(client.states().is_empty());
    }

    #[tokio::test]
    async fn worth_below_the_band_drops_the_note() {
        let gate = MemoryGate::new(FakeClient::new(0.19, 0.0, "global"), enabled());
        let outcome = gate.decide(&candidate(), &store()).await;
        assert!(matches!(
            outcome,
            GateOutcome::Dropped {
                reason: DropReason::NotWorth,
                ..
            }
        ));
    }

    /// 0.5 is the intuitive coin flip and loses 7 of 36 valuable notes.
    #[tokio::test]
    async fn worth_at_the_band_is_kept() {
        let gate = MemoryGate::new(FakeClient::new(0.20, 0.0, "this_folder"), enabled());
        let outcome = gate.decide(&candidate(), &store()).await;
        assert!(outcome.is_stored(), "{outcome:?}");
    }

    #[tokio::test]
    async fn covered_at_the_threshold_is_a_restatement() {
        let gate = MemoryGate::new(FakeClient::new(0.9, 0.50, "global"), enabled());
        let outcome = gate.decide(&candidate(), &store()).await;
        assert!(matches!(
            outcome,
            GateOutcome::Dropped {
                reason: DropReason::Covered,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn routing_answers_map_to_scopes() {
        let this_folder = MemoryGate::new(FakeClient::new(0.9, 0.0, "this_folder"), enabled());
        assert_eq!(
            this_folder.decide(&candidate(), &store()).await,
            GateOutcome::Stored {
                scope: WriteScope::ThisFolder,
                worth: 0.9,
                covered: 0.0
            }
        );
        let global = MemoryGate::new(FakeClient::new(0.9, 0.0, "global"), enabled());
        assert_eq!(
            global.decide(&candidate(), &store()).await,
            GateOutcome::Stored {
                scope: WriteScope::Global,
                worth: 0.9,
                covered: 0.0
            }
        );
    }

    #[tokio::test]
    async fn discard_bucket_drops_even_a_worthwhile_note() {
        let gate = MemoryGate::new(FakeClient::new(0.9, 0.0, "discard"), enabled());
        assert!(matches!(
            gate.decide(&candidate(), &store()).await,
            GateOutcome::Dropped {
                reason: DropReason::Discarded,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn transport_failure_stores() {
        let gate = MemoryGate::new(FakeClient::failing(), enabled());
        let outcome = gate.decide(&candidate(), &store()).await;
        assert!(matches!(outcome, GateOutcome::Failed { .. }), "{outcome:?}");
    }

    #[tokio::test]
    async fn partial_answer_stores() {
        let client = Arc::new(FakeClient {
            answers: json!({ "worth": { "noul": 0.9 } }),
            fail: false,
            seen: Mutex::new(Vec::new()),
        });
        let gate = MemoryGate::new(client, enabled());
        assert!(matches!(
            gate.decide(&candidate(), &store()).await,
            GateOutcome::Failed { .. }
        ));
    }

    #[tokio::test]
    async fn out_of_range_answer_stores() {
        let gate = MemoryGate::new(FakeClient::new(1.4, 0.0, "global"), enabled());
        assert!(matches!(
            gate.decide(&candidate(), &store()).await,
            GateOutcome::Failed { .. }
        ));
    }

    #[tokio::test]
    async fn unknown_bucket_stores() {
        let gate = MemoryGate::new(FakeClient::new(0.9, 0.0, "sideways"), enabled());
        assert!(matches!(
            gate.decide(&candidate(), &store()).await,
            GateOutcome::Failed { .. }
        ));
    }

    /// The store travels in the state on every candidate; without it the gate
    /// re-stores a restatement 6 times out of 7.
    #[tokio::test]
    async fn state_carries_the_store_and_the_folder() {
        let client = FakeClient::new(0.9, 0.0, "global");
        let gate = MemoryGate::new(client.clone(), enabled());
        gate.decide(&candidate(), &store()).await;
        let state = client.states().remove(0);
        assert_eq!(state["workspace_path"], json!("/w/project"));
        assert_eq!(
            state["store"]["entries"][0],
            json!("## Short sleeps poll state directly")
        );
    }

    #[tokio::test]
    async fn store_degrades_to_a_summary_past_the_cap() {
        let config = GateConfig {
            enabled: true,
            store_max_entries: 2,
            ..GateConfig::disabled()
        };
        let entries: Vec<String> = (0..5).map(|i| format!("## entry {i}\n\nbody")).collect();
        let store = Store::build(entries, &config);
        assert_eq!(store.len(), 0);
        let state = store.to_state();
        let summary = state["summary"].as_str().unwrap();
        assert!(summary.contains("entry 3") && summary.contains("entry 4"));
        assert!(!summary.contains("entry 0"), "newest entries are kept");
    }

    #[tokio::test]
    async fn rerank_orders_by_score_and_keeps_ties_stable() {
        let client = Arc::new(FakeClient {
            answers: json!({
                "0": { "noul": 0.1 },
                "1": { "noul": 0.9 },
                "2": { "noul": 0.9 },
            }),
            fail: false,
            seen: Mutex::new(Vec::new()),
        });
        let gate = MemoryGate::new(client, enabled());
        let docs = vec!["a".to_owned(), "b".to_owned(), "c".to_owned()];
        assert_eq!(
            gate.rerank("q", "/w", &docs).await,
            Some(vec![1, 2, 0]),
            "descending score, incoming order for ties"
        );
    }

    #[tokio::test]
    async fn rerank_failure_keeps_the_caller_order() {
        let gate = MemoryGate::new(FakeClient::failing(), enabled());
        let docs = vec!["a".to_owned()];
        assert_eq!(gate.rerank("q", "/w", &docs).await, None);
    }

    #[tokio::test]
    async fn covered_only_drops_what_memory_already_says() {
        let client = Arc::new(FakeClient {
            answers: json!({ "0": { "noul": 0.8 }, "1": { "noul": 0.2 } }),
            fail: false,
            seen: Mutex::new(Vec::new()),
        });
        let gate = MemoryGate::new(client, enabled());
        let candidates = vec!["already there".to_owned(), "brand new".to_owned()];
        assert_eq!(
            gate.covered_only(&candidates, &store()).await,
            Some(vec!["brand new".to_owned()])
        );
    }

    #[test]
    fn provider_block_omits_absent_knobs() {
        let config = GateConfig {
            zdr: Some(true),
            ..GateConfig::disabled()
        };
        assert_eq!(config.provider_block().unwrap()["zdr"], json!(true));
        assert!(
            config
                .provider_block()
                .unwrap()
                .get("data_collection")
                .is_none()
        );
    }

    #[test]
    fn outcome_labels_are_stable() {
        assert_eq!(GateOutcome::Disabled.as_str(), "disabled");
        assert_eq!(DropReason::Covered.as_str(), "covered");
    }
}
