//! Zero Data Retention (ZDR) gate (plan sections 7.2 and 17).
//!
//! The ZDR gate **fails closed from trusted metadata only**: the session's
//! `GrokAuth` (`is_zdr_team()`, which derives from the server-issued
//! `team_blocked_reasons` list) and the provider identity derived from the
//! resolved catalog entry. A user-authored allowlist is rejected by design —
//! the plan's invariant is that a ZDR team's media never leaves the first
//! party, so no user/config flag may re-enable an external route.
//!
//! A route is ZDR-eligible when either:
//! - the target provider is first-party xAI (data stays inside the
//!   ZDR-protected environment), or
//! - the session auth is not a ZDR team (no ZDR constraint applies).

use crate::auth::GrokAuth;
use xai_grok_inference::config::ProviderIdentity;

/// Whether a route targeting `provider_identity` may run under the session's
/// ZDR posture.
///
/// `auth == None` (no session credential) is treated as non-ZDR: without an
/// account there is no ZDR team metadata to honor, and the route still has to
/// pass the consent and permission gates before any bytes leave.
pub(crate) fn zdr_route_eligible(
    provider_identity: ProviderIdentity,
    auth: Option<&GrokAuth>,
) -> bool {
    if provider_identity.is_first_party() {
        return true;
    }
    match auth {
        Some(auth) => !auth.is_zdr_team(),
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zdr_auth() -> GrokAuth {
        // The block-reason constants are private to `auth::model`; use the
        // literal wire values they compare against.
        GrokAuth {
            team_blocked_reasons: vec!["BLOCKED_REASON_NO_LOGS".to_string()],
            ..GrokAuth::test_default()
        }
    }

    fn non_zdr_auth() -> GrokAuth {
        GrokAuth::test_default()
    }

    #[test]
    fn media_zdr_first_party_always_eligible() {
        assert!(zdr_route_eligible(ProviderIdentity::Xai, Some(&zdr_auth())));
        assert!(zdr_route_eligible(
            ProviderIdentity::Xai,
            Some(&non_zdr_auth())
        ));
        assert!(zdr_route_eligible(ProviderIdentity::Xai, None));
    }

    #[test]
    fn media_zdr_fails_closed_for_external_providers() {
        for identity in [
            ProviderIdentity::OpenAi,
            ProviderIdentity::OpenRouter,
            ProviderIdentity::Anthropic,
            ProviderIdentity::Custom,
        ] {
            assert!(
                !zdr_route_eligible(identity, Some(&zdr_auth())),
                "external provider {identity:?} must be ZDR-ineligible"
            );
        }
    }

    #[test]
    fn media_zdr_non_zdr_team_and_no_auth_allow_external() {
        for identity in [
            ProviderIdentity::OpenAi,
            ProviderIdentity::OpenRouter,
            ProviderIdentity::Anthropic,
            ProviderIdentity::Custom,
        ] {
            assert!(zdr_route_eligible(identity, Some(&non_zdr_auth())));
            assert!(zdr_route_eligible(identity, None));
        }
    }

    #[test]
    fn media_zdr_opt_out_retention_is_not_zdr() {
        // `coding_data_retention_opt_out` is a data-collection preference,
        // not a ZDR team flag; it must not lock the route list to first-party
        // providers.
        let opted_out = GrokAuth {
            coding_data_retention_opt_out: true,
            ..GrokAuth::test_default()
        };
        assert!(!opted_out.is_zdr_team());
        assert!(zdr_route_eligible(
            ProviderIdentity::OpenRouter,
            Some(&opted_out)
        ));
    }
}
