use serde::{Deserialize, Serialize};

/// Content-block `_meta` key for per-prompt display texts when several
/// follow-ups were combined (length ≥ 2). Empty / absent = not combined.
pub const COMBINED_DISPLAY_TEXTS_META: &str = "combinedDisplayTexts";

/// Typed wire tag describing who originated a queue row / running turn.
///
/// Additive and default-compatible: a legacy wire entry without an `origin`
/// deserializes to `None`, which consumers map fail-closed to an
/// "unknown / unclassified" origin — never to a real `User` turn. Tag-only
/// (no payloads): completion/task ids ride the prompt-id string. New variants
/// are additive; the JSON form uses snake_case.
///
/// **Display metadata only.** The shell writes this tag for clients to render
/// (e.g. distinguish a cron row); it is NEVER read back into lifecycle or
/// prime decisions. Cross-boundary origins that gate security/lifecycle are
/// carried by the typed in-memory value or the ACP prompt-request origin meta
/// tag instead.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueOrigin {
    /// A real user-initiated prompt.
    User,
    /// A stranded user interjection promoted to its own turn (side-channel).
    Interjection,
    /// Auto-wake from a completed background task.
    TaskCompleted,
    /// Auto-wake from a completed subagent.
    SubagentCompleted,
    /// Auto-wake from a completed workflow.
    WorkflowCompleted,
    /// Idle-gated notification drain.
    NotificationDrain,
    /// Orchestrator summary turn.
    GoalSummary,
    /// Scheduled task (`/loop`) prompt.
    SchedulerFired,
    /// Injected plan-resume follow-up turn.
    PlanResume,
    /// Legacy / wire entry without an explicit origin. Fail-closed: never a
    /// real user turn. Tag-only `Unknown` also absorbs any future/unknown tag
    /// (`#[serde(other)]`), so an old binary that meets an arbitrary new tag
    /// still deserializes to a safe, never-priming value instead of erroring.
    /// MUST remain the last variant (serde `other` requires a trailing catch-all).
    #[default]
    #[serde(other)]
    Unknown,
}

/// Per-item queue metadata the session actor attaches to user-originated inputs; synthetic
/// inputs (auto-wake, nudges) carry none and never appear in the visible queue. Held in
/// actor state, never serialized itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueueEntryMeta {
    /// Stable id, reusing the prompt's unique `prompt_id`.
    pub id: String,
    /// Monotonic, bumped on each in-place edit; an edit against a stale version is a no-op.
    pub version: u64,
    /// Enqueuing client identifier (attribution); never overwritten by edits.
    pub owner: Option<String>,
    /// Most recent editor's client identifier, replaced on every in-place edit.
    pub last_editor: Option<String>,
    /// Display kind label; client-cosmetic kinds resolve to their send-intent before enqueue.
    pub kind: String,
    /// Plain prompt text for the shared queue display.
    pub text: String,
    /// Per-prompt display texts when combine merged several follow-ups (len ≥ 2).
    pub combined_texts: Option<Vec<String>>,
}

/// One queue row on the wire.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QueueEntryWire {
    pub id: String,
    #[serde(default)]
    pub version: u64,
    /// Omitted from the wire when `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// Mirrors [`QueueEntryMeta::last_editor`]; omitted from the wire when `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_editor: Option<String>,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub text: String,
    /// See [`QueueEntryMeta::combined_texts`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub combined_texts: Option<Vec<String>>,
    /// Typed display origin. Omitted when `None` for the legacy wire shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<QueueOrigin>,
    /// 0-based position among queued, not-yet-running prompts.
    #[serde(default)]
    pub position: usize,
}

