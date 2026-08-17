//! Selectable external ACP backend that launches `workbench agent stdio`.
//!
//! Architecture (monorepo `docs/architecture/grok-build-terminal-integration.md`):
//!
//! ```text
//! AgentBackend
//! |-- GrokShellBackend        # existing spawn_grok_shell (default)
//! `-- WorkbenchBackend        # this module: launches workbench agent stdio
//! ```
//!
//! Selection (all required for Workbench path):
//! - `WORKBENCH_TERMINAL_BACKEND=1` **or** `GROK_AGENT_BACKEND=workbench`
//! - absolute path via `WORKBENCH_EXECUTABLE` / `--workbench-executable`
//!
//! Child launch contract matches monorepo `workbench-terminal-backend`:
//! - argv: `<exe> agent stdio`
//! - cwd: workspace root
//! - env: `WORKBENCH_TERMINAL_BACKEND=1`

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::thread;

use anyhow::{Context, Result};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
use tokio_util::sync::CancellationToken;

use agent_client_protocol as acp;
use xai_acp_lib::{AcpGatewayReceiver, AcpGatewaySender, LineBufferedRead, acp_channels};
use xai_grok_shell::auth::{AuthManager, GrokComConfig};
use xai_grok_shell::util::grok_home::grok_home;

use super::spawn::SpawnedAgent;

/// Documented CLI subcommand the terminal launches (excluding the executable).
pub const WORKBENCH_AGENT_STDIO_ARGS: &[&str] = &["agent", "stdio"];

/// Env var: when `1`/`true`/`yes`, select the Workbench ACP backend (with an
/// absolute executable path).
pub const ENV_WORKBENCH_TERMINAL_BACKEND: &str = "WORKBENCH_TERMINAL_BACKEND";

/// Env var: set to `workbench` to select the Workbench ACP backend.
pub const ENV_GROK_AGENT_BACKEND: &str = "GROK_AGENT_BACKEND";

/// Env var / CLI: absolute path to the `workbench` CLI binary.
pub const ENV_WORKBENCH_EXECUTABLE: &str = "WORKBENCH_EXECUTABLE";

/// Errors from constructing or validating a Workbench backend launch plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkbenchBackendError {
    EmptyExecutable,
    RelativeExecutable,
    ParentTraversal,
    EmptyWorkspace,
    RelativeWorkspace,
    WorkspaceParentTraversal,
    InvalidLaunch(&'static str),
}

impl std::fmt::Display for WorkbenchBackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyExecutable => write!(f, "workbench executable path is empty"),
            Self::RelativeExecutable => {
                write!(f, "workbench executable path is not absolute")
            }
            Self::ParentTraversal => {
                write!(
                    f,
                    "workbench executable path must not contain parent traversal"
                )
            }
            Self::EmptyWorkspace => write!(f, "workspace path is empty"),
            Self::RelativeWorkspace => write!(f, "workspace path is not absolute"),
            Self::WorkspaceParentTraversal => {
                write!(f, "workspace path must not contain parent traversal")
            }
            Self::InvalidLaunch(msg) => {
                write!(f, "workbench agent stdio launch is invalid: {msg}")
            }
        }
    }
}

impl std::error::Error for WorkbenchBackendError {}

/// Which ACP agent backend the pager should use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentBackendKind {
    /// In-process Grok shell (upstream default).
    GrokShell,
    /// External `workbench agent stdio` process.
    Workbench,
}

/// Launch configuration for the Workbench ACP agent stdio bridge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkbenchBackend {
    executable: PathBuf,
    workspace: PathBuf,
}

impl WorkbenchBackend {
    /// Builds a backend that will launch `executable agent stdio`.
    pub fn new(
        executable: impl Into<PathBuf>,
        workspace: impl Into<PathBuf>,
    ) -> Result<Self, WorkbenchBackendError> {
        let executable = executable.into();
        let workspace = workspace.into();
        validate_absolute_exe(&executable)?;
        validate_absolute_workspace(&workspace)?;
        Ok(Self {
            executable,
            workspace,
        })
    }

    #[must_use]
    pub fn executable(&self) -> &Path {
        &self.executable
    }

    #[must_use]
    pub fn workspace(&self) -> &Path {
        &self.workspace
    }

    /// Argv for the ACP agent stdio child (excluding the executable).
    #[must_use]
    pub fn agent_stdio_args() -> Vec<OsString> {
        WORKBENCH_AGENT_STDIO_ARGS
            .iter()
            .map(OsString::from)
            .collect()
    }

