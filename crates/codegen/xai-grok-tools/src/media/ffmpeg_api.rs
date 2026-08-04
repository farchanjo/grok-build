//! Always-compiled public seam for the native FFmpeg 8 preprocessing
//! primitives (plan section 10).
//!
//! This module is compiled into every build of `xai-grok-tools`. The native
//! backend itself (`super::ffmpeg`) is compiled only when `build.rs`
//! discovers compatible FFmpeg 8 headers on a supported target and
//! `GROK_DISABLE_MEDIA_FFMPEG` is unset (`cfg(media_ffmpeg)`).
//!
//! Consumers that must not duplicate the build cfg — for example the shell
//! crate's media preprocessing — call these functions directly. Runtime
//! availability reports the three states a caller can act on:
//!
//! - [`FfmpegAvailability::CompiledOut`]: the tools build had no compatible
//!   FFmpeg 8 headers (or the kill switch was set at build time). No runtime
//!   load is attempted.
//! - [`FfmpegAvailability::Unavailable`]: the backend was compiled in but
//!   its runtime libraries could not be loaded (missing/mismatched install,
//!   or the `GROK_DISABLE_MEDIA_FFMPEG` kill switch set at runtime).
//! - [`FfmpegAvailability::Available`]: the backend loaded and ABI-validated.
//!
//! Operations fail with [`FfmpegApiError::Unavailable`] in the first two
//! states and with [`FfmpegApiError::Failed`] for bounded open/probe/decode
//! failures. No subprocess is ever spawned, no paths are accepted, and no
//! media bytes leave the process on any error path.

/// Whether the native FFmpeg backend is usable in this build/process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FfmpegAvailability {
    /// The native backend was compiled out: no compatible FFmpeg 8 headers
    /// at tools build time, or `GROK_DISABLE_MEDIA_FFMPEG` at build time.
    CompiledOut,
    /// The backend was compiled in but its runtime libraries are not
    /// usable: missing install, ABI mismatch, or the runtime kill switch.
    Unavailable(String),
    /// The backend loaded and ABI-validated; operations can execute.
    Available,
}

/// Errors from the always-compiled FFmpeg API.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FfmpegApiError {
    /// The native backend is compiled out or its runtime libraries cannot
    /// load (including the `GROK_DISABLE_MEDIA_FFMPEG` kill switch).
    #[error("native FFmpeg is not available: {0}")]
    Unavailable(String),
    /// A bounded native open/probe/decode failure.
    #[error("native FFmpeg operation failed: {0}")]
    Failed(String),
}

/// Bounded limits for one decode operation.
///
/// The fields mirror the `super::ffmpeg::FfmpegLimits` subset the
/// media-understanding pipeline tunes; the rest of the native limits keep
/// their defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeLimits {
    /// Maximum accepted source byte length.
    pub max_source_bytes: usize,
    /// Maximum media duration in microseconds.
    pub max_duration_us: u64,
    /// Maximum total output PCM samples (frames * channels).
    pub max_audio_samples: u64,
    /// Maximum video frames extracted / sequential frame iterations.
    pub max_video_frames: i32,
    /// Per-request wall-clock deadline in milliseconds.
    pub request_timeout_ms: u64,
}

impl Default for DecodeLimits {
    fn default() -> Self {
        DecodeLimits {
            max_source_bytes: 256 * 1024 * 1024,
            max_duration_us: 900 * 1_000_000,
            max_audio_samples: 900 * 48_000 * 2,
            max_video_frames: 64,
            request_timeout_ms: 30_000,
        }
    }
}

/// Bounded normalized PCM (interleaved float32, `[-1, 1]`).
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedPcmOutput {
    /// Interleaved samples, `len = frames * channels`.
    pub samples: Vec<f32>,
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Channel count.
    pub channels: u32,
    /// Duration in seconds.
    pub duration_secs: f64,
}

