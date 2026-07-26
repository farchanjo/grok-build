//! [`ClaudeCliRuntime`] — process-backed [`ExternalAgentRuntime`] for Claude CLI.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use super::argv::ClaudeCliTurnArgv;
use super::auth;
use super::discovery::{self, ClaudeCliDiscovery, ClaudeCliDiscoveryError, MIN_CLAUDE_CLI_VERSION};
use super::gates;
use super::process::{self, ProcessLimits, TurnProcessError};
use super::protocol::{self, ProtocolError};
use crate::agent::execution_backend::ExternalAgentKind;
use crate::agent::external_runtime::{
    ExternalAgentRuntime, ExternalResultMetadata, ExternalRuntimeCapabilities,
    ExternalRuntimeEnvelope, ExternalRuntimeError, ExternalRuntimeErrorKind,
    ExternalRuntimeFactory, ExternalRuntimeStatus, ExternalRuntimeTurnEvent, ExternalStartRequest,
    ExternalTurnOutcome, ExternalTurnRequest, ExternalUsageMetadata,
};

/// Live Claude CLI runtime. Requires feature + runtime opt-in + successful probe.
pub struct ClaudeCliRuntime {
    configured_path: Option<PathBuf>,
    limits: ProcessLimits,
    /// Last successful discovery (path + version + caps).
    discovery: Mutex<Option<ClaudeCliDiscovery>>,
    /// Cancel token for the in-flight turn (if any).
    inflight_cancel: Mutex<Option<CancellationToken>>,
}

impl ClaudeCliRuntime {
    pub fn new(configured_path: Option<PathBuf>) -> Self {
        Self {
            configured_path,
            limits: ProcessLimits::default(),
            discovery: Mutex::new(None),
            inflight_cancel: Mutex::new(None),
        }
    }

    pub fn with_limits(mut self, limits: ProcessLimits) -> Self {
        self.limits = limits;
        self
    }

    fn gate_error() -> ExternalRuntimeError {
        if !gates::claude_cli_feature_compiled() {
            return ExternalRuntimeError::unavailable(ExternalAgentKind::ClaudeCli);
        }
        ExternalRuntimeError {
            kind: ExternalRuntimeErrorKind::Unavailable,
            message: format!(
                "Claude Agent CLI runtime is disabled. Set {}=1 (or true) to opt in \
                 for development builds that include the claude-cli-runtime feature.",
                gates::CLAUDE_CLI_ENV_OPT_IN
            ),
            agent_kind: Some(ExternalAgentKind::ClaudeCli),
        }
    }

    fn ensure_gates() -> Result<(), ExternalRuntimeError> {
        if !gates::claude_cli_both_gates_open() {
            return Err(Self::gate_error());
        }
        Ok(())
    }

    async fn ensure_discovery(&self) -> Result<ClaudeCliDiscovery, ExternalRuntimeError> {
        Self::ensure_gates()?;
        {
            let guard = self.discovery.lock().await;
            if let Some(d) = guard.as_ref() {
                return Ok(d.clone());
            }
        }
        let path_ref = self.configured_path.as_deref();
        let discovered = discovery::discover_and_probe(path_ref, None)
            .await
            .map_err(map_discovery_error)?;
        *self.discovery.lock().await = Some(discovered.clone());
        Ok(discovered)
    }

    /// Invalidate cached discovery (e.g. after binary path change).
    pub async fn clear_discovery_cache(&self) {
        *self.discovery.lock().await = None;
    }
}

fn map_discovery_error(e: ClaudeCliDiscoveryError) -> ExternalRuntimeError {
    let message = e.to_string();
    let kind = match &e {
        ClaudeCliDiscoveryError::NotFound { .. }
        | ClaudeCliDiscoveryError::InvalidPath { .. }
        | ClaudeCliDiscoveryError::NotExecutable { .. }
        | ClaudeCliDiscoveryError::VersionTooOld { .. }
        | ClaudeCliDiscoveryError::MissingCapability { .. } => {
            ExternalRuntimeErrorKind::Unavailable
        }
        ClaudeCliDiscoveryError::ProbeTimeout { .. } => ExternalRuntimeErrorKind::Transport,
        ClaudeCliDiscoveryError::ProbeFailed { .. }
        | ClaudeCliDiscoveryError::VersionParse { .. } => ExternalRuntimeErrorKind::Unavailable,
    };
    ExternalRuntimeError {
        kind,
        message,
        agent_kind: Some(ExternalAgentKind::ClaudeCli),
    }
}