    /// Builds a std `Command` that launches `workbench agent stdio` (for
    /// inspection / offline tests). Prefer [`spawn_workbench_agent`] at runtime.
    #[must_use]
    pub fn command(&self) -> std::process::Command {
        let mut command = std::process::Command::new(&self.executable);
        command
            .args(WORKBENCH_AGENT_STDIO_ARGS)
            .current_dir(&self.workspace)
            .env(ENV_WORKBENCH_TERMINAL_BACKEND, "1");
        command
    }

    /// Validates the launch plan without spawning (offline unit path).
    pub fn validate_launch_plan(&self) -> Result<(), WorkbenchBackendError> {
        let args = Self::agent_stdio_args();
        if args.len() != 2 {
            return Err(WorkbenchBackendError::InvalidLaunch(
                "expected exactly two args: agent stdio",
            ));
        }
        if args[0] != "agent" || args[1] != "stdio" {
            return Err(WorkbenchBackendError::InvalidLaunch(
                "args must be agent stdio",
            ));
        }
        let _ = (self.executable(), self.workspace());
        Ok(())
    }
}

fn validate_absolute_exe(path: &Path) -> Result<(), WorkbenchBackendError> {
    if path.as_os_str().is_empty() {
        return Err(WorkbenchBackendError::EmptyExecutable);
    }
    if !path.is_absolute() {
        return Err(WorkbenchBackendError::RelativeExecutable);
    }
    if path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(WorkbenchBackendError::ParentTraversal);
    }
    Ok(())
}

fn validate_absolute_workspace(path: &Path) -> Result<(), WorkbenchBackendError> {
    if path.as_os_str().is_empty() {
        return Err(WorkbenchBackendError::EmptyWorkspace);
    }
    if !path.is_absolute() {
        return Err(WorkbenchBackendError::RelativeWorkspace);
    }
    if path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(WorkbenchBackendError::WorkspaceParentTraversal);
    }
    Ok(())
}

/// True when env/CLI request the Workbench agent backend (path may still be missing).
#[must_use]
pub fn workbench_backend_requested(cli_executable: Option<&Path>) -> bool {
    if truthy_env(ENV_WORKBENCH_TERMINAL_BACKEND) {
        return true;
    }
    if env_equals_ignore_case(ENV_GROK_AGENT_BACKEND, "workbench") {
        return true;
    }
    // Explicit absolute path alone is enough when provided via CLI/env executable
    // *and* either selection env is set — path alone does NOT select Workbench,
    // matching the monorepo contract. Keep this helper env-driven only; the
    // executable argument is reserved for resolve().
    let _ = cli_executable;
    false
}

/// Resolve the agent backend from CLI + environment.
///
/// Returns `GrokShell` when Workbench is not selected. When selected without a
/// usable absolute executable, returns an error (fail closed).
pub fn resolve_agent_backend(
    cli_executable: Option<&Path>,
    workspace: Option<&Path>,
) -> Result<(AgentBackendKind, Option<WorkbenchBackend>), WorkbenchBackendError> {
    if !workbench_backend_requested(cli_executable) {
        return Ok((AgentBackendKind::GrokShell, None));
    }

    let exe = cli_executable
        .map(Path::to_path_buf)
        .or_else(|| std::env::var_os(ENV_WORKBENCH_EXECUTABLE).map(PathBuf::from));

    let Some(exe) = exe else {
        return Err(WorkbenchBackendError::EmptyExecutable);
    };

    let workspace = match workspace {
        Some(p) => p.to_path_buf(),
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
    };

    // Ensure workspace is absolute for the launch contract.
    let workspace = if workspace.is_absolute() {
        workspace
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("/"))
            .join(workspace)
    };

    let backend = WorkbenchBackend::new(exe, workspace)?;
    backend.validate_launch_plan()?;
    Ok((AgentBackendKind::Workbench, Some(backend)))
}

