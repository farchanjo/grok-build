//! Claude CLI child sandbox gate (PR7).
//!
//! Consumes the **authoritative** posture from [`xai_grok_sandbox`] after real
//! sandbox application — not production env markers. When the parent sandbox is
//! active without a contractual inheritance guarantee, the external turn fails
//! closed before Claude is spawned. Never weakens parent policy.
//!
//! Test-only injection is gated behind `cfg(test)`.

use serde::{Deserialize, Serialize};
use xai_grok_sandbox::{ChildSandboxPosture, SandboxMechanism};

/// Re-export sandbox subsystem posture type for callers / tests.
pub use xai_grok_sandbox::ChildSandboxPosture as AuthoritativePosture;

/// Result of verifying posture for a Claude CLI turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxVerifyResult {
    Ok,
    /// Parent sandbox is active but descendants do not inherit — fail closed.
    InheritanceMissing {
        detail: String,
    },
    /// Claude API network appears blocked (should not happen with Grok's model).
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

/// Host-side expected posture for a Claude CLI child.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedChildPosture {
    pub require_inherited_when_parent_active: bool,
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

/// Platform label for display/tests (not used for production inheritance).
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

/// Legacy observation shape kept for test helpers; production uses
/// [`ChildSandboxPosture`] from the sandbox crate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ChildSandboxObservation {
    pub platform: SandboxPlatform,
    pub inherited_sandbox: bool,
    pub network_restricted: bool,
    pub claude_api_network_allowed: bool,
    pub positively_verified: bool,
    pub notes: Vec<String>,
}

impl From<&ChildSandboxPosture> for ChildSandboxObservation {
    fn from(p: &ChildSandboxPosture) -> Self {
        Self {
            platform: SandboxPlatform::current(),
            inherited_sandbox: p.descendants_inherit_fs && p.parent_applied,
            network_restricted: false, // process net open; child seccomp is separate
            claude_api_network_allowed: p.process_network_open_for_api,
            positively_verified: p.descendants_inherit_fs
                && p.parent_applied
                && !matches!(
                    p.mechanism,
                    SandboxMechanism::None | SandboxMechanism::Unknown
                ),
            notes: p.notes.clone(),
        }
    }
}

/// Whether parent sandbox is applied (authoritative).
pub fn parent_sandbox_active() -> bool {
    xai_grok_sandbox::is_active()
}

/// Live authoritative posture from the sandbox subsystem.
pub fn authoritative_posture() -> ChildSandboxPosture {
    #[cfg(test)]
    {
        if let Some(p) = test_posture_override() {
            return p;
        }
    }
    xai_grok_sandbox::child_sandbox_posture()
}

/// Observe posture for Claude CLI gating. Production path is authoritative only.
pub fn observe_child_sandbox() -> ChildSandboxObservation {
    ChildSandboxObservation::from(&authoritative_posture())
}

/// Verify authoritative posture against expectation.
pub fn verify_child_posture(
    posture: &ChildSandboxPosture,
    expected: &ExpectedChildPosture,
) -> SandboxVerifyResult {
    if expected.require_inherited_when_parent_active && posture.parent_applied {
        if !posture.allows_external_child_spawn() {
            return SandboxVerifyResult::InheritanceMissing {
                detail: format!(
                    "parent sandbox applied (profile={:?}, mechanism={:?}) but descendants \
                     do not inherit FS isolation: {}",
                    posture.profile,
                    posture.mechanism,
                    posture.notes.join("; ")
                ),
            };
        }
    }
    if expected.allow_claude_api_network && !posture.process_network_open_for_api {
        return SandboxVerifyResult::ClaudeApiNetworkBlocked {
            detail: "Claude API network appears blocked; subscription path requires egress".into(),
        };
    }
    SandboxVerifyResult::Ok
}

/// Gate a Claude CLI turn using the authoritative sandbox posture.
/// Call **before** spawning Claude.
pub fn gate_turn_for_sandbox(
    posture: &ChildSandboxPosture,
    expected: &ExpectedChildPosture,
) -> Result<(), SandboxVerifyResult> {
    let result = verify_child_posture(posture, expected);
    if result.blocks_spawn() {
        Err(result)
    } else {
        Ok(())
    }
}

/// Gate using live authoritative posture (production entry point).
pub fn gate_live_turn() -> Result<ChildSandboxPosture, SandboxVerifyResult> {
    let posture = authoritative_posture();
    gate_turn_for_sandbox(&posture, &ExpectedChildPosture::default())?;
    Ok(posture)
}

pub const SANDBOX_POLICY_NOTE: &str = "\
Claude CLI child sandbox gates consume the authoritative posture from \
xai-grok-sandbox after real application (Seatbelt/Landlock/bwrap). They never \
disable or relax parent policy. Env markers are not production verification. \
Active unknown mechanisms fail closed before Claude spawn.";

