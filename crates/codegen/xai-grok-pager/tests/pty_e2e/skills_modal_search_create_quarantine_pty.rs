//! PTY coverage for `/skills` search, create wizard, quarantine visibility,
//! and that opening/search never starts regression.

#[allow(unused_imports)]
use super::common::*;

fn seed_skills(content: &ContentController) {
    let cwd = content.sandbox().workspace();
    let good = cwd.join(".grok/skills/commit");
    std::fs::create_dir_all(&good).expect("good skill dir");
    std::fs::write(
        good.join("SKILL.md"),
        "---\nname: commit\ndescription: Create well-formatted git commits.\n---\n\n# Commit\n",
    )
    .expect("write good skill");
    let bad = cwd.join(".grok/skills/bad");
    std::fs::create_dir_all(&bad).expect("bad skill dir");
    std::fs::write(bad.join("SKILL.md"), "no frontmatter\n").expect("write quarantined skill");
}

fn spawn_skills_harness(content: &ContentController, cols: u16) -> PtyHarness {
    let binary = pager_binary().expect("resolve pager binary");
    PtyHarness::spawn_with_content_in_dir(
        &binary,
        DEFAULT_ROWS,
        cols,
        content,
        &[],
        Some(content.sandbox().workspace()),
    )
    .expect("spawn pager in workspace")
}

/// Skills-tab chrome that cannot match the `/skills` slash row.
///
/// `SkillsCommand::description` is exactly `"View skills"`. Requiring that
/// copy to vanish fights leftover dropdown/help text after submit. The
/// Skills tab instead paints a labeled Local/Smart field plus list
/// shortcuts (`n new`, `/ search`) that the slash row never shows.
fn skills_modal_chrome_visible(screen: &str) -> bool {
    let chrome = screen.contains("n new")
        || screen.contains("/ search")
        || screen.contains("Local:")
        || screen.contains("Smart:");
    chrome && screen.contains("Skills") && !screen.contains("Grok Build Beta")
}

/// `/skills` submit must open the extensions modal, not send a prompt.
fn skills_slash_command_consumed(screen: &str) -> bool {
    !screen.contains("❯ /skills") && !screen.contains("❯ skills")
}

/// Search focus is modal-local: Local/Smart field with the list footer
/// hidden. Do not wait on the global keybar `"clear search"` — the Skills
/// modal blanks overlay shortcuts while `search_active`, and a clipped
/// shortcuts band never shows that string.
fn skills_search_focused(screen: &str) -> bool {
    let labeled = screen.contains("Local:") || screen.contains("Smart:");
    labeled
        && screen.contains("Skills")
        && !screen.contains("n new")
        && !screen.contains("/ search")
        && !screen.contains("Enter submit")
        && !screen.contains("Grok Build Beta")
}

/// Idle Skills list: search chrome is gone, create-skill `n` is live again.
fn skills_list_idle_for_create(screen: &str) -> bool {
    skills_modal_chrome_visible(screen)
        && !screen.contains("Enter submit")
        && (screen.contains("n new") || screen.contains("/ search") || screen.contains("Esc close"))
}

/// Create-wizard chrome is unique to input mode. `name`/`description` alone
/// can match expanded quarantine diagnostics (`missing-name`).
fn skills_create_wizard_visible(screen: &str) -> bool {
    screen.contains("Enter submit")
        && screen.contains("Esc cancel")
        && !screen.contains("n new")
        && !screen.contains("clear search")
}

fn promote_welcome_to_session(harness: &mut PtyHarness) {
    // The first composer character promotes welcome into a session and is
    // consumed. `/` cannot be that character: it is eaten and later letters
    // land in the session composer without a slash dropdown. Retry the
    // promoter until the logo is gone — a single key can be dropped while
    // the welcome overlay is still attaching the prompt.
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        let screen = harness.screen_contents();
        if !screen.contains("Grok Build Beta") && screen.contains("❯") {
            break;
        }
        let _ = harness.inject_keys(b"x");
        harness.update(Duration::from_millis(150));
    }
    let screen = harness.screen_contents();
    assert!(
        !screen.contains("Grok Build Beta") && screen.contains("❯"),
        "welcome should promote to a session\nscreen:\n{screen}"
    );
    harness.inject_keys(b"\x15").expect("Ctrl+U clear draft");
    harness
        .wait_until("empty session composer", Duration::from_secs(5), |h| {
            let screen = h.screen_contents();
            screen.contains("❯") && !screen.contains("❯ x") && !screen.contains("❯x")
        })
        .expect("draft should clear before /skills");
}

fn open_skills_modal(harness: &mut PtyHarness) {
    promote_welcome_to_session(harness);
    // Pace `/skills` so it is not paste-coalesced after the session exists.
    // Narrow terminals may hide the description column; the composer filter
    // (`❯ skills` / `/skills`) is enough to submit the command.
    inject_keys_paced(harness, b"/skills");
    harness
        .wait_until("slash skills filter", Duration::from_secs(10), |h| {
            let screen = h.screen_contents();
            screen.contains("View skills")
                || screen.contains("/skills")
                || screen.contains("❯ skills")
                || screen.contains("❯ /skills")
        })
        .expect("slash dropdown or composer should show skills");
    harness.inject_keys(b"\r").expect("submit /skills");
    harness
        .wait_until("skills modal chrome", Duration::from_secs(20), |h| {
            let screen = h.screen_contents();
            skills_modal_chrome_visible(&screen) && skills_slash_command_consumed(&screen)
        })
        .expect("skills modal should render");
    let screen = harness.screen_contents();
    assert!(
        !screen.contains("regressing"),
        "opening /skills must not start regression"
    );
}

