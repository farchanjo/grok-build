//! Phase-4 control rows: one typed knob per recommendation, each defaulting to
//! today's behaviour.
//!
//! Every row here exists because a measured recommendation had no surface. The
//! rows share one table ([`CONTROLS`]) so the settings modal, the slash
//! commands and the consumers cannot drift: a knob is declared once, and both
//! the UI and the read path resolve from it.
//!
//! ## Why a store and not a typed config field per knob
//!
//! The knobs are small scalars spread over several tables, and most of their
//! consumers already read the raw table (or will read it through
//! [`bool_at`]/[`int_at`]/[`str_at`]). Giving each one a typed struct plus a
//! `merge_section` arm would triple the surface for no schema gain. The store
//! is parsed once per config load into a flat map, so a read is a lock plus a
//! hash lookup rather than a disk hit — which matters for the permission
//! floors, evaluated on every bash decision.
//!
//! Defaults are the values in force today, so applying the whole table changes
//! nothing until someone flips a row.

use std::collections::HashMap;
use std::sync::OnceLock;

use parking_lot::RwLock;

/// Type and bounds of one control row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ControlKind {
    Bool,
    /// Inclusive bounds; the modal's stepper derives its step from the span.
    Int {
        min: i64,
        max: i64,
    },
    /// A `0.0..=1.0` fraction in the file, shown and edited as a percentage.
    ///
    /// `[memory.gate]` already stores thresholds as fractions, so the row keeps
    /// the file format and the user still reads `20`, not `0.2`.
    Percent {
        min: i64,
        max: i64,
    },
    /// Fixed choice set; the first entry is the default.
    Choice(&'static [&'static str]),
}

/// One control row: its config path, its type, and its presentation.
#[derive(Debug, Clone, Copy)]
pub struct ControlSpec {
    /// Dotted config path, which is also the settings-registry key.
    pub path: &'static str,
    pub kind: ControlKind,
    pub default_bool: bool,
    pub default_int: i64,
    pub label: &'static str,
    pub description: &'static str,
    pub keywords: &'static [&'static str],
}

impl ControlSpec {
    /// Default rendered as the string a choice row or the modal expects.
    pub fn default_str(&self) -> String {
        match self.kind {
            ControlKind::Bool => self.default_bool.to_string(),
            ControlKind::Int { .. } | ControlKind::Percent { .. } => self.default_int.to_string(),
            ControlKind::Choice(choices) => choices.first().copied().unwrap_or("").to_owned(),
        }
    }
}

const fn bool_row(
    path: &'static str,
    default_bool: bool,
    label: &'static str,
    description: &'static str,
    keywords: &'static [&'static str],
) -> ControlSpec {
    ControlSpec {
        path,
        kind: ControlKind::Bool,
        default_bool,
        default_int: 0,
        label,
        description,
        keywords,
    }
}

const fn percent_row(
    path: &'static str,
    default_percent: i64,
    min: i64,
    max: i64,
    label: &'static str,
    description: &'static str,
    keywords: &'static [&'static str],
) -> ControlSpec {
    ControlSpec {
        path,
        kind: ControlKind::Percent { min, max },
        default_bool: false,
        default_int: default_percent,
        label,
        description,
        keywords,
    }
}

const fn int_row(
    path: &'static str,
    default_int: i64,
    min: i64,
    max: i64,
    label: &'static str,
    description: &'static str,
    keywords: &'static [&'static str],
) -> ControlSpec {
    ControlSpec {
        path,
        kind: ControlKind::Int { min, max },
        default_bool: false,
        default_int,
        label,
        description,
        keywords,
    }
}

const fn choice_row(
    path: &'static str,
    choices: &'static [&'static str],
    label: &'static str,
    description: &'static str,
    keywords: &'static [&'static str],
) -> ControlSpec {
    ControlSpec {
        path,
        kind: ControlKind::Choice(choices),
        default_bool: false,
        default_int: 0,
        label,
        description,
        keywords,
    }
}

