//! Inference-free media-understanding domain vocabulary.
//!
//! This module must remain inference-free: it may only use the tool crate's
//! existing dependencies (`serde`, `serde_json`, `schemars`, `strum`,
//! `thiserror`, `async-trait`) and must never import from
//! `xai-grok-inference*`. It is the shared seam established by PR 1 so the
//! shell-owned backend (PR 6) and the deferred `analyze_media` tool (PR 7)
//! can interoperate without creating a tools-to-inference dependency cycle.

use serde::{Deserialize, Serialize};

/// Media category an analysis request targets.
///
/// `Auto` defers to the backend's content sniffing; concrete categories
/// select the configured route list for that category. Defaults to `Auto`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum MediaCategory {
    #[default]
    Auto,
    Image,
    Audio,
    Video,
}

impl MediaCategory {
    /// The concrete categories that carry configured route lists, in a stable
    /// order. `Auto` is deliberately excluded: it is a request-time hint, not
    /// a configurable route category.
    pub const CONCRETE: [MediaCategory; 3] = [
        MediaCategory::Image,
        MediaCategory::Audio,
        MediaCategory::Video,
    ];
}

/// Tri-state modality support for one media category.
///
/// `Unknown` is the serde default so older persisted or remote metadata that
/// predates modality tracking deserializes without error.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum MediaModalitySupport {
    /// The model's capability for this modality is not known.
    #[default]
    Unknown,
    /// The model natively supports this modality.
    Supported,
    /// The model does not natively support this modality.
    Unsupported,
}

/// Per-category modality support for a model.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MediaCapabilities {
    #[serde(default)]
    pub image: MediaModalitySupport,
    #[serde(default)]
    pub audio: MediaModalitySupport,
    #[serde(default)]
    pub video: MediaModalitySupport,
}

/// Concrete wire-transport capability flags for a provider.
///
/// Transport capability is tracked separately from semantic capability:
/// runtime eligibility requires both a semantic route-policy verdict and a
/// concrete provider transport that is known before any bytes leave.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
pub struct MediaTransportCapabilities {
    #[serde(default)]
    pub image_inline: bool,
    #[serde(default)]
    pub audio_inline: bool,
    #[serde(default)]
    pub audio_upload: bool,
    #[serde(default)]
    pub transcription_endpoint: bool,
    #[serde(default)]
    pub video_inline: bool,
    #[serde(default)]
    pub video_upload: bool,
    #[serde(default)]
    pub native_video: bool,
    #[serde(default)]
    pub json_schema: bool,
}

/// Source of one media item in an analysis request.
///
/// Only workspace-relative paths and current-session artifact references are
/// expressible. URLs cannot be represented, which removes the initial SSRF
/// surface by construction.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum MediaSource {
    /// Workspace-relative path, resolved only after permission approval.
    Path { path: String },
    /// BLAKE3-addressed artifact blob in the current session store
    /// (`artifact://blob/<blake3-hex>`).
    ArtifactRef { blob_hash: String },
}

/// Requested detail level for media semantics.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum MediaDetailLevel {
    Low,
    #[default]
    Medium,
    High,
}

/// Preprocessing/delegate strategy for a configured media route.
///
/// Strategies are category-specific; [`MediaCategoryStrategy::allowed_for`]
/// validates combinations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MediaCategoryStrategy {
    /// Resolve automatically: native when concretely available, otherwise the
    /// category's fallback strategy.
    Auto,
    /// Native provider media input for the category.
    Native,
    /// Provider transcription endpoint (audio only).
    Transcription,
    /// Deterministic frames/contact sheet extraction (video only).
    Frames,
}

impl MediaCategoryStrategy {
    /// Whether this strategy may be configured for `category`.
    pub fn allowed_for(self, category: MediaCategory) -> bool {
        use MediaCategory as C;
        use MediaCategoryStrategy as S;
        match (category, self) {
            (C::Image, S::Auto | S::Native) => true,
            (C::Audio, S::Auto | S::Native | S::Transcription) => true,
            (C::Video, S::Auto | S::Native | S::Frames) => true,
            _ => false,
        }
    }
}

