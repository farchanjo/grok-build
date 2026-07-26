//! Resume hardening for Claude CLI envelopes (PR7).
//!
//! Selected model / effort, cwd / worktree identity must match the durable
//! envelope or produce a clear new-session requirement. Binary version /
//! capability drift triggers re-probe; additive capabilities are OK; missing
//! required capabilities fail closed. Expired / missing Claude session
//! pointers produce actionable failures — never replay Grok native tool
//! transcripts as Claude history.

use super::discovery::{ClaudeCliDiscovery, MIN_CLAUDE_CLI_VERSION};
use crate::agent::execution_backend::ExternalAgentKind;
use crate::agent::external_runtime::{
    ExternalRuntimeEnvelope, ExternalRuntimeError, ExternalRuntimeErrorKind,
};

/// Capability that must remain present after resume when it was observed
/// on the envelope (fail closed if live probe lost it).
pub const REQUIRED_ON_RESUME_IF_SEEN: &[&str] = &[
    "streaming_input",
    "streaming_input_v1",
    "persistent_input_v1",
    "session_resume",
    "resume_v1",
    "interrupt_receipt_v1",
];

/// Capabilities required for persistent multi-turn stdin streaming.
/// Only enable persistent input when probe/init explicitly advertises one.
pub const PERSISTENT_INPUT_CAPABILITIES: &[&str] = &[
    "streaming_input",
    "streaming_input_v1",
    "persistent_input_v1",
];

/// Capability for interrupt receipt (cancel mid-turn).
pub const INTERRUPT_CAPABILITIES: &[&str] = &["interrupt_receipt_v1"];

/// Capability that allows one-shot `--resume` after child death.
/// When the capability list is non-empty, at least one of these (or an
/// empty-list baseline for older CLIs that never advertise caps) is required.
pub const RESUME_CAPABILITIES: &[&str] = &["session_resume", "resume_v1"];

/// Baseline capability tokens that establish a known capability matrix.
/// If the probe reports a non-empty list that includes none of the baseline
/// tokens and none of the resume tokens, oneshot resume fails closed.
pub const BASELINE_CAPABILITY_MARKERS: &[&str] = &[
    "interrupt_receipt_v1",
    "streaming_input",
    "streaming_input_v1",
    "persistent_input_v1",
    "session_resume",
    "resume_v1",
];

/// Why a resume was rejected (actionable for the user / UI).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResumeHardeningError {
    MissingSessionPointer,
    KindMismatch { found: String },
    ModelMismatch { envelope: String, requested: String },
    EffortMismatch { envelope: String, requested: String },
    CwdMismatch { envelope: String, current: String },
    WorktreeMismatch { envelope: String, current: String },
    VersionTooOld { found: String, minimum: String },
    MissingRequiredCapability { capability: String, version: String },
    VersionDriftIncompatible { envelope: String, live: String },
}

impl std::fmt::Display for ResumeHardeningError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingSessionPointer => write!(
                f,
                "Claude session pointer is missing or expired. Start a new Claude Agent CLI \
                 session — Grok will not replay native tool history as Claude transcript."
            ),
            Self::KindMismatch { found } => write!(
                f,
                "envelope kind '{found}' is not claude_cli; start a new session"
            ),
            Self::ModelMismatch {
                envelope,
                requested,
            } => write!(
                f,
                "selected model '{requested}' does not match Claude session model '{envelope}'. \
                 Start a new Claude session to change models."
            ),
            Self::EffortMismatch {
                envelope,
                requested,
            } => write!(
                f,
                "reasoning effort '{requested}' does not match Claude session effort '{envelope}'. \
                 Start a new Claude session to change effort."
            ),
            Self::CwdMismatch { envelope, current } => write!(
                f,
                "workspace cwd '{current}' does not match Claude session cwd '{envelope}'. \
                 Start a new Claude session in the current workspace."
            ),
            Self::WorktreeMismatch { envelope, current } => write!(
                f,
                "worktree identity '{current}' does not match Claude session '{envelope}'. \
                 Start a new Claude session for this worktree."
            ),
            Self::VersionTooOld { found, minimum } => write!(
                f,
                "Claude CLI {found} is below minimum {minimum}; upgrade the official binary"
            ),
            Self::MissingRequiredCapability {
                capability,
                version,
            } => write!(
                f,
                "Claude CLI v{version} is missing required capability '{capability}' \
                 (fail closed after version drift re-probe)"
            ),
            Self::VersionDriftIncompatible { envelope, live } => write!(
                f,
                "Claude CLI version drifted incompatibly (session saw {envelope}, live is {live}). \
                 Start a new session after re-probe."
            ),
        }
    }
}