/// Every Phase-4 control row, in registry display order.
///
/// The order is grouped by target, matching the plan's numbering, so a reader
/// can walk the modal and the plan together.
pub const CONTROLS: &[ControlSpec] = &[
    // ── Permission floors (`/permission-classifier`) ──────────────────────
    //
    // Each floor overrides a classifier `Allow`, so its cost is invisible in
    // the classifier's own telemetry. Measured on jev-1.13: 19 of 127 bash
    // commands in a working session were floor tax and 16 of those were the
    // exec floor. All four ship armed, which is today's behaviour.
    bool_row(
        "permission.floors.write",
        true,
        "Floor: real file write",
        "Prompt when a bash command writes a real file, even if the classifier allowed it. 1 of the 19 measured floor prompts.",
        &["permission", "floor", "write", "bash", "prompt"],
    ),
    bool_row(
        "permission.floors.unsafe_env",
        true,
        "Floor: unsafe environment",
        "Prompt when a bash command runs with an environment the classifier cannot vet.",
        &["permission", "floor", "env", "bash", "prompt"],
    ),
    bool_row(
        "permission.floors.opaque_shell",
        true,
        "Floor: opaque shell",
        "Prompt when a bash command hides work behind an opaque shell.",
        &["permission", "floor", "shell", "bash", "prompt"],
    ),
    bool_row(
        "permission.floors.exec",
        true,
        "Floor: unvetted program",
        "Prompt when a bash command may run an unvetted program. The dominant term: 16 of the 19 measured floor prompts, so this is the one to revisit first.",
        &["permission", "floor", "exec", "bash", "prompt"],
    ),
    bool_row(
        "permission.classifier.enabled",
        true,
        "Permission classifier",
        "Let the auto-mode classifier decide an ungranted access. Off keeps the floors and the allowlist fast paths and sends everything else to a prompt.",
        &["permission", "classifier", "auto", "mode", "decision"],
    ),
    // ── Laziness detector (`/laziness`) ──────────────────────────────────
    //
    // Today's behaviour is per-model and off by default. These rows are the
    // global default, applied under any per-model `[models.<id>.laziness_detector]`
    // entry, which keeps winning where it is set.
    bool_row(
        "laziness.enabled",
        false,
        "Laziness detector",
        "Global default for the Layer-3 stall detector. Off today; the per-model `laziness_detector` block still overrides this.",
        &["laziness", "detector", "stall", "classifier"],
    ),
    int_row(
        "laziness.min_confidence",
        50,
        0,
        100,
        "Laziness confidence gate",
        "Minimum classifier confidence, in percent, before a nudge is injected. Measured operating point: 50 — at 70 the detector fires on 3 of 16 stalls.",
        &["laziness", "confidence", "threshold", "gate"],
    ),
    int_row(
        "laziness.idle_threshold_ms",
        10_000,
        1_000,
        600_000,
        "Laziness idle window",
        "How long the session must be idle before the detector runs, in milliseconds.",
        &["laziness", "idle", "window", "delay"],
    ),
    int_row(
        "laziness.max_nudges_per_session",
        0,
        0,
        100,
        "Laziness nudge cap",
        "Hard cap on nudges per session. 0 with the detector on is observation-only: the classifier fires and nothing is injected.",
        &["laziness", "nudge", "cap", "budget"],
    ),
    // ── Prime (`/prime`) ─────────────────────────────────────────────────
    bool_row(
        "prime.enabled",
        true,
        "Prime injection",
        "Inject the prime skill/agent selection into the turn. Off keeps the selection but drops the injection.",
        &["prime", "skills", "agents", "injection"],
    ),
    int_row(
        "prime.index_width",
        200,
        40,
        4_000,
        "Prime index width",
        "Characters of each skill body carried in the index. Accuracy saturates at 200; 400 pays 8,600 tokens per turn for nothing.",
        &["prime", "index", "width", "bytes", "tokens"],
    ),
    bool_row(
        "prime.strip_scaffolding",
        true,
        "Prime scaffolding strip",
        "Drop the frontmatter and scaffolding prose from an injected skill body, keeping the part the model acts on.",
        &["prime", "scaffolding", "strip", "frontmatter", "skill"],
    ),
    // ── Memory gate (`/memory-gate`) ─────────────────────────────────────
    bool_row(
        "memory.enabled",
        true,
        "Memory",
        "Master switch for the memory subsystem. Off makes every memory read and write a no-op.",
        &["memory", "master", "switch", "enabled"],
    ),
    bool_row(
        "memory.session.save_on_end",
        true,
        "Save memory on session end",
        "Write the session's memory notes when the session ends.",
        &["memory", "session", "save", "end"],
    ),
    bool_row(
        "memory.gate.enabled",
        false,
        "Memory admission gate",
        "Let a decision call judge each candidate note before it is stored. Off today.",
        &["memory", "gate", "admission", "dedup"],
    ),
    percent_row(
        "memory.gate.worth_threshold",
        20,
        0,
        100,
        "Memory worth threshold",
        "Minimum `worth`, in percent, for a note to be kept. Measured at 20: below it the gate loses valuable notes, above it noise starts arriving.",
        &["memory", "gate", "worth", "threshold"],
    ),
    percent_row(
        "memory.gate.covered_threshold",
        50,
        0,
        100,
        "Memory restatement threshold",
        "Minimum `covered`, in percent, for a note to be dropped as a restatement of what the store already holds. Measured at 50.",
        &["memory", "gate", "covered", "restatement", "dedup"],
    ),
    bool_row(
        "memory.gate.scope_routing",
        true,
        "Memory scope routing",
        "Let the gate choose the store scope per note instead of writing everything to global. Local queries served by global fell 6/7 to 2/7 with routing on.",
        &["memory", "gate", "scope", "routing", "global"],
    ),
    // ── Agents (`/agents-recommend`) ─────────────────────────────────────
    bool_row(
        "agents.recommend.enabled",
        true,
        "Agent recommendation",
        "Rank the delegate set for the turn instead of listing it.",
        &["agents", "recommend", "delegates", "ranking"],
    ),
    bool_row(
        "agents.recommend.graph",
        false,
        "Delegates graph",
        "Include the delegate graph edges in the recommendation. Off today; it adds on top of retrieval by 0-1 cases.",
        &["agents", "delegates", "graph", "edges"],
    ),
    choice_row(
        "agents.recommend.default_effort",
        &["low", "medium", "high"],
        "Default agent effort",
        "Effort used when an agent does not pin its own. Medium: a constant high scored 64% against 56% for asking, and removing the question removes a decision.",
        &["agents", "effort", "reasoning", "default"],
    ),
    // ── Goal verification (`/goal-verify`) ───────────────────────────────
    bool_row(
        "goal.verify.enabled",
        true,
        "Goal verification",
        "Run the adversarial skeptic panel before a goal is declared achieved.",
        &["goal", "verify", "skeptic", "panel"],
    ),
    int_row(
        "goal.verifier_count",
        3,
        1,
        5,
        "Skeptic panel size",
        "Adversarial skeptics per verification attempt, clamped to 1-5 by the resolver. Already a bounded constant; this row is the missing surface.",
        &["goal", "skeptic", "panel", "size", "count"],
    ),
    bool_row(
        "goal.verify.pre_filter",
        false,
        "Goal pre-filter",
        "Pre-filter the goal before the panel sees it. Off today: 67% accuracy is not enough without the agreement rate.",
        &["goal", "prefilter", "filter", "verify"],
    ),
    // ── Todo gate (`/todo-gate`) ─────────────────────────────────────────
    bool_row(
        "todo_gate.enabled",
        true,
        "Todo gate",
        "Block a turn end while pending todo items remain. On by default; the CLI flag used to be the only switch.",
        &["todo", "gate", "reminder", "backstop"],
    ),
    int_row(
        "todo_gate.max_fires_per_prompt",
        2,
        1,
        10,
        "Todo gate fire cap",
        "Hard cap on how many times the gate may fire per user prompt. Bounds the worst-case extra inference cost.",
        &["todo", "gate", "fire", "cap", "budget"],
    ),
    int_row(
        "todo_gate.max_items_named",
        3,
        1,
        20,
        "Todo gate item cap",
        "Most items the reminder names before collapsing the rest into an `... and N more` line. 3 keeps the dump at 548 chars where 20 items cost 1,391.",
        &["todo", "gate", "items", "cap", "dump"],
    ),
    bool_row(
        "todo_gate.pick_with_decision",
        true,
        "Todo gate decision pick",
        "Ask one decision which pending item to advance instead of always taking insertion order. Insertion order is 61% accurate, the decision call 83%, and it only fires on a list longer than three.",
        &["todo", "gate", "pick", "decision", "order"],
    ),
    // ── Tool search (`/tool-search`) ─────────────────────────────────────
    choice_row(
        "search.tool_backend",
        &["fused", "bm25", "dense"],
        "Tool search backend",
        "How tools are selected for the turn. Fused keeps BM25 and the exact-name short-circuit and adds the dense index, which took cross-lingual pool recall from 7/35 to 33/35.",
        &[
            "tool",
            "search",
            "backend",
            "bm25",
            "dense",
            "fused",
            "retrieval",
        ],
    ),
    // ── Workflows (`/workflows`) ─────────────────────────────────────────
    bool_row(
        "workflows.enabled",
        true,
        "Workflows",
        "Offer the workflow catalog to the model. Off keeps the runs and hides the entry point.",
        &["workflow", "workflows", "catalog", "enabled"],
    ),
];

