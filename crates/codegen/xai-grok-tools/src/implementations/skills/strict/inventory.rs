//! Shared discovered-skill, quarantined-skill, and inventory contracts.

use serde::{Deserialize, Serialize};

use super::diagnostic::{SkillAuthoringWarning, SkillDiagnostic};
use super::manifest::StrictSkillManifest;
use super::spec::AGENTSKILLS_SPEC_REVISION;
use crate::implementations::skills::types::SkillScope;
use crate::util::hash::fnv1a_32;

/// Safe identity for a skill directory. Never stores an absolute path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillIdentity {
    pub parent_dir_name: String,
    /// Always `{parent_dir_name}/SKILL.md`.
    pub file_label: String,
    pub scope: Option<SkillScope>,
}

impl SkillIdentity {
    pub fn new(parent_dir_name: impl Into<String>, scope: Option<SkillScope>) -> Self {
        let parent_dir_name = sanitize_parent_dir_name(&parent_dir_name.into());
        let file_label = format!("{parent_dir_name}/SKILL.md");
        Self {
            parent_dir_name,
            file_label,
            scope,
        }
    }
}

/// A skill that passed strict validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredSkill {
    pub identity: SkillIdentity,
    pub manifest: StrictSkillManifest,
    pub warnings: Vec<SkillAuthoringWarning>,
}

/// A skill that failed strict validation. Invalid rows must not be
/// advertised, invoked, preloaded, or primed (enforced in PR2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuarantinedSkill {
    pub identity: SkillIdentity,
    pub diagnostics: Vec<SkillDiagnostic>,
}

/// Complete valid + quarantined inventory for one generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillInventory {
    pub spec_revision: String,
    pub generation: u64,
    pub valid: Vec<DiscoveredSkill>,
    pub quarantined: Vec<QuarantinedSkill>,
}

impl Default for SkillInventory {
    fn default() -> Self {
        Self::new(0, Vec::new(), Vec::new())
    }
}

