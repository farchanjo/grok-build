//! Core types for Jev-guided compaction pruning.
//!
//! The pruner asks a Jev decisions endpoint, for every tool call outside the
//! pinned window, two `noul` questions: whether the call still matters and
//! whether its full result must stay verbatim. Nothing here touches chat
//! state — the caller rewrites only the view handed to the summarizer.

use std::collections::BTreeMap;

use super::transport::JevTransport;
use crate::agent::config::JevPruneConfig;

/// Default decisions endpoint (OpenRouter alpha surface).
///
/// Kept as a standalone constant for callers that never name a transport; the
/// authoritative pair lives on [`JevTransport`].
pub const DEFAULT_ENDPOINT: &str = "https://openrouter.ai/api/alpha/decisions";
/// Default model reference. A plain string: the Jev model is not a catalog entry.
pub const DEFAULT_MODEL: &str = "~typesafe/jev-latest";
/// Suffix of the derived session key (`{session_info.id}:jev`).
///
/// The Jev payload shape differs from a chat turn's, so reusing the session's
/// own key would make `bootstrap_room` — an `fnv1a_32` of the key — claim a
/// prefix family Jev does not belong to. The derived key leaves the chat path
/// untouched and still gives affinity within Jev's own traffic.
pub const SESSION_KEY_SUFFIX: &str = "jev";
/// Default keep probability; `keep_result >= threshold` keeps the result verbatim.
pub const DEFAULT_KEEP_THRESHOLD: f64 = 0.5;
/// Newest items never pruned, plus the very first item.
pub const DEFAULT_PRESERVE_RECENT_MESSAGES: usize = 6;
/// Estimated token ceiling for the state resent with every batch.
pub const DEFAULT_MAX_STATE_TOKENS: usize = 25_000;
/// Estimated token ceiling for state plus one batch of questions.
pub const DEFAULT_MAX_REQUEST_TOKENS: usize = 30_000;
/// Characters of a truncated tool result retained as a head.
pub const DEFAULT_TRUNCATE_HEAD_CHARS: usize = 300;
/// Per-request timeout in milliseconds.
pub const DEFAULT_TIMEOUT_MS: u64 = 8_000;

/// Resolved `[compaction.jev]` policy.
///
/// Defaults are the documented ones; [`Self::disabled`] (also [`Default`])
/// means the pipeline never issues an HTTP call.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedJevPrune {
    /// Master switch. `false` short-circuits before any candidate work.
    pub enabled: bool,
    /// Which wire the decisions call rides. Defaults to `openrouter`, which is
    /// today's behaviour.
    pub transport: JevTransport,
    /// Decisions endpoint. Defaults to [`DEFAULT_ENDPOINT`].
    pub endpoint: String,
    /// Model reference sent in the request body. Defaults to [`DEFAULT_MODEL`].
    pub model: String,
    /// Environment variable holding the API key, when the caller wants one.
    pub api_key_env: Option<String>,
    /// OpenRouter `provider.zdr`.
    pub zdr: Option<bool>,
    /// OpenRouter `provider.data_collection`.
    pub data_collection: Option<String>,
    /// OpenRouter `provider.require_parameters`.
    pub require_parameters: Option<bool>,
    /// Minimum keep probability for a call or a result to stay.
    pub keep_threshold: f64,
    /// Newest items (and the first item) never pruned.
    pub preserve_recent_messages: usize,
    /// Estimated token ceiling for the state.
    pub max_state_tokens: usize,
    /// Estimated token ceiling for state plus one batch of questions.
    pub max_request_tokens: usize,
    /// Characters of a truncated tool result retained as a head.
    pub truncate_head_chars: usize,
    /// Per-request timeout in milliseconds.
    pub timeout_ms: u64,
}

/// `f64` in `keep_threshold` keeps the type out of `Eq`; every value written
/// here is finite, so the marker is sound in practice and preserves the
/// `Eq` bound `ResolvedCompactionConfig` already carries.
impl Eq for ResolvedJevPrune {}

impl Default for ResolvedJevPrune {
    fn default() -> Self {
        Self::disabled()
    }
}