/// Spec for one path, if it is a control row.
pub fn spec_for(path: &str) -> Option<&'static ControlSpec> {
    CONTROLS.iter().find(|spec| spec.path == path)
}

/// Whether `path` names a control row.
pub fn is_control(path: &str) -> bool {
    spec_for(path).is_some()
}

/// Live values, keyed by dotted path. Missing means "default".
static VALUES: OnceLock<RwLock<HashMap<String, String>>> = OnceLock::new();

fn values() -> &'static RwLock<HashMap<String, String>> {
    VALUES.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Install the values parsed from a config root.
///
/// Called at shell startup and after every settings write, so a flipped row is
/// live on the next read without a restart. Absent keys stay absent, which is
/// what keeps the default authoritative.
pub fn install(root: &toml::Value) {
    let mut parsed = HashMap::new();
    for spec in CONTROLS {
        let Some(value) = lookup(root, spec.path) else {
            continue;
        };
        let rendered = match (spec.kind, value) {
            (ControlKind::Bool, toml::Value::Boolean(b)) => b.to_string(),
            (ControlKind::Int { .. }, toml::Value::Integer(i)) => i.to_string(),
            (ControlKind::Percent { .. }, toml::Value::Float(f)) => (f * 100.0).round().to_string(),
            // A hand-written integer is read as a percentage: `20` is what the
            // row shows, and `0` / `1` are the only ambiguous cases.
            (ControlKind::Percent { .. }, toml::Value::Integer(i)) => i.to_string(),
            (ControlKind::Choice(_), toml::Value::String(s)) => s.clone(),
            _ => continue,
        };
        parsed.insert(spec.path.to_owned(), rendered);
    }
    *values().write() = parsed;
    push_permission_floors();
}

