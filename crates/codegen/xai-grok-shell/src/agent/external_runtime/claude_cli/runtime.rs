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
use crate::agent::external_runtime::probe_cache;
use crate::agent::external_runtime::{
    ExternalAgentRuntime, ExternalRuntimeCapabilities, ExternalRuntimeEnvelope,
    ExternalRuntimeError, ExternalRuntimeErrorKind, ExternalRuntimeFactory, ExternalRuntimeStatus,
    ExternalRuntimeTurnEvent, ExternalStartRequest, ExternalTurnOutcome, ExternalTurnRequest,
};

/// Live Claude CLI runtime. Requires feature + runtime opt-in + successful probe.
pub struct ClaudeCliRuntime {
    configured_path: Option<PathBuf>,
    limits: ProcessLimits,
    /// Last successful discovery (path + version + caps + file identity).
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
        ExternalRuntimeError::new(
            ExternalRuntimeErrorKind::Unavailable,
            format!(
                "Claude Agent CLI runtime is disabled. Set {}=1 (or true) to opt in \
                 for development builds that include the claude-cli-runtime feature.",
                gates::CLAUDE_CLI_ENV_OPT_IN
            ),
            Some(ExternalAgentKind::ClaudeCli),
        )
    }

    fn ensure_gates() -> Result<(), ExternalRuntimeError> {
        if !gates::claude_cli_both_gates_open() {
            return Err(Self::gate_error());
        }
        Ok(())
    }

    /// Discover/probe, revalidate identity, and update the process-wide probe cache.
    async fn ensure_discovery(&self) -> Result<ClaudeCliDiscovery, ExternalRuntimeError> {
        Self::ensure_gates()?;
        {
            let guard = self.discovery.lock().await;
            if let Some(d) = guard.as_ref() {
                // Revalidate cached identity before reuse.
                match discovery::revalidate_executable(&d.executable, d) {
                    Ok(_) => {
                        probe_cache::record_probe_ok(d.version.to_string());
                        return Ok(d.clone());
                    }
                    Err(e) => {
                        // Fall through to re-probe after clearing stale cache.
                        let _ = e;
                    }
                }
            }
        }
        // Clear local cache if revalidation failed.
        *self.discovery.lock().await = None;

        let path_ref = self.configured_path.as_deref();
        match discovery::discover_and_probe(path_ref, None).await {
            Ok(discovered) => {
                // Revalidate immediately before returning (defense in depth).
                if let Err(e) =
                    discovery::revalidate_executable(&discovered.executable, &discovered)
                {
                    probe_cache::record_probe_failed(e.to_string());
                    return Err(map_discovery_error(e));
                }
                probe_cache::record_probe_ok(discovered.version.to_string());
                *self.discovery.lock().await = Some(discovered.clone());
                Ok(discovered)
            }
            Err(e) => {
                probe_cache::record_probe_failed(e.to_string());
                Err(map_discovery_error(e))
            }
        }
    }

    /// Invalidate cached discovery (e.g. after binary path change).
    pub async fn clear_discovery_cache(&self) {
        *self.discovery.lock().await = None;
        probe_cache::clear_probe_cache();
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
    ExternalRuntimeError::new(kind, message, Some(ExternalAgentKind::ClaudeCli))
}

fn map_process_error(e: TurnProcessError) -> ExternalRuntimeError {
    match e {
        TurnProcessError::Cancelled => ExternalRuntimeError::cancelled(
            ExternalAgentKind::ClaudeCli,
            "Claude Agent CLI turn was cancelled",
            None,
            Vec::new(),
        ),
        TurnProcessError::Spawn(m) => ExternalRuntimeError::new(
            ExternalRuntimeErrorKind::Transport,
            format!("failed to spawn Claude CLI: {m}"),
            Some(ExternalAgentKind::ClaudeCli),
        ),
        other => ExternalRuntimeError::new(
            ExternalRuntimeErrorKind::Transport,
            other.to_string(),
            Some(ExternalAgentKind::ClaudeCli),
        ),
    }
}