fn map_process_error(e: TurnProcessError) -> ExternalRuntimeError {
    match e {
        TurnProcessError::Cancelled => ExternalRuntimeError {
            kind: ExternalRuntimeErrorKind::Cancelled,
            message: "Claude Agent CLI turn was cancelled".into(),
            agent_kind: Some(ExternalAgentKind::ClaudeCli),
        },
        TurnProcessError::Spawn(m) => ExternalRuntimeError {
            kind: ExternalRuntimeErrorKind::Transport,
            message: format!("failed to spawn Claude CLI: {m}"),
            agent_kind: Some(ExternalAgentKind::ClaudeCli),
        },
        other => ExternalRuntimeError {
            kind: ExternalRuntimeErrorKind::Transport,
            message: other.to_string(),
            agent_kind: Some(ExternalAgentKind::ClaudeCli),
        },
    }
}

fn map_protocol_error(e: ProtocolError) -> ExternalRuntimeError {
    ExternalRuntimeError {
        kind: ExternalRuntimeErrorKind::Transport,
        message: e.to_string(),
        agent_kind: Some(ExternalAgentKind::ClaudeCli),
    }
}

#[async_trait]
impl ExternalAgentRuntime for ClaudeCliRuntime {
    fn kind(&self) -> ExternalAgentKind {
        ExternalAgentKind::ClaudeCli
    }

    async fn probe(&self) -> Result<ExternalRuntimeCapabilities, ExternalRuntimeError> {
        let d = self.ensure_discovery().await?;
        Ok(ExternalRuntimeCapabilities {
            version: Some(d.version.to_string()),
            capabilities: d.capabilities.clone(),
            models: Vec::new(),
        })
    }

    async fn start(
        &self,
        request: ExternalStartRequest,
    ) -> Result<ExternalRuntimeEnvelope, ExternalRuntimeError> {
        let d = self.ensure_discovery().await?;
        let mut env = ExternalRuntimeEnvelope::for_kind(ExternalAgentKind::ClaudeCli);
        env.observed_version = Some(d.version.to_string());
        env.capabilities = d.capabilities.clone();
        env.selected_model = request.selected_model;
        env.reasoning_effort = request.reasoning_effort;
        env.token_budget = request.token_budget;
        env.cwd = Some(request.cwd);
        env.worktree_identity = request.worktree_identity;
        // First turn will allocate/pass --session-id; pointer filled after turn.
        env.session_pointer = None;
        env.validated().map_err(|e| ExternalRuntimeError {
            kind: ExternalRuntimeErrorKind::InvalidRequest,
            message: e.to_string(),
            agent_kind: Some(ExternalAgentKind::ClaudeCli),
        })
    }

    async fn resume(
        &self,
        envelope: &ExternalRuntimeEnvelope,
    ) -> Result<ExternalRuntimeEnvelope, ExternalRuntimeError> {
        let d = self.ensure_discovery().await?;
        if envelope.kind != ExternalAgentKind::ClaudeCli {
            return Err(ExternalRuntimeError {
                kind: ExternalRuntimeErrorKind::InvalidRequest,
                message: "envelope kind is not claude_cli".into(),
                agent_kind: Some(ExternalAgentKind::ClaudeCli),
            });
        }
        let mut env = envelope.clone();
        env.observed_version = Some(d.version.to_string());
        if env.session_pointer.as_ref().is_none_or(|s| s.is_empty()) {
            return Err(ExternalRuntimeError {
                kind: ExternalRuntimeErrorKind::InvalidRequest,
                message: "cannot resume Claude CLI session without a session pointer".into(),
                agent_kind: Some(ExternalAgentKind::ClaudeCli),
            });
        }
        env.validated().map_err(|e| ExternalRuntimeError {
            kind: ExternalRuntimeErrorKind::InvalidRequest,
            message: e.to_string(),
            agent_kind: Some(ExternalAgentKind::ClaudeCli),
        })
    }

