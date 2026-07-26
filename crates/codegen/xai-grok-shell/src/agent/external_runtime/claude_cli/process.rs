//! Process lifecycle for Claude Agent CLI children.
//!
//! Uses `tokio::process::Command` with typed args, cwd, piped stdio,
//! `xai_tty_utils` process-group detach, bounded timeouts, line/output caps,
//! cancellation (SIGTERM → drain → SIGKILL), and reaping so session/leader
//! shutdown cannot orphan the child tree.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio_util::sync::CancellationToken;
use xai_tty_utils::{ProcessGroup, new_process_group};

use super::argv::{ClaudeCliArgvPlan, stream_json_user_prompt_line};
use super::env_scrub::apply_scrubbed_env;

/// Startup budget for first stdout activity / process spawn.
pub const DEFAULT_STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
/// Idle timeout between NDJSON lines during a turn.
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(120);
/// Hard turn timeout.
pub const DEFAULT_TURN_TIMEOUT: Duration = Duration::from_secs(600);
/// Grace after SIGTERM before SIGKILL.
pub const DEFAULT_SHUTDOWN_GRACE: Duration = Duration::from_secs(3);
/// Max bytes for a single NDJSON line.
pub const DEFAULT_MAX_LINE_BYTES: usize = 2 * 1024 * 1024;
/// Max total stdout bytes for a turn.
pub const DEFAULT_MAX_STDOUT_BYTES: usize = 32 * 1024 * 1024;
/// Max total stderr bytes retained (bounded).
pub const DEFAULT_MAX_STDERR_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone)]
pub struct ProcessLimits {
    pub startup: Duration,
    pub idle: Duration,
    pub turn: Duration,
    pub shutdown_grace: Duration,
    pub max_line_bytes: usize,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
}

impl Default for ProcessLimits {
    fn default() -> Self {
        Self {
            startup: DEFAULT_STARTUP_TIMEOUT,
            idle: DEFAULT_IDLE_TIMEOUT,
            turn: DEFAULT_TURN_TIMEOUT,
            shutdown_grace: DEFAULT_SHUTDOWN_GRACE,
            max_line_bytes: DEFAULT_MAX_LINE_BYTES,
            max_stdout_bytes: DEFAULT_MAX_STDOUT_BYTES,
            max_stderr_bytes: DEFAULT_MAX_STDERR_BYTES,
        }
    }
}

#[derive(Debug)]
pub enum ProbeError {
    Timeout,
    Spawn(String),
    Io(String),
    /// Process-group create/attach failed; child was terminated.
    ProcessGroup(String),
}

#[derive(Debug, Clone)]
pub struct ProbeCommandResult {
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
    pub exit_code: Option<i32>,
}

