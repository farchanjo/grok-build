//! Focused StopCancelled report-slot and FIFO-worker integration tests.

use super::support::*;
use super::turn_end_hooks::{ReportOutcome, TurnEnd, TurnEndQueue};
use super::turn_report_slot::{CommitOutcome, TurnReportState};
use super::*;
use xai_grok_hooks::event::{HookEventName, StopCancelledReason, StopFailureKind};

struct Harness {
    actor: Arc<SessionActor>,
    gateway: tokio::sync::mpsc::UnboundedReceiver<xai_acp_lib::AcpClientMessage>,
    queue: Option<TurnEndQueue>,
}

impl Harness {
    async fn new() -> Self {
        let (gateway_tx, gateway) = tokio::sync::mpsc::unbounded_channel();
        let (persistence_tx, mut persistence) = tokio::sync::mpsc::unbounded_channel();
        tokio::task::spawn_local(async move { while persistence.recv().await.is_some() {} });
        let actor = Arc::new(create_test_actor(0, 256_000, 85, gateway_tx, persistence_tx).await);
        Self {
            queue: Some(TurnEndQueue::spawn(actor.clone())),
            actor,
            gateway,
        }
    }

    fn listen(&self, events: &[HookEventName]) {
        let mut hooks = crate::extensions::hooks::ClientHooks::new();
        for event in events {
            hooks.insert(
                *event,
                vec![crate::extensions::hooks::ClientHookGroup {
                    matcher: None,
                    callback_ids: vec!["test".into()],
                    timeout: None,
                }],
            );
        }
        *self.actor.client_hooks.borrow_mut() = hooks;
    }

    async fn start_turn(&self, prompt_id: &str) {
        self.actor.turn_report.start_next_turn();
        *self.actor.current_prompt_id.lock().expect("prompt mutex") = Some(prompt_id.to_string());
        let handle = tokio::task::spawn_local(async {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        })
        .abort_handle();
        self.actor.state.lock().await.running_task = Some(AgentTask {
            prompt_id: prompt_id.into(),
            handle,
        });
    }

    async fn queue_owned_turn(&self, prompt_id: &str) {
        self.start_turn(prompt_id).await;
        self.actor
            .state
            .lock()
            .await
            .pending_inputs
            .push_back(user_item(prompt_id, "test"));
    }

    async fn drain(&mut self) {
        if let Some(queue) = self.queue.take() {
            queue.drain().await;
        }
    }

    fn fired(&mut self) -> Vec<serde_json::Value> {
        let mut fired = Vec::new();
        while let Ok(message) = self.gateway.try_recv() {
            if let xai_acp_lib::AcpClientMessage::ExtNotification(args) = message
                && args.request.method.as_ref() == "x.ai/hooks/event"
            {
                fired.push(serde_json::from_str(args.request.params.get()).unwrap());
            }
        }
        fired
    }
}

async fn run(test: impl std::future::Future<Output = ()>) {
    tokio::task::LocalSet::new().run_until(test).await;
}

