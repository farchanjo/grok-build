//! Default model IDs loaded from `default_models.json` at runtime.
//! Edit that JSON file to change them.
//!
//! At runtime each model is resolved via:
//!   CLI flag > ENV var > config.toml > remote settings > these defaults

use std::sync::LazyLock;

/// The raw JSON, embedded at compile time. Re-exported through the
/// `xai_grok_shell::models` facade and consumed by `agent::config`, so it must
/// be `pub` (was `pub(crate)` when this lived inside the shell crate).
pub const DEFAULT_MODELS_JSON: &str = include_str!("../default_models.json");

#[derive(serde::Deserialize)]
struct DefaultModels {
    default: String,
    /// Falls back to `default` if not specified in JSON.
    web_search: Option<String>,
    /// Falls back to `default` if not specified in JSON.
    image_description: Option<String>,
    /// Falls back to `default` if not specified in JSON.
    session_summary: Option<String>,
    models: Vec<DefaultModelEntry>,
}

#[derive(serde::Deserialize)]
struct DefaultModelEntry {
    model: String,
    /// Per-category media modality metadata. Additive; mirrors the shapes in
    /// `xai_grok_tools::media::domain` without importing that crate. Parsed
    /// here so a malformed media block in the embedded JSON fails at startup;
    /// the shell parses the same JSON authoritatively through
    /// `agent::config::DefaultModelJson`.
    #[serde(default)]
    media_capabilities: DefaultMediaCapabilities,
    /// Concrete wire-transport capability flags. Additive, see above.
    #[serde(default)]
    media_transport: DefaultMediaTransport,
}

/// Media modality metadata for one bundled default model entry. Field names
/// and value spellings must match `MediaCapabilities` / `MediaModalitySupport`
/// serialization exactly (`image`, `audio`, `video`; `supported`,
/// `unknown`, `unsupported`). Absent modality defaults to `unknown`.
#[derive(serde::Deserialize)]
#[serde(default)]
struct DefaultMediaCapabilities {
    image: String,
    audio: String,
    video: String,
}

impl Default for DefaultMediaCapabilities {
    fn default() -> Self {
        Self {
            image: "unknown".to_string(),
            audio: "unknown".to_string(),
            video: "unknown".to_string(),
        }
    }
}

/// Concrete wire-transport capability flags. Field names must match
/// `MediaTransportCapabilities` serialization exactly. Absent flags default
/// to `false`.
#[derive(serde::Deserialize)]
#[serde(default)]
struct DefaultMediaTransport {
    image_inline: bool,
    audio_inline: bool,
    audio_upload: bool,
    transcription_endpoint: bool,
    video_inline: bool,
    video_upload: bool,
    native_video: bool,
    json_schema: bool,
}

impl Default for DefaultMediaTransport {
    fn default() -> Self {
        Self {
            image_inline: false,
            audio_inline: false,
            audio_upload: false,
            transcription_endpoint: false,
            video_inline: false,
            video_upload: false,
            native_video: false,
            json_schema: false,
        }
    }
}

static DEFAULTS: LazyLock<DefaultModels> = LazyLock::new(|| {
    let defaults: DefaultModels = serde_json::from_str(DEFAULT_MODELS_JSON)
        .expect("default_models.json: invalid JSON or missing 'default' field");

    // Baked-in JSON — a mismatch here is a developer error, not a runtime condition.
    let model_ids: Vec<&str> = defaults.models.iter().map(|m| m.model.as_str()).collect();
    assert!(
        model_ids.contains(&defaults.default.as_str()),
        "default_models.json: 'default' is '{}' but 'models' array only has {model_ids:?}",
        defaults.default,
    );

    // The media blocks are additive metadata; the shell parses the same JSON
    // authoritatively. Validate the modality spellings here so a typo in the
    // embedded JSON fails fast at startup (this also consumes the fields, so
    // they are not dead code).
    for entry in &defaults.models {
        for modality in [
            &entry.media_capabilities.image,
            &entry.media_capabilities.audio,
            &entry.media_capabilities.video,
        ] {
            assert!(
                matches!(modality.as_str(), "supported" | "unknown" | "unsupported"),
                "default_models.json: invalid media modality '{}' for model '{}'",
                modality,
                entry.model,
            );
        }
        // Transport flags deserialize as bools; summing them into a value
        // keeps the fields live here (a non-bool JSON value already fails
        // serde before this point).
        let _transport_flags: u8 = entry.media_transport.image_inline as u8
            + entry.media_transport.audio_inline as u8
            + entry.media_transport.audio_upload as u8
            + entry.media_transport.transcription_endpoint as u8
            + entry.media_transport.video_inline as u8
            + entry.media_transport.video_upload as u8
            + entry.media_transport.native_video as u8
            + entry.media_transport.json_schema as u8;
        debug_assert!(_transport_flags <= 8);
    }

    defaults
});

/// Primary model for coding tasks and general fallback.
pub fn default_model() -> &'static str {
    &DEFAULTS.default
}

/// Model for web search tool synthesis. Falls back to default model.
pub fn default_web_search_model() -> &'static str {
    DEFAULTS.web_search.as_deref().unwrap_or(&DEFAULTS.default)
}

/// Model for image describe. Falls back to default model.
pub fn default_image_description_model() -> &'static str {
    DEFAULTS
        .image_description
        .as_deref()
        .unwrap_or(&DEFAULTS.default)
}

/// Model for session title generation. Falls back to default model.
pub fn default_session_summary_model() -> &'static str {
    DEFAULTS
        .session_summary
        .as_deref()
        .unwrap_or(&DEFAULTS.default)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The embedded default model JSON parses and every entry's media block
    /// carries valid modality spellings. Touching `DEFAULTS` here exercises
    /// the startup validation loop at test time too.
    #[test]
    fn embedded_media_metadata_is_valid() {
        let defaults: DefaultModels =
            serde_json::from_str(DEFAULT_MODELS_JSON).expect("default_models.json must parse");
        assert!(!defaults.models.is_empty());
        for entry in &defaults.models {
            for modality in [
                &entry.media_capabilities.image,
                &entry.media_capabilities.audio,
                &entry.media_capabilities.video,
            ] {
                assert!(
                    matches!(modality.as_str(), "supported" | "unknown" | "unsupported"),
                    "invalid modality {modality} for {}",
                    entry.model
                );
            }
        }
        // Touches the lazy-load (and its validation loop).
        let _ = default_model();
    }
}