fn map_protocol_error(e: ProtocolError) -> ExternalRuntimeError {
    ExternalRuntimeError::new(
        ExternalRuntimeErrorKind::Transport,
        e.to_string(),
        Some(ExternalAgentKind::ClaudeCli),
    )
}

fn cancelled_with_partial(
    msg: &str,
    base: &ExternalRuntimeEnvelope,
    d: &ClaudeCliDiscovery,
    session_id: Option<String>,
    lines: &[String],
) -> ExternalRuntimeError {
    let partial = protocol::parse_turn_lines_allow_incomplete(lines);
    let mut env = base.clone();
    if let Some(sid) = partial.session_id.or(session_id) {
        env.session_pointer = Some(sid);
    }
    env.observed_version = Some(
        partial
            .version
            .clone()
            .unwrap_or_else(|| d.version.to_string()),
    );
    if !partial.capabilities.is_empty() {
        env.capabilities = partial.capabilities.clone();
    }
    if let Some(model) = partial.model {
        env.selected_model = Some(model);
    }
    env.usage = partial.usage.clone();
    let pointer = env.session_pointer.clone();
    let version = env.observed_version.clone();
    let caps = env.capabilities.clone();
    let events = partial.events;
    let env = match env.validated() {
        Ok(v) => Some(v),
        Err(_) => {
            // Minimal pointer-only envelope if full validation fails.
            let mut e = base.clone();
            e.session_pointer = pointer;
            e.observed_version = version;
            e.capabilities = caps;
            e.validated().ok()
        }
    };
    ExternalRuntimeError::cancelled(ExternalAgentKind::ClaudeCli, msg, env, events)
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
        // token_budget is a host token count — never mapped to --max-budget-usd.
        env.token_budget = request.token_budget;
        env.cwd = Some(request.cwd);
        env.worktree_identity = request.worktree_identity;
        env.session_pointer = None;
        env.validated().map_err(|e| {
            ExternalRuntimeError::new(
                ExternalRuntimeErrorKind::InvalidRequest,
                e.to_string(),
                Some(ExternalAgentKind::ClaudeCli),
            )
        })
    }

    async fn resume(
        &self,
        envelope: &ExternalRuntimeEnvelope,
    ) -> Result<ExternalRuntimeEnvelope, ExternalRuntimeError> {
        let d = self.ensure_discovery().await?;
        if envelope.kind != ExternalAgentKind::ClaudeCli {
            return Err(ExternalRuntimeError::new(
                ExternalRuntimeErrorKind::InvalidRequest,
                "envelope kind is not claude_cli",
                Some(ExternalAgentKind::ClaudeCli),
            ));
        }
        let mut env = envelope.clone();
        env.observed_version = Some(d.version.to_string());
        if env.session_pointer.as_ref().is_none_or(|s| s.is_empty()) {
            return Err(ExternalRuntimeError::new(
                ExternalRuntimeErrorKind::InvalidRequest,
                "cannot resume Claude CLI session without a session pointer",
                Some(ExternalAgentKind::ClaudeCli),
            ));
        }
        env.validated().map_err(|e| {
            ExternalRuntimeError::new(
                ExternalRuntimeErrorKind::InvalidRequest,
                e.to_string(),
                Some(ExternalAgentKind::ClaudeCli),
            )
        })
    }

    async fn turn(
        &self,
        envelope: &ExternalRuntimeEnvelope,
        request: ExternalTurnRequest,
    ) -> Result<ExternalTurnOutcome, ExternalRuntimeError> {
        let d = self.ensure_discovery().await?;
        // Revalidate immediately before spawn.
        discovery::revalidate_executable(&d.executable, &d).map_err(map_discovery_error)?;

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
        let session_id = if resume.is_none() {
            Some(uuid::Uuid::new_v4().to_string())
        } else {
            None
        };

        // MVP: do not pass --max-budget-usd. token_budget is host tokens, not USD.
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
            max_budget_usd: None,
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
                return Err(cancelled_with_partial(
                    "Claude Agent CLI turn was cancelled",
                    envelope,
                    &d,
                    session_id,
                    &[],
                ));
            }
            Err(e) => return Err(map_process_error(e)),
        };

        // Single terminal for cancellation: always ExternalRuntimeErrorKind::Cancelled
        // with best-effort partial envelope (session pointer from system/init).
        // Never Ok(EndTurn) / Completed.
        if outcome.cancelled || outcome.exit_code == Some(143) || outcome.exit_signal == Some(15) {
            return Err(cancelled_with_partial(
                "Claude Agent CLI turn was cancelled (SIGTERM/143)",
                envelope,
                &d,
                session_id,
                &outcome.lines,
            ));
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
        let env = env.validated().map_err(|e| {
            ExternalRuntimeError::new(
                ExternalRuntimeErrorKind::InvalidRequest,
                e.to_string(),
                Some(ExternalAgentKind::ClaudeCli),
            )
        })?;

        if let Some(code) = outcome.exit_code {
            if code != 0
                && parsed
                    .result
                    .as_ref()
                    .is_some_and(|r| r.status == "error" || r.status == "error_during_execution")
            {
                return Err(ExternalRuntimeError::new(
                    ExternalRuntimeErrorKind::Other,
                    parsed
                        .events
                        .iter()
                        .find_map(|e| match e {
                            ExternalRuntimeTurnEvent::Error { message } => Some(message.clone()),
                            _ => None,
                        })
                        .unwrap_or_else(|| format!("Claude CLI exited with status {code}")),
                    Some(ExternalAgentKind::ClaudeCli),
                ));
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

    fn status(&self, _envelope: Option<&ExternalRuntimeEnvelope>) -> ExternalRuntimeStatus {
        if !gates::claude_cli_both_gates_open() {
            return ExternalRuntimeStatus::Unavailable;
        }
        match probe_cache::probe_cache_state() {
            probe_cache::ClaudeCliProbeCacheState::NotProbed => ExternalRuntimeStatus::Unavailable,
            probe_cache::ClaudeCliProbeCacheState::Failed { .. } => ExternalRuntimeStatus::Faulted,
            probe_cache::ClaudeCliProbeCacheState::Ok { .. } => ExternalRuntimeStatus::Idle,
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
    // Revalidate executable identity immediately before auth probe.
    let meta_path = executable.clone();
    // Lightweight identity check via validate only (no prior discovery).
    discovery::validate_executable_path(&meta_path).map_err(map_discovery_error)?;
    auth::query_auth_status(&executable).await.map_err(|e| {
        ExternalRuntimeError::new(
            match e {
                auth::ClaudeCliAuthError::Timeout => ExternalRuntimeErrorKind::Transport,
                auth::ClaudeCliAuthError::ProbeFailed { .. }
                | auth::ClaudeCliAuthError::Unparseable => ExternalRuntimeErrorKind::Auth,
            },
            e.to_string(),
            Some(ExternalAgentKind::ClaudeCli),
        )
    })
}

/// Bounded async probe for catalog/UI bootstrap when gates are open.
/// Updates [`crate::agent::external_runtime::probe_cache`]. Safe to call from
/// provider refresh paths; does not block pure sync catalog resolve.
pub async fn bootstrap_probe_if_gated() {
    if !gates::claude_cli_both_gates_open() {
        return;
    }
    let runtime = ClaudeCliRuntime::from_env_path();
    match runtime.probe().await {
        Ok(caps) => {
            if let Some(v) = caps.version {
                probe_cache::record_probe_ok(v);
            } else {
                probe_cache::record_probe_ok("unknown");
            }
        }
        Err(e) => {
            probe_cache::record_probe_failed(e.message);
        }
    }
}

impl ClaudeCliRuntime {
    fn from_env_path() -> Self {
        let configured = std::env::var(discovery::CLAUDE_CLI_PATH_ENV)
            .ok()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .map(PathBuf::from);
        Self::new(configured)
    }
}

/// Public min version constant re-export for status strings.
pub fn min_version_str() -> &'static str {
    MIN_CLAUDE_CLI_VERSION
}
