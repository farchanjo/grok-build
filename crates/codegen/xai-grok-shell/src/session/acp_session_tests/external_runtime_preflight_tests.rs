//! External-runtime preflight: unavailable backends must not mutate turn_count
//! or durable user history, and the session remains switchable to native.
//! Also: goal/workflow slash rejection on external backends before mutation.

use super::support::*;
use super::*;
use crate::agent::execution_backend::{ExecutionBackend, ExternalAgentKind};
use crate::agent::external_runtime::EXTERNAL_RUNTIME_UNAVAILABLE;
use crate::agent::external_runtime::gates::CLAUDE_CLI_ENV_OPT_IN;
use std::rc::Rc;

/// Clear the runtime env opt-in for the duration of `f` and restore it after.
///
/// Feature-compiled builds gate the runtime on `GROK_CLAUDE_CLI_RUNTIME`, so
/// without this the preflight result would depend on the ambient environment.
/// The tests below assert the unavailable path — the default for a release
/// binary with the feature compiled but no env opt-in.
async fn without_env_opt_in<F, Fut, T>(f: F) -> T
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = T>,
{
    let prior = std::env::var(CLAUDE_CLI_ENV_OPT_IN).ok();
    unsafe {
        std::env::remove_var(CLAUDE_CLI_ENV_OPT_IN);
    }
    let result = f().await;
    unsafe {
        match prior {
            Some(v) => std::env::set_var(CLAUDE_CLI_ENV_OPT_IN, v),
            None => std::env::remove_var(CLAUDE_CLI_ENV_OPT_IN),
        }
    }
    result
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial(claude_cli_env)]
async fn external_unavailable_preflight_leaves_turn_and_history_unchanged() {
    without_env_opt_in(|| async {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let (gateway_tx, _gateway_rx) = tokio::sync::mpsc::unbounded_channel();
                let (persistence_tx, _persistence_rx) = tokio::sync::mpsc::unbounded_channel();
                let actor =
                    Rc::new(create_test_actor(0, 200_000, 80, gateway_tx, persistence_tx).await);
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
                        crate::session::PromptOrigin::User,
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
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial(claude_cli_env)]
async fn preflight_external_probe_is_unavailable_before_any_mutation() {
    without_env_opt_in(|| async {
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
                    .expect_err("unavailable before any mutation");
                assert_eq!(err.code(), EXTERNAL_RUNTIME_UNAVAILABLE);
                assert!(!err.is_auth_error());
            })
            .await;
    })
    .await;
}

/// `/goal` on an external session must fail before goal tracker mutation.
/// Default-feature builds use the unavailable stub (preflight fails first);
/// the goal tracker must remain empty either way.
#[tokio::test(flavor = "current_thread")]
async fn external_session_goal_slash_does_not_mutate_goal_state() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let (gateway_tx, _gateway_rx) = tokio::sync::mpsc::unbounded_channel();
            let (persistence_tx, _persistence_rx) = tokio::sync::mpsc::unbounded_channel();
            let actor =
                Rc::new(create_test_actor(0, 200_000, 80, gateway_tx, persistence_tx).await);
            actor.execution_backend.set(ExecutionBackend::ExternalAgent(
                ExternalAgentKind::ClaudeCli,
            ));
            assert!(
                actor.goal_tracker.lock().status().is_none(),
                "goal starts empty"
            );

            let prompt_blocks = vec![acp::ContentBlock::Text(acp::TextContent::new(
                "/goal ship the feature".to_string(),
            ))];
            let _ = actor
                .handle_prompt(
                    "ext-goal-reject",
                    crate::session::PromptOrigin::User,
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

            assert!(
                actor.goal_tracker.lock().status().is_none(),
                "goal tracker must not be mutated on external session /goal"
            );
        })
        .await;
}
