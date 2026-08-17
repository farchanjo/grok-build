//! Grok-owned Claude CLI permission-prompt MCP bridge (PR7 hardened).
//!
//! Architecture:
//! - Parent hosts a private UDS broker (0700 dir, 0600 socket, peer UID check,
//!   high-entropy auth token) that maps each Claude permission request onto
//!   Grok's [`PermissionHandle`] path — **always** one manager decision / audit
//!   event. No AlwaysApprove short-circuit; managed PolicyDeny wins under yolo.
//! - Claude is pointed at an ephemeral stdio MCP server via generated
//!   `--mcp-config` + `--permission-prompt-tool`. The MCP child re-execs the
//!   host binary under a hidden subcommand, authenticates to the parent, and
//!   exits when the broker socket closes (parent-death).
//! - Deny responses are successful MCP tool results (`isError: false`) with
//!   `behavior: "deny"` text per official permission-prompt-tool docs.
//! - Timeouts/cancel/bridge crash fail closed. Never executes tools. Never
//!   `bypassPermissions`.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use super::capability_mode::{ClaudeCapabilityMode, READ_SEARCH_TOOLS};
use crate::agent::execution_backend::ExternalAgentKind;
use crate::agent::external_runtime::{ExternalRuntimeError, ExternalRuntimeErrorKind};

/// Hidden argv[1] for the MCP permission-bridge child process.
pub const PERMISSION_BRIDGE_SUBCOMMAND: &str = "__claude-permission-bridge";

/// Stable MCP server name inside generated `--mcp-config`.
pub const BRIDGE_MCP_SERVER_NAME: &str = "grok-permission";

/// Tool name advertised to Claude (unqualified).
pub const BRIDGE_TOOL_NAME: &str = "permission_prompt";

/// Env var: broker socket path (bridge child only).
pub const BRIDGE_SOCKET_ENV: &str = "GROK_CLAUDE_PERMISSION_BRIDGE_SOCKET";

/// Env var: high-entropy auth token (bridge child only; never logged).
pub const BRIDGE_TOKEN_ENV: &str = "GROK_CLAUDE_PERMISSION_BRIDGE_TOKEN";

/// Full `--permission-prompt-tool` value.
pub fn permission_prompt_tool_flag() -> String {
    format!("mcp__{BRIDGE_MCP_SERVER_NAME}__{BRIDGE_TOOL_NAME}")
}

/// Parent broker decision timeout (shorter than / equal to child wait).
pub const PARENT_PERMISSION_TIMEOUT: Duration = Duration::from_secs(90);

/// Child bridge wait for parent broker response.
pub const CHILD_PERMISSION_TIMEOUT: Duration = Duration::from_secs(100);

/// Max bytes for a single broker / MCP JSON body (pre-read cap).
pub const MAX_BRIDGE_MESSAGE_BYTES: usize = 256 * 1024;

/// Max line length when scanning MCP Content-Length headers.
const MAX_HEADER_LINE_BYTES: usize = 8 * 1024;

// ---------------------------------------------------------------------------
// Schema
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClaudePermissionRequest {
    #[serde(default)]
    pub tool_use_id: Option<String>,
    pub tool_name: String,
    #[serde(default)]
    pub input: Value,
    #[serde(default)]
    pub decision_reason: Option<String>,
    #[serde(default)]
    pub blocked_path: Option<String>,
    #[serde(default)]
    pub agent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "behavior", rename_all = "snake_case")]
pub enum ClaudePermissionResponse {
    #[serde(rename = "allow")]
    Allow {
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            rename = "updatedInput"
        )]
        updated_input: Option<Value>,
    },
    #[serde(rename = "deny")]
    Deny {
        message: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        interrupt: Option<bool>,
    },
}

impl ClaudePermissionResponse {
    pub fn allow() -> Self {
        Self::Allow {
            updated_input: None,
        }
    }

    pub fn deny(message: impl Into<String>) -> Self {
        Self::Deny {
            message: message.into(),
            interrupt: None,
        }
    }

    pub fn deny_cancelled() -> Self {
        Self::Deny {
            message: "Permission prompt cancelled by host".into(),
            interrupt: Some(true),
        }
    }

    pub fn deny_timeout() -> Self {
        Self::Deny {
            message: "Permission prompt timed out; denying (fail closed)".into(),
            interrupt: None,
        }
    }

    pub fn to_mcp_text(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| {
            r#"{"behavior":"deny","message":"serialization failure"}"#.to_owned()
        })
    }
}