/// Lightweight route metadata surfaced by an availability snapshot.
///
/// Carries no credentials, prompts, or provider secrets: only the catalog
/// model ID, its strategy, and resolution eligibility.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MediaRouteMetadata {
    /// Catalog model ID of the configured delegate route.
    pub model_id: String,
    /// Strategy for this route within its category.
    pub strategy: MediaCategoryStrategy,
    /// Whether the catalog currently resolves `model_id`.
    #[serde(default)]
    pub unresolved: bool,
    /// Whether the route is eligible given semantic + transport metadata.
    #[serde(default)]
    pub eligible: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_round_trips_snake_case() {
        assert_eq!(
            serde_json::to_value(MediaCategory::Image).unwrap(),
            serde_json::json!("image")
        );
        assert_eq!(
            serde_json::to_value(MediaCategory::Audio).unwrap(),
            serde_json::json!("audio")
        );
        assert_eq!(
            serde_json::to_value(MediaCategory::Video).unwrap(),
            serde_json::json!("video")
        );
        assert_eq!(
            serde_json::to_value(MediaCategory::Auto).unwrap(),
            serde_json::json!("auto")
        );
        let back: MediaCategory = serde_json::from_value(serde_json::json!("video")).unwrap();
        assert_eq!(back, MediaCategory::Video);
    }

    #[test]
    fn modality_support_defaults_to_unknown() {
        let caps: MediaCapabilities = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(caps.image, MediaModalitySupport::Unknown);
        assert_eq!(caps.audio, MediaModalitySupport::Unknown);
        assert_eq!(caps.video, MediaModalitySupport::Unknown);
    }

    #[test]
    fn modality_support_round_trips_snake_case() {
        assert_eq!(
            serde_json::to_value(MediaModalitySupport::Supported).unwrap(),
            serde_json::json!("supported")
        );
        assert_eq!(
            serde_json::to_value(MediaModalitySupport::Unsupported).unwrap(),
            serde_json::json!("unsupported")
        );
        assert_eq!(
            serde_json::to_value(MediaModalitySupport::Unknown).unwrap(),
            serde_json::json!("unknown")
        );
        let back: MediaModalitySupport =
            serde_json::from_value(serde_json::json!("unknown")).unwrap();
        assert_eq!(back, MediaModalitySupport::Unknown);
    }

    #[test]
    fn capabilities_round_trip() {
        let caps = MediaCapabilities {
            image: MediaModalitySupport::Supported,
            audio: MediaModalitySupport::Unknown,
            video: MediaModalitySupport::Unsupported,
        };
        let json = serde_json::to_value(&caps).unwrap();
        assert_eq!(json["image"], "supported");
        assert_eq!(json["audio"], "unknown");
        assert_eq!(json["video"], "unsupported");
        let back: MediaCapabilities = serde_json::from_value(json).unwrap();
        assert_eq!(back, caps);
    }

    #[test]
    fn transport_capabilities_round_trip_and_default_false() {
        let transports: MediaTransportCapabilities =
            serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(!transports.image_inline);
        assert!(!transports.json_schema);
        let full = MediaTransportCapabilities {
            image_inline: true,
            audio_inline: true,
            audio_upload: true,
            transcription_endpoint: true,
            video_inline: true,
            video_upload: true,
            native_video: true,
            json_schema: true,
        };
        let json = serde_json::to_value(&full).unwrap();
        assert_eq!(json["image_inline"], true);
        assert_eq!(json["native_video"], true);
        let back: MediaTransportCapabilities = serde_json::from_value(json).unwrap();
        assert_eq!(back, full);
    }

    #[test]
    fn media_source_round_trips() {
        let path = MediaSource::Path {
            path: "assets/photo.jpg".to_string(),
        };
        let json = serde_json::to_value(&path).unwrap();
        assert_eq!(json["kind"], "path");
        assert_eq!(json["path"], "assets/photo.jpg");
        assert_eq!(serde_json::from_value::<MediaSource>(json).unwrap(), path);

        let artifact = MediaSource::ArtifactRef {
            blob_hash: "0123456789abcdef".to_string(),
        };
        let json = serde_json::to_value(&artifact).unwrap();
        assert_eq!(json["kind"], "artifact_ref");
        assert_eq!(json["blob_hash"], "0123456789abcdef");
        assert_eq!(
            serde_json::from_value::<MediaSource>(json).unwrap(),
            artifact
        );
    }

    #[test]
    fn detail_level_round_trips() {
        assert_eq!(
            serde_json::to_value(MediaDetailLevel::Medium).unwrap(),
            serde_json::json!("medium")
        );
        let back: MediaDetailLevel = serde_json::from_value(serde_json::json!("high")).unwrap();
        assert_eq!(back, MediaDetailLevel::High);
        let default: MediaDetailLevel = serde_json::from_value(serde_json::json!({})).unwrap_or(
            serde_json::from_value(serde_json::json!(null)).unwrap_or(MediaDetailLevel::default()),
        );
        assert_eq!(default, MediaDetailLevel::default());
    }

    #[test]
    fn strategy_allowed_for_category_matrix() {
        use MediaCategory as C;
        use MediaCategoryStrategy as S;
        assert!(S::Auto.allowed_for(C::Image));
        assert!(S::Native.allowed_for(C::Image));
        assert!(!S::Transcription.allowed_for(C::Image));
        assert!(!S::Frames.allowed_for(C::Image));

        assert!(S::Auto.allowed_for(C::Audio));
        assert!(S::Native.allowed_for(C::Audio));
        assert!(S::Transcription.allowed_for(C::Audio));
        assert!(!S::Frames.allowed_for(C::Audio));

        assert!(S::Auto.allowed_for(C::Video));
        assert!(S::Native.allowed_for(C::Video));
        assert!(!S::Transcription.allowed_for(C::Video));
        assert!(S::Frames.allowed_for(C::Video));

        // No strategy is allowed for the request-time `Auto` category: routes
        // are configured against concrete categories only.
        for strategy in [S::Auto, S::Native, S::Transcription, S::Frames] {
            assert!(!strategy.allowed_for(C::Auto));
        }
    }

    #[test]
    fn strategy_round_trips_snake_case() {
        assert_eq!(
            serde_json::to_value(MediaCategoryStrategy::Transcription).unwrap(),
            serde_json::json!("transcription")
        );
        assert_eq!(
            serde_json::to_value(MediaCategoryStrategy::Frames).unwrap(),
            serde_json::json!("frames")
        );
        let back: MediaCategoryStrategy =
            serde_json::from_value(serde_json::json!("native")).unwrap();
        assert_eq!(back, MediaCategoryStrategy::Native);
    }

    #[test]
    fn concrete_categories_are_stable() {
        assert_eq!(
            MediaCategory::CONCRETE,
            [
                MediaCategory::Image,
                MediaCategory::Audio,
                MediaCategory::Video
            ]
        );
    }

    #[test]
    fn route_metadata_round_trips() {
        let metadata = MediaRouteMetadata {
            model_id: "grok-4.5".to_string(),
            strategy: MediaCategoryStrategy::Native,
            unresolved: false,
            eligible: true,
        };
        let json = serde_json::to_value(&metadata).unwrap();
        assert_eq!(json["model_id"], "grok-4.5");
        assert_eq!(json["strategy"], "native");
        let back: MediaRouteMetadata = serde_json::from_value(json).unwrap();
        assert_eq!(back, metadata);

        // Absent booleans default to false for forward-compatible payloads.
        let sparse: MediaRouteMetadata = serde_json::from_value(serde_json::json!({
            "model_id": "grok-vision",
            "strategy": "auto",
        }))
        .unwrap();
        assert!(!sparse.unresolved);
        assert!(!sparse.eligible);
    }
}
