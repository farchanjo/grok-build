#![cfg_attr(rustfmt, rustfmt::skip)]
use super::*;

/// Build an `x.ai/asset_job_event` notification exactly as the shell emits it
/// (a `SessionNotification` envelope wrapping the flat job payload).
fn make_asset_job_notif(
    session_id: &str,
    job_id: &str,
    state: &str,
    bytes_transferred: u64,
    bytes_total: Option<u64>,
) -> acp::ExtNotification {
    make_asset_job_notif_with_error(session_id, job_id, state, bytes_transferred, bytes_total, None)
}

fn make_asset_job_notif_with_error(
    session_id: &str,
    job_id: &str,
    state: &str,
    bytes_transferred: u64,
    bytes_total: Option<u64>,
    error: Option<&str>,
) -> acp::ExtNotification {
    let notif = SessionNotification {
        session_id: acp::SessionId::new(session_id),
        update: XaiSessionUpdate::AssetJobEvent {
            job_id: job_id.into(),
            kind: "upload".into(),
            key: "uploads/photo.png".into(),
            backend: "s3".into(),
            state: state.into(),
            bytes_transferred,
            bytes_total,
            error: error.map(str::to_string),
        },
        meta: None,
    };
    let raw = serde_json::value::to_raw_value(&notif).unwrap();
    acp::ExtNotification::new("x.ai/asset_job_event", std::sync::Arc::from(raw))
}

#[test]
fn asset_job_event_creates_a_running_row() {
    let mut app = make_app_with_agent("sess-1");
    let id = AgentId(0);

    assert!(
        handle_ext_notification(
            &make_asset_job_notif("sess-1", "job-1", "running", 512, Some(4096)),
            &mut app,
        ),
        "a live transfer must count as agent activity"
    );

    let transfer = app.agents[&id]
        .session
        .transfers
        .get("job-1")
        .expect("first event must insert the transfer");
    assert_eq!(transfer.status, TransferStatus::Running);
    assert_eq!(transfer.bytes_transferred, 512);
    assert_eq!(transfer.bytes_total, Some(4096));
    assert_eq!(transfer.percent(), Some(12));
    assert_eq!(transfer.key, "uploads/photo.png");
    assert_eq!(transfer.backend, "s3");
    assert!(!transfer.progress.is_empty(), "progress line must be recorded");
}

#[test]
fn asset_job_event_updates_the_existing_row() {
    let mut app = make_app_with_agent("sess-1");
    let id = AgentId(0);

    handle_ext_notification(
        &make_asset_job_notif("sess-1", "job-1", "queued", 0, None),
        &mut app,
    );
    handle_ext_notification(
        &make_asset_job_notif("sess-1", "job-1", "running", 2048, Some(4096)),
        &mut app,
    );

    let transfers = &app.agents[&id].session.transfers;
    assert_eq!(transfers.len(), 1, "one row per job, not one per event");
    let transfer = transfers.get("job-1").unwrap();
    assert_eq!(transfer.status, TransferStatus::Running);
    assert_eq!(transfer.bytes_transferred, 2048);
}

#[test]
fn terminal_event_finishes_the_row_and_clears_pending_kill() {
    let mut app = make_app_with_agent("sess-1");
    let id = AgentId(0);

    handle_ext_notification(
        &make_asset_job_notif("sess-1", "job-1", "running", 1024, Some(4096)),
        &mut app,
    );
    {
        let transfer = app.agents[&id].session.transfers.get_mut("job-1").unwrap();
        transfer.pending_kill = true;
        transfer.kill_requested_at = Some(Instant::now());
    }

    handle_ext_notification(
        &make_asset_job_notif("sess-1", "job-1", "cancelled", 1024, Some(4096)),
        &mut app,
    );

    let transfer = app.agents[&id].session.transfers.get("job-1").unwrap();
    assert_eq!(transfer.status, TransferStatus::Cancelled);
    assert!(transfer.ended_at.is_some(), "terminal event stamps end time");
    assert!(!transfer.pending_kill, "the cancel landed; clear the spinner");
    assert!(transfer.kill_requested_at.is_none());
}

