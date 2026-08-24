//! Bundled and plugin author checks. No silent repair, no network.

use std::path::Path;
use std::sync::atomic::AtomicBool;

use serde::{Deserialize, Serialize};

use super::diagnostic::SkillDiagnostic;
use super::evals::{
    EvalSchemaError, EvalSuite, LocalSkillEvidence, load_eval_suite_from_dir, run_eval_suite,
};
use super::inventory::SkillIdentity;
use super::spec::{LEGACY_GROK_TOP_LEVEL_KEYS, SKILL_MD_FILE_NAME};
use super::status::SkillHealthStatus;
use super::validator::{StrictSkillOutcome, validate_strict_skill, validate_strict_skill_dir};
use crate::implementations::skills::types::SkillScope;

/// Publication surface the author check is gating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthorKind {
    Bundled,
    Plugin,
}

impl AuthorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bundled => "bundled",
            Self::Plugin => "plugin",
        }
    }

    fn scope(self) -> SkillScope {
        match self {
            Self::Bundled => SkillScope::Bundled,
            Self::Plugin => SkillScope::Plugin,
        }
    }
}

/// Secret-free author report. Diagnostics never include bodies, paths, or
/// credential-like values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorCheckReport {
    pub kind: AuthorKind,
    pub identity: SkillIdentity,
    pub status: SkillHealthStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<SkillDiagnostic>,
    pub evals_present: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evals_status: Option<SkillHealthStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evals_error: Option<String>,
}

impl AuthorCheckReport {
    pub fn is_publishable(&self) -> bool {
        !matches!(self.status, SkillHealthStatus::Quarantined)
            && self.evals_error.is_none()
            && !matches!(self.evals_status, Some(SkillHealthStatus::Failed))
    }
}

/// Validate a complete skill directory for bundled or plugin publication.
pub fn check_author_skill_dir(kind: AuthorKind, dir: &Path) -> AuthorCheckReport {
    let outcome = validate_strict_skill_dir(dir, Some(kind.scope()));
    finish_report(kind, outcome, load_eval_suite_from_dir(dir))
}

/// Validate in-memory SKILL.md content. `parent_dir_name` is the skill id.
pub fn check_author_skill_content(
    kind: AuthorKind,
    parent_dir_name: &str,
    content: &str,
    evals: Option<&[u8]>,
) -> AuthorCheckReport {
    let outcome = validate_strict_skill(super::validator::StrictSkillInput {
        file_name: SKILL_MD_FILE_NAME,
        parent_dir_name,
        content,
        scope: Some(kind.scope()),
    });
    let evals = match evals {
        Some(bytes) => super::evals::parse_eval_suite(bytes).map(Some),
        None => Ok(None),
    };
    finish_report(kind, outcome, evals)
}

fn finish_report(
    kind: AuthorKind,
    outcome: StrictSkillOutcome,
    evals: Result<Option<EvalSuite>, EvalSchemaError>,
) -> AuthorCheckReport {
    match outcome {
        StrictSkillOutcome::Quarantined(row) => AuthorCheckReport {
            kind,
            identity: row.identity,
            status: SkillHealthStatus::Quarantined,
            diagnostics: row.diagnostics,
            evals_present: evals.ok().flatten().is_some(),
            evals_status: None,
            evals_error: None,
        },
        StrictSkillOutcome::Valid(discovered) => {
            let (evals_present, evals_status, evals_error) = match evals {
                Ok(None) => (false, None, None),
                Err(err) => (true, None, Some(err.message)),
                Ok(Some(suite)) => {
                    let subject = LocalSkillEvidence {
                        name: discovered.manifest.name.clone(),
                        description: discovered.manifest.description.clone(),
                        when_to_use: discovered.manifest.grok.when_to_use.clone(),
                        paths: discovered.manifest.grok.paths.clone().unwrap_or_default(),
                        short_description: discovered.manifest.grok.short_description.clone(),
                    };
                    let report = run_eval_suite(
                        &suite,
                        &subject,
                        &[],
                        discovered.identity.clone(),
                        0,
                        "author",
                        &AtomicBool::new(false),
                    );
                    (true, Some(report.status), None)
                }
            };
            let status = evals_status.unwrap_or(SkillHealthStatus::Untested);
            AuthorCheckReport {
                kind,
                identity: discovered.identity,
                status,
                diagnostics: Vec::new(),
                evals_present,
                evals_status,
                evals_error,
            }
        }
    }
}

