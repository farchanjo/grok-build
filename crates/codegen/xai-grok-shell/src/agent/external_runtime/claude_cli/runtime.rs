//! [`ClaudeCliRuntime`] — process-backed [`ExternalAgentRuntime`] for Claude CLI.
//!
//! PR6: one-process-per-turn MVP. PR7: permission bridge, strict MCP config,
//! capability-mode tool restriction, resume hardening, optional persistent
//! multi-turn when capabilities advertise streaming input.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use super::argv::ClaudeCliTurnArgv;
use super::auth;
use super::capability_mode::ClaudeCapabilityMode;
use super::discovery::{self, ClaudeCliDiscovery, ClaudeCliDiscoveryError, MIN_CLAUDE_CLI_VERSION};
use super::gates;
use super::mcp_config::{self, ApprovedExternalMcpServer, GeneratedMcpConfig};
use super::permission_bridge::{
    ClaudePermissionBroker, PermissionBrokerServer, PolicyPermissionBroker,
};
use super::persistent::{self, PersistentClaudeSession};
use super::process::{self, ProcessLimits, TurnProcessError};
use super::protocol::{self, ProtocolError};
use super::provider_status::{self, ClaudeCliProviderStatus};
use super::resume_guard::{self, supports_persistent_input};
use super::sandbox_probe;
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
    /// PR7 capability mode (defaults ReadOnly for safety).
    capability_mode: ClaudeCapabilityMode,
    /// Explicit always-approve opt-in (still requires yolo PermissionHandle).
    always_approve_opt_in: bool,
    /// Optional Grok permission handle for the bridge.
    permission_handle: Option<xai_grok_workspace::permission::PermissionHandle>,
    /// Explicitly approved external MCP servers (never auto-discovered).
    approved_mcp: Vec<ApprovedExternalMcpServer>,
    /// Host executable for re-exec permission bridge child.
    host_executable: Option<PathBuf>,
    /// Runtime dir for sockets / temp MCP config.
    runtime_dir: Mutex<Option<PathBuf>>,
    /// Live permission broker server (if started).
    broker_server: Mutex<Option<Arc<PermissionBrokerServer>>>,
    /// Generated MCP config (cleaned on shutdown).
    mcp_config: Mutex<Option<GeneratedMcpConfig>>,
    /// Persistent multi-turn session (capability-gated).
    persistent: Mutex<Option<PersistentClaudeSession>>,
    /// Permission bridge readiness for provider status.
    bridge_ready: Mutex<bool>,
}

impl ClaudeCliRuntime {
    pub fn new(configured_path: Option<PathBuf>) -> Self {
        Self {
            configured_path,
            limits: ProcessLimits::default(),
            discovery: Mutex::new(None),
            inflight_cancel: Mutex::new(None),
            capability_mode: ClaudeCapabilityMode::ReadOnly,
            always_approve_opt_in: false,
            permission_handle: None,
            approved_mcp: Vec::new(),
            host_executable: std::env::current_exe().ok(),
            runtime_dir: Mutex::new(None),
            broker_server: Mutex::new(None),
            mcp_config: Mutex::new(None),
            persistent: Mutex::new(None),
            bridge_ready: Mutex::new(false),
        }
    }

    pub fn with_limits(mut self, limits: ProcessLimits) -> Self {
        self.limits = limits;
        self
    }

    pub fn with_capability_mode(mut self, mode: ClaudeCapabilityMode) -> Self {
        self.capability_mode = mode;
        self
    }

    pub fn with_always_approve_opt_in(mut self, enabled: bool) -> Self {
        self.always_approve_opt_in = enabled;
        self
    }

    pub fn with_permission_handle(
        mut self,
        handle: xai_grok_workspace::permission::PermissionHandle,
    ) -> Self {
        self.permission_handle = Some(handle);
        self
    }

    pub fn with_approved_mcp(mut self, servers: Vec<ApprovedExternalMcpServer>) -> Self {
        self.approved_mcp = servers;
        self
    }

