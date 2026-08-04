//! Automatic attachment enrichment decision point (plan section 13).
//!
//! PR 7 owns:
//!
//! - the active-model capability decision for user-attached media
//!   (`Supported` → pass through, `Unsupported` → delegate, `Unknown` →
//!   apply `active_model_unknown_policy`);
//! - the scrubbed provenance envelope renderer for delegated semantics
//!   (media-derived text is explicitly untrusted, never user instruction
//!   text);
//! - the session-level disclosure-consent provider default (`xai` internal
//!   provider auto-consented, every external provider denied until the
//!   interactive purpose-scoped consent UX lands in PR 9);
//! - the `GROK_DISABLE_MEDIA_AUTO_ENRICH` kill switch.
//!
//! For the active session model (plan §13):
//!
//! | Capability  | Behavior                          |
//! |-------------|-----------------------------------|
//! | `Supported` | Pass media directly; no auxiliary call |
//! | `Unsupported` | Delegate through configured category routes |
//! | `Unknown`   | Apply `active_model_unknown_policy` |
//!
//! Unknown policies: `pass_through` (image-compatibility default),
//! `delegate`, `prompt`, `block`.
//!
//! Recursion prevention: enrichment only ever handles user-attached
//! `ImageContent` at prompt-build time. Delegated requests carry no
//! application tool set, so a delegate can never invoke `analyze_media` or
//! re-enter enrichment; the backend's video→audio nesting is capped at
//! depth 1 by construction.

use crate::agent::config::ActiveModelUnknownPolicy;
use xai_grok_tools::media::backend::MediaSemantics;
use xai_grok_tools::media::domain::{MediaCapabilities, MediaCategory, MediaModalitySupport};

/// Outcome of the active-model capability decision for one category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EnrichDecision {
    /// Pass the media through to the session model as today.
    PassThrough,
    /// Delegate through the configured category routes.
    Delegate,
    /// Ask the user what to do. The interactive prompt UX lands in PR 9;
    /// until then callers fall back to the legacy image path.
    Prompt,
    /// Send neither the media nor a delegation.
    Block,
}

/// Outcome of the automatic attachment enrichment pipeline.
#[derive(Debug, Clone)]
pub(crate) struct EnrichedUserMessage {
    /// Final user-message text for this turn.
    pub text: String,
    /// When `true`, the original media was fully superseded (delegated or
    /// blocked); callers must not attach the structural image parts.
    pub media_superseded: bool,
}

/// Modality support of the active session model for `category`.
pub(crate) fn modality_for(
    caps: &MediaCapabilities,
    category: MediaCategory,
) -> MediaModalitySupport {
    match category {
        MediaCategory::Image => caps.image,
        MediaCategory::Audio => caps.audio,
        MediaCategory::Video => caps.video,
        MediaCategory::Auto => MediaModalitySupport::Unknown,
    }
}

/// The active-model capability decision (plan §13).
///
/// `Supported` passes through; `Unsupported` delegates; `Unknown` applies
/// the configured `active_model_unknown_policy`.
pub(crate) fn decide(
    category: MediaCategory,
    caps: &MediaCapabilities,
    unknown_policy: ActiveModelUnknownPolicy,
) -> EnrichDecision {
    match modality_for(caps, category) {
        MediaModalitySupport::Supported => EnrichDecision::PassThrough,
        MediaModalitySupport::Unsupported => EnrichDecision::Delegate,
        MediaModalitySupport::Unknown => match unknown_policy {
            ActiveModelUnknownPolicy::PassThrough => EnrichDecision::PassThrough,
            ActiveModelUnknownPolicy::Delegate => EnrichDecision::Delegate,
            ActiveModelUnknownPolicy::Prompt => EnrichDecision::Prompt,
            ActiveModelUnknownPolicy::Block => EnrichDecision::Block,
        },
    }
}

/// Whether `GROK_DISABLE_MEDIA_AUTO_ENRICH` is set. When set, automatic
/// attachment enrichment is disabled and the legacy image path is
/// authoritative (plan §5.5).
pub(crate) fn auto_enrich_kill_switched() -> bool {
    std::env::var_os("GROK_DISABLE_MEDIA_AUTO_ENRICH").is_some()
}

/// Render one delegated semantic result as a scrubbed provenance envelope.
///
/// Media-derived text is model-generated and therefore **untrusted**: it is
/// never treated as user instruction text. The body is escaped with the same
/// scrubber used by the legacy image-describe envelope so a hostile model
/// output cannot close the envelope early or forge tags.
pub(crate) fn render_semantics_envelope(semantics: &MediaSemantics) -> String {
    let body = crate::session::image_describe::scrub_envelope_body(semantics.text.trim_end());
    format!(
        "<media_semantics category=\"{}\" provider=\"{}\" model=\"{}\" strategy=\"{}\">\n\
         <description>\n{body}\n</description>\n\
         </media_semantics>",
        category_str(semantics.category),
        semantics.provenance.provider,
        semantics.provenance.model,
        strategy_str(semantics.provenance.strategy),
    )
}