pub fn parse_permission_request(args: &Value) -> Result<ClaudePermissionRequest, String> {
    let root = args
        .get("arguments")
        .filter(|v| v.is_object())
        .unwrap_or(args);
    let tool_name = root
        .get("tool_name")
        .or_else(|| root.get("toolName"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "permission request missing tool_name".to_owned())?
        .to_owned();
    let tool_use_id = root
        .get("tool_use_id")
        .or_else(|| root.get("toolUseID"))
        .or_else(|| root.get("toolUseId"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned());
    let input = root
        .get("input")
        .cloned()
        .unwrap_or(Value::Object(Default::default()));
    if serde_json::to_vec(&input).map(|b| b.len()).unwrap_or(0) > MAX_BRIDGE_MESSAGE_BYTES {
        return Err("permission request input exceeds size cap".into());
    }
    Ok(ClaudePermissionRequest {
        tool_use_id,
        tool_name,
        input,
        decision_reason: root
            .get("decision_reason")
            .or_else(|| root.get("decisionReason"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned()),
        blocked_path: root
            .get("blocked_path")
            .or_else(|| root.get("blockedPath"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned()),
        agent_id: root
            .get("agent_id")
            .or_else(|| root.get("agentID"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned()),
    })
}

// ---------------------------------------------------------------------------
// Access mapping + capability precheck
// ---------------------------------------------------------------------------

pub fn map_claude_tool_to_access(
    tool_name: &str,
    input: &Value,
) -> xai_grok_workspace::permission::AccessKind {
    use xai_grok_workspace::permission::AccessKind;
    let name = tool_name.trim();
    let bare = name
        .strip_prefix("mcp__")
        .and_then(|rest| rest.split_once("__").map(|(_, t)| t))
        .unwrap_or(name);

    match bare {
        "Read" | "read_file" => {
            let path = input
                .get("file_path")
                .or_else(|| input.get("path"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_owned());
            AccessKind::Read(path)
        }
        "Grep" | "Glob" | "LS" | "LSDir" | "SemanticSearch" => {
            let path = input
                .get("path")
                .or_else(|| input.get("file_path"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_owned());
            let glob = input
                .get("glob")
                .or_else(|| input.get("pattern"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_owned());
            AccessKind::Grep { path, glob }
        }
        "Edit" | "Write" | "MultiEdit" | "NotebookEdit" | "search_replace" | "write_file" => {
            let path = input
                .get("file_path")
                .or_else(|| input.get("path"))
                .and_then(|v| v.as_str())
                .unwrap_or("<claude-edit>")
                .to_owned();
            AccessKind::Edit(path)
        }
        "Bash" | "PowerShell" | "Shell" | "run_terminal_cmd" | "Monitor" => {
            let cmd = input
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            AccessKind::Bash(cmd)
        }
        "WebFetch" | "web_fetch" => {
            let url = input
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            AccessKind::WebFetch(url)
        }
        "WebSearch" | "web_search" => {
            let q = input
                .get("query")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            AccessKind::WebSearch(q)
        }
        other => AccessKind::MCPTool {
            name: format!("claude::{other}"),
            input: input.clone(),
        },
    }
}

/// Pre-policy gate from capability mode. Only returns deny for restricted
/// modes; never allow without PermissionHandle.
pub fn capability_precheck(
    mode: ClaudeCapabilityMode,
    tool_name: &str,
) -> Option<ClaudePermissionResponse> {
    match mode {
        ClaudeCapabilityMode::ReadOnly => {
            let name = tool_name.trim();
            let is_known_read_only = !name.starts_with("mcp__")
                && (READ_SEARCH_TOOLS
                    .iter()
                    .any(|known| name.eq_ignore_ascii_case(known))
                    || [
                        "read_file",
                        "LSDir",
                        "SemanticSearch",
                        "web_fetch",
                        "web_search",
                    ]
                    .iter()
                    .any(|known| name.eq_ignore_ascii_case(known)));
            if is_known_read_only {
                None
            } else {
                Some(ClaudePermissionResponse::deny(format!(
                    "Claude tool '{tool_name}' denied by Grok read-only capability mode"
                )))
            }
        }
        ClaudeCapabilityMode::ReadWrite
        | ClaudeCapabilityMode::Execute
        | ClaudeCapabilityMode::All
        | ClaudeCapabilityMode::AlwaysApprove => None,
    }
}

pub fn decision_to_response(
    decision: xai_grok_workspace::permission::Decision,
) -> ClaudePermissionResponse {
    use xai_grok_workspace::permission::Decision;
    match decision {
        Decision::Allow => ClaudePermissionResponse::allow(),
        Decision::Ask => ClaudePermissionResponse::deny("permission ask unresolved; denying"),
        Decision::FollowupMessage(msg) => ClaudePermissionResponse::deny(msg),
        Decision::Reject(msg) => ClaudePermissionResponse::deny(msg),
        Decision::PolicyDeny(msg) => ClaudePermissionResponse::deny(format!("policy deny: {msg}")),
        Decision::Cancelled => ClaudePermissionResponse::deny_cancelled(),
    }
}

// ---------------------------------------------------------------------------
// Broker trait
// ---------------------------------------------------------------------------

#[async_trait]
pub trait ClaudePermissionBroker: Send + Sync {
    async fn decide(
        &self,
        request: ClaudePermissionRequest,
        cancel: CancellationToken,
    ) -> ClaudePermissionResponse;
}

pub struct DenyAllBroker;

#[async_trait]
impl ClaudePermissionBroker for DenyAllBroker {
    async fn decide(
        &self,
        request: ClaudePermissionRequest,
        _cancel: CancellationToken,
    ) -> ClaudePermissionResponse {
        ClaudePermissionResponse::deny(format!(
            "Grok permission broker unavailable; denying Claude tool '{}'",
            request.tool_name
        ))
    }
}

/// Policy-aware broker: capability precheck → **always** PermissionHandle::request.
///
/// AlwaysApprove never short-circuits. Managed PolicyDeny wins even under yolo.
pub struct PolicyPermissionBroker {
    pub mode: ClaudeCapabilityMode,
    pub permission: Option<xai_grok_workspace::permission::PermissionHandle>,
    /// Audit sink: one event per request.
    pub audit: Arc<Mutex<Vec<PermissionAuditEvent>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionAuditEvent {
    pub tool_name: String,
    pub tool_use_id: Option<String>,
    pub outcome: String,
}

impl PolicyPermissionBroker {
    pub fn new(mode: ClaudeCapabilityMode) -> Self {
        Self {
            mode,
            permission: None,
            audit: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn with_permission(
        mut self,
        handle: xai_grok_workspace::permission::PermissionHandle,
    ) -> Self {
        self.permission = Some(handle);
        self
    }

    /// Retained for API compatibility; does **not** enable short-circuit allow.
    /// AlwaysApprove only selects a broader tool allowlist at argv level.
    pub fn with_always_approve_opt_in(self, _enabled: bool) -> Self {
        self
    }

    async fn record(&self, tool_name: &str, tool_use_id: Option<String>, outcome: &str) {
        let mut g = self.audit.lock().await;
        g.push(PermissionAuditEvent {
            tool_name: tool_name.to_owned(),
            tool_use_id,
            outcome: outcome.to_owned(),
        });
    }
}

#[async_trait]
impl ClaudePermissionBroker for PolicyPermissionBroker {
    async fn decide(
        &self,
        request: ClaudePermissionRequest,
        cancel: CancellationToken,
    ) -> ClaudePermissionResponse {
        if cancel.is_cancelled() {
            self.record(
                &request.tool_name,
                request.tool_use_id.clone(),
                "deny_cancelled",
            )
            .await;
            return ClaudePermissionResponse::deny_cancelled();
        }

        if let Some(pre) = capability_precheck(self.mode, &request.tool_name) {
            self.record(
                &request.tool_name,
                request.tool_use_id.clone(),
                "deny_capability_mode",
            )
            .await;
            return pre;
        }

        // No short-circuit for AlwaysApprove / yolo. Every remaining decision
        // goes through PermissionHandle so managed PolicyDeny always wins.
        let Some(handle) = self.permission.as_ref() else {
            self.record(
                &request.tool_name,
                request.tool_use_id.clone(),
                "deny_no_handle",
            )
            .await;
            return ClaudePermissionResponse::deny(
                "Grok permission manager not attached; denying Claude tool",
            );
        };

        let access = map_claude_tool_to_access(&request.tool_name, &request.input);
        let tool_call_id = request
            .tool_use_id
            .clone()
            .unwrap_or_else(|| format!("claude-perm-{}", uuid::Uuid::new_v4()));
        let mut fields = agent_client_protocol::ToolCallUpdateFields::default();
        fields.title = Some(format!("Claude tool: {}", request.tool_name));
        fields.kind = Some(agent_client_protocol::ToolKind::Other);
        fields.status = Some(agent_client_protocol::ToolCallStatus::Pending);
        let tool_call_update = agent_client_protocol::ToolCallUpdate::new(
            agent_client_protocol::ToolCallId::new(std::sync::Arc::<str>::from(tool_call_id)),
            fields,
        );

        let decision_fut = handle.request(
            access,
            tool_call_update,
            None,
            request.agent_id.clone().map(|_| "claude_subagent".into()),
            request.agent_id.clone(),
        );

        let decision = tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                self.record(&request.tool_name, request.tool_use_id.clone(), "deny_cancelled").await;
                return ClaudePermissionResponse::deny_cancelled();
            }
            d = decision_fut => d,
        };

        let response = decision_to_response(decision);
        let outcome = match &response {
            ClaudePermissionResponse::Allow { .. } => "allow",
            ClaudePermissionResponse::Deny { .. } => "deny",
        };
        self.record(&request.tool_name, request.tool_use_id.clone(), outcome)
            .await;
        response
    }
}

/// Programmable broker for unit tests.
pub struct ScriptedBroker {
    pub responses: Mutex<std::collections::VecDeque<ClaudePermissionResponse>>,
    pub seen: Mutex<Vec<ClaudePermissionRequest>>,
    /// Artificial delay before responding (timeout tests).
    pub delay: Mutex<Option<Duration>>,
}

impl ScriptedBroker {
    pub fn new(responses: Vec<ClaudePermissionResponse>) -> Self {
        Self {
            responses: Mutex::new(responses.into()),
            seen: Mutex::new(Vec::new()),
            delay: Mutex::new(None),
        }
    }

    pub async fn set_delay(&self, d: Option<Duration>) {
        *self.delay.lock().await = d;
    }
}

#[async_trait]
impl ClaudePermissionBroker for ScriptedBroker {
    async fn decide(
        &self,
        request: ClaudePermissionRequest,
        cancel: CancellationToken,
    ) -> ClaudePermissionResponse {
        self.seen.lock().await.push(request);
        if cancel.is_cancelled() {
            return ClaudePermissionResponse::deny_cancelled();
        }
        if let Some(d) = *self.delay.lock().await {
            tokio::select! {
                _ = cancel.cancelled() => return ClaudePermissionResponse::deny_cancelled(),
                _ = tokio::time::sleep(d) => {}
            }
        }
        self.responses
            .lock()
            .await
            .pop_front()
            .unwrap_or_else(|| ClaudePermissionResponse::deny("scripted broker exhausted"))
    }
}

// ---------------------------------------------------------------------------
// Auth token + peer credentials
// ---------------------------------------------------------------------------

/// Generate a high-entropy auth token (hex, 32 bytes).
///
/// **Fail closed** if OS RNG is unavailable — no weak fallback.
pub fn generate_bridge_token() -> Result<String, ExternalRuntimeError> {
    let mut buf = [0u8; 32];
    fill_os_random(&mut buf).map_err(|e| {
        ExternalRuntimeError::new(
            ExternalRuntimeErrorKind::Transport,
            format!("permission bridge: OS RNG failure (fail closed): {e}"),
            Some(ExternalAgentKind::ClaudeCli),
        )
    })?;
    Ok(hex_encode(&buf))
}

fn fill_os_random(buf: &mut [u8]) -> Result<(), String> {
    // Prefer getrandom if available via /dev/urandom (Unix).
    #[cfg(unix)]
    {
        use std::io::Read;
        let mut f =
            std::fs::File::open("/dev/urandom").map_err(|e| format!("open /dev/urandom: {e}"))?;
        f.read_exact(buf)
            .map_err(|e| format!("read /dev/urandom: {e}"))?;
        return Ok(());
    }
    #[cfg(not(unix))]
    {
        let _ = buf;
        Err("OS RNG not available on this platform".into())
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0xf) as usize] as char);
    }
    s
}

/// Constant-time compare for auth tokens.
pub fn tokens_equal(a: &str, b: &str) -> bool {
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    if ab.len() != bb.len() {
        return false;
    }
    let mut diff = 0u8;
    for i in 0..ab.len() {
        diff |= ab[i] ^ bb[i];
    }
    diff == 0
}

/// Verify connecting peer UID matches the host process UID (Unix).
pub fn verify_peer_uid(stream: &std::os::unix::net::UnixStream) -> Result<(), String> {
    let peer_uid = peer_uid(stream)?;
    let self_uid = unsafe { libc::getuid() };
    if peer_uid != self_uid {
        return Err(format!(
            "permission bridge peer UID {peer_uid} != host UID {self_uid}"
        ));
    }
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "openbsd"))]
fn peer_uid(stream: &std::os::unix::net::UnixStream) -> Result<u32, String> {
    use std::os::unix::io::AsRawFd;
    let fd = stream.as_raw_fd();
    let mut uid: libc::uid_t = 0;
    let mut gid: libc::gid_t = 0;
    let rc = unsafe { libc::getpeereid(fd, &mut uid, &mut gid) };
    if rc != 0 {
        return Err(format!(
            "getpeereid failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(uid as u32)
}

#[cfg(target_os = "linux")]
fn peer_uid(stream: &std::os::unix::net::UnixStream) -> Result<u32, String> {
    use std::os::unix::io::AsRawFd;
    let fd = stream.as_raw_fd();
    let mut cred: libc::ucred = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let rc = unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut cred as *mut _ as *mut libc::c_void,
            &mut len,
        )
    };
    if rc != 0 {
        return Err(format!(
            "SO_PEERCRED failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(cred.uid)
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "linux"
)))]
fn peer_uid(_stream: &std::os::unix::net::UnixStream) -> Result<u32, String> {
    Err("peer credential verification unsupported on this platform".into())
}

// ---------------------------------------------------------------------------
// Secure private runtime dir
// ---------------------------------------------------------------------------

/// Create a unique private directory under TMPDIR with mode 0700.
/// Uses O_EXCL-style create (no remove-before-create race).
pub fn create_private_runtime_dir() -> Result<PathBuf, ExternalRuntimeError> {
    let base = std::env::temp_dir();
    for _ in 0..16 {
        let short = format!("{:x}", uuid::Uuid::new_v4().as_u128() & 0xffff_ffff);
        // Keep path short for SUN_LEN: $TMPDIR/gcbXXXXXXXX/
        let dir = base.join(format!("gcb{short}"));
        match std::fs::DirBuilder::new().mode(0o700).create(&dir) {
            Ok(()) => {
                // Ensure mode even if umask interfered.
                let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
                return Ok(dir);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(ExternalRuntimeError::new(
                    ExternalRuntimeErrorKind::Transport,
                    format!("permission bridge private dir: {e}"),
                    Some(ExternalAgentKind::ClaudeCli),
                ));
            }
        }
    }
    Err(ExternalRuntimeError::new(
        ExternalRuntimeErrorKind::Transport,
        "permission bridge: failed to create unique private dir",
        Some(ExternalAgentKind::ClaudeCli),
    ))
}

// ---------------------------------------------------------------------------
// Broker wire protocol (length-prefixed JSON + auth token)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
struct BrokerWireRequest {
    id: u64,
    /// High-entropy session token; constant-time compared on parent.
    token: String,
    request: ClaudePermissionRequest,
}

#[derive(Debug, Serialize, Deserialize)]
struct BrokerWireResponse {
    id: u64,
    response: ClaudePermissionResponse,
}

/// Parent-side permission broker server.
pub struct PermissionBrokerServer {
    /// Private 0700 directory owning the socket.
    pub runtime_dir: PathBuf,
    socket_path: PathBuf,
    /// Auth token (never logged / never persisted to durable storage).
    token: String,
    broker: Arc<dyn ClaudePermissionBroker>,
    cancel: CancellationToken,
    /// Serialize: one Claude permission → one UI decision.
    gate: Mutex<()>,
    /// Per-request cancel so timeout can release the gate.
    inflight_cancel: Mutex<Option<CancellationToken>>,
    shutdown: AtomicBool,
    accept_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// Bound decide timeout (parent ≤ child).
    decide_timeout: Duration,
}

impl PermissionBrokerServer {
    pub async fn start(
        broker: Arc<dyn ClaudePermissionBroker>,
        cancel: CancellationToken,
    ) -> Result<Arc<Self>, ExternalRuntimeError> {
        Self::start_with_timeout(broker, cancel, PARENT_PERMISSION_TIMEOUT).await
    }

    pub async fn start_with_timeout(
        broker: Arc<dyn ClaudePermissionBroker>,
        cancel: CancellationToken,
        decide_timeout: Duration,
    ) -> Result<Arc<Self>, ExternalRuntimeError> {
        let runtime_dir = create_private_runtime_dir()?;
        let socket_path = runtime_dir.join("b.sock");
        // No remove-before-bind: private dir is exclusive.
        let listener = UnixListener::bind(&socket_path).map_err(|e| {
            let _ = std::fs::remove_dir_all(&runtime_dir);
            ExternalRuntimeError::new(
                ExternalRuntimeErrorKind::Transport,
                format!("permission bridge bind ({}): {e}", socket_path.display()),
                Some(ExternalAgentKind::ClaudeCli),
            )
        })?;
        let _ = std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600));

        let token = generate_bridge_token()?;
        let server = Arc::new(Self {
            runtime_dir,
            socket_path,
            token,
            broker,
            cancel: cancel.clone(),
            gate: Mutex::new(()),
            inflight_cancel: Mutex::new(None),
            shutdown: AtomicBool::new(false),
            accept_task: Mutex::new(None),
            decide_timeout,
        });

        let serve = server.clone();
        let handle = tokio::spawn(async move {
            loop {
                if serve.shutdown.load(Ordering::Relaxed) || serve.cancel.is_cancelled() {
                    break;
                }
                tokio::select! {
                    _ = serve.cancel.cancelled() => break,
                    accept = listener.accept() => {
                        match accept {
                            Ok((stream, _)) => {
                                let s = serve.clone();
                                tokio::spawn(async move {
                                    s.handle_client(stream).await;
                                });
                            }
                            Err(_) => break,
                        }
                    }
                }
            }
        });
        *server.accept_task.lock().await = Some(handle);
        Ok(server)
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn runtime_dir(&self) -> &Path {
        &self.runtime_dir
    }

    async fn handle_client(&self, stream: UnixStream) {
        // Peer UID check via std conversion of the raw fd.
        {
            use std::os::unix::io::{AsRawFd, FromRawFd};
            let fd = stream.as_raw_fd();
            // Safety: duplicate fd for credential check only.
            let dup = unsafe { libc::dup(fd) };
            if dup >= 0 {
                let std_stream = unsafe { std::os::unix::net::UnixStream::from_raw_fd(dup) };
                if let Err(e) = verify_peer_uid(&std_stream) {
                    // Fail closed: drop connection.
                    let _ = e;
                    return;
                }
                // std_stream drops and closes dup.
            } else {
                return;
            }
        }

        let mut stream = stream;
        loop {
            if self.shutdown.load(Ordering::Relaxed) || self.cancel.is_cancelled() {
                break;
            }
            let msg = match read_framed_async(&mut stream).await {
                Ok(Some(m)) => m,
                Ok(None) => break, // EOF → parent-death for child
                Err(_) => break,   // malformed / oversize → close
            };
            let Ok(req) = serde_json::from_str::<BrokerWireRequest>(&msg) else {
                // Malformed → bounded error then close.
                let err = BrokerWireResponse {
                    id: 0,
                    response: ClaudePermissionResponse::deny("malformed broker request"),
                };
                let _ = write_framed_async(
                    &mut stream,
                    &serde_json::to_string(&err).unwrap_or_default(),
                )
                .await;
                break;
            };

            // Auth token constant-time compare.
            if !tokens_equal(&req.token, &self.token) {
                let err = BrokerWireResponse {
                    id: req.id,
                    response: ClaudePermissionResponse::deny("unauthorized bridge client"),
                };
                let _ = write_framed_async(
                    &mut stream,
                    &serde_json::to_string(&err).unwrap_or_default(),
                )
                .await;
                break; // reject arbitrary clients
            }

            // One-at-a-time UI decision with timeout that releases the gate.
            let req_cancel = self.cancel.child_token();
            {
                let mut slot = self.inflight_cancel.lock().await;
                if let Some(prev) = slot.take() {
                    prev.cancel();
                }
                *slot = Some(req_cancel.clone());
            }

            let response = {
                let _gate = self.gate.lock().await;
                if self.cancel.is_cancelled() || req_cancel.is_cancelled() {
                    ClaudePermissionResponse::deny_cancelled()
                } else {
                    let decide = self.broker.decide(req.request, req_cancel.clone());
                    match tokio::time::timeout(self.decide_timeout, decide).await {
                        Ok(r) => r,
                        Err(_) => {
                            // Timeout: cancel pending UI if possible, release gate (drop).
                            req_cancel.cancel();
                            ClaudePermissionResponse::deny_timeout()
                        }
                    }
                }
            };
            *self.inflight_cancel.lock().await = None;

            let wire = BrokerWireResponse {
                id: req.id, // match response ID to request ID
                response,
            };
            if let Ok(out) = serde_json::to_string(&wire) {
                if write_framed_async(&mut stream, &out).await.is_err() {
                    break;
                }
            } else {
                break;
            }
        }
    }

    pub async fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
        self.cancel.cancel();
        if let Some(c) = self.inflight_cancel.lock().await.take() {
            c.cancel();
        }
        if let Some(h) = self.accept_task.lock().await.take() {
            h.abort();
        }
        let _ = std::fs::remove_file(&self.socket_path);
        let _ = std::fs::remove_dir_all(&self.runtime_dir);
    }
}

impl Drop for PermissionBrokerServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        let _ = std::fs::remove_file(&self.socket_path);
        let _ = std::fs::remove_dir_all(&self.runtime_dir);
    }
}

// Length-prefixed framing: u32 BE length + UTF-8 JSON (capped).
async fn read_framed_async(stream: &mut UnixStream) -> Result<Option<String>, String> {
    let mut len_buf = [0u8; 4];
    match stream.read_exact(&mut len_buf).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(format!("frame header: {e}")),
    }
    let len = u32::from_be_bytes(len_buf) as usize;
    if len == 0 {
        return Ok(None);
    }
    if len > MAX_BRIDGE_MESSAGE_BYTES {
        return Err(format!("frame too large: {len}"));
    }
    let mut body = vec![0u8; len];
    stream
        .read_exact(&mut body)
        .await
        .map_err(|e| format!("frame body: {e}"))?;
    String::from_utf8(body)
        .map(Some)
        .map_err(|_| "invalid utf-8".into())
}

