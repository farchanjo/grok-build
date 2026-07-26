//! Typed argv builder for Claude CLI invocations (PR6 one-shot + PR7 advanced).
//!
//! Official noninteractive safe profile: `--safe-mode`, `-p`, stream-json
//! input/output, `--verbose`, `--include-partial-messages`,
//! `--forward-subagent-text`. PR7 adds `--strict-mcp-config`, generated
//! `--mcp-config`, `--permission-prompt-tool`, and capability-mode
//! `--tools` / `--disallowedTools`. Never `--bare`. Never
//! `--dangerously-skip-permissions` / `bypassPermissions`. Never shell strings.

use std::ffi::OsString;
use std::path::PathBuf;

use super::capability_mode::{ClaudeCapabilityMode, disallowed_tools_flag_value, tools_flag_value};

/// Plan describing a single Claude CLI turn invocation.
#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeCliTurnArgv {
    pub executable: PathBuf,
    /// User prompt text (written as stream-json on stdin when using
    /// `--input-format stream-json`, and also available for `-p` argv form).
    pub prompt: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub max_budget_usd: Option<f64>,
    /// First-turn session id (UUID). Mutually exclusive with `resume_session`.
    pub session_id: Option<String>,
    /// Resume pointer from a prior envelope.
    pub resume_session: Option<String>,
    /// Working directory for the child.
    pub cwd: Option<PathBuf>,
    /// PR7: path to generated strict MCP config (permission bridge only).
    pub mcp_config: Option<PathBuf>,
    /// PR7: `--permission-prompt-tool` value (e.g. mcp__grok-permission__…).
    pub permission_prompt_tool: Option<String>,
    /// PR7: capability mode → tools allow/deny lists.
    pub capability_mode: Option<ClaudeCapabilityMode>,
    /// PR7: when true, keep stdin open for multi-turn (persistent session).
    pub persistent_input: bool,
}

/// Built argv + metadata (never includes secrets).
#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeCliArgvPlan {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    /// When true, write a stream-json user message to stdin.
    pub write_stream_json_prompt: bool,
    /// When true (default), close stdin after the first prompt (one-shot).
    /// Persistent multi-turn keeps stdin open.
    pub close_stdin_after_prompt: bool,
    pub prompt: String,
    pub cwd: Option<PathBuf>,
}

impl ClaudeCliTurnArgv {
    /// Build typed argv for the official safe noninteractive profile.
    ///
    /// Guarantees:
    /// - includes `--safe-mode`
    /// - includes `-p` / print mode
    /// - includes stream-json input + output formats
    /// - never includes `--bare` or `--dangerously-skip-permissions`
    /// - model / effort / budget / session / resume are typed flags only
    /// - PR7 MCP / permission / tools flags only when configured
    pub fn build_plan(&self) -> ClaudeCliArgvPlan {
        let mut args: Vec<OsString> = Vec::with_capacity(40);

        // Safe noninteractive profile (order matches common official examples).
        // --safe-mode disables implicit project/user MCP/hooks/plugins/agents.
        args.push(OsString::from("--safe-mode"));
        args.push(OsString::from("-p"));
        args.push(OsString::from("--output-format"));
        args.push(OsString::from("stream-json"));
        args.push(OsString::from("--input-format"));
        args.push(OsString::from("stream-json"));
        args.push(OsString::from("--verbose"));
        args.push(OsString::from("--include-partial-messages"));
        args.push(OsString::from("--forward-subagent-text"));

        // PR7: strict MCP config — only generated bridge (+ explicit approved).
        if let Some(mcp) = self.mcp_config.as_ref() {
            args.push(OsString::from("--strict-mcp-config"));
            args.push(OsString::from("--mcp-config"));
            args.push(OsString::from(mcp.as_os_str()));
        }
        if let Some(tool) = self
            .permission_prompt_tool
            .as_deref()
            .filter(|s| !s.is_empty())
        {
            args.push(OsString::from("--permission-prompt-tool"));
            args.push(OsString::from(tool));
        }

        // Capability-mode tool restriction (never bypassPermissions).
        if let Some(mode) = self.capability_mode {
            let tools = tools_flag_value(mode);
            if !tools.is_empty() {
                args.push(OsString::from("--tools"));
                args.push(OsString::from(tools));
            }
            if let Some(dis) = disallowed_tools_flag_value(mode) {
                args.push(OsString::from("--disallowedTools"));
                args.push(OsString::from(dis));
            }
        }

        if let Some(model) = self.model.as_deref().filter(|s| !s.is_empty()) {
            args.push(OsString::from("--model"));
            args.push(OsString::from(model));
        }
        if let Some(effort) = self.effort.as_deref().filter(|s| !s.is_empty()) {
            args.push(OsString::from("--effort"));
            args.push(OsString::from(effort));
        }
        if let Some(budget) = self.max_budget_usd {
            // Finite positive budgets only; skip non-finite / non-positive.
            if budget.is_finite() && budget > 0.0 {
                args.push(OsString::from("--max-budget-usd"));
                args.push(OsString::from(format_budget(budget)));
            }
        }

        // Resume takes precedence over first-turn session id.
        if let Some(resume) = self.resume_session.as_deref().filter(|s| !s.is_empty()) {
            args.push(OsString::from("--resume"));
            args.push(OsString::from(resume));
        } else if let Some(sid) = self.session_id.as_deref().filter(|s| !s.is_empty()) {
            args.push(OsString::from("--session-id"));
            args.push(OsString::from(sid));
        }

        // Prompt is delivered via stream-json stdin (input-format stream-json).
        // Do not append raw prompt as a free argv token when using stream-json
        // input — keeps quoting/control characters out of the process table.

        ClaudeCliArgvPlan {
            program: self.executable.clone(),
            args,
            write_stream_json_prompt: true,
            // Persistent mode keeps stdin open after the first prompt write.
            close_stdin_after_prompt: !self.persistent_input,
            prompt: self.prompt.clone(),
            cwd: self.cwd.clone(),
        }
    }
}