#[tokio::test(flavor = "current_thread")]
async fn ctrl_c_is_queued_once_and_does_not_wait_for_hook_execution() {
    run(async {
        let mut harness = Harness::new().await;
        harness.listen(&[HookEventName::StopCancelled, HookEventName::StopFailure]);
        // A deliberately slow file hook proves cancel only queues the report; the FIFO worker
        // owns execution and cannot hold up this actor command.
        *harness.actor.hook_registry.borrow_mut() = Some(Arc::new(
            super::client_hooks_tests::file_registry_with_stop_spec(
                HookEventName::StopCancelled,
                "sleep 5",
            ),
        ));
        harness.actor.chat_state_handle.push_assistant_response(
            xai_grok_inference_types::ConversationItem::assistant("partial"),
        );
        harness.start_turn("p1").await;

        let started = std::time::Instant::now();
        harness
            .actor
            .cancel_running_task(true, false, false, Some("ctrl_c".into()))
            .await;
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
        assert_eq!(harness.actor.turn_report.state(), TurnReportState::Reported);
        assert_eq!(
            harness.actor.report_turn_end(
                "p1",
                TurnEnd::Failed {
                    error: StopFailureKind::Unknown,
                    error_details: None,
                    last_assistant_message: None,
                },
            ),
            ReportOutcome::AlreadyReported
        );

        harness.drain().await;
        let fired = harness.fired();
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0]["hookEventName"], "stop_cancelled");
        assert_eq!(fired[0]["reason"], "user_interrupt");
        assert_eq!(fired[0]["cancelTrigger"], "ctrl_c");
        assert_eq!(fired[0]["lastAssistantMessage"], "partial");
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn ctrl_c_releases_only_the_live_stop_gate_claim() {
    run(async {
        let mut harness = Harness::new().await;
        harness.listen(&[HookEventName::StopCancelled]);
        *harness.actor.hook_registry.borrow_mut() = Some(Arc::new(
            super::client_hooks_tests::file_registry_with_stop_spec(HookEventName::Stop, "sleep 5"),
        ));
        harness.actor.turn_report.start_next_turn();
        *harness
            .actor
            .current_prompt_id
            .lock()
            .expect("prompt mutex") = Some("p1".into());
        let actor = harness.actor.clone();
        let gate = tokio::task::spawn_local(async move { actor.run_stop_gate("p1", 0).await });
        harness.actor.state.lock().await.running_task = Some(AgentTask {
            prompt_id: "p1".into(),
            handle: gate.abort_handle(),
        });
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while !matches!(
                harness.actor.turn_report.state(),
                TurnReportState::Held { .. }
            ) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("stop gate must hold the claim");

        harness
            .actor
            .cancel_running_task(true, false, false, Some("ctrl_c".into()))
            .await;
        assert!(
            gate.await
                .expect_err("cancel aborts the stop gate")
                .is_cancelled()
        );
        assert_eq!(harness.actor.turn_report.state(), TurnReportState::Reported);

        harness.drain().await;
        let fired = harness.fired();
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0]["hookEventName"], "stop_cancelled");
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn ctrl_c_after_a_committed_stop_gate_emits_no_second_report() {
    run(async {
        let mut harness = Harness::new().await;
        harness.listen(&[HookEventName::StopCancelled]);
        harness.start_turn("p1").await;
        let gate = harness
            .actor
            .turn_report
            .claim_for_gate()
            .expect("stop gate claim");
        assert_eq!(gate.commit(), CommitOutcome::Reported);

        harness
            .actor
            .cancel_running_task(true, false, false, Some("ctrl_c".into()))
            .await;
        harness.drain().await;
        assert!(harness.fired().is_empty());
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn stale_report_and_stale_completion_cannot_consume_successor() {
    run(async {
        let mut harness = Harness::new().await;
        harness.listen(&[HookEventName::StopCancelled]);
        harness.queue_owned_turn("p1").await;
        let stale_epoch = harness.actor.turn_report.epoch();

        harness
            .actor
            .cancel_running_task(true, false, false, Some("ctrl_c".into()))
            .await;
        harness.start_turn("p2").await;
        assert_eq!(
            harness.actor.claim_and_queue(
                "p1",
                stale_epoch,
                TurnEnd::Cancelled {
                    reason: StopCancelledReason::UserInterrupt,
                    trigger: None,
                    reason_details: None,
                    last_assistant_message: None,
                },
            ),
            ReportOutcome::AlreadyReported
        );

        harness
            .actor
            .handle_completion(
                "p1".into(),
                Ok(PromptTurnOk {
                    stop_reason: acp::StopReason::Cancelled,
                    total_tokens: 0,
                    turn_snapshot: None,
                    completion_kind: PromptCompletionKind::Cancelled {
                        category: Some(crate::session::events::CancellationCategory::MidTurnAbort),
                        context: None,
                    },
                    structured_output: None,
                    usage: None,
                    tool_overrides: None,
                }),
            )
            .await;
        assert_eq!(harness.actor.turn_report.state(), TurnReportState::Free);

        harness
            .actor
            .cancel_running_task(true, false, false, Some("ctrl_c".into()))
            .await;
        harness.drain().await;
        assert_eq!(harness.fired().len(), 2, "one report for each turn");
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn send_now_and_pristine_rewind_are_excluded() {
    run(async {
        for (trigger, rewind) in [("send_now", false), ("esc", true)] {
            let mut harness = Harness::new().await;
            harness.listen(&[HookEventName::StopCancelled]);
            harness.start_turn("p1").await;
            if rewind {
                let mut state = harness.actor.state.lock().await;
                state.rewindable = true;
                state.pending_inputs.push_back(user_item("p1", "test"));
            }
            harness
                .actor
                .cancel_running_task(false, false, rewind, Some(trigger.into()))
                .await;
            harness.drain().await;
            assert!(harness.fired().is_empty(), "{trigger} must not report");
        }
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn completion_classifies_api_failure_max_turns_and_no_progress_exclusively() {
    run(async {
        let mut harness = Harness::new().await;
        harness.listen(&[
            HookEventName::Stop,
            HookEventName::StopFailure,
            HookEventName::StopCancelled,
        ]);

        harness.queue_owned_turn("api").await;
        harness.actor.report_turn_end(
            "api",
            TurnEnd::Failed {
                error: StopFailureKind::ServerError,
                error_details: Some("503".into()),
                last_assistant_message: Some("Turn failed".into()),
            },
        );

        for (prompt_id, category, expected) in [
            ("max", None, "max_turns"),
            (
                "stationary",
                Some(crate::session::events::CancellationCategory::ActionStationarity),
                "no_progress",
            ),
        ] {
            harness.actor.state.lock().await.running_task = None;
            harness.actor.state.lock().await.pending_inputs.clear();
            harness.queue_owned_turn(prompt_id).await;
            let kind = if let Some(category) = category {
                PromptCompletionKind::Cancelled {
                    category: Some(category),
                    context: None,
                }
            } else {
                PromptCompletionKind::MaxTurnsReached { limit: 1 }
            };
            harness
                .actor
                .handle_completion(
                    prompt_id.into(),
                    Ok(PromptTurnOk {
                        stop_reason: acp::StopReason::Cancelled,
                        total_tokens: 0,
                        turn_snapshot: None,
                        completion_kind: kind,
                        structured_output: None,
                        usage: None,
                        tool_overrides: None,
                    }),
                )
                .await;
            let _ = expected;
        }

        harness.drain().await;
        let fired = harness.fired();
        assert_eq!(fired.len(), 3);
        assert_eq!(fired[0]["hookEventName"], "stop_failure");
        assert_eq!(fired[1]["reason"], "max_turns");
        assert_eq!(fired[2]["reason"], "no_progress");
        assert!(fired.iter().all(|event| event["hookEventName"] != "stop"));
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn closed_queue_releases_report_claim() {
    run(async {
        let mut harness = Harness::new().await;
        harness.listen(&[HookEventName::StopCancelled]);
        harness.start_turn("p1").await;
        harness.drain().await;

        assert_eq!(
            harness.actor.report_turn_end(
                "p1",
                TurnEnd::Cancelled {
                    reason: StopCancelledReason::UserInterrupt,
                    trigger: None,
                    reason_details: None,
                    last_assistant_message: None,
                },
            ),
            ReportOutcome::QueueClosed
        );
        assert_eq!(harness.actor.turn_report.state(), TurnReportState::Free);
    })
    .await;
}