/// One decoded RGB24 frame, copied into a Rust-owned buffer.
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedFrameOutput {
    /// RGB24 packed rows; bytes per row is `stride`.
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub stride: i32,
    /// Frame PTS in microseconds (`None` when unknown).
    pub pts_us: Option<i64>,
}

/// Runtime availability of the native FFmpeg backend (see module docs).
pub fn availability() -> FfmpegAvailability {
    #[cfg(media_ffmpeg)]
    {
        use super::ffmpeg::loader::{self, FfmpegLoadOutcome};
        match loader::load_once() {
            FfmpegLoadOutcome::Loaded(_) => FfmpegAvailability::Available,
            FfmpegLoadOutcome::Failed(error) => FfmpegAvailability::Unavailable(error.to_string()),
        }
    }
    #[cfg(not(media_ffmpeg))]
    {
        FfmpegAvailability::CompiledOut
    }
}

/// Decode an audio stream to bounded normalized PCM.
pub fn decode_audio_pcm(
    bytes: Vec<u8>,
    limits: DecodeLimits,
) -> Result<DecodedPcmOutput, FfmpegApiError> {
    #[cfg(media_ffmpeg)]
    {
        decode_audio_pcm_native(bytes, limits)
    }
    #[cfg(not(media_ffmpeg))]
    {
        let _ = (bytes, limits);
        Err(compiled_out())
    }
}

/// Extract a deterministic, bounded set of video frames.
pub fn extract_video_frames(
    bytes: Vec<u8>,
    limits: DecodeLimits,
) -> Result<Vec<DecodedFrameOutput>, FfmpegApiError> {
    #[cfg(media_ffmpeg)]
    {
        extract_video_frames_native(bytes, limits)
    }
    #[cfg(not(media_ffmpeg))]
    {
        let _ = (bytes, limits);
        Err(compiled_out())
    }
}

#[cfg(not(media_ffmpeg))]
fn compiled_out() -> FfmpegApiError {
    FfmpegApiError::Unavailable("native FFmpeg is not compiled into this build".to_string())
}

// ---------------------------------------------------------------------------
// Native implementations (compiled only when the tools build.rs found
// compatible FFmpeg 8 headers and the kill switch is unset).
// ---------------------------------------------------------------------------

#[cfg(media_ffmpeg)]
fn decode_audio_pcm_native(
    bytes: Vec<u8>,
    limits: DecodeLimits,
) -> Result<DecodedPcmOutput, FfmpegApiError> {
    use super::ffmpeg::loader::try_load;
    use super::ffmpeg::{DecodeSession, FfmpegLimits};
    let ffmpeg = try_load().map_err(|e| FfmpegApiError::Unavailable(e.to_string()))?;
    let native_limits = FfmpegLimits {
        max_source_bytes: limits.max_source_bytes,
        max_duration_us: limits.max_duration_us,
        max_audio_samples: limits.max_audio_samples,
        request_timeout_ms: limits.request_timeout_ms,
        ..FfmpegLimits::default()
    };
    let session = DecodeSession::open(&ffmpeg, bytes, native_limits)
        .map_err(|e| FfmpegApiError::Failed(format!("audio open: {e}")))?;
    let pcm = session
        .audio_pcm()
        .map_err(|e| FfmpegApiError::Failed(format!("audio decode: {e}")))?;
    session.close();
    Ok(DecodedPcmOutput {
        sample_rate: pcm.sample_rate,
        channels: pcm.channels,
        duration_secs: pcm.duration_secs(),
        samples: pcm.samples,
    })
}

