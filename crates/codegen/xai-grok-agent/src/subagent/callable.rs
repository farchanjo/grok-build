//! Authoritative callable-agent descriptor snapshot (PR20).
//!
//! Builds a credential/body-free [`CallableAgentDescriptor`] list that exactly
//! reflects what the Task tool and spawn path can see at a given instant.
//!
//! The snapshot is derived from the SAME machinery actual spawning uses —
//! [`crate::discovery::all_subagents_with_plugins`] for discovery/shadowing/
//! toggle/plugin-qualified names (including the filename-based plugin identity
//! and plugin toggles now enforced there), plus the caller's CLI-inline agents
//! and every validation-resolvable built-in. It does not copy or approximate
//! precedence logic.
//!
//! Safety properties:
//! - **Credential/body-free**: descriptions come from frontmatter only and are
//!   never a prompt/system body. No secrets, credentials, or home paths.
//! - **Deterministic**: sorted by canonical name (`str::cmp`); injected order
//!   is deduplicated (first occurrence wins, matching discovery precedence) so
//!   the snapshot is injective on canonical names.
//! - **Debug-safe**: [`CallableAgentDescriptor`]'s custom `Debug` redacts
//!   description text (surfaces only its length).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use strum::IntoEnumIterator;
use xai_grok_tools::types::config_source::ConfigSource;

use crate::config::{AgentDefinition, AgentScope, BuiltinAgentName};
use crate::discovery::{SubagentEntry, SubagentSource, all_subagents_with_plugins};
use crate::plugins::PluginRegistry;

/// Where a callable agent definition came from. Used for source labels and for
/// the shell's trust/plugin-qualification gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallableAgentSource {
    /// One of the built-in agent types (including the non-subagent built-ins
    /// that are still valid Task targets), not shadowed by a higher-priority
    /// user/repo definition.
    Builtin,
    /// A user/repo/bundled definition, at its resolved discovery scope.
    UserDefined { scope: AgentScope },
    /// A plugin-qualified agent. `qualified` is true when the canonical name is
    /// in the `plugin:name` form.
    Plugin { plugin: String, qualified: bool },
    /// A CLI-inline (`Config::cli_agents`) definition. Distinct from [`Builtin`]
    /// so same-named native/CLI entries are never mislabeled as deep-backed.
    CliInline,
}

impl CallableAgentSource {
    /// Short, secret-free source label used by ranking metadata and rendering.
    pub fn label(&self) -> String {
        match self {
            CallableAgentSource::Builtin => "builtin".to_string(),
            CallableAgentSource::UserDefined { scope } => scope.label().to_string(),
            CallableAgentSource::Plugin { plugin, .. } => format!("plugin:{plugin}"),
            CallableAgentSource::CliInline => "cli-inline".to_string(),
        }
    }
}

/// Credential/body-free authoritative snapshot of a callable agent.
#[derive(Clone, PartialEq, Eq)]
pub struct CallableAgentDescriptor {
    /// Canonical name (bare for native; `plugin:name` for plugin agents).
    pub name: String,
    /// Frontmatter-only description (`None` when the source provides none).
    /// Never a prompt/system body.
    pub description: Option<String>,
    pub source: CallableAgentSource,
}

impl std::fmt::Debug for CallableAgentDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Description text is frontmatter metadata, but it is still suppressed
        // here so that `{:?}` of a raw descriptor list never surfaces it in any
        // telemetry/Debug path.
        f.debug_struct("CallableAgentDescriptor")
            .field("name", &self.name)
            .field("source", &self.source)
            .field(
                "description_chars",
                &self.description.as_ref().map(|d| d.chars().count()),
            )
            .finish()
    }
}

/// Inputs mirroring the spawn/validation context the shell supplies.
pub struct CallableAgentOptions<'a> {
    /// Working directory that roots discovery (cwd → git root).
    pub cwd: &'a Path,
    /// `[subagents.toggle]` map; absent keys default to enabled.
    pub toggle: &'a HashMap<String, bool>,
    pub plugins: Option<&'a PluginRegistry>,
    /// CLI-inline agent definitions (shell `Config::cli_agents`).
    pub cli_agents: &'a [AgentDefinition],
}