#[test]
fn trailing_progress_after_terminal_keeps_the_terminal_state() {
    let mut app = make_app_with_agent("sess-1");
    let id = AgentId(0);

    handle_ext_notification(
        &make_asset_job_notif("sess-1", "job-1", "completed", 4096, Some(4096)),
        &mut app,
    );
    // A late coalesced frame from before the completion must not un-finish it.
    handle_ext_notification(
        &make_asset_job_notif("sess-1", "job-1", "running", 3000, Some(4096)),
        &mut app,
    );

    let transfer = app.agents[&id].session.transfers.get("job-1").unwrap();
    assert_eq!(transfer.status, TransferStatus::Done);
    assert_eq!(transfer.bytes_transferred, 3000);
}

#[test]
fn failed_event_records_the_secret_free_error() {
    let mut app = make_app_with_agent("sess-1");
    let id = AgentId(0);

    handle_ext_notification(
        &make_asset_job_notif_with_error(
            "sess-1",
            "job-1",
            "failed",
            12,
            Some(4096),
            Some("connection reset"),
        ),
        &mut app,
    );

    let transfer = app.agents[&id].session.transfers.get("job-1").unwrap();
    assert_eq!(transfer.status, TransferStatus::Failed);
    assert_eq!(transfer.error.as_deref(), Some("connection reset"));
    assert!(transfer.ended_at.is_some());
}

/// The pager renders only what the shell already coalesced, so N progress
/// frames must collapse into ONE row whose bytes track the latest frame —
/// never N rows and never a per-frame UI rebuild.
#[test]
fn many_progress_events_collapse_into_one_row() {
    let mut app = make_app_with_agent("sess-1");
    let id = AgentId(0);

    for step in 1..=25u64 {
        handle_ext_notification(
            &make_asset_job_notif("sess-1", "job-1", "running", step * 100, Some(10_000)),
            &mut app,
        );
    }

    let transfers = &app.agents[&id].session.transfers;
    assert_eq!(transfers.len(), 1);
    assert_eq!(transfers.get("job-1").unwrap().bytes_transferred, 2500);
}

#[test]
fn unknown_state_degrades_to_running() {
    let mut app = make_app_with_agent("sess-1");
    let id = AgentId(0);

    handle_ext_notification(
        &make_asset_job_notif("sess-1", "job-1", "retrying", 1, None),
        &mut app,
    );

    assert_eq!(
        app.agents[&id].session.transfers.get("job-1").unwrap().status,
        TransferStatus::Running,
        "an unknown state from a newer shell must stay non-terminal"
    );
}

#[test]
fn malformed_payload_is_rejected() {
    let mut app = make_app_with_agent("sess-1");
    let raw = serde_json::value::to_raw_value(&serde_json::json!({ "nope": true })).unwrap();
    let notif = acp::ExtNotification::new("x.ai/asset_job_event", std::sync::Arc::from(raw));

    assert!(!handle_ext_notification(&notif, &mut app));
    assert!(app.agents[&AgentId(0)].session.transfers.is_empty());
}

#[test]
fn unrelated_session_is_ignored() {
    let mut app = make_app_with_agent("sess-1");

    assert!(!handle_ext_notification(
        &make_asset_job_notif("sess-unknown", "job-1", "running", 1, None),
        &mut app,
    ));
    assert!(app.agents[&AgentId(0)].session.transfers.is_empty());
}

/// A terminal event is delivered even when the transfer was never seen before
/// (warm reconnect): the row is inserted and finished in one shot so the
/// "done" state is never lost.
#[test]
fn terminal_event_for_unknown_job_inserts_a_finished_row() {
    let mut app = make_app_with_agent("sess-1");
    let id = AgentId(0);

    handle_ext_notification(
        &make_asset_job_notif("sess-1", "job-late", "completed", 4096, Some(4096)),
        &mut app,
    );

    let transfer = app.agents[&id].session.transfers.get("job-late").unwrap();
    assert_eq!(transfer.status, TransferStatus::Done);
    assert!(!transfer.status.is_live());
}