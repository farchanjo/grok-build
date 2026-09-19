//! Subscription-tier classification shared across the shell and the pager.
//!
//! The subscription tier reaches the client as a free-form **display-name
//! string** (from CCP `/settings` `subscription_tier_display`, or the numeric
//! JWT `tier` claim mapped to a display-style string by
//! [`crate::agent::mvp_agent::jwt_tier_claim`]). There is no shared enum, so
//! gating decisions classify the string here in ONE place so the pager's
//! cosmetic slash-command gate and the shell's capability (toolset) gate can't
//! drift apart.
//!
//! "Restricted" tiers are the personal free tier and X Basic — the tiers the
//! server zero-limits on the Imagine and voice endpoints. Everything else
//! (SuperGrok, SuperGrok Heavy/Lite, X Premium/+, and any unknown future name)
//! is unrestricted (**fail-open**).

/// Env override that disables the tier-restricted surface entirely.
///
/// Set to a truthy value (`1` / `true` / `on`) to treat **every** tier as
/// unrestricted, so the pager stops hiding `/usage`, `/imagine`,
/// `/imagine-video` and `/voice`, and the shell stops marking Imagine as
/// tier-restricted. Set to a falsy value to force the stock classification
/// back on; unset (or a typo) falls through to the classification below.
///
/// This exists for deployments that run against a non-xAI inference endpoint
/// (BYOK) while still authenticating with an xAI OAuth2 session: the tier
/// string then arrives as `Free`/`X Basic` from a subscription the endpoint
/// never bills, and the client-side gate — which the module doc already calls
/// a UX optimization, not a security boundary — withholds features the server
/// would happily serve. Prefer [`is_restricted_tier_name`] over reading the
/// env directly so the pager and shell stay in lockstep.
pub const TIER_RESTRICTIONS_DISABLED_ENV: &str = "GROK_DISABLE_TIER_RESTRICTIONS";

/// Whether the tier-restricted surface is disabled by the env override.
fn tier_restrictions_disabled() -> bool {
    xai_grok_config::env_bool(TIER_RESTRICTIONS_DISABLED_ENV).unwrap_or(false)
}

/// Whether a **known** subscription-tier display name is a gated tier: the free
/// tier (CCP display "Free" or an empty string) or X Basic (CCP display
/// "X Basic"; JWT-claim fallback spelling "x_basic").
///
/// Case-insensitive and whitespace-trimmed. Callers decide the policy for an
/// *absent* tier (`None`): the pager treats absence as restricted (cosmetic,
/// recovers live on the next settings update), while the shell treats absence as
/// unrestricted (fail-open — the server authoritatively enforces per-tier
/// limits, so never withhold a capability on a guess).
///
/// Returns `false` unconditionally when [`TIER_RESTRICTIONS_DISABLED_ENV`] is
/// truthy.
pub fn is_restricted_tier_name(tier: &str) -> bool {
    if tier_restrictions_disabled() {
        return false;
    }
    let t = tier.trim().to_ascii_lowercase();
    t.is_empty() || t == "free" || t == "x basic" || t == "x_basic"
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the env override off: a shell that exports
    /// [`TIER_RESTRICTIONS_DISABLED_ENV`] would otherwise fail a test about the
    /// *stock* classification.
    #[test]
    fn restricted_names() {
        let _guard = EnvGuard::set(TIER_RESTRICTIONS_DISABLED_ENV, None);
        assert!(is_restricted_tier_name(""));
        assert!(is_restricted_tier_name("   "));
        assert!(is_restricted_tier_name("Free"));
        assert!(is_restricted_tier_name("free"));
        assert!(is_restricted_tier_name("X Basic"));
        assert!(is_restricted_tier_name("x_basic"));
        assert!(is_restricted_tier_name("  X BASIC  "));
    }

    #[test]
    fn unrestricted_names() {
        assert!(!is_restricted_tier_name("SuperGrok"));
        assert!(!is_restricted_tier_name("SuperGrok Heavy"));
        assert!(!is_restricted_tier_name("supergrok_lite"));
        assert!(!is_restricted_tier_name("X Premium"));
        assert!(!is_restricted_tier_name("x_premium_plus"));
        // API keys are not free-tier gated.
        assert!(!is_restricted_tier_name("api_key"));
        assert!(!is_restricted_tier_name("API Key"));
        // Unknown future tiers fail open.
        assert!(!is_restricted_tier_name("some_new_plan"));
    }

    /// The env override is process-global, so this test owns the variable for
    /// its duration and restores the prior value.
    #[test]
    fn env_override_disables_the_gate_for_every_tier() {
        let _guard = EnvGuard::set(TIER_RESTRICTIONS_DISABLED_ENV, Some("1"));
        // Every string that is otherwise restricted now passes.
        assert!(!is_restricted_tier_name(""));
        assert!(!is_restricted_tier_name("   "));
        assert!(!is_restricted_tier_name("Free"));
        assert!(!is_restricted_tier_name("X Basic"));
        assert!(!is_restricted_tier_name("x_basic"));
        // ...and the already-unrestricted ones are unaffected.
        assert!(!is_restricted_tier_name("SuperGrok"));

        let _off = EnvGuard::set(TIER_RESTRICTIONS_DISABLED_ENV, Some("0"));
        assert!(is_restricted_tier_name("Free"), "falsy restores the gate");

        let _typo = EnvGuard::set(TIER_RESTRICTIONS_DISABLED_ENV, Some("maybe"));
        assert!(
            is_restricted_tier_name("Free"),
            "an unrecognized value falls through to the stock classification"
        );
    }

    /// Restores the previous value of `name` on drop.
    struct EnvGuard {
        name: &'static str,
        previous: Option<String>,
    }

    impl EnvGuard {
        fn set(name: &'static str, value: Option<&str>) -> Self {
            let previous = std::env::var(name).ok();
            // SAFETY: single-threaded test holding the guard; the repo mutates
            // env in tests through the same std API.
            unsafe {
                match value {
                    Some(v) => std::env::set_var(name, v),
                    None => std::env::remove_var(name),
                }
            }
            Self { name, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            // SAFETY: see [`EnvGuard::set`].
            unsafe {
                match self.previous.as_deref() {
                    Some(v) => std::env::set_var(self.name, v),
                    None => std::env::remove_var(self.name),
                }
            }
        }
    }
}