impl ResumeHardeningError {
    pub fn into_runtime_error(self) -> ExternalRuntimeError {
        ExternalRuntimeError::new(
            ExternalRuntimeErrorKind::InvalidRequest,
            self.to_string(),
            Some(ExternalAgentKind::ClaudeCli),
        )
    }
}

/// Validate envelope identity against the live request / discovery.
pub fn validate_resume(
    envelope: &ExternalRuntimeEnvelope,
    live: &ClaudeCliDiscovery,
    requested_model: Option<&str>,
    requested_effort: Option<&str>,
    current_cwd: Option<&str>,
    current_worktree: Option<&str>,
) -> Result<(), ResumeHardeningError> {
    if envelope.kind != ExternalAgentKind::ClaudeCli {
        return Err(ResumeHardeningError::KindMismatch {
            found: envelope.kind.as_str().to_owned(),
        });
    }
    if envelope
        .session_pointer
        .as_ref()
        .map(|s| s.trim().is_empty())
        .unwrap_or(true)
    {
        return Err(ResumeHardeningError::MissingSessionPointer);
    }

    // Model match when both sides present and non-empty.
    if let (Some(env_m), Some(req_m)) = (
        envelope.selected_model.as_deref().filter(|s| !s.is_empty()),
        requested_model.filter(|s| !s.is_empty()),
    ) {
        if !models_compatible(env_m, req_m) {
            return Err(ResumeHardeningError::ModelMismatch {
                envelope: env_m.to_owned(),
                requested: req_m.to_owned(),
            });
        }
    }

    if let (Some(env_e), Some(req_e)) = (
        envelope
            .reasoning_effort
            .as_deref()
            .filter(|s| !s.is_empty()),
        requested_effort.filter(|s| !s.is_empty()),
    ) {
        if !env_e.eq_ignore_ascii_case(req_e) {
            return Err(ResumeHardeningError::EffortMismatch {
                envelope: env_e.to_owned(),
                requested: req_e.to_owned(),
            });
        }
    }

    if let (Some(env_cwd), Some(cur)) = (
        envelope.cwd.as_deref().filter(|s| !s.is_empty()),
        current_cwd.filter(|s| !s.is_empty()),
    ) {
        if !paths_equal(env_cwd, cur) {
            return Err(ResumeHardeningError::CwdMismatch {
                envelope: env_cwd.to_owned(),
                current: cur.to_owned(),
            });
        }
    }

    if let (Some(env_wt), Some(cur)) = (
        envelope
            .worktree_identity
            .as_deref()
            .filter(|s| !s.is_empty()),
        current_worktree.filter(|s| !s.is_empty()),
    ) {
        if env_wt != cur {
            return Err(ResumeHardeningError::WorktreeMismatch {
                envelope: env_wt.to_owned(),
                current: cur.to_owned(),
            });
        }
    }

    // Version floor.
    let minimum = semver::Version::parse(MIN_CLAUDE_CLI_VERSION).expect("static min");
    if live.version < minimum {
        return Err(ResumeHardeningError::VersionTooOld {
            found: live.version.to_string(),
            minimum: minimum.to_string(),
        });
    }

    // Version drift: major must match; minor/patch may advance (additive OK).
    if let Some(env_ver) = envelope.observed_version.as_deref() {
        if let Some(env_parsed) = parse_loose_version(env_ver) {
            if live.version.major != env_parsed.major {
                return Err(ResumeHardeningError::VersionDriftIncompatible {
                    envelope: env_ver.to_owned(),
                    live: live.version.to_string(),
                });
            }
            // Downgrade minor is suspicious — fail closed.
            if live.version.minor < env_parsed.minor {
                return Err(ResumeHardeningError::VersionDriftIncompatible {
                    envelope: env_ver.to_owned(),
                    live: live.version.to_string(),
                });
            }
        }
    }

    // Required capabilities from envelope that must still be present on live
    // when the envelope listed them. Fail closed on loss (not additive-only).
    for req in REQUIRED_ON_RESUME_IF_SEEN {
        if envelope.capabilities.iter().any(|c| c == *req) {
            // Prefer live caps when reported; otherwise re-check envelope set
            // against live empty → treat as missing when live advertised any.
            if !live.capabilities.is_empty() && !live.capabilities.iter().any(|c| c == *req) {
                return Err(ResumeHardeningError::MissingRequiredCapability {
                    capability: (*req).to_owned(),
                    version: live.version.to_string(),
                });
            }
        }
    }

    Ok(())
}