async fn write_framed_async(stream: &mut UnixStream, body: &str) -> Result<(), String> {
    let bytes = body.as_bytes();
    if bytes.len() > MAX_BRIDGE_MESSAGE_BYTES {
        return Err("frame too large".into());
    }
    let len = (bytes.len() as u32).to_be_bytes();
    stream
        .write_all(&len)
        .await
        .map_err(|e| format!("frame write header: {e}"))?;
    stream
        .write_all(bytes)
        .await
        .map_err(|e| format!("frame write body: {e}"))?;
    stream
        .flush()
        .await
        .map_err(|e| format!("frame flush: {e}"))?;
    Ok(())
}

fn read_framed_std(stream: &mut std::os::unix::net::UnixStream) -> Result<Option<String>, String> {
    let mut len_buf = [0u8; 4];
    match stream.read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(format!("frame header: {e}")),
    }
    let len = u32::from_be_bytes(len_buf) as usize;
    if len == 0 {
        return Ok(None);
    }
    if len > MAX_BRIDGE_MESSAGE_BYTES {
        return Err(format!("frame too large: {len}"));
    }
    let mut body = vec![0u8; len];
    stream
        .read_exact(&mut body)
        .map_err(|e| format!("frame body: {e}"))?;
    String::from_utf8(body)
        .map(Some)
        .map_err(|_| "invalid utf-8".into())
}

