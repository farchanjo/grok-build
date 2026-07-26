//! Conservative capability-mode mapping for Claude Agent CLI (PR7).
//!
//! Grok capability modes restrict which Claude **built-in** tools are exposed
//! via `--tools` / `--disallowedTools`. Approvals for remaining tools are
//! **always** brokered by the permission bridge via
//! [`PermissionHandle::request`] — never `bypassPermissions`, never a
//! short-circuit allow path.

use serde::{Deserialize, Serialize};

/// Host capability mode applied to a Claude CLI session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ClaudeCapabilityMode {
    /// Restrict Claude tools to read / search. Bridge denies write/shell.
    #[default]
    ReadOnly,
    /// File read + write tools; shell still brokered (not auto-allowed).
    ReadWrite,
    /// Read/write + shell execution tools; all still brokered.
    Execute,
    /// Full set of required Claude built-ins; still brokered.
    All,
    /// Broader **tool allowlist** only (when user explicitly opts into
    /// always-approve / yolo). Does **not** short-circuit permission decisions;
    /// every call still goes through PermissionHandle so managed PolicyDeny wins.
    AlwaysApprove,
}

impl ClaudeCapabilityMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read_only",
            Self::ReadWrite => "read_write",
            Self::Execute => "execute",
            Self::All => "all",
            Self::AlwaysApprove => "always_approve",
        }
    }

    /// Parse a host permission / session mode string conservatively.
    ///
    /// `auto` maps to brokered [`Self::All`] (not yolo). Explicit
    /// `always-approve` / `yolo` select the broader allowlist mode only.
    pub fn from_host_label(label: &str) -> Self {
        match label.trim().to_ascii_lowercase().as_str() {
            "read" | "read-only" | "read_only" | "readonly" | "plan" => Self::ReadOnly,
            "read-write" | "read_write" | "readwrite" | "edit" | "acceptedits" => Self::ReadWrite,
            "execute" | "exec" | "bash" | "shell" => Self::Execute,
            // Explicit always-approve / yolo → broader allowlist (still brokered).
            "always-approve" | "always_approve" | "yolo" => Self::AlwaysApprove,
            // `auto` is classifier/default-brokered mode, NOT yolo.
            "auto" | "all" | "full" | "default" | "agent" => Self::All,
            _ => Self::ReadOnly, // unknown → most restrictive
        }
    }
}

/// Claude built-in tools considered read/search only.
pub const READ_SEARCH_TOOLS: &[&str] = &[
    "Read",
    "Grep",
    "Glob",
    "LS",
    "WebSearch",
    "WebFetch",
    "TaskOutput",
    "TodoRead",
];

/// Write / mutation tools (denied under ReadOnly by argv + bridge).
pub const WRITE_TOOLS: &[&str] = &["Edit", "Write", "MultiEdit", "NotebookEdit", "TodoWrite"];

/// Shell / execution tools.
pub const SHELL_TOOLS: &[&str] = &["Bash", "PowerShell", "Monitor"];

/// Additional tools exposed in Execute / All (still brokered).
pub const EXTRA_AGENT_TOOLS: &[&str] = &["Agent", "Skill", "AskUserQuestion", "ExitPlanMode"];

/// Whether `tool_name` is write or shell (including mcp-qualified forms).
pub fn is_write_or_shell_tool(tool_name: &str) -> bool {
    let bare = bare_tool_name(tool_name);
    WRITE_TOOLS
        .iter()
        .chain(SHELL_TOOLS.iter())
        .any(|t| bare.eq_ignore_ascii_case(t))
        || bare.eq_ignore_ascii_case("run_terminal_cmd")
        || bare.eq_ignore_ascii_case("search_replace")
        || bare.eq_ignore_ascii_case("write_file")
}

pub fn is_shell_tool(tool_name: &str) -> bool {
    let bare = bare_tool_name(tool_name);
    SHELL_TOOLS.iter().any(|t| bare.eq_ignore_ascii_case(t))
        || bare.eq_ignore_ascii_case("run_terminal_cmd")
}

pub fn is_write_tool(tool_name: &str) -> bool {
    let bare = bare_tool_name(tool_name);
    WRITE_TOOLS.iter().any(|t| bare.eq_ignore_ascii_case(t))
        || bare.eq_ignore_ascii_case("search_replace")
        || bare.eq_ignore_ascii_case("write_file")
}

fn bare_tool_name(tool_name: &str) -> &str {
    let name = tool_name.trim();
    name.strip_prefix("mcp__")
        .and_then(|rest| rest.split_once("__").map(|(_, t)| t))
        .unwrap_or(name)
}