#[test]
fn skills_modal_chrome_requires_tab_semantics_not_slash_row() {
    let slash = "❯ /skills\nView skills\nGrok Build Beta";
    assert!(
        !skills_modal_chrome_visible(slash),
        "slash dropdown copy must not count as Skills chrome"
    );
    assert!(!skills_slash_command_consumed(slash));

    let modal = "Skills\nLocal: \nn new\n/ search\nEsc close";
    assert!(skills_modal_chrome_visible(modal));
    assert!(skills_slash_command_consumed(modal));

    let leftover_slash_copy = "Skills\nLocal: \nn new\nView skills";
    assert!(
        skills_modal_chrome_visible(leftover_slash_copy),
        "leftover View skills description must not hide the Skills tab"
    );
}

#[test]
fn skills_search_focus_is_modal_local_not_global_keybar() {
    let idle = "Skills\nLocal: \nn new\n/ search\nEsc close";
    assert!(!skills_search_focused(idle));
    assert!(skills_list_idle_for_create(idle));

    let search_without_keybar = "Skills\nLocal: commit";
    assert!(
        skills_search_focused(search_without_keybar),
        "search focus is Local/Smart plus a hidden list footer, not clear search"
    );
    assert!(!skills_list_idle_for_create(search_without_keybar));

    let search_smart = "Skills\nSmart: \ncommit";
    assert!(skills_search_focused(search_smart));
    assert!(!skills_search_focused(
        "Skills\nLocal: \nclear search\nn new\n/ search"
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "PTY e2e; run the owning pty_e2e_* Cargo test with --ignored (see Cargo.toml)"]
async fn skills_modal_search_create_quarantine_reload_pty() {
    let content = ContentController::start().await.expect("start content");
    seed_skills(&content);

    let mut harness = spawn_skills_harness(&content, DEFAULT_COLS);
    harness
        .wait_for_text(WELCOME_SCREEN_SENTINEL, WELCOME_TIMEOUT)
        .expect("welcome");
    open_skills_modal(&mut harness);

    harness
        .wait_until("commit skill row", Duration::from_secs(15), |h| {
            h.contains_text("commit")
        })
        .expect("seeded commit skill should list");
    harness
        .wait_until("skills list idle for n", Duration::from_secs(8), |h| {
            skills_list_idle_for_create(&h.screen_contents())
        })
        .expect("list must be idle before search or create-skill n");

    // Skills search is slash-activated. Wait for modal-local search chrome
    // (Local/Smart field, list footer hidden), not the global keybar.
    inject_keys_paced(&mut harness, b"/");
    harness
        .wait_until("skills search field", Duration::from_secs(5), |h| {
            skills_search_focused(&h.screen_contents())
        })
        .expect("search field should activate");
    inject_keys_paced(&mut harness, b"commit");
    harness
        .wait_until("filtered commit row", Duration::from_secs(5), |h| {
            h.contains_text("commit") && skills_search_focused(&h.screen_contents())
        })
        .expect("search should keep the commit row");
    assert!(
        !harness.screen_contents().contains("regressing"),
        "searching must not start regression"
    );

    harness.inject_keys(keys::ESC).expect("esc search");
    harness
        .wait_until("search deactivated", Duration::from_secs(8), |h| {
            skills_list_idle_for_create(&h.screen_contents())
        })
        .expect("search must deactivate before create-skill n");

    harness.inject_keys(b"n").expect("create wizard");
    harness
        .wait_until("create-skill wizard", Duration::from_secs(8), |h| {
            skills_create_wizard_visible(&h.screen_contents())
        })
        .expect("create wizard should open from n");

    harness.inject_keys(keys::ESC).expect("close wizard");
    harness
        .wait_until("wizard closed", Duration::from_secs(8), |h| {
            skills_list_idle_for_create(&h.screen_contents())
        })
        .expect("create wizard should close");
    harness.inject_keys(b"r").expect("reload");
    harness
        .wait_until("skills reload idle", Duration::from_secs(10), |h| {
            let screen = h.screen_contents();
            skills_list_idle_for_create(&screen) && !screen.contains("regressing")
        })
        .expect("reload must not start regression");
    harness.quit().expect("clean quit");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "PTY e2e; run the owning pty_e2e_* Cargo test with --ignored (see Cargo.toml)"]
async fn skills_modal_prime_index_progress_and_failure_fit_footer_pty() {
    let content = ContentController::start().await.expect("start content");
    seed_skills(&content);

    let mut harness = spawn_skills_harness(&content, 48);
    harness
        .wait_for_text(WELCOME_SCREEN_SENTINEL, WELCOME_TIMEOUT)
        .expect("welcome");
    open_skills_modal(&mut harness);
    harness.inject_keys(b"b").expect("backfill");
    harness
        .wait_until("prime index footer idle", Duration::from_secs(8), |h| {
            let screen = h.screen_contents();
            !screen.contains("http://") && !screen.contains("sk-")
        })
        .expect("progress/failure footer must not leak endpoints or credentials");
    let screen = harness.screen_contents();
    assert!(
        !screen.contains("http://"),
        "progress/failure footer must not leak endpoints"
    );
    assert!(
        !screen.contains("sk-"),
        "progress/failure footer must not leak credentials"
    );
    harness.quit().expect("clean quit");
}