impl ResolvedJevPrune {
    /// Resolve from user configuration; `None` (absent `[compaction.jev]`) is disabled.
    pub fn from_config(cfg: Option<&JevPruneConfig>) -> Self {
        let Some(cfg) = cfg else {
            return Self::disabled();
        };
        let blank = |value: &Option<String>| {
            value
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_owned)
        };
        let transport = cfg.transport.unwrap_or_default();
        Self {
            enabled: cfg.enabled,
            transport,
            endpoint: blank(&cfg.endpoint).unwrap_or_else(|| transport.endpoint().to_owned()),
            model: blank(&cfg.model).unwrap_or_else(|| transport.model().to_owned()),
            api_key_env: blank(&cfg.api_key_env),
            zdr: cfg.zdr,
            data_collection: blank(&cfg.data_collection),
            require_parameters: cfg.require_parameters,
            keep_threshold: cfg
                .keep_threshold
                .filter(|v| v.is_finite())
                .unwrap_or(DEFAULT_KEEP_THRESHOLD),
            preserve_recent_messages: cfg
                .preserve_recent_messages
                .unwrap_or(DEFAULT_PRESERVE_RECENT_MESSAGES),
            max_state_tokens: cfg
                .max_state_tokens
                .filter(|v| *v > 0)
                .unwrap_or(DEFAULT_MAX_STATE_TOKENS),
            max_request_tokens: cfg
                .max_request_tokens
                .filter(|v| *v > 0)
                .unwrap_or(DEFAULT_MAX_REQUEST_TOKENS),
            truncate_head_chars: cfg
                .truncate_head_chars
                .unwrap_or(DEFAULT_TRUNCATE_HEAD_CHARS),
            timeout_ms: cfg
                .timeout_ms
                .filter(|v| *v > 0)
                .unwrap_or(DEFAULT_TIMEOUT_MS),
        }
    }

    /// Explicitly disabled policy with defaults for every knob.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            transport: JevTransport::default(),
            endpoint: DEFAULT_ENDPOINT.to_owned(),
            model: DEFAULT_MODEL.to_owned(),
            api_key_env: None,
            zdr: None,
            data_collection: None,
            require_parameters: None,
            keep_threshold: DEFAULT_KEEP_THRESHOLD,
            preserve_recent_messages: DEFAULT_PRESERVE_RECENT_MESSAGES,
            max_state_tokens: DEFAULT_MAX_STATE_TOKENS,
            max_request_tokens: DEFAULT_MAX_REQUEST_TOKENS,
            truncate_head_chars: DEFAULT_TRUNCATE_HEAD_CHARS,
            timeout_ms: DEFAULT_TIMEOUT_MS,
        }
    }

    /// Whether the pruner should run at all.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// OpenRouter `provider` block, or `None` when the transport does not take
    /// one or no routing knob is set.
    ///
    /// The block is OpenRouter-only: the native endpoint rejects unknown
    /// top-level fields.
    pub fn provider_block(&self) -> Option<serde_json::Value> {
        if !self.transport.sends_provider_block() {
            return None;
        }
        let mut block = serde_json::Map::new();
        if let Some(zdr) = self.zdr {
            block.insert("zdr".to_owned(), serde_json::Value::Bool(zdr));
        }
        if let Some(data_collection) = &self.data_collection {
            block.insert(
                "data_collection".to_owned(),
                serde_json::Value::String(data_collection.clone()),
            );
        }
        if let Some(require_parameters) = self.require_parameters {
            block.insert(
                "require_parameters".to_owned(),
                serde_json::Value::Bool(require_parameters),
            );
        }
        (!block.is_empty()).then_some(serde_json::Value::Object(block))
    }
}

