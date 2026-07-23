//! Native Codex agent bridge using the official `codex app-server` protocol.
//!
//! This is intentionally separate from inference providers. OpenAI and
//! OpenRouter supply model responses to Grok Build's own agent loop; Codex
//! app-server is already a complete coding agent and owns its tool loop. The
//! bridge uses JSONL over stdio and the credentials cached by `codex login`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, Command};
use tokio_util::sync::CancellationToken;

const INITIALIZE_ID: u64 = 1;
const THREAD_START_ID: u64 = 2;
const TURN_START_ID: u64 = 3;
const STDERR_LIMIT: usize = 64 * 1024;
const MODEL_LIST_INITIAL_ID: u64 = 2;
const MAX_MODEL_LIST_PAGES: usize = 100;
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

#[derive(Clone, Debug)]
pub struct CodexRunRequest {
    /// Executable followed by arguments, normally
    /// `["codex", "app-server", "--stdio"]`.
    pub command: Vec<String>,
    pub model: String,
    pub cwd: PathBuf,
    pub prompt: String,
    pub reasoning_effort: Option<String>,
    pub output_schema: Option<Value>,
    pub sandbox: CodexSandboxMode,
    /// When present, reconnect to this persisted Codex thread instead of
    /// creating a text-replayed replacement thread.
    pub resume_thread_id: Option<String>,
    /// Per-message deadline. It resets whenever app-server emits a frame.
    pub idle_timeout: Duration,
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
            reasoning_effort: None,
            output_schema: None,
            sandbox: CodexSandboxMode::WorkspaceWrite,
            resume_thread_id: None,
            idle_timeout: Duration::from_secs(300),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexRunResult {
    pub thread_id: String,
    pub turn_id: String,
    pub output: String,
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
    const VERSION: u8 = 1;

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

    let (thread_method, thread_params) = match request.resume_thread_id.as_deref() {
        Some(thread_id) => (
            "thread/resume",
            json!({
                "threadId": thread_id,
                "model": request.model,
                "cwd": request.cwd,
                "approvalPolicy": "never",
                "sandbox": request.sandbox.as_wire()
            }),
        ),
        None => (
            "thread/start",
            json!({
                "model": request.model,
                "cwd": request.cwd,
                "approvalPolicy": "never",
                "sandbox": request.sandbox.as_wire(),
                // The app-server owns durable rollout persistence. An
                // ephemeral thread cannot be resumed after this process exits.
                "ephemeral": false
            }),
        ),
    };
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
        let message = read_message(reader, request.idle_timeout, cancellation).await?;
        if message.get("method").is_some() && message.get("id").is_some() {
            reject_server_request(stdin, &message).await?;
            continue;
        }
        if message.get("id").and_then(Value::as_u64) == Some(TURN_START_ID) {
            ensure_response_ok(&message)?;
            accumulator.turn_id = message
                .pointer("/result/turn/id")
                .and_then(Value::as_str)
                .map(str::to_string);
        }
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

    fn output(self) -> String {
        if self.deltas.is_empty() {
            self.completed_text.unwrap_or_default()
        } else {
            self.deltas
        }
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
      printf '%s\n' '{"method":"item/agentMessage/delta","params":{"delta":"native result"}}'
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

        let result = run_codex_turn(request, CancellationToken::new())
            .await
            .expect("fake app-server should complete");
        assert_eq!(result.thread_id, "thread-test");
        assert_eq!(result.turn_id, "turn-test");
        assert_eq!(result.output, "native result");
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
