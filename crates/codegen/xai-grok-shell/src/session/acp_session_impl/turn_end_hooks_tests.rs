use super::*;
use crate::session::events::CancellationCategory as Category;

fn cancelled(category: Option<Category>) -> PromptCompletionKind {
    PromptCompletionKind::Cancelled {
        category,
        context: None,
    }
}

#[test]
fn classifies_interactive_cancel_but_excludes_replace_rewind_and_teardown() {
    for trigger in [Some("ctrl_c"), Some("esc"), Some("stop_button")] {
        assert_eq!(
            cancel_reason_for_trigger(trigger),
            Some(StopCancelledReason::UserInterrupt)
        );
    }
    for trigger in [
        Some("send_now"),
        Some("compaction_persistence_indeterminate"),
        None,
    ] {
        assert_eq!(cancel_reason_for_trigger(trigger), None);
    }
}

#[test]
fn classifies_completion_reasons_exhaustively() {
    let cases = [
        (
            cancelled(Some(Category::PermissionRejected)),
            Some(StopCancelledReason::PermissionRejected),
        ),
        (
            cancelled(Some(Category::PermissionCancelled)),
            Some(StopCancelledReason::PermissionCancelled),
        ),
        (
            cancelled(Some(Category::MidTurnAbort)),
            Some(StopCancelledReason::UserInterrupt),
        ),
        (
            cancelled(Some(Category::ActionStationarity)),
            Some(StopCancelledReason::NoProgress),
        ),
        (
            cancelled(Some(Category::HookDenied)),
            Some(StopCancelledReason::Unknown),
        ),
        (cancelled(None), Some(StopCancelledReason::Unknown)),
        (
            PromptCompletionKind::MaxTurnsReached { limit: 5 },
            Some(StopCancelledReason::MaxTurns),
        ),
        (PromptCompletionKind::Completed, None),
        (PromptCompletionKind::Rewound, None),
        (PromptCompletionKind::RemovedFromQueue, None),
    ];
    for (kind, expected) in cases {
        assert_eq!(cancel_reason_for_completion(&kind), expected);
    }
}

#[test]
fn cancel_detail_names_subject_and_reason() {
    let kind = PromptCompletionKind::Cancelled {
        category: Some(Category::PermissionRejected),
        context: Some(crate::session::CancellationContext {
            tool_name: Some("read_file".into()),
            hook_name: None,
            reason: Some("user declined".into()),
            trigger: None,
        }),
    };
    assert_eq!(
        cancel_details(&kind),
        Some("read_file: user declined".into())
    );
}
