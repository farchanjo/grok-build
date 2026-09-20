//! Reminder policy — wraps xai-grok-tools reminder config.

/// Default per-prompt fire cap for the runtime turn-end TodoGate. Used
/// only as the default for `TodoGateConfig`; the runtime consumer reads
/// the live value from `ReminderPolicy.todo_gate.max_fires_per_prompt`,
/// so this constant is NOT a hardcoded cap.
pub const DEFAULT_TODO_GATE_MAX_FIRES: u32 = 2;

/// Default number of todo items the gate reminder names before collapsing the
/// rest into a `… and N more` line. The reminder's actionable content is one
/// line (the item to advance next); the rest is context the model already has
/// in its own todo list. Measured against a 20-item list: 1,391 chars
/// uncapped, 548 chars capped at 3, with no per-item loss below 3.
pub const DEFAULT_TODO_GATE_MAX_ITEMS_NAMED: u32 = 3;

/// Session-level system reminder policy.
///
/// Controls whether system reminders are enabled and configures
/// the TodoNudge and TodoGate behavior.
#[derive(Debug, Clone)]
pub struct ReminderPolicy {
    /// Whether system reminders are enabled at all.
    pub enabled: bool,
    /// Configuration for the periodic TodoWrite nudge reminder.
    pub todo_nudge: TodoNudgeConfig,
    /// Configuration for the runtime turn-end TodoGate.
    pub todo_gate: TodoGateConfig,
}

impl Default for ReminderPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            todo_nudge: TodoNudgeConfig::default(),
            todo_gate: TodoGateConfig::default(),
        }
    }
}

/// Configuration for the TodoWrite nudge reminder.
///
/// The system will remind the model to use `todo_write` when it
/// hasn't done so within a configurable number of turns.
#[derive(Debug, Clone)]
pub struct TodoNudgeConfig {
    /// Whether the TodoNudge reminder is enabled.
    pub enabled: bool,
    /// Number of turns since last `todo_write` call before nudging.
    pub turns_since_todo_write: u32,
    /// Minimum turns between nudge reminders.
    pub turns_between_reminders: u32,
}

impl Default for TodoNudgeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            turns_since_todo_write: 3,
            turns_between_reminders: 5,
        }
    }
}

/// Configuration for the runtime turn-end TodoGate.
///
/// The gate inspects `TodoState` after every content-only assistant
/// message and forces another turn via `<system-reminder>` injection
/// if pending/unbacked-in-progress todos remain — see
/// `xai-grok-shell::session::acp_session::evaluate_todo_gate`.
///
/// **Enabled by default.** It is a finished, correct backstop that was only
/// held back for UI reasons (the settings-modal rows for `[reminder.todo_gate]`
/// were deferred); the remote `todo_gate_enabled` key and the `--todo-gate`
/// CLI flag (session-scoped force-enable, highest precedence) can still
/// override it either way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TodoGateConfig {
    /// Whether the gate runs at all.
    pub enabled: bool,
    /// Hard cap on how many times the gate may fire per user prompt
    /// before the next turn is allowed to end with `TurnOutcome::Completed`.
    /// Bounds the worst-case extra inference cost.
    pub max_fires_per_prompt: u32,
    /// Most items the reminder names before collapsing the rest into a
    /// `… and N more` line.
    pub max_items_named: u32,
    /// Ask one decision which pending item to advance when the pending list is
    /// longer than [`PICK_THRESHOLD`], instead of always taking insertion
    /// order. Insertion order is uninformed rather than wrong: on 18 fixtures
    /// built with a blocked item first it picks an actionable item 61% of the
    /// time, while one decision call picks it 83% (+4 gains, 0 losses), and the
    /// call only fires on a long list. Unavailable ⇒ insertion order, never
    /// "name nothing".
    pub pick_with_decision: bool,
}

impl TodoGateConfig {
    /// Pending-list length above which the pick is asked. Below it the dump is
    /// already cheap and insertion order is as good as anything.
    pub const PICK_THRESHOLD: usize = 3;
}

impl Default for TodoGateConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_fires_per_prompt: DEFAULT_TODO_GATE_MAX_FIRES,
            max_items_named: DEFAULT_TODO_GATE_MAX_ITEMS_NAMED,
            pick_with_decision: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_todo_gate_is_enabled_with_const_caps() {
        let cfg = TodoGateConfig::default();
        assert!(cfg.enabled, "TodoGate ships enabled");
        assert_eq!(cfg.max_fires_per_prompt, DEFAULT_TODO_GATE_MAX_FIRES);
        assert_eq!(DEFAULT_TODO_GATE_MAX_FIRES, 2);
        assert_eq!(cfg.max_items_named, DEFAULT_TODO_GATE_MAX_ITEMS_NAMED);
        assert_eq!(DEFAULT_TODO_GATE_MAX_ITEMS_NAMED, 3);
        assert!(cfg.pick_with_decision);
        assert_eq!(TodoGateConfig::PICK_THRESHOLD, 3);
    }

    #[test]
    fn reminder_policy_default_disables_gate_but_keeps_nudge_and_global_enabled() {
        let policy = ReminderPolicy::default();
        assert!(
            policy.enabled,
            "global system reminders stay enabled by default"
        );
        assert!(
            policy.todo_gate.enabled,
            "TodoGate ships enabled; remote/CLI can opt out"
        );
        assert_eq!(policy.todo_gate.max_fires_per_prompt, 2);
        // The two reminder mechanisms are independent — flipping one
        // must not change the other (regression guard).
        assert!(policy.todo_nudge.enabled);
    }

    #[test]
    fn todo_gate_enable_does_not_disturb_nudge() {
        // A remote/CLI flip of the gate must not touch the periodic
        // TodoNudge as a side-effect.
        let mut policy = ReminderPolicy::default();
        policy.todo_gate.enabled = true;
        assert!(policy.todo_gate.enabled);
        assert!(policy.todo_nudge.enabled, "TodoNudge must stay enabled");
        assert!(policy.enabled, "global enable must stay true");
    }
}