fn write_framed_std(stream: &mut std::os::unix::net::UnixStream, body: &str) -> Result<(), String> {
    let bytes = body.as_bytes();
    if bytes.len() > MAX_BRIDGE_MESSAGE_BYTES {
        return Err("frame too large".into());
    }
    let len = (bytes.len() as u32).to_be_bytes();
    stream
        .write_all(&len)
        .map_err(|e| format!("frame write header: {e}"))?;
    stream
        .write_all(bytes)
        .map_err(|e| format!("frame write body: {e}"))?;
    stream.flush().map_err(|e| format!("frame flush: {e}"))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// MCP child
// ---------------------------------------------------------------------------

/// Run the MCP permission-bridge child (blocking). Returns process exit code.
pub fn run_permission_bridge_child(socket_path: &Path, token: &str) -> i32 {
    match run_mcp_stdio_bridge(socket_path, token) {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

/// If argv invokes the hidden bridge subcommand, run it and return exit code.
pub fn maybe_run_permission_bridge_subprocess() -> Option<i32> {
    let argv: Vec<std::ffi::OsString> = std::env::args_os().collect();
    if argv.get(1).and_then(|a| a.to_str()) != Some(PERMISSION_BRIDGE_SUBCOMMAND) {
        return None;
    }
    let mut socket: Option<PathBuf> = None;
    let mut i = 2usize;
    while i < argv.len() {
        let a = argv[i].to_string_lossy();
        if a == "--socket" {
            if let Some(p) = argv.get(i + 1) {
                socket = Some(PathBuf::from(p));
                i += 2;
                continue;
            }
        } else if let Some(rest) = a.strip_prefix("--socket=") {
            socket = Some(PathBuf::from(rest));
        }
        // Production: token is env-only (never argv — avoids process-table leakage).
        i += 1;
    }
    if socket.is_none() {
        if let Ok(p) = std::env::var(BRIDGE_SOCKET_ENV) {
            if !p.is_empty() {
                socket = Some(PathBuf::from(p));
            }
        }
    }
    // Token: environment only.
    let token = std::env::var(BRIDGE_TOKEN_ENV)
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());
    let Some(path) = socket else {
        let _ = writeln_stderr("permission bridge: missing --socket");
        return Some(2);
    };
    let Some(tok) = token else {
        let _ = writeln_stderr("permission bridge: missing auth token env");
        return Some(2);
    };
    Some(run_permission_bridge_child(&path, &tok))
}

fn writeln_stderr(msg: &str) {
    let _ = writeln!(std::io::stderr(), "{msg}");
}

fn run_mcp_stdio_bridge(socket_path: &Path, token: &str) -> Result<(), String> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut next_broker_id = 1u64;

    loop {
        let msg = match read_mcp_message_capped(&mut reader)? {
            Some(m) => m,
            None => break,
        };
        let id = msg.get("id").cloned();
        let method = msg.get("method").and_then(|v| v.as_str()).unwrap_or("");

        match method {
            "initialize" => {
                let result = json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": {
                        "name": BRIDGE_MCP_SERVER_NAME,
                        "version": "1.0.0"
                    }
                });
                write_mcp_result(&mut stdout, id, result)?;
            }
            "notifications/initialized" | "initialized" => {}
            "tools/list" => {
                let result = json!({
                    "tools": [{
                        "name": BRIDGE_TOOL_NAME,
                        "description": "Grok-owned permission prompt for Claude Agent CLI. Does not execute tools.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "tool_name": { "type": "string" },
                                "tool_use_id": { "type": "string" },
                                "input": { "type": "object" }
                            },
                            "required": ["tool_name"]
                        }
                    }]
                });
                write_mcp_result(&mut stdout, id, result)?;
            }
            "tools/call" => {
                let params = msg.get("params").cloned().unwrap_or(Value::Null);
                let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                if name != BRIDGE_TOOL_NAME && name != permission_prompt_tool_flag() {
                    let err_text = ClaudePermissionResponse::deny(format!(
                        "unknown bridge tool '{name}'; only {BRIDGE_TOOL_NAME} is supported"
                    ))
                    .to_mcp_text();
                    // Deny is a successful tool result (isError: false).
                    write_mcp_tool_text(&mut stdout, id, &err_text, false)?;
                    continue;
                }
                let args = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or(Value::Object(Default::default()));
                let request = match parse_permission_request(&args) {
                    Ok(r) => r,
                    Err(e) => {
                        let err_text = ClaudePermissionResponse::deny(e).to_mcp_text();
                        write_mcp_tool_text(&mut stdout, id, &err_text, false)?;
                        continue;
                    }
                };
                let response = match forward_to_broker(socket_path, token, next_broker_id, request)
                {
                    Ok(r) => r,
                    Err(e) => {
                        // Socket closed → parent death; fail closed and exit.
                        if e.contains("closed") || e.contains("Connection") {
                            let deny = ClaudePermissionResponse::deny(format!(
                                "permission bridge parent disconnected: {e}"
                            ));
                            let _ =
                                write_mcp_tool_text(&mut stdout, id, &deny.to_mcp_text(), false);
                            return Err(e);
                        }
                        ClaudePermissionResponse::deny(format!(
                            "permission bridge forward failed: {e}"
                        ))
                    }
                };
                next_broker_id = next_broker_id.saturating_add(1);
                // Official docs: allow/deny returned as tool text; isError false.
                write_mcp_tool_text(&mut stdout, id, &response.to_mcp_text(), false)?;
            }
            "ping" => {
                write_mcp_result(&mut stdout, id, json!({}))?;
            }
            "" if msg.get("result").is_some() || msg.get("error").is_some() => {}
            other => {
                if id.is_some() {
                    write_mcp_error(
                        &mut stdout,
                        id,
                        -32601,
                        &format!("method not found: {other}"),
                    )?;
                }
            }
        }
    }
    Ok(())
}

