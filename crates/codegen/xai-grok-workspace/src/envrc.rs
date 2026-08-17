//! Load environment variables from a directory's `.envrc`: `direnv export
//! json` when available, else bash with direnv stubs.
//!
//! Every evaluator wait is bounded because a blocked `.envrc` must never freeze
//! session creation. Evaluation runs on a dedicated thread, subprocess trees
//! are killed at the deadline, and incomplete output is discarded rather than
//! installing a truncated environment.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const ENVRC_LOAD_TIMEOUT: Duration = Duration::from_secs(10);
const ENVRC_TIMEOUT_ENV: &str = "GROK_ENVRC_TIMEOUT_SECS";
const MAX_TIMEOUT: Duration = Duration::from_secs(3600);
const POLL_INTERVAL: Duration = Duration::from_millis(25);
const PIPE_DRAIN_GRACE: Duration = Duration::from_millis(250);
const PIPE_DRAIN_CAP: Duration = Duration::from_secs(2);
const KILL_REAP_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_DRAIN_BYTES: usize = 4 * 1024 * 1024;

/// Prefix only; each run appends a nonce so no environment entry can forge it.
const OUTPUT_SENTINEL: &str = "__GROK_ENVRC_COMPLETE__";

/// Loader-side slack over the evaluator deadline.
pub const JOIN_SLACK: Duration = Duration::from_secs(10);

pub fn effective_timeout() -> Duration {
    timeout_from(std::env::var(ENVRC_TIMEOUT_ENV).ok().as_deref())
}

/// Total budget callers should allow an in-flight load.
pub fn loader_budget() -> Duration {
    effective_timeout() + JOIN_SLACK
}

/// A `.envrc` evaluation running outside the async executor.
pub struct EnvrcLoad {
    rx: Option<tokio::sync::oneshot::Receiver<HashMap<String, String>>>,
    deadline: tokio::time::Instant,
}

/// Start a trusted `.envrc` load on a dedicated thread. Untrusted loads resolve
/// immediately to an empty map without touching the repository file.
pub fn spawn_envrc_load(cwd: std::path::PathBuf, trusted: bool) -> EnvrcLoad {
    let deadline = tokio::time::Instant::now() + loader_budget();
    if !trusted {
        return EnvrcLoad { rx: None, deadline };
    }
    let (tx, rx) = tokio::sync::oneshot::channel();
    let spawned = std::thread::Builder::new()
        .name("envrc-load".into())
        .spawn(move || {
            let _ = tx.send(load_envrc_or_empty(&cwd));
        });
    match spawned {
        Ok(_) => EnvrcLoad {
            rx: Some(rx),
            deadline,
        },
        Err(error) => {
            tracing::warn!(?error, "failed to spawn envrc loader thread");
            EnvrcLoad { rx: None, deadline }
        }
    }
}

impl EnvrcLoad {
    pub async fn join(self) -> HashMap<String, String> {
        let Some(rx) = self.rx else {
            return HashMap::new();
        };
        match tokio::time::timeout_at(self.deadline, rx).await {
            Ok(Ok(env)) => env,
            Ok(Err(_)) => {
                tracing::warn!("envrc loader thread died without a result");
                HashMap::new()
            }
            Err(_) => {
                tracing::warn!("envrc loader exceeded its budget; continuing without .envrc");
                HashMap::new()
            }
        }
    }
}

fn timeout_from(overriding: Option<&str>) -> Duration {
    let trimmed = overriding.map(str::trim).unwrap_or_default();
    if trimmed.is_empty() {
        return ENVRC_LOAD_TIMEOUT;
    }
    match trimmed.parse::<u64>() {
        Ok(secs) => {
            let capped = Duration::from_secs(secs).min(MAX_TIMEOUT);
            if capped.as_secs() < secs {
                tracing::warn!(secs, "clamping {ENVRC_TIMEOUT_ENV} to one hour");
            }
            capped
        }
        Err(_) => {
            tracing::warn!(value = trimmed, "ignoring unparseable {ENVRC_TIMEOUT_ENV}");
            ENVRC_LOAD_TIMEOUT
        }
    }
}

/// Stub implementations of common direnv helper functions.
const DIRENV_STUBS: &str = r#"
# Stub direnv helper functions
source_up_if_exists() { :; }
source_up() { :; }
source_env_if_exists() {
    if [ -f "$1" ]; then
        . "$1"
    fi
}
source_env() {
    if [ -f "$1" ]; then
        . "$1"
    fi
}
PATH_add() {
    export PATH="$PWD/$1:$PATH"
}
path_add() {
    PATH_add "$@"
}
layout() { :; }
use() { :; }
watch_file() { :; }
"#;

