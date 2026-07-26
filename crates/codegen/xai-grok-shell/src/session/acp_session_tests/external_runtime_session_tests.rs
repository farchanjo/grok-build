//! Production-path integration tests for session-scoped external runtimes:
//! PermissionHandle + capability mode via session-aware factory, Arc reuse
//! across turns, shutdown on session end, and chat-state assistant text.
//!
//! Uses a recording fake runtime injected into SessionActor (no real Claude
//! binary). Feature-off builds still exercise the composition path.

use super::support::*;
use super::*;
use crate::agent::execution_backend::{ExecutionBackend, ExternalAgentKind};
use crate::agent::external_runtime::{
    ExternalAgentRuntime, ExternalRuntimeCapabilities, ExternalRuntimeEnvelope,
    ExternalRuntimeError, ExternalRuntimeStatus, ExternalRuntimeTurnEvent, ExternalStartRequest,
    ExternalTurnOutcome, ExternalTurnRequest, RetainedExternalAgentRuntime,
};
use async_trait::async_trait;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Recording fake external runtime for session composition tests.
struct RecordingExternalRuntime {
    kind: ExternalAgentKind,
    probe_count: AtomicUsize,
    turn_count: AtomicUsize,
    shutdown_count: AtomicUsize,
    cancel_count: AtomicUsize,
    turn_text: String,
}

impl RecordingExternalRuntime {
    fn new(turn_text: impl Into<String>) -> Arc<Self> {
        Arc::new(Self {
            kind: ExternalAgentKind::ClaudeCli,
            probe_count: AtomicUsize::new(0),
            turn_count: AtomicUsize::new(0),
            shutdown_count: AtomicUsize::new(0),
            cancel_count: AtomicUsize::new(0),
            turn_text: turn_text.into(),
        })
    }
}

#[async_trait]
impl ExternalAgentRuntime for RecordingExternalRuntime {
    fn kind(&self) -> ExternalAgentKind {
        self.kind
    }

    async fn probe(&self) -> Result<ExternalRuntimeCapabilities, ExternalRuntimeError> {
        self.probe_count.fetch_add(1, Ordering::SeqCst);
        Ok(ExternalRuntimeCapabilities {
            version: Some("test-0".into()),
            capabilities: vec![],
            models: Vec::new(),
        })
    }

    async fn start(
        &self,
        request: ExternalStartRequest,
    ) -> Result<ExternalRuntimeEnvelope, ExternalRuntimeError> {
        let mut env = ExternalRuntimeEnvelope::for_kind(self.kind);
        env.cwd = Some(request.cwd);
        env.selected_model = request.selected_model;
        env.reasoning_effort = request.reasoning_effort;
        env.token_budget = request.token_budget;
        env.validated().map_err(|e| {
            ExternalRuntimeError::new(
                crate::agent::external_runtime::ExternalRuntimeErrorKind::InvalidRequest,
                e.to_string(),
                Some(self.kind),
            )
        })
    }

    async fn resume(
        &self,
        envelope: &ExternalRuntimeEnvelope,
    ) -> Result<ExternalRuntimeEnvelope, ExternalRuntimeError> {
        Ok(envelope.clone())
    }

    async fn turn(
        &self,
        envelope: &ExternalRuntimeEnvelope,
        _request: ExternalTurnRequest,
    ) -> Result<ExternalTurnOutcome, ExternalRuntimeError> {
        self.turn_count.fetch_add(1, Ordering::SeqCst);
        let mut env = envelope.clone();
        if env.session_pointer.is_none() {
            env.session_pointer = Some("fake-sess".into());
        }
        Ok(ExternalTurnOutcome {
            events: vec![
                ExternalRuntimeTurnEvent::TextDelta {
                    text: self.turn_text.clone(),
                },
                // Claude tool event must NOT become a Grok tool call item.
                ExternalRuntimeTurnEvent::ToolCall {
                    name: "Bash".into(),
                    summary: Some("echo hi".into()),
                },
            ],
            envelope: env,
            result: None,
            usage: None,
        })
    }