fn format_budget(budget: f64) -> String {
    // Avoid scientific notation; trim trailing zeros.
    let s = format!("{budget:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() {
        "0".to_owned()
    } else {
        s.to_owned()
    }
}

/// Encode a single user prompt as one stream-json NDJSON line for stdin.
pub fn stream_json_user_prompt_line(prompt: &str) -> String {
    // Minimal official-compatible user message for print mode stream-json input.
    let value = serde_json::json!({
        "type": "user",
        "message": {
            "role": "user",
            "content": prompt,
        }
    });
    let mut line = serde_json::to_string(&value)
        .unwrap_or_else(|_| r#"{"type":"user","message":{"role":"user","content":""}}"#.to_owned());
    line.push('\n');
    line
}

/// Assert helpers used by tests (and debug checks).
pub fn plan_uses_safe_mode_not_bare(plan: &ClaudeCliArgvPlan) -> bool {
    let args: Vec<String> = plan
        .args
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    args.iter().any(|a| a == "--safe-mode")
        && !args.iter().any(|a| a == "--bare")
        && !args.iter().any(|a| a == "--dangerously-skip-permissions")
        && !args.iter().any(|a| a == "bypassPermissions")
        && args.iter().any(|a| a == "-p")
}

/// True when plan includes strict MCP + permission-prompt-tool (PR7).
pub fn plan_has_strict_permission_bridge(plan: &ClaudeCliArgvPlan) -> bool {
    let args: Vec<String> = plan
        .args
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    args.iter().any(|a| a == "--strict-mcp-config")
        && args.iter().any(|a| a == "--mcp-config")
        && args.iter().any(|a| a == "--permission-prompt-tool")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample() -> ClaudeCliTurnArgv {
        ClaudeCliTurnArgv {
            executable: PathBuf::from("/usr/local/bin/claude"),
            prompt: "hello".into(),
            model: Some("sonnet".into()),
            effort: Some("high".into()),
            max_budget_usd: Some(1.5),
            session_id: Some("550e8400-e29b-41d4-a716-446655440000".into()),
            resume_session: None,
            cwd: Some(PathBuf::from("/tmp/ws")),
            mcp_config: None,
            permission_prompt_tool: None,
            capability_mode: None,
            persistent_input: false,
        }
    }

    #[test]
    fn argv_exact_safe_profile() {
        let plan = sample().build_plan();
        assert!(plan_uses_safe_mode_not_bare(&plan));
        let args: Vec<String> = plan
            .args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(args.contains(&"--output-format".into()));
        assert!(args.contains(&"stream-json".into()));
        assert!(args.contains(&"--input-format".into()));
        assert!(args.contains(&"--verbose".into()));
        assert!(args.contains(&"--include-partial-messages".into()));
        assert!(args.contains(&"--forward-subagent-text".into()));
        assert!(args.contains(&"--model".into()));
        assert!(args.contains(&"sonnet".into()));
        assert!(args.contains(&"--effort".into()));
        assert!(args.contains(&"high".into()));
        assert!(args.contains(&"--max-budget-usd".into()));
        assert!(args.contains(&"1.5".into()));
        assert!(args.contains(&"--session-id".into()));
        assert!(!args.iter().any(|a| a == "--bare"));
        // Prompt not in argv when using stream-json stdin.
        assert!(!args.iter().any(|a| a == "hello"));
        assert!(plan.write_stream_json_prompt);
    }

    #[test]
    fn resume_takes_precedence_over_session_id() {
        let mut t = sample();
        t.resume_session = Some("sess-resume-1".into());
        let plan = t.build_plan();
        let args: Vec<String> = plan
            .args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(args.contains(&"--resume".into()));
        assert!(args.contains(&"sess-resume-1".into()));
        assert!(!args.iter().any(|a| a == "--session-id"));
    }

    #[test]
    fn stream_json_prompt_is_single_ndjson_line() {
        let line = stream_json_user_prompt_line("hi\nthere");
        assert!(line.ends_with('\n'));
        let v: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(v["type"], "user");
        assert_eq!(v["message"]["content"], "hi\nthere");
    }

    #[test]
    fn pr7_strict_mcp_and_tools_flags() {
        let mut t = sample();
        t.mcp_config = Some(PathBuf::from("/tmp/mcp.json"));
        t.permission_prompt_tool = Some("mcp__grok-permission__permission_prompt".into());
        t.capability_mode = Some(ClaudeCapabilityMode::ReadOnly);
        let plan = t.build_plan();
        assert!(plan_has_strict_permission_bridge(&plan));
        assert!(plan_uses_safe_mode_not_bare(&plan));
        let args: Vec<String> = plan
            .args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(args.contains(&"--tools".into()));
        assert!(args.contains(&"--disallowedTools".into()));
        // Read-only denylist includes Edit/Bash
        let dis_idx = args.iter().position(|a| a == "--disallowedTools").unwrap();
        assert!(args[dis_idx + 1].contains("Edit"));
        assert!(args[dis_idx + 1].contains("Bash"));
        assert!(!args.iter().any(|a| a == "--dangerously-skip-permissions"));
    }

    #[test]
    fn persistent_keeps_stdin_open() {
        let mut t = sample();
        t.persistent_input = true;
        let plan = t.build_plan();
        assert!(!plan.close_stdin_after_prompt);
    }
}
