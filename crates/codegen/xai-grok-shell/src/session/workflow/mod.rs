pub(crate) mod host_service;
pub(crate) mod manager;
pub(crate) mod notify;
pub(crate) mod registry;
pub(crate) mod schema_contract;
pub(crate) mod store;
pub(crate) mod tracker;

/// One catalog entry, shaped for a text listing so the pager never has to name
/// an `xai_workflow` type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowCatalogEntry {
    pub name: String,
    pub description: String,
    /// When the model should reach for this workflow, when the script says.
    pub when_to_use: Option<String>,
    /// `builtin` | `user` | `project` | `session`.
    pub source: String,
    /// Absolute path for a file-backed workflow, absent for a builtin.
    pub path: Option<String>,
}

/// Workflow catalog visible from `cwd`, name-sorted.
///
/// Reads the same registry the resolver uses, so the listing and the run can
/// never disagree about what exists.
pub fn catalog(cwd: Option<&std::path::Path>) -> Vec<WorkflowCatalogEntry> {
    registry::list_workflows(cwd)
        .into_iter()
        .map(|entry| WorkflowCatalogEntry {
            name: entry.name,
            description: entry.description,
            when_to_use: entry.when_to_use,
            source: entry.source.to_owned(),
            path: entry.path,
        })
        .collect()
}

#[cfg(test)]
mod builtin_tests {
    #[test]
    fn every_builtin_validates_and_matches_its_registered_name() {
        for builtin in super::registry::BUILTIN_WORKFLOWS {
            let meta = xai_workflow::extract_meta(builtin.script)
                .unwrap_or_else(|e| panic!("builtin '{}' must validate: {e}", builtin.name));
            assert_eq!(
                meta.name, builtin.name,
                "registry key must equal meta.name for '{}'",
                builtin.name
            );
        }
    }

    /// The shipped builtin must survive the same smoke check a user runs: the
    /// full script compiles, one canned path executes, and every `agent_type`
    /// literal it reaches resolves against a roster. The roster below is the
    /// shipped agent set those literals name, so a renamed or misspelled
    /// literal fails here instead of mid-run.
    #[test]
    fn every_builtin_passes_the_smoke_check_with_its_agent_types() {
        const ROSTER: [&str; 7] = [
            "general-purpose",
            "explore",
            "plan",
            "data-analyst",
            "data-scientist",
            "architect-reviewer",
            "documentation-engineer",
        ];
        let roster: Vec<String> = ROSTER.iter().map(|name| (*name).to_owned()).collect();
        for builtin in super::registry::BUILTIN_WORKFLOWS {
            let report = xai_workflow::validate_script_with_known_agents(
                builtin.script,
                None,
                xai_workflow::DEFAULT_AGENT_BUDGET,
                Some(&roster),
            )
            .unwrap_or_else(|e| {
                panic!("builtin '{}' must pass its smoke check: {e}", builtin.name)
            });
            assert_eq!(report.name, builtin.name);
        }
    }

    #[test]
    fn deep_research_binds_shards_and_renders_verified_claims() {
        let script = super::registry::BUILTIN_WORKFLOWS
            .iter()
            .find(|builtin| builtin.name == "deep-research")
            .map(|builtin| builtin.script)
            .expect("deep-research builtin registered");
        assert!(script.contains("expected_ids[shard_idx]"));
        assert!(script.contains("verification_results[assigned_shard]"));
        assert!(script.contains("verified_claim_ids"));
        assert!(script.contains("**Status: Partial**"));
        assert!(!script.contains("label: \"research-reporter\""));
        assert!(script.contains("label: \"report-synthesizer\""));
        assert!(script.contains("<report-body>"));
        assert!(!script.contains("output_schema: synthesis_schema"));
        assert!(script.contains("failed citation validation"));
        assert!(script.contains("let findings_fallback"));
        assert!(script.contains("full_report += \"\\n## Sources\\n\""));
        assert!(script.contains("report: chat_report"));
        assert!(!script.contains("chat_report += \"\\n## Sources\\n\""));
    }
}