/// Run a short probe command (version / auth status) with timeout and caps.
pub async fn run_probe_command(
    executable: &Path,
    args: &[&str],
    timeout: Duration,
    output_cap: usize,
) -> Result<ProbeCommandResult, ProbeError> {
    let mut cmd = Command::new(executable);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    apply_scrubbed_env(&mut cmd, &[]);
    new_process_group(&mut cmd);

    let mut child = cmd.spawn().map_err(|e| ProbeError::Spawn(e.to_string()))?;
    let group = match ProcessGroup::new() {
        Ok(mut g) => {
            if let Err(e) = g.attach(&child) {
                let _ = child.start_kill();
                let _ = child.wait().await;
                return Err(ProbeError::ProcessGroup(format!("attach: {e}")));
            }
            g
        }
        Err(e) => {
            let _ = child.start_kill();
            let _ = child.wait().await;
            return Err(ProbeError::ProcessGroup(format!("create: {e}")));
        }
    };

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let collect = async {
        let mut out_buf = Vec::new();
        let mut err_buf = Vec::new();
        let out_task = async {
            if let Some(mut r) = stdout {
                let mut buf = vec![0u8; 4096];
                loop {
                    match r.read(&mut buf).await {
                        Ok(0) => break,
                        Ok(n) => {
                            if out_buf.len() < output_cap {
                                let take = n.min(output_cap - out_buf.len());
                                out_buf.extend_from_slice(&buf[..take]);
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
            out_buf
        };
        let err_task = async {
            if let Some(mut r) = stderr {
                let mut buf = vec![0u8; 4096];
                loop {
                    match r.read(&mut buf).await {
                        Ok(0) => break,
                        Ok(n) => {
                            if err_buf.len() < output_cap {
                                let take = n.min(output_cap - err_buf.len());
                                err_buf.extend_from_slice(&buf[..take]);
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
            err_buf
        };
        let (o, e) = tokio::join!(out_task, err_task);
        let status = child.wait().await;
        (o, e, status)
    };

    match tokio::time::timeout(timeout, collect).await {
        Ok((out, err, status)) => {
            // Reap group after leader wait (drop without killpg on reusable pid).
            drop(group);
            let status = status.map_err(|e| ProbeError::Io(e.to_string()))?;
            Ok(ProbeCommandResult {
                stdout: String::from_utf8_lossy(&out).into_owned(),
                stderr: String::from_utf8_lossy(&err).into_owned(),
                success: status.success(),
                exit_code: status.code(),
            })
        }
        Err(_) => {
            terminate_child_tree(&mut child, Some(group), Duration::from_millis(500)).await;
            Err(ProbeError::Timeout)
        }
    }
}

/// Outcome of a one-shot turn process.
#[derive(Debug)]
pub struct TurnProcessOutcome {
    pub lines: Vec<String>,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub cancelled: bool,
    /// Unix signal if terminated by signal (e.g. 15 → exit 143).
    pub exit_signal: Option<i32>,
}

#[derive(Debug)]
pub enum TurnProcessError {
    Spawn(String),
    StartupTimeout,
    IdleTimeout,
    TurnTimeout,
    LineTooLarge { bytes: usize, max: usize },
    OutputTooLarge { bytes: usize, max: usize },
    StderrFlood { bytes: usize, max: usize },
    InvalidUtf8,
    Io(String),
    Cancelled,
}

impl std::fmt::Display for TurnProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn(m) => write!(f, "failed to spawn Claude CLI: {m}"),
            Self::StartupTimeout => write!(f, "Claude CLI startup timed out"),
            Self::IdleTimeout => write!(f, "Claude CLI idle timeout (no output)"),
            Self::TurnTimeout => write!(f, "Claude CLI turn timed out"),
            Self::LineTooLarge { bytes, max } => {
                write!(
                    f,
                    "Claude CLI NDJSON line too large ({bytes} > {max} bytes)"
                )
            }
            Self::OutputTooLarge { bytes, max } => {
                write!(f, "Claude CLI stdout exceeded cap ({bytes} > {max} bytes)")
            }
            Self::StderrFlood { bytes, max } => {
                write!(f, "Claude CLI stderr exceeded cap ({bytes} > {max} bytes)")
            }
            Self::InvalidUtf8 => write!(f, "Claude CLI produced invalid UTF-8 on stdout"),
            Self::Io(m) => write!(f, "Claude CLI I/O error: {m}"),
            Self::Cancelled => write!(f, "Claude CLI turn cancelled"),
        }
    }
}

impl std::error::Error for TurnProcessError {}

/// Run one Claude CLI turn: spawn, write prompt, read NDJSON lines, reap.
pub async fn run_turn_process(
    plan: &ClaudeCliArgvPlan,
    limits: &ProcessLimits,
    cancel: CancellationToken,
) -> Result<TurnProcessOutcome, TurnProcessError> {
    let mut cmd = Command::new(&plan.program);
    cmd.args(&plan.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
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
            g
        }
        Err(e) => {
            let _ = child.start_kill();
            let _ = child.wait().await;
            return Err(TurnProcessError::Spawn(format!(
                "process group create failed: {e}"
            )));
        }
    };

    // Write prompt; close stdin for one-shot (default). Persistent mode keeps
    // stdin open (handled by persistent.rs) — one-shot always closes.
    if let Some(mut stdin) = child.stdin.take() {
        if plan.write_stream_json_prompt {
            let line = stream_json_user_prompt_line(&plan.prompt);
            if let Err(e) = stdin.write_all(line.as_bytes()).await {
                terminate_child_tree(&mut child, Some(group), limits.shutdown_grace).await;
                return Err(TurnProcessError::Io(format!("stdin write: {e}")));
            }
        }
        if plan.close_stdin_after_prompt {
            drop(stdin);
        } else {
            // Caller owns persistent stdin; for one-shot path we still drop
            // if somehow persistent flags leak here.
            drop(stdin);
        }
    }

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| TurnProcessError::Io("stdout missing".into()))?;
    let stderr = child.stderr.take();

    let mut reader = BufReader::with_capacity(64 * 1024, stdout);
    let mut lines: Vec<String> = Vec::new();
    let mut total_stdout = 0usize;
    let mut saw_first_line = false;

    // Drain stderr concurrently with a cap.
    let stderr_task = tokio::spawn({
        let max = limits.max_stderr_bytes;
        async move {
            let mut acc = Vec::new();
            if let Some(mut r) = stderr {
                let mut buf = vec![0u8; 4096];
                loop {
                    match r.read(&mut buf).await {
                        Ok(0) => break,
                        Ok(n) => {
                            if acc.len() < max {
                                let take = n.min(max - acc.len());
                                acc.extend_from_slice(&buf[..take]);
                            } else {
                                // Keep reading to avoid backpressure deadlock,
                                // but do not retain more.
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
            acc
        }
    });

    let turn_deadline = tokio::time::Instant::now() + limits.turn;
    let startup_deadline = tokio::time::Instant::now() + limits.startup;

    loop {
        if cancel.is_cancelled() {
            terminate_child_tree(&mut child, Some(group), limits.shutdown_grace).await;
            let _ = stderr_task.await;
            return Ok(TurnProcessOutcome {
                lines,
                stderr: String::new(),
                exit_code: Some(143),
                cancelled: true,
                exit_signal: Some(15),
            });
        }

        let now = tokio::time::Instant::now();
        if now >= turn_deadline {
            terminate_child_tree(&mut child, Some(group), limits.shutdown_grace).await;
            let _ = stderr_task.await;
            return Err(TurnProcessError::TurnTimeout);
        }
        if !saw_first_line && now >= startup_deadline {
            terminate_child_tree(&mut child, Some(group), limits.shutdown_grace).await;
            let _ = stderr_task.await;
            return Err(TurnProcessError::StartupTimeout);
        }

        let idle_budget = if saw_first_line {
            limits.idle
        } else {
            startup_deadline.saturating_duration_since(now)
        };

        let mut line_buf: Vec<u8> = Vec::new();
        let read_result = tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                terminate_child_tree(&mut child, Some(group), limits.shutdown_grace).await;
                let err = stderr_task.await.unwrap_or_default();
                return Ok(TurnProcessOutcome {
                    lines,
                    stderr: String::from_utf8_lossy(&err).into_owned(),
                    exit_code: Some(143),
                    cancelled: true,
                    exit_signal: Some(15),
                });
            }
            _ = tokio::time::sleep(idle_budget) => {
                Err(if saw_first_line {
                    TurnProcessError::IdleTimeout
                } else {
                    TurnProcessError::StartupTimeout
                })
            }
            res = read_line_capped(&mut reader, &mut line_buf, limits.max_line_bytes) => res,
        };

        match read_result {
            Ok(0) => break, // EOF
            Ok(_) => {
                saw_first_line = true;
                total_stdout = total_stdout.saturating_add(line_buf.len());
                if total_stdout > limits.max_stdout_bytes {
                    terminate_child_tree(&mut child, Some(group), limits.shutdown_grace).await;
                    let _ = stderr_task.await;
                    return Err(TurnProcessError::OutputTooLarge {
                        bytes: total_stdout,
                        max: limits.max_stdout_bytes,
                    });
                }
                // Trim trailing newline for parser; keep empty lines out.
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
                        terminate_child_tree(&mut child, Some(group), limits.shutdown_grace).await;
                        let _ = stderr_task.await;
                        return Err(TurnProcessError::InvalidUtf8);
                    }
                };
                lines.push(line);
            }
            Err(e) => {
                terminate_child_tree(&mut child, Some(group), limits.shutdown_grace).await;
                let _ = stderr_task.await;
                return Err(e);
            }
        }
    }

    let status = match tokio::time::timeout(limits.shutdown_grace, child.wait()).await {
        Ok(Ok(st)) => st,
        Ok(Err(e)) => {
            terminate_child_tree(&mut child, None, limits.shutdown_grace).await;
            return Err(TurnProcessError::Io(e.to_string()));
        }
        Err(_) => {
            terminate_child_tree(&mut child, Some(group), limits.shutdown_grace).await;
            let err = stderr_task.await.unwrap_or_default();
            return Ok(TurnProcessOutcome {
                lines,
                stderr: String::from_utf8_lossy(&err).into_owned(),
                exit_code: Some(143),
                cancelled: cancel.is_cancelled(),
                exit_signal: Some(15),
            });
        }
    };

    // Leader reaped; drop group without killpg on reusable pid.
    drop(group);

    let err = stderr_task.await.unwrap_or_default();
    if err.len() > limits.max_stderr_bytes {
        // Cap was enforced during read; still surface flood if exactly at cap
        // and process kept writing (we stopped retaining). Not a hard error
        // unless we never got a result — leave as retained stderr only.
    }

    let code = status.code();
    #[cfg(unix)]
    let exit_signal = {
        use std::os::unix::process::ExitStatusExt;
        status.signal()
    };
    #[cfg(not(unix))]
    let exit_signal: Option<i32> = None;

    // Exit 143 (128+15 SIGTERM) or cancelled → Cancelled outcome.
    let cancelled = cancel.is_cancelled() || code == Some(143) || exit_signal == Some(15);

    Ok(TurnProcessOutcome {
        lines,
        stderr: String::from_utf8_lossy(&err).into_owned(),
        exit_code: code.or_else(|| exit_signal.map(|s| 128 + s)),
        cancelled,
        exit_signal,
    })
}

async fn read_line_capped<R: tokio::io::AsyncBufRead + Unpin>(
    reader: &mut R,
    buf: &mut Vec<u8>,
    max: usize,
) -> Result<usize, TurnProcessError> {
    buf.clear();
    loop {
        let mut byte = [0u8; 1];
        let n = reader
            .read(&mut byte)
            .await
            .map_err(|e| TurnProcessError::Io(e.to_string()))?;
        if n == 0 {
            return Ok(buf.len());
        }
        if buf.len() >= max {
            return Err(TurnProcessError::LineTooLarge {
                bytes: buf.len() + 1,
                max,
            });
        }
        buf.push(byte[0]);
        if byte[0] == b'\n' {
            return Ok(buf.len());
        }
    }
}

async fn terminate_child_tree(child: &mut Child, group: Option<ProcessGroup>, grace: Duration) {
    // SIGTERM the process group first so grandchildren exit cleanly.
    if let Some(ref g) = group {
        let _ = g.terminate();
    }
    // Also signal the leader via tokio (maps to SIGKILL on kill(); we wait first).
    // On Unix, start_kill is SIGKILL — only use after grace.
    match tokio::time::timeout(grace, child.wait()).await {
        Ok(_) => {
            // Leader reaped; drop group without killpg on a reusable pid.
            drop(group);
            return;
        }
        Err(_) => {
            // Escalate: SIGKILL process group + leader.
            if let Some(ref g) = group {
                let _ = g.kill();
            }
            let _ = child.start_kill();
            let _ = tokio::time::timeout(grace, child.wait()).await;
            drop(group);
        }
    }
}

/// Inspect ProcessGroup kill semantics — used by cancel tests.
pub fn process_group_available() -> bool {
    ProcessGroup::new().is_ok()
}