/// Derived session key for the Jev lane: `{session_id}:jev`.
///
/// Never the session's own key — see [`SESSION_KEY_SUFFIX`].
///
/// # The six session-id carriers, audited before adding a seventh
///
/// Every one of these is derived from `session_info.id` on the chat path, each
/// gated differently, and none of them changed when the Jev lane was added:
///
/// | carrier | where | gate |
/// | --- | --- | --- |
/// | `x-grok-session-id` | `client.rs` `GrokRequestHeaders::apply` | `first_party` only |
/// | `X-Session-ID` | `client.rs` self-hosted family | self-hosted base URLs only |
/// | body `session_id` + `bootstrap_room` | `apply_session_affinity` | suppressed for OpenAI and Anthropic |
/// | OpenRouter native `session_id` | same body field | never suppressed (OpenRouter reads it) |
/// | Anthropic `metadata.user_id` | Messages conversion | Anthropic only |
/// | OpenAI `prompt_cache_key` | `finalize_request` | OpenAI and Codex only |
///
/// Two consequences decide the Jev design. First, `x-grok-*` headers never
/// reach a third-party transport at all: `GrokRequestHeaders::apply` returns
/// early when `!first_party`, so a Jev call on OpenRouter could not carry them
/// even if it wanted to. Second, `bootstrap_room` is `fnv1a_32(key)` and is a
/// *routing* key, not decoration: two streams that share it are assumed to
/// share prefixes, which a `{state, questions}` payload does not.
///
/// So Jev sends `session_id` only, on its own derived key, and leaves
/// `bootstrap_room` to the transports that define it.
pub fn derived_session_key(session_id: &str) -> String {
    format!("{session_id}:{SESSION_KEY_SUFFIX}")
}

/// Every failure mode is surfaced as a [`PruneError`]; the caller decides
/// whether to fall back to the unpruned view. The pruner never aborts a
/// compaction on its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PruneError {
    /// No credential resolved. `chain` is the ordered list of links tried, so
    /// a two-transport setup stays debuggable.
    MissingCredential { chain: String },
    /// Non-2xx response. `body` is a short snippet for the log line.
    Http { status: u16, body: String },
    /// Transport failure (DNS, connect, timeout, body read).
    Request(String),
    /// Body was not JSON, or lacked an `answers` object.
    Malformed(String),
    /// An answer was missing, non-numeric, non-finite or outside `0.0..=1.0`.
    InvalidAnswer { name: String, value: String },
    /// State did not fit `max_state_tokens` even after every fitting stage.
    StateTooLarge { tokens: usize, limit: usize },
    /// A single batch of questions does not fit `max_request_tokens`.
    NoRoomForQuestions { state_tokens: usize, limit: usize },
    /// Compaction cancel token fired while asking.
    Cancelled,
}

impl std::fmt::Display for PruneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingCredential { chain } => {
                write!(f, "no Jev credential; tried {chain}")
            }
            Self::Http { status, body } => write!(f, "jev request failed ({status}): {body}"),
            Self::Request(message) => write!(f, "jev request error: {message}"),
            Self::Malformed(message) => write!(f, "jev returned a malformed response: {message}"),
            Self::InvalidAnswer { name, value } => {
                write!(f, "invalid jev answer for {name}: {value}")
            }
            Self::StateTooLarge { tokens, limit } => write!(
                f,
                "history too large for jev (~{tokens} tokens after truncation, limit {limit})"
            ),
            Self::NoRoomForQuestions {
                state_tokens,
                limit,
            } => write!(
                f,
                "state leaves no room for questions (~{state_tokens} of {limit} tokens)"
            ),
            Self::Cancelled => write!(f, "jev request cancelled"),
        }
    }
}

impl std::error::Error for PruneError {}

/// Jev's probability that a call, and its result, still matter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeepDecision {
    /// Probability the call itself should stay (with its input).
    pub keep_call: f64,
    /// Probability the full result must stay verbatim.
    pub keep_result: f64,
}

/// Answers keyed by `tool_use_id` — never by index, so the input ladder and the
/// lossy prepare can rewrite the list without invalidating them.
#[derive(Debug, Clone)]
pub struct PruneDecisions {
    by_id: BTreeMap<String, KeepDecision>,
    /// Threshold captured at decide time so [`super::apply_decisions`] is
    /// self-contained and pure.
    threshold: f64,
}

impl Default for PruneDecisions {
    fn default() -> Self {
        Self {
            by_id: BTreeMap::new(),
            threshold: DEFAULT_KEEP_THRESHOLD,
        }
    }
}

