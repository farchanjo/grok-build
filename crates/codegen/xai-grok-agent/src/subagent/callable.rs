//! Authoritative callable-agent descriptor snapshot (PR20).
//!
//! Builds a credential/body-free [`CallableAgentDescriptor`] list that exactly
//! reflects what the Task tool and spawn path can see at a given instant.
//!
//! The snapshot is derived from the SAME machinery actual spawning uses —
//! [`crate::discovery::all_subagents_with_plugins`] for discovery/shadowing/
//! toggle/plugin-qualified names, plus the caller's CLI-inline agents and a
//! vendor-compat gate. It does not copy or approximate precedence logic.
//!
//! Safety properties:
//! - **Credential/body-free**: descriptions come from frontmatter only and are
//!   never a prompt/system body. No secrets, credentials, or home paths.
//! - **Deterministic**: sorted by canonical name (`str::cmp`), matching the
//!   stable ordering the validation layer uses for its `available` lists.
//! - **Debug-safe**: [`CallableAgentDescriptor`] carries no secretable data.

use std::collections::HashMap;
use std::path::Path;

use xai_grok_tools::types::compat::CompatConfig;
use xai_grok_tools::types::config_source::ConfigSource;

use crate::config::{AgentDefinition, AgentScope};
use crate::discovery::{SubagentEntry, SubagentSource, all_subagents_with_plugins};
use crate::plugins::PluginRegistry;

/// Where a callable agent definition came from. Used for source labels and for
/// the shell's trust/plugin-qualification gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallableAgentSource {
    /// One of the built-in subagent types (or a built-in whose name was not
    /// shadowed by a higher-priority user/repo definition).
    Builtin,
    /// A user/repo/bundled definition, at its resolved discovery scope.
    UserDefined { scope: AgentScope },
    /// A plugin-qualified agent. `qualified` is true when the canonical name is
    /// in the `plugin:name` form.
    Plugin { plugin: String, qualified: bool },
}

impl CallableAgentSource {
    /// Short, secret-free source label used by ranking metadata and rendering.
    pub fn label(&self) -> String {
        match self {
            CallableAgentSource::Builtin => "builtin".to_string(),
            CallableAgentSource::UserDefined { scope } => scope.label().to_string(),
            CallableAgentSource::Plugin { plugin, .. } => format!("plugin:{plugin}"),
        }
    }
}

/// Credential/body-free authoritative snapshot of a callable agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallableAgentDescriptor {
    /// Canonical name (bare for native; `plugin:name` for plugin agents).
    pub name: String,
    /// Frontmatter-only description (`None` when the source provides none).
    /// Never a prompt/system body.
    pub description: Option<String>,
    pub source: CallableAgentSource,
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
    /// Vendor-compat gate. When `Some`, Claude-vendor agent sources are filtered
    /// on `compat.claude.agents`. `None` applies no vendor filter (the caller
    /// already applied it).
    pub compat: Option<&'a CompatConfig>,
}