/// Validate that persistent mode may be selected given advertised capabilities.
/// Missing required streaming-input capability fails closed.
pub fn require_persistent_capability(capabilities: &[String]) -> Result<(), ResumeHardeningError> {
    if supports_persistent_input(capabilities) {
        Ok(())
    } else {
        Err(ResumeHardeningError::MissingRequiredCapability {
            capability: "streaming_input_v1".into(),
            version: "unknown".into(),
        })
    }
}

/// Whether envelope capabilities advertise persistent multi-turn stdin.
pub fn supports_persistent_input(capabilities: &[String]) -> bool {
    capabilities.iter().any(|c| {
        PERSISTENT_INPUT_CAPABILITIES
            .iter()
            .any(|req| c == req || c.eq_ignore_ascii_case(req))
    })
}

/// Whether interrupt receipt is supported.
pub fn supports_interrupt(capabilities: &[String]) -> bool {
    capabilities.iter().any(|c| {
        INTERRUPT_CAPABILITIES
            .iter()
            .any(|req| c == req || c.eq_ignore_ascii_case(req))
    })
}

/// Whether one-shot `--resume` after child death is allowed by capability matrix.
///
/// - Empty capability list (older CLI / no advertisement): allow (official
///   `--resume` by session id has long been available without a capability token).
/// - Non-empty list: require an explicit resume token **or** a known baseline
///   marker (e.g. `interrupt_receipt_v1`) so we know the matrix is from a
///   compatible binary. Unknown-only matrices fail closed.
pub fn supports_oneshot_resume(capabilities: &[String]) -> bool {
    if capabilities.is_empty() {
        return true;
    }
    let has_resume = capabilities.iter().any(|c| {
        RESUME_CAPABILITIES
            .iter()
            .any(|req| c == req || c.eq_ignore_ascii_case(req))
    });
    if has_resume {
        return true;
    }
    // Compatible baseline without explicit resume token (still allows --resume).
    capabilities.iter().any(|c| {
        BASELINE_CAPABILITY_MARKERS
            .iter()
            .any(|req| c == req || c.eq_ignore_ascii_case(req))
    })
}

fn models_compatible(a: &str, b: &str) -> bool {
    if a.eq_ignore_ascii_case(b) {
        return true;
    }
    // Alias normalization: "sonnet" matches "claude-sonnet-*" prefixes.
    let na = normalize_model_alias(a);
    let nb = normalize_model_alias(b);
    na == nb
}

fn normalize_model_alias(m: &str) -> String {
    let lower = m.trim().to_ascii_lowercase();
    if lower.contains("opus") {
        return "opus".into();
    }
    if lower.contains("haiku") {
        return "haiku".into();
    }
    if lower.contains("sonnet") {
        return "sonnet".into();
    }
    lower
}

fn paths_equal(a: &str, b: &str) -> bool {
    let pa = std::path::Path::new(a);
    let pb = std::path::Path::new(b);
    if pa == pb {
        return true;
    }
    // Best-effort canonicalize when paths exist.
    match (std::fs::canonicalize(pa), std::fs::canonicalize(pb)) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => a.trim_end_matches('/') == b.trim_end_matches('/'),
    }
}