impl PruneDecisions {
    /// Empty decision set carrying the threshold used for the three-way split.
    pub fn with_threshold(threshold: f64) -> Self {
        Self {
            by_id: BTreeMap::new(),
            threshold,
        }
    }

    /// Decision for one tool call, if the call was a candidate.
    pub fn get(&self, tool_use_id: &str) -> Option<KeepDecision> {
        self.by_id.get(tool_use_id).copied()
    }

    /// Number of decided calls.
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// Whether nothing was decided.
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    /// Threshold used to turn an answer into a keep / truncate / drop action.
    pub fn threshold(&self) -> f64 {
        self.threshold
    }

    /// Iterate over `(tool_use_id, decision)` in stable id order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &KeepDecision)> {
        self.by_id
            .iter()
            .map(|(id, decision)| (id.as_str(), decision))
    }

    pub(crate) fn insert(&mut self, tool_use_id: String, decision: KeepDecision) {
        self.by_id.insert(tool_use_id, decision);
    }
}

/// Terminal state of one prune attempt, for the log line and the counters.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum PruneOutcome {
    /// Decisions were applied to the summarizer view.
    Applied,
    /// `enabled = false`; no HTTP call was made.
    #[default]
    SkippedDisabled,
    /// Enabled, but no unpinned call with a result existed.
    SkippedNoCandidates,
    /// Any failure; the caller continues with the unpruned view.
    Failed(String),
}

impl PruneOutcome {
    /// Stable telemetry label.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::SkippedDisabled => "skipped_disabled",
            Self::SkippedNoCandidates => "skipped_no_candidates",
            Self::Failed(_) => "failed_fallback",
        }
    }
}

/// Counters for one prune attempt.
#[derive(Debug, Default, Clone)]
pub struct PruneStats {
    /// Unpinned calls with a result that were asked about.
    pub candidates: usize,
    /// Calls pinned by the first item or the newest window.
    pub pinned: usize,
    /// Calls whose result Jev kept verbatim.
    pub kept: usize,
    /// Calls kept with a truncated result.
    pub results_truncated: usize,
    /// Calls dropped together with their result.
    pub calls_dropped: usize,
    /// Estimated tokens of the state that was sent.
    pub state_tokens: usize,
    /// Which fitting stage produced the state (`""` when no request was made).
    pub state_stage: &'static str,
    /// Number of HTTP requests (one per batch).
    pub requests: usize,
    /// Wall time of the decide stage.
    pub ms: u64,
    /// Characters of the summarizer view before pruning.
    pub chars_before: usize,
    /// Characters of the summarizer view after pruning.
    pub chars_after: usize,
    /// Terminal state.
    pub outcome: PruneOutcome,
}

impl PruneStats {
    /// One-line summary for the compaction log.
    pub fn summary(&self) -> String {
        format!(
            "outcome={} candidates={} pinned={} kept={} truncated={} dropped={} \
             state_tokens={} stage={} requests={} chars={}->{} ms={}",
            self.outcome.as_str(),
            self.candidates,
            self.pinned,
            self.kept,
            self.results_truncated,
            self.calls_dropped,
            self.state_tokens,
            if self.state_stage.is_empty() {
                "-"
            } else {
                self.state_stage
            },
            self.requests,
            self.chars_before,
            self.chars_after,
            self.ms,
        )
    }
}

