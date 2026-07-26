//! Persistent streaming input for Claude Agent CLI (PR7).
//!
//! Enabled **only** when probe/init capabilities explicitly advertise
//! streaming/persistent input support (see [`resume_guard::supports_persistent_input`]).
//! One child per external session; queue **one turn at a time**; send multiple
//! stream-json user messages on stdin; parse turn boundaries/results. The host
//! prompt queue remains the authority for ordering — no concurrent turn
//! consumption.
//!
//! On child death: persist session pointer, then fall back to one-shot
//! `--resume` only when the capability matrix permits. If capability is absent,
//! each turn spawns one process on the session-scoped runtime. Interrupt/cancel
//! when supported. Process bridge + Claude + descendants form one lifecycle tree.

use std::path::PathBuf;
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use xai_tty_utils::{ProcessGroup, new_process_group};

use super::argv::{ClaudeCliArgvPlan, stream_json_user_prompt_line};
use super::env_scrub::apply_scrubbed_env;
use super::process::{ProcessLimits, TurnProcessError, TurnProcessOutcome};
use super::protocol;
use super::resume_guard::{supports_interrupt, supports_oneshot_resume, supports_persistent_input};
use crate::agent::external_runtime::{
    ExternalRuntimeEnvelope, ExternalRuntimeTurnEvent, ExternalTurnOutcome,
};

/// Whether persistent mode may be used for this capability set.
pub fn persistent_mode_allowed(capabilities: &[String]) -> bool {
    supports_persistent_input(capabilities)
}

/// Live persistent Claude child (stdin held open across turns).
pub struct PersistentClaudeSession {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<tokio::process::ChildStdout>,
    group: Option<ProcessGroup>,
    limits: ProcessLimits,
    /// Capabilities observed at init (authority for interrupt/resume).
    pub capabilities: Vec<String>,
    pub session_pointer: Option<String>,
    pub observed_version: Option<String>,
    pub model: Option<String>,
    /// Turn gate: only one turn at a time.
    turn_busy: bool,
    /// Accumulated lines for the current turn.
    current_lines: Vec<String>,
    stderr_acc: Arc<Mutex<Vec<u8>>>,
    pub cancelled: bool,
}

