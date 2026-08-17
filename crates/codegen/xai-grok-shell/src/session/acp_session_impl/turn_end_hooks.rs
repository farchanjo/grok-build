//! Queued `StopFailure` and `StopCancelled` reports.
//!
//! These two observe-only reports run instead of the turn's blocking `Stop` gate. Reporters
//! claim the turn before placing a bounded payload on one actor-local FIFO worker, so cancel,
//! completion, and successor promotion never wait for hook execution.

use super::*;
use xai_grok_hooks::event::{self, StopCancelledReason, StopFailureKind};

const TURN_END_DRAIN_BUDGET: std::time::Duration = std::time::Duration::from_millis(250);

#[derive(Debug)]
pub(super) enum TurnEnd {
    Failed {
        error: StopFailureKind,
        error_details: Option<String>,
        last_assistant_message: Option<String>,
    },
    Cancelled {
        reason: StopCancelledReason,
        trigger: Option<String>,
        reason_details: Option<String>,
        last_assistant_message: Option<String>,
    },
}

impl TurnEnd {
    fn event_name(&self) -> event::HookEventName {
        match self {
            Self::Failed { .. } => event::HookEventName::StopFailure,
            Self::Cancelled { .. } => event::HookEventName::StopCancelled,
        }
    }

    fn is_inherited_interrupt(&self) -> bool {
        matches!(
            self,
            Self::Cancelled {
                reason: StopCancelledReason::UserInterrupt,
                ..
            }
        )
    }

    fn into_payload(self, subagent_type: Option<String>) -> event::HookPayload {
        let clip_detail = |text: Option<&str>| text.map(event::clip_stop_entry_text);
        let clip_message = |text: Option<&str>| text.map(event::clip_assistant_message);
        match self {
            Self::Failed {
                error,
                error_details,
                last_assistant_message,
            } => event::HookPayload::StopFailure {
                error,
                error_details: clip_detail(error_details.as_deref()),
                last_assistant_message: clip_message(last_assistant_message.as_deref()),
            },
            Self::Cancelled {
                reason,
                trigger,
                reason_details,
                last_assistant_message,
            } => event::HookPayload::StopCancelled {
                reason,
                cancelled_by: reason.cancelled_by(),
                cancel_trigger: trigger
                    .as_deref()
                    .map(|value| event::clip_text(value, event::MAX_CANCEL_TRIGGER_CHARS)),
                reason_details: clip_detail(reason_details.as_deref()),
                last_assistant_message: clip_message(last_assistant_message.as_deref()),
                subagent_type,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ReportOutcome {
    Queued,
    InheritedInterrupt,
    NoListener,
    AlreadyReported,
    QueueClosed,
}

pub(crate) enum QueueItem {
    Report(Box<TurnEndReport>),
    Barrier(tokio::sync::oneshot::Sender<()>),
}

#[derive(Debug)]
pub(crate) struct TurnEndReport {
    prompt_id: String,
    event: event::HookEventName,
    payload: event::HookPayload,
}

/// Classify an actor-command cancellation. Replace/teardown (`send_now`, pristine rewind,
/// internal fail-stop, and child teardown) deliberately report nothing.
pub(super) fn cancel_reason_for_trigger(trigger: Option<&str>) -> Option<StopCancelledReason> {
    match trigger {
        Some("send_now" | "compaction_persistence_indeterminate") | None => None,
        Some("ctrl_c" | "esc") => Some(StopCancelledReason::UserInterrupt),
        // Client stop buttons and future interactive stop gestures carry their own bounded token.
        Some(_) => Some(StopCancelledReason::UserInterrupt),
    }
}

pub(super) fn cancel_reason_for_completion(
    kind: &PromptCompletionKind,
) -> Option<StopCancelledReason> {
    use crate::session::events::CancellationCategory as Category;
    match kind {
        PromptCompletionKind::Cancelled { category, .. } => Some(match category {
            Some(Category::PermissionRejected) => StopCancelledReason::PermissionRejected,
            Some(Category::PermissionCancelled) => StopCancelledReason::PermissionCancelled,
            Some(Category::MidTurnAbort) => StopCancelledReason::UserInterrupt,
            Some(Category::ActionStationarity) => StopCancelledReason::NoProgress,
            Some(Category::HookDenied) | None => StopCancelledReason::Unknown,
        }),
        PromptCompletionKind::MaxTurnsReached { .. } => Some(StopCancelledReason::MaxTurns),
        PromptCompletionKind::Completed
        | PromptCompletionKind::Rewound
        | PromptCompletionKind::RemovedFromQueue => None,
    }
}

pub(super) fn cancel_details(kind: &PromptCompletionKind) -> Option<String> {
    let PromptCompletionKind::Cancelled {
        context: Some(context),
        ..
    } = kind
    else {
        return None;
    };
    let subject = context.tool_name.as_ref().or(context.hook_name.as_ref());
    match (subject, &context.reason) {
        (Some(subject), Some(reason)) => Some(format!("{subject}: {reason}")),
        (Some(subject), None) => Some(subject.clone()),
        (None, reason) => reason.clone(),
    }
}

#[must_use = "the FIFO turn-end worker must be drained during session teardown"]
pub(super) struct TurnEndQueue {
    session: Arc<SessionActor>,
    tx: Option<tokio::sync::mpsc::UnboundedSender<QueueItem>>,
    worker: Option<tokio::task::JoinHandle<()>>,
}

impl TurnEndQueue {
    pub(super) fn spawn(session: Arc<SessionActor>) -> Self {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<QueueItem>();
        *session.turn_end_tx.borrow_mut() = Some(tx.clone());
        let worker = tokio::task::spawn_local({
            let session = session.clone();
            async move {
                while let Some(item) = rx.recv().await {
                    match item {
                        QueueItem::Report(report) => session.dispatch_turn_end(*report).await,
                        QueueItem::Barrier(reached) => {
                            let _ = reached.send(());
                        }
                    }
                }
            }
        });
        Self {
            session,
            tx: Some(tx),
            worker: Some(worker),
        }
    }

    fn disarm(&self) {
        let Some(tx) = self.tx.as_ref() else {
            return;
        };
        let mut installed = self.session.turn_end_tx.borrow_mut();
        if installed.as_ref().is_some_and(|live| live.same_channel(tx)) {
            *installed = None;
        }
    }

    pub(super) async fn flush(&mut self) {
        let (reached, wait) = tokio::sync::oneshot::channel();
        let sent = self
            .tx
            .as_ref()
            .is_some_and(|tx| tx.send(QueueItem::Barrier(reached)).is_ok());
        if sent
            && tokio::time::timeout(TURN_END_DRAIN_BUDGET, wait)
                .await
                .is_err()
        {
            tracing::warn!(
                budget_ms = TURN_END_DRAIN_BUDGET.as_millis(),
                "a turn-end hook is still running; teardown will not wait indefinitely"
            );
        }
    }

    pub(super) async fn drain(mut self) {
        self.disarm();
        self.tx = None;
        let Some(mut worker) = self.worker.take() else {
            return;
        };
        match tokio::time::timeout(TURN_END_DRAIN_BUDGET, &mut worker).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => tracing::error!(%error, "turn-end hook worker failed"),
            Err(_) => {
                tracing::warn!(
                    budget_ms = TURN_END_DRAIN_BUDGET.as_millis(),
                    "session teardown abandoned unfinished turn-end hooks"
                );
                worker.abort();
            }
        }
    }
}

impl Drop for TurnEndQueue {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            self.disarm();
            worker.abort();
        }
    }
}

impl SessionActor {
    pub(super) fn has_enabled_hooks_for(&self, event: event::HookEventName) -> bool {
        let has_file_hooks = self
            .hook_registry
            .borrow()
            .as_ref()
            .is_some_and(|registry| registry.has_enabled_hooks_for_canonical(event));
        has_file_hooks || self.client_hooks.borrow().contains_key(&event.canonical())
    }