    pub fn with_host_executable(mut self, path: PathBuf) -> Self {
        self.host_executable = Some(path);
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

    /// Ensure permission bridge + strict MCP config exist for this runtime.
    async fn ensure_permission_bridge(&self) -> Result<(), ExternalRuntimeError> {
        if *self.bridge_ready.lock().await && self.broker_server.lock().await.is_some() {
            return Ok(());
        }

        let broker: Arc<dyn ClaudePermissionBroker> = match &self.permission_handle {
            Some(h) => Arc::new(
                PolicyPermissionBroker::new(self.capability_mode)
                    .with_permission(h.clone())
                    // always_approve_opt_in only selects allowlist mode; no short-circuit.
                    .with_always_approve_opt_in(self.always_approve_opt_in),
            ),
            None => {
                // Still start the bridge so Claude has a tool; decisions fail closed.
                Arc::new(PolicyPermissionBroker::new(self.capability_mode))
            }
        };
        let cancel = CancellationToken::new();
        // Private 0700 dir + 0600 socket + auth token created inside start().
        let server = PermissionBrokerServer::start(broker, cancel).await?;
        *self.runtime_dir.lock().await = Some(server.runtime_dir().to_path_buf());
        let host = self.host_executable.clone().ok_or_else(|| {
            ExternalRuntimeError::new(
                ExternalRuntimeErrorKind::Transport,
                "host executable path unavailable for permission bridge child",
                Some(ExternalAgentKind::ClaudeCli),
            )
        })?;
        let mcp = mcp_config::write_strict_mcp_config(
            server.runtime_dir(),
            &host,
            server.socket_path(),
            server.token(),
            &self.approved_mcp,
        )?;
        *self.broker_server.lock().await = Some(server);
        *self.mcp_config.lock().await = Some(mcp);
        *self.bridge_ready.lock().await = true;
        Ok(())
    }

    fn build_turn_argv(
        &self,
        d: &ClaudeCliDiscovery,
        envelope: &ExternalRuntimeEnvelope,
        request: &ExternalTurnRequest,
        session_id: Option<String>,
        resume: Option<String>,
        persistent: bool,
        mcp_path: Option<PathBuf>,
        permission_tool: Option<String>,
    ) -> ClaudeCliTurnArgv {
        ClaudeCliTurnArgv {
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
            session_id,
            resume_session: resume,
            cwd: envelope.cwd.as_ref().map(PathBuf::from),
            mcp_config: mcp_path,
            permission_prompt_tool: permission_tool,
            capability_mode: Some(self.capability_mode),
            persistent_input: persistent,
        }
    }

    /// Provider status for UI (binary / auth / bridge distinct from API key).
    pub async fn provider_status(&self) -> ClaudeCliProviderStatus {
        let binary = self.discovery.lock().await.clone();
        let (binary_ready, ver, detail) = match binary {
            Some(d) => (true, Some(d.version.to_string()), None),
            None => (false, None, Some("not probed".into())),
        };
        let bridge = *self.bridge_ready.lock().await;
        provider_status::build_status(binary_ready, ver, detail, None, bridge)
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
        // PR7 resume hardening: model/effort/cwd/worktree/version/caps.
        resume_guard::validate_resume(
            envelope,
            &d,
            envelope.selected_model.as_deref(),
            envelope.reasoning_effort.as_deref(),
            envelope.cwd.as_deref(),
            envelope.worktree_identity.as_deref(),
        )
        .map_err(|e| e.into_runtime_error())?;

        let mut env = envelope.clone();
        env.observed_version = Some(d.version.to_string());
        // Merge live additive capabilities.
        if !d.capabilities.is_empty() {
            for c in &d.capabilities {
                if !env.capabilities.iter().any(|x| x == c) {
                    env.capabilities.push(c.clone());
                }
            }
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

        // Resume identity checks when a pointer is present.
        if envelope
            .session_pointer
            .as_ref()
            .is_some_and(|s| !s.is_empty())
        {
            resume_guard::validate_resume(
                envelope,
                &d,
                request
                    .selected_model
                    .as_deref()
                    .or(envelope.selected_model.as_deref()),
                request
                    .reasoning_effort
                    .as_deref()
                    .or(envelope.reasoning_effort.as_deref()),
                envelope.cwd.as_deref(),
                envelope.worktree_identity.as_deref(),
            )
            .map_err(|e| e.into_runtime_error())?;
        }

        // Sandbox gate: when parent is active, inheritance must be positively
        // verified or the turn fails closed before Claude spawn. Never weakens
        // parent policy.
        let sandbox_obs = sandbox_probe::observe_child_sandbox(sandbox_probe::default_child_probe);
        if let Err(fail) = sandbox_probe::gate_turn_for_sandbox(
            &sandbox_obs,
            &sandbox_probe::ExpectedChildPosture::default(),
            sandbox_probe::parent_sandbox_active(),
        ) {
            return Err(ExternalRuntimeError::new(
                ExternalRuntimeErrorKind::Unavailable,
                format!(
                    "Claude CLI turn blocked by sandbox inheritance gate: {fail:?}. {}",
                    sandbox_probe::SANDBOX_POLICY_NOTE
                ),
                Some(ExternalAgentKind::ClaudeCli),
            ));
        }

        // Permission bridge + strict MCP (fail closed if cannot start).
        self.ensure_permission_bridge().await?;
        let (mcp_path, permission_tool) = {
            let guard = self.mcp_config.lock().await;
            let cfg = guard.as_ref().ok_or_else(|| {
                ExternalRuntimeError::new(
                    ExternalRuntimeErrorKind::Transport,
                    "permission bridge MCP config missing",
                    Some(ExternalAgentKind::ClaudeCli),
                )
            })?;
            (
                Some(cfg.path.clone()),
                Some(cfg.permission_prompt_tool.clone()),
            )
        };

        let cancel = CancellationToken::new();
        {
            let mut slot = self.inflight_cancel.lock().await;
            if let Some(prev) = slot.take() {
                prev.cancel();
            }
            *slot = Some(cancel.clone());
        }

        // Capability matrix for persistent multi-turn.
        let caps_for_mode: Vec<String> = if !envelope.capabilities.is_empty() {
            envelope.capabilities.clone()
        } else {
            d.capabilities.clone()
        };
        let want_persistent = supports_persistent_input(&caps_for_mode);

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

        // --- Persistent path (capability-gated) ---
        if want_persistent {
            let outcome = self
                .turn_persistent(
                    &d,
                    envelope,
                    &request,
                    session_id.clone(),
                    resume.clone(),
                    mcp_path.clone(),
                    permission_tool.clone(),
                    cancel.clone(),
                )
                .await;
            *self.inflight_cancel.lock().await = None;
            return outcome;
        }

        // --- PR6 one-process-per-turn path ---
        let argv = self.build_turn_argv(
            &d,
            envelope,
            &request,
            session_id.clone(),
            resume.clone(),
            false,
            mcp_path,
            permission_tool,
        );
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

        finalize_turn_outcome(envelope, &d, session_id, &outcome.lines, false)
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
        // Tear down persistent child + permission bridge + temp config.
        if let Some(mut sess) = self.persistent.lock().await.take() {
            sess.shutdown().await;
        }
        if let Some(server) = self.broker_server.lock().await.take() {
            server.shutdown().await;
        }
        if let Some(cfg) = self.mcp_config.lock().await.take() {
            cfg.cleanup();
        }
        if let Some(dir) = self.runtime_dir.lock().await.take() {
            let _ = std::fs::remove_dir_all(&dir);
        }
        *self.bridge_ready.lock().await = false;
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

impl ClaudeCliRuntime {
    /// Persistent multi-turn path: one child, queue one turn at a time.
    async fn turn_persistent(
        &self,
        d: &ClaudeCliDiscovery,
        envelope: &ExternalRuntimeEnvelope,
        request: &ExternalTurnRequest,
        session_id: Option<String>,
        resume: Option<String>,
        mcp_path: Option<PathBuf>,
        permission_tool: Option<String>,
        cancel: CancellationToken,
    ) -> Result<ExternalTurnOutcome, ExternalRuntimeError> {
        // Reuse live session if still alive; else spawn (or fall back).
        {
            let mut slot = self.persistent.lock().await;
            if let Some(sess) = slot.as_mut() {
                if sess.is_alive() {
                    let outcome = sess
                        .run_turn(&request.prompt, cancel.clone())
                        .await
                        .map_err(map_process_error)?;
                    if outcome.cancelled {
                        return Err(cancelled_with_partial(
                            "Claude Agent CLI persistent turn cancelled",
                            envelope,
                            d,
                            sess.pointer_for_resume().or(session_id),
                            &outcome.lines,
                        ));
                    }
                    return finalize_turn_outcome(envelope, d, session_id, &outcome.lines, false);
                }
                // Child died: persist pointer then fall back if allowed.
                let pointer = sess.pointer_for_resume().or(resume.clone());
                let can_resume = sess.oneshot_resume_allowed() || pointer.is_some();
                let _ = slot.take();
                if can_resume {
                    return self
                        .turn_oneshot_resume(
                            d,
                            envelope,
                            request,
                            pointer,
                            mcp_path,
                            permission_tool,
                            cancel,
                        )
                        .await;
                }
                return Err(ExternalRuntimeError::new(
                    ExternalRuntimeErrorKind::Transport,
                    "Claude CLI persistent child died without a resumable session pointer",
                    Some(ExternalAgentKind::ClaudeCli),
                ));
            }
        }

        // Spawn new persistent child.
        let argv = self.build_turn_argv(
            d,
            envelope,
            request,
            session_id.clone(),
            resume.clone(),
            true,
            mcp_path.clone(),
            permission_tool.clone(),
        );
        let plan = argv.build_plan();
        let mut sess = PersistentClaudeSession::spawn(&plan, self.limits.clone())
            .await
            .map_err(map_process_error)?;
        let outcome = sess
            .run_turn(&request.prompt, cancel.clone())
            .await
            .map_err(map_process_error)?;
        if outcome.cancelled {
            let ptr = sess.pointer_for_resume().or(session_id.clone());
            // Keep pointer on envelope even if we drop the child.
            *self.persistent.lock().await = None;
            sess.shutdown().await;
            return Err(cancelled_with_partial(
                "Claude Agent CLI persistent turn cancelled",
                envelope,
                d,
                ptr,
                &outcome.lines,
            ));
        }
        if !sess.is_alive() {
            // Died after result — OK; do not keep dead session.
            let out = finalize_turn_outcome(envelope, d, session_id, &outcome.lines, false)?;
            *self.persistent.lock().await = None;
            return Ok(out);
        }
        *self.persistent.lock().await = Some(sess);
        finalize_turn_outcome(envelope, d, session_id, &outcome.lines, false)
    }

    async fn turn_oneshot_resume(
        &self,
        d: &ClaudeCliDiscovery,
        envelope: &ExternalRuntimeEnvelope,
        request: &ExternalTurnRequest,
        resume: Option<String>,
        mcp_path: Option<PathBuf>,
        permission_tool: Option<String>,
        cancel: CancellationToken,
    ) -> Result<ExternalTurnOutcome, ExternalRuntimeError> {
        let argv = self.build_turn_argv(
            d,
            envelope,
            request,
            None,
            resume,
            false,
            mcp_path,
            permission_tool,
        );
        let plan = argv.build_plan();
        let outcome = process::run_turn_process(&plan, &self.limits, cancel)
            .await
            .map_err(map_process_error)?;
        if outcome.cancelled {
            return Err(cancelled_with_partial(
                "Claude Agent CLI resume turn cancelled",
                envelope,
                d,
                None,
                &outcome.lines,
            ));
        }
        finalize_turn_outcome(envelope, d, None, &outcome.lines, false)
    }
}

fn finalize_turn_outcome(
    envelope: &ExternalRuntimeEnvelope,
    d: &ClaudeCliDiscovery,
    session_id: Option<String>,
    lines: &[String],
    allow_incomplete: bool,
) -> Result<ExternalTurnOutcome, ExternalRuntimeError> {
    let mut outcome = if allow_incomplete {
        persistent::apply_outcome_to_envelope(envelope, lines, true).map_err(map_protocol_error)?
    } else {
        persistent::apply_outcome_to_envelope(envelope, lines, false).map_err(map_protocol_error)?
    };
    if outcome.envelope.session_pointer.is_none() {
        if let Some(sid) = session_id {
            outcome.envelope.session_pointer = Some(sid);
        }
    }
    if outcome.envelope.observed_version.is_none() {
        outcome.envelope.observed_version = Some(d.version.to_string());
    }
    // Claude-owned tool labeling (display only; no Grok hooks/dispatcher).
    persistent::label_claude_owned_events(&mut outcome.events);
    // Surface status that Claude tools are not Grok-native.
    if outcome
        .events
        .iter()
        .any(|e| matches!(e, ExternalRuntimeTurnEvent::ToolCall { .. }))
    {
        outcome.events.push(ExternalRuntimeTurnEvent::Status {
            message: "Claude-owned tools (display only; not executed by Grok)".into(),
        });
    }
    let env = outcome.envelope.validated().map_err(|e| {
        ExternalRuntimeError::new(
            ExternalRuntimeErrorKind::InvalidRequest,
            e.to_string(),
            Some(ExternalAgentKind::ClaudeCli),
        )
    })?;
    Ok(ExternalTurnOutcome {
        events: outcome.events,
        envelope: env,
        result: outcome.result,
        usage: outcome.usage,
    })
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
