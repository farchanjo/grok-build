// Per-test-case module for the `pty_e2e_config_ui` integration test crate.
#[allow(unused_imports)]
use super::common::*;

/// F2 -> filter `media` -> verify the reconciled `[media]` settings render in
/// a real PTY without introducing a separate media tool or modal category.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "PTY e2e; run the owning pty_e2e_config_ui Cargo test with --ignored"]
async fn media_settings_pty() {
    let content = ContentController::start().await.expect("start content");
    content.set_response("MEDIA_SETTINGS_SENTINEL");

    let binary = pager_binary().expect("resolve pager binary");
    let mut harness =
        PtyHarness::spawn_with_content(&binary, DEFAULT_ROWS, DEFAULT_COLS, &content, &[])
            .expect("spawn pager");

    harness
        .wait_for_text(WELCOME_SCREEN_SENTINEL, WELCOME_TIMEOUT)
        .expect("welcome");

    harness.inject_keys(b"\x1bOQ").expect("F2 open settings");
    harness.update(Duration::from_millis(500));
    harness.inject_keys(b"/").expect("start filter");
    harness.update(Duration::from_millis(150));
    for ch in b"media" {
        harness
            .inject_keys(std::slice::from_ref(ch))
            .expect("filter char");
        harness.update(Duration::from_millis(30));
    }
    harness.update(Duration::from_millis(300));

    assert!(
        harness.contains_text("Media routing"),
        "media filter must show the routing row\nscreen:\n{}",
        harness.screen_contents()
    );
    assert!(
        harness.contains_text("Media status"),
        "media filter must show the effective status row\nscreen:\n{}",
        harness.screen_contents()
    );
    assert!(
        !harness.contains_text("panicked"),
        "pager panicked\nscreen:\n{}",
        harness.screen_contents()
    );

    harness.inject_keys(keys::ESC).expect("clear filter");
    harness.update(Duration::from_millis(250));
    harness.inject_keys(keys::ESC).expect("close settings");
    harness.update(Duration::from_millis(250));
    harness.quit().expect("clean quit");
}
