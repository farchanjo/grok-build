//! Native Codex agent bridge using the official `codex app-server` protocol.
//!
//! This is intentionally separate from inference providers. OpenAI and
//! OpenRouter supply model responses to Grok Build's own agent loop; Codex
//! app-server is already a complete coding agent and owns its tool loop. The
//! bridge uses JSONL over stdio and the credentials cached by `codex login`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, Command};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::agent::codex_stream::{self, CodexStreamEvent};

const INITIALIZE_ID: u64 = 1;
const THREAD_START_ID: u64 = 2;
const TURN_START_ID: u64 = 3;
const STDERR_LIMIT: usize = 64 * 1024;
const MODEL_LIST_INITIAL_ID: u64 = 2;
const MAX_MODEL_LIST_PAGES: usize = 100;
const TURN_INTERRUPT_ID: u64 = 4;
pub(crate) const PRIMARY_THREAD_STATE_FILE: &str = "codex_thread.json";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CodexSandboxMode {
    ReadOnly,
    #[default]
    WorkspaceWrite,
    DangerFullAccess,
}

impl CodexSandboxMode {
    pub(crate) fn as_wire(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::WorkspaceWrite => "workspace-write",
            Self::DangerFullAccess => "danger-full-access",
        }
    }

    pub(crate) fn from_wire(value: &str) -> Option<Self> {
        match value {
            "read-only" => Some(Self::ReadOnly),
            "workspace-write" => Some(Self::WorkspaceWrite),
            "danger-full-access" => Some(Self::DangerFullAccess),
            _ => None,
        }
    }
}

/// A host function advertised to Codex through `thread/start.dynamicTools`.
#[derive(Clone, Debug, PartialEq)]
pub struct CodexDynamicToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Sanitized result returned to Codex for one host dynamic-tool call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexDynamicToolResult {
    pub success: bool,
    pub text: String,
}

#[async_trait::async_trait]
pub trait CodexDynamicToolHandler: Send + Sync + std::fmt::Debug + 'static {
    async fn call(&self, tool: &str, call_id: &str, arguments: Value) -> CodexDynamicToolResult;
}

/// Dynamic-tool adapter backed by the session's finalized tool registry.
///
/// Codex sees stable task-lifecycle names while the adapter resolves whatever
/// client-facing aliases the active agent registered. Validation, depth
/// limits, model routing, polling, and cancellation therefore stay in the
/// existing tool implementations instead of being duplicated in this bridge.
pub struct CodexHostTaskTools {
    bridge: Arc<xai_grok_tools::bridge::ToolBridge>,
    routes: HashMap<String, String>,
}

impl std::fmt::Debug for CodexHostTaskTools {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodexHostTaskTools")
            .field("routes", &self.routes)
            .finish_non_exhaustive()
    }
}

impl CodexHostTaskTools {
    pub async fn from_bridge(
        bridge: Arc<xai_grok_tools::bridge::ToolBridge>,
    ) -> Result<(Vec<CodexDynamicToolSpec>, Arc<Self>), CodexAppServerError> {
        use xai_grok_tools::types::tool::ToolKind;

        let definitions = bridge.tool_definitions().await;
        let requested = [
            ("task", ToolKind::Task),
            ("get_task_output", ToolKind::BackgroundTaskAction),
            ("kill_task", ToolKind::KillTaskAction),
        ];
        let mut routes = HashMap::new();
        let mut specs = Vec::with_capacity(requested.len());
        for (external_name, kind) in requested {
            let registered_name = bridge.tool_for_kind(kind).await.ok_or_else(|| {
                CodexAppServerError::Protocol(format!(
                    "active agent does not provide the required '{external_name}' task tool"
                ))
            })?;
            let definition = definitions
                .iter()
                .find(|definition| definition.function.name == registered_name)
                .ok_or_else(|| {
                    CodexAppServerError::Protocol(format!(
                        "active agent did not advertise a schema for '{registered_name}'"
                    ))
                })?;
            routes.insert(external_name.to_owned(), registered_name);
            specs.push(CodexDynamicToolSpec {
                name: external_name.to_owned(),
                description: definition
                    .function
                    .description
                    .clone()
                    .unwrap_or_else(|| format!("Host-managed {external_name} operation.")),
                input_schema: definition.function.parameters.clone(),
            });
        }
        let handler = Arc::new(Self { bridge, routes });
        Ok((specs, handler))
    }
}