/// Tools list for `--tools` (restrict available built-ins).
pub fn tools_allowlist(mode: ClaudeCapabilityMode) -> Vec<&'static str> {
    match mode {
        ClaudeCapabilityMode::ReadOnly => READ_SEARCH_TOOLS.to_vec(),
        ClaudeCapabilityMode::ReadWrite => {
            let mut v = READ_SEARCH_TOOLS.to_vec();
            v.extend_from_slice(WRITE_TOOLS);
            v
        }
        ClaudeCapabilityMode::Execute => {
            let mut v = READ_SEARCH_TOOLS.to_vec();
            v.extend_from_slice(WRITE_TOOLS);
            v.extend_from_slice(SHELL_TOOLS);
            v
        }
        ClaudeCapabilityMode::All | ClaudeCapabilityMode::AlwaysApprove => {
            let mut v = READ_SEARCH_TOOLS.to_vec();
            v.extend_from_slice(WRITE_TOOLS);
            v.extend_from_slice(SHELL_TOOLS);
            v.extend_from_slice(EXTRA_AGENT_TOOLS);
            v
        }
    }
}

/// Tools list for `--disallowedTools` (hard remove from context where possible).
pub fn tools_denylist(mode: ClaudeCapabilityMode) -> Vec<&'static str> {
    match mode {
        ClaudeCapabilityMode::ReadOnly => {
            let mut v = WRITE_TOOLS.to_vec();
            v.extend_from_slice(SHELL_TOOLS);
            v.extend_from_slice(EXTRA_AGENT_TOOLS);
            v
        }
        ClaudeCapabilityMode::ReadWrite => {
            let mut v = SHELL_TOOLS.to_vec();
            v.extend_from_slice(&["Agent"]);
            v
        }
        ClaudeCapabilityMode::Execute => Vec::new(),
        ClaudeCapabilityMode::All | ClaudeCapabilityMode::AlwaysApprove => Vec::new(),
    }
}

/// Format for `--tools` CLI flag (comma-separated).
pub fn tools_flag_value(mode: ClaudeCapabilityMode) -> String {
    tools_allowlist(mode).join(",")
}

/// Format for `--disallowedTools` when non-empty.
pub fn disallowed_tools_flag_value(mode: ClaudeCapabilityMode) -> Option<String> {
    let d = tools_denylist(mode);
    if d.is_empty() {
        None
    } else {
        Some(d.join(","))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_excludes_edit_and_bash() {
        let allow = tools_allowlist(ClaudeCapabilityMode::ReadOnly);
        assert!(allow.contains(&"Read"));
        assert!(!allow.contains(&"Edit"));
        assert!(!allow.contains(&"Bash"));
        let deny = tools_denylist(ClaudeCapabilityMode::ReadOnly);
        assert!(deny.contains(&"Edit"));
        assert!(deny.contains(&"Bash"));
    }

    #[test]
    fn auto_maps_to_all_not_always_approve() {
        assert_eq!(
            ClaudeCapabilityMode::from_host_label("auto"),
            ClaudeCapabilityMode::All
        );
        assert_eq!(
            ClaudeCapabilityMode::from_host_label("AUTO"),
            ClaudeCapabilityMode::All
        );
        assert_ne!(
            ClaudeCapabilityMode::from_host_label("auto"),
            ClaudeCapabilityMode::AlwaysApprove
        );
    }

    #[test]
    fn yolo_and_always_approve_select_allowlist_mode_only() {
        assert_eq!(
            ClaudeCapabilityMode::from_host_label("yolo"),
            ClaudeCapabilityMode::AlwaysApprove
        );
        assert_eq!(
            ClaudeCapabilityMode::from_host_label("always-approve"),
            ClaudeCapabilityMode::AlwaysApprove
        );
        // Broader allowlist, still no bypass flag semantics here.
        let allow = tools_allowlist(ClaudeCapabilityMode::AlwaysApprove);
        assert!(allow.contains(&"Bash"));
    }

    #[test]
    fn host_label_defaults_unknown_to_read_only() {
        assert_eq!(
            ClaudeCapabilityMode::from_host_label("mystery"),
            ClaudeCapabilityMode::ReadOnly
        );
    }

    #[test]
    fn write_shell_detection() {
        assert!(is_write_or_shell_tool("Edit"));
        assert!(is_write_or_shell_tool("Bash"));
        assert!(!is_write_or_shell_tool("Read"));
    }
}
