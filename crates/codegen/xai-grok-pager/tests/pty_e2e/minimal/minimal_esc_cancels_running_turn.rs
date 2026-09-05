// Per-test-case module for the `pty_e2e` integration test crate.
#[allow(unused_imports)]
use crate::common::*;

/// Ctrl+C is the sole mid-turn cancel gesture: Esc must NOT cancel a running
/// turn (even in minimal mode, where it used to), while Ctrl+C still does. The
/// test streams a paced response, presses Esc (turn keeps streaming, no cancel
/// marker), then Ctrl+C (cancellation marker committed to native scrollback).
/// The cancellation marker is finalized and committed like any other block.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn minimal_esc_does_not_cancel_ctrl_c_does() {
    let content = ContentController::start().await.expect("start content");
    // Paced, long stream so the turn is provably still running when each key lands.
    let long = format!(
        "{MOCK_RESPONSE_SENTINEL} {}",
        "streaming filler words for the cancellation window. ".repeat(120)
    );
    content.set_response(long);
    content.set_chunk_delay(Some(Duration::from_millis(50)));

    let mut harness = spawn_minimal(&content);
    wait_minimal_ready(&mut harness);

    harness
        .inject_keys(format!("{PROMPT}\r").as_bytes())
        .expect("submit prompt");
    harness
        .wait_for_text(MOCK_RESPONSE_SENTINEL, Duration::from_secs(30))
        .expect("turn streaming in the live tail");

    // Esc must be a no-op: no cancel marker may appear after pressing it.
    harness.inject_keys(keys::ESC).expect("press esc");
    std::thread::sleep(Duration::from_millis(1500));
    harness
        .wait_for_text(MOCK_RESPONSE_SENTINEL, Duration::from_millis(500))
        .expect("turn still streaming after Esc");
    assert!(
        !harness.contains_text("Turn cancelled by user"),
        "Esc must NOT cancel a running turn anymore\nscreen:\n{}",
        harness.screen_contents()
    );

    // Ctrl+C is the cancel key: the marker must commit to scrollback.
    harness
        .inject_keys(keys::CTRL_C)
        .expect("press ctrl+c to cancel");

    // Full-text: minimal commits the cancel marker to native scrollback, so it
    // may sit above the pinned viewport — check scrollback + screen.
    harness
        .wait_for_full_text("Turn cancelled by user", Duration::from_secs(15))
        .expect("cancellation marker committed to scrollback");
    assert!(
        !harness.contains_text("panicked"),
        "pager panicked\nscreen:\n{}",
        harness.screen_contents()
    );

    quit_minimal(&mut harness);
}