    async fn cancel(
        &self,
        _envelope: &ExternalRuntimeEnvelope,
    ) -> Result<(), ExternalRuntimeError> {
        self.cancel_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn shutdown(
        &self,
        _envelope: &ExternalRuntimeEnvelope,
    ) -> Result<(), ExternalRuntimeError> {
        self.shutdown_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn status(&self, _envelope: Option<&ExternalRuntimeEnvelope>) -> ExternalRuntimeStatus {
        ExternalRuntimeStatus::Idle
    }
}

#[tokio::test(flavor = "current_thread")]
async fn host_mode_label_plan_yolo_auto_default() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) = tokio::sync::mpsc::unbounded_channel();
            let (persistence_tx, _persistence_rx) = tokio::sync::mpsc::unbounded_channel();
            let actor = create_test_actor(0, 200_000, 80, gateway_tx, persistence_tx).await;

            // create_test_actor uses PermissionHandle::allow_all → yolo.
            assert!(
                actor.permissions.is_yolo_mode(),
                "test PermissionHandle::allow_all is yolo"
            );
            assert_eq!(actor.external_host_mode_label(), "yolo");

            // Plan mode wins over yolo (read-only).
            *actor.current_prompt_mode.lock() = PromptMode::Plan;
            actor.plan_mode.lock().enter_pending();
            assert_eq!(actor.external_host_mode_label(), "plan");

