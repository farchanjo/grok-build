use crate::host::{AgentResult, BudgetState, WorkflowHostMessage, WorkflowHostRequest};
use crate::{Journal, WorkflowOutcome, WorkflowRunParams, extract_meta, run_workflow};

#[derive(Debug, Clone)]
pub struct ValidationReport {
    pub name: String,
    pub phases: usize,
    pub outcome_ok: bool,
    pub outcome_summary: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("meta: {0}")]
    Meta(#[from] crate::MetaError),
    #[error("dry-run: {0}")]
    Run(String),
    /// An `agent_type` literal the live callable roster does not resolve. The
    /// canned host never resolves types, so without this check a typo passes
    /// the smoke check and fails mid-run at the first spawn.
    #[error("agent_type: {0}")]
    AgentType(String),
}

/// Longest known-type list echoed into the [`ValidationError::AgentType`]
/// message.
const MAX_KNOWN_TYPES_ECHOED: usize = 24;

pub fn default_probe_args() -> serde_json::Value {
    serde_json::json!({
        "objective": "stub objective",
        "query": "stub query",
        "breadth": 2,
        "target": "stub target",
        "skeptic_count": 1,
        "max_verify_attempts": 1,
        "baseline_commit": "",
        "test_command": "cargo test",
        "diff_summary": "stub diff",
        "since_commit": "abc123",
    })
}

pub fn validate_script(
    script: &str,
    args: Option<serde_json::Value>,
) -> Result<ValidationReport, ValidationError> {
    validate_script_with_agent_budget(script, args, crate::DEFAULT_AGENT_BUDGET)
}

pub fn validate_script_with_agent_budget(
    script: &str,
    args: Option<serde_json::Value>,
    agent_budget: u64,
) -> Result<ValidationReport, ValidationError> {
    validate_script_with_known_agents(script, args, agent_budget, None)
}

/// [`validate_script_with_agent_budget`], additionally resolving every
/// `agent_type` literal a reached `agent()` call carries against
/// `known_agent_types` (the live callable roster; `None` skips the check).
///
/// The canned host answers every spawn successfully, so the types are checked
/// beside it rather than through it: an unknown type is collected and the
/// whole run is reported as failed after the script has been exercised, which
/// also catches a typo inside `parallel()` (where a host error would be
/// swallowed into a `()` slot).
pub fn validate_script_with_known_agents(
    script: &str,
    args: Option<serde_json::Value>,
    agent_budget: u64,
    known_agent_types: Option<&[String]>,
) -> Result<ValidationReport, ValidationError> {
    let meta = extract_meta(script)?;

    let unknown_agent_types: std::sync::Arc<std::sync::Mutex<std::collections::BTreeSet<String>>> =
        std::sync::Arc::new(std::sync::Mutex::new(std::collections::BTreeSet::new()));
    let known_owned: Option<Vec<String>> = known_agent_types.map(<[String]>::to_vec);
    let (host_tx, mut host_rx) = tokio::sync::mpsc::unbounded_channel();
    let host_unknown_agent_types = std::sync::Arc::clone(&unknown_agent_types);
    let host = std::thread::spawn(move || {
        use WorkflowHostRequest as R;
        let mut agent_calls = 0u64;
        while let Some(message) = host_rx.blocking_recv() {
            let req = match message {
                WorkflowHostMessage::Request(req) => req,
                WorkflowHostMessage::AssignedSpawn(envelope) => envelope.request,
            };
            match req {
                R::ReserveAgentCalls { count, reply } => {
                    let requested = agent_calls.saturating_add(count);
                    if requested > agent_budget {
                        let _ = reply.send(Err(crate::HostError::AgentCallQuotaExceeded {
                            requested,
                            maximum: agent_budget,
                        }));
                    } else {
                        agent_calls = requested;
                        let _ = reply.send(Ok(()));
                    }
                }
                R::ReleaseAgentCalls { count, reply } => {
                    agent_calls = agent_calls.saturating_sub(count);
                    let _ = reply.send(Ok(()));
                }
                R::SpawnAgent { opts, reply } => {
                    if let (Some(known), Some(requested)) =
                        (known_owned.as_deref(), opts.agent_type.as_deref())
                        && !known.iter().any(|name| name == requested)
                    {
                        host_unknown_agent_types
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner())
                            .insert(requested.to_string());
                    }
                    let _ = reply.send(Ok(AgentResult {
                        agent_id: "stub".into(),
                        success: true,
                        output: serde_json::json!({
                            "achieved": true,
                            "gaps": "",
                            "evidence": "stub evidence",
                            "questions": ["q1", "q2"],
                            "claims": [],
                            "uncertainties": [],
                            "verdicts": [],
                            "failures": ["test_a"],
                            "issues": "none",
                            "stub": true
                        }),
                        cancelled: false,
                        tokens_used: 1,
                        duration_ms: 1,
                    }));
                }
                R::Decide {
                    questions, reply, ..
                } => {
                    // Deterministic canned answers: every question key gets a
                    // `noul` of 0.5 and, when the question carries criteria,
                    // the first criterion key as the `choice`, so a script
                    // that reads either shape exercises its branch.
                    let mut answers = serde_json::Map::new();
                    for (key, question) in questions.as_object().cloned().unwrap_or_default() {
                        let mut answer = serde_json::Map::new();
                        answer.insert("noul".into(), serde_json::json!(0.5));
                        if let Some(first) = question
                            .get("criteria")
                            .and_then(|criteria| criteria.as_object())
                            .and_then(|criteria| criteria.keys().next())
                        {
                            answer.insert("choice".into(), serde_json::json!(first));
                        }
                        answers.insert(key, serde_json::Value::Object(answer));
                    }
                    let _ = reply.send(Ok(serde_json::Value::Object(answers)));
                }
                R::BudgetQuery { reply } => {
                    let _ = reply.send(Ok(BudgetState {
                        total: None,
                        spent: 0,
                        reserved: 0,
                        remaining: None,
                    }));
                }
                R::RenderTemplate { reply, .. } => {
                    let _ = reply.send(Ok("stub template".into()));
                }
                R::WriteScratchFile { name, reply, .. } => {
                    let _ = reply.send(Ok(format!("scratch/{name}")));
                }
                R::ReadScratchFile { reply, .. } => {
                    let _ = reply.send(Ok("stub content".into()));
                }
                R::GitDiffSince { reply, .. } => {
                    let _ = reply.send(Ok("".into()));
                }
                R::Phase { .. } | R::Log { .. } | R::Telemetry { .. } => {}
            }
        }
    });

    let outcome = run_workflow(WorkflowRunParams {
        script: script.to_string(),
        args: args.unwrap_or_else(default_probe_args),
        journal: Journal::new(None),
        host_tx,
        cancel: tokio_util::sync::CancellationToken::new(),
        max_ops: 10_000_000,
    });
    drop(host);

    let (outcome_ok, outcome_summary) = match &outcome {
        WorkflowOutcome::Completed { result } => (
            true,
            format!("completed: {}", truncate(&result.to_string())),
        ),
        WorkflowOutcome::Paused { kind, message } => {
            (true, format!("paused ({kind:?}): {}", truncate(message)))
        }
        WorkflowOutcome::Failed { error } => (false, format!("failed: {error}")),
        other => (false, format!("{other:?}")),
    };
    if !outcome_ok {
        return Err(ValidationError::Run(outcome_summary));
    }
    let unknown = unknown_agent_types
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    if !unknown.is_empty()
        && let Some(known) = known_agent_types
    {
        return Err(ValidationError::AgentType(unknown_agent_type_message(
            &unknown, known,
        )));
    }

    Ok(ValidationReport {
        name: meta.name,
        phases: meta.phases.len(),
        outcome_ok,
        outcome_summary,
    })
}

