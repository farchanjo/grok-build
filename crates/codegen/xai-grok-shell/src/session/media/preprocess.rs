//! Preprocessing for media-understanding routes (plan sections 5.1, 9, and
//! 10).
//!
//! Image normalization reuses the shared re-encode utility (same bytes, same
//! deterministic params) so cache keys stay stable. Audio decode and video
//! frame extraction go through the always-compiled tools FFmpeg API
//! (`xai_grok_tools::media::ffmpeg_api`): the tools crate owns the native
//! build-time gating (`cfg(media_ffmpeg)`), so this crate never touches that
//! cfg. When the tools backend is compiled out or its runtime libraries are
//! missing, the FFmpeg-dependent strategies degrade to "unavailable" — they
//! never fall back to a subprocess and never panic.
//!
//! Preprocessing is deterministic and bounded: byte caps come from the
//! resolved config, frame extraction is deterministic across equal inputs, and
//! the preprocess profile (name + version) folds into the canonical semantic
//! cache key so a config change invalidates cached semantics.

use xai_grok_tools::media::domain::{MediaCategory, MediaCategoryStrategy};

use crate::agent::config::ResolvedMediaUnderstandingConfig;

/// Preprocess profile version. Bump when normalization/decode parameters that
/// change the output change.
pub(crate) const PREPROCESS_VERSION: u32 = 1;

/// Preprocess profile name/version; both feed the canonical semantic cache
/// key so two requests with different preprocessing must not share a result.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PreprocessProfile {
    pub profile: String,
    pub version: u32,
}

impl PreprocessProfile {
    /// Default profile (used when no config is available).
    pub(crate) fn default() -> Self {
        Self {
            profile: "default".to_string(),
            version: PREPROCESS_VERSION,
        }
    }

    /// Profile for a resolved config.
    ///
    /// The preprocess limits that change the output (frame cap, contact-sheet
    /// side, source byte cap) fold into the profile name so a config change
    /// invalidates cached semantics.
    pub(crate) fn for_config(config: &ResolvedMediaUnderstandingConfig) -> Self {
        let fingerprint = format!(
            "frames={max_video_frames};sheet={max_contact_sheet_side_px};bytes={max_media_bytes}",
            max_video_frames = config.max_video_frames,
            max_contact_sheet_side_px = config.max_contact_sheet_side_px,
            max_media_bytes = config.max_media_bytes,
        );
        Self {
            profile: format!("default-{fingerprint}"),
            version: PREPROCESS_VERSION,
        }
    }
}

/// Output of preprocessing one media item for a concrete strategy.
#[derive(Debug, Clone)]
pub(crate) enum PreprocessOutcome {
    /// A single normalized image ready for `ContentPart::Image`.
    Image {
        bytes: Vec<u8>,
        mime: String,
        width: u32,
        height: u32,
    },
    /// Deterministic frame set extracted from a video (each re-encoded as an
    /// image for `ContentPart::Image`).
    VideoFrames { frames: Vec<PreprocessedFrame> },
    /// Bounded normalized PCM for an audio stream. No concrete wire path
    /// exists today (the transcription/native-audio adapters are additive);
    /// produced so future adapters can consume it.
    AudioPcm {
        sample_rate: u32,
        channels: u32,
        duration_secs: f64,
        samples: Vec<f32>,
    },
}

/// One re-encoded video frame.
#[derive(Debug, Clone)]
pub(crate) struct PreprocessedFrame {
    pub bytes: Vec<u8>,
    pub mime: String,
    pub width: u32,
    pub height: u32,
    /// Approximate timestamp in seconds within the source.
    pub timestamp_secs: u32,
}

/// Preprocessing failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum PreprocessError {
    #[error("preprocessing strategy not available: {0}")]
    Unavailable(String),
    #[error("preprocessing failed: {0}")]
    Failed(String),
}