/// A tool call paired with its result by `tool_use_id`.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolPair {
    /// Short id used in the state and in question names (`t1`, `t2`, ...).
    pub id: String,
    /// The `tool_use_id` shared by call and result.
    pub tool_use_id: String,
    /// Tool name.
    pub tool: String,
    /// Raw JSON arguments of the call.
    pub input: String,
    /// Index of the item holding the tool call.
    pub call_index: usize,
    /// Index of the item holding the tool result.
    pub result_index: usize,
    /// Result length in characters.
    pub result_chars: usize,
    /// Whether the result was an error.
    pub is_error: bool,
    /// Inside the first item or the newest window; never a candidate.
    pub pinned: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_config_resolves_to_disabled_defaults() {
        let cfg = ResolvedJevPrune::from_config(None);
        assert!(!cfg.is_enabled());
        assert_eq!(cfg, ResolvedJevPrune::disabled());
        assert_eq!(cfg.endpoint, DEFAULT_ENDPOINT);
        assert_eq!(cfg.model, DEFAULT_MODEL);
        assert_eq!(cfg.keep_threshold, DEFAULT_KEEP_THRESHOLD);
        assert!(cfg.provider_block().is_none());
    }

    #[test]
    fn transport_moves_endpoint_model_and_provider_together() {
        let native = ResolvedJevPrune::from_config(Some(&JevPruneConfig {
            enabled: true,
            transport: Some(JevTransport::Native),
            ..Default::default()
        }));
        assert_eq!(native.transport, JevTransport::Native);
        assert_eq!(native.endpoint, JevTransport::Native.endpoint());
        assert_eq!(native.model, "jev-latest");
        assert!(!native.model.starts_with("~typesafe/"));

        let openrouter = ResolvedJevPrune::from_config(Some(&JevPruneConfig {
            enabled: true,
            ..Default::default()
        }));
        assert_eq!(openrouter.transport, JevTransport::Openrouter);
        assert_eq!(openrouter.endpoint, DEFAULT_ENDPOINT);
        assert_eq!(openrouter.model, DEFAULT_MODEL);
    }

    #[test]
    fn explicit_endpoint_and_model_still_win_over_the_transport() {
        let cfg = JevPruneConfig {
            enabled: true,
            transport: Some(JevTransport::Native),
            endpoint: Some("http://127.0.0.1:9/decisions".to_owned()),
            model: Some("custom-model".to_owned()),
            ..Default::default()
        };
        let resolved = ResolvedJevPrune::from_config(Some(&cfg));
        assert_eq!(resolved.endpoint, "http://127.0.0.1:9/decisions");
        assert_eq!(resolved.model, "custom-model");
    }

    #[test]
    fn native_never_sends_the_provider_block() {
        let cfg = JevPruneConfig {
            enabled: true,
            transport: Some(JevTransport::Native),
            zdr: Some(true),
            require_parameters: Some(true),
            ..Default::default()
        };
        assert!(
            ResolvedJevPrune::from_config(Some(&cfg))
                .provider_block()
                .is_none()
        );
    }

    #[test]
    fn derived_session_key_is_namespaced_and_stable() {
        assert_eq!(derived_session_key("01a0b71b"), "01a0b71b:jev");
        assert_eq!(
            derived_session_key("01a0b71b"),
            derived_session_key("01a0b71b")
        );
        assert_ne!(derived_session_key("01a0b71b"), "01a0b71b");
    }

    #[test]
    fn blank_strings_fall_back_to_defaults() {
        let cfg = JevPruneConfig {
            enabled: true,
            model: Some("   ".to_owned()),
            endpoint: Some(String::new()),
            api_key_env: Some("  ".to_owned()),
            ..Default::default()
        };
        let resolved = ResolvedJevPrune::from_config(Some(&cfg));
        assert!(resolved.is_enabled());
        assert_eq!(resolved.model, DEFAULT_MODEL);
        assert_eq!(resolved.endpoint, DEFAULT_ENDPOINT);
        assert_eq!(resolved.api_key_env, None);
    }

    #[test]
    fn provider_block_omits_absent_knobs() {
        let cfg = JevPruneConfig {
            transport: Some(JevTransport::Openrouter),
            zdr: Some(true),
            require_parameters: Some(true),
            ..Default::default()
        };
        let block = ResolvedJevPrune::from_config(Some(&cfg))
            .provider_block()
            .expect("provider block");
        assert_eq!(block["zdr"], serde_json::json!(true));
        assert_eq!(block["require_parameters"], serde_json::json!(true));
        assert!(block.get("data_collection").is_none());
    }

    #[test]
    fn outcome_labels_are_stable() {
        assert_eq!(PruneOutcome::Applied.as_str(), "applied");
        assert_eq!(PruneOutcome::default().as_str(), "skipped_disabled");
        assert_eq!(
            PruneOutcome::Failed("boom".to_owned()).as_str(),
            "failed_fallback"
        );
    }
}
