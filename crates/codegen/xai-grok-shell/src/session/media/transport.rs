//! Provider transport planner (plan section 9).
//!
//! The cardinal invariant is: **a concrete transport must be known before any
//! bytes leave the host**. A route is wire-eligible only when the provider's
//! [`MediaTransportCapabilities`] advertise a path the current implementation
//! can actually produce.
//!
//! Today `ContentPart` supports only `Text` and `Image`, so the concrete wire
//! paths are:
//!
//! - **Image** (`native`/`auto`): the normalized bytes travel as a
//!   `data:<mime>;base64,...` URL in `ContentPart::Image` — requires
//!   `image_inline`.
//! - **Video `frames`**: deterministic frames/contact sheet re-encoded as
//!   images, sent via `ContentPart::Image` — requires `image_inline`.
//!
//! Native audio, transcription endpoints, and native video require additive
//! inference/provider adapters. Until those adapters exist the routes are
//! skipped **without sending bytes** (plan section 9 and hard blocker 4), even
//! when the provider advertises the corresponding capability flag.
//!
//! `Auto` picks the first concretely supported option for the category.

use xai_grok_tools::media::domain::{
    MediaCategory, MediaCategoryStrategy, MediaTransportCapabilities,
};

/// Concrete wire plan for a delegate request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum TransportPlan {
    /// Image bytes travel as a `data:<mime>;base64,...` URL in
    /// `ContentPart::Image`.
    ImageDataUrl,
    /// Provider-native audio input (inline or upload). Requires the additive
    /// audio adapter; until then routes using it are skipped.
    AudioNative,
    /// Provider transcription endpoint. Requires the transcription adapter;
    /// until then routes using it are skipped.
    AudioTranscription,
    /// Provider-native video input. Requires the additive video adapter;
    /// until then routes using it are skipped.
    VideoNative,
    /// Deterministic frame extraction; frames are re-encoded as images and
    /// sent via `ContentPart::Image`.
    VideoFrames,
}

/// Whether the current implementation has a native-audio wire adapter.
///
/// `ContentPart` only supports `Text` and `Image`, so this is `false` today;
/// routes that require native audio are skipped without sending bytes until
/// additive inference/provider adapters land (plan section 9).
const NATIVE_AUDIO_ADAPTER: bool = false;

/// Whether the current implementation has a transcription-endpoint adapter.
///
/// There is no transcription endpoint client today, so this is `false`;
/// routes that require transcription are skipped without sending bytes.
const TRANSCRIPTION_ENDPOINT_ADAPTER: bool = false;

/// Whether the current implementation has a native-video wire adapter.
const NATIVE_VIDEO_ADAPTER: bool = false;

/// Whether `strategy` for `category` has a concrete wire path given `caps`.
///
/// Both halves of plan section 6.2 must hold: the strategy must be producible
/// by this implementation (the adapter constants above) AND the provider must
/// advertise the matching transport capability.
pub(crate) fn route_is_transport_eligible(
    category: MediaCategory,
    strategy: MediaCategoryStrategy,
    caps: &MediaTransportCapabilities,
) -> bool {
    use MediaCategory as C;
    use MediaCategoryStrategy as S;
    match (category, strategy) {
        (C::Image, S::Auto | S::Native) => caps.image_inline,
        (C::Audio, S::Auto) => concrete_strategy_for_auto(C::Audio, caps).is_some(),
        (C::Audio, S::Native) => NATIVE_AUDIO_ADAPTER && (caps.audio_inline || caps.audio_upload),
        (C::Audio, S::Transcription) => {
            TRANSCRIPTION_ENDPOINT_ADAPTER && caps.transcription_endpoint
        }
        (C::Video, S::Auto) => concrete_strategy_for_auto(C::Video, caps).is_some(),
        (C::Video, S::Native) => {
            NATIVE_VIDEO_ADAPTER && (caps.native_video || caps.video_inline || caps.video_upload)
        }
        (C::Video, S::Frames) => caps.image_inline,
        _ => false,
    }
}

/// Pick the first concretely supported strategy for an `Auto` route.
///
/// Image `auto` resolves to native when `image_inline` is advertised; audio
/// `auto` prefers transcription then native; video `auto` prefers native then
/// frames. Returns `None` when no strategy has a concrete wire path, which
/// makes the route (and therefore the category) ineligible.
pub(crate) fn concrete_strategy_for_auto(
    category: MediaCategory,
    caps: &MediaTransportCapabilities,
) -> Option<MediaCategoryStrategy> {
    use MediaCategory as C;
    use MediaCategoryStrategy as S;
    match category {
        C::Image => caps.image_inline.then_some(S::Native),
        C::Audio => [S::Transcription, S::Native]
            .into_iter()
            .find(|strategy| route_is_transport_eligible(C::Audio, *strategy, caps)),
        C::Video => [S::Native, S::Frames]
            .into_iter()
            .find(|strategy| route_is_transport_eligible(C::Video, *strategy, caps)),
        C::Auto => None,
    }
}

