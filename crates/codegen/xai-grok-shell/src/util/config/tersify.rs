//! Tersify config: scope/level resolution and persistence.
//!
//! Backing keys live in `[hints]` of `config.toml` (the pager-owned hint
//! namespace written in place by `set_hint`):
//!
//! - `tersify_scope = "main_only" | "all" | "off"` — who sees compressed style.
//!   The default `main_only` is the product rule: subagent output enters the
//!   main context raw; only the main conversation's own replies are compressed.
//! - `tersify_level = "lite" | "full" | "ultra"` — how hard prose is compressed.
//!   An unrecognized value is a config error: it falls back to the default with
//!   a loud log, never silently enables anything stronger.
//!
//! Resolution is fail-closed: scope `off` short-circuits everything, and any
//! store/engine failure degrades to raw output rather than to guessing.

use serde::{Deserialize, Serialize};
use toml::Value as TomlValue;

/// Who sees tersified main-context output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TersifyScope {
    /// Only the main conversation (subagent output stays raw). Default.
    #[default]
    MainOnly,
    /// Main conversation and subagent-visible text.
    All,
    /// Feature off: everything stays raw.
    Off,
}

impl TersifyScope {
    /// Parse with fail-closed semantics. Unrecognized strings fall back to the
    /// default with a debug log, matching how `[hints] worktree_mode` parses.
    #[must_use]
    pub fn from_config_str(s: &str) -> Self {
        match s {
            "main_only" => Self::MainOnly,
            "all" => Self::All,
            "off" => Self::Off,
            other => {
                tracing::debug!(
                    value = other,
                    "unrecognised tersify_scope, defaulting to main_only"
                );
                Self::MainOnly
            }
        }
    }

    #[must_use]
    pub const fn as_config_str(self) -> &'static str {
        match self {
            Self::MainOnly => "main_only",
            Self::All => "all",
            Self::Off => "off",
        }
    }
}

/// How hard main-context prose is compressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TersifyLevel {
    /// No filler or hedging; full sentences and articles stay.
    Lite,
    /// Drop articles and filler, fragments allowed. Default.
    #[default]
    Full,
    /// Maximum compression while cause-and-effect stays unambiguous.
    Ultra,
}

impl TersifyLevel {
    #[must_use]
    pub fn from_config_str(s: &str) -> Self {
        match s {
            "lite" => Self::Lite,
            "full" => Self::Full,
            "ultra" => Self::Ultra,
            other => {
                tracing::debug!(
                    value = other,
                    "unrecognised tersify_level, defaulting to full"
                );
                Self::Full
            }
        }
    }

    #[must_use]
    pub const fn as_config_str(self) -> &'static str {
        match self {
            Self::Lite => "lite",
            Self::Full => "full",
            Self::Ultra => "ultra",
        }
    }
}

/// The resolved tersify policy for one process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TersifyConfig {
    pub scope: TersifyScope,
    pub level: TersifyLevel,
}

impl TersifyConfig {
    /// Read the effective config and resolve `[hints] tersify_*`. Any read
    /// error yields defaults (scope main_only, level full).
    #[must_use]
    pub fn load() -> Self {
        crate::config::load_effective_config()
            .ok()
            .map(|v| Self::from_value(Some(&v)))
            .unwrap_or_default()
    }

    /// Resolve from a merged config value (unit-testable; `None` = defaults).
    #[must_use]
    pub fn from_value(root: Option<&TomlValue>) -> Self {
        let hints = root.and_then(|r| r.get("hints"));
        let scope = hints
            .and_then(|h| h.get("tersify_scope"))
            .and_then(TomlValue::as_str)
            .map(TersifyScope::from_config_str);
        let level = hints
            .and_then(|h| h.get("tersify_level"))
            .and_then(TomlValue::as_str)
            .map(TersifyLevel::from_config_str);
        Self {
            scope: scope.unwrap_or_default(),
            level: level.unwrap_or_default(),
        }
    }

    /// Whether the main conversation's replies should be tersified.
    #[must_use]
    pub const fn applies_to_main_context(self) -> bool {
        !matches!(self.scope, TersifyScope::Off)
    }