/// Preprocess one media item for a concrete route strategy.
///
/// `bytes` must already be bounded by the policy layer
/// (`MediaPolicyLimits::max_media_bytes`).
///
/// Traced as `media.preprocess` (plan section 17) with category, strategy,
/// and byte count only — never media bytes or paths.
#[tracing::instrument(
    name = "media.preprocess",
    level = "debug",
    skip_all,
    fields(
        category = ?category,
        strategy = ?strategy,
        source_bytes = bytes.len(),
    )
)]
pub(crate) fn preprocess_media(
    category: MediaCategory,
    strategy: MediaCategoryStrategy,
    bytes: &[u8],
    _mime: Option<&str>,
    config: &ResolvedMediaUnderstandingConfig,
) -> Result<PreprocessOutcome, PreprocessError> {
    use MediaCategory as C;
    use MediaCategoryStrategy as S;
    match (category, strategy) {
        (C::Image, S::Native | S::Auto) => {
            normalize_image(bytes, config).map(|(bytes, mime, width, height)| {
                PreprocessOutcome::Image {
                    bytes,
                    mime,
                    width,
                    height,
                }
            })
        }
        (C::Video, S::Frames) => video_frames(bytes, config),
        (C::Audio, S::Native | S::Transcription | S::Auto) => decode_audio(bytes, config),
        (C::Video, S::Native) => Err(PreprocessError::Unavailable(
            "native video transport is not available".to_string(),
        )),
        _ => Err(PreprocessError::Unavailable(format!(
            "no preprocessing path for category={category:?} strategy={strategy:?}"
        ))),
    }
}

/// Sniff the concrete media category of raw bytes / MIME / path suffix.
///
/// Used to resolve `MediaCategory::Auto` requests. Image detection is
/// byte-driven (validated allow-listed decode); audio/video fall back to
/// MIME and extension hints.
pub(crate) fn sniff_category(
    bytes: &[u8],
    mime: Option<&str>,
    path: Option<&str>,
) -> Option<MediaCategory> {
    if xai_grok_tools::util::image_validate::validate_image_bytes(bytes).is_ok() {
        return Some(MediaCategory::Image);
    }
    if let Some(category) = mime.and_then(|mime| {
        let mime = mime.to_ascii_lowercase();
        if mime.starts_with("audio/") {
            Some(MediaCategory::Audio)
        } else if mime.starts_with("video/") {
            Some(MediaCategory::Video)
        } else if mime.starts_with("image/") {
            Some(MediaCategory::Image)
        } else {
            None
        }
    }) {
        return Some(category);
    }
    let ext = path
        .and_then(|p| p.rsplit('.').next())
        .map(str::to_ascii_lowercase);
    match ext.as_deref() {
        Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "tiff" | "tif") => {
            Some(MediaCategory::Image)
        }
        Some("mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "opus") => Some(MediaCategory::Audio),
        Some("mp4" | "mov" | "mkv" | "webm" | "avi" | "m4v" | "ts") => Some(MediaCategory::Video),
        _ => None,
    }
}

/// Shared re-encode params for auxiliary images (deterministic across calls).
fn re_encode_params(
    config: &ResolvedMediaUnderstandingConfig,
) -> xai_grok_tools::util::image_compress::ReEncodeParams {
    use xai_grok_tools::util::image_compress::{FilterType, ReEncodeParams};
    ReEncodeParams {
        // Auxiliary images ride inside prompt content; keep the wire payload
        // modest even when the source cap is large.
        max_bytes: 8 * 1024 * 1024,
        max_side_px: config.max_contact_sheet_side_px as u32,
        max_pixels: u64::MAX,
        min_side_px: 256,
        quality_steps: &[88, 72, 56, 40, 24],
        filter: FilterType::CatmullRom,
    }
}