/// Broadcast payload for the `x.ai/queue/changed` notification.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QueueChanged {
    /// The session this queue belongs to; drives per-session fan-out routing.
    pub session_id: String,
    #[serde(default)]
    pub entries: Vec<QueueEntryWire>,
    /// The prompt the actor is currently draining, `None` when no turn runs. The correlation
    /// signal a subscriber uses to adopt `current_prompt_id` for notification routing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub running_prompt_id: Option<String>,
    /// Display text for the running prompt. Carried explicitly because the
    /// running row is omitted from [`Self::entries`]; clients use this for the
    /// turn-start user block without relying on a stale local mirror.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub running_text: Option<String>,
    /// Kind for the running prompt (`"prompt"` / `"bash"` / …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub running_kind: Option<String>,
    /// Per-prompt display texts when the running turn was combined (len ≥ 2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub running_combined_texts: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_changed_full_round_trip() {
        let original = QueueChanged {
            session_id: "sess-42".into(),
            entries: vec![
                QueueEntryWire {
                    id: "p1".into(),
                    version: 3,
                    owner: Some("alice".into()),
                    last_editor: Some("bob".into()),
                    kind: "prompt".into(),
                    text: "fix the bug".into(),
                    position: 0,
                    combined_texts: None,
                    origin: None,
                },
                QueueEntryWire {
                    id: "p2".into(),
                    version: 0,
                    owner: None,
                    last_editor: None,
                    kind: "bash".into(),
                    text: "ls -la".into(),
                    position: 1,
                    combined_texts: None,
                    origin: None,
                },
            ],
            running_prompt_id: Some("p0".into()),

            running_text: None,
            running_kind: None,
            running_combined_texts: None,
        };
        let json = serde_json::to_value(&original).unwrap();
        assert_eq!(json["sessionId"], "sess-42");
        assert_eq!(json["entries"][0]["lastEditor"], "bob");
        assert_eq!(json["runningPromptId"], "p0");
        assert!(json["entries"][1].get("owner").is_none());
        assert!(json["entries"][1].get("lastEditor").is_none());
        let round: QueueChanged = serde_json::from_value(json).unwrap();
        assert_eq!(round, original);
    }

    /// Pins the exact wire JSON; a key rename here breaks deployed clients.
    #[test]
    fn queue_changed_golden_wire_json() {
        let payload = QueueChanged {
            session_id: "s1".into(),
            entries: vec![QueueEntryWire {
                id: "p1".into(),
                version: 2,
                owner: Some("alice".into()),
                last_editor: Some("bob".into()),
                kind: "prompt".into(),
                text: "hi".into(),
                position: 0,
                combined_texts: None,
                origin: None,
            }],
            running_prompt_id: Some("p0".into()),

            running_text: None,
            running_kind: None,
            running_combined_texts: None,
        };
        let expected = serde_json::json!({
            "sessionId": "s1",
            "entries": [{
                "id": "p1",
                "version": 2,
                "owner": "alice",
                "lastEditor": "bob",
                "kind": "prompt",
                "text": "hi",
                "position": 0
            }],
            "runningPromptId": "p0"
        });
        assert_eq!(serde_json::to_value(&payload).unwrap(), expected);
    }

    /// A broadcast without sessionId must fail to parse, not apply under the wrong key.
    #[test]
    fn queue_changed_requires_session_id() {
        let missing = serde_json::json!({ "entries": [] });
        assert!(serde_json::from_value::<QueueChanged>(missing).is_err());
    }

    #[test]
    fn sparse_payload_deserializes_with_defaults() {
        let sparse = serde_json::json!({
            "sessionId": "s1",
            "entries": [{"id": "p1"}]
        });
        let parsed: QueueChanged = serde_json::from_value(sparse).unwrap();
        assert_eq!(parsed.entries[0].version, 0);
        assert_eq!(parsed.entries[0].kind, "");
        assert_eq!(parsed.entries[0].text, "");
        assert_eq!(parsed.entries[0].position, 0);
        assert!(parsed.entries[0].owner.is_none());
        assert!(parsed.running_prompt_id.is_none());
    }

    #[test]
    fn extra_unknown_fields_ignored() {
        let json = serde_json::json!({
            "sessionId": "s1",
            "entries": [],
            "runningPromptId": null,
            "futureField": "should be ignored"
        });
        let parsed: QueueChanged = serde_json::from_value(json).unwrap();
        assert_eq!(parsed.session_id, "s1");
    }

    #[test]
    fn queue_changed_derives_default() {
        let d = QueueChanged::default();
        assert_eq!(d.session_id, "");
        assert!(d.entries.is_empty());
        assert!(d.running_prompt_id.is_none());
    }

    #[test]
    fn queue_entry_origin_round_trips_and_absent_means_none() {
        // Additive wire: a typed origin round-trips.
        let entry = QueueEntryWire {
            id: "p1".into(),
            version: 0,
            owner: None,
            last_editor: None,
            kind: "prompt".into(),
            text: "hi".into(),
            position: 0,
            combined_texts: None,
            origin: Some(QueueOrigin::TaskCompleted),
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["origin"], "task_completed");
        let round: QueueEntryWire = serde_json::from_value(json).unwrap();
        assert_eq!(round.origin, Some(QueueOrigin::TaskCompleted));

        // Legacy wire without `origin` deserializes to `None` (fail-closed
        // unknown on the consumer side, never a real `User` turn).
        let legacy = serde_json::json!({
            "id": "p1", "version": 0, "kind": "prompt", "text": "hi", "position": 0
        });
        let parsed: QueueEntryWire = serde_json::from_value(legacy).unwrap();
        assert_eq!(parsed.origin, None);

        // Omitted when None (keeps the golden wire shape for legacy clients).
        let entry_none = QueueEntryWire {
            origin: None,
            ..entry
        };
        let json_none = serde_json::to_value(&entry_none).unwrap();
        assert!(json_none.get("origin").is_none());
    }

    #[test]
    fn queue_origin_json_unknown_default() {
        assert_eq!(QueueOrigin::default(), QueueOrigin::Unknown);
        let v: QueueOrigin = serde_json::from_value(serde_json::json!("unknown")).unwrap();
        assert_eq!(v, QueueOrigin::Unknown);
    }

    #[test]
    fn queue_origin_unknown_tag_absorbs_arbitrary_future_tag() {
        // An old binary meeting an unknown future tag must not error: it
        // degrades to the safe, never-priming `Unknown` via `#[serde(other)]`.
        for tag in [
            "subagent_assignment",
            "agent_prime_assignment",
            "future_tag",
            "bogus",
        ] {
            let v: QueueOrigin = serde_json::from_value(serde_json::json!(tag)).unwrap();
            assert_eq!(
                v,
                QueueOrigin::Unknown,
                "unknown tag {tag} must degrade to Unknown"
            );
            assert_ne!(v, QueueOrigin::User);
        }
        // Recognized tags still resolve (exact wire names preserved).
        assert_eq!(
            serde_json::from_value::<QueueOrigin>(serde_json::json!("scheduler_fired")).unwrap(),
            QueueOrigin::SchedulerFired
        );
        assert_eq!(
            serde_json::from_value::<QueueOrigin>(serde_json::json!("user")).unwrap(),
            QueueOrigin::User
        );
    }

    #[test]
    fn running_combined_texts_round_trip() {
        let original = QueueChanged {
            session_id: "s1".into(),
            entries: vec![],
            running_prompt_id: Some("p0".into()),
            running_text: Some("a\n\nb".into()),
            running_kind: Some("prompt".into()),
            running_combined_texts: Some(vec!["a".into(), "b".into()]),
        };
        let json = serde_json::to_value(&original).unwrap();
        assert_eq!(json["runningCombinedTexts"], serde_json::json!(["a", "b"]));
        let round: QueueChanged = serde_json::from_value(json).unwrap();
        assert_eq!(round, original);
    }
}