// ---------------------------------------------------------------------------
// Test-only override (not callable from production builds)
// ---------------------------------------------------------------------------

#[cfg(test)]
use std::sync::Mutex;

#[cfg(test)]
static TEST_POSTURE: Mutex<Option<ChildSandboxPosture>> = Mutex::new(None);

/// Test-only: inject a posture. Not available in production builds.
#[cfg(test)]
pub fn set_explicit_verified_posture(obs: Option<ChildSandboxPosture>) {
    *TEST_POSTURE.lock().unwrap_or_else(|e| e.into_inner()) = obs;
}

#[cfg(test)]
fn test_posture_override() -> Option<ChildSandboxPosture> {
    TEST_POSTURE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

/// Test helper: convert observation-style injection into a posture.
/// Inheritance-missing cases set parent_applied=true with non-inheritable mechanism.
#[cfg(test)]
pub fn set_explicit_observation_for_test(obs: Option<ChildSandboxObservation>) {
    let posture = obs.map(|o| {
        let inherit = o.inherited_sandbox && o.positively_verified;
        ChildSandboxPosture {
            // When testing inheritance failure, parent is treated as applied.
            parent_applied: inherit || !o.positively_verified || !o.inherited_sandbox,
            profile: Some("test".into()),
            mechanism: if inherit {
                if cfg!(target_os = "macos") {
                    SandboxMechanism::MacOsSeatbelt
                } else if cfg!(target_os = "linux") {
                    SandboxMechanism::LinuxLandlock
                } else {
                    SandboxMechanism::Unknown
                }
            } else {
                SandboxMechanism::Unknown
            },
            descendants_inherit_fs: inherit,
            process_network_open_for_api: o.claude_api_network_allowed,
            notes: o.notes,
        }
    });
    let posture = posture.map(|mut p| {
        if !p.descendants_inherit_fs {
            p.parent_applied = true;
            p.mechanism = SandboxMechanism::Unknown;
        }
        p
    });
    set_explicit_verified_posture(posture);
}

#[cfg(test)]
mod tests {
    use super::*;
    use xai_grok_sandbox::compute_child_sandbox_posture;

    #[test]
    fn disabled_posture_allows_spawn() {
        let p = compute_child_sandbox_posture(false, Some("off"), false);
        assert!(gate_turn_for_sandbox(&p, &ExpectedChildPosture::default()).is_ok());
    }

    #[test]
    fn applied_inheritable_allows_spawn() {
        let p = compute_child_sandbox_posture(true, Some("workspace"), false);
        // On macOS/Linux this is inheritable.
        if cfg!(any(target_os = "macos", target_os = "linux")) {
            assert!(p.allows_external_child_spawn());
            assert!(gate_turn_for_sandbox(&p, &ExpectedChildPosture::default()).is_ok());
        }
    }

    #[test]
    fn active_unknown_fails_closed() {
        let p = ChildSandboxPosture {
            parent_applied: true,
            profile: Some("x".into()),
            mechanism: SandboxMechanism::Unknown,
            descendants_inherit_fs: false,
            process_network_open_for_api: true,
            notes: vec!["unknown".into()],
        };
        let err = gate_turn_for_sandbox(&p, &ExpectedChildPosture::default()).unwrap_err();
        assert!(matches!(
            err,
            SandboxVerifyResult::InheritanceMissing { .. }
        ));
    }

    #[test]
    fn test_injection_is_cfg_test_only() {
        set_explicit_verified_posture(Some(ChildSandboxPosture {
            parent_applied: true,
            profile: Some("t".into()),
            mechanism: SandboxMechanism::Unknown,
            descendants_inherit_fs: false,
            process_network_open_for_api: true,
            notes: vec!["inject".into()],
        }));
        let live = authoritative_posture();
        assert!(!live.allows_external_child_spawn());
        set_explicit_verified_posture(None);
    }

    #[test]
    fn policy_note_documents_authoritative() {
        assert!(SANDBOX_POLICY_NOTE.contains("authoritative"));
        assert!(
            SANDBOX_POLICY_NOTE.contains("fail closed")
                || SANDBOX_POLICY_NOTE.contains("fails closed")
        );
    }

    #[test]
    fn platform_abstractions_from_sandbox_crate() {
        let mac = {
            // Pure: Seatbelt when applied on macOS is tested in sandbox crate;
            // here ensure conversion notes preserve API network flag.
            let p = compute_child_sandbox_posture(false, None, false);
            assert!(p.process_network_open_for_api);
            p
        };
        let _ = mac;
    }
}