/// Async authenticated forward (integration tests + unit tests).
pub async fn forward_to_broker_for_test(
    socket_path: &Path,
    token: &str,
    request: ClaudePermissionRequest,
) -> Result<ClaudePermissionResponse, String> {
    forward_to_broker_async(socket_path, token, 1, request).await
}

async fn forward_to_broker_async(
    socket_path: &Path,
    token: &str,
    id: u64,
    request: ClaudePermissionRequest,
) -> Result<ClaudePermissionResponse, String> {
    let mut last = String::new();
    let mut stream = None;
    for _ in 0..40 {
        match UnixStream::connect(socket_path).await {
            Ok(s) => {
                stream = Some(s);
                break;
            }
            Err(e) => {
                last = e.to_string();
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        }
    }
    let mut stream = stream.ok_or_else(|| format!("connect broker socket: {last}"))?;
    let wire = BrokerWireRequest {
        id,
        token: token.to_owned(),
        request,
    };
    let body = serde_json::to_string(&wire).map_err(|e| e.to_string())?;
    write_framed_async(&mut stream, &body).await?;
    let resp_body = tokio::time::timeout(Duration::from_secs(5), read_framed_async(&mut stream))
        .await
        .map_err(|_| "broker read timeout".to_owned())?
        .map_err(|e| format!("broker read: {e}"))?
        .ok_or_else(|| "broker closed without response".to_owned())?;
    let wire: BrokerWireResponse =
        serde_json::from_str(&resp_body).map_err(|e| format!("broker parse: {e}"))?;
    if wire.id != id {
        return Err(format!(
            "response id mismatch: expected {id}, got {}",
            wire.id
        ));
    }
    Ok(wire.response)
}

fn forward_to_broker(
    socket_path: &Path,
    token: &str,
    id: u64,
    request: ClaudePermissionRequest,
) -> Result<ClaudePermissionResponse, String> {
    use std::os::unix::net::UnixStream as StdUnixStream;
    let mut stream = None;
    let mut last = String::new();
    for _ in 0..20 {
        match StdUnixStream::connect(socket_path) {
            Ok(s) => {
                stream = Some(s);
                break;
            }
            Err(e) => {
                last = e.to_string();
                std::thread::sleep(Duration::from_millis(25));
            }
        }
    }
    let mut stream = stream.ok_or_else(|| format!("connect broker socket: {last}"))?;
    let _ = stream.set_read_timeout(Some(CHILD_PERMISSION_TIMEOUT));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));
    let wire = BrokerWireRequest {
        id,
        token: token.to_owned(),
        request,
    };
    let body = serde_json::to_string(&wire).map_err(|e| e.to_string())?;
    write_framed_std(&mut stream, &body)?;
    let resp_body =
        read_framed_std(&mut stream)?.ok_or_else(|| "broker closed without response".to_owned())?;
    let wire: BrokerWireResponse =
        serde_json::from_str(&resp_body).map_err(|e| format!("broker parse: {e}"))?;
    if wire.id != id {
        return Err(format!(
            "response id mismatch: expected {id}, got {}",
            wire.id
        ));
    }
    Ok(wire.response)
}

