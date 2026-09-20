//! Pure application of prune decisions, and the decide orchestration.
//!
//! Deciding is expensive (network, once); applying is pure, cheap, and
//! re-applied wherever the compactor re-materializes its input. Decisions are
//! keyed by `tool_use_id`, never by index: the input ladder and the lossy
//! prepare both rewrite the list.

use std::collections::BTreeMap;
use std::time::Instant;

use futures_util::future::try_join_all;
use serde_json::Value;
use tokio_util::sync::CancellationToken;
use xai_grok_inference_types::ConversationItem;

use super::client::JevClient;
use super::state::{batch_calls, fit_state, questions_for};
use super::types::{
    KeepDecision, PruneDecisions, PruneError, PruneOutcome, PruneStats, ResolvedJevPrune, ToolPair,
};

/// Longest note appended to a truncated result, in characters.
const NOTE_MAX_CHARS: usize = 120;
/// Head kept for a truncated result: the note plus its separating newline.
const TRUNCATED_MAX_EXTRA_CHARS: usize = NOTE_MAX_CHARS + 1;

/// What happens to one decided call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    /// Drop the call and its result together.
    DropCall,
    /// Keep the call, truncate its result to a head plus a note.
    DropResult,
}

/// Characters of text, tool input and tool output the items hold.
fn items_chars(items: &[ConversationItem]) -> usize {
    items
        .iter()
        .map(|item| match item {
            ConversationItem::Assistant(assistant) => {
                assistant.content.chars().count()
                    + assistant
                        .tool_calls
                        .iter()
                        .map(|call| call.arguments.chars().count())
                        .sum::<usize>()
            }
            ConversationItem::ToolResult(result) => result.content.chars().count(),
            other => other.text_content().chars().count(),
        })
        .sum()
}

fn head(text: &str, limit: usize) -> &str {
    match text.char_indices().nth(limit) {
        Some((index, _)) => &text[..index],
        None => text,
    }
}

fn truncation_note(omitted: usize, is_error: bool) -> String {
    format!(
        "[jev truncated {omitted} chars of this tool result{}; re-run the tool if needed]",
        if is_error { " (error)" } else { "" }
    )
}

/// Bounded head plus a one-line note. Idempotent: the produced text always
/// fits [`TRUNCATED_MAX_EXTRA_CHARS`] over the head, so a second pass is a no-op.
fn truncate_result(text: &str, is_error: bool, head_chars: usize) -> String {
    let total = text.chars().count();
    if total <= head_chars + TRUNCATED_MAX_EXTRA_CHARS {
        return text.to_owned();
    }
    let note = truncation_note(total.saturating_sub(head_chars), is_error);
    if head_chars == 0 {
        return note;
    }
    format!("{}\n{note}", head(text, head_chars))
}

/// Rewrites the summarizer view from the decisions.
///
/// Pure and idempotent. Keyed by `tool_use_id`, so it survives any
/// index-shifting rebuild. A dropped call disappears together with its result;
/// a truncated result keeps a bounded head plus a note; user and assistant
/// text is never touched; item order is preserved. Items with nothing to do
/// are returned as they came in.
pub fn apply_decisions(
    items: &[ConversationItem],
    decisions: &PruneDecisions,
    truncate_head_chars: usize,
) -> Vec<ConversationItem> {
    if decisions.is_empty() {
        return items.to_vec();
    }
    let threshold = decisions.threshold();
    let mut actions: BTreeMap<&str, Action> = BTreeMap::new();
    for (tool_use_id, decision) in decisions.iter() {
        let action = if decision.keep_result >= threshold {
            None
        } else if decision.keep_call >= threshold {
            Some(Action::DropResult)
        } else {
            Some(Action::DropCall)
        };
        if let Some(action) = action {
            actions.insert(tool_use_id, action);
        }
    }
    if actions.is_empty() {
        return items.to_vec();
    }
    let mut kept = Vec::with_capacity(items.len());
    for item in items {
        match item {
            ConversationItem::Assistant(assistant) => {
                if !assistant
                    .tool_calls
                    .iter()
                    .any(|call| actions.contains_key(call.id.as_ref()))
                {
                    kept.push(item.clone());
                    continue;
                }
                let mut rebuilt = assistant.clone();
                rebuilt
                    .tool_calls
                    .retain(|call| actions.get(call.id.as_ref()) != Some(&Action::DropCall));
                if rebuilt.content.trim().is_empty() && rebuilt.tool_calls.is_empty() {
                    continue;
                }
                kept.push(ConversationItem::Assistant(rebuilt));
            }
            ConversationItem::ToolResult(result) => match actions.get(result.tool_call_id.as_str())
            {
                Some(Action::DropCall) => {}
                Some(Action::DropResult) => {
                    let truncated = truncate_result(
                        &result.content,
                        result.is_error.unwrap_or(false),
                        truncate_head_chars,
                    );
                    if truncated == result.content.as_ref() {
                        kept.push(item.clone());
                    } else {
                        let mut rebuilt = result.clone();
                        rebuilt.content = truncated.into();
                        kept.push(ConversationItem::ToolResult(rebuilt));
                    }
                }
                None => kept.push(item.clone()),
            },
            other => kept.push(other.clone()),
        }
    }
    kept
}