impl SkillInventory {
    pub fn new(
        generation: u64,
        mut valid: Vec<DiscoveredSkill>,
        mut quarantined: Vec<QuarantinedSkill>,
    ) -> Self {
        valid.sort_by(|a, b| {
            a.identity
                .parent_dir_name
                .cmp(&b.identity.parent_dir_name)
                .then_with(|| a.manifest.name.cmp(&b.manifest.name))
        });
        quarantined.sort_by(|a, b| {
            a.identity
                .parent_dir_name
                .cmp(&b.identity.parent_dir_name)
                .then_with(|| {
                    a.diagnostics
                        .first()
                        .map(|d| d.code)
                        .cmp(&b.diagnostics.first().map(|d| d.code))
                })
        });
        Self {
            spec_revision: AGENTSKILLS_SPEC_REVISION.to_string(),
            generation,
            valid,
            quarantined,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.valid.is_empty() && self.quarantined.is_empty()
    }

    /// Canonical non-secret fingerprint. Changes when any valid manifest or
    /// quarantine code set changes. Bodies, paths, and raw values are absent.
    pub fn fingerprint(&self) -> String {
        let mut buf = String::new();
        buf.push_str(&self.spec_revision);
        buf.push('\n');
        buf.push_str(&self.generation.to_string());
        buf.push('\n');
        for skill in &self.valid {
            buf.push_str("valid\t");
            buf.push_str(&skill.identity.parent_dir_name);
            buf.push('\t');
            buf.push_str(&skill.manifest.name);
            buf.push('\t');
            buf.push_str(&skill.manifest.description);
            buf.push('\t');
            if let Some(license) = &skill.manifest.license {
                buf.push_str(license);
            }
            buf.push('\t');
            if let Some(compat) = &skill.manifest.compatibility {
                buf.push_str(compat);
            }
            buf.push('\t');
            if let Some(tools) = &skill.manifest.allowed_tools {
                buf.push_str(tools);
            }
            buf.push('\t');
            for (k, v) in &skill.manifest.metadata {
                buf.push_str(k);
                buf.push('=');
                buf.push_str(v);
                buf.push(';');
            }
            buf.push('\t');
            append_grok_fingerprint(&mut buf, &skill.manifest.grok);
            buf.push('\n');
        }
        for skill in &self.quarantined {
            buf.push_str("quarantined\t");
            buf.push_str(&skill.identity.parent_dir_name);
            buf.push('\t');
            for diagnostic in &skill.diagnostics {
                buf.push_str(diagnostic.code.as_str());
                buf.push(',');
            }
            buf.push('\n');
        }
        format!("{:08x}", fnv1a_32(buf.as_bytes()))
    }

    /// True when either the generation or the fingerprint disagrees.
    pub fn is_stale(&self, expected_generation: u64, expected_fingerprint: &str) -> bool {
        self.generation != expected_generation || self.fingerprint() != expected_fingerprint
    }

    /// Merge incremental valid/quarantined rows by identity.
    ///
    /// A one-file generation-0 report never wipes a fuller inventory. Incoming
    /// valid rows displace quarantined rows of the same identity, and the
    /// reverse. Unmentioned identities are kept. Generation is the max of both
    /// sides, bumped when membership or content changes under a gen-0 ingest.
    pub fn merge_incremental(&self, incoming: Self) -> Self {
        use std::collections::HashMap;

        fn key(
            identity: &SkillIdentity,
        ) -> (
            String,
            Option<crate::implementations::skills::types::SkillScope>,
        ) {
            (identity.parent_dir_name.clone(), identity.scope)
        }

        let mut valid: HashMap<_, _> = self
            .valid
            .iter()
            .cloned()
            .map(|row| (key(&row.identity), row))
            .collect();
        let mut quarantined: HashMap<_, _> = self
            .quarantined
            .iter()
            .cloned()
            .map(|row| (key(&row.identity), row))
            .collect();

        for row in incoming.valid {
            let k = key(&row.identity);
            quarantined.remove(&k);
            valid.insert(k, row);
        }
        for row in incoming.quarantined {
            let k = key(&row.identity);
            valid.remove(&k);
            quarantined.insert(k, row);
        }

        let next = SkillInventory::new(
            0,
            valid.into_values().collect(),
            quarantined.into_values().collect(),
        );
        let content_changed = next.valid != self.valid || next.quarantined != self.quarantined;
        let generation = if incoming.generation > self.generation {
            incoming.generation
        } else if content_changed {
            self.generation.saturating_add(1).max(1)
        } else {
            self.generation
        };
        SkillInventory::new(generation, next.valid, next.quarantined)
    }
}

fn append_grok_fingerprint(buf: &mut String, grok: &super::manifest::GrokSkillExtensions) {
    if let Some(v) = &grok.when_to_use {
        buf.push_str("when-to-use=");
        buf.push_str(v);
        buf.push(';');
    }
    if let Some(paths) = &grok.paths {
        buf.push_str("paths=");
        buf.push_str(&paths.join(","));
        buf.push(';');
    }
    if let Some(v) = &grok.argument_hint {
        buf.push_str("argument-hint=");
        buf.push_str(v);
        buf.push(';');
    }
    if let Some(v) = &grok.model {
        buf.push_str("model=");
        buf.push_str(v);
        buf.push(';');
    }
    if let Some(v) = &grok.effort {
        buf.push_str("effort=");
        buf.push_str(v);
        buf.push(';');
    }
    if let Some(v) = grok.user_invocable {
        buf.push_str("user-invocable=");
        buf.push_str(if v { "true" } else { "false" });
        buf.push(';');
    }
    if let Some(v) = grok.disable_model_invocation {
        buf.push_str("disable-model-invocation=");
        buf.push_str(if v { "true" } else { "false" });
        buf.push(';');
    }
    if let Some(v) = &grok.short_description {
        buf.push_str("short-description=");
        buf.push_str(v);
        buf.push(';');
    }
}

pub(crate) fn sanitize_parent_dir_name(raw: &str) -> String {
    let trimmed = raw.trim_end_matches(['/', '\\']);
    std::path::Path::new(trimmed)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty() && *name != "." && *name != "..")
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::implementations::skills::strict::diagnostic::{
        DiagnosticPosition, SkillDiagnostic, SkillDiagnosticCode,
    };
    use crate::implementations::skills::strict::manifest::{
        GrokSkillExtensions, StrictSkillManifest,
    };