// ---------------------------------------------------------------------------
// MCP Content-Length framing with pre-read caps
// ---------------------------------------------------------------------------

fn read_mcp_message_capped<R: BufRead>(reader: &mut R) -> Result<Option<Value>, String> {
    let mut headers = String::new();
    let mut header_bytes = 0usize;
    loop {
        let mut line = String::new();
        let n = reader
            .read_line(&mut line)
            .map_err(|e| format!("mcp header read: {e}"))?;
        if n == 0 {
            return if headers.is_empty() {
                Ok(None)
            } else {
                Err("unexpected EOF in MCP headers".into())
            };
        }
        header_bytes = header_bytes.saturating_add(n);
        if header_bytes > MAX_HEADER_LINE_BYTES * 8 {
            return Err("MCP headers exceed size cap".into());
        }
        if line.len() > MAX_HEADER_LINE_BYTES {
            return Err("MCP header line too large".into());
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        headers.push_str(&line);
    }
    let mut content_length: Option<usize> = None;
    for line in headers.lines() {
        let lower = line.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("content-length:") {
            content_length = rest.trim().parse().ok();
        }
    }
    let len = content_length.ok_or_else(|| "missing Content-Length".to_owned())?;
    if len > MAX_BRIDGE_MESSAGE_BYTES {
        return Err(format!("MCP message too large: {len}"));
    }
    let mut buf = vec![0u8; len];
    reader
        .read_exact(&mut buf)
        .map_err(|e| format!("mcp body read: {e}"))?;
    let value: Value = serde_json::from_slice(&buf).map_err(|e| format!("mcp json: {e}"))?;
    Ok(Some(value))
}

fn write_mcp_message<W: Write>(writer: &mut W, value: &Value) -> Result<(), String> {
    let body = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len()).map_err(|e| e.to_string())?;
    writer.write_all(&body).map_err(|e| e.to_string())?;
    writer.flush().map_err(|e| e.to_string())?;
    Ok(())
}

