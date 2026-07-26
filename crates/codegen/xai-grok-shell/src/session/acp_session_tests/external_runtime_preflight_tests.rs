//! External-runtime preflight: unavailable backends must not mutate turn_count
//! or durable user history, and the session remains switchable to native.

use super::support::*;
use super::*;
use crate::agent::execution_backend::{ExecutionBackend, ExternalAgentKind};
use crate::agent::external_runtime::EXTERNAL_RUNTIME_UNAVAILABLE;
use std::sync::Arc;

#[tokio::test(flavor = "current_thread")]
async fn external_unavailable_preflight_leaves_turn_and_history_unchanged() {
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

            let turn_before = actor
                .signals_handle()
                .snapshot()
                .await
                .map(|s| s.turn_count)
                .unwrap_or(0);
            let conv_len_before = actor.chat_state_handle.get_conversation_len().await;

            let prompt_blocks = vec![acp::ContentBlock::Text(acp::TextContent::new(
                "hello external".to_string(),
            ))];
            let result = actor
                .handle_prompt(
                    "ext-preflight-test",
                    prompt_blocks,
                    PromptMode::Agent,
                    None,
                    None,
                    None,
                    None,
                    false,
                    None,
                    None,
                    None,
                )
                .await;

            let err = result.expect_err("external preflight must fail closed");
            let data = err.data.as_ref().expect("error data");
            assert_eq!(
                data.get("code").and_then(|v| v.as_str()),
                Some(EXTERNAL_RUNTIME_UNAVAILABLE)
            );
            assert_eq!(data.get("authError").and_then(|v| v.as_bool()), Some(false));

            let turn_after = actor
                .signals_handle()
                .snapshot()
                .await
                .map(|s| s.turn_count)
                .unwrap_or(0);
            let conv_len_after = actor.chat_state_handle.get_conversation_len().await;
            assert_eq!(
                turn_after, turn_before,
                "turn_count must not increment on external preflight failure"
            );
            assert_eq!(
                conv_len_after, conv_len_before,
                "durable conversation must not grow on external preflight failure"
            );

            // Session remains usable for a native switch: flip mode and preflight passes.
            actor
                .execution_backend
                .set(ExecutionBackend::NativeInference);
            actor
                .preflight_external_execution_backend()
                .await
                .expect("native preflight must succeed");
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn preflight_external_probe_is_unavailable_before_any_mutation() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) = tokio::sync::mpsc::unbounded_channel();
            let (persistence_tx, _persistence_rx) = tokio::sync::mpsc::unbounded_channel();
            let actor = create_test_actor(0, 200_000, 80, gateway_tx, persistence_tx).await;
            actor.execution_backend.set(ExecutionBackend::ExternalAgent(
                ExternalAgentKind::ClaudeCli,
            ));
            let err = actor
                .preflight_external_execution_backend()
                .await
                .expect_err("stub unavailable");
            assert_eq!(err.code(), EXTERNAL_RUNTIME_UNAVAILABLE);
            assert!(!err.is_auth_error());
        })
        .await;
}
