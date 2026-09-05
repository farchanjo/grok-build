// Per-test-case module for the `pty_e2e` integration test crate.
#[allow(unused_imports)]
use super::common::*;

/// **Ctrl+C cancels a running turn from the SCROLLBACK pane; Esc does not.**
/// The policy treats Prompt and Scrollback identically while a turn runs: both
/// swallow Esc (cancel is Ctrl+C-only now) and both take Ctrl+C. Tab (not Esc)
/// is used to leave the prompt; the footer's "Space:prompt" hint confirms the
/// scrollback owns keys before the keys are sent.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn esc_does_not_cancel_ctrl_c_does_from_scrollback() {
    let content = ContentController::start().await.expect("start content");
    let long_response = format!(
        "{MOCK_RESPONSE_SENTINEL} {}",
        "streaming filler words for the cancellation window. ".repeat(120)
    );
    content.set_response(long_response);
    content.set_chunk_delay(Some(Duration::from_millis(50)));

    let binary = pager_binary().expect("resolve pager binary");
    let mut harness =
        PtyHarness::spawn_with_content(&binary, DEFAULT_ROWS, DEFAULT_COLS, &content, &[])
            .expect("spawn pager");

    harness
        .wait_for_text(WELCOME_SCREEN_SENTINEL, WELCOME_TIMEOUT)
        .expect("welcome text");

    harness
        .inject_keys(format!("{PROMPT}\r").as_bytes())
        .expect("submit prompt");
    harness
        .wait_for_text(MOCK_RESPONSE_SENTINEL, Duration::from_secs(30))
        .expect("stream started");

    // Leave the prompt with a SINGLE Tab (Esc is reserved for clear/rewind
    // only), then wait for the footer to prove the scrollback owns keys. Tab
    // TOGGLES focus, so re-pressing it could bounce focus back to the prompt —
    // press once and poll the render instead (mirrors `drive_to_scrollback_with_turn`).
    harness.inject_keys(b"\t").expect("tab to scrollback");
    harness
        .wait_for_text("Space:prompt", Duration::from_secs(10))
        .expect("scrollback must own keys before the cancel keys");

    // 1× Esc from scrollback must NOT cancel the running turn.
    harness.inject_keys(keys::ESC).expect("press esc");
    harness.update(Duration::from_millis(1500));
    let screen = harness.screen_contents();
    assert!(
        !screen.contains("Turn cancelled by user"),
        "Esc must NOT cancel a running turn anymore\nscreen:\n{screen}"
    );

    // Ctrl+C from scrollback cancels the running turn.
    harness
        .inject_keys(keys::CTRL_C)
        .expect("press ctrl+c to cancel");

    harness
        .wait_for_text("Turn cancelled by user", Duration::from_secs(15))
        .expect("turn cancelled marker (from scrollback)");

    harness.update(Duration::from_millis(600));
    let screen = harness.screen_contents();
    assert_eq!(
        screen.matches("Turn cancelled by user").count(),
        1,
        "'Turn cancelled' must appear exactly once\nscreen:\n{screen}"
    );
    assert!(
        !harness.contains_text("panicked"),
        "pager panicked\nscreen:\n{}",
        harness.screen_contents()
    );

    harness.quit().expect("clean quit");
}