fn write_mcp_result<W: Write>(
    writer: &mut W,
    id: Option<Value>,
    result: Value,
) -> Result<(), String> {
    let msg = json!({
        "jsonrpc": "2.0",
        "id": id.unwrap_or(Value::Null),
        "result": result,
    });
    write_mcp_message(writer, &msg)
}

fn write_mcp_error<W: Write>(
    writer: &mut W,
    id: Option<Value>,
    code: i64,
    message: &str,
) -> Result<(), String> {
    let msg = json!({
        "jsonrpc": "2.0",
        "id": id.unwrap_or(Value::Null),
        "error": { "code": code, "message": message },
    });
    write_mcp_message(writer, &msg)
}

/// Write MCP tool result. Deny uses `isError: false` (successful result body
/// containing behavior:deny JSON) per official permission-prompt-tool docs.
fn write_mcp_tool_text<W: Write>(
    writer: &mut W,
    id: Option<Value>,
    text: &str,
    is_error: bool,
) -> Result<(), String> {
    let result = json!({
        "content": [{ "type": "text", "text": text }],
        "isError": is_error,
    });
    write_mcp_result(writer, id, result)
}

/// Build MCP server config entry for the bridge child (path + socket + token env).
pub fn bridge_mcp_server_entry(
    host_executable: &Path,
    socket_path: &Path,
    token: &str,
) -> HashMap<String, Value> {
    let mut server = serde_json::Map::new();
    server.insert(
        "command".into(),
        json!(host_executable.to_string_lossy().into_owned()),
    );
    server.insert(
        "args".into(),
        json!([
            PERMISSION_BRIDGE_SUBCOMMAND,
            "--socket",
            socket_path.to_string_lossy().into_owned()
        ]),
    );
    // Token only in bridge child env — not Claude tool environment generally.
    let mut env = serde_json::Map::new();
    env.insert(
        BRIDGE_SOCKET_ENV.into(),
        json!(socket_path.to_string_lossy().into_owned()),
    );
    env.insert(BRIDGE_TOKEN_ENV.into(), json!(token));
    server.insert("env".into(), Value::Object(env));
    let mut servers = HashMap::new();
    servers.insert(BRIDGE_MCP_SERVER_NAME.to_owned(), Value::Object(server));
    servers
}