pub fn load_envrc(dir: &Path) -> Option<HashMap<String, String>> {
    load_envrc_with_timeout(dir, effective_timeout())
}

pub fn load_envrc_or_empty(dir: &Path) -> HashMap<String, String> {
    load_envrc(dir).unwrap_or_default()
}

/// Synchronous compatibility API. The trust check happens before any file
/// metadata or subprocess operation.
pub fn load_envrc_or_empty_when_trusted(dir: &Path, trusted: bool) -> HashMap<String, String> {
    if trusted {
        load_envrc_or_empty(dir)
    } else {
        HashMap::new()
    }
}

fn load_envrc_with_timeout(dir: &Path, timeout: Duration) -> Option<HashMap<String, String>> {
    if timeout.is_zero() {
        tracing::info!(".envrc evaluation disabled by zero {ENVRC_TIMEOUT_ENV}");
        return None;
    }
    let deadline = Instant::now() + timeout;
    let envrc_path = dir.join(".envrc");
    match std::fs::metadata(&envrc_path) {
        Err(_) => {
            tracing::debug!(?dir, ".envrc not found");
            return None;
        }
        Ok(metadata) if !metadata.is_file() => {
            tracing::warn!(?envrc_path, "refusing to evaluate non-regular .envrc");
            return None;
        }
        Ok(_) => {}
    }
    if Instant::now() >= deadline {
        tracing::warn!(?envrc_path, ".envrc stat consumed the evaluation budget");
        return None;
    }

    match try_direnv_export(dir, deadline) {
        DirenvExport::Env(env) => Some(env),
        DirenvExport::TimedOut | DirenvExport::SideEffectsRan => None,
        DirenvExport::Unavailable => load_envrc_via_bash(dir, deadline),
    }
}

enum DirenvExport {
    Env(HashMap<String, String>),
    TimedOut,
    /// Evaluation ran but capture was unusable, so side effects must not run a
    /// second time through the bash fallback.
    SideEffectsRan,
    Unavailable,
}

fn try_direnv_export(dir: &Path, deadline: Instant) -> DirenvExport {
    let mut cmd = Command::new("direnv");
    cmd.args(["export", "json"]).current_dir(dir);
    let output = match run_with_deadline(cmd, deadline, "direnv") {
        RunOutcome::Completed {
            output,
            truncated: false,
        } => output,
        RunOutcome::Completed {
            truncated: true, ..
        } => {
            tracing::warn!(?dir, "direnv output capture incomplete; skipping .envrc");
            return DirenvExport::SideEffectsRan;
        }
        RunOutcome::TimedOut => return DirenvExport::TimedOut,
        RunOutcome::Failed => return DirenvExport::Unavailable,
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.contains("not allowed") {
            tracing::debug!(?dir, %stderr, "direnv export failed");
        }
        return DirenvExport::Unavailable;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return DirenvExport::Unavailable;
    }

    match serde_json::from_str::<HashMap<String, serde_json::Value>>(&stdout) {
        Ok(json) => {
            let env: HashMap<String, String> = json
                .into_iter()
                .filter_map(|(key, value)| value.as_str().map(|value| (key, value.to_owned())))
                .collect();
            if env.is_empty() {
                DirenvExport::Unavailable
            } else {
                tracing::info!(?dir, count = env.len(), "Loaded environment via direnv");
                DirenvExport::Env(env)
            }
        }
        Err(error) => {
            tracing::warn!(?dir, ?error, "Failed to parse direnv JSON output");
            DirenvExport::SideEffectsRan
        }
    }
}