/// The concrete wire plan for a route, if one exists.
///
/// `strategy` must already be a concrete (non-`Auto`) strategy; callers that
/// need `Auto` resolution use [`concrete_strategy_for_auto`] first.
pub(crate) fn transport_plan_for(
    category: MediaCategory,
    strategy: MediaCategoryStrategy,
    caps: &MediaTransportCapabilities,
) -> Option<TransportPlan> {
    use MediaCategory as C;
    use MediaCategoryStrategy as S;
    if !route_is_transport_eligible(category, strategy, caps) {
        return None;
    }
    match (category, strategy) {
        (C::Image, S::Native) | (C::Image, S::Auto) => Some(TransportPlan::ImageDataUrl),
        (C::Audio, S::Native) | (C::Audio, S::Auto) => Some(TransportPlan::AudioNative),
        (C::Audio, S::Transcription) => Some(TransportPlan::AudioTranscription),
        (C::Video, S::Native) | (C::Video, S::Auto) => Some(TransportPlan::VideoNative),
        (C::Video, S::Frames) => Some(TransportPlan::VideoFrames),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use MediaCategory as C;
    use MediaCategoryStrategy as S;

    fn caps(
        image_inline: bool,
        audio_inline: bool,
        audio_upload: bool,
        transcription: bool,
    ) -> MediaTransportCapabilities {
        MediaTransportCapabilities {
            image_inline,
            audio_inline,
            audio_upload,
            transcription_endpoint: transcription,
            ..MediaTransportCapabilities::default()
        }
    }

    fn full_transport_caps() -> MediaTransportCapabilities {
        MediaTransportCapabilities {
            image_inline: true,
            audio_inline: true,
            audio_upload: true,
            transcription_endpoint: true,
            video_inline: true,
            video_upload: true,
            native_video: true,
            json_schema: true,
        }
    }

    #[test]
    fn media_transport_image_requires_inline() {
        let none = MediaTransportCapabilities::default();
        assert!(!route_is_transport_eligible(C::Image, S::Native, &none));
        assert!(!route_is_transport_eligible(C::Image, S::Auto, &none));
        let inline = caps(true, false, false, false);
        assert!(route_is_transport_eligible(C::Image, S::Native, &inline));
        assert!(route_is_transport_eligible(C::Image, S::Auto, &inline));
        assert_eq!(
            transport_plan_for(C::Image, S::Native, &inline),
            Some(TransportPlan::ImageDataUrl)
        );
    }

    #[test]
    fn media_transport_audio_has_no_concrete_path_yet() {
        // Even with every provider flag advertised, the additive adapter does
        // not exist yet, so the route must be skipped without sending bytes.
        let full = full_transport_caps();
        assert!(!route_is_transport_eligible(C::Audio, S::Native, &full));
        assert!(!route_is_transport_eligible(
            C::Audio,
            S::Transcription,
            &full
        ));
        assert!(concrete_strategy_for_auto(C::Audio, &full).is_none());
        assert!(transport_plan_for(C::Audio, S::Native, &full).is_none());
    }

    #[test]
    fn media_transport_video_frames_requires_image_inline() {
        let none = MediaTransportCapabilities::default();
        assert!(!route_is_transport_eligible(C::Video, S::Frames, &none));
        assert!(concrete_strategy_for_auto(C::Video, &none).is_none());

        let image_only = caps(true, false, false, false);
        assert!(route_is_transport_eligible(
            C::Video,
            S::Frames,
            &image_only
        ));
        assert_eq!(
            concrete_strategy_for_auto(C::Video, &image_only),
            Some(S::Frames),
            "native video has no adapter, so auto resolves to frames"
        );
        assert_eq!(
            transport_plan_for(C::Video, S::Frames, &image_only),
            Some(TransportPlan::VideoFrames)
        );
    }

    #[test]
    fn media_transport_native_video_is_not_concrete_yet() {
        let full = full_transport_caps();
        assert!(!route_is_transport_eligible(C::Video, S::Native, &full));
        assert_eq!(
            concrete_strategy_for_auto(C::Video, &full),
            Some(S::Frames),
            "auto must not pick native video while the adapter is absent"
        );
    }

    #[test]
    fn media_transport_auto_audio_prefers_concrete_only() {
        let none = MediaTransportCapabilities::default();
        assert!(concrete_strategy_for_auto(C::Audio, &none).is_none());
        // With transcription advertised but no adapter, still none.
        let transcription_only = caps(false, false, false, true);
        assert!(concrete_strategy_for_auto(C::Audio, &transcription_only).is_none());
    }

    #[test]
    fn media_transport_auto_image_and_video_resolve() {
        let image = caps(true, false, false, false);
        assert_eq!(
            concrete_strategy_for_auto(C::Image, &image),
            Some(S::Native)
        );
        assert_eq!(
            concrete_strategy_for_auto(C::Video, &image),
            Some(S::Frames)
        );
    }

    #[test]
    fn media_transport_auto_category_has_no_concrete_option() {
        let full = full_transport_caps();
        assert!(concrete_strategy_for_auto(C::Auto, &full).is_none());
        assert!(!route_is_transport_eligible(C::Auto, S::Auto, &full));
    }
}