impl<'a> CallableAgentOptions<'a> {
    /// Construct with the usual non-optional inputs (no plugins, no CLI agents).
    pub fn new(cwd: &'a Path, toggle: &'a HashMap<String, bool>) -> Self {
        Self {
            cwd,
            toggle,
            plugins: None,
            cli_agents: &[],
        }
    }
}

/// Max UTF-8 chars for a frontmatter description captured into a descriptor.
/// Description is metadata only (rendering caps further); this is a defensive
/// upper bound so an oversized description never dominates ranking or render.
const MAX_DESCRIPTION_CHARS: usize = 512;

fn cap_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// Normalize a frontmatter description into a safe, bounded `Option<String>`.
/// Empty/whitespace-only descriptions become `None`.
fn safe_description(description: &str) -> Option<String> {
    let trimmed = description.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(cap_chars(trimmed, MAX_DESCRIPTION_CHARS))
    }
}

/// Classify a [`SubagentEntry`] into a secret-free [`CallableAgentSource`].
fn classify(entry: &SubagentEntry) -> CallableAgentSource {
    // Plugin-backed entries are authoritative via their ConfigSource.
    if let ConfigSource::Plugin { plugin_name, .. } = &entry.config_source {
        let qualified = entry.name.contains(':');
        return CallableAgentSource::Plugin {
            plugin: plugin_name.clone(),
            qualified,
        };
    }
    match &entry.source {
        SubagentSource::Builtin(_) => CallableAgentSource::Builtin,
        SubagentSource::UserDefined { scope } => CallableAgentSource::UserDefined { scope: *scope },
    }
}

/// Every built-in agent name that is a valid Task target. The spawn path
/// resolves any `BuiltinAgentName` via `by_name_in_cwd_with_plugins`, and
/// `validate_subagent_type`/`gate_subagent_type` apply only toggle + allow-list
/// on top. So the full built-in set (not just `subagent_variants()`) is
/// spawnable; include the non-subagent builtins unless a higher-priority
/// user/repo/plugin definition shadows their name.
///
/// `grok-build-plan-no-subagents` is deliberately EXCLUDED: its definition
/// strips the Task tool from its own toolset (`grok_build_plan_no_subagents_toolset`
/// omits TaskTool), so it is defined to spawn no sub-agents; recommending it as
/// a callable Task target would contradict that policy. It is still resolvable
/// by name at spawn (existing behavior), just not surfaced as a recommendation.
fn non_subagent_builtins() -> Vec<AgentDefinition> {
    let variants: HashSet<&'static str> = BuiltinAgentName::subagent_variants()
        .iter()
        .map(|b| b.as_ref())
        .collect();
    BuiltinAgentName::iter()
        .filter(|b| !variants.contains(b.as_ref()))
        .filter(|b| !matches!(b, BuiltinAgentName::GrokBuildPlanNoSubagents))
        .map(BuiltinAgentName::definition)
        .collect()
}

