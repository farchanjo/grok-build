// Per-test-case module for the `pty_e2e` integration test crate.
#[allow(unused_imports)]
use super::common::*;

/// Distinctive tokens the removed surfaces would have painted.
const CRITICAL_MSG: &str = "ZZCRITICALBANNERZZ";
const PROMO_LABEL: &str = "Click here to Upgrade";
const PROMO_MSG: &str = "ZZPROMOMESSAGEZZ";

/// This build removed the announcement surfaces: a settings publish carrying
/// both a critical and a pinned promo announcement must never paint anything —
/// no welcome-hero block, no promo `[label]` CTA, no in-session banner — and
/// the session must still be fully usable.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "PTY e2e; run the owning pty_e2e_* Cargo test with --ignored (see Cargo.toml)"]
async fn published_announcements_never_paint() {
    let content = ContentController::start().await.expect("start content");
    content.set_response(format!("{MOCK_RESPONSE_SENTINEL} announcements removed."));
    content.server().set_settings(json!({
        "allow_access": true,
        "announcements": [
            {
                "id": "pty-crit",
                "title": "Outage",
                "message": CRITICAL_MSG,
                "severity": "critical",
            },
            {
                "id": "pty-promo",
                "message": PROMO_MSG,
                "severity": "promo",
                "dismissible": false,
                "cta": {
                    "label": PROMO_LABEL,
                    "url": "https://x.ai/zz-promo-cta",
                    "caption": "or use Ctrl+O",
                },
            },
        ],
    }));
    seed_fake_oauth(&content, "pty-announce-removed");
    let binary = pager_binary().expect("resolve pager binary");
    let mut harness = PtyHarness::spawn_with_content_env_ops(
        &binary,
        DEFAULT_ROWS,
        DEFAULT_COLS,
        &content,
        &[],
        &oauth_credential_ops(),
    )
    .expect("spawn pager");

    harness
        .wait_for_text(WELCOME_SCREEN_SENTINEL, WELCOME_TIMEOUT)
        .expect("welcome renders");
    harness.update(Duration::from_secs(5));
    let screen = harness.screen_contents();
    assert!(
        !screen.contains(CRITICAL_MSG),
        "the critical announcement must not paint on the welcome screen\nscreen:\n{screen}"
    );
    assert!(
        !screen.contains(PROMO_MSG) && !screen.contains(PROMO_LABEL),
        "the promo announcement and its CTA must not paint on the welcome screen\nscreen:\n{screen}"
    );

    // The session surface must be equally clean: no banner above the prompt.
    harness
        .inject_keys(format!("{PROMPT}\r").as_bytes())
        .expect("submit prompt");
    harness
        .wait_for_text(MOCK_RESPONSE_SENTINEL, Duration::from_secs(30))
        .expect("session response");
    harness.update(Duration::from_secs(3));
    let screen = harness.screen_contents();
    assert!(
        !screen.contains(CRITICAL_MSG) && !screen.contains(PROMO_LABEL),
        "no announcement may paint in-session\nscreen:\n{screen}"
    );

    harness.quit().expect("clean quit");
}
