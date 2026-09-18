//! State construction, token estimation and the fitting ladder.
//!
//! The state is the whole conversation, oldest first, with every tool result
//! replaced by a short `ok|error, N chars (omitted)` note. It is shrunk in
//! stages until it fits `max_state_tokens`; when even the last stage does not
//! fit, the caller falls back to the unpruned view.

use serde_json::{Value, json};
use xai_grok_inference_types::{ContentPart, ConversationItem};

use super::types::{PruneError, ToolPair};

/// Fixed context line sent with every state.
pub const STATE_CONTEXT: &str = "A coding assistant conversation is being compacted to free context. `history` is the whole \
     conversation so far, oldest first; tool outputs are replaced by a short `result` note and \
     long texts may be abridged. Each question asks whether one tool call, or the full output of \
     that call, still needs to stay in the history verbatim. Whatever is not kept is deleted \
     permanently, but the assistant can always re-run a tool or re-read a file.";

/// Successive caps on the serialized tool input included per call.
const INPUT_CHARS: [usize; 3] = [1000, 200, 60];
/// Head kept when a long text is abridged.
const TEXT_HEAD: usize = 400;
/// Tail kept when a long text is abridged.
const TEXT_TAIL: usize = 150;
/// Longest user text contributed to the `goal`.
const GOAL_TEXT_CHARS: usize = 500;
/// How many trailing user texts form the `goal`.
const GOAL_USER_TEXTS: usize = 3;

/// Tokens the request envelope (`model`, key names) adds around state and questions.
pub(crate) const REQUEST_OVERHEAD_TOKENS: usize = 20;

/// A fitted state plus the stage that produced it.
#[derive(Debug, Clone)]
pub struct FittedState {
    /// `{context, goal, history}` sent as the `state` of every request.
    pub state: Value,
    /// Estimated tokens of `state`.
    pub tokens: usize,
    /// Which fitting stage produced it, for diagnostics.
    pub stage: &'static str,
}

/// Estimates tokens without a tokenizer: a word costs one token per six
/// letters, a digit half a token, any other symbol nine tenths. Calibrated
/// against the usage Jev reports for real transcripts, where it lands 2–18%
/// above the true count; a plain characters-per-token ratio undercounts the
/// JSON-heavy states by up to 40%.
pub fn estimate_tokens(text: &str) -> usize {
    let mut tokens = 0.0_f64;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_whitespace() {
            continue;
        }
        if c.is_ascii_digit() {
            let mut len = 1_usize;
            while chars.peek().is_some_and(char::is_ascii_digit) {
                chars.next();
                len += 1;
            }
            tokens += len as f64 / 2.0;
        } else if c.is_ascii_alphabetic() {
            let mut len = 1_usize;
            while chars.peek().is_some_and(char::is_ascii_alphabetic) {
                chars.next();
                len += 1;
            }
            tokens += 1.0 + ((len - 1) / 6) as f64;
        } else {
            tokens += 0.9;
        }
    }
    tokens.ceil() as usize
}