#[async_trait::async_trait]
impl CodexDynamicToolHandler for CodexHostTaskTools {
    async fn call(&self, tool: &str, call_id: &str, arguments: Value) -> CodexDynamicToolResult {
        let Some(registered_name) = self.routes.get(tool) else {
            return CodexDynamicToolResult {
                success: false,
                text: format!("Unknown host dynamic tool '{tool}'."),
            };
        };
        match self.bridge.call(registered_name, arguments, call_id).await {
            Ok(result) => CodexDynamicToolResult {
                success: true,
                text: result.prompt_text,
            },
            Err(error) => CodexDynamicToolResult {
                success: false,
                text: format!("{error}"),
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct CodexRunRequest {
    /// Executable followed by arguments, normally
    /// `["codex", "app-server", "--stdio"]`.
    pub command: Vec<String>,
    pub model: String,
    pub cwd: PathBuf,
    pub prompt: String,
    /// Provider-neutral engineering and role instructions. These augment the
    /// native Codex base instructions; they never replace its safety contract.
    pub developer_instructions: Option<String>,
    pub reasoning_effort: Option<String>,
    pub output_schema: Option<Value>,
    pub sandbox: CodexSandboxMode,
    /// When present, reconnect to this persisted Codex thread instead of
    /// creating a text-replayed replacement thread.
    pub resume_thread_id: Option<String>,
    /// Host-owned functions exposed to a primary Codex thread.
    pub dynamic_tools: Vec<CodexDynamicToolSpec>,
    pub dynamic_tool_handler: Option<Arc<dyn CodexDynamicToolHandler>>,
    /// Disable Codex's own collaboration tools for this managed thread. The
    /// host coordinator remains the sole owner of subagent depth and routing.
    pub disable_native_multi_agent: bool,
    /// Per-message deadline. It resets whenever app-server emits a frame.
    pub idle_timeout: Duration,
    /// Optional live event sink. When set, app-server notifications are
    /// classified and forwarded while the turn is still running so the host
    /// can stream text, reasoning, tools, and plan updates natively.
    pub stream_tx: Option<mpsc::UnboundedSender<CodexStreamEvent>>,
}

impl CodexRunRequest {
    pub fn new(model: impl Into<String>, cwd: PathBuf, prompt: impl Into<String>) -> Self {
        Self {
            command: vec![
                "codex".to_string(),
                "app-server".to_string(),
                "--stdio".to_string(),
            ],
            model: model.into(),
            cwd,
            prompt: prompt.into(),
            developer_instructions: None,
            reasoning_effort: None,
            output_schema: None,
            sandbox: CodexSandboxMode::WorkspaceWrite,
            resume_thread_id: None,
            dynamic_tools: Vec::new(),
            dynamic_tool_handler: None,
            disable_native_multi_agent: false,
            idle_timeout: Duration::from_secs(300),
            stream_tx: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CodexTokenUsage {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_output_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CodexToolUsage {
    pub command_executions: u32,
    pub file_changes: u32,
    pub mcp_calls: u32,
    pub dynamic_tool_calls: u32,
    pub collaboration_calls: u32,
    pub other_calls: u32,
}

impl CodexToolUsage {
    pub fn total(&self) -> u32 {
        self.command_executions
            .saturating_add(self.file_changes)
            .saturating_add(self.mcp_calls)
            .saturating_add(self.dynamic_tool_calls)
            .saturating_add(self.collaboration_calls)
            .saturating_add(self.other_calls)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexRunResult {
    pub thread_id: String,
    pub turn_id: String,
    pub output: String,
    pub usage: CodexTokenUsage,
    pub tools: CodexToolUsage,
}

/// Request for the authenticated Codex app-server model catalogue.
///
/// The launched process inherits the user's environment, including the
/// credentials established by `codex login`.
#[derive(Clone, Debug)]
pub struct CodexModelListRequest {
    /// Executable followed by arguments, normally
    /// `['codex', 'app-server', '--stdio']`.
    pub command: Vec<String>,
    /// Hidden models are excluded unless this is explicitly enabled.
    pub include_hidden: bool,
    /// Optional app-server page size.
    pub page_size: Option<u32>,
    /// Per-message deadline. It resets whenever app-server emits a frame.
    pub idle_timeout: Duration,
}

impl Default for CodexModelListRequest {
    fn default() -> Self {
        Self {
            command: vec![
                "codex".to_string(),
                "app-server".to_string(),
                "--stdio".to_string(),
            ],
            include_hidden: false,
            page_size: None,
            idle_timeout: Duration::from_secs(30),
        }
    }
}

/// A model advertised by the authenticated Codex app-server instance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexCatalogModel {
    pub id: String,
    pub model: String,
    pub display_name: String,
    pub description: String,
    pub supported_reasoning_efforts: Vec<String>,
    pub default_reasoning_effort: String,
    pub is_default: bool,
}

/// Durable link between a Grok Build session and the Codex thread that owns
/// its native conversation history.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub(crate) struct PrimaryCodexThreadState {
    version: u8,
    provider_id: String,
    thread_id: String,
    model: String,
    cwd: PathBuf,
}

impl PrimaryCodexThreadState {
    // Version 2 starts a fresh persisted Codex thread so the dynamic host-task
    // declarations introduced by this bridge are stored on thread/start.
    const VERSION: u8 = 2;

    pub(crate) fn new(
        provider_id: impl Into<String>,
        thread_id: impl Into<String>,
        model: impl Into<String>,
        cwd: PathBuf,
    ) -> Self {
        Self {
            version: Self::VERSION,
            provider_id: provider_id.into(),
            thread_id: thread_id.into(),
            model: model.into(),
            cwd,
        }
    }

    pub(crate) fn matching_thread_id(
        &self,
        provider_id: &str,
        model: &str,
        cwd: &Path,
    ) -> Option<&str> {
        (self.version == Self::VERSION
            && self.provider_id == provider_id
            && self.model == model
            && self.cwd == cwd
            && !self.thread_id.trim().is_empty())
        .then_some(self.thread_id.as_str())
    }
}

pub(crate) fn load_primary_thread_state(
    session_dir: &Path,
) -> std::io::Result<Option<PrimaryCodexThreadState>> {
    let path = session_dir.join(PRIMARY_THREAD_STATE_FILE);
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}

pub(crate) fn save_primary_thread_state(
    session_dir: &Path,
    state: &PrimaryCodexThreadState,
) -> std::io::Result<()> {
    std::fs::create_dir_all(session_dir)?;
    let bytes = serde_json::to_vec_pretty(state)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    crate::session::storage::write_bytes_atomic(
        &session_dir.join(PRIMARY_THREAD_STATE_FILE),
        &bytes,
    )
}

#[derive(Debug, thiserror::Error)]
pub enum CodexAppServerError {
    #[error("Codex app-server command is empty")]
    EmptyCommand,
    #[error("failed to start Codex app-server: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("Codex app-server I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("Codex app-server returned invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Codex app-server protocol error: {0}")]
    Protocol(String),
    #[error("Codex app-server request failed: {0}")]
    Server(String),
    #[error("Codex agent was cancelled")]
    Cancelled,
    #[error("Codex app-server was idle for {0:?}")]
    IdleTimeout(Duration),
}

/// Run a fresh or persisted Codex thread and return its final assistant message.
///
/// The child inherits the user's environment and Codex home so an existing
/// `codex login` session (including ChatGPT subscription auth) is reused.
pub async fn run_codex_turn(
    request: CodexRunRequest,
    cancellation: CancellationToken,
) -> Result<CodexRunResult, CodexAppServerError> {
    let (program, args) = request
        .command
        .split_first()
        .ok_or(CodexAppServerError::EmptyCommand)?;
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(CodexAppServerError::Spawn)?;
    let mut stdin = child.stdin.take().ok_or_else(|| {
        CodexAppServerError::Protocol("app-server stdin was not piped".to_string())
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        CodexAppServerError::Protocol("app-server stdout was not piped".to_string())
    })?;
    let stderr = child.stderr.take();
    let mut stderr_task = tokio::spawn(async move {
        let Some(mut stderr) = stderr else {
            return String::new();
        };
        let mut bytes = Vec::new();
        let _ = stderr
            .take(STDERR_LIMIT as u64)
            .read_to_end(&mut bytes)
            .await;
        String::from_utf8_lossy(&bytes).trim().to_string()
    });
    let mut reader = BufReader::new(stdout);

    let result = run_protocol(&request, &cancellation, &mut stdin, &mut reader).await;
    // The npm-distributed `codex` executable is a launcher for the native
    // binary. Close our inherited pipes before waiting: otherwise the native
    // app-server can remain alive after its launcher exits, keeping stderr
    // open and deadlocking this cleanup path.
    drop(stdin);
    drop(reader);
    let _ = child.start_kill();
    let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
    let stderr = match tokio::time::timeout(Duration::from_secs(2), &mut stderr_task).await {
        Ok(joined) => joined.unwrap_or_default(),
        Err(_) => {
            stderr_task.abort();
            String::new()
        }
    };

    result.map_err(|error| attach_stderr(error, &stderr))
}

/// Fetch every visible model page from the authenticated native Codex
/// app-server. This is a JSONL transport only; it does not use ACP or replay
/// any conversation data.
pub async fn list_codex_models(
    request: CodexModelListRequest,
    cancellation: CancellationToken,
) -> Result<Vec<CodexCatalogModel>, CodexAppServerError> {
    let (program, args) = request
        .command
        .split_first()
        .ok_or(CodexAppServerError::EmptyCommand)?;
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(CodexAppServerError::Spawn)?;
    let mut stdin = child.stdin.take().ok_or_else(|| {
        CodexAppServerError::Protocol("app-server stdin was not piped".to_string())
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        CodexAppServerError::Protocol("app-server stdout was not piped".to_string())
    })?;
    let stderr = child.stderr.take();
    let mut stderr_task = tokio::spawn(async move {
        let Some(mut stderr) = stderr else {
            return String::new();
        };
        let mut bytes = Vec::new();
        let _ = stderr
            .take(STDERR_LIMIT as u64)
            .read_to_end(&mut bytes)
            .await;
        String::from_utf8_lossy(&bytes).trim().to_string()
    });
    let mut reader = BufReader::new(stdout);

    let result = list_models_protocol(&request, &cancellation, &mut stdin, &mut reader).await;
    drop(stdin);
    drop(reader);
    let _ = child.start_kill();
    let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
    let stderr = match tokio::time::timeout(Duration::from_secs(2), &mut stderr_task).await {
        Ok(joined) => joined.unwrap_or_default(),
        Err(_) => {
            stderr_task.abort();
            String::new()
        }
    };

    result.map_err(|error| attach_stderr(error, &stderr))
}

async fn list_models_protocol(
    request: &CodexModelListRequest,
    cancellation: &CancellationToken,
    stdin: &mut ChildStdin,
    reader: &mut BufReader<tokio::process::ChildStdout>,
) -> Result<Vec<CodexCatalogModel>, CodexAppServerError> {
    send(
        stdin,
        &json!({
            "method": "initialize",
            "id": INITIALIZE_ID,
            "params": {
                "clientInfo": {
                    "name": "grok_build",
                    "title": "Grok Build",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }
        }),
    )
    .await?;
    wait_for_response(
        INITIALIZE_ID,
        request.idle_timeout,
        cancellation,
        stdin,
        reader,
    )
    .await?;
    send(stdin, &json!({"method": "initialized", "params": {}})).await?;

    let mut cursor: Option<String> = None;
    let mut seen_cursors = HashSet::new();
    let mut models = Vec::new();
    for page in 0..MAX_MODEL_LIST_PAGES {
        let mut params = json!({"includeHidden": request.include_hidden});
        if let Some(cursor) = cursor.as_deref() {
            params["cursor"] = Value::String(cursor.to_string());
        }
        if let Some(limit) = request.page_size {
            params["limit"] = Value::from(limit);
        }
        let id = MODEL_LIST_INITIAL_ID + page as u64;
        send(
            stdin,
            &json!({"method": "model/list", "id": id, "params": params}),
        )
        .await?;
        let response =
            wait_for_response(id, request.idle_timeout, cancellation, stdin, reader).await?;
        let data = response
            .pointer("/result/data")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                CodexAppServerError::Protocol(
                    "model/list response did not contain result.data array".to_string(),
                )
            })?;
        for model in data {
            if !request.include_hidden && model.get("hidden").and_then(Value::as_bool) == Some(true)
            {
                continue;
            }
            models.push(parse_catalog_model(model)?);
        }

        cursor = match response.pointer("/result/nextCursor") {
            None | Some(Value::Null) => return Ok(models),
            Some(Value::String(cursor)) if cursor.is_empty() => {
                return Err(CodexAppServerError::Protocol(
                    "model/list returned an empty nextCursor".to_string(),
                ));
            }
            Some(Value::String(cursor)) => Some(cursor.clone()),
            Some(_) => {
                return Err(CodexAppServerError::Protocol(
                    "model/list returned a non-string nextCursor".to_string(),
                ));
            }
        };
        if !seen_cursors.insert(cursor.clone().unwrap_or_default()) {
            return Err(CodexAppServerError::Protocol(
                "model/list returned a repeated nextCursor".to_string(),
            ));
        }
    }
    Err(CodexAppServerError::Protocol(format!(
        "model/list exceeded the maximum of {MAX_MODEL_LIST_PAGES} pages"
    )))
}

fn parse_catalog_model(model: &Value) -> Result<CodexCatalogModel, CodexAppServerError> {
    let string = |name: &str| {
        model.get(name).and_then(Value::as_str).ok_or_else(|| {
            CodexAppServerError::Protocol(format!("model/list entry is missing string '{name}'"))
        })
    };
    let efforts = model
        .get("supportedReasoningEfforts")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            CodexAppServerError::Protocol(
                "model/list entry is missing supportedReasoningEfforts array".to_string(),
            )
        })?
        .iter()
        .map(|effort| {
            effort
                .get("reasoningEffort")
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| {
                    CodexAppServerError::Protocol(
                        "model/list reasoning effort is missing reasoningEffort".to_string(),
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let is_default = model
        .get("isDefault")
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            CodexAppServerError::Protocol(
                "model/list entry is missing boolean 'isDefault'".to_string(),
            )
        })?;
    Ok(CodexCatalogModel {
        id: string("id")?.to_string(),
        model: string("model")?.to_string(),
        display_name: string("displayName")?.to_string(),
        description: string("description")?.to_string(),
        supported_reasoning_efforts: efforts,
        default_reasoning_effort: string("defaultReasoningEffort")?.to_string(),
        is_default,
    })
}

async fn run_protocol(
    request: &CodexRunRequest,
    cancellation: &CancellationToken,
    stdin: &mut ChildStdin,
    reader: &mut BufReader<tokio::process::ChildStdout>,
) -> Result<CodexRunResult, CodexAppServerError> {
    send(
        stdin,
        &json!({
            "method": "initialize",
            "id": INITIALIZE_ID,
            "params": {
                "clientInfo": {
                    "name": "grok_build",
                    "title": "Grok Build",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": {
                    "experimentalApi": true
                }
            }
        }),
    )
    .await?;
    wait_for_response(
        INITIALIZE_ID,
        request.idle_timeout,
        cancellation,
        stdin,
        reader,
    )
    .await?;
    send(stdin, &json!({"method": "initialized", "params": {}})).await?;

    let dynamic_tools = request
        .dynamic_tools
        .iter()
        .map(|tool| {
            json!({
                "type": "function",
                "name": tool.name,
                "description": tool.description,
                "inputSchema": tool.input_schema,
            })
        })
        .collect::<Vec<_>>();
    let mut common_thread_params = json!({
        "model": request.model,
        "cwd": request.cwd,
        "approvalPolicy": "never",
        "sandbox": request.sandbox.as_wire()
    });
    if let Some(instructions) = request.developer_instructions.as_deref() {
        common_thread_params["developerInstructions"] = Value::String(instructions.to_owned());
    }
    // App-server accepts dynamicTools only on thread/start. They are persisted
    // with the thread and remain available when a later process resumes it.
    if request.resume_thread_id.is_none() && !dynamic_tools.is_empty() {
        common_thread_params["dynamicTools"] = Value::Array(dynamic_tools);
    }
    if request.disable_native_multi_agent {
        common_thread_params["config"] = json!({
            "features": {
                "multi_agent": false,
                "multi_agent_v2": false
            }
        });
    }

    let (thread_method, mut thread_params) = match request.resume_thread_id.as_deref() {
        Some(thread_id) => ("thread/resume", common_thread_params),
        None => ("thread/start", common_thread_params),
    };
    if let Some(thread_id) = request.resume_thread_id.as_deref() {
        thread_params["threadId"] = Value::String(thread_id.to_owned());
    } else {
        // The app-server owns durable rollout persistence. An ephemeral
        // thread cannot be resumed after this process exits.
        thread_params["ephemeral"] = Value::Bool(false);
    }
    send(
        stdin,
        &json!({
            "method": thread_method,
            "id": THREAD_START_ID,
            "params": thread_params
        }),
    )
    .await?;
    let thread_response = wait_for_response(
        THREAD_START_ID,
        request.idle_timeout,
        cancellation,
        stdin,
        reader,
    )
    .await?;
    let thread_id = thread_response
        .pointer("/result/thread/id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            CodexAppServerError::Protocol(
                "thread/start response did not contain result.thread.id".to_string(),
            )
        })?
        .to_string();
    if let Some(expected) = request.resume_thread_id.as_deref()
        && thread_id != expected
    {
        return Err(CodexAppServerError::Protocol(format!(
            "thread/resume returned thread id '{thread_id}', expected '{expected}'"
        )));
    }

    let mut turn_params = json!({
        "threadId": thread_id,
        "input": [{"type": "text", "text": request.prompt}],
        "approvalPolicy": "never",
        "cwd": request.cwd,
        "model": request.model
    });
    if let Some(effort) = request.reasoning_effort.as_deref() {
        turn_params["effort"] = Value::String(effort.to_string());
    }
    if let Some(schema) = request.output_schema.clone() {
        turn_params["outputSchema"] = schema;
    }
    send(
        stdin,
        &json!({
            "method": "turn/start",
            "id": TURN_START_ID,
            "params": turn_params
        }),
    )
    .await?;

    let mut accumulator = TurnAccumulator::default();
    loop {
        let message = match read_message(reader, request.idle_timeout, cancellation).await {
            Ok(message) => message,
            Err(CodexAppServerError::Cancelled) => {
                let _ = send(
                    stdin,
                    &json!({
                        "method": "turn/interrupt",
                        "id": TURN_INTERRUPT_ID,
                        "params": {
                            "threadId": thread_id,
                            "turnId": accumulator.turn_id
                        }
                    }),
                )
                .await;
                return Err(CodexAppServerError::Cancelled);
            }
            Err(error) => return Err(error),
        };
        if message.get("method").is_some() && message.get("id").is_some() {
            handle_server_request(
                stdin,
                &message,
                request.dynamic_tool_handler.as_deref(),
                &mut accumulator,
            )
            .await?;
            continue;
        }
        if message.get("id").and_then(Value::as_u64) == Some(TURN_START_ID) {
            ensure_response_ok(&message)?;
            accumulator.turn_id = message
                .pointer("/result/turn/id")
                .and_then(Value::as_str)
                .map(str::to_string);
        }
        emit_stream_events(&request.stream_tx, &message);
        if accumulator.apply(&message)? {
            break;
        }
    }

    let turn_id = accumulator.turn_id.clone().unwrap_or_default();
    let output = accumulator.output();
    Ok(CodexRunResult {
        thread_id,
        turn_id,
        output,
        usage: accumulator.usage,
        tools: accumulator.tools,
    })
}

async fn wait_for_response(
    expected_id: u64,
    idle_timeout: Duration,
    cancellation: &CancellationToken,
    stdin: &mut ChildStdin,
    reader: &mut BufReader<tokio::process::ChildStdout>,
) -> Result<Value, CodexAppServerError> {
    loop {
        let message = read_message(reader, idle_timeout, cancellation).await?;
        if message.get("method").is_some() && message.get("id").is_some() {
            reject_server_request(stdin, &message).await?;
            continue;
        }
        if message.get("id").and_then(Value::as_u64) == Some(expected_id) {
            ensure_response_ok(&message)?;
            return Ok(message);
        }
    }
}

async fn send(stdin: &mut ChildStdin, message: &Value) -> Result<(), CodexAppServerError> {
    let mut line = serde_json::to_vec(message)?;
    line.push(b'\n');
    stdin.write_all(&line).await?;
    stdin.flush().await?;
    Ok(())
}

async fn read_message(
    reader: &mut BufReader<tokio::process::ChildStdout>,
    idle_timeout: Duration,
    cancellation: &CancellationToken,
) -> Result<Value, CodexAppServerError> {
    let mut line = String::new();
    tokio::select! {
        _ = cancellation.cancelled() => Err(CodexAppServerError::Cancelled),
        read = tokio::time::timeout(idle_timeout, reader.read_line(&mut line)) => {
            match read {
                Err(_) => Err(CodexAppServerError::IdleTimeout(idle_timeout)),
                Ok(Err(error)) => Err(CodexAppServerError::Io(error)),
                Ok(Ok(0)) => Err(CodexAppServerError::Protocol(
                    "app-server closed stdout before completing the turn".to_string(),
                )),
                Ok(Ok(_)) => Ok(serde_json::from_str(line.trim())?),
            }
        }
    }
}

async fn reject_server_request(
    stdin: &mut ChildStdin,
    message: &Value,
) -> Result<(), CodexAppServerError> {
    let Some(id) = message.get("id").cloned() else {
        return Ok(());
    };
    send(
        stdin,
        &json!({
            "id": id,
            "error": {
                "code": -32601,
                "message": "Grok Build's Codex bridge runs with approvalPolicy=never and does not accept interactive server requests"
            }
        }),
    )
    .await
}

async fn handle_server_request(
    stdin: &mut ChildStdin,
    message: &Value,
    handler: Option<&dyn CodexDynamicToolHandler>,
    accumulator: &mut TurnAccumulator,
) -> Result<(), CodexAppServerError> {
    let method = message.get("method").and_then(Value::as_str);
    if method != Some("item/tool/call") {
        return reject_server_request(stdin, message).await;
    }
    let Some(id) = message.get("id").cloned() else {
        return Ok(());
    };
    let Some(handler) = handler else {
        return send(
            stdin,
            &json!({
                "id": id,
                "result": {
                    "success": false,
                    "contentItems": [{
                        "type": "inputText",
                        "text": "Host dynamic tools are unavailable for this Codex thread."
                    }]
                }
            }),
        )
        .await;
    };
    let tool = message
        .pointer("/params/tool")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let call_id = message
        .pointer("/params/callId")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let arguments = message
        .pointer("/params/arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    if tool.is_empty() || call_id.is_empty() {
        return send(
            stdin,
            &json!({
                "id": id,
                "result": {
                    "success": false,
                    "contentItems": [{
                        "type": "inputText",
                        "text": "Codex sent an invalid dynamic-tool request."
                    }]
                }
            }),
        )
        .await;
    }

    let result = handler.call(tool, call_id, arguments).await;
    accumulator.tools.dynamic_tool_calls = accumulator.tools.dynamic_tool_calls.saturating_add(1);
    send(
        stdin,
        &json!({
            "id": id,
            "result": {
                "success": result.success,
                "contentItems": [{
                    "type": "inputText",
                    "text": result.text
                }]
            }
        }),
    )
    .await
}

fn emit_stream_events(
    stream_tx: &Option<mpsc::UnboundedSender<CodexStreamEvent>>,
    message: &Value,
) {
    let Some(tx) = stream_tx else {
        return;
    };
    for event in codex_stream::classify_notification(message) {
        // A closed receiver means the host abandoned the UI stream; keep
        // accumulating the turn so the final result remains available.
        if tx.send(event).is_err() {
            break;
        }
    }
}

fn ensure_response_ok(message: &Value) -> Result<(), CodexAppServerError> {
    if let Some(error) = message.get("error") {
        let rendered = error
            .get("message")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| error.to_string());
        return Err(CodexAppServerError::Server(rendered));
    }
    Ok(())
}

#[derive(Default)]
struct TurnAccumulator {
    turn_id: Option<String>,
    deltas: String,
    completed_text: Option<String>,
    usage: CodexTokenUsage,
    tools: CodexToolUsage,
    counted_items: HashSet<String>,
}

impl TurnAccumulator {
    /// Returns `true` when the terminal turn notification was consumed.
    fn apply(&mut self, message: &Value) -> Result<bool, CodexAppServerError> {
        match message.get("method").and_then(Value::as_str) {
            Some("item/agentMessage/delta") => {
                if let Some(delta) = message.pointer("/params/delta").and_then(Value::as_str) {
                    self.deltas.push_str(delta);
                }
            }
            Some("item/completed") => {
                let item = message.pointer("/params/item");
                self.count_tool_item(item);
                if item
                    .and_then(|item| item.get("type"))
                    .and_then(Value::as_str)
                    == Some("agentMessage")
                    && let Some(text) = item
                        .and_then(|item| item.get("text"))
                        .and_then(Value::as_str)
                {
                    self.completed_text = Some(text.to_string());
                }
            }
            Some("thread/tokenUsage/updated") => {
                if let Some(usage) = message
                    .pointer("/params/tokenUsage/last")
                    .or_else(|| message.pointer("/params/tokenUsage/total"))
                {
                    self.usage = parse_token_usage(usage);
                }
            }
            Some("turn/completed") => {
                let status = message
                    .pointer("/params/turn/status")
                    .and_then(Value::as_str)
                    .unwrap_or("failed");
                if status != "completed" {
                    let detail = message
                        .pointer("/params/turn/error/message")
                        .and_then(Value::as_str)
                        .unwrap_or(status);
                    return Err(CodexAppServerError::Server(detail.to_string()));
                }
                if self.turn_id.is_none() {
                    self.turn_id = message
                        .pointer("/params/turn/id")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                }
                return Ok(true);
            }
            Some("error") => {
                let detail = message
                    .pointer("/params/error/message")
                    .or_else(|| message.pointer("/params/message"))
                    .and_then(Value::as_str)
                    .unwrap_or("unknown Codex app-server error");
                return Err(CodexAppServerError::Server(detail.to_string()));
            }
            _ => {}
        }
        Ok(false)
    }

    fn count_tool_item(&mut self, item: Option<&Value>) {
        let Some(item) = item else {
            return;
        };
        let item_type = item.get("type").and_then(Value::as_str).unwrap_or_default();
        if item_type == "agentMessage" {
            return;
        }
        let item_id = item
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| format!("{item_type}:{}", self.counted_items.len()));
        if !self.counted_items.insert(item_id) {
            return;
        }
        match item_type {
            "commandExecution" => {
                self.tools.command_executions = self.tools.command_executions.saturating_add(1);
            }
            "fileChange" => {
                self.tools.file_changes = self.tools.file_changes.saturating_add(1);
            }
            "mcpToolCall" => {
                self.tools.mcp_calls = self.tools.mcp_calls.saturating_add(1);
            }
            "dynamicToolCall" => {
                // Counted when the corresponding server request is handled so
                // app-server versions that omit a completed item still report
                // the host call exactly once.
            }
            "collabAgentToolCall" | "collabToolCall" => {
                self.tools.collaboration_calls = self.tools.collaboration_calls.saturating_add(1);
            }
            "" => {}
            _ => {
                self.tools.other_calls = self.tools.other_calls.saturating_add(1);
            }
        }
    }

    fn output(&self) -> String {
        if self.deltas.is_empty() {
            self.completed_text.clone().unwrap_or_default()
        } else {
            self.deltas.clone()
        }
    }
}

fn parse_token_usage(value: &Value) -> CodexTokenUsage {
    let token = |field: &str| value.get(field).and_then(Value::as_u64).unwrap_or(0);
    let input_tokens = token("inputTokens");
    let cached_input_tokens = token("cachedInputTokens");
    let output_tokens = token("outputTokens");
    let reasoning_output_tokens = token("reasoningOutputTokens");
    let total_tokens = value
        .get("totalTokens")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| {
            input_tokens
                .saturating_add(output_tokens)
                .saturating_add(reasoning_output_tokens)
        });
    CodexTokenUsage {
        input_tokens,
        cached_input_tokens,
        output_tokens,
        reasoning_output_tokens,
        total_tokens,
    }
}

fn attach_stderr(error: CodexAppServerError, stderr: &str) -> CodexAppServerError {
    if stderr.is_empty() {
        return error;
    }
    match error {
        CodexAppServerError::Protocol(message) => {
            CodexAppServerError::Protocol(format!("{message}; stderr: {stderr}"))
        }
        CodexAppServerError::Server(message) => {
            CodexAppServerError::Server(format!("{message}; stderr: {stderr}"))
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct RecordingDynamicTool {
        calls: Arc<std::sync::Mutex<Vec<(String, String, Value)>>>,
    }

    #[async_trait::async_trait]
    impl CodexDynamicToolHandler for RecordingDynamicTool {
        async fn call(
            &self,
            tool: &str,
            call_id: &str,
            arguments: Value,
        ) -> CodexDynamicToolResult {
            self.calls
                .lock()
                .unwrap()
                .push((tool.to_owned(), call_id.to_owned(), arguments));
            CodexDynamicToolResult {
                success: true,
                text: "task accepted".to_owned(),
            }
        }
    }

    #[test]
    fn accumulates_streamed_agent_message() {
        let mut acc = TurnAccumulator::default();
        assert!(
            !acc.apply(&json!({
                "method": "item/agentMessage/delta",
                "params": {"delta": "hello "}
            }))
            .unwrap()
        );
        assert!(
            !acc.apply(&json!({
                "method": "item/agentMessage/delta",
                "params": {"delta": "world"}
            }))
            .unwrap()
        );
        assert!(
            acc.apply(&json!({
                "method": "turn/completed",
                "params": {"turn": {"id": "turn-1", "status": "completed"}}
            }))
            .unwrap()
        );
        assert_eq!(acc.turn_id.as_deref(), Some("turn-1"));
        assert_eq!(acc.output(), "hello world");
    }

    #[test]
    fn completed_item_is_fallback_when_deltas_are_suppressed() {
        let mut acc = TurnAccumulator::default();
        acc.apply(&json!({
            "method": "item/completed",
            "params": {"item": {"type": "agentMessage", "text": "final"}}
        }))
        .unwrap();
        assert_eq!(acc.output(), "final");
    }

    #[test]
    fn failed_turn_preserves_server_error() {
        let mut acc = TurnAccumulator::default();
        let error = acc
            .apply(&json!({
                "method": "turn/completed",
                "params": {
                    "turn": {
                        "id": "turn-1",
                        "status": "failed",
                        "error": {"message": "model unavailable"}
                    }
                }
            }))
            .unwrap_err();
        assert!(error.to_string().contains("model unavailable"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn completes_jsonl_stdio_handshake_with_app_server_process() {
        let script = r#"
while IFS= read -r line; do
  case "$line" in
    *'"id":1'*) printf '%s\n' '{"id":1,"result":{}}' ;;
    *'"id":2'*) printf '%s\n' '{"id":2,"result":{"thread":{"id":"thread-test"}}}' ;;
    *'"id":3'*)
      printf '%s\n' '{"id":3,"result":{"turn":{"id":"turn-test"}}}'
      printf '%s\n' '{"method":"turn/started","params":{"turn":{"id":"turn-test","status":"inProgress"}}}'
      printf '%s\n' '{"method":"item/started","params":{"item":{"id":"cmd-1","type":"commandExecution","command":"echo hi","status":"inProgress"}}}'
      printf '%s\n' '{"method":"item/commandExecution/outputDelta","params":{"itemId":"cmd-1","delta":"hi\n"}}'
      printf '%s\n' '{"method":"item/completed","params":{"item":{"id":"cmd-1","type":"commandExecution","command":"echo hi","status":"completed","aggregatedOutput":"hi\n","exitCode":0}}}'
      printf '%s\n' '{"method":"item/reasoning/summaryTextDelta","params":{"itemId":"r1","delta":"thinking "}}'
      printf '%s\n' '{"method":"item/agentMessage/delta","params":{"itemId":"m1","delta":"native "}}'
      printf '%s\n' '{"method":"item/agentMessage/delta","params":{"itemId":"m1","delta":"result"}}'
      printf '%s\n' '{"method":"turn/completed","params":{"turn":{"id":"turn-test","status":"completed"}}}'
      break
      ;;
  esac
done
"#;
        let mut request =
            CodexRunRequest::new("gpt-test", std::env::current_dir().unwrap(), "do the work");
        request.command = vec!["sh".to_string(), "-c".to_string(), script.to_string()];
        request.idle_timeout = Duration::from_secs(2);
        let (stream_tx, mut stream_rx) = mpsc::unbounded_channel();
        request.stream_tx = Some(stream_tx);

        let result = run_codex_turn(request, CancellationToken::new())
            .await
            .expect("fake app-server should complete");
        assert_eq!(result.thread_id, "thread-test");
        assert_eq!(result.turn_id, "turn-test");
        assert_eq!(result.output, "native result");
        assert_eq!(result.tools.command_executions, 1);

        let mut events = Vec::new();
        while let Ok(event) = stream_rx.try_recv() {
            events.push(event);
        }
        assert!(
            events
                .iter()
                .any(|e| matches!(e, CodexStreamEvent::TurnStarted { .. })),
            "expected turn/started: {events:?}"
        );
        assert!(
            events.iter().any(|e| matches!(
                e,
                CodexStreamEvent::ItemStarted(item)
                    if item.kind == crate::agent::codex_stream::CodexItemKind::CommandExecution
            )),
            "expected command item/started: {events:?}"
        );
        assert!(
            events.iter().any(|e| matches!(
                e,
                CodexStreamEvent::ItemOutputDelta { item_id, text }
                    if item_id == "cmd-1" && text == "hi\n"
            )),
            "expected command output delta: {events:?}"
        );
        assert!(
            events.iter().any(|e| matches!(
                e,
                CodexStreamEvent::ReasoningDelta { text, .. } if text == "thinking "
            )),
            "expected reasoning delta: {events:?}"
        );
        let agent_text: String = events
            .iter()
            .filter_map(|e| match e {
                CodexStreamEvent::AgentMessageDelta { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(agent_text, "native result");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn advertises_and_dispatches_host_tools_with_usage() {
        let script = r#"
while IFS= read -r line; do
  case "$line" in
    *'"id":1'*'"experimentalApi":true'*) printf '%s\n' '{"id":1,"result":{}}' ;;
    *'"id":1'*) printf '%s\n' '{"id":1,"error":{"message":"experimental API capability missing"}}'; break ;;
    *'"id":2'*'"developerInstructions":"neutral engineer"'*'"dynamicTools"'*'"multi_agent":false'*)
      printf '%s\n' '{"id":2,"result":{"thread":{"id":"thread-tools"}}}'
      ;;
    *'"id":2'*) printf '%s\n' '{"id":2,"error":{"message":"managed thread contract missing"}}'; break ;;
    *'"id":3'*)
      printf '%s\n' '{"id":3,"result":{"turn":{"id":"turn-tools"}}}'
      printf '%s\n' '{"method":"item/tool/call","id":99,"params":{"threadId":"thread-tools","turnId":"turn-tools","callId":"call-1","tool":"task","arguments":{"prompt":"inspect"}}}'
      ;;
    *'"id":99'*'"success":true'*'"task accepted"'*)
      printf '%s\n' '{"method":"thread/tokenUsage/updated","params":{"threadId":"thread-tools","tokenUsage":{"last":{"inputTokens":100,"cachedInputTokens":20,"outputTokens":30,"reasoningOutputTokens":5,"totalTokens":135}}}}'
      printf '%s\n' '{"method":"item/completed","params":{"item":{"id":"cmd-1","type":"commandExecution"}}}'
      printf '%s\n' '{"method":"item/agentMessage/delta","params":{"delta":"coordinated"}}'
      printf '%s\n' '{"method":"turn/completed","params":{"turn":{"id":"turn-tools","status":"completed"}}}'
      break
      ;;
  esac
done
"#;
        let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
        let handler = Arc::new(RecordingDynamicTool {
            calls: Arc::clone(&calls),
        });
        let mut request =
            CodexRunRequest::new("gpt-test", std::env::current_dir().unwrap(), "delegate");
        request.command = vec!["sh".to_owned(), "-c".to_owned(), script.to_owned()];
        request.developer_instructions = Some("neutral engineer".to_owned());
        request.dynamic_tools = vec![CodexDynamicToolSpec {
            name: "task".to_owned(),
            description: "delegate".to_owned(),
            input_schema: json!({"type": "object"}),
        }];
        request.dynamic_tool_handler = Some(handler);
        request.disable_native_multi_agent = true;
        request.idle_timeout = Duration::from_secs(2);

        let result = run_codex_turn(request, CancellationToken::new())
            .await
            .expect("managed dynamic tool turn should complete");
        assert_eq!(result.output, "coordinated");
        assert_eq!(result.usage.input_tokens, 100);
        assert_eq!(result.usage.cached_input_tokens, 20);
        assert_eq!(result.usage.output_tokens, 30);
        assert_eq!(result.usage.reasoning_output_tokens, 5);
        assert_eq!(result.usage.total_tokens, 135);
        assert_eq!(result.tools.dynamic_tool_calls, 1);
        assert_eq!(result.tools.command_executions, 1);
        assert_eq!(
            calls.lock().unwrap().as_slice(),
            &[(
                "task".to_owned(),
                "call-1".to_owned(),
                json!({"prompt": "inspect"})
            )]
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn resumes_persisted_thread_with_a_new_app_server_process() {
        let script = r#"
while IFS= read -r line; do
  case "$line" in
    *'"id":1'*) printf '%s\n' '{"id":1,"result":{}}' ;;
    *'"method":"thread/resume"'*'"excludeTurns"'*) printf '%s\n' '{"id":2,"error":{"message":"excludeTurns requires experimentalApi capability"}}'; break ;;
    *'"method":"thread/resume"'*) printf '%s\n' '{"id":2,"result":{"thread":{"id":"thread-persisted"}}}' ;;
    *'"method":"thread/start"'*) printf '%s\n' '{"id":2,"error":{"message":"must not create a new thread"}}' ;;
    *'"id":3'*)
      printf '%s\n' '{"id":3,"result":{"turn":{"id":"turn-resumed"}}}'
      printf '%s\n' '{"method":"item/agentMessage/delta","params":{"delta":"resumed natively"}}'
      printf '%s\n' '{"method":"turn/completed","params":{"turn":{"id":"turn-resumed","status":"completed"}}}'
      break
      ;;
  esac
done
"#;
        let mut request = CodexRunRequest::new(
            "gpt-test",
            std::env::current_dir().unwrap(),
            "continue the work",
        );
        request.command = vec!["sh".to_string(), "-c".to_string(), script.to_string()];
        request.resume_thread_id = Some("thread-persisted".to_string());
        request.idle_timeout = Duration::from_secs(2);