/// `unknown` names plus a bounded echo of the roster, so the author can fix
/// the literal without a second lookup.
fn unknown_agent_type_message(
    unknown: &std::collections::BTreeSet<String>,
    known: &[String],
) -> String {
    let names = unknown.iter().cloned().collect::<Vec<_>>().join(", ");
    let mut roster = known.to_vec();
    roster.sort();
    let shown = roster
        .iter()
        .take(MAX_KNOWN_TYPES_ECHOED)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    let more = roster.len().saturating_sub(MAX_KNOWN_TYPES_ECHOED);
    let suffix = if more > 0 {
        format!("{shown}, … ({more} more)")
    } else {
        shown
    };
    format!(
        "unknown agent_type {names} — the canned host never resolves types, so this would fail \
         at the first spawn mid-run. Known agent types: {suffix}"
    )
}

fn truncate(s: &str) -> String {
    if s.chars().count() > 200 {
        let head: String = s.chars().take(200).collect();
        format!("{head}…")
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_script_passes() {
        let report = validate_script(
            "let meta = #{ name: \"t\", description: \"d\" };\nlet r = agent(\"work\");\ncomplete(r.output);",
            None,
        )
        .unwrap();
        assert_eq!(report.name, "t");
        assert!(report.outcome_ok);
    }

    #[test]
    fn missing_meta_fails() {
        assert!(matches!(
            validate_script("let x = 1;", None),
            Err(ValidationError::Meta(_))
        ));
    }

    #[test]
    fn default_probe_args_exercise_bundled_and_authoring_examples() {
        let args = default_probe_args();
        assert!(
            args["objective"]
                .as_str()
                .is_some_and(|value| !value.is_empty())
        );
        assert!(
            args["query"]
                .as_str()
                .is_some_and(|value| !value.is_empty())
        );
        assert!(
            args["target"]
                .as_str()
                .is_some_and(|value| !value.is_empty())
        );
        assert!(args["breadth"].as_u64().is_some_and(|value| value >= 2));
        assert!(
            args["skeptic_count"]
                .as_u64()
                .is_some_and(|value| value >= 1)
        );
        assert!(
            args["max_verify_attempts"]
                .as_u64()
                .is_some_and(|value| value >= 1)
        );
    }

    #[test]
    fn runtime_misuse_fails() {
        let err = validate_script(
            "let meta = #{ name: \"t\", description: \"d\" };\nnot_a_host_fn();",
            None,
        )
        .unwrap_err();
        assert!(matches!(err, ValidationError::Run(_)), "{err}");
    }

    #[test]
    fn unknown_agent_type_fails_the_smoke_check_but_a_known_one_passes() {
        let script = r#"
            let meta = #{ name: "t", description: "d" };
            let r = agent("work", #{ agent_type: "rust-enginer" });
            complete(r.output);
        "#;
        let known = vec!["explore".to_string(), "rust-engineer".to_string()];
        let error = validate_script_with_known_agents(script, None, 128, Some(&known)).unwrap_err();
        assert!(matches!(error, ValidationError::AgentType(_)), "{error}");
        assert!(error.to_string().contains("rust-enginer"), "{error}");
        assert!(error.to_string().contains("rust-engineer"), "{error}");

        let fixed = script.replace("rust-enginer", "rust-engineer");
        validate_script_with_known_agents(&fixed, None, 128, Some(&known)).unwrap();
        // No roster supplied keeps today's behaviour: no type check.
        validate_script(script, None).unwrap();
    }

    #[test]
    fn unknown_agent_type_inside_parallel_is_caught() {
        let script = r#"
            let meta = #{ name: "t", description: "d" };
            let jobs = [#{ prompt: "a", agent_type: "nope" }];
            parallel(jobs);
            complete("done");
        "#;
        let known = vec!["explore".to_string()];
        let error = validate_script_with_known_agents(script, None, 128, Some(&known)).unwrap_err();
        assert!(matches!(error, ValidationError::AgentType(_)), "{error}");
    }

    #[test]
    fn decide_with_a_canned_answer_passes_the_smoke_check() {
        let script = r#"
            let meta = #{ name: "t", description: "d" };
            let a = decide(#{ x: 1 }, #{ q1: #{ "type": "noul", "instructions": "?" } });
            complete(a.q1.noul);
        "#;
        // The canned host has no Decide arm of its own: the request is
        // answered by the same `Ok` reply the spawn arm uses.
        validate_script(script, None).unwrap();
    }

    #[test]
    fn pause_counts_as_valid() {
        let report = validate_script(
            "let meta = #{ name: \"t\", description: \"d\" };\npause(\"verification\", \"needs input\");",
            None,
        )
        .unwrap();
        assert!(report.outcome_ok);
    }

    #[test]
    fn engine_limits_are_reported_as_dry_run_failures() {
        let script = format!(
            r#"
            let meta = #{{ name: "t", description: "d" }};
            let jobs = [];
            for i in 0..{} {{ jobs.push(#{{ prompt: "job" + i.to_string() }}); }}
            parallel(jobs);
            "#,
            crate::MAX_PARALLEL + 1
        );
        let error = validate_script(&script, None).unwrap_err().to_string();
        assert!(error.contains("parallel() accepts at most"), "got: {error}");

        let script = format!(
            r#"
            let meta = #{{ name: "t", description: "d" }};
            let jobs = [];
            for i in 0..{} {{ jobs.push(#{{ prompt: "job" + i.to_string() }}); }}
            parallel(jobs);
            agent("synthesize");
            "#,
            crate::DEFAULT_AGENT_BUDGET
        );
        let error = validate_script(&script, None).unwrap_err().to_string();
        assert!(
            error.contains(&format!(
                "agent budget exceeded: requested {}, maximum {}",
                crate::DEFAULT_AGENT_BUDGET + 1,
                crate::DEFAULT_AGENT_BUDGET
            )),
            "got: {error}"
        );
    }

    #[test]
    fn authoring_landmines_are_fixed_or_hinted() {
        let concat = |terms: usize| {
            let chain = (0..terms)
                .map(|i| format!("\"part{i}\""))
                .collect::<Vec<_>>()
                .join(" + ");
            format!(
                "let meta = #{{ name: \"t\", description: \"d\" }};\nlet p = {chain};\ncomplete(p);"
            )
        };
        assert!(validate_script(&concat(100), None).unwrap().outcome_ok);

        let hinted = |script: &str, expect: &[&str]| {
            let msg = validate_script(script, None).unwrap_err().to_string();
            for e in expect {
                assert!(msg.contains(e), "missing {e:?} in: {msg}");
            }
        };
        hinted(&concat(300), &["maximum complexity", "`+=` statements"]);
        hinted(
            "let meta = #{ name: \"t\", description: \"d\" };\nlet shared = false;\ncomplete(shared);",
            &["reserved keyword", "rename the variable"],
        );
        hinted(
            "let meta = #{ name: \"t\", description: \"d\" };\nlet s = \"abc\";\ncomplete(s[0].severity);",
            &["type 'char'", "indexing a string"],
        );
    }
}
