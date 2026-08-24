//! Stable, bounded, secret-free skill diagnostics and authoring warnings.

use serde::{Deserialize, Serialize};
use strum::{AsRefStr, IntoStaticStr};

use crate::util::truncate::truncate_line;

/// Maximum diagnostics retained for one skill.
pub const MAX_DIAGNOSTICS: usize = 24;

/// Maximum Unicode code points in a diagnostic or warning message.
pub const MAX_MESSAGE_CHARS: usize = 200;

/// Maximum Unicode code points in a remediation string.
pub const MAX_REMEDIATION_CHARS: usize = 240;

/// Maximum Unicode code points in a reported field name.
pub const MAX_FIELD_CHARS: usize = 64;

/// 1-based line/column pointing at a frontmatter key, never at a raw value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DiagnosticPosition {
    pub line: u32,
    pub column: u32,
}

impl DiagnosticPosition {
    pub const FILE_START: Self = Self { line: 1, column: 1 };

    pub fn new(line: u32, column: u32) -> Self {
        Self {
            line: line.max(1),
            column: column.max(1),
        }
    }
}

/// Quarantine error codes. Serialized as stable kebab-case identifiers.
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
    PartialOrd,
    Ord,
)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum SkillDiagnosticCode {
    NotADirectory,
    MissingSkillMd,
    WrongSkillFileName,
    NotRegularFile,
    UnreadableSkillMd,
    MissingFrontmatter,
    UnclosedFrontmatter,
    FrontmatterTooLarge,
    InvalidYaml,
    FrontmatterNotMapping,
    DuplicateTopLevelKey,
    TopLevelKeyNotString,
    UnexpectedTopLevelKey,
    MissingName,
    MissingDescription,
    NameNotString,
    DescriptionNotString,
    EmptyName,
    EmptyDescription,
    NameTooLong,
    NameNotLowercase,
    NameLeadingOrTrailingHyphen,
    NameConsecutiveHyphens,
    NameInvalidCharacters,
    NameDirectoryMismatch,
    DescriptionTooLong,
    LicenseNotString,
    CompatibilityNotString,
    CompatibilityTooLong,
    MetadataNotMapping,
    MetadataKeyNotString,
    MetadataValueNotString,
    AllowedToolsNotString,
    GrokExtensionNotMapping,
    GrokExtensionUnknownKey,
    GrokExtensionInvalidValue,
    GrokExtensionConflict,
    TooManyIssues,
}

impl SkillDiagnosticCode {
    pub fn as_str(self) -> &'static str {
        self.into()
    }
}

/// Advisory authoring warning. Never quarantines a skill.
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
    PartialOrd,
    Ord,
)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum SkillWarningCode {
    ShortDescription,
    EmptyBody,
    LongBody,
    MissingLicense,
    MissingGrokWhenToUse,
    EmptyAllowedTools,
    EmptyCompatibility,
}

impl SkillWarningCode {
    pub fn as_str(self) -> &'static str {
        self.into()
    }
}

/// One quarantine diagnostic. Messages never include raw field values,
/// absolute paths, YAML parser text, or file bodies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillDiagnostic {
    pub code: SkillDiagnosticCode,
    pub field: Option<String>,
    pub message: String,
    pub remediation: String,
    pub position: DiagnosticPosition,
}

impl SkillDiagnostic {
    pub fn new(
        code: SkillDiagnosticCode,
        field: Option<&str>,
        message: impl Into<String>,
        remediation: impl Into<String>,
        position: DiagnosticPosition,
    ) -> Self {
        Self {
            code,
            field: field.map(bound_field),
            message: bound_text(&message.into(), MAX_MESSAGE_CHARS),
            remediation: bound_text(&remediation.into(), MAX_REMEDIATION_CHARS),
            position,
        }
    }

    /// Stable machine-readable line: `code:line:column: message`.
    pub fn stable_line(&self) -> String {
        match &self.field {
            Some(field) => format!(
                "{}:{}:{}:{}: {}",
                self.code.as_str(),
                field,
                self.position.line,
                self.position.column,
                self.message
            ),
            None => format!(
                "{}:{}:{}: {}",
                self.code.as_str(),
                self.position.line,
                self.position.column,
                self.message
            ),
        }
    }
}

/// Advisory warning emitted alongside a valid manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillAuthoringWarning {
    pub code: SkillWarningCode,
    pub field: Option<String>,
    pub message: String,
    pub remediation: String,
    pub position: DiagnosticPosition,
}

impl SkillAuthoringWarning {
    pub fn new(
        code: SkillWarningCode,
        field: Option<&str>,
        message: impl Into<String>,
        remediation: impl Into<String>,
        position: DiagnosticPosition,
    ) -> Self {
        Self {
            code,
            field: field.map(bound_field),
            message: bound_text(&message.into(), MAX_MESSAGE_CHARS),
            remediation: bound_text(&remediation.into(), MAX_REMEDIATION_CHARS),
            position,
        }
    }
}

pub fn bound_text(s: &str, max_chars: usize) -> String {
    truncate_line(s, max_chars).into_owned()
}

pub fn bound_field(s: &str) -> String {
    bound_text(s, MAX_FIELD_CHARS)
}

pub fn cap_diagnostics(mut diagnostics: Vec<SkillDiagnostic>) -> Vec<SkillDiagnostic> {
    if diagnostics.len() <= MAX_DIAGNOSTICS {
        return diagnostics;
    }
    diagnostics.truncate(MAX_DIAGNOSTICS.saturating_sub(1));
    diagnostics.push(SkillDiagnostic::new(
        SkillDiagnosticCode::TooManyIssues,
        None,
        "Additional validation issues were omitted.",
        "Fix the reported issues and validate again.",
        DiagnosticPosition::FILE_START,
    ));
    diagnostics
}