/// Normalize a source image to a bounded re-encoded image.
///
/// Mirrors the user-attachment normalizer: header validation, structural
/// completeness check, then PNG/JPEG re-encode under the byte/dimension caps.
fn normalize_image(
    bytes: &[u8],
    config: &ResolvedMediaUnderstandingConfig,
) -> Result<(Vec<u8>, String, u32, u32), PreprocessError> {
    let (width, height, _) =
        xai_grok_tools::util::image_validate::validate_image_bytes_with(bytes, false)
            .map_err(|e| PreprocessError::Failed(format!("image validation: {e}")))?;
    if !xai_grok_tools::util::image_validate::image_structurally_complete(bytes) {
        return Err(PreprocessError::Failed(
            "image bytes are structurally incomplete".to_string(),
        ));
    }
    let decoded = image::load_from_memory(bytes)
        .map_err(|e| PreprocessError::Failed(format!("image decode: {e}")))?;
    let params = re_encode_params(config);
    let (out, new_width, new_height, mime) =
        xai_grok_tools::util::image_compress::re_encode_under_limit(&decoded, &params)
            .map_err(|e| PreprocessError::Failed(format!("image re-encode: {e}")))?;
    if out.len() >= bytes.len() && width == new_width && height == new_height {
        // Nothing gained by re-encoding; pass the original bytes through.
        return Ok((bytes.to_vec(), mime.to_string(), width, height));
    }
    Ok((out, mime.to_string(), new_width, new_height))
}

/// Decode an audio stream to bounded normalized PCM.
///
/// Goes through the always-compiled tools FFmpeg API: a compiled-out or
/// runtime-unloadable backend degrades to `Unavailable` (the route is
/// skipped without sending bytes); bounded open/decode failures are
/// `Failed` (terminal). No subprocess is ever spawned.
fn decode_audio(
    bytes: &[u8],
    config: &ResolvedMediaUnderstandingConfig,
) -> Result<PreprocessOutcome, PreprocessError> {
    use xai_grok_tools::media::ffmpeg_api::{DecodeLimits, decode_audio_pcm};
    let limits = DecodeLimits {
        max_source_bytes: config.max_media_bytes as usize,
        max_duration_us: config.max_audio_seconds.saturating_mul(1_000_000),
        max_audio_samples: config.max_audio_seconds.saturating_mul(48_000 * 2),
        request_timeout_ms: config.max_preprocess_wallclock_ms,
        ..DecodeLimits::default()
    };
    let pcm = decode_audio_pcm(bytes.to_vec(), limits).map_err(ffmpeg_api_error)?;
    Ok(PreprocessOutcome::AudioPcm {
        sample_rate: pcm.sample_rate,
        channels: pcm.channels,
        duration_secs: pcm.duration_secs,
        samples: pcm.samples,
    })
}

/// Extract deterministic video frames and re-encode them as images.
///
/// Goes through the always-compiled tools FFmpeg API: a compiled-out or
/// runtime-unloadable backend degrades to `Unavailable` (the route is
/// skipped without sending bytes); bounded open/probe/decode failures are
/// `Failed` (terminal). No subprocess is ever spawned.
fn video_frames(
    bytes: &[u8],
    config: &ResolvedMediaUnderstandingConfig,
) -> Result<PreprocessOutcome, PreprocessError> {
    use xai_grok_tools::media::ffmpeg_api::{DecodeLimits, extract_video_frames};
    let limits = DecodeLimits {
        max_source_bytes: config.max_media_bytes as usize,
        max_duration_us: config.max_video_seconds.saturating_mul(1_000_000),
        max_video_frames: config.max_video_frames.min(i32::MAX as u64) as i32,
        request_timeout_ms: config.max_preprocess_wallclock_ms,
        ..DecodeLimits::default()
    };
    let frames = extract_video_frames(bytes.to_vec(), limits).map_err(ffmpeg_api_error)?;
    let mut out = Vec::with_capacity(frames.len());
    for frame in frames {
        if let Ok(encoded) = encode_frame(&frame, config) {
            out.push(encoded);
        }
    }
    if out.is_empty() {
        return Err(PreprocessError::Failed(
            "no video frames could be extracted".to_string(),
        ));
    }
    Ok(PreprocessOutcome::VideoFrames { frames: out })
}

/// Map a tools FFmpeg API error onto the shell preprocess error space.
///
/// Compile-out and runtime load failures are `Unavailable` (route skipped
/// without sending bytes); decode/open failures are `Failed` (terminal).
fn ffmpeg_api_error(error: xai_grok_tools::media::ffmpeg_api::FfmpegApiError) -> PreprocessError {
    use xai_grok_tools::media::ffmpeg_api::FfmpegApiError;
    match error {
        FfmpegApiError::Unavailable(reason) => {
            PreprocessError::Unavailable(format!("ffmpeg: {reason}"))
        }
        FfmpegApiError::Failed(reason) => PreprocessError::Failed(reason),
    }
}