    async fn turn(
        &self,
        envelope: &ExternalRuntimeEnvelope,
        request: ExternalTurnRequest,
    ) -> Result<ExternalTurnOutcome, ExternalRuntimeError> {
        let d = self.ensure_discovery().await?;

        let cancel = CancellationToken::new();
        {
            let mut slot = self.inflight_cancel.lock().await;
            if let Some(prev) = slot.take() {
                prev.cancel();
            }
            *slot = Some(cancel.clone());
        }

        let resume = envelope
            .session_pointer
            .as_ref()
            .filter(|s| !s.is_empty())
            .cloned();
        // First turn: generate a UUID session id for --session-id when no resume.
        let session_id = if resume.is_none() {
            Some(uuid::Uuid::new_v4().to_string())
        } else {
            None
        };

        let max_budget_usd = request.token_budget.map(|t| {
            // Rough USD estimate is not available; pass budget as-is only if
            // callers encoded USD cents? Spec: token_budget maps to
            // --max-budget-usd when set as whole dollars in u64.
            t as f64
        });

        let argv = ClaudeCliTurnArgv {
            executable: d.executable.clone(),
            prompt: request.prompt.clone(),
            model: request
                .selected_model
                .clone()
                .or_else(|| envelope.selected_model.clone()),
            effort: request
                .reasoning_effort
                .clone()
                .or_else(|| envelope.reasoning_effort.clone()),
            max_budget_usd,
            session_id: session_id.clone(),
            resume_session: resume.clone(),
            cwd: envelope.cwd.as_ref().map(PathBuf::from),
        };
        let plan = argv.build_plan();

        let outcome = process::run_turn_process(&plan, &self.limits, cancel.clone()).await;
        *self.inflight_cancel.lock().await = None;

        let outcome = match outcome {
            Ok(o) => o,
            Err(TurnProcessError::Cancelled) => {
                return Err(ExternalRuntimeError {
                    kind: ExternalRuntimeErrorKind::Cancelled,
                    message: "Claude Agent CLI turn was cancelled".into(),
                    agent_kind: Some(ExternalAgentKind::ClaudeCli),
                });
            }
            Err(e) => return Err(map_process_error(e)),
        };

        if outcome.cancelled {
            let partial = protocol::parse_turn_lines_allow_incomplete(&outcome.lines);
            let mut env = envelope.clone();
            if let Some(sid) = partial.session_id.or(session_id) {
                env.session_pointer = Some(sid);
            }
            env.observed_version = Some(d.version.to_string());
            return Ok(ExternalTurnOutcome {
                events: partial.events,
                envelope: env,
                result: Some(ExternalResultMetadata {
                    status: "cancelled".into(),
                    stop_reason: Some("cancelled".into()),
                }),
                usage: partial.usage,
            });
        }

        // Exit 143 without explicit cancel flag still maps to Cancelled.
        if outcome.exit_code == Some(143) || outcome.exit_signal == Some(15) {
            return Err(ExternalRuntimeError {
                kind: ExternalRuntimeErrorKind::Cancelled,
                message: "Claude Agent CLI exited with SIGTERM (143)".into(),
                agent_kind: Some(ExternalAgentKind::ClaudeCli),
            });
        }

        let parsed = protocol::parse_turn_lines(&outcome.lines).map_err(map_protocol_error)?;

        let mut env = envelope.clone();
        if let Some(sid) = parsed.session_id.clone().or(session_id) {
            env.session_pointer = Some(sid);
        }
        env.observed_version = Some(
            parsed
                .version
                .clone()
                .unwrap_or_else(|| d.version.to_string()),
        );
        if !parsed.capabilities.is_empty() {
            env.capabilities = parsed.capabilities.clone();
        }
        if let Some(model) = parsed.model.clone() {
            env.selected_model = Some(model);
        }
        env.result = parsed.result.clone();
        env.usage = parsed.usage.clone();
        let env = env.validated().map_err(|e| ExternalRuntimeError {
            kind: ExternalRuntimeErrorKind::InvalidRequest,
            message: e.to_string(),
            agent_kind: Some(ExternalAgentKind::ClaudeCli),
        })?;

        // Non-zero exit without cancel and with error result → transport/other.
        if let Some(code) = outcome.exit_code {
            if code != 0 {
                if parsed
                    .result
                    .as_ref()
                    .is_some_and(|r| r.status == "error" || r.status == "error_during_execution")
                {
                    return Err(ExternalRuntimeError {
                        kind: ExternalRuntimeErrorKind::Other,
                        message: parsed
                            .events
                            .iter()
                            .find_map(|e| match e {
                                ExternalRuntimeTurnEvent::Error { message } => {
                                    Some(message.clone())
                                }
                                _ => None,
                            })
                            .unwrap_or_else(|| format!("Claude CLI exited with status {code}")),
                        agent_kind: Some(ExternalAgentKind::ClaudeCli),
                    });
                }
            }
        }

        Ok(ExternalTurnOutcome {
            events: parsed.events,
            envelope: env,
            result: parsed.result,
            usage: parsed.usage,
        })
    }