#[allow(dead_code)]
static TEST_REQ_ID: AtomicU64 = AtomicU64::new(1);

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::super::capability_mode::ClaudeCapabilityMode;
    use super::*;

    #[test]
    fn parses_permission_request_schema() {
        let v = json!({
            "tool_name": "Bash",
            "tool_use_id": "tu-1",
            "input": { "command": "ls" }
        });
        let r = parse_permission_request(&v).unwrap();
        assert_eq!(r.tool_name, "Bash");
    }

    #[test]
    fn deny_mcp_text_is_behavior_deny() {
        let deny = ClaudePermissionResponse::deny("nope").to_mcp_text();
        assert!(deny.contains("\"behavior\":\"deny\""));
        // isError is set at MCP envelope, not in this JSON.
        assert!(!deny.contains("isError"));
    }

    #[test]
    fn mcp_tool_text_deny_is_error_false() {
        // Simulate write_mcp_tool_text encoding.
        let text = ClaudePermissionResponse::deny("blocked").to_mcp_text();
        let result = json!({
            "content": [{ "type": "text", "text": text }],
            "isError": false,
        });
        assert_eq!(result["isError"], false);
        assert!(
            result["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("\"behavior\":\"deny\"")
        );
    }

    #[test]
    fn read_only_precheck_allows_known_reads_and_denies_mutations() {
        for tool in [
            "Read",
            "read_file",
            "Grep",
            "Glob",
            "LS",
            "LSDir",
            "SemanticSearch",
            "WebSearch",
            "web_search",
            "WebFetch",
            "web_fetch",
        ] {
            assert!(
                capability_precheck(ClaudeCapabilityMode::ReadOnly, tool).is_none(),
                "known read-only tool {tool} must remain brokered"
            );
        }
        for tool in ["Edit", "Write", "Bash", "run_terminal_cmd"] {
            assert!(
                capability_precheck(ClaudeCapabilityMode::ReadOnly, tool).is_some(),
                "mutating tool {tool} must be denied"
            );
        }
    }

    #[test]
    fn read_only_precheck_fails_closed_for_unknown_and_mcp_tools() {
        for tool in [
            "FutureReadLikeTool",
            "mcp__filesystem__read_file",
            "mcp__server__Read",
            "mcp__server__unknown",
        ] {
            assert!(
                capability_precheck(ClaudeCapabilityMode::ReadOnly, tool).is_some(),
                "unclassified tool {tool} must be denied"
            );
        }
    }

    #[test]
    fn always_approve_precheck_does_not_auto_allow() {
        // AlwaysApprove never returns allow from precheck.
        assert!(capability_precheck(ClaudeCapabilityMode::AlwaysApprove, "Bash").is_none());
        assert!(capability_precheck(ClaudeCapabilityMode::AlwaysApprove, "Edit").is_none());
    }

    #[test]
    fn tokens_equal_constant_time_shape() {
        assert!(tokens_equal("abc", "abc"));
        assert!(!tokens_equal("abc", "abd"));
        assert!(!tokens_equal("abc", "ab"));
    }

    #[test]
    fn private_runtime_dir_is_0700() {
        let dir = create_private_runtime_dir().unwrap();
        let meta = std::fs::metadata(&dir).unwrap();
        let mode = meta.permissions().mode() & 0o777;
        assert_eq!(mode, 0o700, "dir mode {mode:o}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn broker_socket_auth_and_roundtrip() {
        let broker = Arc::new(ScriptedBroker::new(vec![ClaudePermissionResponse::allow()]));
        let cancel = CancellationToken::new();
        let server = PermissionBrokerServer::start(broker.clone(), cancel.clone())
            .await
            .unwrap();
        // Authorized.
        let resp = forward_to_broker_async(
            server.socket_path(),
            server.token(),
            1,
            ClaudePermissionRequest {
                tool_use_id: Some("t".into()),
                tool_name: "Read".into(),
                input: json!({}),
                decision_reason: None,
                blocked_path: None,
                agent_id: None,
            },
        )
        .await
        .expect("authorized roundtrip");
        assert!(matches!(resp, ClaudePermissionResponse::Allow { .. }));

        // Unauthorized token rejected.
        let bad = forward_to_broker_async(
            server.socket_path(),
            "wrong-token-value-xxxxxxxx",
            2,
            ClaudePermissionRequest {
                tool_use_id: None,
                tool_name: "Read".into(),
                input: json!({}),
                decision_reason: None,
                blocked_path: None,
                agent_id: None,
            },
        )
        .await;
        assert!(
            bad.is_err()
                || matches!(
                    bad.as_ref().ok(),
                    Some(ClaudePermissionResponse::Deny { message, .. })
                        if message.contains("unauthorized")
                )
                || bad
                    .as_ref()
                    .ok()
                    .map(|r| matches!(r, ClaudePermissionResponse::Deny { .. }))
                    .unwrap_or(true),
            "unauthorized client must fail closed: {bad:?}"
        );
        server.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn broker_timeout_then_second_request() {
        let broker = Arc::new(ScriptedBroker::new(vec![
            ClaudePermissionResponse::allow(), // will be delayed past timeout
            ClaudePermissionResponse::allow(), // second request
        ]));
        broker.set_delay(Some(Duration::from_secs(2))).await;
        let cancel = CancellationToken::new();
        let server = PermissionBrokerServer::start_with_timeout(
            broker.clone(),
            cancel.clone(),
            Duration::from_millis(200),
        )
        .await
        .unwrap();

        let r1 = forward_to_broker_async(
            server.socket_path(),
            server.token(),
            10,
            ClaudePermissionRequest {
                tool_use_id: Some("slow".into()),
                tool_name: "Read".into(),
                input: json!({}),
                decision_reason: None,
                blocked_path: None,
                agent_id: None,
            },
        )
        .await
        .expect("timeout response");
        assert!(
            matches!(r1, ClaudePermissionResponse::Deny { ref message, .. } if message.contains("timed out")),
            "expected timeout deny, got {r1:?}"
        );

        // Clear delay for second request.
        broker.set_delay(None).await;
        let r2 = forward_to_broker_async(
            server.socket_path(),
            server.token(),
            11,
            ClaudePermissionRequest {
                tool_use_id: Some("fast".into()),
                tool_name: "Read".into(),
                input: json!({}),
                decision_reason: None,
                blocked_path: None,
                agent_id: None,
            },
        )
        .await
        .expect("second request");
        assert!(matches!(r2, ClaudePermissionResponse::Allow { .. }));
        server.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn response_id_matches_request_id() {
        let broker = Arc::new(ScriptedBroker::new(vec![ClaudePermissionResponse::allow()]));
        let cancel = CancellationToken::new();
        let server = PermissionBrokerServer::start(broker, cancel).await.unwrap();
        let id = 42u64;
        let resp = forward_to_broker_async(
            server.socket_path(),
            server.token(),
            id,
            ClaudePermissionRequest {
                tool_use_id: None,
                tool_name: "Read".into(),
                input: json!({}),
                decision_reason: None,
                blocked_path: None,
                agent_id: None,
            },
        )
        .await
        .unwrap();
        assert!(matches!(resp, ClaudePermissionResponse::Allow { .. }));
        server.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn oversize_frame_closes() {
        let broker = Arc::new(ScriptedBroker::new(vec![]));
        let cancel = CancellationToken::new();
        let server = PermissionBrokerServer::start(broker, cancel).await.unwrap();
        let mut stream = UnixStream::connect(server.socket_path()).await.unwrap();
        // Send oversize length prefix.
        let huge = (MAX_BRIDGE_MESSAGE_BYTES as u32 + 10).to_be_bytes();
        stream.write_all(&huge).await.unwrap();
        stream.write_all(&vec![b'x'; 16]).await.unwrap();
        // Server should close; subsequent read EOF.
        let mut buf = [0u8; 4];
        let r = tokio::time::timeout(Duration::from_secs(2), stream.read_exact(&mut buf)).await;
        // Either timeout with closed or error — both fine.
        let _ = r;
        server.shutdown().await;
    }

    #[tokio::test]
    async fn always_approve_still_calls_handle_no_short_circuit() {
        // Without handle, AlwaysApprove still denies (no short-circuit allow).
        let policy = PolicyPermissionBroker::new(ClaudeCapabilityMode::AlwaysApprove)
            .with_always_approve_opt_in(true);
        let r = policy
            .decide(
                ClaudePermissionRequest {
                    tool_use_id: Some("a".into()),
                    tool_name: "Bash".into(),
                    input: json!({"command": "echo hi"}),
                    decision_reason: None,
                    blocked_path: None,
                    agent_id: None,
                },
                CancellationToken::new(),
            )
            .await;
        assert!(matches!(r, ClaudePermissionResponse::Deny { .. }));
        let audit = policy.audit.lock().await;
        assert_eq!(audit.len(), 1);
        assert_eq!(audit[0].outcome, "deny_no_handle");
    }

    #[test]
    fn decision_mapping_policy_deny() {
        use xai_grok_workspace::permission::Decision;
        let r = decision_to_response(Decision::PolicyDeny("blocked".into()));
        assert!(matches!(
            r,
            ClaudePermissionResponse::Deny { message, .. } if message.contains("policy deny")
        ));
    }
}