    /// Whether subagent-visible text should be tersified. Under `MainOnly`
    /// (the default) subagent output is always raw.
    #[must_use]
    pub const fn applies_to_subagents(self) -> bool {
        matches!(self.scope, TersifyScope::All)
    }

    /// Whether a session acting in the given role should tersify its own text.
    ///
    /// The single decision point for style application. A child subagent
    /// session under `MainOnly` must receive the model stream raw; only the
    /// main conversation is compressed.
    #[must_use]
    pub const fn applies_to(self, is_subagent_session: bool) -> bool {
        match self.scope {
            TersifyScope::Off => false,
            TersifyScope::MainOnly => !is_subagent_session,
            TersifyScope::All => true,
        }
    }
}

/// Test-only: parse a TOML string via the crate's own `toml` (which does not
/// re-export `IntoDeserializer` at this path; `from_str` is its entry point).
#[cfg(test)]
fn toml_src(s: &str) -> TomlValue {
    toml::from_str(s).expect("test TOML must parse")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_main_only_and_full() {
        let cfg = TersifyConfig::from_value(None);
        assert_eq!(cfg.scope, TersifyScope::MainOnly);
        assert_eq!(cfg.level, TersifyLevel::Full);
        assert!(cfg.applies_to_main_context());
        assert!(!cfg.applies_to_subagents());
    }

    #[test]
    fn off_scope_disables_everything() {
        let v = toml_src("[hints]\ntersify_scope = \"off\"\n");
        let cfg = TersifyConfig::from_value(Some(&v));
        assert!(!cfg.applies_to_main_context());
        assert!(!cfg.applies_to_subagents());
    }

    #[test]
    fn all_scope_reaches_subagents() {
        let v = toml_src("[hints]\ntersify_scope = \"all\"\n");
        let cfg = TersifyConfig::from_value(Some(&v));
        assert!(cfg.applies_to_main_context());
        assert!(cfg.applies_to_subagents());
    }

    #[test]
    fn unrecognized_scope_falls_back_without_strongening() {
        // A typo must never enable MORE compression than the default.
        let v = toml_src("[hints]\ntersify_scope = \"everything\"\ntersify_level = \"nonsense\"\n");
        let cfg = TersifyConfig::from_value(Some(&v));
        assert_eq!(cfg.scope, TersifyScope::MainOnly);
        assert_eq!(cfg.level, TersifyLevel::Full);
    }

    #[test]
    fn round_trips_through_config_strings() {
        for (scope, s) in [
            (TersifyScope::MainOnly, "main_only"),
            (TersifyScope::All, "all"),
            (TersifyScope::Off, "off"),
        ] {
            assert_eq!(scope.as_config_str(), s);
            assert_eq!(TersifyScope::from_config_str(s), scope);
        }
        for (level, s) in [
            (TersifyLevel::Lite, "lite"),
            (TersifyLevel::Full, "full"),
            (TersifyLevel::Ultra, "ultra"),
        ] {
            assert_eq!(level.as_config_str(), s);
            assert_eq!(TersifyLevel::from_config_str(s), level);
        }
    }

    #[test]
    fn main_only_scope_keeps_subagent_output_raw() {
        // The product rule, pinned: under the default scope a child subagent
        // session never tersifies; the main conversation always does.
        let cfg = TersifyConfig::from_value(None);
        assert!(cfg.applies_to(false), "main context tersifies");
        assert!(!cfg.applies_to(true), "subagent output must stay raw");
    }

    #[test]
    fn off_scope_applies_to_no_session_at_all() {
        let v = toml_src("[hints]\ntersify_scope = \"off\"\n");
        let cfg = TersifyConfig::from_value(Some(&v));
        assert!(!cfg.applies_to(false));
        assert!(!cfg.applies_to(true));
    }

    #[test]
    fn all_scope_reaches_both_roles() {
        let v = toml_src("[hints]\ntersify_scope = \"all\"\n");
        let cfg = TersifyConfig::from_value(Some(&v));
        assert!(cfg.applies_to(false));
        assert!(cfg.applies_to(true));
    }

    #[test]
    fn hints_from_another_table_never_leak_in() {
        let v = toml_src("[ui]\ntersify_scope = \"all\"\n");
        let cfg = TersifyConfig::from_value(Some(&v));
        assert_eq!(cfg.scope, TersifyScope::MainOnly);
    }
}
