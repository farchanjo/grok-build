//! Official skills-ref and Grok-extension parity fixtures.

use std::path::{Path, PathBuf};

use super::diagnostic::SkillDiagnosticCode;
use super::spec::SKILL_MD_FILE_NAME;
use super::validator::{
    StrictSkillInput, StrictSkillOutcome, validate_strict_skill, validate_strict_skill_dir,
};

fn testdata_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/implementations/skills/strict/testdata")
}

fn load(rel: &str) -> (String, String) {
    let path = testdata_root().join(rel);
    let parent = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .expect("fixture parent")
        .to_string();
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read fixture {}: {err}", path.display()));
    (parent, content)
}

fn validate_file(rel: &str) -> StrictSkillOutcome {
    let (parent, content) = load(rel);
    validate_strict_skill(StrictSkillInput {
        file_name: SKILL_MD_FILE_NAME,
        parent_dir_name: &parent,
        content: &content,
        scope: None,
    })
}

fn codes(outcome: &StrictSkillOutcome) -> Vec<SkillDiagnosticCode> {
    outcome.diagnostics().iter().map(|d| d.code).collect()
}

fn assert_valid(rel: &str) {
    match validate_file(rel) {
        StrictSkillOutcome::Valid(_) => {}
        StrictSkillOutcome::Quarantined(row) => {
            panic!(
                "{rel} should be accepted, got {:?}",
                row.diagnostics
                    .iter()
                    .map(|d| d.stable_line())
                    .collect::<Vec<_>>()
            );
        }
    }
}

fn assert_code(rel: &str, expected: SkillDiagnosticCode) {
    let outcome = validate_file(rel);
    assert!(
        codes(&outcome).contains(&expected),
        "{rel} expected {expected:?}, got {:?}",
        codes(&outcome)
    );
}

#[test]
fn official_accepted_fixtures() {
    assert_valid("official-accepted/my-skill/SKILL.md");
    assert_valid("official-accepted/pdf-processing/SKILL.md");
    assert_valid("official-accepted/skill-with-compat/SKILL.md");
    assert_valid("official-accepted/scalar-lexemes/SKILL.md");
    assert_valid("official-accepted/null/SKILL.md");
    match validate_file("official-accepted/pdf-processing/SKILL.md") {
        StrictSkillOutcome::Valid(skill) => {
            assert_eq!(
                skill.manifest.metadata.get("version").map(String::as_str),
                Some("1.0")
            );
        }
        StrictSkillOutcome::Quarantined(row) => panic!("{:?}", row.diagnostics),
    }
    match validate_file("official-accepted/scalar-lexemes/SKILL.md") {
        StrictSkillOutcome::Valid(skill) => {
            assert_eq!(skill.manifest.compatibility.as_deref(), Some("3.10"));
            assert_eq!(
                skill.manifest.metadata.get("version").map(String::as_str),
                Some("1.10")
            );
            assert_eq!(
                skill.manifest.metadata.get("quoted").map(String::as_str),
                Some("1.0")
            );
        }
        StrictSkillOutcome::Quarantined(row) => panic!("{:?}", row.diagnostics),
    }
    match validate_file("official-accepted/null/SKILL.md") {
        StrictSkillOutcome::Valid(skill) => {
            assert_eq!(skill.manifest.name, "null");
            assert_eq!(skill.manifest.license, None);
            assert_eq!(
                skill.manifest.metadata.get("version").map(String::as_str),
                Some("null")
            );
        }
        StrictSkillOutcome::Quarantined(row) => panic!("{:?}", row.diagnostics),
    }
}

#[test]
fn official_rejected_fixtures() {
    assert_code(
        "official-rejected/missing-name/SKILL.md",
        SkillDiagnosticCode::MissingName,
    );
    assert_code(
        "official-rejected/missing-description/SKILL.md",
        SkillDiagnosticCode::MissingDescription,
    );
    assert_code(
        "official-rejected/empty-name/SKILL.md",
        SkillDiagnosticCode::EmptyName,
    );
    assert_code(
        "official-rejected/empty-description/SKILL.md",
        SkillDiagnosticCode::EmptyDescription,
    );
    assert_code(
        "official-rejected/unexpected-fields/SKILL.md",
        SkillDiagnosticCode::UnexpectedTopLevelKey,
    );
    assert_code(
        "official-rejected/no-frontmatter/SKILL.md",
        SkillDiagnosticCode::MissingFrontmatter,
    );
    assert_code(
        "official-rejected/unclosed-frontmatter/SKILL.md",
        SkillDiagnosticCode::UnclosedFrontmatter,
    );
    assert_code(
        "official-rejected/invalid-yaml/SKILL.md",
        SkillDiagnosticCode::InvalidYaml,
    );
    assert_code(
        "official-rejected/non-mapping/SKILL.md",
        SkillDiagnosticCode::FrontmatterNotMapping,
    );
    assert_code(
        "official-rejected/dir-mismatch/SKILL.md",
        SkillDiagnosticCode::NameDirectoryMismatch,
    );
    assert_code(
        "official-rejected/true/SKILL.md",
        SkillDiagnosticCode::NameNotLowercase,
    );
}