    pub(super) async fn last_assistant_message_for_cancel(&self) -> Option<String> {
        if !self.has_enabled_hooks_for(event::HookEventName::StopCancelled) {
            return None;
        }
        self.chat_state_handle
            .get_last_assistant_text_in_turn()
            .await
    }

    pub(super) fn report_turn_end(&self, prompt_id: &str, end: TurnEnd) -> ReportOutcome {
        self.claim_and_queue(prompt_id, self.turn_report.epoch(), end)
    }

    pub(super) fn claim_and_queue(
        &self,
        prompt_id: &str,
        epoch: TurnEpoch,
        end: TurnEnd,
    ) -> ReportOutcome {
        if self.startup_hints.is_subagent && end.is_inherited_interrupt() {
            return ReportOutcome::InheritedInterrupt;
        }
        let event = end.event_name();
        if !self.has_enabled_hooks_for(event) {
            return ReportOutcome::NoListener;
        }
        let Some(claim) = self.turn_report.claim_at(epoch) else {
            return ReportOutcome::AlreadyReported;
        };
        let report = TurnEndReport {
            prompt_id: prompt_id.to_string(),
            event,
            payload: end.into_payload(self.subagent_type_label()),
        };
        if !self.send_queue_item(QueueItem::Report(Box::new(report))) {
            // Dropping the uncommitted claim releases the slot for a later reporter.
            return ReportOutcome::QueueClosed;
        }
        match claim.commit() {
            CommitOutcome::Reported => ReportOutcome::Queued,
            CommitOutcome::LostToAnotherReporter => ReportOutcome::AlreadyReported,
        }
    }

    async fn dispatch_turn_end(&self, report: TurnEndReport) {
        self.dispatch_hook(report.event, report.payload, Some(&report.prompt_id), None)
            .await;
    }

    fn send_queue_item(&self, item: QueueItem) -> bool {
        self.turn_end_tx
            .borrow()
            .as_ref()
            .is_some_and(|tx| tx.send(item).is_ok())
    }
}

#[cfg(test)]
#[path = "turn_end_hooks_tests.rs"]
mod tests;