impl PersistentClaudeSession {
    /// Spawn a persistent child from an argv plan (stdin kept open).
    pub async fn spawn(
        plan: &ClaudeCliArgvPlan,
        limits: ProcessLimits,
    ) -> Result<Self, TurnProcessError> {
        let mut cmd = Command::new(&plan.program);
        cmd.args(&plan.args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        if let Some(cwd) = &plan.cwd {
            cmd.current_dir(cwd);
        }
        apply_scrubbed_env(&mut cmd, &[]);
        new_process_group(&mut cmd);

        let mut child = cmd
            .spawn()
            .map_err(|e| TurnProcessError::Spawn(e.to_string()))?;

        let group = match ProcessGroup::new() {
            Ok(mut g) => {
                if let Err(e) = g.attach(&child) {
                    let _ = child.start_kill();
                    let _ = child.wait().await;
                    return Err(TurnProcessError::Spawn(format!(
                        "process group attach failed: {e}"
                    )));
                }
                Some(g)
            }
            Err(e) => {
                let _ = child.start_kill();
                let _ = child.wait().await;
                return Err(TurnProcessError::Spawn(format!(
                    "process group create failed: {e}"
                )));
            }
        };

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| TurnProcessError::io("stdin missing"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| TurnProcessError::io("stdout missing"))?;
        let stderr = child.stderr.take();

        let stderr_acc = Arc::new(Mutex::new(Vec::new()));
        let stderr_acc2 = stderr_acc.clone();
        let max_err = limits.max_stderr_bytes;
        tokio::spawn(async move {
            if let Some(mut r) = stderr {
                use tokio::io::AsyncReadExt;
                let mut buf = vec![0u8; 4096];
                loop {
                    match r.read(&mut buf).await {
                        Ok(0) => break,
                        Ok(n) => {
                            let mut acc = stderr_acc2.lock().await;
                            if acc.len() < max_err {
                                let take = n.min(max_err - acc.len());
                                acc.extend_from_slice(&buf[..take]);
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
        });

        Ok(Self {
            child,
            stdin,
            stdout: BufReader::with_capacity(64 * 1024, stdout),
            group,
            limits,
            capabilities: Vec::new(),
            session_pointer: None,
            observed_version: None,
            model: None,
            turn_busy: false,
            current_lines: Vec::new(),
            stderr_acc,
            cancelled: false,
        })
    }

    /// Run one turn: write user message, read until final result (or cancel).
    ///
    /// Refuses concurrent turns (`turn_busy`).
    pub async fn run_turn(
        &mut self,
        prompt: &str,
        cancel: CancellationToken,
    ) -> Result<TurnProcessOutcome, TurnProcessError> {
        if self.turn_busy {
            return Err(TurnProcessError::io(
                "persistent session refuses concurrent turns",
            ));
        }
        self.turn_busy = true;
        self.current_lines.clear();
        self.cancelled = false;

        let line = stream_json_user_prompt_line(prompt);
        if let Err(e) = self.stdin.write_all(line.as_bytes()).await {
            self.turn_busy = false;
            return Err(TurnProcessError::io(format!("stdin write: {e}")));
        }
        if let Err(e) = self.stdin.flush().await {
            self.turn_busy = false;
            return Err(TurnProcessError::io(format!("stdin flush: {e}")));
        }

        let turn_deadline = tokio::time::Instant::now() + self.limits.turn;
        let startup_deadline = tokio::time::Instant::now() + self.limits.startup;
        let mut saw_first = false;
        let mut total_stdout = 0usize;
        let mut saw_result = false;

        let outcome = loop {
            if cancel.is_cancelled() {
                self.cancelled = true;
                if supports_interrupt(&self.capabilities) {
                    // Soft interrupt: still terminate process group for MVP
                    // safety (official control interrupt is SDK-path only).
                }
                self.terminate(cancel.clone()).await;
                break TurnProcessOutcome {
                    lines: std::mem::take(&mut self.current_lines),
                    stderr: self.stderr_string().await,
                    exit_code: Some(143),
                    cancelled: true,
                    exit_signal: Some(15),
                };
            }

            let now = tokio::time::Instant::now();
            if now >= turn_deadline {
                self.terminate(cancel.clone()).await;
                self.turn_busy = false;
                return Err(TurnProcessError::TurnTimeout {
                    partial_lines: self.current_lines.clone(),
                });
            }
            if !saw_first && now >= startup_deadline {
                self.terminate(cancel.clone()).await;
                self.turn_busy = false;
                return Err(TurnProcessError::StartupTimeout {
                    partial_lines: self.current_lines.clone(),
                });
            }

            let idle = if saw_first {
                self.limits.idle
            } else {
                startup_deadline.saturating_duration_since(now)
            };

            let mut line_buf: Vec<u8> = Vec::new();
            let read_result = tokio::select! {
                biased;
                _ = cancel.cancelled() => {
                    self.cancelled = true;
                    self.terminate(CancellationToken::new()).await;
                    break TurnProcessOutcome {
                        lines: std::mem::take(&mut self.current_lines),
                        stderr: self.stderr_string().await,
                        exit_code: Some(143),
                        cancelled: true,
                        exit_signal: Some(15),
                    };
                }
                _ = tokio::time::sleep(idle) => {
                    Err(if saw_first {
                        TurnProcessError::IdleTimeout {
                            partial_lines: self.current_lines.clone(),
                        }
                    } else {
                        TurnProcessError::StartupTimeout {
                            partial_lines: self.current_lines.clone(),
                        }
                    })
                }
                res = read_line_capped(&mut self.stdout, &mut line_buf, self.limits.max_line_bytes) => {
                    res.map_err(|e| match e {
                        TurnProcessError::LineTooLarge { bytes, max, .. } => {
                            TurnProcessError::LineTooLarge {
                                bytes,
                                max,
                                partial_lines: self.current_lines.clone(),
                            }
                        }
                        TurnProcessError::Io { message, .. } => {
                            TurnProcessError::io_with_lines(message, self.current_lines.clone())
                        }
                        other => other,
                    })
                }
            };

            match read_result {
                Ok(0) => {
                    // EOF — child died mid-session.
                    break TurnProcessOutcome {
                        lines: std::mem::take(&mut self.current_lines),
                        stderr: self.stderr_string().await,
                        exit_code: self
                            .child
                            .try_wait()
                            .ok()
                            .and_then(|s| s.map(|x| x.code()).flatten()),
                        cancelled: self.cancelled,
                        exit_signal: None,
                    };
                }
                Ok(_) => {
                    saw_first = true;
                    total_stdout = total_stdout.saturating_add(line_buf.len());
                    if total_stdout > self.limits.max_stdout_bytes {
                        self.terminate(cancel.clone()).await;
                        self.turn_busy = false;
                        return Err(TurnProcessError::OutputTooLarge {
                            bytes: total_stdout,
                            max: self.limits.max_stdout_bytes,
                            partial_lines: self.current_lines.clone(),
                        });
                    }
                    while line_buf.last().copied() == Some(b'\n')
                        || line_buf.last().copied() == Some(b'\r')
                    {
                        line_buf.pop();
                    }
                    if line_buf.is_empty() {
                        continue;
                    }
                    let line = match String::from_utf8(line_buf) {
                        Ok(s) => s,
                        Err(_) => {
                            self.terminate(cancel.clone()).await;
                            self.turn_busy = false;
                            return Err(TurnProcessError::InvalidUtf8 {
                                partial_lines: self.current_lines.clone(),
                            });
                        }
                    };
                    // Update session metadata from init/result opportunistically.
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                        let ty = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        if ty == "system"
                            && v.get("subtype").and_then(|s| s.as_str()) == Some("init")
                        {
                            if let Some(sid) = v.get("session_id").and_then(|s| s.as_str()) {
                                self.session_pointer = Some(sid.to_owned());
                            }
                            if let Some(model) = v.get("model").and_then(|s| s.as_str()) {
                                self.model = Some(model.to_owned());
                            }
                            if let Some(ver) = v
                                .get("claude_code_version")
                                .or_else(|| v.get("version"))
                                .and_then(|s| s.as_str())
                            {
                                self.observed_version = Some(ver.to_owned());
                            }
                            if let Some(caps) = v.get("capabilities").and_then(|c| c.as_array()) {
                                self.capabilities = caps
                                    .iter()
                                    .filter_map(|c| c.as_str().map(|s| s.to_owned()))
                                    .collect();
                            }
                        }
                        if ty == "result" {
                            if let Some(sid) = v.get("session_id").and_then(|s| s.as_str()) {
                                self.session_pointer = Some(sid.to_owned());
                            }
                            saw_result = true;
                        }
                    }
                    self.current_lines.push(line);
                    if saw_result {
                        break TurnProcessOutcome {
                            lines: std::mem::take(&mut self.current_lines),
                            stderr: self.stderr_string().await,
                            exit_code: None, // still alive
                            cancelled: false,
                            exit_signal: None,
                        };
                    }
                }
                Err(e) => {
                    self.terminate(cancel.clone()).await;
                    self.turn_busy = false;
                    return Err(e);
                }
            }
        };

        self.turn_busy = false;
        Ok(outcome)
    }

    /// Whether the child is still running.
    pub fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Persistable session pointer after child death / cancel.
    pub fn pointer_for_resume(&self) -> Option<String> {
        self.session_pointer.clone()
    }

    /// Whether one-shot --resume fallback is allowed after death.
    pub fn oneshot_resume_allowed(&self) -> bool {
        supports_oneshot_resume(&self.capabilities) && self.session_pointer.is_some()
    }

    async fn stderr_string(&self) -> String {
        let acc = self.stderr_acc.lock().await;
        String::from_utf8_lossy(&acc).into_owned()
    }

    async fn terminate(&mut self, _cancel: CancellationToken) {
        if let Some(ref g) = self.group {
            let _ = g.terminate();
        }
        let grace = self.limits.shutdown_grace;
        match tokio::time::timeout(grace, self.child.wait()).await {
            Ok(_) => {
                self.group = None;
            }
            Err(_) => {
                if let Some(ref g) = self.group {
                    let _ = g.kill();
                }
                let _ = self.child.start_kill();
                let _ = tokio::time::timeout(grace, self.child.wait()).await;
                self.group = None;
            }
        }
    }

    /// Shutdown: SIGTERM process group, reap, no orphans.
    pub async fn shutdown(&mut self) {
        self.terminate(CancellationToken::new()).await;
        // Drop stdin to signal EOF if still open.
        // (already consumed on terminate paths)
    }
}

impl Drop for PersistentClaudeSession {
    fn drop(&mut self) {
        // Best-effort kill on drop to avoid orphans.
        if let Some(ref g) = self.group {
            let _ = g.kill();
        }
        let _ = self.child.start_kill();
    }
}

async fn read_line_capped<R: tokio::io::AsyncBufRead + Unpin>(
    reader: &mut R,
    buf: &mut Vec<u8>,
    max: usize,
) -> Result<usize, TurnProcessError> {
    use tokio::io::AsyncReadExt;
    buf.clear();
    loop {
        let mut byte = [0u8; 1];
        let n = reader
            .read(&mut byte)
            .await
            .map_err(|e| TurnProcessError::io(e.to_string()))?;
        if n == 0 {
            return Ok(buf.len());
        }
        if buf.len() >= max {
            return Err(TurnProcessError::LineTooLarge {
                bytes: buf.len() + 1,
                max,
                partial_lines: Vec::new(),
            });
        }
        buf.push(byte[0]);
        if byte[0] == b'\n' {
            return Ok(buf.len());
        }
    }
}

/// Apply a turn outcome to an envelope (shared by one-shot and persistent).
pub fn apply_outcome_to_envelope(
    envelope: &ExternalRuntimeEnvelope,
    lines: &[String],
    allow_incomplete: bool,
) -> Result<ExternalTurnOutcome, protocol::ProtocolError> {
    let parsed = if allow_incomplete {
        protocol::parse_turn_lines_allow_incomplete(lines)
    } else {
        protocol::parse_turn_lines(lines)?
    };
    let mut env = envelope.clone();
    if let Some(sid) = parsed.session_id.clone() {
        env.session_pointer = Some(sid);
    }
    if let Some(v) = parsed.version.clone() {
        env.observed_version = Some(v);
    }
    if !parsed.capabilities.is_empty() {
        env.capabilities = parsed.capabilities.clone();
    }
    if let Some(m) = parsed.model.clone() {
        env.selected_model = Some(m);
    }
    env.result = parsed.result.clone();
    env.usage = parsed.usage.clone();
    Ok(ExternalTurnOutcome {
        events: parsed.events,
        envelope: env,
        result: parsed.result,
        usage: parsed.usage,
    })
}

/// Label Claude-owned tool events for UI (display-only; no Grok hooks).
pub fn label_claude_owned_events(events: &mut [ExternalRuntimeTurnEvent]) {
    for e in events.iter_mut() {
        if let ExternalRuntimeTurnEvent::ToolCall { name, summary } = e {
            if !name.starts_with("Claude:") && !name.starts_with("claude:") {
                *name = format!("Claude:{name}");
            }
            let extra = "claude-owned display-only";
            *summary = Some(match summary.take() {
                Some(s) if s.contains(extra) => s,
                Some(s) => format!("{s}; {extra}"),
                None => extra.to_owned(),
            });
        }
    }
}

/// Cwd for spawn plans.
pub fn cwd_path(envelope: &ExternalRuntimeEnvelope) -> Option<PathBuf> {
    envelope.cwd.as_ref().map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persistent_requires_explicit_capability() {
        assert!(!persistent_mode_allowed(&[]));
        assert!(!persistent_mode_allowed(&["interrupt_receipt_v1".into()]));
        assert!(persistent_mode_allowed(&["streaming_input_v1".into()]));
    }

    #[test]
    fn labels_claude_tools_display_only() {
        let mut events = vec![ExternalRuntimeTurnEvent::ToolCall {
            name: "Bash".into(),
            summary: Some("id=t1".into()),
        }];
        label_claude_owned_events(&mut events);
        match &events[0] {
            ExternalRuntimeTurnEvent::ToolCall { name, summary } => {
                assert_eq!(name, "Claude:Bash");
                assert!(summary.as_ref().unwrap().contains("display-only"));
            }
            _ => panic!("expected tool call"),
        }
    }

    #[test]
    fn no_concurrent_turn_error_message() {
        // Documented contract string for UI.
        let err = TurnProcessError::io("persistent session refuses concurrent turns");
        assert!(err.to_string().contains("concurrent"));
    }
}
