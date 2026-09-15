//! Shared gate for the xAI-only extension surfaces.
//!
//! `/share`, `/billing`, `/usage` and the `x.ai/cloud/*` handlers are served by
//! grok.com: they need an xAI account, not merely *a* bearer. With
//! `GROK_XAI_ENABLED=0`, or with only a third-party credential, every one of
//! them must refuse with the same one-line sentence and leave the session
//! usable — instead of each command inventing its own wording, or leaking a raw
//! error from a request that should never have been sent.

use agent_client_protocol as acp;

use crate::auth::{AuthManager, GrokAuth};

/// An extension surface that only a grok.com session can serve.
///
/// The variant is the *named reason* returned to the caller: it names the
/// command the user typed, so the refusal is actionable without the user having
/// to guess which of the xAI-only surfaces they hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum XaiSurface {
    /// `/share` (share links are built on grok.com).
    Share,
    /// `/billing` (credits and usage come from the backend).
    Billing,
    /// `/usage` (auto top-up rule; the credit bar polls the same endpoint).
    Usage,
    /// `x.ai/cloud/*` (cloud sandboxes run on xAI infrastructure).
    Cloud,
}

impl XaiSurface {
    /// How the surface is named in the refusal.
    const fn label(self) -> &'static str {
        match self {
            Self::Share => "`/share`",
            Self::Billing => "`/billing`",
            Self::Usage => "`/usage`",
            Self::Cloud => "Cloud sandboxes",
        }
    }

    /// The single actionable sentence every xAI-only surface returns.
    ///
    /// Deliberately identical in shape for every caller and for both refusal
    /// reasons (no credential at all, or a non-xAI credential): the remedy is
    /// the same either way, and one wording is what makes the off-xAI failure
    /// mode recognizable instead of a per-command surprise.
    pub(crate) fn message(self) -> String {
        // Every surface but `Cloud` is named by a single command, so the verb
        // agrees with the label rather than the whole sentence being spelled
        // out four times.
        let verb = if matches!(self, Self::Cloud) {
            "need"
        } else {
            "needs"
        };
        format!(
            "{} {verb} a grok.com session: connect xAI in /providers to authenticate.",
            self.label()
        )
    }
}

/// Require xAI auth from a sync context, accepting tokens in the client-side
/// buffer window.
///
/// Refuses with [`acp::Error::auth_required`] (a recoverable, named refusal —
/// not an internal error) so the pager keeps the session and shows
/// [`XaiSurface::message`] as the one line the user needs.
pub(crate) fn require_xai_auth(
    auth_manager: &AuthManager,
    surface: XaiSurface,
) -> Result<GrokAuth, acp::Error> {
    let auth = auth_manager
        .current_or_expired()
        .ok_or_else(|| acp::Error::auth_required().data(surface.message()))?;
    if !auth.is_xai_auth() {
        return Err(acp::Error::auth_required().data(surface.message()));
    }
    Ok(auth)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{AuthMode, GrokAuth};
    use chrono::Utc;
    use std::sync::Arc;
    use tempfile::tempdir;

    fn auth_manager() -> (Arc<AuthManager>, tempfile::TempDir) {
        let dir = tempdir().expect("tempdir for auth gate test");
        let mgr = Arc::new(AuthManager::new(
            dir.path(),
            crate::auth::GrokComConfig::default(),
        ));
        (mgr, dir)
    }

    fn non_xai_auth() -> GrokAuth {
        GrokAuth {
            auth_mode: AuthMode::ApiKey,
            key: "plain-bearer".into(),
            create_time: Utc::now(),
            ..Default::default()
        }
    }

    fn data_of(err: &acp::Error) -> String {
        serde_json::to_value(err)
            .expect("acp::Error serializes to JSON-RPC shape")
            .get("data")
            .and_then(|v| v.as_str())
            .expect("auth_required error carries a data string")
            .to_string()
    }

    /// Every surface must refuse with the same sentence shape, so a user who hits
    /// two of them in a row is not taught two different stories.
    #[test]
    fn every_surface_shares_one_message_shape() {
        for (surface, subject) in [
            (XaiSurface::Share, "`/share` needs"),
            (XaiSurface::Billing, "`/billing` needs"),
            (XaiSurface::Usage, "`/usage` needs"),
            (XaiSurface::Cloud, "Cloud sandboxes need"),
        ] {
            assert_eq!(
                surface.message(),
                format!("{subject} a grok.com session: connect xAI in /providers to authenticate."),
                "{surface:?} must name the missing session and the remedy"
            );
            assert!(
                !surface.message().contains('\n'),
                "{surface:?} must stay on one line"
            );
        }
    }

    #[test]
    fn missing_credential_refusal_names_the_surface() {
        let (mgr, _dir) = auth_manager();
        let err = require_xai_auth(&mgr, XaiSurface::Share).expect_err("no credential at all");
        assert_eq!(
            data_of(&err),
            "`/share` needs a grok.com session: connect xAI in /providers to authenticate."
        );
    }

    #[test]
    fn non_xai_credential_refusal_is_the_same_sentence() {
        let (mgr, _dir) = auth_manager();
        mgr.hot_swap(non_xai_auth());
        let err = require_xai_auth(&mgr, XaiSurface::Billing).expect_err("plain bearer only");
        assert_eq!(
            data_of(&err),
            "`/billing` needs a grok.com session: connect xAI in /providers to authenticate."
        );
    }

    /// A refusal must not consume or mutate the credential: the same manager
    /// still resolves the very same token, so a later command (or a login) sees
    /// the session intact.
    #[test]
    fn refusal_leaves_the_credential_untouched() {
        let (mgr, _dir) = auth_manager();
        let auth = non_xai_auth();
        let key = auth.key.clone();
        mgr.hot_swap(auth);

        require_xai_auth(&mgr, XaiSurface::Cloud).expect_err("plain bearer only");

        let after = mgr
            .current_or_expired()
            .expect("credential survives the refusal");
        assert_eq!(after.key, key);
        assert_eq!(after.auth_mode, AuthMode::ApiKey);
        assert_eq!(
            require_xai_auth(&mgr, XaiSurface::Cloud)
                .expect_err("still refused")
                .code,
            acp::Error::auth_required().code,
        );
    }

    #[test]
    fn xai_session_passes_and_is_returned() {
        let (mgr, _dir) = auth_manager();
        mgr.hot_swap(GrokAuth {
            auth_mode: AuthMode::Oidc,
            oidc_issuer: Some("https://auth.x.ai".to_string()),
            key: "session-key".into(),
            create_time: Utc::now(),
            ..Default::default()
        });
        let auth = require_xai_auth(&mgr, XaiSurface::Usage).expect("xAI session is accepted");
        assert_eq!(auth.key, "session-key");
    }
}