/// True when SKILL.md bytes still contain a rejected top-level Grok key.
/// Used by release gates; does not repair the file.
pub fn content_has_legacy_top_level_grok_key(content: &str) -> bool {
    let Some(rest) = content.strip_prefix("---") else {
        return false;
    };
    let Some(end) = rest.find("\n---") else {
        return false;
    };
    let front = &rest[..end];
    for line in front.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            continue;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let key = trimmed.split(':').next().unwrap_or("").trim();
        if LEGACY_GROK_TOP_LEVEL_KEYS.contains(&key) {
            return true;
        }
    }
    false
}

/// Secret-free leak scan for diagnostics, JSON, and rendered text.
pub fn secret_leak_tokens() -> &'static [&'static str] {
    &[
        "sk-",
        "sk_live",
        "BEGIN PRIVATE",
        "/Users/",
        "/home/",
        "file://",
        "http://",
        "https://",
        "Authorization",
        "Bearer ",
        "api_key",
        "0.39215687",
        "YOU ARE A HELPFUL ASSISTANT",
        "SECRET-BODY",
    ]
}

pub fn text_leaks_secrets(text: &str) -> Option<&'static str> {
    let lower = text.to_ascii_lowercase();
    for token in secret_leak_tokens() {
        if lower.contains(&token.to_ascii_lowercase()) {
            return Some(*token);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::implementations::skills::strict::render_skill_md;

    fn testdata() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src/implementations/skills/strict/testdata")
    }

    #[test]
    fn bundled_author_fixture_is_publishable_with_passing_evals() {
        let dir = testdata().join("author/bundled/commit");
        let report = check_author_skill_dir(AuthorKind::Bundled, &dir);
        assert_eq!(report.kind, AuthorKind::Bundled);
        assert!(report.evals_present);
        assert_eq!(report.evals_status, Some(SkillHealthStatus::ValidPass));
        assert_eq!(report.status, SkillHealthStatus::ValidPass);
        assert!(report.is_publishable());
        assert!(report.diagnostics.is_empty());
        let json = serde_json::to_string(&report).unwrap();
        assert!(text_leaks_secrets(&json).is_none(), "leaked in {json}");
    }

    #[test]
    fn plugin_author_fixture_is_publishable() {
        let dir = testdata().join("author/plugin/review");
        let report = check_author_skill_dir(AuthorKind::Plugin, &dir);
        assert_eq!(report.kind, AuthorKind::Plugin);
        assert_eq!(report.status, SkillHealthStatus::ValidPass);
        assert!(report.is_publishable());
    }

    #[test]
    fn plugin_legacy_top_level_key_is_quarantined_without_repair() {
        let dir = testdata().join("author/plugin/quarantined");
        let report = check_author_skill_dir(AuthorKind::Plugin, &dir);
        assert_eq!(report.status, SkillHealthStatus::Quarantined);
        assert!(!report.is_publishable());
        let content = std::fs::read_to_string(dir.join("SKILL.md")).unwrap();
        assert!(content_has_legacy_top_level_grok_key(&content));
        // The original file is untouched: still contains the rejected key.
        assert!(content.contains("when-to-use:"));
        let json = serde_json::to_string(&report).unwrap();
        assert!(
            !json.contains("when-to-use: review a pull request"),
            "diagnostics must not echo the raw rejected value"
        );
        assert!(text_leaks_secrets(&json).is_none(), "leaked in {json}");
    }

    #[test]
    fn invalid_evals_fail_the_author_gate_without_quarantine_bypass() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("commit");
        std::fs::create_dir_all(dir.join("evals")).unwrap();
        std::fs::write(
            dir.join("SKILL.md"),
            render_skill_md("commit", "Create well-formatted git commits.", "# Body\n"),
        )
        .unwrap();
        std::fs::write(dir.join("evals/cases.yaml"), "version: 9\ncases: []\n").unwrap();
        let report = check_author_skill_dir(AuthorKind::Bundled, &dir);
        assert_ne!(report.status, SkillHealthStatus::Quarantined);
        assert!(report.evals_error.is_some());
        assert!(!report.is_publishable());
    }

    #[test]
    fn content_check_does_not_infer_name_from_directory() {
        let content =
            "---\nname: other\ndescription: Create well-formatted git commits.\n---\n\n# Other\n";
        let report = check_author_skill_content(AuthorKind::Bundled, "commit", content, None);
        assert_eq!(report.status, SkillHealthStatus::Quarantined);
        assert!(!report.is_publishable());
    }
}
