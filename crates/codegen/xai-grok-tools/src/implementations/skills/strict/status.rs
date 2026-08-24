//! Shared skill health vocabulary used by inventory, regression, TUI, and CLI.

use serde::{Deserialize, Serialize};
use strum::{AsRefStr, IntoStaticStr};

/// Canonical skill health status. Serialized as kebab-case wire tokens.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    AsRefStr,
    IntoStaticStr,
    Default,
)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum SkillHealthStatus {
    /// Strictly valid and last local regression passed.
    ValidPass,
    /// Strictly valid but last local regression failed.
    Failed,
    /// Failed strict validation; visible but never enableable.
    Quarantined,
    /// Prior regression results no longer match the current fingerprints.
    Stale,
    /// Strictly valid and never locally tested.
    #[default]
    Untested,
}

impl SkillHealthStatus {
    pub fn as_str(self) -> &'static str {
        self.into()
    }

    /// Quarantined rows must never be enabled, advertised, or invoked.
    pub fn enableable(self) -> bool {
        !matches!(self, Self::Quarantined)
    }
}

/// Compact counts for the `/skills` health header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsHealthHeader {
    pub valid_pass: u32,
    pub failed: u32,
    pub quarantined: u32,
    pub stale: u32,
    pub untested: u32,
}

impl SkillsHealthHeader {
    pub fn tally(statuses: impl IntoIterator<Item = SkillHealthStatus>) -> Self {
        let mut header = Self::default();
        for status in statuses {
            match status {
                SkillHealthStatus::ValidPass => header.valid_pass += 1,
                SkillHealthStatus::Failed => header.failed += 1,
                SkillHealthStatus::Quarantined => header.quarantined += 1,
                SkillHealthStatus::Stale => header.stale += 1,
                SkillHealthStatus::Untested => header.untested += 1,
            }
        }
        header
    }

    /// Compact one-line header. Truncate at the consumer for narrow widths.
    pub fn compact_line(self) -> String {
        format!(
            "valid-pass {} · failed {} · quarantined {} · stale {} · untested {}",
            self.valid_pass, self.failed, self.quarantined, self.stale, self.untested
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_tokens_are_canonical() {
        assert_eq!(
            serde_json::to_value(SkillHealthStatus::ValidPass).unwrap(),
            serde_json::json!("valid-pass")
        );
        assert_eq!(
            serde_json::to_value(SkillHealthStatus::Failed).unwrap(),
            serde_json::json!("failed")
        );
        assert_eq!(
            serde_json::to_value(SkillHealthStatus::Quarantined).unwrap(),
            serde_json::json!("quarantined")
        );
        assert_eq!(
            serde_json::to_value(SkillHealthStatus::Stale).unwrap(),
            serde_json::json!("stale")
        );
        assert_eq!(
            serde_json::to_value(SkillHealthStatus::Untested).unwrap(),
            serde_json::json!("untested")
        );
    }

    #[test]
    fn quarantined_is_never_enableable() {
        assert!(!SkillHealthStatus::Quarantined.enableable());
        assert!(SkillHealthStatus::ValidPass.enableable());
        assert!(SkillHealthStatus::Failed.enableable());
        assert!(SkillHealthStatus::Stale.enableable());
        assert!(SkillHealthStatus::Untested.enableable());
    }

    #[test]
    fn health_header_tallies_and_renders_compactly() {
        let header = SkillsHealthHeader::tally([
            SkillHealthStatus::ValidPass,
            SkillHealthStatus::ValidPass,
            SkillHealthStatus::Quarantined,
            SkillHealthStatus::Untested,
            SkillHealthStatus::Failed,
            SkillHealthStatus::Stale,
        ]);
        assert_eq!(header.valid_pass, 2);
        assert_eq!(header.failed, 1);
        assert_eq!(header.quarantined, 1);
        assert_eq!(header.stale, 1);
        assert_eq!(header.untested, 1);
        let line = header.compact_line();
        assert!(line.contains("valid-pass 2"));
        assert!(line.contains("quarantined 1"));
        assert!(!line.contains('/'), "no absolute paths in header");
    }
}