/// Spawn `workbench agent stdio` and bridge NDJSON ACP into an [`SpawnedAgent`].
///
/// Uses the same `ClientSideConnection` + gateway pattern as the leader bridge,
/// with child process stdin/stdout as the transport.
pub async fn spawn_workbench_agent(
    plan: &WorkbenchBackend,
    cancel: &CancellationToken,
) -> Result<SpawnedAgent> {
    plan.validate_launch_plan()
        .map_err(|e| anyhow::anyhow!(e))?;

    let agent_cancel = cancel.child_token();
    let (acp_client, acp_agent) = acp_channels();

    let executable = plan.executable().to_path_buf();
    let workspace = plan.workspace().to_path_buf();
    let thread_cancel = agent_cancel.clone();

    let handle = thread::Builder::new()
        .name("acp-workbench-agent".into())
        .spawn(move || -> Result<()> {
            let mut runtime_builder = tokio::runtime::Builder::new_current_thread();
            runtime_builder.enable_all();
            let rt = xai_tty_utils::runtime::apply_blocking_pool(&mut runtime_builder).build()?;
            let local = tokio::task::LocalSet::new();
            local.block_on(&rt, async move {
                let mut child = tokio::process::Command::new(&executable)
                    .args(WORKBENCH_AGENT_STDIO_ARGS)
                    .current_dir(&workspace)
                    .env(ENV_WORKBENCH_TERMINAL_BACKEND, "1")
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::inherit())
                    .kill_on_drop(true)
                    .spawn()
                    .with_context(|| {
                        format!(
                            "failed to spawn workbench agent stdio at {}",
                            executable.display()
                        )
                    })?;

                let stdin = child
                    .stdin
                    .take()
                    .context("workbench child missing stdin")?;
                let stdout = child
                    .stdout
                    .take()
                    .context("workbench child missing stdout")?;

                let gw_tx = AcpGatewaySender::new(acp_agent.tx).with_tracing(true);
                let incoming = LineBufferedRead::spawn_local(stdout.compat());
                let (conn, handle_io) =
                    acp::ClientSideConnection::new(gw_tx, stdin.compat_write(), incoming, |fut| {
                        tokio::task::spawn_local(fut);
                    });
                let gw_rx = AcpGatewayReceiver::new(acp_agent.rx, conn).with_tracing(true);
                tokio::task::spawn_local(handle_io);
                tokio::task::spawn_local(gw_rx.run());
                tokio::task::yield_now().await;

                tokio::select! {
                    biased;
                    _ = thread_cancel.cancelled() => {
                        tracing::info!("workbench agent cancelled; killing child");
                        let _ = child.kill().await;
                    }
                    status = child.wait() => {
                        match status {
                            Ok(s) => tracing::warn!(?s, "workbench agent process exited"),
                            Err(e) => tracing::error!(error = %e, "workbench agent wait failed"),
                        }
                        thread_cancel.cancel();
                    }
                }
                Ok(())
            })
        })
        .context("failed to spawn workbench agent worker thread")?;

    // Workbench owns provider auth; pager only needs a local AuthManager for
    // optional voice channels (same non-refreshing pattern as leader mode).
    let auth_manager =
        std::sync::Arc::new(AuthManager::new(&grok_home(), GrokComConfig::default()));

    Ok(SpawnedAgent {
        _thread_handle: handle,
        channel: acp_client,
        cancel: agent_cancel,
        auth_manager,
    })
}

fn truthy_env(key: &str) -> bool {
    match std::env::var(key) {
        Ok(v) => {
            let v = v.trim();
            v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes")
        }
        Err(_) => false,
    }
}

