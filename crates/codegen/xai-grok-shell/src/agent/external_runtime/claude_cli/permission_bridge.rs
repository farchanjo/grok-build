//! Grok-owned Claude CLI permission-prompt MCP bridge (PR7).
//!
//! Architecture:
//! - Parent Grok process hosts a Unix-domain-socket **broker** that maps each
//!   Claude permission request onto Grok's [`PermissionHandle`] / AcpPrompter
//!   path (one request → one UI decision / audit event).
//! - Claude is pointed at an ephemeral stdio MCP server via generated
//!   `--mcp-config` + `--permission-prompt-tool`. That MCP server is a
//!   re-exec of the host binary under a hidden subcommand that speaks MCP
//!   over stdio and forwards each `tools/call` to the parent broker.
//! - The bridge **never** executes the requested Claude tool. Timeouts,
//!   cancel, and bridge crashes **fail closed** (deny). Unknown tools and
//!   actions default to **ask** (or deny under capability mode). Never
//!   `bypassPermissions` / `--dangerously-skip-permissions`.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader as TokioBufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{Mutex, oneshot};
use tokio_util::sync::CancellationToken;

use super::capability_mode::{ClaudeCapabilityMode, is_write_or_shell_tool};
use crate::agent::execution_backend::ExternalAgentKind;
use crate::agent::external_runtime::{ExternalRuntimeError, ExternalRuntimeErrorKind};

/// Hidden argv[1] for the MCP permission-bridge child process.
pub const PERMISSION_BRIDGE_SUBCOMMAND: &str = "__claude-permission-bridge";

/// Stable MCP server name inside generated `--mcp-config`.
pub const BRIDGE_MCP_SERVER_NAME: &str = "grok-permission";

/// Tool name advertised to Claude (unqualified; Claude qualifies as
/// `mcp__grok-permission__permission_prompt`).
pub const BRIDGE_TOOL_NAME: &str = "permission_prompt";

/// Full `--permission-prompt-tool` value.
pub fn permission_prompt_tool_flag() -> String {
    format!("mcp__{BRIDGE_MCP_SERVER_NAME}__{BRIDGE_TOOL_NAME}")
}

/// Default UI/broker timeout for a single permission prompt.
pub const DEFAULT_PERMISSION_TIMEOUT: Duration = Duration::from_secs(120);

/// Max bytes accepted for a single broker / MCP JSON body.
pub const MAX_BRIDGE_MESSAGE_BYTES: usize = 256 * 1024;

// ---------------------------------------------------------------------------
// Schema (Claude → MCP tool → broker)
// ---------------------------------------------------------------------------

/// Claude CLI permission-prompt-tool input (conservative documented shape).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClaudePermissionRequest {
    #[serde(default)]
    pub tool_use_id: Option<String>,
    pub tool_name: String,
    #[serde(default)]
    pub input: Value,
    /// Optional extended fields from newer CLIs (ignored for decision).
    #[serde(default)]
    pub decision_reason: Option<String>,
    #[serde(default)]
    pub blocked_path: Option<String>,
    #[serde(default)]
    pub agent_id: Option<String>,
}

/// Documented MCP tool text-body response for allow / deny.
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

    pub fn allow_with_input(input: Value) -> Self {
        Self::Allow {
            updated_input: Some(input),
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

    pub fn to_mcp_text(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| {
            r#"{"behavior":"deny","message":"serialization failure"}"#.to_owned()
        })
    }
}

