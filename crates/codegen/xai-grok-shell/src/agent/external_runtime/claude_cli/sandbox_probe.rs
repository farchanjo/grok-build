//! Process / sandbox inheritance probes for Claude CLI children (PR7).
//!
//! Parent sandbox inheritance is **not assumed blindly**. When the parent
//! sandbox is active and inheritance cannot be positively verified, the
//! external turn **fails closed before Claude is spawned**. Probes never
//! weaken parent sandbox or permission policy.
//!
//! Tests use fake child probes on macOS and Linux.

use std::path::Path;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// Observed sandbox posture of a (fake or real) child process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ChildSandboxObservation {
    pub platform: SandboxPlatform,
    /// Whether the child appears to run under an inherited sandbox profile.
    pub inherited_sandbox: bool,
    /// Whether outbound network appears restricted (not open).
    pub network_restricted: bool,
    /// Whether Claude API egress is expected to be permitted.
    pub claude_api_network_allowed: bool,
    /// True when posture was positively verified (marker or subsystem).
    pub positively_verified: bool,
    /// Human-readable probe notes (no secrets).
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxPlatform {
    MacOs,
    Linux,
    Other,
}

impl SandboxPlatform {
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else {
            Self::Other
        }
    }
}

/// Host-side expected posture for a Claude CLI child.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedChildPosture {
    /// When the parent has an active sandbox, children must inherit it.
    pub require_inherited_when_parent_active: bool,
    /// Claude subscription API needs egress; do not require total net deny.
    pub allow_claude_api_network: bool,
}

impl Default for ExpectedChildPosture {
    fn default() -> Self {
        Self {
            require_inherited_when_parent_active: true,
            allow_claude_api_network: true,
        }
    }
}

/// Result of verifying observation against expectation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxVerifyResult {
    Ok,
    /// Child is not under the intended sandbox — **fail closed** before spawn.
    InheritanceMissing {
        detail: String,
    },
    /// Network is more open than intended.
    NetworkTooOpen {
        detail: String,
    },
    /// Claude API network blocked when subscription path needs it.
    ClaudeApiNetworkBlocked {
        detail: String,
    },
}

impl SandboxVerifyResult {
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok)
    }

    pub fn blocks_spawn(&self) -> bool {
        !self.is_ok()
    }
}

/// Probe parent sandbox markers without weakening anything.
pub fn parent_sandbox_active() -> bool {
    xai_grok_sandbox::is_active()
}

/// Optional explicit verified posture injected by the sandbox subsystem
/// (or tests). When set, takes precedence over env-marker heuristics.
static EXPLICIT_VERIFIED_POSTURE: Mutex<Option<ChildSandboxObservation>> = Mutex::new(None);

/// Install an explicit verified child posture (sandbox subsystem or tests).
pub fn set_explicit_verified_posture(obs: Option<ChildSandboxObservation>) {
    *EXPLICIT_VERIFIED_POSTURE
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = obs;
}

/// Observe child sandbox posture via a probe function, preferring any
/// explicit verified posture from the sandbox subsystem.
pub fn observe_child_sandbox<F>(probe: F) -> ChildSandboxObservation
where
    F: FnOnce(SandboxPlatform) -> ChildSandboxObservation,
{
    if let Some(explicit) = EXPLICIT_VERIFIED_POSTURE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
    {
        return explicit;
    }
    probe(SandboxPlatform::current())
}