/// Produce the authoritative callable-agent snapshot.
///
/// Order is deterministic (sorted by canonical name), matching the validation
/// layer's stable ordering. Disabled (toggled-off) agents are excluded via the
/// same toggle the Task tool uses, including plugin-qualified names. CLI-inline
/// and non-shadowed built-in agents are appended without duplicating an
/// already-present name. The result is injective on canonical names.
pub fn callable_agent_snapshot(opts: &CallableAgentOptions<'_>) -> Vec<CallableAgentDescriptor> {
    let mut out: Vec<CallableAgentDescriptor> = Vec::new();

    // Authoritative discovery + shadowing + toggle + plugin-qualified names,
    // exactly the list `validate_subagent_type` / `by_name_in_cwd_with_plugins`
    // enumerate for the Task tool. `all_subagents_with_plugins` already applies
    // the toggle to native and plugin agents by canonical name.
    for entry in all_subagents_with_plugins(opts.cwd, opts.toggle, opts.plugins) {
        out.push(CallableAgentDescriptor {
            name: entry.name.clone(),
            description: safe_description(&entry.description),
            source: classify(&entry),
        });
    }

    // Non-subagent built-ins that are still valid Task targets, appended only
    // when not shadowed by a higher-priority user/repo/plugin name above.
    for def in non_subagent_builtins() {
        if opts.toggle.get(&def.name).copied().unwrap_or(true)
            && !out.iter().any(|d| d.name == def.name)
        {
            out.push(CallableAgentDescriptor {
                name: def.name.clone(),
                description: safe_description(&def.description),
                source: CallableAgentSource::Builtin,
            });
        }
    }

    // CLI-inline agents (shell `Config::cli_agents`), appended under the same
    // toggle default, never duplicating an existing name, and labeled CliInline
    // (never Builtin).
    for def in opts.cli_agents {
        if opts.toggle.get(&def.name).copied().unwrap_or(true)
            && !out.iter().any(|d| d.name == def.name)
        {
            out.push(CallableAgentDescriptor {
                name: def.name.clone(),
                description: safe_description(&def.description),
                source: CallableAgentSource::CliInline,
            });
        }
    }

    // Defensive injectivity: keep the first occurrence of each canonical name
    // (discovery precedence order) so ids/ranks/render stay one-to-one.
    let mut deduped: Vec<CallableAgentDescriptor> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for d in out {
        if seen.insert(d.name.clone()) {
            deduped.push(d);
        }
    }

    deduped.sort_by(|a, b| a.name.cmp(&b.name));
    deduped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AgentDefinition, AgentScope};
    use crate::plugins::discovery::{PluginId, PluginScope};
    use crate::plugins::manifest::PluginManifest;
    use crate::plugins::{PluginOrigin, PluginRegistry};
    use std::fs;
    use std::path::PathBuf;

    fn write_agent(dir: &Path, filename: &str, name: &str, desc: &str) {
        let content = format!("---\nname: {name}\ndescription: {desc}\n---\n");
        let _ = fs::create_dir_all(dir);
        fs::write(dir.join(filename), content).unwrap();
    }

    fn make_plugin_registry(
        plugin_name: &str,
        scope: PluginScope,
        agent_dirs: Vec<PathBuf>,
    ) -> PluginRegistry {
        let root = PathBuf::from(format!("/tmp/primed-{plugin_name}"));
        let discovered = crate::plugins::DiscoveredPlugin {
            manifest: PluginManifest {
                name: plugin_name.to_string(),
                version: Some("1.0.0".to_string()),
                description: Some(format!("Plugin {plugin_name}")),
                author: None,
                homepage: None,
                repository: None,
                license: None,
                keywords: vec![],
                skills: None,
                commands: None,
                agents: None,
                hooks: None,
                mcp_servers: None,
                lsp_servers: None,
            },
            id: PluginId::new(scope, &root, plugin_name),
            root: root.clone(),
            canonical_root: root,
            scope,
            origin: PluginOrigin::CliOverride,
            trusted: true,
            skill_dirs: vec![],
            command_dirs: vec![],
            agent_dirs,
            hooks_path: None,
            mcp_config_path: None,
            lsp_config_path: None,
            conflict: None,
        };
        PluginRegistry::from_discovered(vec![discovered], &[], &[plugin_name.to_string()])
    }

    #[test]
    fn snapshot_includes_builtin_repo_plugin_and_cli_inline() {
        let tmp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        let cwd = root.join("workspace");
        fs::create_dir_all(&cwd).unwrap();
        // Repo agent.
        write_agent(
            &cwd.join(".grok").join("agents"),
            "reviewer.md",
            "reviewer",
            "Repo reviewer",
        );
        // Plugin agent.
        let plugin_dir = root.join("plugins").join("my-plugin");
        let plugin_agents = plugin_dir.join("agents");
        write_agent(&plugin_agents, "arch.md", "arch", "Plugin architect");
        let registry = make_plugin_registry("my-plugin", PluginScope::User, vec![plugin_agents]);
        // CLI-inline agent.
        let mut cli_def = AgentDefinition::general_purpose();
        cli_def.name = "cli-inline".into();
        cli_def.description = "CLI-inline agent".into();
        let opts = CallableAgentOptions {
            cwd: &cwd,
            toggle: &HashMap::new(),
            plugins: Some(&registry),
            cli_agents: &[cli_def],
        };
        let snapshot = callable_agent_snapshot(&opts);
        let names: Vec<&str> = snapshot.iter().map(|d| d.name.as_str()).collect();
        assert!(
            snapshot.iter().any(|d| d.name == "general-purpose")
                || snapshot.iter().any(|d| d.name == "explore"),
            "builtin agents expected: {names:?}"
        );
        assert!(names.contains(&"reviewer"), "repo agent missing: {names:?}");
        assert!(
            names.contains(&"my-plugin:arch"),
            "plugin agent missing: {names:?}"
        );
        assert!(
            names.contains(&"cli-inline"),
            "cli-inline missing: {names:?}"
        );
        // Plugin source classification.
        let arch = snapshot
            .iter()
            .find(|d| d.name == "my-plugin:arch")
            .unwrap();
        assert_eq!(
            arch.source,
            CallableAgentSource::Plugin {
                plugin: "my-plugin".into(),
                qualified: true,
            }
        );
        // CLI-inline source classification (never labeled Builtin).
        let cli = snapshot.iter().find(|d| d.name == "cli-inline").unwrap();
        assert_eq!(cli.source, CallableAgentSource::CliInline);
    }

    #[test]
    fn snapshot_includes_non_subagent_builtins_when_not_shadowed() {
        let tmp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        let cwd = root.join("workspace");
        fs::create_dir_all(&cwd).unwrap();
        let opts = CallableAgentOptions {
            cwd: &cwd,
            toggle: &HashMap::new(),
            plugins: None,
            cli_agents: &[],
        };
        let snapshot = callable_agent_snapshot(&opts);
        // Non-subagent builtins are valid Task targets and must be present.
        assert!(
            snapshot.iter().any(|d| d.name == "grok-build"),
            "expected non-subagent builtin grok-build; got {:?}",
            snapshot.iter().map(|d| d.name.as_str()).collect::<Vec<_>>()
        );
        assert!(snapshot.iter().any(|d| d.name == "codex"));
        assert!(snapshot.iter().any(|d| d.name == "explore"));
        // R3: `grok-build-plan-no-subagents` is deliberately excluded — its
        // definition strips the Task tool from its own toolset (it is defined
        // to spawn no sub-agents), so it must not be recommended.
        assert!(
            !snapshot
                .iter()
                .any(|d| d.name == "grok-build-plan-no-subagents"),
            "no-subagents builtin must not be surfaced as a callable recommendation"
        );
    }

    #[test]
    fn non_subagent_builtin_is_shadowed_by_higher_priority_repo_agent() {
        let tmp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        let cwd = root.join("workspace");
        fs::create_dir_all(&cwd).unwrap();
        // A project "grok-build" shadows the built-in under the same name.
        write_agent(
            &cwd.join(".grok").join("agents"),
            "grok-build.md",
            "grok-build",
            "Project grok-build",
        );
        // A project "codex" too.
        write_agent(
            &cwd.join(".grok").join("agents"),
            "codex.md",
            "codex",
            "Project codex",
        );
        let opts = CallableAgentOptions {
            cwd: &cwd,
            toggle: &HashMap::new(),
            plugins: None,
            cli_agents: &[],
        };
        let snapshot = callable_agent_snapshot(&opts);
        let grok = snapshot.iter().find(|d| d.name == "grok-build").unwrap();
        assert_eq!(
            grok.source,
            CallableAgentSource::UserDefined {
                scope: AgentScope::Project
            },
            "project agent must shadow the built-in"
        );
        assert!(
            snapshot.iter().filter(|d| d.name == "grok-build").count() == 1,
            "no duplicate builtin+project entry"
        );
    }

    #[test]
    fn toggle_excludes_disabled_agents() {
        let tmp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        let cwd = root.join("workspace");
        fs::create_dir_all(&cwd).unwrap();
        write_agent(
            &cwd.join(".grok").join("agents"),
            "reviewer.md",
            "reviewer",
            "Rev",
        );
        let mut toggle = HashMap::new();
        toggle.insert("reviewer".to_string(), false);
        let opts = CallableAgentOptions {
            cwd: &cwd,
            toggle: &toggle,
            plugins: None,
            cli_agents: &[],
        };
        let snapshot = callable_agent_snapshot(&opts);
        assert!(
            !snapshot.iter().any(|d| d.name == "reviewer"),
            "toggled-off agent must be excluded"
        );
    }

    #[test]
    fn toggle_excludes_disabled_plugin_agent() {
        // M1: a toggled-off plugin agent (qualified name key) must be excluded
        // from the descriptor snapshot itself, not string-until-revalidation.
        let tmp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        let cwd = root.join("workspace");
        fs::create_dir_all(&cwd).unwrap();
        let plugin_agents = root.join("plugins").join("pl").join("agents");
        write_agent(&plugin_agents, "arch.md", "arch", "Plugin architect");
        let registry = make_plugin_registry("pl", PluginScope::User, vec![plugin_agents]);
        let mut toggle = HashMap::new();
        toggle.insert("pl:arch".to_string(), false);
        let opts = CallableAgentOptions {
            cwd: &cwd,
            toggle: &toggle,
            plugins: Some(&registry),
            cli_agents: &[],
        };
        let snapshot = callable_agent_snapshot(&opts);
        assert!(
            !snapshot.iter().any(|d| d.name == "pl:arch"),
            "toggled-off plugin agent must be excluded: {:?}",
            snapshot.iter().map(|d| d.name.as_str()).collect::<Vec<_>>()
        );

        // Reverting the toggle restores it.
        let mut toggle = HashMap::new();
        toggle.insert("pl:arch".to_string(), true);
        let opts = CallableAgentOptions {
            cwd: &cwd,
            toggle: &toggle,
            plugins: Some(&registry),
            cli_agents: &[],
        };
        let snapshot = callable_agent_snapshot(&opts);
        assert!(snapshot.iter().any(|d| d.name == "pl:arch"));
    }

    #[test]
    fn plugin_identity_uses_filename_not_frontmatter_name() {
        // M4: plugin `alpha.md` with frontmatter `name: beta` spawns as
        // `pl:alpha`, never `pl:beta`. The snapshot must key on the filename.
        let tmp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        let cwd = root.join("workspace");
        fs::create_dir_all(&cwd).unwrap();
        let plugin_agents = root.join("plugins").join("pl").join("agents");
        write_agent(&plugin_agents, "alpha.md", "beta", "Plugin beta-named");
        let registry = make_plugin_registry("pl", PluginScope::User, vec![plugin_agents]);
        let opts = CallableAgentOptions {
            cwd: &cwd,
            toggle: &HashMap::new(),
            plugins: Some(&registry),
            cli_agents: &[],
        };
        let snapshot = callable_agent_snapshot(&opts);
        assert!(
            snapshot.iter().any(|d| d.name == "pl:alpha"),
            "filename-based name expected: {:?}",
            snapshot.iter().map(|d| d.name.as_str()).collect::<Vec<_>>()
        );
        assert!(
            !snapshot.iter().any(|d| d.name == "pl:beta"),
            "frontmatter name must not be used for plugin identity"
        );
        // The description still comes from frontmatter.
        let alpha = snapshot.iter().find(|d| d.name == "pl:alpha").unwrap();
        assert_eq!(alpha.description.as_deref(), Some("Plugin beta-named"));
    }

    #[test]
    fn cli_inline_does_not_duplicate_native_name() {
        let tmp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        let cwd = root.join("workspace");
        fs::create_dir_all(&cwd).unwrap();
        // A native repo "explore" shadows the builtin.
        write_agent(
            &cwd.join(".grok").join("agents"),
            "explore.md",
            "explore",
            "Repo explore",
        );
        let mut cli_def = AgentDefinition::general_purpose();
        cli_def.name = "explore".into();
        cli_def.description = "CLI explore".into();
        let opts = CallableAgentOptions {
            cwd: &cwd,
            toggle: &HashMap::new(),
            plugins: None,
            cli_agents: &[cli_def],
        };
        let snapshot = callable_agent_snapshot(&opts);
        let count = snapshot.iter().filter(|d| d.name == "explore").count();
        assert_eq!(
            count, 1,
            "discovery must win over the CLI-inline fallback (no duplicate)"
        );
        let explore = snapshot.iter().find(|d| d.name == "explore").unwrap();
        assert_eq!(
            explore.source,
            CallableAgentSource::UserDefined {
                scope: AgentScope::Project
            },
            "discovery entry wins; the CLI fallback must not replace it"
        );
    }

    #[test]
    fn snapshot_is_sorted_deterministic_and_injective() {
        let tmp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        let cwd = root.join("workspace");
        fs::create_dir_all(&cwd).unwrap();
        write_agent(&cwd.join(".grok").join("agents"), "zebra.md", "zebra", "Z");
        write_agent(&cwd.join(".grok").join("agents"), "apple.md", "apple", "A");
        write_agent(&cwd.join(".grok").join("agents"), "mango.md", "mango", "M");
        let opts = CallableAgentOptions {
            cwd: &cwd,
            toggle: &HashMap::new(),
            plugins: None,
            cli_agents: &[],
        };
        let a = callable_agent_snapshot(&opts);
        let b = callable_agent_snapshot(&opts);
        assert_eq!(a, b, "snapshot must be deterministic");
        let names: Vec<&str> = a.iter().map(|d| d.name.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted, "snapshot must be name-sorted");
        // Injective on canonical names.
        let mut set = std::collections::HashSet::new();
        for n in &names {
            assert!(set.insert(n), "duplicate name in snapshot: {n}");
        }
    }

    #[test]
    fn descriptions_are_frontmatter_only_and_bounded() {
        let tmp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        let cwd = root.join("workspace");
        fs::create_dir_all(&cwd).unwrap();
        let hugo = "x".repeat(10_000);
        write_agent(&cwd.join(".grok").join("agents"), "huge.md", "huge", &hugo);
        write_agent(
            &cwd.join(".grok").join("agents"),
            "blank.md",
            "blank",
            "   ",
        );
        let opts = CallableAgentOptions {
            cwd: &cwd,
            toggle: &HashMap::new(),
            plugins: None,
            cli_agents: &[],
        };
        let snapshot = callable_agent_snapshot(&opts);
        let huge = snapshot.iter().find(|d| d.name == "huge").unwrap();
        assert!(
            huge.description.as_ref().unwrap().chars().count() <= MAX_DESCRIPTION_CHARS,
            "description must be bounded"
        );
        let blank = snapshot.iter().find(|d| d.name == "blank").unwrap();
        assert!(blank.description.is_none(), "blank description -> None");
        // Debug must not surface the description text.
        let dbg = format!("{huge:?}");
        assert!(
            !dbg.contains("xxxxx"),
            "Debug leaked description text: {dbg}"
        );
        assert!(dbg.contains("description_chars"));
    }

    #[test]
    fn cli_qualified_style_name_is_not_duplicated_by_plugin() {
        // A CLI-inline agent literally named `p:arch` and a plugin `p:arch`
        // share a display name; the snapshot keeps one (discovery wins), and the
        // remaining id is one-to-one.
        let tmp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        let cwd = root.join("workspace");
        fs::create_dir_all(&cwd).unwrap();
        let plugin_agents = root.join("plugins").join("p").join("agents");
        write_agent(&plugin_agents, "arch.md", "arch", "Plugin arch");
        let registry = make_plugin_registry("p", PluginScope::User, vec![plugin_agents]);
        let mut cli_def = AgentDefinition::general_purpose();
        cli_def.name = "p:arch".into();
        cli_def.description = "CLI p:arch".into();
        let opts = CallableAgentOptions {
            cwd: &cwd,
            toggle: &HashMap::new(),
            plugins: Some(&registry),
            cli_agents: &[cli_def],
        };
        let snapshot = callable_agent_snapshot(&opts);
        assert_eq!(
            snapshot.iter().filter(|d| d.name == "p:arch").count(),
            1,
            "qualified-style CLI name must not duplicate the plugin entry"
        );
    }
}