/// Character-safe truncation with an ellipsis.
pub fn truncate(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(limit.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// First `limit` characters of `text`, on a char boundary.
fn head(text: &str, limit: usize) -> &str {
    match text.char_indices().nth(limit) {
        Some((index, _)) => &text[..index],
        None => text,
    }
}

/// Head+tail abridgement for long texts.
fn abridge(text: &str, head_chars: usize, tail_chars: usize) -> String {
    let total = text.chars().count();
    if total <= head_chars + tail_chars + 40 {
        return text.to_owned();
    }
    let omitted = total - head_chars - tail_chars;
    let tail: String = text.chars().skip(total - tail_chars).collect::<String>();
    format!(
        "{}\n[… {omitted} chars omitted …]\n{tail}",
        head(text, head_chars)
    )
}

/// Whether an item index is inside the pinned window.
pub fn is_pinned(index: usize, total: usize, preserve_recent_messages: usize) -> bool {
    index == 0 || index >= total.saturating_sub(preserve_recent_messages)
}

fn item_text(item: &ConversationItem) -> String {
    match item {
        ConversationItem::User(user) => user
            .content
            .iter()
            .filter_map(|part| match part {
                ContentPart::Text { text } => Some(text.as_ref()),
                ContentPart::Image { .. } => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        other => other.text_content(),
    }
}

/// Entry role. Tool results are folded into their call's note, so they are not
/// entries of their own.
fn role_of(item: &ConversationItem) -> Option<&'static str> {
    match item {
        ConversationItem::System(_) => Some("system"),
        ConversationItem::User(_) => Some("user"),
        ConversationItem::Assistant(_) => Some("assistant"),
        ConversationItem::ToolResult(_) => None,
        ConversationItem::BackendToolCall(_) => Some("tool"),
        ConversationItem::Reasoning(_) => Some("reasoning"),
    }
}

/// Pairs every tool call with its result by `tool_use_id`. Calls without a
/// result are not candidates: there is nothing to drop yet.
pub fn collect_candidates(
    items: &[ConversationItem],
    preserve_recent_messages: usize,
) -> Vec<ToolPair> {
    let total = items.len();
    let mut results: std::collections::HashMap<&str, (usize, bool, usize)> =
        std::collections::HashMap::new();
    for (index, item) in items.iter().enumerate() {
        if let ConversationItem::ToolResult(result) = item {
            results.insert(
                result.tool_call_id.as_str(),
                (
                    index,
                    result.is_error.unwrap_or(false),
                    result.content.chars().count(),
                ),
            );
        }
    }
    let mut pairs = Vec::new();
    for (call_index, item) in items.iter().enumerate() {
        let ConversationItem::Assistant(assistant) = item else {
            continue;
        };
        for call in &assistant.tool_calls {
            let Some((result_index, is_error, result_chars)) =
                results.get(call.id.as_ref()).copied()
            else {
                continue;
            };
            pairs.push(ToolPair {
                id: format!("t{}", pairs.len() + 1),
                tool_use_id: call.id.to_string(),
                tool: call.name.clone(),
                input: call.arguments.to_string(),
                call_index,
                result_index,
                result_chars,
                is_error,
                pinned: is_pinned(call_index, total, preserve_recent_messages)
                    || is_pinned(result_index, total, preserve_recent_messages),
            });
        }
    }
    pairs
}

/// The last few user prompts, as the default `goal`.
pub fn goal_from_items(items: &[ConversationItem]) -> String {
    let texts: Vec<(bool, String)> = items
        .iter()
        .filter_map(|item| match item {
            ConversationItem::User(user) => {
                let text = item_text(item);
                (!text.trim().is_empty()).then(|| (user.synthetic_reason.is_none(), text))
            }
            _ => None,
        })
        .collect();
    let real: Vec<&(bool, String)> = texts.iter().filter(|(real, _)| *real).collect();
    let chosen: Vec<&(bool, String)> = if real.is_empty() {
        texts.iter().collect()
    } else {
        real
    };
    chosen
        .into_iter()
        .rev()
        .take(GOAL_USER_TEXTS)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|(_, text)| truncate(text, GOAL_TEXT_CHARS))
        .collect::<Vec<_>>()
        .join("\n")
}

/// One history entry of the state.
#[derive(Debug, Clone)]
struct Entry {
    /// Index of the source item.
    i: usize,
    role: &'static str,
    text: String,
    /// Structured per call, or one compact line per call once the state shrinks.
    calls: Option<Vec<Value>>,
}

impl Entry {
    fn to_json(&self) -> Value {
        let mut map = serde_json::Map::new();
        map.insert("i".to_owned(), json!(self.i));
        map.insert("role".to_owned(), json!(self.role));
        map.insert("text".to_owned(), json!(self.text));
        if let Some(calls) = &self.calls {
            map.insert("tool_calls".to_owned(), Value::Array(calls.clone()));
        }
        Value::Object(map)
    }
}

fn input_text(input: &str, limit: usize) -> String {
    truncate(
        &input.split_whitespace().collect::<Vec<_>>().join(" "),
        limit,
    )
}

/// One call as a single line, for when the structured form is too costly.
fn compact_call(call: &ToolPair) -> String {
    let input = match serde_json::from_str::<Value>(&call.input) {
        Ok(Value::Object(map)) => map
            .iter()
            .map(|(key, value)| {
                let text = match value {
                    Value::String(text) => text.clone(),
                    other => other.to_string(),
                };
                format!(
                    "{key}={}",
                    text.split_whitespace().collect::<Vec<_>>().join(" ")
                )
            })
            .collect::<Vec<_>>()
            .join(" "),
        _ => call.input.clone(),
    };
    format!(
        "{} {} {} → {} {}ch",
        call.id,
        call.tool,
        truncate(&input, INPUT_CHARS[2]),
        if call.is_error { "error" } else { "ok" },
        call.result_chars
    )
}

fn result_note(call: &ToolPair) -> String {
    format!(
        "{}, {} chars (omitted)",
        if call.is_error { "error" } else { "ok" },
        call.result_chars
    )
}

fn calls_by_item(calls: &[ToolPair]) -> std::collections::HashMap<usize, Vec<&ToolPair>> {
    let mut by_item: std::collections::HashMap<usize, Vec<&ToolPair>> =
        std::collections::HashMap::new();
    for call in calls {
        by_item.entry(call.call_index).or_default().push(call);
    }
    by_item
}

fn build_entries(items: &[ConversationItem], calls: &[ToolPair], input_chars: usize) -> Vec<Entry> {
    let by_item = calls_by_item(calls);
    let mut entries = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let Some(role) = role_of(item) else {
            continue;
        };
        let text = item_text(item);
        let own: Vec<Value> = by_item
            .get(&index)
            .map(|list| {
                list.iter()
                    .map(|call| {
                        json!({
                            "id": call.id,
                            "tool": call.tool,
                            "input": input_text(&call.input, input_chars),
                            "result": result_note(call),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        if text.trim().is_empty() && own.is_empty() {
            continue;
        }
        entries.push(Entry {
            i: index,
            role,
            text,
            calls: (!own.is_empty()).then_some(own),
        });
    }
    entries
}

fn merge_call_runs(history: &[Entry], pinned: &dyn Fn(&Entry) -> bool) -> Vec<Entry> {
    let foldable = |entry: &Entry| {
        !pinned(entry)
            && entry.text.is_empty()
            && matches!(
                entry.calls.as_ref().and_then(|calls| calls.first()),
                Some(Value::String(_))
            )
    };
    let mut merged: Vec<Entry> = Vec::new();
    for entry in history {
        if let Some(previous) = merged.last_mut() {
            if foldable(previous) && foldable(entry) && previous.role == entry.role {
                let mut combined = previous.calls.take().unwrap_or_default();
                combined.extend(entry.calls.clone().unwrap_or_default());
                previous.calls = Some(combined);
                continue;
            }
        }
        merged.push(entry.clone());
    }
    merged
}

fn state_of(history: Vec<Value>, goal: &str) -> Value {
    json!({
        "context": STATE_CONTEXT,
        "goal": goal,
        "history": history,
    })
}

fn entry_tokens(entry: &Entry) -> usize {
    estimate_tokens(&entry.to_json().to_string()) + 1
}

/// Builds the state and shrinks it in stages until it fits `max_state_tokens`.
///
/// Stages, in order: tool inputs 1000 → 200 → 60 characters, abridge long
/// texts oldest-first (pinned last), collapse old messages to a one-line note,
/// one line per old call, drop old call-less messages, fold runs of old
/// call-only entries. Errors when even that does not fit.
pub fn fit_state(
    items: &[ConversationItem],
    calls: &[ToolPair],
    max_state_tokens: usize,
    preserve_recent_messages: usize,
    goal: &str,
) -> Result<FittedState, PruneError> {
    let goal = if goal.trim().is_empty() {
        goal_from_items(items)
    } else {
        goal.to_owned()
    };
    let base_tokens = estimate_tokens(&state_of(Vec::new(), &goal).to_string());

    let mut history = build_entries(items, calls, INPUT_CHARS[0]);
    let mut per_entry: Vec<usize> = history.iter().map(entry_tokens).collect();
    let mut tokens = base_tokens + per_entry.iter().sum::<usize>();
    let fits = |tokens: usize| tokens <= max_state_tokens;
    let fitted = |history: &[Entry], tokens: usize, stage: &'static str| FittedState {
        state: state_of(history.iter().map(Entry::to_json).collect(), &goal),
        tokens,
        stage,
    };
    if fits(tokens) {
        return Ok(fitted(&history, tokens, "full"));
    }

    for limit in INPUT_CHARS.iter().skip(1) {
        history = build_entries(items, calls, *limit);
        per_entry = history.iter().map(entry_tokens).collect();
        tokens = base_tokens + per_entry.iter().sum::<usize>();
        if fits(tokens) {
            let stage: &'static str = match limit {
                200 => "inputs<=200",
                _ => "inputs<=60",
            };
            return Ok(fitted(&history, tokens, stage));
        }
    }

    let total = items.len();
    let pinned = |entry: &Entry| is_pinned(entry.i, total, preserve_recent_messages);
    let order: Vec<usize> = {
        let unpinned: Vec<usize> = (0..history.len())
            .filter(|index| !pinned(&history[*index]))
            .collect();
        let pinned_idx: Vec<usize> = (0..history.len())
            .filter(|index| pinned(&history[*index]))
            .collect();
        unpinned.into_iter().chain(pinned_idx).collect()
    };
    let mut shrink = |history: &mut Vec<Entry>,
                      per_entry: &mut Vec<usize>,
                      tokens: &mut usize,
                      index: usize,
                      change: &dyn Fn(&mut Entry)| {
        let Some(entry) = history.get_mut(index) else {
            return;
        };
        change(entry);
        let now = entry_tokens(entry);
        *tokens = tokens.saturating_sub(per_entry[index]) + now;
        per_entry[index] = now;
    };

    for index in &order {
        let entry = &history[*index];
        if entry.text.chars().count() <= TEXT_HEAD + TEXT_TAIL + 40 {
            continue;
        }
        shrink(
            &mut history,
            &mut per_entry,
            &mut tokens,
            *index,
            &|entry| {
                entry.text = abridge(&entry.text, TEXT_HEAD, TEXT_TAIL);
            },
        );
        if fits(tokens) {
            return Ok(fitted(&history, tokens, "texts abridged"));
        }
    }

    for index in &order {
        let entry = &history[*index];
        if pinned(entry) || entry.text.is_empty() {
            continue;
        }
        let original = items
            .get(entry.i)
            .map(|item| item_text(item).chars().count())
            .unwrap_or_else(|| entry.text.chars().count());
        shrink(
            &mut history,
            &mut per_entry,
            &mut tokens,
            *index,
            &|entry| {
                entry.text = format!("[… {original} chars omitted …]");
            },
        );
        if fits(tokens) {
            return Ok(fitted(&history, tokens, "old messages collapsed"));
        }
    }

    let by_item = calls_by_item(calls);
    for index in &order {
        let entry = &history[*index];
        let Some(own) = by_item.get(&entry.i) else {
            continue;
        };
        if pinned(entry) {
            continue;
        }
        let lines: Vec<Value> = own
            .iter()
            .map(|call| Value::String(compact_call(call)))
            .collect();
        shrink(
            &mut history,
            &mut per_entry,
            &mut tokens,
            *index,
            &|entry| {
                entry.calls = Some(lines.clone());
            },
        );
        if fits(tokens) {
            return Ok(fitted(&history, tokens, "old calls compacted"));
        }
    }

    let mut left = std::collections::HashSet::new();
    for index in &order {
        let entry = &history[*index];
        if pinned(entry) || entry.calls.is_some() {
            continue;
        }
        left.insert(*index);
        tokens = tokens.saturating_sub(per_entry[*index]);
        if fits(tokens) {
            let kept: Vec<Entry> = history
                .iter()
                .enumerate()
                .filter(|(index, _)| !left.contains(index))
                .map(|(_, entry)| entry.clone())
                .collect();
            return Ok(fitted(&kept, tokens, "old messages left out"));
        }
    }

    let kept: Vec<Entry> = history
        .iter()
        .enumerate()
        .filter(|(index, _)| !left.contains(index))
        .map(|(_, entry)| entry.clone())
        .collect();
    let merged = merge_call_runs(&kept, &|entry| pinned(entry));
    per_entry = merged.iter().map(entry_tokens).collect();
    tokens = base_tokens + per_entry.iter().sum::<usize>();
    if fits(tokens) {
        return Ok(fitted(&merged, tokens, "old calls merged"));
    }

    Err(PruneError::StateTooLarge {
        tokens,
        limit: max_state_tokens,
    })
}

/// The two `noul` questions asked about one call: keep the call, keep its result.
pub fn questions_for(call: &ToolPair) -> Value {
    json!({
        format!("call_{}", call.id): {
            "type": "noul",
            "instructions": format!(
                "Tool call {} ({}) should stay in the history: knowing this call was made, with \
                 its input, still matters for what the assistant does next",
                call.id, call.tool
            ),
        },
        format!("result_{}", call.id): {
            "type": "noul",
            "instructions": format!(
                "The full output of tool call {} ({}, {} chars) should stay in the history \
                 verbatim: the assistant still needs its contents and re-running the tool would \
                 not do",
                call.id, call.tool, call.result_chars
            ),
        },
    })
}

/// Splits the candidate calls into batches whose questions, together with the
/// (always complete) state, fit one request.
pub fn batch_calls(
    calls: &[ToolPair],
    state_tokens: usize,
    max_request_tokens: usize,
) -> Result<Vec<Vec<ToolPair>>, PruneError> {
    let budget = max_request_tokens.saturating_sub(state_tokens + REQUEST_OVERHEAD_TOKENS);
    let mut batches: Vec<Vec<ToolPair>> = Vec::new();
    let mut current: Vec<ToolPair> = Vec::new();
    let mut current_tokens = 0_usize;
    for call in calls {
        let tokens = estimate_tokens(&questions_for(call).to_string());
        if !current.is_empty() && current_tokens + tokens > budget {
            batches.push(std::mem::take(&mut current));
            current_tokens = 0;
        }
        if current.is_empty() && tokens > budget {
            return Err(PruneError::NoRoomForQuestions {
                state_tokens,
                limit: max_request_tokens,
            });
        }
        current.push(call.clone());
        current_tokens += tokens;
    }
    if !current.is_empty() {
        batches.push(current);
    }
    Ok(batches)
}

#[cfg(test)]
mod tests {
    use super::*;
    use xai_grok_inference_types::ToolCall;

    fn assistant_with_call(index: u32, id: &str, tool: &str) -> ConversationItem {
        let mut item = ConversationItem::assistant(format!("call {index}"));
        if let ConversationItem::Assistant(assistant) = &mut item {
            assistant.tool_calls.push(ToolCall {
                id: id.into(),
                name: tool.to_owned(),
                arguments: format!("{{\"path\":\"file{index}.rs\"}}").into(),
            });
        }
        item
    }

    fn result(id: &str, chars: usize, is_error: bool) -> ConversationItem {
        let content = "x".repeat(chars);
        if is_error {
            ConversationItem::tool_result_error(id, content)
        } else {
            ConversationItem::tool_result(id, content)
        }
    }

    fn history(calls: usize, result_chars: usize) -> Vec<ConversationItem> {
        let mut items = vec![ConversationItem::system("system")];
        for index in 0..calls {
            let id = format!("call_{index}");
            items.push(ConversationItem::user(format!("ask {index}")));
            items.push(assistant_with_call(index as u32, &id, "read_file"));
            items.push(result(&id, result_chars, index % 2 == 1));
        }
        items
    }

    /// History shaped so every fitting stage has something to do: long tool
    /// inputs, long call-free texts, and adjacent call-only entries.
    fn stage_ladder_history() -> Vec<ConversationItem> {
        let mut items = vec![ConversationItem::system("system")];
        for index in 0..10 {
            let id = format!("call_{index}");
            let mut assistant = ConversationItem::assistant("");
            if let ConversationItem::Assistant(assistant) = &mut assistant {
                assistant.tool_calls.push(ToolCall {
                    id: id.as_str().into(),
                    name: "read_file".to_owned(),
                    arguments: format!(
                        "{{\"path\":\"file{index}.rs\",\"pad\":\"{}\"}}",
                        "p".repeat(1_500)
                    )
                    .into(),
                });
            }
            items.push(assistant);
            items.push(ConversationItem::tool_result(&id, "x".repeat(6_000)));
        }
        for index in 0..6 {
            items.push(ConversationItem::user(format!(
                "note {index} {}",
                "n".repeat(2_000)
            )));
        }
        items.push(ConversationItem::user("last question"));
        items.push(ConversationItem::assistant("last answer"));
        items
    }

    #[test]
    fn token_estimate_weights_words_digits_and_symbols() {
        assert_eq!(estimate_tokens(""), 0);
        // six letters cost one token; one more letter starts a second.
        assert_eq!(estimate_tokens("abcdef"), 1);
        assert_eq!(estimate_tokens("abcdefg"), 2);
        // digits cost half a token each, rounded up at the end.
        assert_eq!(estimate_tokens("1234"), 2);
        // any other symbol costs nine tenths.
        assert_eq!(estimate_tokens("{}"), 2);
        assert_eq!(estimate_tokens("...."), 4);
        // whitespace is free.
        assert_eq!(estimate_tokens("ab cd"), 2);
    }

    #[test]
    fn candidates_pair_calls_with_results_and_pin_the_window() {
        let items = history(8, 40);
        let pairs = collect_candidates(&items, 3);
        // Every call has a result, so nothing is skipped for a missing pair.
        assert_eq!(pairs.len(), 8);
        let pinned = pairs.iter().filter(|pair| pair.pinned).count();
        assert!(pinned > 0, "the newest window must pin something");
        // The first item is the system message; no call lives there.
        assert_eq!(pairs[0].tool_use_id, "call_0");
        assert_eq!(pairs[0].result_index, 3);
    }

    #[test]
    fn calls_without_a_result_are_not_candidates() {
        let mut items = vec![ConversationItem::system("s")];
        items.push(assistant_with_call(0, "orphan", "read_file"));
        items.push(ConversationItem::user("next"));
        assert!(collect_candidates(&items, 0).is_empty());
    }

    #[test]
    fn each_fitting_stage_is_reachable() {
        let items = stage_ladder_history();
        let calls = collect_candidates(&items, 2);
        let mut stages: Vec<&'static str> = Vec::new();
        for limit in (300..=6_000).step_by(10).chain([40_000, 200_000]) {
            let Ok(fitted) = fit_state(&items, &calls, limit, 2, "goal") else {
                continue;
            };
            if stages.last() != Some(&fitted.stage) {
                stages.push(fitted.stage);
            }
        }
        // The sweep ascends in limit, so the reached stage walks the ladder
        // backwards: every stage must appear, in that reversed order.
        assert_eq!(
            stages,
            vec![
                "old calls merged",
                "old messages left out",
                "old calls compacted",
                "old messages collapsed",
                "texts abridged",
                "inputs<=60",
                "inputs<=200",
                "full",
            ]
        );
    }

    #[test]
    fn state_never_carries_result_text() {
        let items = history(4, 4_000);
        let calls = collect_candidates(&items, 1);
        let fitted = fit_state(&items, &calls, 200_000, 1, "goal").unwrap();
        let serialized = fitted.state.to_string();
        assert!(!serialized.contains(&"x".repeat(100)));
        assert!(serialized.contains("chars (omitted)"));
    }

    #[test]
    fn goal_uses_the_last_three_user_texts() {
        let mut items = Vec::new();
        for index in 0..5 {
            items.push(ConversationItem::user(format!("prompt {index}")));
        }
        let goal = goal_from_items(&items);
        assert_eq!(goal, "prompt 2\nprompt 3\nprompt 4");
    }

    #[test]
    fn batching_splits_on_the_request_budget() {
        let items = history(20, 40);
        let calls = collect_candidates(&items, 0);
        let state_tokens = 100;
        let one = batch_calls(&calls[..2], state_tokens, 200_000).unwrap();
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].len(), 2);

        let per_call = estimate_tokens(&questions_for(&calls[0]).to_string());
        // Headroom covers the longer `t10..t20` ids.
        let budget = state_tokens + REQUEST_OVERHEAD_TOKENS + per_call + 8;
        let many = batch_calls(&calls, state_tokens, budget).unwrap();
        assert_eq!(many.len(), calls.len());
        assert!(many.iter().all(|batch| batch.len() == 1));
    }

    #[test]
    fn batching_reports_when_the_state_leaves_no_room() {
        let items = history(2, 40);
        let calls = collect_candidates(&items, 0);
        let error = batch_calls(&calls, 10_000, 10_001).unwrap_err();
        assert!(matches!(error, PruneError::NoRoomForQuestions { .. }));
    }

    #[test]
    fn oversized_history_is_an_error() {
        let items = history(30, 20_000);
        let calls = collect_candidates(&items, 0);
        let error = fit_state(&items, &calls, 50, 0, "goal").unwrap_err();
        assert!(matches!(error, PruneError::StateTooLarge { .. }));
    }
}