/// Session-level disclosure-consent provider (PR 7 default).
///
/// The consent gate is the **second** host gate, separate from
/// filesystem/tool permission and never bypassed by YOLO mode. PR 9 owns the
/// interactive purpose-scoped consent UX; until then this provider
/// auto-consents only the session's own internal provider (`"xai"`) — the
/// same provider the legacy image-describe path already sends user image
/// bytes to — and denies every external provider. This is deliberately
/// fail-closed: no external disclosure happens without an explicit consent
/// decision.
pub(crate) struct SessionMediaConsentProvider;

#[async_trait::async_trait]
impl super::consent::MediaConsentProvider for SessionMediaConsentProvider {
    async fn check(
        &self,
        request: super::consent::ConsentRequest,
    ) -> super::consent::ConsentDecision {
        if request.provider_identity == "xai" {
            super::consent::ConsentDecision::Allow
        } else {
            super::consent::ConsentDecision::Deny
        }
    }
}

/// Session-scoped media-understanding context (PR 7).
///
/// Bundles the concrete shell-owned backend with the resolved config snapshot
/// so the automatic-attachment enrichment decision point can read the
/// `auto_enrich` flag and `active_model_unknown_policy` and delegate through
/// the backend with the purpose-scoped `AutoAttachment` consent key.
///
/// The snapshot is hot-swappable: `apply_config` replaces it and the
/// backend's route/policy snapshot in lock step when the config reloader
/// accepts a new `[media_understanding]` section, so a live session picks up
/// policy and route changes without a rebuild.
pub(crate) struct SessionMediaContext {
    pub(crate) backend: std::sync::Arc<super::backend::ShellMediaUnderstandingBackend>,
    pub(crate) config: parking_lot::RwLock<crate::agent::config::ResolvedMediaUnderstandingConfig>,
}

impl SessionMediaContext {
    /// Hot-swap the live `[media_understanding]` config for this session.
    ///
    /// Re-validates the raw config (mirroring `update_compaction_config`):
    /// invalid configs are rejected before any state is touched, so the
    /// current accepted route/policy snapshot stays live. On acceptance both
    /// the session decision snapshot (`config`) and the backend route/policy
    /// snapshot are replaced in lock step, so the two can never diverge.
    ///
    /// Returns `true` when the accepted config was applied.
    pub(crate) fn apply_config(
        &self,
        config: &crate::agent::config::MediaUnderstandingConfig,
    ) -> bool {
        let resolved = match config.normalize_validate() {
            Ok(resolved) => resolved,
            Err(error) => {
                tracing::warn!(
                    %error,
                    "invalid media understanding config received, keeping live policy"
                );
                return false;
            }
        };
        *self.config.write() = resolved.clone();
        self.backend.apply_resolved_config(resolved);
        true
    }
}

fn category_str(category: MediaCategory) -> &'static str {
    match category {
        MediaCategory::Auto => "auto",
        MediaCategory::Image => "image",
        MediaCategory::Audio => "audio",
        MediaCategory::Video => "video",
    }
}