/// Validates one `noul` answer: present, numeric, finite, within `0.0..=1.0`.
fn noul_answer(answers: &Value, name: &str) -> Result<f64, PruneError> {
    let Some(answer) = answers.get(name) else {
        return Err(PruneError::InvalidAnswer {
            name: name.to_owned(),
            value: "missing".to_owned(),
        });
    };
    let Some(noul) = answer.get("noul").and_then(Value::as_f64) else {
        return Err(PruneError::InvalidAnswer {
            name: name.to_owned(),
            value: answer.to_string(),
        });
    };
    if !noul.is_finite() || !(0.0..=1.0).contains(&noul) {
        return Err(PruneError::InvalidAnswer {
            name: name.to_owned(),
            value: noul.to_string(),
        });
    }
    Ok(noul)
}

/// Asks one batch; the state is resent with every batch.
async fn ask_batch(
    client: &JevClient,
    state: &Value,
    batch: &[ToolPair],
) -> Result<Vec<(String, KeepDecision)>, PruneError> {
    let mut questions = serde_json::Map::new();
    for call in batch {
        if let Value::Object(map) = questions_for(call) {
            questions.extend(map);
        }
    }
    // A 404 on the alpha path is loud in the log, silent in behaviour: the
    // error propagates to `decide`, which propagates to the caller's fallback.
    let answers = client.ask(state, &Value::Object(questions)).await?;
    let mut out = Vec::with_capacity(batch.len());
    for call in batch {
        let keep_call = noul_answer(&answers, &format!("call_{}", call.id))?;
        let keep_result = noul_answer(&answers, &format!("result_{}", call.id))?;
        out.push((
            call.tool_use_id.clone(),
            KeepDecision {
                keep_call,
                keep_result,
            },
        ));
    }
    Ok(out)
}

/// A [`decide`] failure together with the counters known when it happened.
///
/// Every [`PruneError`] is fail-open for the caller, which continues with the
/// unpruned view; carrying the counters keeps the failure log line as
/// informative as the success one instead of reporting zeros.
#[derive(Debug, Clone)]
pub struct PruneFailure {
    /// The underlying failure.
    pub error: PruneError,
    /// Counters at failure time; `outcome` is already `Failed`.
    pub stats: PruneStats,
}

impl std::fmt::Display for PruneFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error)
    }
}

impl std::error::Error for PruneFailure {}

