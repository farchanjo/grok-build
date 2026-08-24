//! Versioned shell-authoritative skill-management DTOs.
//!
//! Mixed-version ACP defaults fail safely: new operations require
//! [`SKILLS_API_VERSION`]. Legacy `x.ai/skills/list` without a version keeps
//! the historical `{ skills: [...] }` shape.

use serde::{Deserialize, Serialize};

use super::diagnostic::SkillDiagnostic;
use super::evals::EvalRunReport;
use super::inventory::{QuarantinedSkill, SkillIdentity, SkillInventory};
use super::status::{SkillHealthStatus, SkillsHealthHeader};
use crate::implementations::skills::types::SkillInfo;

/// Current skills ACP/JSON contract version.
pub const SKILLS_API_VERSION: u32 = 1;

/// Require `apiVersion == 1`. Missing or mismatched versions fail closed.
pub fn require_api_version(api_version: Option<u32>) -> Result<u32, SkillsVersionError> {
    match api_version {
        Some(SKILLS_API_VERSION) => Ok(SKILLS_API_VERSION),
        Some(found) => Err(SkillsVersionError::Unsupported { found }),
        None => Err(SkillsVersionError::Missing),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillsVersionError {
    Missing,
    Unsupported { found: u32 },
}

impl SkillsVersionError {
    pub fn message(self) -> &'static str {
        match self {
            Self::Missing => "apiVersion is required for this skills operation.",
            Self::Unsupported { .. } => "Unsupported skills apiVersion.",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsVersionedRequest {
    #[serde(default)]
    pub api_version: Option<u32>,
}

/// Compact regression summary. No queries, bodies, or paths.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillRegressionSummary {
    pub status: SkillHealthStatus,
    pub generation: u64,
    pub cases_fingerprint: String,
    pub passed: u32,
    pub failed: u32,
    pub cancelled: bool,
    pub stable: bool,
}

impl SkillRegressionSummary {
    pub fn from_report(report: &EvalRunReport) -> Self {
        let passed = report.results.values().filter(|r| r.passed).count() as u32;
        let failed = report.results.values().filter(|r| !r.passed).count() as u32;
        Self {
            status: report.status,
            generation: report.generation,
            cases_fingerprint: report.cases_fingerprint.clone(),
            passed,
            failed,
            cancelled: report.cancelled,
            stable: report.stable,
        }
    }
}

/// One list row. Quarantined rows set `enableable = false`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedSkillRow {
    pub identity: SkillIdentity,
    pub status: SkillHealthStatus,
    pub enableable: bool,
    pub skill: Option<SkillInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<SkillDiagnostic>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regression: Option<SkillRegressionSummary>,
}

impl ManagedSkillRow {
    pub fn from_valid(
        skill: SkillInfo,
        identity: SkillIdentity,
        mut status: SkillHealthStatus,
        regression: Option<SkillRegressionSummary>,
    ) -> Self {
        if let Some(summary) = &regression {
            status = summary.status;
        }
        Self {
            identity,
            enableable: status.enableable(),
            status,
            skill: Some(skill),
            diagnostics: Vec::new(),
            regression,
        }
    }

    pub fn from_quarantined(row: &QuarantinedSkill) -> Self {
        Self {
            identity: row.identity.clone(),
            status: SkillHealthStatus::Quarantined,
            enableable: false,
            skill: None,
            diagnostics: row.diagnostics.clone(),
            regression: None,
        }
    }

    pub fn display_name(&self) -> &str {
        self.skill
            .as_ref()
            .map(|s| s.label())
            .unwrap_or(self.identity.parent_dir_name.as_str())
    }
}

/// Versioned list payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsListV1Response {
    pub api_version: u32,
    pub generation: u64,
    pub fingerprint: String,
    pub health: SkillsHealthHeader,
    pub skills: Vec<ManagedSkillRow>,
}

impl SkillsListV1Response {
    pub fn from_rows(generation: u64, fingerprint: String, skills: Vec<ManagedSkillRow>) -> Self {
        let health = SkillsHealthHeader::tally(skills.iter().map(|row| row.status));
        Self {
            api_version: SKILLS_API_VERSION,
            generation,
            fingerprint,
            health,
            skills,
        }
    }
}

/// Build versioned rows from a live inventory plus enabled `SkillInfo` rows
/// and optional persisted regression summaries. Quarantined rows are visible
/// and never enableable.
pub fn build_managed_rows(
    inventory: &SkillInventory,
    skills: &[SkillInfo],
    regressions: &dyn Fn(&SkillIdentity) -> Option<SkillRegressionSummary>,
) -> Vec<ManagedSkillRow> {
    let mut rows = Vec::new();
    for skill in skills {
        let identity = SkillIdentity::new(&skill.name, Some(skill.scope));
        let summary = regressions(&identity);
        let status = summary
            .as_ref()
            .map(|s| s.status)
            .unwrap_or(SkillHealthStatus::Untested);
        rows.push(ManagedSkillRow::from_valid(
            skill.clone(),
            identity,
            status,
            summary,
        ));
    }
    for quarantined in &inventory.quarantined {
        rows.push(ManagedSkillRow::from_quarantined(quarantined));
    }
    rows.sort_by(|a, b| {
        a.identity
            .parent_dir_name
            .cmp(&b.identity.parent_dir_name)
            .then_with(|| a.status.as_str().cmp(b.status.as_str()))
    });
    rows
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsValidateResponse {
    pub api_version: u32,
    pub generation: u64,
    pub status: SkillHealthStatus,
    pub identity: SkillIdentity,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<SkillDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsPublishResponse {
    pub api_version: u32,
    pub generation: u64,
    pub identity: SkillIdentity,
    pub created: bool,
    pub status: SkillHealthStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsRegressStatusResponse {
    pub api_version: u32,
    pub generation: u64,
    pub running: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<SkillRegressionSummary>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::implementations::skills::strict::diagnostic::{
        DiagnosticPosition, SkillDiagnostic, SkillDiagnosticCode,
    };
    use crate::implementations::skills::types::SkillScope;

    #[test]
    fn mixed_version_defaults_fail_closed() {
        assert!(matches!(
            require_api_version(None),
            Err(SkillsVersionError::Missing)
        ));
        assert!(matches!(
            require_api_version(Some(0)),
            Err(SkillsVersionError::Unsupported { found: 0 })
        ));
        assert_eq!(require_api_version(Some(1)).unwrap(), 1);
    }

    #[test]
    fn quarantined_rows_are_visible_and_not_enableable() {
        let quarantined = QuarantinedSkill {
            identity: SkillIdentity::new("bad", Some(SkillScope::Local)),
            diagnostics: vec![SkillDiagnostic::new(
                SkillDiagnosticCode::MissingName,
                Some("name"),
                "Field 'name' is required.",
                "Set name to the parent directory name.",
                DiagnosticPosition::FILE_START,
            )],
        };
        let row = ManagedSkillRow::from_quarantined(&quarantined);
        assert_eq!(row.status, SkillHealthStatus::Quarantined);
        assert!(!row.enableable);
        assert!(row.skill.is_none());
        let json = serde_json::to_string(&row).unwrap();
        assert!(!json.contains("/Users"));
        assert!(!json.contains("raw"));
    }

    #[test]
    fn list_v1_health_counts_quarantined() {
        let valid = SkillInfo {
            name: "commit".into(),
            description: "Create commits".into(),
            ..SkillInfo::default()
        };
        let inventory = SkillInventory::new(
            4,
            vec![],
            vec![QuarantinedSkill {
                identity: SkillIdentity::new("bad", None),
                diagnostics: vec![],
            }],
        );
        let rows = build_managed_rows(&inventory, &[valid], &|_| None);
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|r| r.status == SkillHealthStatus::Untested));
        assert!(
            rows.iter()
                .any(|r| r.status == SkillHealthStatus::Quarantined && !r.enableable)
        );
        let list = SkillsListV1Response::from_rows(4, inventory.fingerprint(), rows);
        assert_eq!(list.api_version, 1);
        assert_eq!(list.health.untested, 1);
        assert_eq!(list.health.quarantined, 1);
    }
}