/// Push the four floor rows into the workspace crate.
///
/// The floors are evaluated in `xai-grok-workspace`, which cannot see this
/// store, so they are mirrored onto its atomic mask on every install — which
/// is what makes a flipped row live on the next bash decision rather than on
/// the next restart.
fn push_permission_floors() {
    use xai_grok_workspace::permission::BashFloorKind;
    for (path, kind) in [
        ("permission.floors.write", BashFloorKind::Write),
        ("permission.floors.unsafe_env", BashFloorKind::UnsafeEnv),
        ("permission.floors.opaque_shell", BashFloorKind::OpaqueShell),
        ("permission.floors.exec", BashFloorKind::Exec),
    ] {
        xai_grok_workspace::permission::set_floor_enabled(kind, bool_at(path, true));
    }
}

/// Re-read `config.toml` from disk and install. Best-effort: a read failure
/// leaves the previous values in place rather than resetting to defaults.
pub fn install_from_disk() {
    if let Ok(root) = crate::config::load_effective_config() {
        install(&root);
    }
}

fn lookup<'a>(root: &'a toml::Value, path: &str) -> Option<&'a toml::Value> {
    let mut node = root;
    for segment in path.split('.') {
        node = node.get(segment)?;
    }
    Some(node)
}

/// Boolean row, falling back to its declared default.
pub fn bool_at(path: &str, default: bool) -> bool {
    let raw = values().read().get(path).cloned();
    match raw {
        Some(raw) => raw.parse().unwrap_or(default),
        None => spec_for(path)
            .map(|spec| spec.default_bool)
            .unwrap_or(default),
    }
}

/// Integer row, clamped to its declared bounds. Also serves [`ControlKind::Percent`],
/// which is an integer percentage at this layer.
pub fn int_at(path: &str, default: i64) -> i64 {
    let Some(spec) = spec_for(path) else {
        return default;
    };
    let (ControlKind::Int { min, max } | ControlKind::Percent { min, max }) = spec.kind else {
        return default;
    };
    let raw = values().read().get(path).cloned();
    let parsed = raw
        .and_then(|raw| raw.parse::<i64>().ok())
        .unwrap_or(spec.default_int);
    parsed.clamp(min, max)
}

/// Choice row, falling back to its first declared choice.
pub fn str_at(path: &str, default: &str) -> String {
    let Some(spec) = spec_for(path) else {
        return default.to_owned();
    };
    let ControlKind::Choice(choices) = spec.kind else {
        return default.to_owned();
    };
    let raw = values().read().get(path).cloned();
    match raw {
        Some(raw) if choices.contains(&raw.as_str()) => raw,
        _ => choices.first().copied().unwrap_or(default).to_owned(),
    }
}

