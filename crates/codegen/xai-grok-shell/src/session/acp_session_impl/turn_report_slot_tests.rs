use super::*;

#[test]
fn stale_epoch_release_leaves_live_gate_claim_held() {
    let slot = TurnReportSlot::default();
    let stale = slot.epoch();
    slot.start_next_turn();

    let live = slot.claim_for_gate().expect("successor gate claim");
    slot.release_aborted(stale);

    assert!(matches!(slot.state(), TurnReportState::Held { .. }));
    assert_eq!(live.commit(), CommitOutcome::Reported);
}

#[test]
fn cancel_release_does_not_reopen_reported_slot() {
    let slot = TurnReportSlot::default();
    let claim = slot.claim_for_gate().expect("gate claim");
    assert_eq!(claim.commit(), CommitOutcome::Reported);

    slot.release_aborted(slot.epoch());

    assert_eq!(slot.state(), TurnReportState::Reported);
}

#[test]
fn released_claim_cannot_commit_over_successor() {
    let slot = TurnReportSlot::default();
    let epoch = slot.epoch();

    let first = slot.claim_for_gate().expect("first gate claim");
    slot.release_aborted(epoch);
    let second = slot.claim_for_gate().expect("successor gate claim");

    assert_eq!(first.commit(), CommitOutcome::LostToAnotherReporter);
    assert!(matches!(slot.state(), TurnReportState::Held { .. }));
    assert_eq!(second.commit(), CommitOutcome::Reported);
}

#[test]
fn cancel_cannot_release_report_claim() {
    let slot = TurnReportSlot::default();
    let epoch = slot.epoch();

    let report = slot.claim_at(epoch).expect("report claim");
    slot.release_aborted(epoch);

    assert!(matches!(slot.state(), TurnReportState::Held { .. }));
    assert!(slot.claim_at(epoch).is_none());
    assert_eq!(report.commit(), CommitOutcome::Reported);
}
