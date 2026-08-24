pub mod acp_types;
pub mod announcement_state;
pub mod commands;
pub mod compaction_config;
pub mod handle;
pub mod memory_state;
pub mod merge;
pub mod notifications;
pub mod pending_interaction;
pub mod prime;
pub mod prompt_queue;
pub(crate) mod rolling_compaction;
pub mod two_pass;
pub use self::acp_session::*;
pub use self::acp_types::*;
pub use self::commands::*;
pub use self::fork::{ForkSessionRequest, ForkSessionResponse, fork_session};
pub use self::handle::*;
pub use self::persistence::{
    LocalFeedbackEntry, UserFeedbackEntry, find_local_child_for_remote, resolve_local_session,
    resolve_local_session_any_cwd, session_exists_for_cwd,
};
pub use self::result::{Empty, ExtMethodResult};
pub use self::share::{ShareSessionRequest, ShareSessionResponse};
pub use prod_mc_cli_chat_proxy_types::feedback_types::{
    ClientType, FeedbackTerminalInfo, RatingType,
};
pub use xai_fsnotify::{FsConfig, FsEvent, FsEventKind, FsEventSource, FsNotifyError, GitMetaKind};
/// `false` twin: this template is not compiled into this build, so no
/// template matches. Keeps ungated call sites compiling in both
/// configurations.
pub(crate) fn is_cursor_user_template(
    _template: &xai_grok_agent::prompt::user_message::UserMessageTemplate,
) -> bool {
    false
}
/// `false` twin of [`is_cursor_system_template`]; see [`is_cursor_user_template`].
pub(crate) fn is_cursor_system_template(
    _template: &xai_grok_agent::prompt::context::TemplateOverride,
) -> bool {
    false
}
/// Pull the `ContentBlock::Image`s out of a block list — the single spelling
/// of "only Image blocks ride structurally" (interject parse + queue-interject
/// harvest).
pub(crate) fn image_blocks(
    blocks: impl IntoIterator<Item = agent_client_protocol::ContentBlock>,
) -> Vec<agent_client_protocol::ImageContent> {
    blocks
        .into_iter()
        .filter_map(|block| match block {
            agent_client_protocol::ContentBlock::Image(img) => Some(img),
            _ => None,
        })
        .collect()
}
/// Describes who originated a prompt: the user, the shell's auto-wake
/// system reacting to a completed background task / subagent, or a
/// system/parent-originated assignment.
///
/// Every producer constructs an explicit variant (PR19): this enum is carried
/// as a **typed value** end-to-end from the producer through
/// `SessionCommand::Prompt` / `queue_input` / `InputItem` / `AgentTask` /
/// `handle_prompt`. The lifecycle NEVER infers origin from prompt-id strings
/// after construction; [`Self::from_prompt_id`] survives only as a fail-closed
/// decoder for legacy wire entries and returns [`Self::Unknown`] for
/// unrecognized ids (never [`Self::User`]).
///
/// Every variant must be deliberately decided in the exhaustive matches on
/// [`Self::prime_eligible`], [`Self::is_synthetic`], [`Self::is_user_typed`],
/// [`Self::hide_user_echo_from_scrollback`], [`Self::completion_id`], and the
/// `From<&PromptOrigin> for QueueOrigin` conversion — the compiler forces each
/// new variant through every lifecycle/security decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptOrigin {
    /// A normal user-initiated prompt. The only [`Self::prime_eligible`]
    /// origin.
    User,
    /// A prompt injected because the user interjected mid-turn (a stranded
    /// interjection promoted to its own turn). User-typed content, but a
    /// side-channel interjection — never a queued `User` turn, so it must not
    /// prime.
    Interjection,
    /// The first prompt of a child sub-agent session: the task text authored
    /// by the parent (ultimately derived from a user request). Never primes —
    /// only an explicit real `User` turn in the child primes, using the
    /// child's own workspace/inventory.
    SubagentAssignment,
    /// Auto-wake prompt injected when a background terminal task completed.
    TaskCompleted {
        /// The background task ID (without the `task-completed-` prefix).
        task_id: String,
    },
    /// Auto-wake prompt injected when a background subagent completed.
    SubagentCompleted {
        /// The subagent ID (without the `subagent-completed-` prefix).
        subagent_id: String,
    },
    WorkflowCompleted {
        completion_id: String,
    },
    /// Server-initiated prompt from the idle-gated notification drain
    /// (`maybe_drain_notifications`). Batches one or more monitor-event
    /// or bash-task-completed notifications into a single turn while the
    /// user is idle.
    NotificationDrain,
    /// Orchestrator-initiated summary turn. The goal orchestrator injects a
    /// system reminder into context and then triggers a model turn so the
    /// model can print a visible progress update.
    GoalSummary,
    /// Scheduled task (`/loop`) prompt fired by the pager scheduler and
    /// carried across the ACP boundary via the prompt-request origin meta tag
    /// ([`Self::from_prompt_origin_meta`]). Never primes.
    SchedulerFired,
    /// Turn injected after a resumed plan-approval decision: the
    /// shell re-parked `exit_plan_mode` on resume, the user approved/revised,
    /// and the shell injects the follow-up turn. Synthetic so the user never
    /// typed it — kept out of prompt history — but it still runs a real turn.
    PlanResume,
    /// Legacy / wire entries that carried no explicit origin. Fail-closed:
    /// rendered and dispatched like a prompt but NEVER primed and never
    /// treated as a real [`Self::User`] turn.
    Unknown,
}
/// Meta key on an ACP `PromptRequest` carrying an optional client-declared
/// prompt-origin tag. Additive: absent tags are user prompts except reserved
/// legacy synthetic prompt IDs, which fail closed at the ACP boundary.
pub const PROMPT_ORIGIN_META_KEY: &str = "promptOrigin";
pub const PROMPT_ORIGIN_SCHEDULER_FIRED: &str = "scheduler_fired";
impl PromptOrigin {
    /// Exhaustive prime gate. Only an explicit real nominal [`Self::User`]
    /// is prime-eligible; every synthetic, unknown, assignment, and
    /// external-runtime origin is excluded. Each variant has a deliberate
    /// arm, so adding a variant forces a decision here at compile time.
    pub fn prime_eligible(&self) -> bool {
        match self {
            Self::User => true,
            Self::Interjection
            | Self::SubagentAssignment
            | Self::TaskCompleted { .. }
            | Self::SubagentCompleted { .. }
            | Self::WorkflowCompleted { .. }
            | Self::NotificationDrain
            | Self::GoalSummary
            | Self::SchedulerFired
            | Self::PlanResume
            | Self::Unknown => false,
        }
    }
    /// Exhaustive "did a real user type this content" gate. True for a direct
    /// user prompt and a promoted user interjection; false for every
    /// synthetic/orchestrator/assignment/legacy origin. Deliberate arm per
    /// variant.
    pub fn is_user_typed(&self) -> bool {
        match self {
            Self::User | Self::Interjection => true,
            Self::SubagentAssignment
            | Self::TaskCompleted { .. }
            | Self::SubagentCompleted { .. }
            | Self::WorkflowCompleted { .. }
            | Self::NotificationDrain
            | Self::GoalSummary
            | Self::SchedulerFired
            | Self::PlanResume
            | Self::Unknown => false,
        }
    }
    /// Decode a legacy prompt-id string into an origin, FAIL-CLOSED.
    ///
    /// Used for legacy wire entries and at the ACP boundary when an older
    /// client omits the typed origin tag. Recognized synthetic prefixes map to
    /// their variant; **any unrecognized id returns [`Self::Unknown`], never
    /// [`Self::User`]** — a legacy id can never claim to be a real user turn.
    /// After that boundary, the live pipeline carries the typed [`Self`]
    /// without further prompt-id inference.
    pub fn from_prompt_id(prompt_id: &str) -> Self {
        if let Some(task_id) = prompt_id.strip_prefix("task-completed-") {
            Self::TaskCompleted {
                task_id: task_id.to_string(),
            }
        } else if let Some(subagent_id) = prompt_id.strip_prefix("subagent-completed-") {
            Self::SubagentCompleted {
                subagent_id: subagent_id.to_string(),
            }
        } else if let Some(completion_id) = prompt_id.strip_prefix("workflow-completed-") {
            Self::WorkflowCompleted {
                completion_id: completion_id.to_string(),
            }
        } else if prompt_id.starts_with("interject-fallback-") {
            Self::Interjection
        } else if prompt_id.starts_with("notifications-") {
            Self::NotificationDrain
        } else if prompt_id.starts_with("goal-summary-") {
            Self::GoalSummary
        } else if prompt_id.starts_with("scheduler-fired-") {
            Self::SchedulerFired
        } else if prompt_id.starts_with("plan-resume-") {
            Self::PlanResume
        } else {
            // Fail-closed: an unrecognized id is Unknown, never User.
            Self::Unknown
        }
    }
    /// Lossless reader for the ACP `PromptRequest` meta origin tag
    /// ([`Self::PROMPT_ORIGIN_META_KEY`]) — the one cross-boundary origin
    /// carrier that IS read back into lifecycle decisions.
    ///
    /// The ACP boundary classifies absent tags before this reader. A recognized scheduler tag
    /// (`scheduler_fired`, stamped by the pager for `/loop`) maps to
    /// [`Self::SchedulerFired`] and never primes. Any other present tag fails
    /// closed to [`Self::Unknown`]: a client can never claim completion
    /// identities that carry server-side payloads, and an unknown claim never
    /// primes.
    pub fn from_prompt_origin_meta(tag: Option<&str>) -> Self {
        match tag {
            None => Self::User,
            Some(PROMPT_ORIGIN_SCHEDULER_FIRED) => Self::SchedulerFired,
            Some(_) => Self::Unknown,
        }
    }
    /// Returns `true` for a direct client user prompt. Exhaustive: each
    /// variant has a deliberate arm.
    pub fn is_client_user_prompt(&self) -> bool {
        match self {
            Self::User => true,
            Self::Interjection
            | Self::SubagentAssignment
            | Self::TaskCompleted { .. }
            | Self::SubagentCompleted { .. }
            | Self::WorkflowCompleted { .. }
            | Self::NotificationDrain
            | Self::GoalSummary
            | Self::SchedulerFired
            | Self::PlanResume
            | Self::Unknown => false,
        }
    }
    /// Returns `true` for auto-wake (synthetic) prompts. Exhaustive: each
    /// variant has a deliberate arm.
    pub fn is_synthetic(&self) -> bool {
        match self {
            Self::User | Self::SubagentAssignment => false,
            Self::Interjection
            | Self::TaskCompleted { .. }
            | Self::SubagentCompleted { .. }
            | Self::WorkflowCompleted { .. }
            | Self::NotificationDrain
            | Self::GoalSummary
            | Self::SchedulerFired
            | Self::PlanResume
            | Self::Unknown => true,
        }
    }
    /// Whether a `UserMessageChunk` echo for this origin must stay out of
    /// client scrollback (live and on resume). Model-only / side-channel
    /// content — UI already surfaces it via task pane, monitor gutter, etc.
    ///
    /// Cron (`SchedulerFired`) and plan-resume follow-ups still render;
    /// real user turns always render.
    pub fn hide_user_echo_from_scrollback(&self) -> bool {
        match self {
            Self::User
            | Self::SubagentAssignment
            | Self::SchedulerFired
            | Self::PlanResume
            | Self::Unknown => false,
            Self::Interjection
            | Self::TaskCompleted { .. }
            | Self::SubagentCompleted { .. }
            | Self::WorkflowCompleted { .. }
            | Self::NotificationDrain
            | Self::GoalSummary => true,
        }
    }
    pub fn completion_id(&self) -> Option<&str> {
        match self {
            Self::TaskCompleted { task_id } => Some(task_id),
            Self::SubagentCompleted { subagent_id } => Some(subagent_id),
            Self::WorkflowCompleted { completion_id } => Some(completion_id),
            Self::User
            | Self::Interjection
            | Self::SubagentAssignment
            | Self::NotificationDrain
            | Self::GoalSummary
            | Self::SchedulerFired
            | Self::PlanResume
            | Self::Unknown => None,
        }
    }
}
impl From<&PromptOrigin> for xai_prompt_queue::QueueOrigin {
    fn from(origin: &PromptOrigin) -> Self {
        use xai_prompt_queue::QueueOrigin as Q;
        match origin {
            PromptOrigin::User => Q::User,
            PromptOrigin::Interjection => Q::Interjection,
            // A child sub-agent's first prompt is never on a client queue
            // roster; on the wire it maps to the safe, display-only `Unknown`
            // tag (never a newly-emitted enum value an old client can't
            // parse). Prime eligibility stays server-gated by the typed value.
            PromptOrigin::SubagentAssignment => Q::Unknown,
            PromptOrigin::TaskCompleted { .. } => Q::TaskCompleted,
            PromptOrigin::SubagentCompleted { .. } => Q::SubagentCompleted,
            PromptOrigin::WorkflowCompleted { .. } => Q::WorkflowCompleted,
            PromptOrigin::NotificationDrain => Q::NotificationDrain,
            PromptOrigin::GoalSummary => Q::GoalSummary,
            PromptOrigin::SchedulerFired => Q::SchedulerFired,
            PromptOrigin::PlanResume => Q::PlanResume,
            PromptOrigin::Unknown => Q::Unknown,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::{PROMPT_ORIGIN_SCHEDULER_FIRED, PromptOrigin};
    #[test]
    fn from_prompt_id_unrecognized_is_unknown_never_user() {
        // Legacy fail-closed: an unrecognized id is Unknown, never User.
        assert_eq!(
            PromptOrigin::from_prompt_id("my-prompt"),
            PromptOrigin::Unknown
        );
        assert!(PromptOrigin::from_prompt_id("my-prompt").is_synthetic());
        assert!(!PromptOrigin::from_prompt_id("my-prompt").prime_eligible());
        assert_eq!(
            PromptOrigin::from_prompt_id("anything-not-matching"),
            PromptOrigin::Unknown
        );
    }
    #[test]
    fn prime_eligible_only_for_explicit_user() {
        assert!(PromptOrigin::User.prime_eligible());
        assert!(!PromptOrigin::Unknown.prime_eligible());
        assert!(!PromptOrigin::Interjection.prime_eligible());
        assert!(!PromptOrigin::SubagentAssignment.prime_eligible());
        // Every non-User origin must be excluded.
        for o in [
            PromptOrigin::TaskCompleted {
                task_id: "t".into(),
            },
            PromptOrigin::SubagentCompleted {
                subagent_id: "s".into(),
            },
            PromptOrigin::WorkflowCompleted {
                completion_id: "w".into(),
            },
            PromptOrigin::NotificationDrain,
            PromptOrigin::GoalSummary,
            PromptOrigin::SchedulerFired,
            PromptOrigin::PlanResume,
        ] {
            assert!(!o.prime_eligible(), "synthetic origin must not prime");
        }
    }
    #[test]
    fn from_prompt_origin_meta_absent_is_user_and_scheduler_is_typed() {
        // A non-reserved untagged ACP prompt is a real user prompt and primes.
        assert_eq!(
            PromptOrigin::from_prompt_origin_meta(None),
            PromptOrigin::User
        );
        assert!(
            PromptOrigin::from_prompt_origin_meta(None).prime_eligible(),
            "a client prompt without an origin claim primes"
        );
        // The pager stamps scheduler_fired for `/loop` cron turns: typed,
        // synthetic, non-priming.
        let cron = PromptOrigin::from_prompt_origin_meta(Some(PROMPT_ORIGIN_SCHEDULER_FIRED));
        assert_eq!(cron, PromptOrigin::SchedulerFired);
        assert!(cron.is_synthetic());
        assert!(!cron.prime_eligible());
        assert!(!cron.is_user_typed());
        assert_eq!(cron.completion_id(), None);
    }
    #[test]
    fn from_prompt_origin_meta_unknown_claim_fails_closed() {
        // A client can never claim completion/synthetic identities; unknown
        // claims fail closed to Unknown and never prime.
        for tag in [
            "task_completed",
            "subagent_completed",
            "goal_summary",
            "interjection",
            "bogus",
        ] {
            let origin = PromptOrigin::from_prompt_origin_meta(Some(tag));
            assert_eq!(origin, PromptOrigin::Unknown, "tag {tag} must fail closed");
            assert!(!origin.prime_eligible());
            assert!(origin.is_synthetic());
        }
    }
    #[test]
    fn legacy_synthetic_prompt_ids_fail_closed_at_the_acp_boundary() {
        let origin = PromptOrigin::from_prompt_id("scheduler-fired-legacy");
        assert_eq!(origin, PromptOrigin::SchedulerFired);
        assert!(!origin.prime_eligible());
    }
    #[test]
    fn subagent_assignment_maps_to_unknown_on_wire_not_new_tag() {
        // QueueOrigin is display-only and old clients cannot parse a newly
        // emitted enum value. A child sub-agent's assignment is never on a
        // client queue roster, so it maps to the safe `Unknown` tag on the
        // wire (never a new `subagent_assignment` value an old binary errors
        // on). Prime eligibility stays server-gated by the typed value.
        let wire = xai_prompt_queue::QueueOrigin::from(&PromptOrigin::SubagentAssignment);
        assert_eq!(wire, xai_prompt_queue::QueueOrigin::Unknown);
        let json = serde_json::to_value(&wire).unwrap();
        assert_eq!(json, serde_json::json!("unknown"));
    }
    #[test]
    fn producer_matrix_classifications_are_exhaustive_and_non_circular() {
        // Non-circular producer matrix: the full variant set is enumerated
        // here with each production producer that constructs it, and each
        // variant's lifecycle/security classification is asserted directly on
        // the typed value (no round-trip through the conversion under test).
        // Producers (all set explicit typed origins):
        //   User                 — ACP client prompt dispatch (mvp_agent/acp_agent.rs)
        //   Interjection        — interjection fallback (acp_session_impl/interjection.rs)
        //   SubagentAssignment  — child subagent kick-off (agent/subagent/handle_request.rs)
        //   TaskCompleted       — task-completed wake (tools/notification_bridge.rs)
        //   SubagentCompleted   — subagent completion wake (agent/subagent/mod.rs)
        //   WorkflowCompleted   — workflow completion wake (acp_session_impl/run_loop.rs)
        //   NotificationDrain   — idle notification drain (notification_drain.rs)
        //   GoalSummary         — goal summary/continuation (goal.rs, run_loop.rs)
        //   SchedulerFired      — pager `/loop` via ACP prompt-origin meta tag
        //   PlanResume          — plan-resume follow-up (tool_calls.rs)
        //   Unknown             — fail-closed legacy/wire/unknown meta (no producer)
        let cases: &[(
            PromptOrigin,
            bool, /*synthetic*/
            bool, /*user_typed*/
            bool, /*prime*/
        )] = &[
            (PromptOrigin::User, false, true, true),
            (PromptOrigin::Interjection, true, true, false),
            (PromptOrigin::SubagentAssignment, false, false, false),
            (
                PromptOrigin::TaskCompleted {
                    task_id: "t".into(),
                },
                true,
                false,
                false,
            ),
            (
                PromptOrigin::SubagentCompleted {
                    subagent_id: "s".into(),
                },
                true,
                false,
                false,
            ),
            (
                PromptOrigin::WorkflowCompleted {
                    completion_id: "w".into(),
                },
                true,
                false,
                false,
            ),
            (PromptOrigin::NotificationDrain, true, false, false),
            (PromptOrigin::GoalSummary, true, false, false),
            (PromptOrigin::SchedulerFired, true, false, false),
            (PromptOrigin::PlanResume, true, false, false),
            (PromptOrigin::Unknown, true, false, false),
        ];
        // Exhaustive: exactly one case per variant — a new variant must be
        // added here (and decided in every classification fn) at compile time
        // because the classification fns are exhaustive matches.
        let mut seen: std::collections::BTreeSet<&'static str> = std::collections::BTreeSet::new();
        for (origin, synthetic, user_typed, prime) in cases {
            assert_eq!(
                origin.is_synthetic(),
                *synthetic,
                "is_synthetic mismatch for {origin:?}"
            );
            assert_eq!(
                origin.is_user_typed(),
                *user_typed,
                "is_user_typed mismatch"
            );
            assert_eq!(
                origin.prime_eligible(),
                *prime,
                "prime_eligible must be true only for User"
            );
            seen.insert(origin_label(origin));
        }
        // Ensure every variant is covered exactly once.
        assert_eq!(
            seen.len(),
            cases.len(),
            "producer matrix must cover every variant"
        );
    }
    fn origin_label(o: &PromptOrigin) -> &'static str {
        match o {
            PromptOrigin::User => "User",
            PromptOrigin::Interjection => "Interjection",
            PromptOrigin::SubagentAssignment => "SubagentAssignment",
            PromptOrigin::TaskCompleted { .. } => "TaskCompleted",
            PromptOrigin::SubagentCompleted { .. } => "SubagentCompleted",
            PromptOrigin::WorkflowCompleted { .. } => "WorkflowCompleted",
            PromptOrigin::NotificationDrain => "NotificationDrain",
            PromptOrigin::GoalSummary => "GoalSummary",
            PromptOrigin::SchedulerFired => "SchedulerFired",
            PromptOrigin::PlanResume => "PlanResume",
            PromptOrigin::Unknown => "Unknown",
        }
    }
    #[test]
    fn from_prompt_id_task_completed() {
        let origin = PromptOrigin::from_prompt_id("task-completed-abc-123");
        assert_eq!(
            origin,
            PromptOrigin::TaskCompleted {
                task_id: "abc-123".into()
            }
        );
        assert!(origin.is_synthetic());
        assert_eq!(origin.completion_id(), Some("abc-123"));
    }
    #[test]
    fn from_prompt_id_subagent_completed() {
        let origin = PromptOrigin::from_prompt_id("subagent-completed-xyz-789");
        assert_eq!(
            origin,
            PromptOrigin::SubagentCompleted {
                subagent_id: "xyz-789".into()
            }
        );
        assert!(origin.is_synthetic());
        assert_eq!(origin.completion_id(), Some("xyz-789"));
    }
    #[test]
    fn from_prompt_id_interjection() {
        let origin = PromptOrigin::from_prompt_id("interject-fallback-abc");
        assert_eq!(origin, PromptOrigin::Interjection);
        assert!(origin.is_synthetic());
        assert!(!origin.prime_eligible());
    }
    #[test]
    fn from_prompt_id_notification_drain() {
        let origin =
            PromptOrigin::from_prompt_id("notifications-019e0000-0000-7000-8000-0000000000aa");
        assert_eq!(origin, PromptOrigin::NotificationDrain);
        assert!(origin.is_synthetic());
        assert_eq!(origin.completion_id(), None);
    }
    #[test]
    fn goal_summary_origin_from_prompt_id() {
        let origin = PromptOrigin::from_prompt_id("goal-summary-019e2d3e");
        assert!(matches!(origin, PromptOrigin::GoalSummary));
        assert!(origin.is_synthetic());
        assert_eq!(origin.completion_id(), None);
    }
    #[test]
    fn scheduler_fired_origin_from_prompt_id() {
        // Legacy decode still recognizes the scheduler prefix for old stored
        // ids, but the live path carries the typed ACP meta tag instead.
        let origin = PromptOrigin::from_prompt_id("scheduler-fired-019e51a3-abcd-1234");
        assert!(matches!(origin, PromptOrigin::SchedulerFired));
        assert!(origin.is_synthetic());
        assert_eq!(origin.completion_id(), None);
    }
    #[test]
    fn plan_resume_origin_from_prompt_id() {
        let origin = PromptOrigin::from_prompt_id("plan-resume-1730000000000");
        assert!(matches!(origin, PromptOrigin::PlanResume));
        assert!(origin.is_synthetic());
        assert_eq!(origin.completion_id(), None);
    }
    #[test]
    fn notification_drain_is_server_initiated() {
        let prompt_id = "notifications-019e0000-0000-7000-8000-0000000000aa";
        assert!(PromptOrigin::from_prompt_id(prompt_id).is_synthetic());
    }
    #[test]
    fn hide_user_echo_from_scrollback_by_origin() {
        assert!(!PromptOrigin::User.hide_user_echo_from_scrollback());
        assert!(
            !PromptOrigin::from_prompt_id("scheduler-fired-abc").hide_user_echo_from_scrollback()
        );
        assert!(!PromptOrigin::from_prompt_id("plan-resume-1").hide_user_echo_from_scrollback());
        assert!(PromptOrigin::from_prompt_id("task-completed-t1").hide_user_echo_from_scrollback());
        assert!(
            PromptOrigin::from_prompt_id("subagent-completed-s1").hide_user_echo_from_scrollback()
        );
        assert!(
            PromptOrigin::from_prompt_id("notifications-uuid").hide_user_echo_from_scrollback()
        );
        assert!(
            PromptOrigin::from_prompt_id("workflow-completed-wf-1-9")
                .hide_user_echo_from_scrollback()
        );
        assert!(PromptOrigin::from_prompt_id("goal-summary-1").hide_user_echo_from_scrollback());
        assert!(PromptOrigin::Interjection.hide_user_echo_from_scrollback());
        assert!(!PromptOrigin::SubagentAssignment.hide_user_echo_from_scrollback());
        assert!(!PromptOrigin::Unknown.hide_user_echo_from_scrollback());
    }
}
/// Client-requested fs notification mode (was xai_fsnotify::FsNotifyMode).
/// Determines whether the session sends an initial file index to the client
/// or just streams raw file events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum ClientFsMode {
    #[default]
    Events,
    Index,
}
/// Client-side fs notification config: fs source settings + mode.
#[derive(Debug, Clone, Default)]
pub struct ClientFsConfig {
    pub fs: FsConfig,
    pub mode: ClientFsMode,
}
/// Share session request/response types
pub mod share {
    /// Request to share a session via URL
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct ShareSessionRequest {
        pub session_id: String,
    }
    /// Response containing the shareable URL
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct ShareSessionResponse {
        pub share_url: String,
    }
}
/// Proxy config for the session registry client.
/// Shared between `acp_session` (slash commands) and `persistence` (title generation).
#[derive(Clone)]
pub(crate) struct RegistryConfig {
    pub base_url: String,
    pub user_token: String,
    pub deployment_key: Option<String>,
    pub alpha_test_key: Option<String>,
}
pub mod acp_conversion;
pub mod acp_mcp;
pub(crate) mod acp_session;
pub(crate) mod agent_rebuild;
/// Exact auxiliary inference route resolution (compaction/media/web/title/…).
pub(crate) mod auxiliary_route;
pub mod chat_persistence;
pub(crate) mod events;
pub mod export;
pub mod feedback;
pub mod feedback_manager;
pub mod file_system;
pub mod fork;
pub(crate) mod fs_watch;
pub(crate) mod goal_classifier;
pub(crate) mod goal_evaluator;
pub(crate) mod goal_next_step;
pub(crate) mod goal_orchestrator;
pub(crate) mod goal_planner;
pub(crate) mod goal_role_tools;
pub(crate) mod goal_stop_detector;
pub(crate) mod goal_strategist;
pub(crate) mod goal_summarizer;
pub mod goal_tracker;
pub mod helpers;
pub(crate) mod image_describe;
pub(crate) mod image_normalize;
pub mod inference_metrics;
pub(crate) mod media_descriptors;
pub(crate) mod media_pipeline;
pub(crate) mod media_stt;
#[cfg(test)]
mod pr6_auxiliary_boundary_tests;
/// Production sampler route context resolution (credential-free).
pub mod route_context;
pub use xai_grok_shared::session::info;
pub mod managed_mcp;
pub(crate) mod mcp_descriptors;
pub mod mcp_dispatcher;
#[cfg(test)]
mod mcp_dispatcher_e2e_tests;
pub mod mcp_restart;
pub mod mcp_servers;
pub mod memory;
pub(crate) mod normalize_cache;
pub mod persistence;
pub use xai_grok_shared::placeholder_images;
pub mod plan_mode;
pub mod prompt_history;
pub mod prompt_parser;
pub(crate) mod prompt_timing;
pub(crate) mod replay_events;
pub mod repo_changes;
#[path = "restore_stub.rs"]
pub mod restore;
pub mod result;
pub mod signals;
pub(crate) mod slash_commands;
pub mod storage;
pub(crate) mod streaming_capture;
pub(crate) mod streaming_tool_calls;
pub(crate) mod summary;
pub(crate) mod telemetry;
pub mod tool_index;
pub(crate) mod turn_completion;
pub mod unified_list;
pub(crate) mod user_message;
pub(crate) mod wire_tags;
pub(crate) mod workflow;
pub mod worktree;
pub mod worktree_pool;