/// Current value of any row, as a rendered string. Used by `status` output so
/// the surface and the consumer read the same source.
pub fn render(path: &str) -> String {
    match spec_for(path).map(|spec| spec.kind) {
        Some(ControlKind::Bool) => {
            if bool_at(path, true) {
                "on".to_owned()
            } else {
                "off".to_owned()
            }
        }
        Some(ControlKind::Int { .. }) => int_at(path, 0).to_string(),
        Some(ControlKind::Percent { .. }) => format!("{}%", int_at(path, 0)),
        Some(ControlKind::Choice(_)) => str_at(path, ""),
        None => "?".to_owned(),
    }
}

/// Local-only write of one row's rendered value, for the optimistic UI update
/// that must land before the disk write does.
///
/// The shell store is process-wide, so the modal and the consumers see the new
/// value immediately; [`install`] replaces the whole map on the next config
/// load, which is what keeps a hand-edited file authoritative.
pub fn set_local(path: &str, rendered: String) {
    if !is_control(path) {
        return;
    }
    values().write().insert(path.to_owned(), rendered);
}

/// Test-only reset so a unit test can install without leaking into a sibling.
#[cfg(test)]
pub fn reset() {
    values().write().clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(toml_src: &str) -> toml::Value {
        toml::from_str(toml_src).unwrap()
    }

    #[test]
    fn every_row_defaults_to_todays_behaviour() {
        for spec in CONTROLS {
            match spec.kind {
                ControlKind::Bool => {
                    // The two rows that ship OFF today are named here; a third
                    // appearing silently would mean a behaviour change.
                    let expected = !matches!(
                        spec.path,
                        "laziness.enabled"
                            | "memory.gate.enabled"
                            | "agents.recommend.graph"
                            | "goal.verify.pre_filter"
                    );
                    assert_eq!(
                        spec.default_bool, expected,
                        "{} default drifts from today",
                        spec.path
                    );
                }
                ControlKind::Int { min, max } | ControlKind::Percent { min, max } => {
                    assert!(min < max, "{} has an empty span", spec.path);
                    assert!(
                        (min..=max).contains(&spec.default_int),
                        "{} default is out of bounds",
                        spec.path
                    );
                }
                ControlKind::Choice(choices) => {
                    assert!(!choices.is_empty(), "{} has no choices", spec.path);
                }
            }
        }
    }

    #[test]
    fn absent_config_reads_the_declared_default() {
        reset();
        install(&root(""));
        assert!(!bool_at("laziness.enabled", true));
        assert_eq!(int_at("todo_gate.max_items_named", 0), 3);
        assert_eq!(str_at("search.tool_backend", ""), "fused");
    }

    #[test]
    fn install_reads_the_dotted_paths() {
        reset();
        install(&root(
            "[permission.floors]\nexec = false\n\
             [todo_gate]\nmax_items_named = 7\n\
             [search]\ntool_backend = \"bm25\"\n",
        ));
        assert!(!bool_at("permission.floors.exec", true));
        assert!(bool_at("permission.floors.write", false));
        assert_eq!(int_at("todo_gate.max_items_named", 0), 7);
        assert_eq!(str_at("search.tool_backend", ""), "bm25");
    }

    #[test]
    fn integers_clamp_and_choices_reject_junk() {
        reset();
        install(&root(
            "[todo_gate]\nmax_items_named = 900\n\
             [search]\ntool_backend = \"carrier-pigeon\"\n",
        ));
        assert_eq!(int_at("todo_gate.max_items_named", 0), 20);
        assert_eq!(str_at("search.tool_backend", ""), "fused");
    }

    #[test]
    fn wrong_typed_value_falls_back_to_the_default() {
        reset();
        install(&root("[todo_gate]\nmax_items_named = \"three\"\n"));
        assert_eq!(int_at("todo_gate.max_items_named", 0), 3);
    }

    #[test]
    fn render_matches_the_typed_getters() {
        reset();
        install(&root("[permission.floors]\nexec = false\n"));
        assert_eq!(render("permission.floors.exec"), "off");
        assert_eq!(render("permission.floors.write"), "on");
        assert_eq!(render("todo_gate.max_items_named"), "3");
    }

    #[test]
    fn unknown_path_is_not_a_control() {
        assert!(!is_control("nope.nope"));
        assert!(is_control("todo_gate.enabled"));
    }
}
