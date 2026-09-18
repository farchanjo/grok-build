//! Jev-guided compaction pruning.
//!
//! A compaction asks Jev, for every tool call outside the pinned first item
//! and the newest window, two `noul` questions: whether the call still
//! matters, and whether its full result must stay verbatim. The answer is
//! three-way — keep both, keep the call with a truncated result, or drop call
//! and result together — and it is applied to **the view handed to the
//! summarizer only**. Chat state, the CAS identity and rewind semantics are
//! untouched.
//!
//! Decide once ([`decide`]), apply anywhere ([`apply_decisions`]). Applying is
//! pure and keyed by `tool_use_id`, so the input ladder can re-apply the same
//! decisions after it re-fetches the conversation from chat state.
//!
//! Every failure is a [`PruneError`] wrapped in a [`PruneFailure`] carrying the
//! counters reached so far; the caller logs it and continues with the unpruned
//! view.

mod apply;
mod client;
mod state;
mod types;

pub use apply::{PruneFailure, apply_decisions, decide};
pub use client::{JEV_API_KEY_ENV, JevClient};
pub use state::collect_candidates;
pub use types::{
    DEFAULT_ENDPOINT, DEFAULT_KEEP_THRESHOLD, DEFAULT_MAX_REQUEST_TOKENS, DEFAULT_MAX_STATE_TOKENS,
    DEFAULT_MODEL, DEFAULT_PRESERVE_RECENT_MESSAGES, DEFAULT_TIMEOUT_MS,
    DEFAULT_TRUNCATE_HEAD_CHARS, KeepDecision, PruneDecisions, PruneError, PruneOutcome,
    PruneStats, ResolvedJevPrune, ToolPair,
};

#[cfg(test)]
mod tests {
    use super::*;
    use xai_grok_inference_types::{ConversationItem, ToolCall};

    fn conversation(pairs: usize, result_chars: usize) -> Vec<ConversationItem> {
        let mut items = vec![ConversationItem::system("system")];
        for index in 0..pairs {
            let id = format!("call_{index}");
            let mut assistant = ConversationItem::assistant(format!("call {index}"));
            if let ConversationItem::Assistant(assistant) = &mut assistant {
                assistant.tool_calls.push(ToolCall {
                    id: id.as_str().into(),
                    name: "read_file".to_owned(),
                    arguments: format!("{{\"path\":\"file{index}.rs\"}}").into(),
                });
            }
            items.push(ConversationItem::user(format!("ask {index}")));
            items.push(assistant);
            items.push(ConversationItem::tool_result(&id, "x".repeat(result_chars)));
        }
        items
    }

    fn decisions(entries: &[(&str, f64, f64)]) -> PruneDecisions {
        let mut decisions = PruneDecisions::with_threshold(DEFAULT_KEEP_THRESHOLD);
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

    /// The authoritative items keep their fingerprint: pruning rewrites the
    /// view, never the source of `CompactSourceIdentity`.
    #[test]
    fn pruning_the_view_leaves_the_source_fingerprint_intact() {
        let authoritative = conversation(4, 4_000);
        let before = xai_chat_state::fingerprint_conversation_items(&authoritative).unwrap();
        let pruned = apply_decisions(
            &authoritative,
            &decisions(&[("call_1", 0.0, 0.0), ("call_2", 0.9, 0.1)]),
            300,
        );
        let after = xai_chat_state::fingerprint_conversation_items(&authoritative).unwrap();
        assert_eq!(before, after);
        assert!(
            serde_json::to_value(&pruned).unwrap() != serde_json::to_value(&authoritative).unwrap(),
            "the view really did change"
        );
    }

    /// Pinned calls are never candidates, so the first item and the newest
    /// window survive any decision set.
    #[test]
    fn pinned_calls_survive() {
        let items = conversation(3, 4_000);
        let calls = collect_candidates(&items, 2);
        let unpinned: Vec<&ToolPair> = calls.iter().filter(|call| !call.pinned).collect();
        assert_eq!(
            unpinned.len(),
            2,
            "only the two oldest calls are candidates"
        );
        assert!(unpinned.iter().all(|call| call.id != "t3"));
        // `decide` only asks about unpinned calls, so only they carry a
        // decision; the pinned newest result survives untouched.
        let pruned = apply_decisions(
            &items,
            &decisions(&[("call_0", 0.0, 0.0), ("call_1", 0.0, 0.0)]),
            300,
        );
        assert!(pruned.iter().any(|item| matches!(
            item,
            ConversationItem::ToolResult(result) if result.tool_call_id == "call_2"
        )));
        assert!(!pruned.iter().any(|item| matches!(
            item,
            ConversationItem::ToolResult(result) if result.tool_call_id == "call_1"
        )));
    }
}
