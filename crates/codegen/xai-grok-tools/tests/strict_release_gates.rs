//! Hermetic bundled/plugin author and strict-skills release gates.
//!
//! No network, no live providers, no GROK_HOME mutation beyond process-local
//! reads of in-crate testdata.

use std::fs;
use std::path::{Path, PathBuf};

use xai_grok_tools::implementations::skills::strict::{
    AuthorKind, SkillDiagnosticCode, SkillHealthStatus, StrictSkillOutcome, check_author_skill_dir,
    content_has_legacy_top_level_grok_key, text_leaks_secrets, validate_strict_skill_dir,
};

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/implementations/skills/strict/testdata")
}

fn walk_skill_md(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().and_then(|n| n.to_str()) == Some("SKILL.md") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

fn parent_dir(path: &Path) -> PathBuf {
    path.parent().unwrap().to_path_buf()
}

#[test]
fn official_and_grok_accepted_fixtures_remain_valid() {
    for rel in [
        "official-accepted",
        "grok-accepted",
        "author/bundled",
        "author/plugin/review",
    ] {
        let root = testdata().join(rel);
        for skill_md in walk_skill_md(&root) {
            let dir = parent_dir(&skill_md);
            match validate_strict_skill_dir(&dir, None) {
                StrictSkillOutcome::Valid(_) => {}
                StrictSkillOutcome::Quarantined(row) => panic!(
                    "{} must stay valid, got {:?}",
                    dir.file_name().unwrap().to_string_lossy(),
                    row.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>()
                ),
            }
            let content = fs::read_to_string(&skill_md).unwrap();
            assert!(
                !content_has_legacy_top_level_grok_key(&content),
                "{} must not use legacy top-level Grok keys",
                dir.file_name().unwrap().to_string_lossy()
            );
        }
    }
}

#[test]
fn rejected_and_privacy_fixtures_quarantine_without_leaking() {
    for rel in [
        "official-rejected",
        "grok-rejected",
        "privacy",
        "author/plugin/quarantined",
    ] {
        let root = testdata().join(rel);
        for skill_md in walk_skill_md(&root) {
            let dir = parent_dir(&skill_md);
            let outcome = validate_strict_skill_dir(&dir, None);
            match &outcome {
                StrictSkillOutcome::Quarantined(row) => {
                    assert!(!row.diagnostics.is_empty());
                    for diagnostic in &row.diagnostics {
                        let line = diagnostic.stable_line();
                        if let Some(token) = text_leaks_secrets(&line) {
                            panic!("diagnostic leaked {token}: {line}");
                        }
                    }
                }
                StrictSkillOutcome::Valid(_) => {
                    panic!(
                        "{} must remain quarantined",
                        dir.file_name().unwrap().to_string_lossy()
                    )
                }
            }
        }
    }
}

#[test]
fn bundled_and_plugin_author_gates_are_hermetic() {
    let bundled = check_author_skill_dir(
        AuthorKind::Bundled,
        &testdata().join("author/bundled/commit"),
    );
    assert_eq!(bundled.status, SkillHealthStatus::ValidPass);
    assert!(bundled.is_publishable());

    let plugin =
        check_author_skill_dir(AuthorKind::Plugin, &testdata().join("author/plugin/review"));
    assert_eq!(plugin.status, SkillHealthStatus::ValidPass);
    assert!(plugin.is_publishable());

    let rejected = check_author_skill_dir(
        AuthorKind::Plugin,
        &testdata().join("author/plugin/quarantined"),
    );
    assert_eq!(rejected.status, SkillHealthStatus::Quarantined);
    assert!(!rejected.is_publishable());
    assert!(
        rejected
            .diagnostics
            .iter()
            .any(|d| d.code == SkillDiagnosticCode::UnexpectedTopLevelKey)
    );
}

#[test]
fn leaky_privacy_fixture_never_echoes_secret_material() {
    let dir = testdata().join("privacy/leaky");
    let outcome = validate_strict_skill_dir(&dir, None);
    let StrictSkillOutcome::Quarantined(row) = outcome else {
        panic!("leaky fixture must quarantine");
    };
    let blob = serde_json::to_string(&row).unwrap();
    assert!(
        !blob.contains("SUPERSECRETVALUE"),
        "quarantine JSON must not echo secret-like frontmatter values"
    );
    if let Some(token) = text_leaks_secrets(&blob) {
        panic!("privacy fixture leaked {token}");
    }
}
