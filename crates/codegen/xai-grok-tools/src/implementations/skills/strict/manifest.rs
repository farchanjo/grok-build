//! Strict skill manifest and namespaced Grok extension contracts.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Parsed Grok extensions from `metadata.grok` or `metadata.grok.*`.
///
/// Absent optional fields stay `None`. No defaults are inferred.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct GrokSkillExtensions {
    pub when_to_use: Option<String>,
    pub paths: Option<Vec<String>>,
    pub argument_hint: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub user_invocable: Option<bool>,
    pub disable_model_invocation: Option<bool>,
    pub short_description: Option<String>,
}

impl GrokSkillExtensions {
    pub fn is_empty(&self) -> bool {
        self.when_to_use.is_none()
            && self.paths.is_none()
            && self.argument_hint.is_none()
            && self.model.is_none()
            && self.effort.is_none()
            && self.user_invocable.is_none()
            && self.disable_model_invocation.is_none()
            && self.short_description.is_none()
    }
}

/// Canonical strict skill manifest produced only after validation succeeds.
///
/// Official fields keep their authored string forms. Grok extensions live in
/// [`GrokSkillExtensions`]. The markdown body is intentionally omitted so
/// inventories never carry prompt text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrictSkillManifest {
    pub name: String,
    pub description: String,
    pub license: Option<String>,
    pub compatibility: Option<String>,
    pub allowed_tools: Option<String>,
    /// Official string-to-string metadata, excluding the reserved `grok`
    /// object and `grok.*` dotted extension keys.
    pub metadata: BTreeMap<String, String>,
    pub grok: GrokSkillExtensions,
}

impl StrictSkillManifest {
    /// Tokens of the official space-separated `allowed-tools` wire string.
    pub fn allowed_tool_tokens(&self) -> Vec<&str> {
        self.allowed_tools
            .as_deref()
            .map(|raw| raw.split_ascii_whitespace().collect())
            .unwrap_or_default()
    }
}
