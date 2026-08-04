#![no_main]

use libfuzzer_sys::fuzz_target;

/// Always-on surface: the content-only request and source types must
/// deserialize arbitrary bytes without panicking (unknown fields ignored,
/// wrong shapes error). These are public `xai-grok-tools` types.
fn fuzz_deserialization(data: &[u8]) {
    let _ = serde_json::from_slice::<xai_grok_tools::media::backend::MediaUnderstandingRequest>(
        data,
    );
    let _ = serde_json::from_slice::<xai_grok_tools::media::domain::MediaSource>(data);
}

/// Native FFmpeg decode surface. Compiled only when this crate's build.rs
/// emits `cfg(media_ffmpeg)` (compatible FFmpeg 8 headers available and the
/// `GROK_DISABLE_MEDIA_FFMPEG` kill switch unset).
#[cfg(media_ffmpeg)]
mod native {
    use xai_grok_tools::media::ffmpeg::loader::try_load;
    use xai_grok_tools::media::ffmpeg::{DecodeSession, FfmpegLimits};

    /// Bounded limits: the native layer enforces byte/pixel/dimension/
    /// duration/frame/PCM caps plus a per-request wall-clock deadline, so a
    /// fuzz iteration can never run away.
    pub(crate) fn run(data: &[u8]) {
        let Ok(ffmpeg) = try_load() else {
            // Runtime FFmpeg libraries missing: nothing to fuzz. The harness
            // still builds and runs; coverage simply stays empty.
            return;
        };
        let limits = FfmpegLimits {
            max_source_bytes: 16 * 1024 * 1024,
            max_pixels: 4096 * 4096,
            max_width: 4096,
            max_height: 4096,
            max_duration_us: 60 * 1_000_000,
            max_audio_samples: 1_000_000,
            max_video_frames: 32,
            max_frame_bytes: 64 * 1024 * 1024,
            request_timeout_ms: 5_000,
        };
        let Ok(session) = DecodeSession::open(&ffmpeg, data.to_vec(), limits) else {
            return;
        };
        let _ = session.probe();
        let _ = session.next_frame();
        let _ = session.frame_at_seconds(0);
        let _ = session.audio_pcm();
        session.close();
    }
}

/// Degraded arm when `cfg(media_ffmpeg)` is unset: the native decode API
/// does not exist in this build, so the target is a no-op for the native
/// surface and only the deserialization surface runs.
#[cfg(not(media_ffmpeg))]
mod native {
    pub(crate) fn run(_data: &[u8]) {}
}

fuzz_target!(|data: &[u8]| {
    fuzz_deserialization(data);
    native::run(data);
});