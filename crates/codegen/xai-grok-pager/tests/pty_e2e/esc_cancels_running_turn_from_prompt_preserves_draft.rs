// Per-test-case module for the `pty_e2e` integration test crate.
#[allow(unused_imports)]
use super::common::*;

/// **Ctrl+C is the sole mid-turn cancel gesture.** From the PROMPT pane with a
/// non-empty draft: Esc is swallowed (no cancel, draft preserved), the first
/// Ctrl+C clears the draft and keeps the turn, and the second Ctrl+C (empty
/// prompt) cancels. Proves the real binary routes a bare Esc through
/// `try_handle_esc_policy`'s mid-turn swallow (no cancel, no idle-clear arm)
/// and that the classic two-step Ctrl+C gesture still cancels.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn esc_does_not_cancel_ctrl_c_twice_does_from_prompt() {
    let content = ContentController::start().await.expect("start content");
    // Long paced stream so the turn is still visibly running when keys land.
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

    // Type a draft into the prompt WHILE the turn streams (prompt stays focused
    // after submit). A distinctive single token avoids any wrapping ambiguity.
    let draft = "DRAFTKEEPME";
    harness.inject_keys(draft.as_bytes()).expect("type draft");
    harness
        .wait_for_text(draft, Duration::from_secs(10))
        .expect("draft renders in the composer");

    // 1× Esc must be a no-op: no cancel, no idle-clear arm, draft intact.
    harness.inject_keys(keys::ESC).expect("press esc");
    harness.update(Duration::from_millis(1500));
    let screen = harness.screen_contents();
    assert!(
        !screen.contains("Turn cancelled by user"),
        "Esc must NOT cancel a running turn anymore\nscreen:\n{screen}"
    );
    assert!(
        screen.contains(draft),
        "Esc must leave the draft untouched\nscreen:\n{screen}"
    );
    assert!(
        !screen.contains("press again to clear"),
        "mid-turn Esc must never arm the idle clear\nscreen:\n{screen}"
    );

    // First Ctrl+C clears the draft and keeps the turn running.
    harness
        .inject_keys(keys::CTRL_C)
        .expect("first ctrl+c (clear draft)");
    harness.update(Duration::from_millis(400));
    let screen = harness.screen_contents();
    assert!(
        !screen.contains("Turn cancelled by user"),
        "first Ctrl+C with a draft clears it, not the turn\nscreen:\n{screen}"
    );

    // Second Ctrl+C on the now-empty prompt cancels the turn.
    harness
        .inject_keys(keys::CTRL_C)
        .expect("second ctrl+c (cancel)");
    harness
        .wait_for_text("Turn cancelled by user", Duration::from_secs(15))
        .expect("turn cancelled marker");

    assert!(
        !harness.contains_text("panicked"),
        "pager panicked\nscreen:\n{}",
        harness.screen_contents()
    );

    harness.quit().expect("clean quit");
}