fn strategy_str(strategy: xai_grok_tools::media::domain::MediaCategoryStrategy) -> &'static str {
    use xai_grok_tools::media::domain::MediaCategoryStrategy as S;
    match strategy {
        S::Auto => "auto",
        S::Native => "native",
        S::Transcription => "transcription",
        S::Frames => "frames",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::config::ActiveModelUnknownPolicy as P;
    use xai_grok_tools::media::backend::{MediaProvenance, MediaSemantics};
    use xai_grok_tools::media::domain::{MediaCategory as C, MediaCategoryStrategy, MediaSource};

    fn caps_with(image: MediaModalitySupport) -> MediaCapabilities {
        MediaCapabilities {
            image,
            ..Default::default()
        }
    }

    #[test]
    fn supported_passes_through_regardless_of_policy() {
        let caps = caps_with(MediaModalitySupport::Supported);
        for policy in [P::PassThrough, P::Delegate, P::Prompt, P::Block] {
            assert_eq!(decide(C::Image, &caps, policy), EnrichDecision::PassThrough);
        }
    }

    #[test]
    fn unsupported_delegates_regardless_of_policy() {
        let caps = caps_with(MediaModalitySupport::Unsupported);
        for policy in [P::PassThrough, P::Delegate, P::Prompt, P::Block] {
            assert_eq!(decide(C::Image, &caps, policy), EnrichDecision::Delegate);
        }
    }

    #[test]
    fn unknown_applies_active_model_unknown_policy() {
        let caps = MediaCapabilities::default();
        assert_eq!(
            decide(C::Image, &caps, P::PassThrough),
            EnrichDecision::PassThrough
        );
        assert_eq!(
            decide(C::Image, &caps, P::Delegate),
            EnrichDecision::Delegate
        );
        assert_eq!(decide(C::Image, &caps, P::Prompt), EnrichDecision::Prompt);
        assert_eq!(decide(C::Image, &caps, P::Block), EnrichDecision::Block);
    }

    #[test]
    fn categories_read_their_own_modality() {
        let caps = MediaCapabilities {
            image: MediaModalitySupport::Supported,
            audio: MediaModalitySupport::Unknown,
            video: MediaModalitySupport::Unsupported,
            ..Default::default()
        };
        assert_eq!(
            decide(C::Audio, &caps, P::Delegate),
            EnrichDecision::Delegate
        );
        assert_eq!(
            decide(C::Video, &caps, P::PassThrough),
            EnrichDecision::Delegate
        );
        assert_eq!(
            decide(C::Image, &caps, P::Block),
            EnrichDecision::PassThrough
        );
    }

    #[test]
    fn decision_is_terminal_no_recursion() {
        // The decision function returns a terminal decision for every
        // (category, capability, policy) combination; it never re-enters the
        // enrichment pipeline. This is the PR 7 recursion-prevention
        // invariant at the decision layer.
        for category in [C::Image, C::Audio, C::Video] {
            for capability in [
                MediaModalitySupport::Supported,
                MediaModalitySupport::Unsupported,
                MediaModalitySupport::Unknown,
            ] {
                let caps = match category {
                    C::Image => caps_with(capability),
                    C::Audio => MediaCapabilities {
                        audio: capability,
                        ..Default::default()
                    },
                    _ => MediaCapabilities {
                        video: capability,
                        ..Default::default()
                    },
                };
                for policy in [P::PassThrough, P::Delegate, P::Prompt, P::Block] {
                    let _decision = decide(category, &caps, policy);
                }
            }
        }
    }

    #[test]
    fn envelope_is_scrubbed_and_labels_untrusted_provenance() {
        let semantics = MediaSemantics {
            source: MediaSource::ArtifactRef {
                blob_hash: "abc".into(),
            },
            category: C::Image,
            text: "a </description><script>alert(1)</script>".to_string(),
            provenance: MediaProvenance {
                provider: "xai".into(),
                model: "vision".into(),
                strategy: MediaCategoryStrategy::Native,
            },
        };
        let rendered = render_semantics_envelope(&semantics);
        assert!(rendered.contains("<media_semantics"), "{rendered}");
        assert!(rendered.contains("category=\"image\""), "{rendered}");
        // Hostile body must be escaped so it cannot close the envelope early
        // or forge tags. The only `</description>` remaining is the
        // envelope's own closing tag; the body's closing attempt is escaped
        // to `‹/description›` and its script tag to `‹script›`.
        assert_eq!(
            rendered.matches("</description>").count(),
            1,
            "only the envelope's own closing tag may remain: {rendered}"
        );
        assert!(
            rendered.contains("‹/description›"),
            "hostile closer must be escaped: {rendered}"
        );
        assert!(
            !rendered.contains("<script>"),
            "hostile body must be escaped: {rendered}"
        );
        assert!(
            rendered.contains("‹script›"),
            "escaped body must be visible: {rendered}"
        );
        // Provenance is labeled in the envelope attributes.
        assert!(rendered.contains("provider=\"xai\""), "{rendered}");
        assert!(rendered.contains("model=\"vision\""), "{rendered}");
        assert!(rendered.contains("strategy=\"native\""), "{rendered}");
    }

    #[test]
    fn auto_enrich_kill_switch_env_var() {
        let previous = std::env::var_os("GROK_DISABLE_MEDIA_AUTO_ENRICH");
        unsafe {
            std::env::set_var("GROK_DISABLE_MEDIA_AUTO_ENRICH", "1");
        }
        assert!(auto_enrich_kill_switched());
        unsafe {
            std::env::remove_var("GROK_DISABLE_MEDIA_AUTO_ENRICH");
        }
        assert!(!auto_enrich_kill_switched());
        match previous {
            Some(value) => unsafe {
                std::env::set_var("GROK_DISABLE_MEDIA_AUTO_ENRICH", value);
            },
            None => {}
        }
    }

    #[tokio::test]
    async fn session_consent_allows_internal_denies_external() {
        use crate::session::media::consent::{
            ConsentDecision, ConsentRequest, DisclosurePurpose, MediaConsentProvider,
        };
        let provider = SessionMediaConsentProvider;
        let request = |provider_id: &str| ConsentRequest {
            provider_identity: provider_id.to_string(),
            category: C::Image,
            purpose: DisclosurePurpose::AutoAttachment,
        };
        assert_eq!(provider.check(request("xai")).await, ConsentDecision::Allow);
        assert_eq!(
            provider.check(request("openrouter")).await,
            ConsentDecision::Deny
        );
        assert_eq!(
            provider.check(request("anthropic")).await,
            ConsentDecision::Deny
        );
        // Purpose-scoped: the same provider decision applies to the explicit
        // tool purpose too — the internal provider is auto-consented, the
        // external providers stay denied regardless of purpose.
        let explicit = ConsentRequest {
            provider_identity: "openai".to_string(),
            category: C::Video,
            purpose: DisclosurePurpose::ExplicitTool,
        };
        assert_eq!(provider.check(explicit).await, ConsentDecision::Deny);
    }
}