/// Re-encode one decoded RGB24 frame as a bounded image.
fn encode_frame(
    frame: &xai_grok_tools::media::ffmpeg_api::DecodedFrameOutput,
    config: &ResolvedMediaUnderstandingConfig,
) -> Result<PreprocessedFrame, PreprocessError> {
    use image::{Rgb, RgbImage};
    let width = frame.width.max(1);
    let height = frame.height.max(1);
    let stride = frame.stride.max(width as i32 * 3) as usize;
    let mut img = RgbImage::new(width, height);
    for y in 0..height {
        let start = y as usize * stride;
        let row = &frame.data[start..(start + width as usize * 3)];
        for x in 0..width {
            let offset = x as usize * 3;
            img.put_pixel(x, y, Rgb([row[offset], row[offset + 1], row[offset + 2]]));
        }
    }
    let decoded = image::DynamicImage::ImageRgb8(img);
    let params = re_encode_params(config);
    let (bytes, new_width, new_height, mime) =
        xai_grok_tools::util::image_compress::re_encode_under_limit(&decoded, &params)
            .map_err(|e| PreprocessError::Failed(format!("frame re-encode: {e}")))?;
    Ok(PreprocessedFrame {
        bytes,
        mime: mime.to_string(),
        width: new_width,
        height: new_height,
        timestamp_secs: frame.pts_us.unwrap_or(0).max(0) as u32 / 1_000_000,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::config::ResolvedMediaCategoryConfig;

    fn config() -> ResolvedMediaUnderstandingConfig {
        ResolvedMediaUnderstandingConfig {
            enabled: true,
            auto_enrich: false,
            compaction_enrichment: false,
            active_model_unknown_policy: Default::default(),
            compaction_preflight_policy: Default::default(),
            max_output_chars: 20_000,
            max_aux_tokens_per_call: 8_192,
            max_aux_budget_usd_ticks: 1_000_000_000,
            max_media_bytes: 256 * 1024 * 1024,
            max_audio_seconds: 1_800,
            max_video_seconds: 900,
            max_video_frames: 32,
            max_contact_sheet_side_px: 2_048,
            max_preprocess_wallclock_ms: 120_000,
            preprocess_concurrency: 2,
            circuit_breaker: crate::agent::config::ResolvedMediaCircuitBreakerConfig {
                failures: 5,
                window_secs: 300,
            },
            image: ResolvedMediaCategoryConfig {
                routes: vec![],
                max_seconds: None,
                max_frames: None,
            },
            audio: ResolvedMediaCategoryConfig {
                routes: vec![],
                max_seconds: None,
                max_frames: None,
            },
            video: ResolvedMediaCategoryConfig {
                routes: vec![],
                max_seconds: None,
                max_frames: None,
            },
        }
    }

    fn png_bytes(width: u32, height: u32) -> Vec<u8> {
        use image::{ImageBuffer, Rgba};
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(width, height, Rgba([128, 64, 32, 255]));
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        buf
    }

    #[test]
    fn media_preprocess_image_normalizes_deterministically() {
        let cfg = config();
        let bytes = png_bytes(64, 64);
        let first = preprocess_media(
            MediaCategory::Image,
            MediaCategoryStrategy::Native,
            &bytes,
            None,
            &cfg,
        )
        .unwrap();
        let second = preprocess_media(
            MediaCategory::Image,
            MediaCategoryStrategy::Native,
            &bytes,
            None,
            &cfg,
        )
        .unwrap();
        match (first, second) {
            (
                PreprocessOutcome::Image {
                    bytes: a, mime: ma, ..
                },
                PreprocessOutcome::Image {
                    bytes: b, mime: mb, ..
                },
            ) => {
                assert_eq!(a, b, "normalization must be deterministic");
                assert_eq!(ma, mb);
            }
            _ => panic!("expected Image outcomes"),
        }
    }

    #[test]
    fn media_preprocess_image_rejects_truncated_bytes() {
        let cfg = config();
        let mut bytes = png_bytes(16, 16);
        bytes.truncate(bytes.len() / 2);
        let result = preprocess_media(
            MediaCategory::Image,
            MediaCategoryStrategy::Native,
            &bytes,
            None,
            &cfg,
        );
        assert!(result.is_err());
    }

    /// Audio/video preprocessing is bounded with or without the native
    /// FFmpeg backend:
    /// - compiled-out or runtime-unloadable backend: both strategies
    ///   degrade to `Unavailable` (route skipped, no bytes sent);
    /// - available backend: malformed inputs fail closed as `Failed`
    ///   (terminal) — never a panic, never a hang.
    #[test]
    fn media_preprocess_audio_video_degrades_or_fails_boundedly() {
        use xai_grok_tools::media::ffmpeg_api::{FfmpegAvailability, availability};
        let cfg = config();
        let audio = preprocess_media(
            MediaCategory::Audio,
            MediaCategoryStrategy::Transcription,
            b"not-real-audio",
            Some("audio/mp4"),
            &cfg,
        );
        let video = preprocess_media(
            MediaCategory::Video,
            MediaCategoryStrategy::Frames,
            b"not-real-video",
            Some("video/mp4"),
            &cfg,
        );
        match availability() {
            FfmpegAvailability::CompiledOut | FfmpegAvailability::Unavailable(_) => {
                match audio {
                    Err(PreprocessError::Unavailable(_)) => {}
                    other => {
                        panic!("audio must degrade to Unavailable without FFmpeg, got {other:?}")
                    }
                }
                match video {
                    Err(PreprocessError::Unavailable(_)) => {}
                    other => {
                        panic!(
                            "video frames must degrade to Unavailable without FFmpeg, \
                             got {other:?}"
                        )
                    }
                }
            }
            FfmpegAvailability::Available => {
                match audio {
                    Err(PreprocessError::Failed(_)) => {}
                    other => panic!(
                        "audio garbage must fail closed as Failed when FFmpeg is available, \
                         got {other:?}"
                    ),
                }
                match video {
                    Err(PreprocessError::Failed(_)) => {}
                    other => panic!(
                        "video garbage must fail closed as Failed when FFmpeg is available, \
                         got {other:?}"
                    ),
                }
            }
        }
    }

    /// The native frames/PCM paths execute end-to-end when the tools FFmpeg
    /// backend is available: a WAV decodes to bounded normalized PCM and an
    /// AVI yields re-encoded frames. Without the backend the same routes
    /// degrade to `Unavailable`.
    #[test]
    fn media_preprocess_audio_pcm_and_video_frames_execute_when_ffmpeg_available() {
        use xai_grok_tools::media::ffmpeg_api::{FfmpegAvailability, availability};
        let cfg = config();
        match availability() {
            FfmpegAvailability::Available => {
                let wav = make_wav(44_100, 1, &sine_samples(8_000, 44_100, 440.0));
                let outcome = preprocess_media(
                    MediaCategory::Audio,
                    MediaCategoryStrategy::Transcription,
                    &wav,
                    Some("audio/wav"),
                    &cfg,
                )
                .expect("WAV must decode when the native FFmpeg backend is available");
                match outcome {
                    PreprocessOutcome::AudioPcm {
                        sample_rate,
                        channels,
                        duration_secs,
                        samples,
                    } => {
                        assert!(sample_rate > 0, "sample rate must be reported");
                        assert!(channels > 0, "channel count must be reported");
                        assert!(!samples.is_empty(), "PCM samples must be produced");
                        assert!(duration_secs > 0.0, "duration must be reported");
                        assert!(samples.iter().all(|s| s.is_finite()), "PCM must be finite");
                    }
                    other => panic!("expected AudioPcm outcome, got {other:?}"),
                }

                let jpeg_frames = [
                    make_jpeg_frame(6, 6, 0x10),
                    make_jpeg_frame(6, 6, 0x50),
                    make_jpeg_frame(6, 6, 0x90),
                ];
                let avi = make_avi(&jpeg_frames, 6, 6);
                let outcome = preprocess_media(
                    MediaCategory::Video,
                    MediaCategoryStrategy::Frames,
                    &avi,
                    Some("video/avi"),
                    &cfg,
                )
                .expect("AVI frames must extract when the native FFmpeg backend is available");
                match outcome {
                    PreprocessOutcome::VideoFrames { frames } => {
                        assert!(!frames.is_empty(), "at least one frame must be produced");
                        assert!(
                            frames
                                .iter()
                                .all(|f| f.width > 0 && f.height > 0 && !f.bytes.is_empty()),
                            "frames must be non-empty re-encoded images"
                        );
                    }
                    other => panic!("expected VideoFrames outcome, got {other:?}"),
                }
            }
            FfmpegAvailability::CompiledOut | FfmpegAvailability::Unavailable(_) => {
                // Degradation contract, pinned here too so both modes are
                // covered by focused tests.
                assert!(matches!(
                    preprocess_media(
                        MediaCategory::Audio,
                        MediaCategoryStrategy::Transcription,
                        b"x",
                        None,
                        &cfg,
                    ),
                    Err(PreprocessError::Unavailable(_))
                ));
                assert!(matches!(
                    preprocess_media(
                        MediaCategory::Video,
                        MediaCategoryStrategy::Frames,
                        b"x",
                        None,
                        &cfg,
                    ),
                    Err(PreprocessError::Unavailable(_))
                ));
            }
        }
    }

    /// Hand-rolled WAV (PCM s16).
    fn make_wav(sample_rate: u32, channels: u16, samples: &[i16]) -> Vec<u8> {
        let data_size = samples.len() * 2;
        let mut bytes = Vec::new();
        bytes.extend(b"RIFF");
        bytes.extend(&(data_size as u32 + 36).to_le_bytes());
        bytes.extend(b"WAVE");
        bytes.extend(b"fmt ");
        bytes.extend(&16u32.to_le_bytes());
        bytes.extend(&1u16.to_le_bytes()); // PCM
        bytes.extend(&channels.to_le_bytes());
        bytes.extend(&sample_rate.to_le_bytes());
        bytes.extend(&(sample_rate * channels as u32 * 2).to_le_bytes());
        bytes.extend(&(channels * 2).to_le_bytes());
        bytes.extend(&16u16.to_le_bytes());
        bytes.extend(b"data");
        bytes.extend(&(data_size as u32).to_le_bytes());
        for s in samples {
            bytes.extend(&s.to_le_bytes());
        }
        bytes
    }

    fn sine_samples(count: usize, sample_rate: u32, freq: f32) -> Vec<i16> {
        (0..count)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                (0.9 * (2.0 * std::f32::consts::PI * freq * t).sin() * i16::MAX as f32) as i16
            })
            .collect()
    }

    fn make_jpeg_frame(width: u32, height: u32, base: u8) -> Vec<u8> {
        let mut img = image::RgbImage::new(width, height);
        for (x, y, pixel) in img.enumerate_pixels_mut() {
            *pixel = image::Rgb([
                base.wrapping_add((x * 13) as u8),
                base.wrapping_add((y * 17) as u8),
                0x80,
            ]);
        }
        let mut bytes = Vec::new();
        img.write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Jpeg,
        )
        .expect("jpeg encode");
        bytes
    }

    /// Hand-rolled AVI (MJPEG) with `frames` JPEG-encoded frames. No `idx1`
    /// index: sequential decoding works; seeking may fail gracefully.
    fn make_avi(frames: &[Vec<u8>], width: u32, height: u32) -> Vec<u8> {
        // RIFF/LIST chunk sizes (the size field counts bytes after the size
        // field, including the "hdrl"/"movi"/"strl" fourcc).
        let strh_size: u32 = 56;
        let strf_size: u32 = 40;
        let strl_payload: u32 = 4 + (4 + 4 + strh_size) + (4 + 4 + strf_size); // 116
        let avih_chunk: u32 = 4 + 4 + 56;
        let strl_chunk: u32 = 4 + 4 + strl_payload;
        let hdrl_size: u32 = 4 + avih_chunk + strl_chunk; // 192
        let movi_payload: u32 = frames
            .iter()
            .map(|f| 8 + f.len() as u32 + (f.len() as u32 & 1))
            .sum();
        let movi_size: u32 = 4 + movi_payload;
        let riff_size: u32 = 4 + (4 + 4 + hdrl_size) + (4 + 4 + movi_size);

        let mut bytes = Vec::new();
        bytes.extend(b"RIFF");
        bytes.extend(&riff_size.to_le_bytes());
        bytes.extend(b"AVI ");

        bytes.extend(b"LIST");
        bytes.extend(&hdrl_size.to_le_bytes());
        bytes.extend(b"hdrl");

        bytes.extend(b"avih");
        bytes.extend(&56u32.to_le_bytes());
        bytes.extend(&40_000u32.to_le_bytes()); // dwMicroSecPerFrame (25 fps)
        bytes.extend(&0u32.to_le_bytes()); // dwMaxBytesPerSec
        bytes.extend(&0u32.to_le_bytes()); // dwPaddingGranularity
        bytes.extend(&0u32.to_le_bytes()); // dwFlags
        bytes.extend(&(frames.len() as u32).to_le_bytes()); // dwTotalFrames
        bytes.extend(&0u32.to_le_bytes()); // dwInitialFrames
        bytes.extend(&1u32.to_le_bytes()); // dwStreams
        bytes.extend(&0u32.to_le_bytes()); // dwSuggestedBufferSize
        bytes.extend(&width.to_le_bytes());
        bytes.extend(&height.to_le_bytes());
        bytes.extend(&[0u8; 16]);

        bytes.extend(b"LIST");
        bytes.extend(&strl_payload.to_le_bytes());
        bytes.extend(b"strl");

        bytes.extend(b"strh");
        bytes.extend(&56u32.to_le_bytes());
        bytes.extend(b"vids");
        bytes.extend(b"MJPG");
        bytes.extend(&0u32.to_le_bytes()); // dwFlags
        bytes.extend(&0u16.to_le_bytes()); // wPriority
        bytes.extend(&0u16.to_le_bytes()); // wLanguage
        bytes.extend(&0u32.to_le_bytes()); // dwInitialFrames
        bytes.extend(&1u32.to_le_bytes()); // dwScale
        bytes.extend(&25u32.to_le_bytes()); // dwRate
        bytes.extend(&0u32.to_le_bytes()); // dwStart
        bytes.extend(&(frames.len() as u32).to_le_bytes()); // dwLength
        bytes.extend(&0u32.to_le_bytes()); // dwSuggestedBufferSize
        bytes.extend(&0u32.to_le_bytes()); // dwQuality
        bytes.extend(&0u32.to_le_bytes()); // dwSampleSize
        bytes.extend(&[0u8; 8]);

        bytes.extend(b"strf");
        bytes.extend(&40u32.to_le_bytes());
        bytes.extend(&40u32.to_le_bytes()); // biSize
        bytes.extend(&width.to_le_bytes()); // biWidth
        bytes.extend(&((height as i32).wrapping_neg()).to_le_bytes()); // biHeight (top-down)
        bytes.extend(&1u16.to_le_bytes()); // biPlanes
        bytes.extend(&24u16.to_le_bytes()); // biBitCount
        bytes.extend(b"MJPG"); // biCompression
        bytes.extend(&(width * height * 3).to_le_bytes()); // biSizeImage
        bytes.extend(&0i32.to_le_bytes()); // biXPelsPerMeter
        bytes.extend(&0i32.to_le_bytes()); // biYPelsPerMeter
        bytes.extend(&0u32.to_le_bytes()); // biClrUsed
        bytes.extend(&0u32.to_le_bytes()); // biClrImportant

        bytes.extend(b"LIST");
        bytes.extend(&movi_size.to_le_bytes());
        bytes.extend(b"movi");
        for frame in frames {
            bytes.extend(b"00dc");
            bytes.extend(&(frame.len() as u32).to_le_bytes());
            bytes.extend(frame);
            if frame.len() & 1 == 1 {
                bytes.push(0);
            }
        }
        bytes
    }

    #[test]
    fn media_preprocess_native_video_is_unavailable() {
        let cfg = config();
        let result = preprocess_media(
            MediaCategory::Video,
            MediaCategoryStrategy::Native,
            b"x",
            None,
            &cfg,
        );
        assert!(matches!(result, Err(PreprocessError::Unavailable(_))));
    }

    #[test]
    fn media_preprocess_profile_folds_limits_into_name() {
        let a = config();
        let mut b = config();
        b.max_video_frames = 8;
        let profile_a = PreprocessProfile::for_config(&a);
        let profile_b = PreprocessProfile::for_config(&b);
        assert_ne!(profile_a.profile, profile_b.profile);
        assert_eq!(profile_a.version, PREPROCESS_VERSION);
        assert_eq!(profile_b.version, PREPROCESS_VERSION);
    }

    #[test]
    fn media_preprocess_sniff_category() {
        let image = png_bytes(8, 8);
        assert_eq!(
            sniff_category(&image, None, None),
            Some(MediaCategory::Image)
        );
        assert_eq!(
            sniff_category(b"junk", Some("video/mp4"), None),
            Some(MediaCategory::Video)
        );
        assert_eq!(
            sniff_category(b"junk", Some("audio/wav"), None),
            Some(MediaCategory::Audio)
        );
        assert_eq!(
            sniff_category(b"junk", None, Some("clip.mp4")),
            Some(MediaCategory::Video)
        );
        assert_eq!(
            sniff_category(b"junk", None, Some("note.mp3")),
            Some(MediaCategory::Audio)
        );
        assert_eq!(
            sniff_category(b"junk", None, Some("photo.png")),
            Some(MediaCategory::Image)
        );
        assert_eq!(sniff_category(b"junk", None, Some("notes.txt")), None);
    }

    /// PR 10: adversarial preprocessing coverage. Malformed, truncated,
    /// wrong-magic, and plain-text inputs must be handled by
    /// `preprocess_media` across every category/strategy combination with
    /// hard bounds and no hangs, and `sniff_category` must never panic on
    /// arbitrary bytes.
    #[test]
    fn media_preprocess_adversarial_corpus_never_panics_or_hangs() {
        let cfg = config();
        let start = std::time::Instant::now();

        let mut corpus: Vec<Vec<u8>> = Vec::new();
        let png = png_bytes(16, 16);
        // Truncated PNGs (including empty and full-length control).
        for cut in [0usize, 1, 8, 30, png.len() / 2, png.len()] {
            corpus.push(png[..cut].to_vec());
        }
        // Wrong-magic pseudo-random garbage with media magic prefixes.
        let mut state = 0x1234_5678u32;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state
        };
        for magic in [
            b"\x89PNG".as_slice(),
            b"RIFF".as_slice(),
            b"GIF89a".as_slice(),
            b"ID3".as_slice(),
            b"OggS".as_slice(),
            b"\xff\xd8\xff".as_slice(),
        ] {
            let mut blob = magic.to_vec();
            for _ in 0..2048 {
                blob.push((next() & 0xFF) as u8);
            }
            corpus.push(blob);
        }
        corpus.push(b"this is not media at all".to_vec());
        corpus.push(vec![]);

        let combinations = [
            (MediaCategory::Image, MediaCategoryStrategy::Native),
            (MediaCategory::Audio, MediaCategoryStrategy::Transcription),
            (MediaCategory::Video, MediaCategoryStrategy::Frames),
            (MediaCategory::Video, MediaCategoryStrategy::Native),
        ];

        for input in &corpus {
            for (category, strategy) in combinations {
                // Must never panic; Ok or Err are both valid bounded outcomes.
                let _result = preprocess_media(category, strategy, input, None, &cfg);
                // Sniffing must never panic on arbitrary bytes.
                let _sniffed = sniff_category(input, None, None);
            }
        }

        assert!(
            start.elapsed() < std::time::Duration::from_secs(30),
            "adversarial corpus exceeded the wall-clock bound"
        );
    }
}
