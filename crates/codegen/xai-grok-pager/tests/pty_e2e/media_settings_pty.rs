// Per-test-case module for the `pty_e2e` integration test crate.
#[allow(unused_imports)]
use super::common::*;

/// F2 → filter `media` → assert the Media-understanding page renders its
/// toggle row. Guards the settings-modal media page end to end: registry
/// registration, row building, filtering, and rendering all agree.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "PTY e2e; run the owning pty_e2e_* Cargo test with --ignored (see Cargo.toml)"]
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

    // F2 opens the settings modal.
    harness.inject_keys(b"\x1bOQ").expect("F2 open settings");
    harness.update(Duration::from_millis(500));

    // Filter to the Media-understanding section.
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
        harness.contains_text("Enable media understanding"),
        "media filter must show the media-understanding rows\nscreen:\n{}",
        harness.screen_contents()
    );
    assert!(
        harness.contains_text("Media understanding"),
        "media page header must render\nscreen:\n{}",
        harness.screen_contents()
    );

    // Commit the filter and toggle the master switch via Space; the modal
    // must stay alive (no panic, row value flips).
    harness.inject_keys(b"\r").expect("commit filter");
    harness.update(Duration::from_millis(300));
    harness.inject_keys(b" ").expect("toggle media enabled");
    harness.update(Duration::from_millis(400));

    assert!(
        !harness.contains_text("panicked"),
        "pager panicked\nscreen:\n{}",
        harness.screen_contents()
    );

    // Esc out of the settings modal, then quit cleanly.
    harness.inject_keys(keys::ESC).expect("esc settings");
    harness.update(Duration::from_millis(300));
    harness.inject_keys(keys::ESC).expect("esc settings");
    harness.update(Duration::from_millis(300));

    harness.quit().expect("clean quit");
}