    fn valid(name: &str, description: &str) -> DiscoveredSkill {
        DiscoveredSkill {
            identity: SkillIdentity::new(name, None),
            manifest: StrictSkillManifest {
                name: name.to_string(),
                description: description.to_string(),
                license: None,
                compatibility: None,
                allowed_tools: None,
                metadata: Default::default(),
                grok: GrokSkillExtensions::default(),
            },
            warnings: Vec::new(),
        }
    }

    #[test]
    fn identity_strips_absolute_path_components() {
        let id = SkillIdentity::new("/home/user/.grok/skills/commit", None);
        assert_eq!(id.parent_dir_name, "commit");
        assert_eq!(id.file_label, "commit/SKILL.md");
    }

    #[test]
    fn fingerprint_changes_when_manifest_changes() {
        let first = SkillInventory::new(1, vec![valid("commit", "Create commits")], vec![]);
        let second = SkillInventory::new(1, vec![valid("commit", "Create better commits")], vec![]);
        assert_ne!(first.fingerprint(), second.fingerprint());
        assert!(first.is_stale(1, &second.fingerprint()));
        assert!(!first.is_stale(1, &first.fingerprint()));
    }

    #[test]
    fn fingerprint_changes_when_generation_changes() {
        let first = SkillInventory::new(1, vec![valid("commit", "Create commits")], vec![]);
        let second = SkillInventory::new(2, vec![valid("commit", "Create commits")], vec![]);
        assert_ne!(first.fingerprint(), second.fingerprint());
        assert!(first.is_stale(2, &first.fingerprint()));
    }

    #[test]
    fn quarantined_fingerprint_uses_codes_not_values() {
        let quarantined = QuarantinedSkill {
            identity: SkillIdentity::new("bad", None),
            diagnostics: vec![SkillDiagnostic::new(
                SkillDiagnosticCode::MissingName,
                Some("name"),
                "Required field 'name' is missing.",
                "Add a nonempty name that matches the parent directory.",
                DiagnosticPosition::FILE_START,
            )],
        };
        let inventory = SkillInventory::new(0, vec![], vec![quarantined]);
        assert!(!inventory.fingerprint().contains("password"));
        assert!(!inventory.fingerprint().contains('/'));
    }

    fn quarantined_row(name: &str, code: SkillDiagnosticCode) -> QuarantinedSkill {
        QuarantinedSkill {
            identity: SkillIdentity::new(name, None),
            diagnostics: vec![SkillDiagnostic::new(
                code,
                Some("name"),
                "quarantined",
                "repair",
                DiagnosticPosition::FILE_START,
            )],
        }
    }

    #[test]
    fn merge_incremental_keeps_unmentioned_quarantined_rows() {
        let first = SkillInventory::new(
            1,
            vec![],
            vec![quarantined_row("alpha", SkillDiagnosticCode::MissingName)],
        );
        let second = SkillInventory::new(
            0,
            vec![],
            vec![quarantined_row("beta", SkillDiagnosticCode::MissingName)],
        );
        let merged = first.merge_incremental(second);
        assert_eq!(merged.valid.len(), 0);
        assert_eq!(merged.quarantined.len(), 2);
        assert!(merged.generation >= 1);
        let names: Vec<_> = merged
            .quarantined
            .iter()
            .map(|row| row.identity.parent_dir_name.as_str())
            .collect();
        assert!(names.contains(&"alpha"));
        assert!(names.contains(&"beta"));
    }

    #[test]
    fn merge_incremental_moves_valid_identity_to_quarantine() {
        let first = SkillInventory::new(1, vec![valid("commit", "Create commits")], vec![]);
        let second = SkillInventory::new(
            0,
            vec![],
            vec![quarantined_row(
                "commit",
                SkillDiagnosticCode::UnexpectedTopLevelKey,
            )],
        );
        let merged = first.merge_incremental(second);
        assert!(merged.valid.is_empty());
        assert_eq!(merged.quarantined.len(), 1);
        assert_eq!(merged.quarantined[0].identity.parent_dir_name, "commit");
    }
}