/// Decides which calls and results stay, once per compaction.
///
/// `calls` is the full candidate list from
/// [`collect_candidates`](super::state::collect_candidates), collected
/// once by the caller so the enabled path does not walk the conversation
/// twice. `SkippedDisabled` and `SkippedNoCandidates` make no HTTP call. Every
/// failure is a [`PruneFailure`] carrying the counters reached so far; the
/// caller continues with the unpruned view.
pub async fn decide(
    items: &[ConversationItem],
    calls: &[ToolPair],
    cfg: &ResolvedJevPrune,
    client: &JevClient,
    cancel: &CancellationToken,
) -> Result<(PruneDecisions, PruneStats), PruneFailure> {
    let started = Instant::now();
    if !cfg.is_enabled() {
        return Ok((
            PruneDecisions::with_threshold(cfg.keep_threshold),
            PruneStats {
                outcome: PruneOutcome::SkippedDisabled,
                ..Default::default()
            },
        ));
    }
    let candidates: Vec<ToolPair> = calls.iter().filter(|call| !call.pinned).cloned().collect();
    let mut stats = PruneStats {
        candidates: candidates.len(),
        pinned: calls.len() - candidates.len(),
        chars_before: items_chars(items),
        chars_after: items_chars(items),
        outcome: PruneOutcome::Applied,
        ..Default::default()
    };
    // Every failure keeps the counters known at that point (plus the elapsed
    // time) and flips the outcome to `Failed`.
    let fail = |stats: &PruneStats, error: PruneError| {
        let mut stats = stats.clone();
        stats.ms = started.elapsed().as_millis() as u64;
        stats.outcome = PruneOutcome::Failed(error.to_string());
        PruneFailure { error, stats }
    };
    if cancel.is_cancelled() {
        return Err(fail(&stats, PruneError::Cancelled));
    }
    if candidates.is_empty() {
        stats.outcome = PruneOutcome::SkippedNoCandidates;
        return Ok((PruneDecisions::with_threshold(cfg.keep_threshold), stats));
    }
    let fitted = fit_state(
        items,
        calls,
        cfg.max_state_tokens,
        cfg.preserve_recent_messages,
        "",
    )
    .map_err(|error| fail(&stats, error))?;
    let batches = batch_calls(&candidates, fitted.tokens, cfg.max_request_tokens)
        .map_err(|error| fail(&stats, error))?;
    let batches_len = batches.len();
    stats.requests = batches_len;
    let answers = tokio::select! {
        answers = try_join_all(batches.iter().map(|batch| ask_batch(client, &fitted.state, batch))) => answers.map_err(|error| fail(&stats, error))?,
        () = cancel.cancelled() => return Err(fail(&stats, PruneError::Cancelled)),
    };
    let mut decisions = PruneDecisions::with_threshold(cfg.keep_threshold);
    for batch in answers {
        for (tool_use_id, decision) in batch {
            decisions.insert(tool_use_id, decision);
        }
    }
    for (_, decision) in decisions.iter() {
        if decision.keep_result >= cfg.keep_threshold {
            stats.kept += 1;
        } else if decision.keep_call >= cfg.keep_threshold {
            stats.results_truncated += 1;
        } else {
            stats.calls_dropped += 1;
        }
    }
    let pruned = apply_decisions(items, &decisions, cfg.truncate_head_chars);
    stats.chars_after = items_chars(&pruned);
    stats.state_tokens = fitted.tokens;
    stats.state_stage = fitted.stage;
    stats.requests = batches_len;
    stats.ms = started.elapsed().as_millis() as u64;
    Ok((decisions, stats))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use xai_grok_inference_types::ToolCall;

    use super::super::state::collect_candidates;

    /// The caller collects candidates once and passes them into `decide`.
    fn decide_with<'a>(
        items: &'a [ConversationItem],
        cfg: &'a ResolvedJevPrune,
        client: &'a JevClient,
        cancel: &'a CancellationToken,
    ) -> impl std::future::Future<Output = Result<(PruneDecisions, PruneStats), PruneFailure>> + 'a
    {
        let calls = collect_candidates(items, cfg.preserve_recent_messages);
        async move { decide(items, &calls, cfg, client, cancel).await }
    }

    fn call(index: usize, id: &str, result_chars: usize) -> Vec<ConversationItem> {
        let mut assistant = ConversationItem::assistant(format!("call {index}"));
        if let ConversationItem::Assistant(assistant) = &mut assistant {
            assistant.tool_calls.push(ToolCall {
                id: id.into(),
                name: "read_file".to_owned(),
                arguments: format!("{{\"path\":\"file{index}.rs\"}}").into(),
            });
        }
        vec![
            ConversationItem::user(format!("ask {index}")),
            assistant,
            ConversationItem::tool_result(id, "x".repeat(result_chars)),
        ]
    }

    /// A conversation with `pairs` call/result pairs, the first item being a
    /// system message so index 0 stays pinned.
    fn conversation(pairs: usize, result_chars: usize) -> Vec<ConversationItem> {
        let mut items = vec![ConversationItem::system("system")];
        for index in 0..pairs {
            items.extend(call(index, &format!("call_{index}"), result_chars));
        }
        items
    }

    /// `ConversationItem` has no `PartialEq`; compare on the serialized shape.
    fn same_items(left: &[ConversationItem], right: &[ConversationItem]) -> bool {
        serde_json::to_value(left).unwrap() == serde_json::to_value(right).unwrap()
    }

    fn decisions(entries: &[(&str, f64, f64)], threshold: f64) -> PruneDecisions {
        let mut decisions = PruneDecisions::with_threshold(threshold);
        for (id, keep_call, keep_result) in entries {
            decisions.insert(
                (*id).to_owned(),
                KeepDecision {
                    keep_call: *keep_call,
                    keep_result: *keep_result,
                },
            );
        }
        decisions
    }

    fn fake_client(handler: axum::Router) -> (JevClient, tokio::sync::oneshot::Sender<()>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();
        let listener = tokio::net::TcpListener::from_std(listener).unwrap();
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        tokio::spawn(async move {
            axum::serve(listener, handler)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .unwrap();
        });
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = ResolvedJevPrune {
            enabled: true,
            endpoint: format!("http://{addr}/decisions"),
            api_key_env: Some("GROK_JEV_APPLY_TEST_KEY".to_owned()),
            ..ResolvedJevPrune::disabled()
        };
        cfg.max_state_tokens = 200_000;
        cfg.max_request_tokens = 200_000;
        cfg.preserve_recent_messages = 1;
        unsafe { std::env::set_var("GROK_JEV_APPLY_TEST_KEY", "test-key") };
        let client = JevClient::new(&cfg, dir.path(), None).unwrap();
        (client, shutdown_tx)
    }

    fn answers_router(
        answer: impl Fn(usize) -> f64 + Send + Sync + 'static,
    ) -> (axum::Router, Arc<AtomicUsize>) {
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&hits);
        let answer = Arc::new(answer);
        let router = axum::Router::new().route(
            "/decisions",
            axum::routing::post(move |axum::Json(body): axum::Json<Value>| {
                let counter = Arc::clone(&counter);
                let answer = Arc::clone(&answer);
                async move {
                    let hit = counter.fetch_add(1, Ordering::SeqCst);
                    let questions = body["questions"].as_object().cloned().unwrap_or_default();
                    let mut out = serde_json::Map::new();
                    for key in questions.keys() {
                        out.insert(key.clone(), json!({ "noul": answer(hit) }));
                    }
                    axum::Json(json!({ "answers": out }))
                }
            }),
        );
        (router, hits)
    }

    // --- apply_decisions ---

    #[test]
    fn keep_leaves_items_untouched() {
        let items = conversation(2, 500);
        let result = apply_decisions(&items, &decisions(&[("call_0", 1.0, 1.0)], 0.5), 300);
        assert_eq!(result.len(), items.len());
        for (before, after) in items.iter().zip(&result) {
            assert_eq!(before.text_content(), after.text_content());
        }
    }

    #[test]
    fn drop_result_truncates_and_drop_call_removes_both() {
        let items = conversation(2, 5_000);
        let pruned = apply_decisions(
            &items,
            &decisions(&[("call_0", 0.9, 0.1), ("call_1", 0.1, 0.1)], 0.5),
            300,
        );
        // call_0 keeps its call with a truncated result; call_1's result
        // disappears with the call. The assistant item stays because it still
        // carries text ("call 1").
        assert_eq!(pruned.len(), items.len() - 1);
        let truncated = pruned
            .iter()
            .find_map(|item| match item {
                ConversationItem::ToolResult(result) if result.tool_call_id == "call_0" => {
                    Some(result.content.to_string())
                }
                _ => None,
            })
            .expect("kept result");
        assert!(truncated.ends_with("re-run the tool if needed]"));
        assert!(truncated.starts_with(&"x".repeat(300)));
        assert!(pruned.iter().all(|item| {
            match item {
                ConversationItem::Assistant(assistant) => assistant
                    .tool_calls
                    .iter()
                    .all(|call| call.id.as_ref() != "call_1"),
                _ => true,
            }
        }));
        assert!(!pruned.iter().any(|item| matches!(
            item,
            ConversationItem::ToolResult(result) if result.tool_call_id == "call_1"
        )));
    }

    #[test]
    fn apply_is_idempotent_and_pure() {
        let items = conversation(3, 5_000);
        let decisions = decisions(&[("call_0", 0.9, 0.1), ("call_2", 0.1, 0.1)], 0.5);
        let snapshot = items.clone();
        let once = apply_decisions(&items, &decisions, 300);
        let twice = apply_decisions(&once, &decisions, 300);
        assert!(same_items(&items, &snapshot), "input must not be mutated");
        assert!(same_items(&once, &twice), "apply must be idempotent");
    }

    #[test]
    fn empty_decisions_return_the_items_unchanged() {
        let items = conversation(2, 5_000);
        assert!(same_items(
            &apply_decisions(&items, &PruneDecisions::default(), 10),
            &items
        ));
    }

    #[test]
    fn decisions_survive_an_index_shifting_rebuild() {
        let items = conversation(3, 5_000);
        let decisions = decisions(&[("call_2", 0.1, 0.1)], 0.5);
        // Insert an unrelated item in front: every index shifts by one.
        let mut shifted = vec![ConversationItem::system("preamble")];
        shifted.extend(items.clone());
        let pruned = apply_decisions(&shifted, &decisions, 300);
        assert!(!pruned.iter().any(|item| matches!(
            item,
            ConversationItem::ToolResult(result) if result.tool_call_id == "call_2"
        )));
        assert!(pruned.iter().any(|item| matches!(
            item,
            ConversationItem::ToolResult(result) if result.tool_call_id == "call_1"
        )));
    }

    #[test]
    fn user_and_assistant_text_is_never_touched() {
        let items = conversation(2, 5_000);
        let pruned = apply_decisions(
            &items,
            &decisions(&[("call_0", 0.0, 0.0), ("call_1", 0.0, 0.0)], 0.5),
            300,
        );
        assert_eq!(pruned[0].text_content(), "system");
        assert_eq!(pruned[1].text_content(), "ask 0");
        assert_eq!(pruned[2].text_content(), "call 0");
    }

    #[test]
    fn truncation_note_is_stable_for_a_long_result() {
        let note = truncate_result(&"x".repeat(50_000), false, 300);
        assert_eq!(note, truncate_result(&note, false, 300));
        assert!(note.contains("chars of this tool result"));
    }

    // --- decide ---

    #[tokio::test]
    async fn disabled_makes_no_http_call() {
        let (router, hits) = answers_router(|_| 0.0);
        let (client, shutdown) = fake_client(router);
        let items = conversation(4, 2_000);
        let cfg = ResolvedJevPrune::disabled();
        let (decisions, stats) = decide_with(&items, &cfg, &client, &CancellationToken::new())
            .await
            .unwrap();
        assert!(decisions.is_empty());
        assert_eq!(stats.outcome, PruneOutcome::SkippedDisabled);
        assert_eq!(hits.load(Ordering::SeqCst), 0);
        let _ = shutdown.send(());
    }

    #[tokio::test]
    async fn no_candidates_short_circuits_before_the_request() {
        let (router, hits) = answers_router(|_| 0.0);
        let (client, shutdown) = fake_client(router);
        // Two items only: the only call is pinned by the first item.
        let items = vec![
            ConversationItem::system("system"),
            ConversationItem::assistant("hello"),
        ];
        let mut cfg = ResolvedJevPrune::disabled();
        cfg.enabled = true;
        let (decisions, stats) = decide_with(&items, &cfg, &client, &CancellationToken::new())
            .await
            .unwrap();
        assert!(decisions.is_empty());
        assert_eq!(stats.outcome, PruneOutcome::SkippedNoCandidates);
        assert_eq!(hits.load(Ordering::SeqCst), 0);
        let _ = shutdown.send(());
    }

    #[tokio::test]
    async fn threshold_splits_three_ways() {
        // Answer depends on the question name: call_* keeps the call, result_*
        // decides verbatim retention.
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&hits);
        let router = axum::Router::new().route(
            "/decisions",
            axum::routing::post(move |axum::Json(body): axum::Json<Value>| {
                let counter = Arc::clone(&counter);
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    let questions = body["questions"].as_object().cloned().unwrap_or_default();
                    let mut out = serde_json::Map::new();
                    for key in questions.keys() {
                        // t1 keeps both, t2 drops both, t3 keeps only the call.
                        let noul = if key.ends_with("t2") {
                            0.1
                        } else if key.starts_with("call_") {
                            0.9
                        } else if key.ends_with("t1") {
                            0.9
                        } else {
                            0.1
                        };
                        out.insert(key.clone(), json!({ "noul": noul }));
                    }
                    axum::Json(json!({ "answers": out }))
                }
            }),
        );
        let (client, shutdown) = fake_client(router);
        let items = conversation(3, 5_000);
        let mut cfg = ResolvedJevPrune::disabled();
        cfg.enabled = true;
        cfg.preserve_recent_messages = 0;
        let (decisions, stats) = decide_with(&items, &cfg, &client, &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(stats.outcome, PruneOutcome::Applied);
        assert_eq!(stats.kept, 1);
        assert_eq!(stats.results_truncated, 1);
        assert_eq!(stats.calls_dropped, 1);
        assert_eq!(stats.candidates, 3);
        assert!(stats.chars_after < stats.chars_before);
        assert_eq!(decisions.len(), 3);
        let _ = shutdown.send(());
    }

    #[tokio::test]
    async fn malformed_answers_are_an_error() {
        let router = axum::Router::new().route(
            "/decisions",
            axum::routing::post(|| async {
                axum::Json(json!({ "answers": { "call_t1": { "noul": "high" } } }))
            }),
        );
        let (client, shutdown) = fake_client(router);
        let items = conversation(2, 500);
        let mut cfg = ResolvedJevPrune::disabled();
        cfg.enabled = true;
        cfg.preserve_recent_messages = 1;
        let error = decide_with(&items, &cfg, &client, &CancellationToken::new())
            .await
            .unwrap_err();
        assert!(
            matches!(error.error, PruneError::InvalidAnswer { .. }),
            "{error:?}"
        );
        assert_eq!(error.stats.candidates, 1);
        assert_eq!(error.stats.requests, 1);
        let _ = shutdown.send(());
    }

    #[tokio::test]
    async fn out_of_range_answers_are_an_error() {
        let (router, _) = answers_router(|_| 1.4);
        let (client, shutdown) = fake_client(router);
        let items = conversation(2, 500);
        let mut cfg = ResolvedJevPrune::disabled();
        cfg.enabled = true;
        cfg.preserve_recent_messages = 1;
        let error = decide_with(&items, &cfg, &client, &CancellationToken::new())
            .await
            .unwrap_err();
        assert!(matches!(error.error, PruneError::InvalidAnswer { .. }));
        let _ = shutdown.send(());
    }

    /// The input ladder re-fetches the conversation from chat state and
    /// re-applies the stored decisions: the view must come out identical, and
    /// no second Jev call may happen.
    #[tokio::test]
    async fn ladder_reapplies_the_same_decisions_without_a_second_call() {
        let (router, hits) = answers_router(|_| 0.1);
        let (client, shutdown) = fake_client(router);
        let items = conversation(4, 5_000);
        let mut cfg = ResolvedJevPrune::disabled();
        cfg.enabled = true;
        cfg.preserve_recent_messages = 1;
        let (decisions, stats) = decide_with(&items, &cfg, &client, &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(stats.outcome, PruneOutcome::Applied);
        assert_eq!(
            hits.load(Ordering::SeqCst),
            1,
            "one request for all batches"
        );

        let first = apply_decisions(&items, &decisions, cfg.truncate_head_chars);
        let refetched = items.clone();
        let stepped_down = apply_decisions(&refetched, &decisions, cfg.truncate_head_chars);
        assert!(same_items(&first, &stepped_down));
        assert_eq!(hits.load(Ordering::SeqCst), 1, "no second jev call");
        let _ = shutdown.send(());
    }

    #[tokio::test]
    async fn an_already_cancelled_token_short_circuits() {
        let (router, hits) = answers_router(|_| 1.0);
        let (client, shutdown) = fake_client(router);
        let items = conversation(4, 500);
        let mut cfg = ResolvedJevPrune::disabled();
        cfg.enabled = true;
        cfg.preserve_recent_messages = 1;
        let cancel = CancellationToken::new();
        cancel.cancel();
        let error = decide_with(&items, &cfg, &client, &cancel)
            .await
            .unwrap_err();
        assert_eq!(error.error, PruneError::Cancelled);
        assert_eq!(hits.load(Ordering::SeqCst), 0);
        // The failure keeps the counters reached before the cancellation.
        assert_eq!(
            error.stats.outcome,
            PruneOutcome::Failed("jev request cancelled".to_owned())
        );
        assert_eq!(error.stats.candidates, 3);
        assert_eq!(error.stats.pinned, 1);
        let _ = shutdown.send(());
    }
}
