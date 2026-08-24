//! Focused PTY coverage for `/agents`, `/retrieval-settings`, `/context`,
//! and `/skills` secret-free rendering.

#[allow(unused_imports)]
use super::common::*;

fn assert_secret_free(screen: &str, surface: &str) {
    for leak in [
        "sk-",
        "sk_live",
        "BEGIN PRIVATE",
        "/Users/",
        "/home/",
        "file://",
        "http://",
        "Authorization",
        "Bearer ",
        "SECRET-BODY",
        "0.39215687",
        "raw provider error",
    ] {
        assert!(
            !screen.contains(leak),
            "{surface} leaked {leak}\nscreen:\n{screen}"
        );
    }
}

fn seed_skills(content: &ContentController) {
    let cwd = content.sandbox().workspace();
    let good = cwd.join(".grok/skills/commit");
    std::fs::create_dir_all(&good).expect("good skill dir");
    std::fs::write(
        good.join("SKILL.md"),
        "---\nname: commit\ndescription: Create well-formatted git commits.\nmetadata:\n  grok:\n    when-to-use: commit changes\n---\n\n# Commit\n",
    )
    .expect("write good skill");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "PTY e2e; run the owning pty_e2e_config_ui Cargo test with --ignored"]
async fn agents_modal_compact_index_state_pty() {
    let content = ContentController::start().await.expect("start content");
    seed_skills(&content);
    let binary = pager_binary().expect("resolve pager binary");
    let mut harness =
        PtyHarness::spawn_with_content(&binary, DEFAULT_ROWS, DEFAULT_COLS, &content, &[])
            .expect("spawn pager");
    harness
        .wait_for_text(WELCOME_SCREEN_SENTINEL, WELCOME_TIMEOUT)
        .expect("welcome");
    harness.inject_keys(b"/agents\r").expect("open agents");
    let deadline = Instant::now() + Duration::from_secs(12);
    let mut saw = false;
    while Instant::now() < deadline {
        let screen = harness.screen_contents();
        if screen.contains("Agents") || screen.contains("Personas") {
            saw = true;
            assert_secret_free(&screen, "/agents");
            break;
        }
        harness.update(Duration::from_millis(150));
    }
    assert!(saw, "agents modal should render");
    harness.inject_keys(b"\x1b").expect("close");
    harness.quit().expect("clean quit");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "PTY e2e; run the owning pty_e2e_config_ui Cargo test with --ignored"]
async fn retrieval_settings_skills_agents_pages_pty() {
    let content = ContentController::start().await.expect("start content");
    seed_skills(&content);
    let binary = pager_binary().expect("resolve pager binary");
    let mut harness =
        PtyHarness::spawn_with_content(&binary, DEFAULT_ROWS, DEFAULT_COLS, &content, &[])
            .expect("spawn pager");
    harness
        .wait_for_text(WELCOME_SCREEN_SENTINEL, WELCOME_TIMEOUT)
        .expect("welcome");
    harness
        .inject_keys(b"/retrieval-settings\r")
        .expect("open retrieval settings");
    let deadline = Instant::now() + Duration::from_secs(12);
    let mut saw = false;
    while Instant::now() < deadline {
        let screen = harness.screen_contents();
        if screen.contains("Embeddings")
            || screen.contains("Prime")
            || screen.contains("Retrieval")
            || screen.contains("Profiles")
        {
            saw = true;
            assert_secret_free(&screen, "/retrieval-settings");
            break;
        }
        harness.update(Duration::from_millis(150));
    }
    assert!(saw, "retrieval settings should render");
    harness.inject_keys(b"\t").expect("next page");
    harness.update(Duration::from_millis(200));
    assert_secret_free(&harness.screen_contents(), "/retrieval-settings tab");
    harness.inject_keys(b"\x1b").expect("close");
    harness.quit().expect("clean quit");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "PTY e2e; run the owning pty_e2e_config_ui Cargo test with --ignored"]
async fn context_prime_redaction_pty() {
    let content = ContentController::start().await.expect("start content");
    content.set_response(format!(
        "{MOCK_RESPONSE_SENTINEL} hello from the mock inference server."
    ));
    seed_skills(&content);
    let binary = pager_binary().expect("resolve pager binary");
    let mut harness =
        PtyHarness::spawn_with_content(&binary, DEFAULT_ROWS, DEFAULT_COLS, &content, &[])
            .expect("spawn pager");
    harness
        .wait_for_text(WELCOME_SCREEN_SENTINEL, WELCOME_TIMEOUT)
        .expect("welcome");
    harness
        .inject_keys(format!("{PROMPT}\r").as_bytes())
        .expect("submit prompt");
    harness
        .wait_for_text(MOCK_RESPONSE_SENTINEL, Duration::from_secs(30))
        .expect("mock response");
    harness.inject_keys(b"/context\r").expect("open context");
    let deadline = Instant::now() + Duration::from_secs(12);
    let mut saw = false;
    while Instant::now() < deadline {
        let screen = harness.screen_contents();
        if screen.contains("Context") || screen.contains("tokens") {
            saw = true;
            assert_secret_free(&screen, "/context");
            break;
        }
        harness.update(Duration::from_millis(150));
    }
    assert!(saw, "context view should render");
    harness.quit().expect("clean quit");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "PTY e2e; run the owning pty_e2e_config_ui Cargo test with --ignored"]
async fn skills_modal_secret_free_pty() {
    let content = ContentController::start().await.expect("start content");
    seed_skills(&content);
    let binary = pager_binary().expect("resolve pager binary");
    let mut harness = PtyHarness::spawn_with_content(&binary, DEFAULT_ROWS, 48, &content, &[])
        .expect("spawn pager");
    harness
        .wait_for_text(WELCOME_SCREEN_SENTINEL, WELCOME_TIMEOUT)
        .expect("welcome");
    harness.inject_keys(b"/skills\r").expect("open skills");
    let deadline = Instant::now() + Duration::from_secs(12);
    let mut saw = false;
    while Instant::now() < deadline {
        let screen = harness.screen_contents();
        if screen.contains("commit") || screen.contains("Skills") {
            saw = true;
            assert_secret_free(&screen, "/skills");
            assert!(
                !screen.contains("regressing"),
                "opening /skills must not start regression"
            );
            break;
        }
        harness.update(Duration::from_millis(150));
    }
    assert!(saw, "skills modal should render");
    harness.quit().expect("clean quit");
}
