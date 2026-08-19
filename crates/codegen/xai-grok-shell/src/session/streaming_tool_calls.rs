//! Mid-stream ACP tool-call cards for Chat Completions / Messages deltas.
//!
//! The sampler already emits [`xai_grok_inference::InferenceEvent::ToolCallDelta`]
//! as the model streams `delta.tool_calls`. This accumulator turns those
//! fragments into a first ACP `ToolCall` (Pending) plus later
//! `ToolCallUpdate`s. Execution still happens only in
//! `prepare_tool_call` / `execute_tool_calls` after the sampler completes.
//!
//! Partial argument strings are **not** parsed as tool input. Incomplete JSON
//! is surfaced as `{"raw": "..."}` so the pager can refresh the card without
//! dispatching the tool.

use std::collections::BTreeMap;

use serde_json::{Value, json};

/// One in-flight streamed tool call, keyed by the model's `tool_index`.
#[derive(Debug, Default, Clone)]
pub(crate) struct StreamingToolCall {
    pub(crate) id: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) arguments: String,
    /// True after the first ACP `ToolCall` notification was sent.
    pub(crate) announced: bool,
}

/// What the session actor should emit after applying one delta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StreamingToolCallEmit {
    None,
    Announce {
        id: String,
        name: String,
        raw_input: Option<Value>,
    },
    Update {
        id: String,
        raw_input: Option<Value>,
    },
}

/// Per-inference-call accumulator. Cleared on each `StreamStarted`.
#[derive(Debug, Default, Clone)]
pub(crate) struct StreamingToolCallAcc {
    by_index: BTreeMap<u32, StreamingToolCall>,
}

impl StreamingToolCallAcc {
    pub(crate) fn clear(&mut self) {
        self.by_index.clear();
    }

    /// Call ids that already have an ACP card on screen.
    pub(crate) fn announced_ids(&self) -> Vec<String> {
        self.by_index
            .values()
            .filter(|c| c.announced)
            .filter_map(|c| c.id.clone())
            .collect()
    }

    pub(crate) fn is_announced(&self, call_id: &str) -> bool {
        self.by_index
            .values()
            .any(|c| c.announced && c.id.as_deref() == Some(call_id))
    }

    /// Drop a call that `prepare_tool_call` has taken over so the next
    /// `StreamStarted` does not mark an already-dispatched card Failed.
    pub(crate) fn forget(&mut self, call_id: &str) {
        self.by_index
            .retain(|_, c| c.id.as_deref() != Some(call_id));
    }

    /// Apply one sampler delta. Marks `announced` when returning [`StreamingToolCallEmit::Announce`].
    pub(crate) fn apply_delta(
        &mut self,
        tool_index: u32,
        id: Option<String>,
        name: Option<String>,
        arguments_delta: Option<String>,
    ) -> StreamingToolCallEmit {
        let entry = self.by_index.entry(tool_index).or_default();
        if let Some(id) = id {
            entry.id = Some(id);
        }
        if let Some(name) = name {
            entry.name = Some(name);
        }
        if let Some(delta) = arguments_delta {
            entry.arguments.push_str(&delta);
        }

        let Some(id) = entry.id.clone() else {
            return StreamingToolCallEmit::None;
        };
        let Some(name) = entry.name.clone() else {
            return StreamingToolCallEmit::None;
        };
        let raw_input = raw_input_for_args(&entry.arguments);
        if !entry.announced {
            entry.announced = true;
            return StreamingToolCallEmit::Announce {
                id,
                name,
                raw_input,
            };
        }
        StreamingToolCallEmit::Update { id, raw_input }
    }
}

/// Parsed object when `args` is complete JSON; otherwise `{"raw": args}` so
/// the pager can show the growing fragment without treating it as tool input.
pub(crate) fn raw_input_for_args(args: &str) -> Option<Value> {
    let trimmed = args.trim();
    if trimmed.is_empty() {
        return None;
    }
    match serde_json::from_str::<Value>(trimmed) {
        Ok(value) => Some(value),
        Err(_) => Some(json!({ "raw": args })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits_for_id_and_name_before_announce() {
        let mut acc = StreamingToolCallAcc::default();
        assert_eq!(
            acc.apply_delta(0, None, None, Some("{".into())),
            StreamingToolCallEmit::None
        );
        assert_eq!(
            acc.apply_delta(0, Some("call-1".into()), None, None),
            StreamingToolCallEmit::None
        );
        match acc.apply_delta(0, None, Some("read_file".into()), None) {
            StreamingToolCallEmit::Announce {
                id,
                name,
                raw_input,
            } => {
                assert_eq!(id, "call-1");
                assert_eq!(name, "read_file");
                assert_eq!(raw_input, Some(json!({ "raw": "{" })));
            }
            other => panic!("expected Announce, got {other:?}"),
        }
        assert!(acc.is_announced("call-1"));
    }

    #[test]
    fn subsequent_deltas_are_updates_and_parse_complete_json() {
        let mut acc = StreamingToolCallAcc::default();
        let first = acc.apply_delta(
            0,
            Some("call-1".into()),
            Some("get_weather".into()),
            Some("{".into()),
        );
        assert!(matches!(first, StreamingToolCallEmit::Announce { .. }));
        assert_eq!(
            acc.apply_delta(0, None, None, Some(r#""city": "Recife""#.into())),
            StreamingToolCallEmit::Update {
                id: "call-1".into(),
                raw_input: Some(json!({ "raw": r#"{"city": "Recife""# })),
            }
        );
        assert_eq!(
            acc.apply_delta(0, None, None, Some("}".into())),
            StreamingToolCallEmit::Update {
                id: "call-1".into(),
                raw_input: Some(json!({ "city": "Recife" })),
            }
        );
    }

    #[test]
    fn parallel_indexes_are_independent() {
        let mut acc = StreamingToolCallAcc::default();
        let a = acc.apply_delta(0, Some("call-a".into()), Some("read_file".into()), None);
        let b = acc.apply_delta(1, Some("call-b".into()), Some("web_search".into()), None);
        assert!(matches!(a, StreamingToolCallEmit::Announce { id, .. } if id == "call-a"));
        assert!(matches!(b, StreamingToolCallEmit::Announce { id, .. } if id == "call-b"));
        assert_eq!(acc.announced_ids().len(), 2);
        acc.forget("call-a");
        assert!(!acc.is_announced("call-a"));
        assert!(acc.is_announced("call-b"));
        assert_eq!(acc.announced_ids(), vec!["call-b".to_string()]);
    }
}