/// Default probe using environment markers and platform heuristics.
///
/// Does **not** claim inheritance solely because the parent is sandboxed.
/// Without a positive marker, `positively_verified` is false.
pub fn default_child_probe(platform: SandboxPlatform) -> ChildSandboxObservation {
    let mut notes = Vec::new();
    let parent_active = parent_sandbox_active();
    if parent_active {
        notes.push("parent sandbox marker active".into());
    } else {
        notes.push("parent sandbox not active (or disabled profile)".into());
    }

    let (inherited, verified) = match platform {
        SandboxPlatform::MacOs => {
            // Positive markers only. Seatbelt alone is not assumed inherited.
            let marker = std::env::var_os("GROK_SANDBOX_CHILD_INHERITED").is_some()
                || std::env::var_os("APP_SANDBOX_CONTAINER_ID").is_some();
            if marker {
                notes.push("macOS child sandbox marker present (positively verified)".into());
            } else {
                notes.push(
                    "macOS: no child sandbox marker; inheritance not positively verified".into(),
                );
            }
            (marker, marker)
        }
        SandboxPlatform::Linux => {
            let marker = std::env::var_os("GROK_SANDBOX_CHILD_INHERITED").is_some()
                || std::env::var_os("container").is_some()
                || path_exists("/.dockerenv")
                || xai_grok_sandbox::is_inside_bwrap();
            if marker {
                notes.push("Linux child isolation marker present (positively verified)".into());
            } else {
                notes.push(
                    "Linux: no bwrap/container child marker; inheritance not positively verified"
                        .into(),
                );
            }
            (marker, marker)
        }
        SandboxPlatform::Other => {
            notes.push("unsupported platform for sandbox inheritance probe".into());
            (false, false)
        }
    };

    let network_restricted = std::env::var_os("GROK_SANDBOX_CHILD_NET_RESTRICTED").is_some();
    let claude_api_network_allowed = std::env::var_os("GROK_SANDBOX_BLOCK_CLAUDE_API").is_none();

    ChildSandboxObservation {
        platform,
        inherited_sandbox: inherited,
        network_restricted,
        claude_api_network_allowed,
        positively_verified: verified,
        notes,
    }
}

fn path_exists(p: &str) -> bool {
    Path::new(p).exists()
}

/// Verify observation against expectation and parent activity.
///
/// When parent is active and inheritance is required, both
/// `inherited_sandbox` **and** `positively_verified` must be true.
pub fn verify_child_posture(
    observation: &ChildSandboxObservation,
    expected: &ExpectedChildPosture,
    parent_active: bool,
) -> SandboxVerifyResult {
    if parent_active && expected.require_inherited_when_parent_active {
        if !observation.inherited_sandbox || !observation.positively_verified {
            return SandboxVerifyResult::InheritanceMissing {
                detail: format!(
                    "parent sandbox is active but child inheritance was not positively verified \
                     (inherited={}, verified={}): {}",
                    observation.inherited_sandbox,
                    observation.positively_verified,
                    observation.notes.join("; ")
                ),
            };
        }
    }
    if expected.allow_claude_api_network && !observation.claude_api_network_allowed {
        return SandboxVerifyResult::ClaudeApiNetworkBlocked {
            detail: "Claude API network appears blocked; subscription path requires egress".into(),
        };
    }
    SandboxVerifyResult::Ok
}

/// Gate a Claude CLI turn: fail closed when sandbox inheritance is required
/// but not verified. Call **before** spawning Claude.
pub fn gate_turn_for_sandbox(
    observation: &ChildSandboxObservation,
    expected: &ExpectedChildPosture,
    parent_active: bool,
) -> Result<(), SandboxVerifyResult> {
    let result = verify_child_posture(observation, expected, parent_active);
    if result.blocks_spawn() {
        Err(result)
    } else {
        Ok(())
    }
}

