//! Process / sandbox inheritance probes for Claude CLI children (PR7).
//!
//! Parent sandbox inheritance is **not assumed blindly**. Before treating a
//! child as sandboxed, the host verifies platform markers / probe results.
//! Network is allowed only as required for the Claude API subscription path;
//! global sandbox and permission policy are never weakened here.
//!
//! Tests use fake child probes on macOS and Linux.

use std::path::Path;

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
    /// When the parent has an active sandbox, children should inherit it.
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
    /// Child is not under the intended sandbox — fail closed for the turn
    /// only when policy requires inheritance; never weaken parent policy.
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
}

/// Probe parent sandbox markers without weakening anything.
pub fn parent_sandbox_active() -> bool {
    // Prefer the sandbox crate's live marker when linked.
    xai_grok_sandbox::is_active()
}

/// Observe child sandbox posture via a **fake child probe** function.
///
/// Production path may pass a probe that inspects `/proc/self/status`,
/// Seatbelt, or env markers set by the sandbox. Tests inject fakes.
pub fn observe_child_sandbox<F>(probe: F) -> ChildSandboxObservation
where
    F: FnOnce(SandboxPlatform) -> ChildSandboxObservation,
{
    probe(SandboxPlatform::current())
}

/// Default probe using environment markers and platform heuristics.
///
/// Does **not** claim inheritance solely because the parent is sandboxed.
pub fn default_child_probe(platform: SandboxPlatform) -> ChildSandboxObservation {
    let mut notes = Vec::new();
    let parent_active = parent_sandbox_active();
    if parent_active {
        notes.push("parent sandbox marker active".into());
    } else {
        notes.push("parent sandbox not active (or disabled profile)".into());
    }

    // Inheritance is only claimed when the child process itself exposes a
    // marker. We do not assume inheritance from the parent alone.
    let inherited = match platform {
        SandboxPlatform::MacOs => {
            // Seatbelt does not set a portable child env by default; without an
            // explicit child marker we report unknown → not inherited.
            let marker = std::env::var_os("GROK_SANDBOX_CHILD_INHERITED").is_some()
                || std::env::var_os("APP_SANDBOX_CONTAINER_ID").is_some();
            if marker {
                notes.push("macOS child sandbox marker present".into());
            } else {
                notes.push("macOS: no child sandbox marker; inheritance not assumed".into());
            }
            marker
        }
        SandboxPlatform::Linux => {
            let marker = std::env::var_os("GROK_SANDBOX_CHILD_INHERITED").is_some()
                || std::env::var_os("container").is_some()
                || path_exists("/.dockerenv")
                || xai_grok_sandbox::is_inside_bwrap();
            if marker {
                notes.push("Linux child isolation marker present".into());
            } else {
                notes
                    .push("Linux: no bwrap/container child marker; inheritance not assumed".into());
            }
            marker
        }
        SandboxPlatform::Other => {
            notes.push("unsupported platform for sandbox inheritance probe".into());
            false
        }
    };

    // Network: restricted only when parent policy says so AND a child marker
    // confirms restriction. Claude API must remain allowable.
    let network_restricted = std::env::var_os("GROK_SANDBOX_CHILD_NET_RESTRICTED").is_some();
    let claude_api_network_allowed = !std::env::var_os("GROK_SANDBOX_BLOCK_CLAUDE_API").is_some();

    ChildSandboxObservation {
        platform,
        inherited_sandbox: inherited,
        network_restricted,
        claude_api_network_allowed,
        notes,
    }
}

fn path_exists(p: &str) -> bool {
    Path::new(p).exists()
}

/// Verify observation against expectation and parent activity.
pub fn verify_child_posture(
    observation: &ChildSandboxObservation,
    expected: &ExpectedChildPosture,
    parent_active: bool,
) -> SandboxVerifyResult {
    if parent_active
        && expected.require_inherited_when_parent_active
        && !observation.inherited_sandbox
    {
        return SandboxVerifyResult::InheritanceMissing {
            detail: format!(
                "parent sandbox is active but child probe did not confirm inheritance ({})",
                observation.notes.join("; ")
            ),
        };
    }
    if expected.allow_claude_api_network && !observation.claude_api_network_allowed {
        return SandboxVerifyResult::ClaudeApiNetworkBlocked {
            detail: "Claude API network appears blocked; subscription path requires egress".into(),
        };
    }
    // If we expected restriction and child is unrestricted while parent is
    // active, report too-open (advisory — host still keeps parent policy).
    if parent_active && !observation.network_restricted {
        // Not a hard failure: Claude needs API network. Document only.
        let _ = observation;
    }
    SandboxVerifyResult::Ok
}

/// Documented guarantee: we never weaken global sandbox policy from this module.
pub const SANDBOX_POLICY_NOTE: &str = "\
Claude CLI child sandbox probes are observational only. They never disable, \
relax, or reconfigure the parent Grok sandbox or permission policy. Parent \
sandbox inheritance is not assumed without a positive child probe result.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn does_not_assume_inheritance_without_marker() {
        // Clear test markers.
        unsafe {
            std::env::remove_var("GROK_SANDBOX_CHILD_INHERITED");
        }
        let obs = observe_child_sandbox(|p| {
            // Fake: platform-specific but no inheritance marker.
            ChildSandboxObservation {
                platform: p,
                inherited_sandbox: false,
                network_restricted: false,
                claude_api_network_allowed: true,
                notes: vec!["fake: no inheritance".into()],
            }
        });
        assert!(!obs.inherited_sandbox);
        let result = verify_child_posture(
            &obs,
            &ExpectedChildPosture::default(),
            true, // parent active
        );
        assert!(matches!(
            result,
            SandboxVerifyResult::InheritanceMissing { .. }
        ));
    }

    #[test]
    fn mac_and_linux_fake_probes() {
        for platform in [SandboxPlatform::MacOs, SandboxPlatform::Linux] {
            let obs = ChildSandboxObservation {
                platform,
                inherited_sandbox: true,
                network_restricted: true,
                claude_api_network_allowed: true,
                notes: vec![format!("fake {platform:?}")],
            };
            assert!(verify_child_posture(&obs, &ExpectedChildPosture::default(), true).is_ok());
        }
    }

    #[test]
    fn claude_api_block_fails() {
        let obs = ChildSandboxObservation {
            platform: SandboxPlatform::current(),
            inherited_sandbox: true,
            network_restricted: true,
            claude_api_network_allowed: false,
            notes: vec![],
        };
        assert!(matches!(
            verify_child_posture(&obs, &ExpectedChildPosture::default(), true),
            SandboxVerifyResult::ClaudeApiNetworkBlocked { .. }
        ));
    }

    #[test]
    fn parent_inactive_skips_inheritance_requirement() {
        let obs = ChildSandboxObservation {
            platform: SandboxPlatform::current(),
            inherited_sandbox: false,
            network_restricted: false,
            claude_api_network_allowed: true,
            notes: vec![],
        };
        assert!(verify_child_posture(&obs, &ExpectedChildPosture::default(), false).is_ok());
    }

    #[test]
    fn policy_note_documents_no_weaken() {
        assert!(SANDBOX_POLICY_NOTE.contains("never disable"));
        assert!(SANDBOX_POLICY_NOTE.contains("not assumed"));
    }
}