/// Parse MCP tool arguments into a request. Missing `tool_name` fails closed.
pub fn parse_permission_request(args: &Value) -> Result<ClaudePermissionRequest, String> {
    // Accept both flat tool args and nested `{ "arguments": ... }` wrappers.
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
    if input_size_bytes(&input) > MAX_BRIDGE_MESSAGE_BYTES {
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

fn input_size_bytes(v: &Value) -> usize {
    serde_json::to_vec(v).map(|b| b.len()).unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Conservative AccessKind mapping (Claude tools → Grok permission surface)
// ---------------------------------------------------------------------------

/// Map a Claude tool name + input to a Grok [`AccessKind`]-like classification
/// used for policy. Unknown tools map to a generic MCPTool access so the
/// effective policy defaults to **ask** (never silent allow).
pub fn map_claude_tool_to_access(
    tool_name: &str,
    input: &Value,
) -> xai_grok_workspace::permission::AccessKind {
    use xai_grok_workspace::permission::AccessKind;
    let name = tool_name.trim();
    // Strip mcp__server__ prefix if Claude re-qualifies.
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

/// Pre-policy gate from capability mode (before PermissionHandle).
///
/// Returns `Some(deny/allow)` when the mode resolves without UI; `None` means
/// fall through to the broker / PermissionHandle (ask path).
pub fn capability_precheck(
    mode: ClaudeCapabilityMode,
    tool_name: &str,
) -> Option<ClaudePermissionResponse> {
    match mode {
        ClaudeCapabilityMode::ReadOnly => {
            if is_write_or_shell_tool(tool_name) {
                Some(ClaudePermissionResponse::deny(format!(
                    "Claude tool '{tool_name}' denied by Grok read-only capability mode"
                )))
            } else {
                None
            }
        }
        // Always-approve still never bypasses: it only broadens auto-allow of
        // decisions that managed policy already permits. Bridge still consults
        // PermissionHandle (yolo only when explicitly opted-in + allowed).
        ClaudeCapabilityMode::ReadWrite
        | ClaudeCapabilityMode::Execute
        | ClaudeCapabilityMode::All
        | ClaudeCapabilityMode::AlwaysApprove => None,
    }
}

/// Map a Grok [`Decision`] to the Claude MCP response. Cancel / policy deny
/// / reject all fail closed as deny. Never returns allow on unknown.
pub fn decision_to_response(
    decision: xai_grok_workspace::permission::Decision,
) -> ClaudePermissionResponse {
    use xai_grok_workspace::permission::Decision;
    match decision {
        Decision::Allow => ClaudePermissionResponse::allow(),
        Decision::Ask => {
            // Should not reach here — Ask is resolved by the prompter into
            // Allow/Reject/Cancelled. Fail closed.
            ClaudePermissionResponse::deny("permission ask unresolved; denying")
        }
        Decision::FollowupMessage(msg) => ClaudePermissionResponse::deny(msg),
        Decision::Reject(msg) => ClaudePermissionResponse::deny(msg),
        Decision::PolicyDeny(msg) => ClaudePermissionResponse::deny(format!("policy deny: {msg}")),
        Decision::Cancelled => ClaudePermissionResponse::deny_cancelled(),
    }
}

// ---------------------------------------------------------------------------
// Broker trait + parent socket server
// ---------------------------------------------------------------------------

/// Host-side decision surface for one Claude permission request.
#[async_trait]
pub trait ClaudePermissionBroker: Send + Sync {
    async fn decide(
        &self,
        request: ClaudePermissionRequest,
        cancel: CancellationToken,
    ) -> ClaudePermissionResponse;
}

/// Fail-closed broker used when no PermissionHandle is wired.
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

/// Policy-aware broker: capability precheck → optional PermissionHandle → deny.
pub struct PolicyPermissionBroker {
    pub mode: ClaudeCapabilityMode,
    pub permission: Option<xai_grok_workspace::permission::PermissionHandle>,
    /// When true and PermissionHandle is in yolo / allow-all, broader allow is
    /// permitted for AlwaysApprove mode only (still no bypassPermissions flag).
    pub always_approve_opt_in: bool,
    /// Audit sink: one event per request (tests capture this).
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
            always_approve_opt_in: false,
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

    pub fn with_always_approve_opt_in(mut self, enabled: bool) -> Self {
        self.always_approve_opt_in = enabled;
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

        // Always-approve mode: only broaden when user opted in AND handle is
        // already allow-all / yolo. Never invent bypass.
        if matches!(self.mode, ClaudeCapabilityMode::AlwaysApprove)
            && self.always_approve_opt_in
            && self
                .permission
                .as_ref()
                .map(|p| p.is_yolo_mode())
                .unwrap_or(false)
        {
            self.record(
                &request.tool_name,
                request.tool_use_id.clone(),
                "allow_always_approve_opt_in",
            )
            .await;
            return ClaudePermissionResponse::allow();
        }

        let Some(handle) = self.permission.as_ref() else {
            // No handle → ask path unavailable → fail closed.
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
}

impl ScriptedBroker {
    pub fn new(responses: Vec<ClaudePermissionResponse>) -> Self {
        Self {
            responses: Mutex::new(responses.into()),
            seen: Mutex::new(Vec::new()),
        }
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
        self.responses
            .lock()
            .await
            .pop_front()
            .unwrap_or_else(|| ClaudePermissionResponse::deny("scripted broker exhausted"))
    }
}

// ---------------------------------------------------------------------------
// Broker UDS protocol (parent ↔ MCP child)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
struct BrokerWireRequest {
    id: u64,
    request: ClaudePermissionRequest,
}

#[derive(Debug, Serialize, Deserialize)]
struct BrokerWireResponse {
    id: u64,
    response: ClaudePermissionResponse,
}

/// Parent-side permission broker server (one socket, many sequential requests).
pub struct PermissionBrokerServer {
    socket_path: PathBuf,
    broker: Arc<dyn ClaudePermissionBroker>,
    cancel: CancellationToken,
    /// Serialize: one Claude permission → one UI decision.
    gate: Mutex<()>,
    shutdown: AtomicBool,
    accept_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl PermissionBrokerServer {
    /// Bind a new UDS broker.
    ///
    /// Socket path is kept **short** (`$TMPDIR/gcbXXXXXXXX.sock`) because
    /// macOS `sockaddr_un.sun_path` is limited (~104 bytes). `dir` is still
    /// created for sibling MCP config artifacts.
    pub async fn start(
        dir: &Path,
        broker: Arc<dyn ClaudePermissionBroker>,
        cancel: CancellationToken,
    ) -> Result<Arc<Self>, ExternalRuntimeError> {
        std::fs::create_dir_all(dir).map_err(|e| {
            ExternalRuntimeError::new(
                ExternalRuntimeErrorKind::Transport,
                format!("permission bridge dir: {e}"),
                Some(ExternalAgentKind::ClaudeCli),
            )
        })?;
        // Short absolute path under temp_dir to stay within SUN_LEN.
        let short_id = format!("{:x}", uuid::Uuid::new_v4().as_u128() & 0xffff_ffff_ffff);
        let socket_path = std::env::temp_dir().join(format!("gcb{short_id}.sock"));
        let _ = std::fs::remove_file(&socket_path);
        let listener = UnixListener::bind(&socket_path).map_err(|e| {
            ExternalRuntimeError::new(
                ExternalRuntimeErrorKind::Transport,
                format!("permission bridge bind ({}): {e}", socket_path.display()),
                Some(ExternalAgentKind::ClaudeCli),
            )
        })?;

        let server = Arc::new(Self {
            socket_path,
            broker,
            cancel: cancel.clone(),
            gate: Mutex::new(()),
            shutdown: AtomicBool::new(false),
            accept_task: Mutex::new(None),
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

    async fn handle_client(&self, stream: UnixStream) {
        let (reader, mut writer) = stream.into_split();
        let mut lines = TokioBufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if self.shutdown.load(Ordering::Relaxed) || self.cancel.is_cancelled() {
                break;
            }
            if line.len() > MAX_BRIDGE_MESSAGE_BYTES {
                continue;
            }
            let Ok(req) = serde_json::from_str::<BrokerWireRequest>(&line) else {
                continue;
            };
            // One-at-a-time UI decision.
            let _gate = self.gate.lock().await;
            let response = if self.cancel.is_cancelled() {
                ClaudePermissionResponse::deny_cancelled()
            } else {
                self.broker
                    .decide(req.request, self.cancel.child_token())
                    .await
            };
            let wire = BrokerWireResponse {
                id: req.id,
                response,
            };
            if let Ok(mut out) = serde_json::to_string(&wire) {
                out.push('\n');
                let _ = writer.write_all(out.as_bytes()).await;
                let _ = writer.flush().await;
            }
        }
    }

    /// Stop accepting and remove the socket file.
    pub async fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
        self.cancel.cancel();
        if let Some(h) = self.accept_task.lock().await.take() {
            h.abort();
        }
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

impl Drop for PermissionBrokerServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

// ---------------------------------------------------------------------------
// MCP child: stdio JSON-RPC (Content-Length framing) + UDS forward
// ---------------------------------------------------------------------------

/// Env var the MCP child uses to find the parent broker socket.
pub const BRIDGE_SOCKET_ENV: &str = "GROK_CLAUDE_PERMISSION_BRIDGE_SOCKET";

/// Run the MCP permission-bridge child (blocking). Returns process exit code.
///
/// Called from binary `main` via [`maybe_run_permission_bridge_subprocess`].
pub fn run_permission_bridge_child(socket_path: &Path) -> i32 {
    match run_mcp_stdio_bridge(socket_path) {
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
    // Prefer --socket flag; fall back to env.
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
        i += 1;
    }
    if socket.is_none() {
        if let Ok(p) = std::env::var(BRIDGE_SOCKET_ENV) {
            if !p.is_empty() {
                socket = Some(PathBuf::from(p));
            }
        }
    }
    let Some(path) = socket else {
        let _ = writeln_stderr("permission bridge: missing --socket");
        return Some(2);
    };
    Some(run_permission_bridge_child(&path))
}

fn writeln_stderr(msg: &str) {
    use std::io::Write;
    let _ = writeln!(std::io::stderr(), "{msg}");
}

fn run_mcp_stdio_bridge(socket_path: &Path) -> Result<(), String> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut next_broker_id = 1u64;

    loop {
        let msg = match read_mcp_message(&mut reader)? {
            Some(m) => m,
            None => break, // EOF
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
            "notifications/initialized" | "initialized" => {
                // Notification — no response.
            }
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
                    write_mcp_tool_text(&mut stdout, id, &err_text, true)?;
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
                        write_mcp_tool_text(&mut stdout, id, &err_text, true)?;
                        continue;
                    }
                };
                let response = match forward_to_broker(socket_path, next_broker_id, request) {
                    Ok(r) => r,
                    Err(e) => ClaudePermissionResponse::deny(format!(
                        "permission bridge forward failed: {e}"
                    )),
                };
                next_broker_id = next_broker_id.saturating_add(1);
                let is_err = matches!(response, ClaudePermissionResponse::Deny { .. });
                write_mcp_tool_text(&mut stdout, id, &response.to_mcp_text(), is_err)?;
            }
            "ping" => {
                write_mcp_result(&mut stdout, id, json!({}))?;
            }
            "" if msg.get("result").is_some() || msg.get("error").is_some() => {
                // Response to something we didn't send — ignore.
            }
            other => {
                // Unknown method — JSON-RPC method-not-found when it has an id.
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

/// Test helper: async forward (avoids current-thread runtime deadlock with
/// blocking std sockets).
#[cfg(test)]
pub async fn forward_to_broker_for_test(
    socket_path: &Path,
    request: ClaudePermissionRequest,
) -> Result<ClaudePermissionResponse, String> {
    forward_to_broker_async(socket_path, 1, request).await
}

/// Async client used by in-process tests (parent and child share a runtime).
#[cfg(test)]
async fn forward_to_broker_async(
    socket_path: &Path,
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
    let stream = stream.ok_or_else(|| format!("connect broker socket: {last}"))?;
    let (reader, mut writer) = stream.into_split();
    let wire = BrokerWireRequest { id, request };
    let mut line = serde_json::to_string(&wire).map_err(|e| e.to_string())?;
    line.push('\n');
    writer
        .write_all(line.as_bytes())
        .await
        .map_err(|e| format!("broker write: {e}"))?;
    writer
        .flush()
        .await
        .map_err(|e| format!("broker flush: {e}"))?;
    let mut lines = TokioBufReader::new(reader).lines();
    let resp_line = tokio::time::timeout(Duration::from_secs(5), lines.next_line())
        .await
        .map_err(|_| "broker read timeout".to_owned())?
        .map_err(|e| format!("broker read: {e}"))?
        .ok_or_else(|| "broker closed without response".to_owned())?;
    let wire: BrokerWireResponse =
        serde_json::from_str(resp_line.trim()).map_err(|e| format!("broker parse: {e}"))?;
    Ok(wire.response)
}

fn forward_to_broker(
    socket_path: &Path,
    id: u64,
    request: ClaudePermissionRequest,
) -> Result<ClaudePermissionResponse, String> {
    use std::os::unix::net::UnixStream as StdUnixStream;
    // Retry connect briefly so the accept loop can start.
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
    // Generous timeouts; avoid zero-timeout nonblocking reads (EAGAIN on macOS).
    let _ = stream.set_read_timeout(Some(DEFAULT_PERMISSION_TIMEOUT));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));
    let wire = BrokerWireRequest { id, request };
    let mut line = serde_json::to_string(&wire).map_err(|e| e.to_string())?;
    line.push('\n');
    stream
        .write_all(line.as_bytes())
        .map_err(|e| format!("broker write: {e}"))?;
    stream.flush().map_err(|e| format!("broker flush: {e}"))?;
    let mut reader = BufReader::new(stream);
    let mut resp_line = String::new();
    // Retry read on transient WouldBlock / Interrupted.
    for _ in 0..40 {
        resp_line.clear();
        match reader.read_line(&mut resp_line) {
            Ok(0) => return Err("broker closed without response".into()),
            Ok(_) if !resp_line.trim().is_empty() => break,
            Ok(_) => continue,
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut
                    || e.kind() == std::io::ErrorKind::Interrupted =>
            {
                std::thread::sleep(Duration::from_millis(25));
                continue;
            }
            Err(e) => return Err(format!("broker read: {e}")),
        }
    }
    if resp_line.trim().is_empty() {
        return Err("broker closed without response".into());
    }
    let wire: BrokerWireResponse =
        serde_json::from_str(resp_line.trim()).map_err(|e| format!("broker parse: {e}"))?;
    Ok(wire.response)
}

fn read_mcp_message<R: BufRead>(reader: &mut R) -> Result<Option<Value>, String> {
    // Content-Length framing (MCP stdio).
    let mut headers = String::new();
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
        return Err("MCP message too large".into());
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

/// Build MCP server config entry for the bridge child (path + socket).
pub fn bridge_mcp_server_entry(
    host_executable: &Path,
    socket_path: &Path,
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
    let mut env = serde_json::Map::new();
    env.insert(
        BRIDGE_SOCKET_ENV.into(),
        json!(socket_path.to_string_lossy().into_owned()),
    );
    server.insert("env".into(), Value::Object(env));
    let mut servers = HashMap::new();
    servers.insert(BRIDGE_MCP_SERVER_NAME.to_owned(), Value::Object(server));
    servers
}

/// Request id counter for tests.
#[allow(dead_code)]
static TEST_REQ_ID: AtomicU64 = AtomicU64::new(1);

#[cfg(test)]
mod tests {
    use super::super::capability_mode::ClaudeCapabilityMode;
    use super::*;

    #[test]
    fn parses_permission_request_schema() {
        let v = json!({
            "tool_name": "Edit",
            "tool_use_id": "tu1",
            "input": { "file_path": "/tmp/a.rs", "old_string": "a", "new_string": "b" }
        });
        let r = parse_permission_request(&v).unwrap();
        assert_eq!(r.tool_name, "Edit");
        assert_eq!(r.tool_use_id.as_deref(), Some("tu1"));
    }

    #[test]
    fn missing_tool_name_fails_closed() {
        assert!(parse_permission_request(&json!({"input": {}})).is_err());
    }

    #[test]
    fn response_json_shapes() {
        let allow = ClaudePermissionResponse::allow().to_mcp_text();
        assert!(allow.contains("\"behavior\":\"allow\""));
        let deny = ClaudePermissionResponse::deny("nope").to_mcp_text();
        assert!(deny.contains("\"behavior\":\"deny\""));
        assert!(deny.contains("nope"));
    }

    #[test]
    fn read_only_precheck_denies_edit_and_bash() {
        assert!(capability_precheck(ClaudeCapabilityMode::ReadOnly, "Edit").is_some());
        assert!(capability_precheck(ClaudeCapabilityMode::ReadOnly, "Bash").is_some());
        assert!(capability_precheck(ClaudeCapabilityMode::ReadOnly, "Write").is_some());
        assert!(capability_precheck(ClaudeCapabilityMode::ReadOnly, "Read").is_none());
        assert!(capability_precheck(ClaudeCapabilityMode::ReadOnly, "Grep").is_none());
    }

    #[test]
    fn map_tools_to_access_kinds() {
        use xai_grok_workspace::permission::AccessKind;
        assert!(matches!(
            map_claude_tool_to_access("Read", &json!({"file_path": "a.rs"})),
            AccessKind::Read(Some(ref p)) if p == "a.rs"
        ));
        assert!(matches!(
            map_claude_tool_to_access("Bash", &json!({"command": "ls"})),
            AccessKind::Bash(ref c) if c == "ls"
        ));
        assert!(matches!(
            map_claude_tool_to_access("Edit", &json!({"file_path": "x"})),
            AccessKind::Edit(ref p) if p == "x"
        ));
        assert!(matches!(
            map_claude_tool_to_access("TotallyUnknown", &json!({})),
            AccessKind::MCPTool { .. }
        ));
    }

    #[tokio::test]
    async fn scripted_broker_one_request_one_decision() {
        let broker = ScriptedBroker::new(vec![
            ClaudePermissionResponse::allow(),
            ClaudePermissionResponse::deny("no"),
        ]);
        let r1 = broker
            .decide(
                ClaudePermissionRequest {
                    tool_use_id: Some("1".into()),
                    tool_name: "Read".into(),
                    input: json!({}),
                    decision_reason: None,
                    blocked_path: None,
                    agent_id: None,
                },
                CancellationToken::new(),
            )
            .await;
        assert!(matches!(r1, ClaudePermissionResponse::Allow { .. }));
        let r2 = broker
            .decide(
                ClaudePermissionRequest {
                    tool_use_id: Some("2".into()),
                    tool_name: "Bash".into(),
                    input: json!({}),
                    decision_reason: None,
                    blocked_path: None,
                    agent_id: None,
                },
                CancellationToken::new(),
            )
            .await;
        assert!(matches!(r2, ClaudePermissionResponse::Deny { .. }));
        assert_eq!(broker.seen.lock().await.len(), 2);
    }

    #[tokio::test]
    async fn cancel_fails_closed() {
        let broker = DenyAllBroker;
        let cancel = CancellationToken::new();
        cancel.cancel();
        // DenyAll ignores cancel but Policy maps cancel; test Policy path.
        let policy = PolicyPermissionBroker::new(ClaudeCapabilityMode::All);
        let r = policy
            .decide(
                ClaudePermissionRequest {
                    tool_use_id: None,
                    tool_name: "Read".into(),
                    input: json!({}),
                    decision_reason: None,
                    blocked_path: None,
                    agent_id: None,
                },
                cancel,
            )
            .await;
        assert!(matches!(
            r,
            ClaudePermissionResponse::Deny {
                interrupt: Some(true),
                ..
            }
        ));
        let _ = broker;
    }

    #[tokio::test]
    async fn policy_broker_read_only_denies_edit_without_handle() {
        let policy = PolicyPermissionBroker::new(ClaudeCapabilityMode::ReadOnly);
        let r = policy
            .decide(
                ClaudePermissionRequest {
                    tool_use_id: Some("e1".into()),
                    tool_name: "Edit".into(),
                    input: json!({"file_path": "a.rs"}),
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
        assert_eq!(audit[0].outcome, "deny_capability_mode");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn broker_socket_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let broker = Arc::new(ScriptedBroker::new(vec![ClaudePermissionResponse::allow()]));
        let cancel = CancellationToken::new();
        let server = PermissionBrokerServer::start(dir.path(), broker.clone(), cancel.clone())
            .await
            .unwrap();
        let resp = forward_to_broker_async(
            server.socket_path(),
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
        .expect("broker roundtrip");
        assert!(matches!(resp, ClaudePermissionResponse::Allow { .. }));
        server.shutdown().await;
    }

    #[test]
    fn permission_prompt_tool_flag_format() {
        assert_eq!(
            permission_prompt_tool_flag(),
            "mcp__grok-permission__permission_prompt"
        );
    }

    #[test]
    fn decision_mapping_fail_closed() {
        use xai_grok_workspace::permission::Decision;
        assert!(matches!(
            decision_to_response(Decision::Allow),
            ClaudePermissionResponse::Allow { .. }
        ));
        assert!(matches!(
            decision_to_response(Decision::PolicyDeny("x".into())),
            ClaudePermissionResponse::Deny { .. }
        ));
        assert!(matches!(
            decision_to_response(Decision::Cancelled),
            ClaudePermissionResponse::Deny {
                interrupt: Some(true),
                ..
            }
        ));
        assert!(matches!(
            decision_to_response(Decision::Ask),
            ClaudePermissionResponse::Deny { .. }
        ));
    }
}