fn load_envrc_via_bash(dir: &Path, deadline: Instant) -> Option<HashMap<String, String>> {
    if Instant::now() >= deadline {
        return None;
    }
    let envrc_path = dir.join(".envrc");
    let sentinel = format!("{OUTPUT_SENTINEL}{}", uuid::Uuid::new_v4().simple());
    let script = format!(
        r#"
set -e
cd "{dir}"
{stubs}
. "{envrc}"
env -0
printf '%s' '{sentinel}'
"#,
        dir = dir.display(),
        stubs = DIRENV_STUBS,
        envrc = envrc_path.display(),
    );
    let baseline: HashMap<String, String> = std::env::vars().collect();

    let mut bash_cmd = Command::new("/bin/bash");
    bash_cmd.arg("-c").arg(&script).current_dir(dir);
    let output = match run_with_deadline(bash_cmd, deadline, "bash") {
        RunOutcome::Completed { output, .. } if !output.status.success() => {
            tracing::warn!(?envrc_path, "Failed to execute .envrc via bash");
            return None;
        }
        RunOutcome::Completed { output, .. } => output,
        RunOutcome::TimedOut | RunOutcome::Failed => return None,
    };

    let mut anchored = Vec::with_capacity(sentinel.len() + 1);
    anchored.push(0);
    anchored.extend_from_slice(sentinel.as_bytes());
    let stdout_bytes = match output
        .stdout
        .windows(anchored.len())
        .position(|window| window == anchored.as_slice())
    {
        Some(sentinel_at) => &output.stdout[..sentinel_at],
        None if output.stdout.starts_with(sentinel.as_bytes()) => return None,
        None => {
            tracing::warn!(?envrc_path, ".envrc output capture incomplete; discarding");
            return None;
        }
    };

    let stdout = String::from_utf8_lossy(stdout_bytes);
    let mut result = HashMap::new();
    for entry in stdout.split('\0') {
        let Some((key, value)) = entry.split_once('=') else {
            continue;
        };
        if ["_", "SHLVL", "PWD", "OLDPWD"].contains(&key) {
            continue;
        }
        if baseline.get(key).is_none_or(|baseline| baseline != value) {
            result.insert(key.to_owned(), value.to_owned());
        }
    }

    if result.is_empty() {
        tracing::debug!(?envrc_path, "No environment changes from .envrc");
        None
    } else {
        tracing::info!(
            ?envrc_path,
            count = result.len(),
            "Loaded environment from .envrc via bash"
        );
        Some(result)
    }
}

enum RunOutcome {
    Completed { output: Output, truncated: bool },
    TimedOut,
    Failed,
}

/// Run an evaluator until `deadline`, killing its process group on expiry.
fn run_with_deadline(mut cmd: Command, deadline: Instant, label: &str) -> RunOutcome {
    let budget = deadline.saturating_duration_since(Instant::now());
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    xai_grok_tools::util::detach_std_command(&mut cmd);
    #[allow(clippy::disallowed_methods)]
    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(error) => {
            tracing::debug!(label, ?error, "failed to spawn .envrc evaluator");
            return RunOutcome::Failed;
        }
    };

    let mut process_group = xai_grok_tools::util::ProcessGroup::new().ok();
    if let Some(group) = process_group.as_mut()
        && group.attach_std(&child).is_err()
    {
        process_group = None;
    }
    let process_group = process_group.map(Arc::new);
    if let Some(group) = &process_group {
        xai_grok_tools::util::global_process_scope().register(group);
    }

    let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
        kill_and_reap(&mut child, process_group.as_deref(), label);
        return RunOutcome::Failed;
    };
    let stdout = PipeDrain::start(stdout);
    let stderr = PipeDrain::start(stderr);

    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(POLL_INTERVAL),
            Ok(None) => {
                tracing::warn!(
                    label,
                    budget_ms = budget.as_millis() as u64,
                    "`.envrc` evaluation timed out; continuing without its environment (set {ENVRC_TIMEOUT_ENV} to extend)"
                );
                kill_and_reap(&mut child, process_group.as_deref(), label);
                return RunOutcome::TimedOut;
            }
            Err(error) => {
                tracing::warn!(label, ?error, "failed to wait for .envrc evaluator");
                kill_and_reap(&mut child, process_group.as_deref(), label);
                return RunOutcome::Failed;
            }
        }
    };
    drop(process_group);

    let cap = Instant::now() + PIPE_DRAIN_CAP;
    let (stdout, stdout_cut) = stdout.finish(cap);
    let (stderr, _) = stderr.finish(cap);
    RunOutcome::Completed {
        output: Output {
            status,
            stdout,
            stderr,
        },
        truncated: stdout_cut,
    }
}

fn kill_and_reap(
    child: &mut std::process::Child,
    process_group: Option<&xai_grok_tools::util::ProcessGroup>,
    label: &str,
) {
    if process_group.is_none_or(|group| group.kill().is_err()) {
        let _ = child.kill();
    }
    let deadline = Instant::now() + KILL_REAP_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(_)) | Err(_) => return,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(POLL_INTERVAL),
            Ok(None) => {
                tracing::warn!(
                    label,
                    pid = child.id(),
                    "abandoning unreapable .envrc evaluator"
                );
                return;
            }
        }
    }
}