        let result = run_codex_turn(request, CancellationToken::new())
            .await
            .expect("fake app-server should resume the persisted thread");
        assert_eq!(result.thread_id, "thread-persisted");
        assert_eq!(result.turn_id, "turn-resumed");
        assert_eq!(result.output, "resumed natively");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn rejects_resume_when_server_cannot_find_the_thread() {
        let script = r#"
while IFS= read -r line; do
  case "$line" in
    *'"id":1'*) printf '%s\n' '{"id":1,"result":{}}' ;;
    *'"method":"thread/resume"'*) printf '%s\n' '{"id":2,"error":{"message":"thread not found"}}'; break ;;
  esac
done
"#;
        let mut request = CodexRunRequest::new("gpt-test", std::env::current_dir().unwrap(), "x");
        request.command = vec!["sh".to_string(), "-c".to_string(), script.to_string()];
        request.resume_thread_id = Some("missing-thread".to_string());
        request.idle_timeout = Duration::from_secs(2);

        let error = run_codex_turn(request, CancellationToken::new())
            .await
            .expect_err("missing persisted thread must fail closed");
        assert!(error.to_string().contains("thread not found"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn lists_visible_models_across_jsonl_pages() {
        let script = r#"
while IFS= read -r line; do
  case "$line" in
    *'"id":1'*) printf '%s\n' '{"id":1,"result":{}}' ;;
    *'"method":"model/list"'*'"cursor":"page-2"'*)
      printf '%s\n' '{"id":3,"result":{"data":[{"id":"codex-b","model":"gpt-5-codex-b","displayName":"Codex B","description":"second page","hidden":false,"isDefault":true,"defaultReasoningEffort":"high","supportedReasoningEfforts":[{"reasoningEffort":"medium","description":"balanced"},{"reasoningEffort":"high","description":"deep"}]}],"nextCursor":null}}'
      break
      ;;
    *'"method":"model/list"'*'"includeHidden":false'*)
      printf '%s\n' '{"id":2,"result":{"data":[{"id":"codex-a","model":"gpt-5-codex-a","displayName":"Codex A","description":"first page","hidden":false,"isDefault":false,"defaultReasoningEffort":"low","supportedReasoningEfforts":[{"reasoningEffort":"low","description":"fast"}]},{"id":"hidden","model":"hidden","displayName":"Hidden","description":"must not leak","hidden":true,"isDefault":false,"defaultReasoningEffort":"low","supportedReasoningEfforts":[{"reasoningEffort":"low","description":"fast"}]}],"nextCursor":"page-2"}}'
      ;;
    *'"method":"model/list"'*) printf '%s\n' '{"id":2,"error":{"message":"includeHidden=false required"}}'; break ;;
  esac