#[cfg(media_ffmpeg)]
fn extract_video_frames_native(
    bytes: Vec<u8>,
    limits: DecodeLimits,
) -> Result<Vec<DecodedFrameOutput>, FfmpegApiError> {
    use super::ffmpeg::loader::try_load;
    use super::ffmpeg::{DecodeSession, FfmpegLimits};
    let ffmpeg = try_load().map_err(|e| FfmpegApiError::Unavailable(e.to_string()))?;
    let native_limits = FfmpegLimits {
        max_source_bytes: limits.max_source_bytes,
        max_duration_us: limits.max_duration_us,
        max_video_frames: limits.max_video_frames,
        request_timeout_ms: limits.request_timeout_ms,
        ..FfmpegLimits::default()
    };
    let session = DecodeSession::open(&ffmpeg, bytes, native_limits)
        .map_err(|e| FfmpegApiError::Failed(format!("video open: {e}")))?;
    let probe = session
        .probe()
        .map_err(|e| FfmpegApiError::Failed(format!("video probe: {e}")))?;
    let frame_count = (limits.max_video_frames.max(1) as usize).min(16);
    let duration_secs = probe.duration_us.unwrap_or(0) as f64 / 1_000_000.0;
    let mut frames: Vec<DecodedFrameOutput> = Vec::with_capacity(frame_count);
    for index in 0..frame_count {
        let timestamp = if duration_secs > 0.0 {
            (duration_secs * index as f64 / frame_count as f64).floor() as i64
        } else {
            index as i64
        };
        if let Ok(frame) = session.frame_at_seconds(timestamp) {
            frames.push(frame_output(frame));
        }
    }
    if frames.is_empty() {
        // Seek-less or unseekable sources (e.g. index-less AVI) still need
        // a deterministic frame set: fall back to sequential decode up to
        // the same cap.
        while frames.len() < frame_count {
            match session.next_frame() {
                Ok(frame) => frames.push(frame_output(frame)),
                Err(_) => break,
            }
        }
    }
    session.close();
    if frames.is_empty() {
        return Err(FfmpegApiError::Failed(
            "no video frames could be extracted".to_string(),
        ));
    }
    Ok(frames)
}