fn parse_loose_version(s: &str) -> Option<semver::Version> {
    let re = s
        .split(|c: char| !c.is_ascii_digit() && c != '.')
        .find(|t| t.split('.').count() >= 3 && t.chars().any(|c| c == '.'))?;
    let core: String = re
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    semver::Version::parse(&core).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::external_runtime::ExternalRuntimeEnvelope;
    use std::time::SystemTime;

    fn disc(version: &str, caps: &[&str]) -> ClaudeCliDiscovery {
        ClaudeCliDiscovery {
            executable: std::path::PathBuf::from("/usr/bin/claude"),
            version: semver::Version::parse(version).unwrap(),
            capabilities: caps.iter().map(|s| (*s).to_owned()).collect(),
            file_len: 1,
            modified: Some(SystemTime::UNIX_EPOCH),
        }
    }

    fn env_with(
        pointer: Option<&str>,
        model: Option<&str>,
        effort: Option<&str>,
        cwd: Option<&str>,
        version: Option<&str>,
    ) -> ExternalRuntimeEnvelope {
        let mut e = ExternalRuntimeEnvelope::for_kind(ExternalAgentKind::ClaudeCli);
        e.session_pointer = pointer.map(|s| s.to_owned());
        e.selected_model = model.map(|s| s.to_owned());
        e.reasoning_effort = effort.map(|s| s.to_owned());
        e.cwd = cwd.map(|s| s.to_owned());
        e.observed_version = version.map(|s| s.to_owned());
        e
    }

    #[test]
    fn missing_pointer_actionable() {
        let e = env_with(None, None, None, None, None);
        let err = validate_resume(&e, &disc("2.1.250", &[]), None, None, None, None).unwrap_err();
        assert!(matches!(err, ResumeHardeningError::MissingSessionPointer));
        assert!(err.to_string().contains("Start a new"));
        assert!(err.to_string().contains("will not replay"));
    }

    #[test]
    fn model_mismatch_requires_new_session() {
        let e = env_with(Some("s1"), Some("sonnet"), None, None, Some("2.1.250"));
        let err =
            validate_resume(&e, &disc("2.1.250", &[]), Some("opus"), None, None, None).unwrap_err();
        assert!(matches!(err, ResumeHardeningError::ModelMismatch { .. }));
    }

    #[test]
    fn model_aliases_compatible() {
        let e = env_with(
            Some("s1"),
            Some("claude-sonnet-4"),
            None,
            None,
            Some("2.1.250"),
        );
        validate_resume(&e, &disc("2.1.250", &[]), Some("sonnet"), None, None, None).unwrap();
    }

    #[test]
    fn cwd_mismatch() {
        let e = env_with(Some("s1"), None, None, Some("/old"), Some("2.1.250"));
        let err =
            validate_resume(&e, &disc("2.1.250", &[]), None, None, Some("/new"), None).unwrap_err();
        assert!(matches!(err, ResumeHardeningError::CwdMismatch { .. }));
    }

    #[test]
    fn major_version_drift_fails() {
        let e = env_with(Some("s1"), None, None, None, Some("2.1.250"));
        let err = validate_resume(&e, &disc("3.0.0", &[]), None, None, None, None).unwrap_err();
        assert!(matches!(
            err,
            ResumeHardeningError::VersionDriftIncompatible { .. }
        ));
    }

    #[test]
    fn additive_patch_ok() {
        let e = env_with(Some("s1"), None, None, None, Some("2.1.217"));
        validate_resume(&e, &disc("2.1.250", &[]), None, None, None, None).unwrap();
    }

    #[test]
    fn persistent_input_requires_explicit_cap() {
        assert!(!supports_persistent_input(&[]));
        assert!(!supports_persistent_input(&["interrupt_receipt_v1".into()]));
        assert!(supports_persistent_input(&["streaming_input_v1".into()]));
        assert!(supports_persistent_input(&["streaming_input".into()]));
    }

    #[test]
    fn effort_mismatch() {
        let e = env_with(Some("s1"), None, Some("high"), None, Some("2.1.250"));
        let err =
            validate_resume(&e, &disc("2.1.250", &[]), None, Some("low"), None, None).unwrap_err();
        assert!(matches!(err, ResumeHardeningError::EffortMismatch { .. }));
    }

    #[test]
    fn missing_required_capability_on_resume_fails_closed() {
        let mut e = env_with(Some("s1"), None, None, None, Some("2.1.250"));
        e.capabilities = vec!["streaming_input_v1".into(), "interrupt_receipt_v1".into()];
        let live = disc("2.1.250", &["interrupt_receipt_v1"]);
        let err = validate_resume(&e, &live, None, None, None, None).unwrap_err();
        assert!(matches!(
            err,
            ResumeHardeningError::MissingRequiredCapability { capability, .. }
                if capability == "streaming_input_v1"
        ));
    }

    #[test]
    fn oneshot_resume_empty_caps_allowed() {
        assert!(supports_oneshot_resume(&[]));
    }

    #[test]
    fn oneshot_resume_requires_baseline_when_caps_advertised() {
        assert!(supports_oneshot_resume(&["interrupt_receipt_v1".into()]));
        assert!(supports_oneshot_resume(&["session_resume".into()]));
        assert!(!supports_oneshot_resume(&["totally_unknown_cap".into()]));
    }

    #[test]
    fn require_persistent_capability_fails_closed() {
        assert!(require_persistent_capability(&[]).is_err());
        assert!(require_persistent_capability(&["streaming_input_v1".into()]).is_ok());
    }
}