done
"#;
        let request = CodexModelListRequest {
            command: vec!["sh".to_string(), "-c".to_string(), script.to_string()],
            page_size: Some(1),
            idle_timeout: Duration::from_secs(2),
            ..Default::default()
        };

        let models = list_codex_models(request, CancellationToken::new())
            .await
            .expect("fake app-server should return the complete visible catalogue");
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "codex-a");
        assert_eq!(models[0].model, "gpt-5-codex-a");
        assert_eq!(models[0].display_name, "Codex A");
        assert_eq!(models[0].description, "first page");
        assert_eq!(models[0].supported_reasoning_efforts, vec!["low"]);
        assert_eq!(models[0].default_reasoning_effort, "low");
        assert!(!models[0].is_default);
        assert_eq!(models[1].id, "codex-b");
        assert_eq!(
            models[1].supported_reasoning_efforts,
            vec!["medium", "high"]
        );
        assert!(models[1].is_default);
    }

    #[test]
    fn primary_thread_state_roundtrips_and_matches_its_provider_context() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().join("workspace");
        let state = PrimaryCodexThreadState::new(
            "grok_build_codex",
            "thread-primary",
            "gpt-test",
            cwd.clone(),
        );

        save_primary_thread_state(dir.path(), &state).unwrap();
        let loaded = load_primary_thread_state(dir.path()).unwrap().unwrap();

        assert_eq!(loaded, state);
        assert_eq!(
            loaded.matching_thread_id("grok_build_codex", "gpt-test", &cwd),
            Some("thread-primary")
        );
        assert_eq!(
            loaded.matching_thread_id("another-provider", "gpt-test", &cwd),
            None
        );
        assert_eq!(
            loaded.matching_thread_id("grok_build_codex", "another-model", &cwd),
            None
        );
    }
}