fn env_equals_ignore_case(key: &str, expected: &str) -> bool {
    std::env::var(key)
        .map(|v| v.trim().eq_ignore_ascii_case(expected))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    /// Serialise env-mutating tests (process-global env).
    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn rejects_relative_and_empty_executable() {
        assert_eq!(
            WorkbenchBackend::new("", "/tmp/ws").expect_err("empty"),
            WorkbenchBackendError::EmptyExecutable
        );
        assert_eq!(
            WorkbenchBackend::new("workbench", "/tmp/ws").expect_err("relative"),
            WorkbenchBackendError::RelativeExecutable
        );
        assert_eq!(
            WorkbenchBackend::new("/opt/bin/../workbench", "/tmp/ws").expect_err("parent"),
            WorkbenchBackendError::ParentTraversal
        );
    }

    #[test]
    fn plans_agent_stdio_launch_argv_cwd_and_env() {
        let backend = WorkbenchBackend::new(
            PathBuf::from("/usr/local/bin/workbench"),
            PathBuf::from("/workspace/repo"),
        )
        .expect("absolute paths");
        backend.validate_launch_plan().expect("plan");
        assert_eq!(
            WorkbenchBackend::agent_stdio_args(),
            vec![OsString::from("agent"), OsString::from("stdio")]
        );
        let program = backend.command();
        assert_eq!(program.get_program(), "/usr/local/bin/workbench");
        let args: Vec<_> = program.get_args().collect();
        assert_eq!(args, ["agent", "stdio"]);
        assert_eq!(
            program.get_current_dir().expect("cwd"),
            Path::new("/workspace/repo")
        );
        let env: Vec<(String, String)> = program
            .get_envs()
            .filter_map(|(k, v)| Some((k.to_str()?.to_owned(), v?.to_str()?.to_owned())))
            .collect();
        assert!(
            env.iter()
                .any(|(k, v)| k == ENV_WORKBENCH_TERMINAL_BACKEND && v == "1"),
            "child must set WORKBENCH_TERMINAL_BACKEND=1; env={env:?}"
        );
    }

    #[test]
    fn resolve_defaults_to_grok_shell_without_selection() {
        let _g = env_lock();
        // Clear selection env for this process (best-effort for hermetic unit test).
        // SAFETY: serialised by env_lock; tests in this module only.
        unsafe {
            std::env::remove_var(ENV_WORKBENCH_TERMINAL_BACKEND);
            std::env::remove_var(ENV_GROK_AGENT_BACKEND);
            std::env::remove_var(ENV_WORKBENCH_EXECUTABLE);
        }
        let (kind, plan) = resolve_agent_backend(None, Some(Path::new("/tmp/ws"))).unwrap();
        assert_eq!(kind, AgentBackendKind::GrokShell);
        assert!(plan.is_none());
    }

    #[test]
    fn resolve_workbench_via_env_and_cli_path() {
        let _g = env_lock();
        unsafe {
            std::env::set_var(ENV_WORKBENCH_TERMINAL_BACKEND, "1");
            std::env::remove_var(ENV_WORKBENCH_EXECUTABLE);
        }
        let exe = Path::new("/usr/local/bin/workbench");
        let (kind, plan) =
            resolve_agent_backend(Some(exe), Some(Path::new("/workspace"))).expect("resolve");
        assert_eq!(kind, AgentBackendKind::Workbench);
        let plan = plan.expect("plan");
        assert_eq!(plan.executable(), exe);
        assert_eq!(plan.workspace(), Path::new("/workspace"));
        unsafe {
            std::env::remove_var(ENV_WORKBENCH_TERMINAL_BACKEND);
        }
    }

    #[test]
    fn resolve_workbench_via_grok_agent_backend_and_executable_env() {
        let _g = env_lock();
        unsafe {
            std::env::remove_var(ENV_WORKBENCH_TERMINAL_BACKEND);
            std::env::set_var(ENV_GROK_AGENT_BACKEND, "workbench");
            std::env::set_var(ENV_WORKBENCH_EXECUTABLE, "/opt/workbench");
        }
        let (kind, plan) = resolve_agent_backend(None, Some(Path::new("/ws"))).expect("resolve");
        assert_eq!(kind, AgentBackendKind::Workbench);
        assert_eq!(
            plan.expect("plan").executable(),
            Path::new("/opt/workbench")
        );
        unsafe {
            std::env::remove_var(ENV_GROK_AGENT_BACKEND);
            std::env::remove_var(ENV_WORKBENCH_EXECUTABLE);
        }
    }

    #[test]
    fn resolve_workbench_without_executable_fails_closed() {
        let _g = env_lock();
        unsafe {
            std::env::set_var(ENV_GROK_AGENT_BACKEND, "workbench");
            std::env::remove_var(ENV_WORKBENCH_EXECUTABLE);
        }
        let err = resolve_agent_backend(None, Some(Path::new("/ws"))).expect_err("no exe");
        assert_eq!(err, WorkbenchBackendError::EmptyExecutable);
        unsafe {
            std::env::remove_var(ENV_GROK_AGENT_BACKEND);
        }
    }

    #[test]
    fn resolve_rejects_relative_executable_when_selected() {
        let _g = env_lock();
        unsafe {
            std::env::set_var(ENV_WORKBENCH_TERMINAL_BACKEND, "1");
        }
        let err = resolve_agent_backend(Some(Path::new("workbench")), Some(Path::new("/ws")))
            .expect_err("relative");
        assert_eq!(err, WorkbenchBackendError::RelativeExecutable);
        unsafe {
            std::env::remove_var(ENV_WORKBENCH_TERMINAL_BACKEND);
        }
    }

    /// Live smoke: only runs when `WORKBENCH_LIVE_TEST=1` and a real binary is set.
    #[tokio::test]
    #[ignore = "requires live workbench binary; set WORKBENCH_LIVE_TEST=1"]
    async fn live_spawn_workbench_initialize_smoke() {
        if std::env::var("WORKBENCH_LIVE_TEST").ok().as_deref() != Some("1") {
            return;
        }
        let exe = std::env::var("WORKBENCH_EXECUTABLE")
            .expect("WORKBENCH_EXECUTABLE required for live test");
        let ws = std::env::current_dir().expect("cwd");
        let plan = WorkbenchBackend::new(exe, ws).expect("plan");
        let cancel = CancellationToken::new();
        let spawned = spawn_workbench_agent(&plan, &cancel).await.expect("spawn");
        // Soft check: channel is live (caller would run initialize).
        let _ = &spawned.channel;
        cancel.cancel();
    }
}