/// Captures a pipe on a helper thread without waiting forever for descendants
/// that inherited the write end.
struct PipeDrain {
    buf: Arc<Mutex<Vec<u8>>>,
    done: Arc<AtomicBool>,
    truncated: Arc<AtomicBool>,
    reader: Option<std::thread::JoinHandle<()>>,
}

impl PipeDrain {
    #[cfg(unix)]
    fn start(mut pipe: impl Read + std::os::fd::AsRawFd + Send + 'static) -> Self {
        let fd = pipe.as_raw_fd();
        unsafe {
            let flags = libc::fcntl(fd, libc::F_GETFL);
            if flags >= 0 {
                libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
            }
        }
        Self::spawn_reader(move |stop, sink, cut| {
            let mut chunk = [0_u8; 8192];
            loop {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                let mut pollfd = libc::pollfd {
                    fd,
                    events: libc::POLLIN,
                    revents: 0,
                };
                let ready = unsafe { libc::poll(&mut pollfd, 1, POLL_INTERVAL.as_millis() as i32) };
                if ready <= 0 {
                    continue;
                }
                match pipe.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(read) => {
                        let mut buf = lock_ignore_poison(&sink);
                        if buf.len() + read > MAX_DRAIN_BYTES {
                            cut.store(true, Ordering::Relaxed);
                            break;
                        }
                        buf.extend_from_slice(&chunk[..read]);
                    }
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            || error.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(_) => break,
                }
            }
        })
    }

    #[cfg(not(unix))]
    fn start(mut pipe: impl Read + Send + 'static) -> Self {
        Self::spawn_reader(move |stop, sink, cut| {
            let mut chunk = [0_u8; 8192];
            loop {
                match pipe.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(read) => {
                        if stop.load(Ordering::Relaxed) {
                            cut.store(true, Ordering::Relaxed);
                            break;
                        }
                        let mut buf = lock_ignore_poison(&sink);
                        if buf.len() + read > MAX_DRAIN_BYTES {
                            cut.store(true, Ordering::Relaxed);
                            break;
                        }
                        buf.extend_from_slice(&chunk[..read]);
                    }
                }
            }
        })
    }

    fn spawn_reader(
        read_loop: impl FnOnce(Arc<AtomicBool>, Arc<Mutex<Vec<u8>>>, Arc<AtomicBool>) + Send + 'static,
    ) -> Self {
        let buf = Arc::new(Mutex::new(Vec::new()));
        let done = Arc::new(AtomicBool::new(false));
        let truncated = Arc::new(AtomicBool::new(false));
        let reader = std::thread::Builder::new()
            .name("envrc-pipe".into())
            .spawn({
                let stop = Arc::clone(&done);
                let sink = Arc::clone(&buf);
                let cut = Arc::clone(&truncated);
                move || read_loop(stop, sink, cut)
            })
            .ok();
        Self {
            buf,
            done,
            truncated,
            reader,
        }
    }

    fn finish(mut self, cap: Instant) -> (Vec<u8>, bool) {
        let spawned = self.reader.is_some();
        if let Some(reader) = &self.reader {
            let mut quiet_since = Instant::now();
            let mut last_len = lock_ignore_poison(&self.buf).len();
            while !reader.is_finished()
                && Instant::now() < cap
                && quiet_since.elapsed() < PIPE_DRAIN_GRACE
            {
                std::thread::sleep(POLL_INTERVAL);
                let len = lock_ignore_poison(&self.buf).len();
                if len != last_len {
                    last_len = len;
                    quiet_since = Instant::now();
                }
            }
        }
        self.done.store(true, Ordering::Relaxed);
        #[cfg(unix)]
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        let buf = std::mem::take(&mut *lock_ignore_poison(&self.buf));
        let truncated = self.truncated.load(Ordering::Relaxed) || !spawned;
        (buf, truncated)
    }
}

impl Drop for PipeDrain {
    fn drop(&mut self) {
        self.done.store(true, Ordering::Relaxed);
    }
}