/// Documented guarantee: we never weaken global sandbox policy from this module.
pub const SANDBOX_POLICY_NOTE: &str = "\
Claude CLI child sandbox probes never disable, relax, or reconfigure the parent \
Grok sandbox or permission policy. When the parent sandbox is active, inheritance \
must be positively verified or the external turn fails closed before spawn.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn does_not_assume_inheritance_without_marker() {
        unsafe {
            std::env::remove_var("GROK_SANDBOX_CHILD_INHERITED");
        }
        set_explicit_verified_posture(None);
        let obs = observe_child_sandbox(|p| ChildSandboxObservation {
            platform: p,
            inherited_sandbox: false,
            network_restricted: false,
            claude_api_network_allowed: true,
            positively_verified: false,
            notes: vec!["fake: no inheritance".into()],
        });
        assert!(!obs.inherited_sandbox);
        let result = verify_child_posture(&obs, &ExpectedChildPosture::default(), true);
        assert!(matches!(
            result,
            SandboxVerifyResult::InheritanceMissing { .. }
        ));
        assert!(result.blocks_spawn());
    }

    #[test]
    fn inheritance_missing_blocks_gate_turn() {
        let obs = ChildSandboxObservation {
            platform: SandboxPlatform::current(),
            inherited_sandbox: false,
            network_restricted: false,
            claude_api_network_allowed: true,
            positively_verified: false,
            notes: vec!["test".into()],
        };
        let err = gate_turn_for_sandbox(&obs, &ExpectedChildPosture::default(), true).unwrap_err();
        assert!(matches!(
            err,
            SandboxVerifyResult::InheritanceMissing { .. }
        ));
    }

    #[test]
    fn positively_verified_inheritance_allows_spawn() {
        for platform in [SandboxPlatform::MacOs, SandboxPlatform::Linux] {
            let obs = ChildSandboxObservation {
                platform,
                inherited_sandbox: true,
                network_restricted: true,
                claude_api_network_allowed: true,
                positively_verified: true,
                notes: vec![format!("fake {platform:?}")],
            };
            assert!(gate_turn_for_sandbox(&obs, &ExpectedChildPosture::default(), true).is_ok());
        }
    }

    #[test]
    fn inherited_without_positive_verification_fails() {
        let obs = ChildSandboxObservation {
            platform: SandboxPlatform::MacOs,
            inherited_sandbox: true,
            network_restricted: false,
            claude_api_network_allowed: true,
            positively_verified: false,
            notes: vec!["claimed but not verified".into()],
        };
        assert!(matches!(
            verify_child_posture(&obs, &ExpectedChildPosture::default(), true),
            SandboxVerifyResult::InheritanceMissing { .. }
        ));
    }

    #[test]
    fn explicit_verified_posture_overrides_probe() {
        set_explicit_verified_posture(Some(ChildSandboxObservation {
            platform: SandboxPlatform::current(),
            inherited_sandbox: true,
            network_restricted: true,
            claude_api_network_allowed: true,
            positively_verified: true,
            notes: vec!["subsystem verified".into()],
        }));
        let obs = observe_child_sandbox(|_| ChildSandboxObservation {
            platform: SandboxPlatform::current(),
            inherited_sandbox: false,
            network_restricted: false,
            claude_api_network_allowed: true,
            positively_verified: false,
            notes: vec!["would fail".into()],
        });
        assert!(obs.positively_verified);
        assert!(obs.inherited_sandbox);
        set_explicit_verified_posture(None);
    }

    #[test]
    fn parent_inactive_skips_inheritance_requirement() {
        let obs = ChildSandboxObservation {
            platform: SandboxPlatform::current(),
            inherited_sandbox: false,
            network_restricted: false,
            claude_api_network_allowed: true,
            positively_verified: false,
            notes: vec![],
        };
        assert!(gate_turn_for_sandbox(&obs, &ExpectedChildPosture::default(), false).is_ok());
    }

    #[test]
    fn claude_api_block_fails() {
        let obs = ChildSandboxObservation {
            platform: SandboxPlatform::current(),
            inherited_sandbox: true,
            network_restricted: true,
            claude_api_network_allowed: false,
            positively_verified: true,
            notes: vec![],
        };
        assert!(matches!(
            verify_child_posture(&obs, &ExpectedChildPosture::default(), true),
            SandboxVerifyResult::ClaudeApiNetworkBlocked { .. }
        ));
    }

    #[test]
    fn policy_note_documents_fail_closed() {
        assert!(SANDBOX_POLICY_NOTE.contains("fails closed"));
        assert!(
            SANDBOX_POLICY_NOTE.contains("never disable") || SANDBOX_POLICY_NOTE.contains("never")
        );
    }
}