#[test]
fn official_rejected_name_grammar_cases() {
    let cases: &[(&str, &str, SkillDiagnosticCode)] = &[
        (
            "MySkill",
            "---\nname: MySkill\ndescription: A test skill\n---\nBody\n",
            SkillDiagnosticCode::NameNotLowercase,
        ),
        (
            "true",
            "---\nname: True\ndescription: A test skill\n---\nBody\n",
            SkillDiagnosticCode::NameNotLowercase,
        ),
        (
            "my--skill",
            "---\nname: my--skill\ndescription: A test skill\n---\nBody\n",
            SkillDiagnosticCode::NameConsecutiveHyphens,
        ),
        (
            "-my-skill",
            "---\nname: -my-skill\ndescription: A test skill\n---\nBody\n",
            SkillDiagnosticCode::NameLeadingOrTrailingHyphen,
        ),
        (
            "my_skill",
            "---\nname: my_skill\ndescription: A test skill\n---\nBody\n",
            SkillDiagnosticCode::NameInvalidCharacters,
        ),
        (
            "навык",
            "---\nname: навык\ndescription: A skill with Russian lowercase name used in official fixtures.\n---\nBody\n",
            // accepted; this slot is only the valid control
            SkillDiagnosticCode::MissingName,
        ),
    ];
    let (parent, content, _) = cases.last().expect("control case");
    match validate_strict_skill(StrictSkillInput {
        file_name: SKILL_MD_FILE_NAME,
        parent_dir_name: parent,
        content,
        scope: None,
    }) {
        StrictSkillOutcome::Valid(skill) => assert_eq!(skill.manifest.name, "навык"),
        StrictSkillOutcome::Quarantined(row) => panic!("{:?}", row.diagnostics),
    }
    for (parent, content, expected) in &cases[..cases.len() - 1] {
        let outcome = validate_strict_skill(StrictSkillInput {
            file_name: SKILL_MD_FILE_NAME,
            parent_dir_name: parent,
            content,
            scope: None,
        });
        assert!(
            codes(&outcome).contains(expected),
            "{parent} expected {expected:?} got {:?}",
            codes(&outcome)
        );
    }
}

#[test]
fn grok_extension_fixtures() {
    assert_valid("grok-accepted/commit/SKILL.md");
    assert_code(
        "grok-rejected/bad-bool/SKILL.md",
        SkillDiagnosticCode::GrokExtensionInvalidValue,
    );
    assert_code(
        "grok-rejected/non-string-metadata/SKILL.md",
        SkillDiagnosticCode::MetadataValueNotString,
    );
}

#[test]
fn privacy_fixture_does_not_leak_secret() {
    let outcome = validate_file("privacy/leaky/SKILL.md");
    let rendered = outcome
        .diagnostics()
        .iter()
        .map(|d| format!("{}|{}|{}", d.stable_line(), d.message, d.remediation))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!rendered.contains("sk-live-SUPERSECRETVALUE"));
    assert!(!rendered.contains("SUPERSECRET"));
    assert!(!rendered.contains(env!("CARGO_MANIFEST_DIR")));
}

#[test]
fn dir_validator_matches_fixture_files() {
    let accepted = testdata_root().join("official-accepted/my-skill");
    match validate_strict_skill_dir(&accepted, None) {
        StrictSkillOutcome::Valid(skill) => {
            assert_eq!(skill.identity.file_label, "my-skill/SKILL.md")
        }
        StrictSkillOutcome::Quarantined(row) => panic!("{:?}", row.diagnostics),
    }
    let rejected = testdata_root().join("official-rejected/missing-name");
    match validate_strict_skill_dir(&rejected, None) {
        StrictSkillOutcome::Quarantined(row) => {
            assert!(
                row.diagnostics
                    .iter()
                    .any(|d| d.code == SkillDiagnosticCode::MissingName)
            );
        }
        StrictSkillOutcome::Valid(_) => panic!("expected quarantine"),
    }
}

#[test]
fn testdata_root_contains_official_and_grok_trees() {
    let root = testdata_root();
    assert!(root.join("official-accepted").is_dir());
    assert!(root.join("official-rejected").is_dir());
    assert!(root.join("grok-accepted").is_dir());
    assert!(root.join("grok-rejected").is_dir());
    assert!(Path::new(&root).join("privacy/leaky/SKILL.md").is_file());
}
