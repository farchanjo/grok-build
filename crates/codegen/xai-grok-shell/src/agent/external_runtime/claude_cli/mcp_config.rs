//! Explicit Claude CLI MCP config generation (PR7).
//!
//! Generated temp config contains **only**:
//! 1. the Grok permission bridge, and
//! 2. explicitly user-approved external MCP servers when safely representable.
//!
//! Combined with `--strict-mcp-config` and `--safe-mode` so Claude does not
//! auto-discover project/user MCP, hooks, plugins, agents, or settings.
//! Never auto-imports Claude or Grok MCP catalogs. Never exposes duplicate
//! Grok Read/Edit/Bash through MCP while Claude built-ins are enabled.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::permission_bridge::{
    BRIDGE_MCP_SERVER_NAME, bridge_mcp_server_entry, permission_prompt_tool_flag,
};
use crate::agent::execution_backend::ExternalAgentKind;
use crate::agent::external_runtime::{ExternalRuntimeError, ExternalRuntimeErrorKind};

/// Explicitly approved external MCP server (stdio only — safest representation).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApprovedExternalMcpServer {
    pub name: String,
    pub command: PathBuf,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

impl ApprovedExternalMcpServer {
    /// Validate name + absolute command path. Rejects names that collide with
    /// the permission bridge or look like Grok built-in tool shims.
    pub fn validated(self) -> Result<Self, String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err("MCP server name is empty".into());
        }
        if name == BRIDGE_MCP_SERVER_NAME {
            return Err("MCP server name collides with permission bridge".into());
        }
        if name.eq_ignore_ascii_case("grok")
            || name.eq_ignore_ascii_case("grok-tools")
            || name.eq_ignore_ascii_case("grok-builtins")
        {
            return Err("refusing to expose Grok built-in tool shims via Claude MCP".into());
        }
        if self.command.as_os_str().is_empty() {
            return Err("MCP server command is empty".into());
        }
        if !self.command.is_absolute() {
            return Err(format!(
                "MCP server command must be absolute: {}",
                self.command.display()
            ));
        }
        // Reject obvious Grok tool duplicates by command basename.
        if let Some(base) = self.command.file_name().and_then(|s| s.to_str()) {
            let lower = base.to_ascii_lowercase();
            if lower.contains("grok-read")
                || lower.contains("grok-edit")
                || lower.contains("grok-bash")
            {
                return Err(
                    "refusing MCP command that looks like a Grok Read/Edit/Bash duplicate".into(),
                );
            }
        }
        Ok(Self {
            name: name.to_owned(),
            command: self.command,
            args: self.args,
            env: self.env,
        })
    }

    fn to_json(&self) -> Value {
        let mut m = serde_json::Map::new();
        m.insert(
            "command".into(),
            json!(self.command.to_string_lossy().into_owned()),
        );
        if !self.args.is_empty() {
            m.insert("args".into(), json!(self.args));
        }
        if !self.env.is_empty() {
            m.insert("env".into(), json!(self.env));
        }
        Value::Object(m)
    }
}

/// On-disk ephemeral MCP config + metadata for cleanup.
#[derive(Debug)]
pub struct GeneratedMcpConfig {
    pub path: PathBuf,
    pub permission_prompt_tool: String,
    /// Server names present in the config (bridge first).
    pub server_names: Vec<String>,
    dir: PathBuf,
}

impl GeneratedMcpConfig {
    pub fn cleanup(&self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_dir(&self.dir);
    }
}

impl Drop for GeneratedMcpConfig {
    fn drop(&mut self) {
        self.cleanup();
    }
}