            // Clear plan → yolo surfaces again.
            actor.plan_mode.lock().user_exit(false);
            *actor.current_prompt_mode.lock() = PromptMode::Agent;
            assert_eq!(actor.external_host_mode_label(), "yolo");
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn two_turns_reuse_same_runtime_instance() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) = tokio::sync::mpsc::unbounded_channel();
            let (persistence_tx, _persistence_rx) = tokio::sync::mpsc::unbounded_channel();
            let actor = create_test_actor(0, 200_000, 80, gateway_tx, persistence_tx).await;
            actor.execution_backend.set(ExecutionBackend::ExternalAgent(
                ExternalAgentKind::ClaudeCli,
            ));

            let fake = RecordingExternalRuntime::new("hello");
            let mode = actor.external_host_mode_label();
            *actor.external_agent_runtime.borrow_mut() = Some(RetainedExternalAgentRuntime::new(
                ExternalAgentKind::ClaudeCli,
                mode,
                fake.clone() as Arc<dyn ExternalAgentRuntime>,
            ));

            let r1 = actor
                .ensure_external_agent_runtime(ExternalAgentKind::ClaudeCli)
                .await
                .expect("retained runtime");
            let r2 = actor
                .ensure_external_agent_runtime(ExternalAgentKind::ClaudeCli)
                .await
                .expect("retained runtime");
            assert!(
                Arc::ptr_eq(&r1, &r2),
                "preflight/turn must reuse the same Arc"
            );
            // Same concrete instance as injected.
            assert!(Arc::ptr_eq(
                &(fake.clone() as Arc<dyn ExternalAgentRuntime>),
                &r1
            ));
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn session_shutdown_calls_runtime_shutdown() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) = tokio::sync::mpsc::unbounded_channel();
            let (persistence_tx, _persistence_rx) = tokio::sync::mpsc::unbounded_channel();
            let actor = create_test_actor(0, 200_000, 80, gateway_tx, persistence_tx).await;
            actor.execution_backend.set(ExecutionBackend::ExternalAgent(
                ExternalAgentKind::ClaudeCli,
            ));

            let fake = RecordingExternalRuntime::new("x");
            *actor.external_agent_runtime.borrow_mut() = Some(RetainedExternalAgentRuntime::new(
                ExternalAgentKind::ClaudeCli,
                actor.external_host_mode_label(),
                fake.clone() as Arc<dyn ExternalAgentRuntime>,
            ));

            assert_eq!(fake.shutdown_count.load(Ordering::SeqCst), 0);
            actor.shutdown_external_agent_runtime().await;
            assert_eq!(
                fake.shutdown_count.load(Ordering::SeqCst),
                1,
                "session termination must call runtime.shutdown"
            );
            assert!(
                actor.external_agent_runtime.borrow().is_none(),
                "retained slot must be cleared"
            );
            // Idempotent.
            actor.shutdown_external_agent_runtime().await;
            assert_eq!(fake.shutdown_count.load(Ordering::SeqCst), 1);
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn external_assistant_text_lands_in_chat_state_without_tool_calls() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) = tokio::sync::mpsc::unbounded_channel();
            let (persistence_tx, _persistence_rx) = tokio::sync::mpsc::unbounded_channel();
            let actor =
                Arc::new(create_test_actor(0, 200_000, 80, gateway_tx, persistence_tx).await);
            actor.execution_backend.set(ExecutionBackend::ExternalAgent(
                ExternalAgentKind::ClaudeCli,
            ));

            let fake = RecordingExternalRuntime::new("external assistant reply");
            *actor.external_agent_runtime.borrow_mut() = Some(RetainedExternalAgentRuntime::new(
                ExternalAgentKind::ClaudeCli,
                actor.external_host_mode_label(),
                fake.clone() as Arc<dyn ExternalAgentRuntime>,
            ));

            let len_before = actor.chat_state_handle.get_conversation_len().await;
            let result = actor
                .run_external_agent_turn("ext-chat-1", "user says hi")
                .await
                .expect("external turn ok");
            assert_eq!(result.stop_reason, acp::StopReason::EndTurn);
            assert_eq!(fake.turn_count.load(Ordering::SeqCst), 1);

            let conv = actor.chat_state_handle.get_conversation().await;
            assert!(
                conv.len() > len_before,
                "conversation must grow with assistant text"
            );
            let last_assistant = conv.iter().rev().find_map(|item| match item {
                ConversationItem::Assistant(a) => Some(a),
                _ => None,
            });
            let assistant = last_assistant.expect("assistant item persisted");
            assert!(
                assistant.content.contains("external assistant reply"),
                "assistant text missing: {:?}",
                assistant.content
            );
            // No Grok tool_calls from Claude tool events.
            assert!(
                assistant.tool_calls.is_empty(),
                "Claude tools must not become Grok tool_calls: {:?}",
                assistant.tool_calls
            );
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn switching_to_native_shuts_down_external_runtime() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) = tokio::sync::mpsc::unbounded_channel();
            let (persistence_tx, _persistence_rx) = tokio::sync::mpsc::unbounded_channel();
            let actor = create_test_actor(0, 200_000, 80, gateway_tx, persistence_tx).await;
            actor.execution_backend.set(ExecutionBackend::ExternalAgent(
                ExternalAgentKind::ClaudeCli,
            ));
            let fake = RecordingExternalRuntime::new("x");
            *actor.external_agent_runtime.borrow_mut() = Some(RetainedExternalAgentRuntime::new(
                ExternalAgentKind::ClaudeCli,
                actor.external_host_mode_label(),
                fake.clone() as Arc<dyn ExternalAgentRuntime>,
            ));

            let mut cfg = xai_grok_inference::InferenceConfig::default();
            cfg.model = "test-model".into();
            cfg.base_url = "http://localhost".into();
            cfg.context_window = 128_000;
            let _ = actor
                .handle_set_session_model(
                    cfg,
                    false,
                    false,
                    true,
                    80,
                    ExecutionBackend::NativeInference,
                )
                .await
                .expect("switch to native");
            assert_eq!(
                fake.shutdown_count.load(Ordering::SeqCst),
                1,
                "leaving external must shutdown retained runtime"
            );
            assert!(actor.external_agent_runtime.borrow().is_none());
        })
        .await;
}

// Suppress unused import warnings for AtomicBool when only AtomicUsize is used.
#[allow(dead_code)]
fn _keep_atomic_bool(_: &AtomicBool) {}