    async fn cancel(
        &self,
        _envelope: &ExternalRuntimeEnvelope,
    ) -> Result<(), ExternalRuntimeError> {
        if let Some(token) = self.inflight_cancel.lock().await.as_ref() {
            token.cancel();
        }
        Ok(())
    }

    async fn shutdown(
        &self,
        _envelope: &ExternalRuntimeEnvelope,
    ) -> Result<(), ExternalRuntimeError> {
        if let Some(token) = self.inflight_cancel.lock().await.take() {
            token.cancel();
        }
        Ok(())
    }

    fn status(&self, envelope: Option<&ExternalRuntimeEnvelope>) -> ExternalRuntimeStatus {
        if !gates::claude_cli_both_gates_open() {
            return ExternalRuntimeStatus::Unavailable;
        }
        if envelope.is_some() {
            ExternalRuntimeStatus::Idle
        } else {
            ExternalRuntimeStatus::Idle
        }
    }
}

/// Factory registered for [`ExternalAgentKind::ClaudeCli`] when the feature
/// is enabled. Still fails closed when runtime opt-in is off (via probe).
pub struct ClaudeCliRuntimeFactory {
    configured_path: Option<PathBuf>,
}

impl ClaudeCliRuntimeFactory {
    pub fn new(configured_path: Option<PathBuf>) -> Self {
        Self { configured_path }
    }

    pub fn from_env() -> Self {
        let configured = std::env::var(discovery::CLAUDE_CLI_PATH_ENV)
            .ok()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .map(PathBuf::from);
        Self::new(configured)
    }
}

impl ExternalRuntimeFactory for ClaudeCliRuntimeFactory {
    fn create(&self, kind: ExternalAgentKind) -> Arc<dyn ExternalAgentRuntime> {
        match kind {
            ExternalAgentKind::ClaudeCli => {
                Arc::new(ClaudeCliRuntime::new(self.configured_path.clone()))
            }
        }
    }
}

/// Optional auth status helper for provider UI (gated).
pub async fn auth_status_for_ui(
    configured_path: Option<&Path>,
) -> Result<auth::ClaudeCliAuthStatus, ExternalRuntimeError> {
    ClaudeCliRuntime::ensure_gates()?;
    let executable =
        discovery::discover_claude_executable(configured_path).map_err(map_discovery_error)?;
    auth::query_auth_status(&executable)
        .await
        .map_err(|e| ExternalRuntimeError {
            kind: match e {
                auth::ClaudeCliAuthError::Timeout => ExternalRuntimeErrorKind::Transport,
                auth::ClaudeCliAuthError::ProbeFailed { .. }
                | auth::ClaudeCliAuthError::Unparseable => ExternalRuntimeErrorKind::Auth,
            },
            message: e.to_string(),
            agent_kind: Some(ExternalAgentKind::ClaudeCli),
        })
}

/// Public min version constant re-export for status strings.
pub fn min_version_str() -> &'static str {
    MIN_CLAUDE_CLI_VERSION
}