/// Build a strict temp MCP config containing only the permission bridge and
/// any explicitly approved external servers.
pub fn write_strict_mcp_config(
    runtime_dir: &Path,
    host_executable: &Path,
    bridge_socket: &Path,
    approved_external: &[ApprovedExternalMcpServer],
) -> Result<GeneratedMcpConfig, ExternalRuntimeError> {
    let dir = runtime_dir.join("mcp");
    std::fs::create_dir_all(&dir).map_err(|e| {
        ExternalRuntimeError::new(
            ExternalRuntimeErrorKind::Transport,
            format!("mcp config dir: {e}"),
            Some(ExternalAgentKind::ClaudeCli),
        )
    })?;

    let mut servers = bridge_mcp_server_entry(host_executable, bridge_socket);
    let mut names = vec![BRIDGE_MCP_SERVER_NAME.to_owned()];

    for ext in approved_external {
        let validated = ext.clone().validated().map_err(|e| {
            ExternalRuntimeError::new(
                ExternalRuntimeErrorKind::InvalidRequest,
                format!("approved MCP server rejected: {e}"),
                Some(ExternalAgentKind::ClaudeCli),
            )
        })?;
        if servers.contains_key(&validated.name) {
            return Err(ExternalRuntimeError::new(
                ExternalRuntimeErrorKind::InvalidRequest,
                format!("duplicate MCP server name '{}'", validated.name),
                Some(ExternalAgentKind::ClaudeCli),
            ));
        }
        servers.insert(validated.name.clone(), validated.to_json());
        names.push(validated.name);
    }

    // Claude accepts either `{ "mcpServers": { ... } }` or a bare server map.
    // Prefer the documented wrapper form.
    let doc = json!({ "mcpServers": servers });
    let path = dir.join("claude-mcp-strict.json");
    let body = serde_json::to_vec_pretty(&doc).map_err(|e| {
        ExternalRuntimeError::new(
            ExternalRuntimeErrorKind::Transport,
            format!("mcp config serialize: {e}"),
            Some(ExternalAgentKind::ClaudeCli),
        )
    })?;
    std::fs::write(&path, body).map_err(|e| {
        ExternalRuntimeError::new(
            ExternalRuntimeErrorKind::Transport,
            format!("mcp config write: {e}"),
            Some(ExternalAgentKind::ClaudeCli),
        )
    })?;

    // Restrict permissions on the config file (contains paths only, no secrets
    // by construction — still best-effort private).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }

    Ok(GeneratedMcpConfig {
        path,
        permission_prompt_tool: permission_prompt_tool_flag(),
        server_names: names,
        dir,
    })
}

/// Assert a generated config document is strict (only listed servers, no
/// implicit discovery keys).
pub fn config_is_strict_only(doc: &Value, expected_servers: &[&str]) -> bool {
    let servers = doc
        .get("mcpServers")
        .and_then(|v| v.as_object())
        .or_else(|| doc.as_object());
    let Some(servers) = servers else {
        return false;
    };
    if servers.len() != expected_servers.len() {
        return false;
    }
    for name in expected_servers {
        if !servers.contains_key(*name) {
            return false;
        }
    }
    // No Grok built-in tool servers.
    !servers.keys().any(|k| {
        let lower = k.to_ascii_lowercase();
        lower.contains("grok-tools") || lower == "grok-builtins"
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn writes_bridge_only_config() {
        let dir = tempfile::tempdir().unwrap();
        let host = dir.path().join("host-bin");
        std::fs::write(&host, b"#!/bin/sh\n").unwrap();
        let mut perms = std::fs::metadata(&host).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&host, perms).unwrap();
        let sock = dir.path().join("bridge.sock");
        let cfg = write_strict_mcp_config(dir.path(), &host, &sock, &[]).unwrap();
        let raw = std::fs::read_to_string(&cfg.path).unwrap();
        let doc: Value = serde_json::from_str(&raw).unwrap();
        assert!(config_is_strict_only(&doc, &[BRIDGE_MCP_SERVER_NAME]));
        assert!(raw.contains(BRIDGE_MCP_SERVER_NAME));
        assert!(raw.contains("__claude-permission-bridge"));
        assert!(!raw.contains("ANTHROPIC_API_KEY"));
        assert_eq!(
            cfg.permission_prompt_tool,
            "mcp__grok-permission__permission_prompt"
        );
    }

    #[test]
    fn rejects_grok_builtin_shim_names() {
        let bad = ApprovedExternalMcpServer {
            name: "grok-tools".into(),
            command: PathBuf::from("/usr/bin/true"),
            args: vec![],
            env: HashMap::new(),
        };
        assert!(bad.validated().is_err());
    }

    #[test]
    fn accepts_explicit_external_stdio() {
        let dir = tempfile::tempdir().unwrap();
        let host = dir.path().join("host");
        std::fs::write(&host, b"x").unwrap();
        let mut perms = std::fs::metadata(&host).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&host, perms).unwrap();
        let sock = dir.path().join("s.sock");
        let ext = ApprovedExternalMcpServer {
            name: "docs-search".into(),
            command: PathBuf::from("/usr/bin/true"),
            args: vec!["--stdio".into()],
            env: HashMap::new(),
        };
        let cfg = write_strict_mcp_config(dir.path(), &host, &sock, &[ext]).unwrap();
        let doc: Value =
            serde_json::from_str(&std::fs::read_to_string(&cfg.path).unwrap()).unwrap();
        assert!(config_is_strict_only(
            &doc,
            &[BRIDGE_MCP_SERVER_NAME, "docs-search"]
        ));
    }

    #[test]
    fn rejects_relative_command() {
        let ext = ApprovedExternalMcpServer {
            name: "x".into(),
            command: PathBuf::from("relative-bin"),
            args: vec![],
            env: HashMap::new(),
        };
        assert!(ext.validated().is_err());
    }
}