fn lock_ignore_poison(buf: &Mutex<Vec<u8>>) -> std::sync::MutexGuard<'_, Vec<u8>> {
    buf.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_simple_export() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".envrc"), "export FOO=bar\n").unwrap();
        let env = load_envrc_with_timeout(dir.path(), Duration::from_secs(10)).unwrap();
        assert_eq!(env.get("FOO"), Some(&"bar".to_string()));
    }

    #[test]
    fn test_variable_expansion() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".envrc"), "export MY_DIR=$PWD/subdir\n").unwrap();
        let env = load_envrc_with_timeout(dir.path(), Duration::from_secs(10)).unwrap();
        assert_eq!(
            env.get("MY_DIR"),
            Some(&format!("{}/subdir", dir.path().display()))
        );
    }

    #[test]
    fn test_no_envrc() {
        let dir = TempDir::new().unwrap();
        assert!(load_envrc_with_timeout(dir.path(), Duration::from_secs(10)).is_none());
    }

    #[test]
    fn test_path_add() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".envrc"), "PATH_add bin\n").unwrap();
        let env = load_envrc_with_timeout(dir.path(), Duration::from_secs(10)).unwrap();
        assert!(env["PATH"].contains(&format!("{}/bin", dir.path().display())));
    }

    #[test]
    fn untrusted_envrc_is_not_evaluated() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".envrc"), "export FOO=bar\n").unwrap();
        assert!(load_envrc_or_empty_when_trusted(dir.path(), false).is_empty());
    }

    #[test]
    fn hung_evaluation_fails_open_at_the_deadline() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".envrc"), "sleep 300\n").unwrap();
        let started = Instant::now();
        assert!(load_envrc_with_timeout(dir.path(), Duration::from_millis(500)).is_none());
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn zero_timeout_disables_evaluation() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".envrc"), "export FOO=bar\n").unwrap();
        assert!(load_envrc_with_timeout(dir.path(), Duration::ZERO).is_none());
    }

    #[test]
    fn timeout_override_parses_and_clamps() {
        assert_eq!(timeout_from(None), ENVRC_LOAD_TIMEOUT);
        assert_eq!(timeout_from(Some(" 3 ")), Duration::from_secs(3));
        assert_eq!(timeout_from(Some("0")), Duration::ZERO);
        assert_eq!(timeout_from(Some("invalid")), ENVRC_LOAD_TIMEOUT);
        assert_eq!(timeout_from(Some(&u64::MAX.to_string())), MAX_TIMEOUT);
    }

    #[cfg(unix)]
    #[test]
    fn background_child_does_not_discard_env() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".envrc"), "export FOO=bar\nsleep 5 &\n").unwrap();
        let env = load_envrc_with_timeout(dir.path(), Duration::from_secs(10)).unwrap();
        assert_eq!(env.get("FOO"), Some(&"bar".to_string()));
    }

    #[cfg(unix)]
    #[test]
    fn sentinel_named_variable_does_not_truncate_env() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join(".envrc"),
            "export __GROK_ENVRC_COMPLETE__=decoy\nexport ZZ_AFTER_DECOY=survives\n",
        )
        .unwrap();
        let env = load_envrc_with_timeout(dir.path(), Duration::from_secs(10)).unwrap();
        assert_eq!(
            env.get("__GROK_ENVRC_COMPLETE__"),
            Some(&"decoy".to_string())
        );
        assert_eq!(env.get("ZZ_AFTER_DECOY"), Some(&"survives".to_string()));
    }

    #[cfg(unix)]
    #[test]
    fn fifo_envrc_is_refused() {
        use std::os::unix::fs::FileTypeExt;
        let dir = TempDir::new().unwrap();
        assert!(
            Command::new("mkfifo")
                .arg(dir.path().join(".envrc"))
                .status()
                .unwrap()
                .success()
        );
        assert!(
            fs::metadata(dir.path().join(".envrc"))
                .unwrap()
                .file_type()
                .is_fifo()
        );
        let started = Instant::now();
        assert!(load_envrc_with_timeout(dir.path(), Duration::from_secs(10)).is_none());
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[tokio::test]
    async fn spawned_loader_is_async_and_trust_gated() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".envrc"), "export FOO=bar\n").unwrap();
        let env = spawn_envrc_load(dir.path().to_path_buf(), true)
            .join()
            .await;
        assert_eq!(env.get("FOO"), Some(&"bar".to_string()));
        let env = spawn_envrc_load(dir.path().to_path_buf(), false)
            .join()
            .await;
        assert!(env.is_empty());
    }
}
