//! Pinned Agent Skills specification/reference constants.
//!
//! The SHA below is an immutable pin of Apache-2.0
//! `agentskills/agentskills` (specification + `skills-ref` reference
//! behavior). Runtime code must never fetch that repository.

use unicode_normalization::UnicodeNormalization;

/// Git revision of `https://github.com/agentskills/agentskills` whose
/// specification and `skills-ref` validator behavior this crate pins.
pub const AGENTSKILLS_SPEC_REVISION: &str = "69ef37e9424c0a7ea9dd2293b559e43ec8176379";

/// Canonical repository URL for the pinned revision. Informational only;
/// never used as a fetch endpoint.
pub const AGENTSKILLS_SPEC_REPOSITORY: &str = "https://github.com/agentskills/agentskills";

/// Required skill file name. Lowercase `skill.md` is not accepted: that
/// official `skills-ref` fallback is treated as repair and is rejected.
pub const SKILL_MD_FILE_NAME: &str = "SKILL.md";

/// Official `name` maximum length in Unicode code points.
pub const MAX_NAME_CHARS: usize = 64;

/// Official `description` maximum length in Unicode code points.
pub const MAX_DESCRIPTION_CHARS: usize = 1024;

/// Official `compatibility` maximum length in Unicode code points.
pub const MAX_COMPATIBILITY_CHARS: usize = 500;

/// Frontmatter YAML is rejected above this byte size. Bounds parse work
/// and avoids copying large untrusted blobs into diagnostics.
pub const MAX_FRONTMATTER_BYTES: usize = 4096;

/// Official top-level frontmatter keys (`skills-ref` `ALLOWED_FIELDS`).
pub const OFFICIAL_TOP_LEVEL_KEYS: &[&str] = &[
    "name",
    "description",
    "license",
    "compatibility",
    "metadata",
    "allowed-tools",
];

/// Legacy Grok top-level keys that must move under `metadata.grok.*`.
pub const LEGACY_GROK_TOP_LEVEL_KEYS: &[&str] = &[
    "when-to-use",
    "when_to_use",
    "argument-hint",
    "user-invocable",
    "disable-model-invocation",
    "model",
    "effort",
    "paths",
    "short-description",
];

/// Nested `metadata.grok` object key.
pub const GROK_EXTENSION_OBJECT_KEY: &str = "grok";

/// Dotted `metadata` key prefix for namespaced Grok extensions.
pub const GROK_EXTENSION_PREFIX: &str = "grok.";

/// Allowed `metadata.grok.*` extension keys (without the `grok.` prefix).
pub const GROK_EXTENSION_KEYS: &[&str] = &[
    "when-to-use",
    "paths",
    "argument-hint",
    "model",
    "effort",
    "user-invocable",
    "disable-model-invocation",
    "short-description",
];

pub const MAX_GROK_WHEN_TO_USE_CHARS: usize = 1024;
pub const MAX_GROK_SHORT_DESCRIPTION_CHARS: usize = 256;
pub const MAX_GROK_ARGUMENT_HINT_CHARS: usize = 256;
pub const MAX_GROK_MODEL_CHARS: usize = 128;
pub const MAX_GROK_EFFORT_CHARS: usize = 32;
pub const MAX_GROK_PATH_CHARS: usize = 256;
pub const MAX_GROK_PATHS: usize = 32;

/// Runtime discovery routes every SKILL.md source through the canonical
/// strict validator. Invalid rows are quarantined and never advertised.
pub const STRICT_VALIDATOR_RUNTIME_ENABLED: bool = true;

/// Apply Unicode NFKC, matching `skills-ref` `unicodedata.normalize("NFKC", ...)`.
pub fn nfkc(s: &str) -> String {
    s.nfkc().collect()
}

/// Official `skills-ref` name grammar after NFKC (letters, digits, hyphen).
///
/// Matches [`super::validator`] `validate_name` aside from the directory-name
/// equality check. Used by atomic publish/create so Unicode names the pinned
/// validator accepts are also publishable.
pub fn is_official_publishable_name(name: &str) -> bool {
    let normalized = nfkc(name.trim());
    !normalized.is_empty()
        && normalized.chars().count() <= MAX_NAME_CHARS
        && normalized == normalized.to_lowercase()
        && !normalized.starts_with('-')
        && !normalized.ends_with('-')
        && !normalized.contains("--")
        && normalized.chars().all(|c| c.is_alphanumeric() || c == '-')
}

pub fn is_official_top_level_key(key: &str) -> bool {
    OFFICIAL_TOP_LEVEL_KEYS.contains(&key)
}

pub fn is_legacy_grok_top_level_key(key: &str) -> bool {
    LEGACY_GROK_TOP_LEVEL_KEYS.contains(&key)
}

pub fn grok_extension_leaf(key: &str) -> Option<&str> {
    key.strip_prefix(GROK_EXTENSION_PREFIX)
        .filter(|leaf| GROK_EXTENSION_KEYS.contains(leaf))
}

pub fn is_known_grok_extension_leaf(leaf: &str) -> bool {
    GROK_EXTENSION_KEYS.contains(&leaf)
}