impl<'a> CallableAgentOptions<'a> {
    /// Construct with the usual non-optional inputs (no plugins, no CLI agents,
    /// no compat gate).
    pub fn new(cwd: &'a Path, toggle: &'a HashMap<String, bool>) -> Self {
        Self {
            cwd,
            toggle,
            plugins: None,
            cli_agents: &[],
            compat: None,
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

/// Extract a filesystem path from a `ConfigSource` that carries one (`None` for
/// path-less sources). Used only to classify vendor provenance; never rendered.
fn config_source_path(cs: &ConfigSource) -> Option<&Path> {
    match cs {
        ConfigSource::Bundled { path }
        | ConfigSource::Server { path }
        | ConfigSource::Project { path }
        | ConfigSource::User { path }
        | ConfigSource::Plugin { path, .. }
        | ConfigSource::ConfigToml { path }
        | ConfigSource::ClaudeJson { path }
        | ConfigSource::McpJson { path }
        | ConfigSource::Cli { path } => Some(path),
        ConfigSource::Builtin => None,
        ConfigSource::Managed { path } => path.as_deref(),
    }
}

/// True when a config-source path belongs to a `.claude` vendor agent dir.
/// Used to gate Claude-vendor sources on the compat `claude.agents` cell.
fn is_claude_vendor_path(cs: &ConfigSource) -> bool {
    config_source_path(cs).is_some_and(|p| p.components().any(|c| c.as_os_str() == ".claude"))
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

/// Produce the authoritative callable-agent snapshot.
///
/// Order is deterministic (sorted by canonical name), matching the validation
/// layer's stable ordering. Disabled (toggled-off) agents are excluded via the
/// same toggle the Task tool uses, and CLI-inline agents are appended without
/// duplicating an already-present name.
pub fn callable_agent_snapshot(opts: &CallableAgentOptions<'_>) -> Vec<CallableAgentDescriptor> {
    let mut out: Vec<CallableAgentDescriptor> = Vec::new();

    // Authoritative discovery + shadowing + toggle + plugin-qualified names,
    // exactly the list `validate_subagent_type` / `by_name_in_cwd_with_plugins`
    // enumerate for the Task tool.
    for entry in all_subagents_with_plugins(opts.cwd, opts.toggle, opts.plugins) {
        // Vendor-compat gate: Claude-vendor agent sources honor `claude.agents`.
        if let Some(compat) = opts.compat
            && is_claude_vendor_path(&entry.config_source)
            && !compat.claude.agents
        {
            continue;
        }
        out.push(CallableAgentDescriptor {
            name: entry.name.clone(),
            description: safe_description(&entry.description),
            source: classify(&entry),
        });
    }

    // CLI-inline agents (shell `Config::cli_agents`), appended under the same
    // toggle default and never duplicating an existing name.
    for def in opts.cli_agents {
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

    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AgentDefinition, AgentScope};
    use crate::plugins::PluginOrigin;
    use crate::plugins::discovery::{PluginId, PluginScope};
    use crate::plugins::manifest::PluginManifest;
    use crate::plugins::{LoadedPlugin, PluginRegistry};
    use std::fs;
    use std::path::PathBuf;
    use xai_grok_tools::types::compat::VendorCompat;

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
        let loaded = LoadedPlugin {
            name: plugin_name.to_string(),
            id: PluginId::new(scope, &root, plugin_name),
            root: root.clone(),
            canonical_root: root.clone(),
            scope,
            origin: PluginOrigin::CliOverride,
            trusted: true,
            enabled: true,
            version: Some("1.0.0".to_string()),
            description: Some(format!("Plugin {plugin_name}")),
            skill_dirs: vec![],
            command_dirs: vec![],
            agent_dirs,
            hooks_path: None,
            mcp_config_path: None,
            skill_count: 0,
            agent_count: 0,
            skill_names: vec![],
            agent_names: vec![],
            has_hooks: false,
            hook_count: 0,
            has_inline_hooks_only: false,
            lsp_config_path: None,
            mcp_server_count: 0,
            has_inline_mcp_only: false,
            lsp_server_count: 0,
            has_inline_lsp_only: false,
            inline_hooks: None,
            inline_mcp_servers: None,
            inline_lsp_servers: None,
            conflict: None,
        };
        let LoadedPlugin { agent_dirs, .. } = loaded;
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
            compat: None,
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
            compat: None,
        };
        let snapshot = callable_agent_snapshot(&opts);
        assert!(
            !snapshot.iter().any(|d| d.name == "reviewer"),
            "toggled-off agent must be excluded"
        );
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
            compat: None,
        };
        let snapshot = callable_agent_snapshot(&opts);
        let count = snapshot.iter().filter(|d| d.name == "explore").count();
        assert_eq!(
            count, 1,
            "discovery must win over the CLI-inline fallback (no duplicate)"
        );
    }

    #[test]
    fn snapshot_is_sorted_deterministic() {
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
            compat: None,
        };
        let a = callable_agent_snapshot(&opts);
        let b = callable_agent_snapshot(&opts);
        assert_eq!(a, b, "snapshot must be deterministic");
        let names: Vec<&str> = a.iter().map(|d| d.name.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted, "snapshot must be name-sorted");
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
            compat: None,
        };
        let snapshot = callable_agent_snapshot(&opts);
        let huge = snapshot.iter().find(|d| d.name == "huge").unwrap();
        assert!(
            huge.description.as_ref().unwrap().chars().count() <= MAX_DESCRIPTION_CHARS,
            "description must be bounded"
        );
        let blank = snapshot.iter().find(|d| d.name == "blank").unwrap();
        assert!(blank.description.is_none(), "blank description -> None");
    }

    #[test]
    fn claude_vendor_filter_respects_compat() {
        let tmp = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        let cwd = root.join("workspace");
        fs::create_dir_all(&cwd).unwrap();
        write_agent(
            &cwd.join(".claude").join("agents"),
            "vendor.md",
            "vendor",
            "Vendor agent",
        );
        write_agent(
            &cwd.join(".grok").join("agents"),
            "native.md",
            "native",
            "Native agent",
        );

        let compat_off = CompatConfig {
            claude: VendorCompat {
                skills: true,
                rules: true,
                agents: false,
                mcps: true,
                hooks: true,
                sessions: true,
            },
            ..CompatConfig::default()
        };
        let opts_off = CallableAgentOptions {
            cwd: &cwd,
            toggle: &HashMap::new(),
            plugins: None,
            cli_agents: &[],
            compat: Some(&compat_off),
        };
        let snapshot_off = callable_agent_snapshot(&opts_off);
        assert!(
            !snapshot_off.iter().any(|d| d.name == "vendor"),
            "claude agent must be filtered when claude.agents disabled"
        );
        assert!(snapshot_off.iter().any(|d| d.name == "native"));

        let opts_on = CallableAgentOptions {
            cwd: &cwd,
            toggle: &HashMap::new(),
            plugins: None,
            cli_agents: &[],
            compat: Some(&CompatConfig::default()), // all cells on
        };
        let snapshot_on = callable_agent_snapshot(&opts_on);
        assert!(
            snapshot_on.iter().any(|d| d.name == "vendor"),
            "claude agent must appear when claude.agents enabled"
        );
    }
}