#[cfg(media_ffmpeg)]
fn frame_output(frame: super::ffmpeg::DecodedFrame) -> DecodedFrameOutput {
    // Compute the borrowed PTS conversion before moving `frame.data` into
    // the output (immutably borrowing `frame` after a partial move is E0382).
    let pts_us = frame.pts_us();
    DecodedFrameOutput {
        data: frame.data,
        width: frame.width,
        height: frame.height,
        stride: frame.stride,
        pts_us,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The default limits must mirror the native defaults so callers that
    /// only tune a subset keep safe bounds.
    #[test]
    fn decode_limits_defaults_are_safe() {
        let limits = DecodeLimits::default();
        assert!(limits.max_source_bytes >= 256 * 1024 * 1024);
        assert!(limits.max_duration_us >= 900 * 1_000_000);
        assert!(limits.max_video_frames > 0);
        assert!(limits.request_timeout_ms > 0);
        assert!(limits.max_audio_samples > 0);
    }

    /// Without the native backend compiled in, availability reports
    /// `CompiledOut` and every operation degrades to `Unavailable` — never
    /// a panic, never a subprocess.
    #[cfg(not(media_ffmpeg))]
    #[test]
    fn compiled_out_backend_reports_compiled_out_and_degrades() {
        assert_eq!(availability(), FfmpegAvailability::CompiledOut);
        assert!(matches!(
            decode_audio_pcm(Vec::new(), DecodeLimits::default()),
            Err(FfmpegApiError::Unavailable(_))
        ));
        assert!(matches!(
            extract_video_frames(Vec::new(), DecodeLimits::default()),
            Err(FfmpegApiError::Unavailable(_))
        ));
    }

    /// With the native backend compiled in, availability must report the
    /// real runtime state (`Available` when libraries load, `Unavailable`
    /// when they do not) — never `CompiledOut`.
    #[cfg(media_ffmpeg)]
    #[test]
    fn compiled_in_backend_reports_runtime_state() {
        match availability() {
            FfmpegAvailability::Available | FfmpegAvailability::Unavailable(_) => {}
            FfmpegAvailability::CompiledOut => {
                panic!("cfg(media_ffmpeg) is set; availability must never report CompiledOut")
            }
        }
    }

    /// Fail-closed runtime gate: when the backend was compiled in but its
    /// libraries cannot load, runtime tests panic loudly (a broken
    /// environment must not masquerade as decoder coverage). The documented
    /// `GROK_DISABLE_MEDIA_FFMPEG` kill switch is the only legitimate skip.
    #[cfg(media_ffmpeg)]
    fn runtime_available() -> bool {
        use super::super::ffmpeg::loader::{self, FfmpegLoadOutcome};
        if loader::disabled_by_env() {
            return false;
        }
        match loader::load_once() {
            FfmpegLoadOutcome::Loaded(_) => true,
            FfmpegLoadOutcome::Failed(err) => panic!(
                "native FFmpeg backend was compiled in (media_ffmpeg) but its runtime \
                 libraries could not be loaded: {err}. Install FFmpeg 8 runtime libraries \
                 or set GROK_DISABLE_MEDIA_FFMPEG=1 to skip these tests."
            ),
        }
    }

    /// PCM path executes end-to-end through the public API when the native
    /// backend is available: WAV in, normalized PCM out.
    #[cfg(media_ffmpeg)]
    #[test]
    fn audio_pcm_executes_when_backend_available() {
        if !runtime_available() {
            eprintln!(
                "skipping: GROK_DISABLE_MEDIA_FFMPEG is set; runtime FFmpeg tests are disabled"
            );
            return;
        }
        let wav = make_wav(44_100, 1, &sine_samples(8_000, 44_100, 440.0));
        let limits = DecodeLimits {
            max_source_bytes: 32 * 1024 * 1024,
            request_timeout_ms: 5_000,
            ..DecodeLimits::default()
        };
        let pcm = decode_audio_pcm(wav, limits).expect("wav decodes to pcm");
        assert_eq!(pcm.sample_rate, 44_100);
        assert_eq!(pcm.channels, 1);
        assert!(!pcm.samples.is_empty(), "pcm has samples");
        assert!(pcm.duration_secs > 0.0);
        assert!(pcm.samples.iter().all(|s| s.is_finite()));
    }

    /// Frames path executes end-to-end through the public API when the
    /// native backend is available: PNG (single-frame video stream) in,
    /// decoded RGB frames out.
    #[cfg(media_ffmpeg)]
    #[test]
    fn video_frames_execute_when_backend_available() {
        if !runtime_available() {
            eprintln!(
                "skipping: GROK_DISABLE_MEDIA_FFMPEG is set; runtime FFmpeg tests are disabled"
            );
            return;
        }
        let png = make_png(8, 8);
        let limits = DecodeLimits {
            max_source_bytes: 32 * 1024 * 1024,
            request_timeout_ms: 5_000,
            ..DecodeLimits::default()
        };
        let frames = extract_video_frames(png, limits).expect("png yields frames");
        assert!(!frames.is_empty(), "at least one frame");
        for frame in &frames {
            assert_eq!((frame.width, frame.height), (8, 8));
            assert!(frame.stride >= 8 * 3);
            assert_eq!(frame.data.len(), frame.stride as usize * 8);
        }
    }

    #[cfg(media_ffmpeg)]
    fn make_png(width: u32, height: u32) -> Vec<u8> {
        let mut img = image::RgbImage::new(width, height);
        for (x, y, pixel) in img.enumerate_pixels_mut() {
            *pixel = image::Rgb([(x * 47) as u8, (y * 71) as u8, 0xFF]);
        }
        let mut bytes = Vec::new();
        img.write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .expect("png encode");
        bytes
    }

    #[cfg(media_ffmpeg)]
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

    #[cfg(media_ffmpeg)]
    fn sine_samples(count: usize, sample_rate: u32, freq: f32) -> Vec<i16> {
        (0..count)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                (0.9 * (2.0 * std::f32::consts::PI * freq * t).sin() * i16::MAX as f32) as i16
            })
            .collect()
    }
}
