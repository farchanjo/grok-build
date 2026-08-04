//! Tests for the native FFmpeg preprocessing layer (plan PR 4).
//!
//! All tests in this file are compiled only when `cfg(media_ffmpeg)` is set,
//! i.e. when build.rs discovered compatible FFmpeg 8 headers. Runtime tests
//! are fail-closed: when the backend was compiled in but its runtime
//! libraries cannot be loaded, they fail loudly instead of silently passing
//! (a broken environment must not masquerade as decoder coverage). The
//! documented `GROK_DISABLE_MEDIA_FFMPEG` kill switch is the only reason a
//! runtime test skips. Decode fixtures are synthesized in pure Rust
//! (PNG/JPEG via the `image` crate, AVI and WAV containers hand-rolled) so
//! tests never depend on an ffmpeg CLI or on external model calls.

use super::abi;
use super::audio::DecodedPcm;
use super::decode::{DecodeSession, FfmpegLimits};
use super::error::FfmpegError;
use super::loader::{self, FfmpegLoadOutcome, LoadedFfmpeg};
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::fmt::format::FmtSpan;

/// The process-wide loaded FFmpeg, or `None` to skip runtime tests.
///
/// Fail-closed: this module is only compiled when `cfg(media_ffmpeg)` is
/// set (compatible FFmpeg 8 headers were present at build time), so a
/// runtime load failure is a broken environment, not a legitimate skip. A
/// load failure therefore panics (failing the test loudly) instead of
/// silently passing as if the decoder had been exercised. The documented
/// kill switch `GROK_DISABLE_MEDIA_FFMPEG` is the only legitimate skip
/// reason.
fn test_ffmpeg() -> Option<Arc<LoadedFfmpeg>> {
    if loader::disabled_by_env() {
        return None;
    }
    match loader::load_once() {
        FfmpegLoadOutcome::Loaded(loaded) => Some(Arc::clone(loaded)),
        FfmpegLoadOutcome::Failed(err) => panic!(
            "native FFmpeg backend was compiled in (media_ffmpeg) but its runtime \
             libraries could not be loaded: {err}. Install FFmpeg 8 runtime libraries \
             or set GROK_DISABLE_MEDIA_FFMPEG=1 to skip these tests."
        ),
    }
}

/// Lowered limits for fast, bounded tests.
fn test_limits() -> FfmpegLimits {
    FfmpegLimits {
        max_source_bytes: 32 * 1024 * 1024,
        max_pixels: 4096 * 4096,
        max_width: 4096,
        max_height: 4096,
        max_duration_us: 60 * 1_000_000,
        max_audio_samples: 1_000_000,
        max_video_frames: 32,
        max_frame_bytes: 64 * 1024 * 1024,
        request_timeout_ms: 5_000,
    }
}

// ------------------------------------------------------------------
// Build-script invariants
// ------------------------------------------------------------------

/// The build script must never emit FFmpeg link metadata: only
/// `pkg_config::Config::cargo_metadata(false)` for header/version discovery.
#[test]
fn build_script_emits_no_ffmpeg_link_metadata() {
    let build_rs =
        std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("build.rs"))
            .expect("build.rs readable");
    for forbidden in [
        "cargo:rustc-link-lib",
        "cargo:rustc-link-search",
        "cargo_metadata(true)",
    ] {
        assert!(
            !build_rs.contains(forbidden),
            "build.rs must not contain `{forbidden}`"
        );
    }
    assert!(
        build_rs.contains("cargo_metadata(false)"),
        "build.rs must discover FFmpeg via cargo_metadata(false)"
    );
    assert!(
        build_rs.contains("GROK_DISABLE_MEDIA_FFMPEG"),
        "build.rs must honor GROK_DISABLE_MEDIA_FFMPEG"
    );
    assert!(
        build_rs.contains("media_ffmpeg"),
        "build.rs must gate the native backend on cfg(media_ffmpeg)"
    );
}

/// Missing/incompatible headers must compile the backend out with a clear
/// `cargo:warning`, never a hard failure.
#[test]
fn build_script_compiles_out_without_headers() {
    let build_rs =
        std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("build.rs"))
            .expect("build.rs readable");
    assert!(
        build_rs.contains("cargo:warning=media_ffmpeg"),
        "build.rs must surface a warning diagnostic when headers are absent"
    );
    // The FFmpeg discovery path returns Ok(()) on every absence branch.
    assert!(
        build_rs.contains("Compiling the native backend out") || build_rs.contains("compiled out"),
        "build.rs absence branch must compile the backend out"
    );
}

// ------------------------------------------------------------------
// Loader behavior
// ------------------------------------------------------------------

#[test]
fn loader_without_libraries_fails_gracefully() {
    let err = loader::load_from_dirs(&[]).expect_err("empty search dirs must fail");
    assert!(
        matches!(err, FfmpegError::LibraryLoad { .. }),
        "got {err:?}"
    );
}

#[test]
fn loader_rejects_wrong_major_directly() {
    // major_from_version is the single source of truth; verify the constant
    // majors match the plan.
    let majors: Vec<u32> = loader::FFMPEG_LIBS.iter().map(|(_, m, _)| *m).collect();
    assert_eq!(majors, vec![60, 62, 62, 9, 6]);
}

#[test]
fn loader_resolves_avpacket_helpers_from_libavcodec() {
    // AVPacket helpers are exported by libavcodec (avcodec.h), not libavutil.
    // The loader previously resolved them from libraries[0] (libavutil),
    // which made load_from_dirs fail on every real FFmpeg 8 install and
    // silently skipped the runtime decode tests. Source-level invariant so
    // the grouping cannot silently regress (mirrors the build-script
    // invariant tests above).
    let loader_rs = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/media/ffmpeg/loader.rs"),
    )
    .expect("loader.rs readable");
    for symbol in ["av_packet_alloc", "av_packet_free", "av_packet_unref"] {
        assert!(
            loader_rs.contains(&format!(
                "{symbol}: get_symbol(&libraries[1], names[1], \"{symbol}\")"
            )),
            "loader.rs must resolve {symbol} from libavcodec (libraries[1])"
        );
    }
}

#[test]
fn resolve_library_path_skips_missing_dirs() {
    assert!(loader::resolve_library_path("libavutil", 60, &[]).is_none());
    assert!(
        loader::resolve_library_path("libavutil", 60, &[PathBuf::from("/definitely/not/here")],)
            .is_none()
    );
}

// ------------------------------------------------------------------
// Fixtures (pure Rust, no ffmpeg CLI)
// ------------------------------------------------------------------

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

// ------------------------------------------------------------------
// Video decode
// ------------------------------------------------------------------

#[test]
fn png_decodes_to_rgb_frame() {
    let Some(ffmpeg) = test_ffmpeg() else {
        eprintln!("skipping: GROK_DISABLE_MEDIA_FFMPEG is set; runtime FFmpeg tests are disabled");
        return;
    };
    let png = make_png(8, 8);
    let session = DecodeSession::open(&ffmpeg, png, test_limits()).expect("open png");
    let probe = session.probe().expect("probe png");
    assert!(probe.has_video, "png has a video stream");
    assert_eq!((probe.width, probe.height), (8, 8));

    let frame = session.next_frame().expect("decode first frame");
    assert_eq!((frame.width, frame.height), (8, 8));
    assert!(frame.stride >= 8 * 3);
    assert_eq!(frame.data.len(), frame.stride as usize * 8);
    // First pixel should not be black for a non-black PNG.
    assert!(frame.data.iter().any(|&b| b > 0), "frame has content");

    // frame_at_seconds must yield a frame or a graceful seek/decode error,
    // never a silent EndOfMedia after a successful seek+decode.
    match session.frame_at_seconds(0) {
        Ok(seek_frame) => assert_eq!((seek_frame.width, seek_frame.height), (8, 8)),
        Err(e) => {
            assert!(
                matches!(
                    e,
                    FfmpegError::Seek(_)
                        | FfmpegError::Native(_)
                        | FfmpegError::Unsupported(_)
                        | FfmpegError::DecodeFailed(_)
                ),
                "unexpected frame_at_seconds result: {e:?}"
            )
        }
    }

    // A single-frame PNG ends after one frame.
    assert_eq!(
        session.next_frame(),
        Err(FfmpegError::EndOfMedia),
        "png has exactly one frame"
    );

    // No audio stream.
    assert!(matches!(
        session.audio_pcm(),
        Err(FfmpegError::NoStream("audio"))
    ));
}

#[test]
fn avi_decodes_multiple_frames_sequentially() {
    let Some(ffmpeg) = test_ffmpeg() else {
        eprintln!("skipping: GROK_DISABLE_MEDIA_FFMPEG is set; runtime FFmpeg tests are disabled");
        return;
    };
    let frames = [
        make_jpeg_frame(6, 6, 0x10),
        make_jpeg_frame(6, 6, 0x50),
        make_jpeg_frame(6, 6, 0x90),
    ];
    let avi = make_avi(&frames, 6, 6);
    let session = DecodeSession::open(&ffmpeg, avi, test_limits()).expect("open avi");

    let probe = session.probe().expect("probe avi");
    assert!(probe.has_video);
    assert_eq!((probe.width, probe.height), (6, 6));
    assert!(probe.duration_us.is_some() || probe.duration_us.is_none());

    let f1 = session.next_frame().expect("frame 1");
    let f2 = session.next_frame().expect("frame 2");
    let f3 = session.next_frame().expect("frame 3");
    assert_eq!((f1.width, f1.height), (6, 6));
    assert_ne!(f1.data, f2.data, "frames differ");
    assert_ne!(f2.data, f3.data, "frames differ");

    // No fourth frame.
    assert_eq!(session.next_frame(), Err(FfmpegError::EndOfMedia));

    // frame_at_seconds on an index-less AVI either yields a frame or a
    // graceful error, never a silent EndOfMedia.
    match session.frame_at_seconds(0) {
        Ok(frame) => assert_eq!((frame.width, frame.height), (6, 6)),
        Err(e) => {
            assert!(
                matches!(
                    e,
                    FfmpegError::Seek(_)
                        | FfmpegError::Native(_)
                        | FfmpegError::Unsupported(_)
                        | FfmpegError::DecodeFailed(_)
                ),
                "unexpected seek result: {e:?}"
            )
        }
    }
}

#[test]
fn video_frame_cap_is_enforced() {
    let Some(ffmpeg) = test_ffmpeg() else {
        eprintln!("skipping: GROK_DISABLE_MEDIA_FFMPEG is set; runtime FFmpeg tests are disabled");
        return;
    };
    let frames = [
        make_jpeg_frame(4, 4, 0x10),
        make_jpeg_frame(4, 4, 0x40),
        make_jpeg_frame(4, 4, 0x70),
    ];
    let avi = make_avi(&frames, 4, 4);
    let mut limits = test_limits();
    limits.max_video_frames = 2;
    let session = DecodeSession::open(&ffmpeg, avi, limits).expect("open avi");
    session.next_frame().expect("frame 1 under cap");
    session.next_frame().expect("frame 2 under cap");
    let third = session.next_frame();
    assert!(
        matches!(third, Err(FfmpegError::Limit(_))),
        "frame 3 must trip the cap, got {third:?}"
    );
}

#[test]
fn pixel_cap_is_enforced() {
    let Some(ffmpeg) = test_ffmpeg() else {
        eprintln!("skipping: GROK_DISABLE_MEDIA_FFMPEG is set; runtime FFmpeg tests are disabled");
        return;
    };
    let png = make_png(64, 64);
    let mut limits = test_limits();
    limits.max_pixels = 16 * 16; // far below 64x64
    let session = DecodeSession::open(&ffmpeg, png, limits).expect("open png");
    let first = session.next_frame();
    assert!(
        matches!(first, Err(FfmpegError::Limit(_))),
        "oversized frame must trip the pixel cap, got {first:?}"
    );
}

// ------------------------------------------------------------------
// Audio decode
// ------------------------------------------------------------------

#[test]
fn wav_decodes_to_normalized_pcm() {
    let Some(ffmpeg) = test_ffmpeg() else {
        eprintln!("skipping: GROK_DISABLE_MEDIA_FFMPEG is set; runtime FFmpeg tests are disabled");
        return;
    };
    let samples = sine_samples(8000, 44100, 440.0);
    let wav = make_wav(44100, 1, &samples);
    let session = DecodeSession::open(&ffmpeg, wav, test_limits()).expect("open wav");
    let probe = session.probe().expect("probe wav");
    assert!(probe.has_audio);
    assert_eq!(probe.sample_rate, 44100);
    assert_eq!(probe.channels, 1);

    let pcm = session.audio_pcm().expect("decode pcm");
    assert_eq!(pcm.sample_rate, 44100);
    assert_eq!(pcm.channels, 1);
    assert!(!pcm.truncated);
    assert!(!pcm.samples.is_empty(), "pcm has samples");
    // Normalized to [-1,1] with the sine amplitude ~0.9.
    let peak = pcm.samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    assert!(
        (0.7..=1.05).contains(&peak),
        "peak {peak} in expected range"
    );
    // No NaN or inf.
    assert!(pcm.samples.iter().all(|s| s.is_finite()));
}

#[test]
fn audio_cap_truncates_output() {
    let Some(ffmpeg) = test_ffmpeg() else {
        eprintln!("skipping: GROK_DISABLE_MEDIA_FFMPEG is set; runtime FFmpeg tests are disabled");
        return;
    };
    let samples = sine_samples(8000, 44100, 440.0);
    let wav = make_wav(44100, 1, &samples);
    let mut limits = test_limits();
    limits.max_audio_samples = 200;
    let session = DecodeSession::open(&ffmpeg, wav, limits).expect("open wav");
    let pcm: DecodedPcm = session.audio_pcm().expect("decode truncated pcm");
    assert!(pcm.truncated, "cap must be reported as truncation");
    assert!(pcm.samples.len() <= 200, "bounded by cap");
    assert_eq!(pcm.channels, 1);
}

// ------------------------------------------------------------------
// Duration cap (max_duration_us)
// ------------------------------------------------------------------

/// The fixture's probed duration in microseconds.
///
/// WAV PCM exposes its exact byte length and byte rate, so FFmpeg reports a
/// reliable, deterministic duration derived from the `data` chunk. Reusing
/// the probed value (instead of recomputing it) keeps the boundary tests
/// robust against demuxer-specific rounding.
fn probed_duration_us(ffmpeg: &Arc<LoadedFfmpeg>, media: &[u8]) -> i64 {
    let session = DecodeSession::open(ffmpeg, media.to_vec(), test_limits()).expect("open media");
    let duration = session
        .probe()
        .expect("probe media")
        .duration_us
        .expect("media duration is reliable");
    session.close();
    duration
}

#[test]
fn over_duration_media_is_rejected_at_open() {
    let Some(ffmpeg) = test_ffmpeg() else {
        eprintln!("skipping: GROK_DISABLE_MEDIA_FFMPEG is set; runtime FFmpeg tests are disabled");
        return;
    };
    let wav = make_wav(44100, 1, &sine_samples(8000, 44100, 440.0));
    let duration_us = probed_duration_us(&ffmpeg, &wav);
    assert!(
        duration_us > 0,
        "wav fixture must have a positive reliable duration"
    );

    let mut limits = test_limits();
    limits.max_duration_us = (duration_us - 1) as u64;
    assert!(
        matches!(
            DecodeSession::open(&ffmpeg, wav, limits),
            Err(FfmpegError::Limit(_))
        ),
        "media whose probed duration exceeds max_duration_us must fail at open"
    );
}

#[test]
fn exact_duration_bound_is_accepted() {
    let Some(ffmpeg) = test_ffmpeg() else {
        eprintln!("skipping: GROK_DISABLE_MEDIA_FFMPEG is set; runtime FFmpeg tests are disabled");
        return;
    };
    let wav = make_wav(44100, 1, &sine_samples(8000, 44100, 440.0));
    let duration_us = probed_duration_us(&ffmpeg, &wav);
    assert!(
        duration_us > 0,
        "wav fixture must have a positive reliable duration"
    );

    let mut limits = test_limits();
    limits.max_duration_us = duration_us as u64;
    let session = DecodeSession::open(&ffmpeg, wav, limits)
        .expect("media at the exact duration bound must open");
    // The session still decodes normally past the gate.
    let pcm = session.audio_pcm().expect("decode at exact bound");
    assert!(!pcm.samples.is_empty(), "pcm has samples");
}

#[test]
fn unknown_duration_media_passes_the_duration_gate() {
    let Some(ffmpeg) = test_ffmpeg() else {
        eprintln!("skipping: GROK_DISABLE_MEDIA_FFMPEG is set; runtime FFmpeg tests are disabled");
        return;
    };
    // A single still image has no container duration: the png_pipe demuxer
    // leaves the stream duration at AV_NOPTS_VALUE and no bitrate/PTS-based
    // estimate exists, so `fmt->duration` stays AV_NOPTS_VALUE after
    // avformat_find_stream_info. The shim's duration gate must treat that
    // as "unknown" and let the session open.
    let png = make_png(8, 8);
    let session = DecodeSession::open(&ffmpeg, png.clone(), test_limits()).expect("open png");
    let probe = session.probe().expect("probe png");
    session.close();
    assert!(
        probe.duration_us.is_none(),
        "a single PNG must report an unknown duration; got {:?}",
        probe.duration_us
    );

    // A cap far below any real media duration must not reject it.
    let mut limits = test_limits();
    limits.max_duration_us = 1;
    let session = DecodeSession::open(&ffmpeg, png, limits)
        .expect("unknown-duration media must pass the duration gate");
    session.close();
}

#[test]
fn duration_gate_leaves_cancellation_unaffected() {
    let Some(ffmpeg) = test_ffmpeg() else {
        eprintln!("skipping: GROK_DISABLE_MEDIA_FFMPEG is set; runtime FFmpeg tests are disabled");
        return;
    };
    // A session accepted at the exact duration bound must still honor
    // cooperative cancellation: the gate must not alter the cancellation
    // path.
    let wav = make_wav(44100, 1, &sine_samples(8000, 44100, 440.0));
    let duration_us = probed_duration_us(&ffmpeg, &wav);
    let mut limits = test_limits();
    limits.max_duration_us = duration_us as u64;
    let session = DecodeSession::open(&ffmpeg, wav, limits).expect("wav at the exact bound opens");
    session.cancel();
    assert_eq!(
        session.audio_pcm(),
        Err(FfmpegError::Cancelled),
        "cancellation must not be affected by the duration gate"
    );
}

// ------------------------------------------------------------------
// Cancellation and lifecycle
// ------------------------------------------------------------------

#[test]
fn cooperative_cancellation_interrupts_decode() {
    let Some(ffmpeg) = test_ffmpeg() else {
        eprintln!("skipping: GROK_DISABLE_MEDIA_FFMPEG is set; runtime FFmpeg tests are disabled");
        return;
    };
    let png = make_png(8, 8);
    let session = DecodeSession::open(&ffmpeg, png, test_limits()).expect("open png");
    session.cancel();
    // The worker observes the C-owned atomic before reading a packet.
    assert_eq!(
        session.next_frame(),
        Err(FfmpegError::Cancelled),
        "cancelled session yields Cancelled"
    );
}

#[test]
fn closed_session_rejects_new_requests() {
    let Some(ffmpeg) = test_ffmpeg() else {
        eprintln!("skipping: GROK_DISABLE_MEDIA_FFMPEG is set; runtime FFmpeg tests are disabled");
        return;
    };
    let png = make_png(8, 8);
    let session = DecodeSession::open(&ffmpeg, png, test_limits()).expect("open png");
    session.close();
    assert_eq!(session.probe(), Err(FfmpegError::Closed));
}

#[test]
fn empty_and_oversized_inputs_are_rejected() {
    let Some(ffmpeg) = test_ffmpeg() else {
        eprintln!("skipping: GROK_DISABLE_MEDIA_FFMPEG is set; runtime FFmpeg tests are disabled");
        return;
    };
    assert!(matches!(
        DecodeSession::open(&ffmpeg, Vec::new(), test_limits()),
        Err(FfmpegError::OpenFailed(_))
    ));
    let mut limits = test_limits();
    limits.max_source_bytes = 10;
    assert!(matches!(
        DecodeSession::open(&ffmpeg, make_png(8, 8), limits),
        Err(FfmpegError::Limit(_))
    ));
}

// ------------------------------------------------------------------
// Malformed inputs: bounded smoke fuzz (PR 4 fuzz harness seed; PR 10
// extends to ASan/UBSan corpus runs)
// ------------------------------------------------------------------

#[test]
fn malformed_inputs_never_panic_or_hang() {
    let Some(ffmpeg) = test_ffmpeg() else {
        eprintln!("skipping: GROK_DISABLE_MEDIA_FFMPEG is set; runtime FFmpeg tests are disabled");
        return;
    };

    let mut corpus: Vec<Vec<u8>> = Vec::new();
    // Truncated / mutated versions of the valid fixtures.
    let png = make_png(8, 8);
    let wav = make_wav(44100, 1, &sine_samples(1000, 44100, 440.0));
    for cut in [0, 1, 8, 30, png.len() / 2] {
        corpus.push(png[..cut.min(png.len())].to_vec());
    }
    for cut in [1, 12, wav.len() / 2] {
        corpus.push(wav[..cut.min(wav.len())].to_vec());
    }
    // Deterministic pseudo-random garbage with a PNG/RIFF magic prefix.
    let mut state = 0x1234_5678u32;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state
    };
    let magics: [&[u8]; 5] = [
        b"RIFF".as_slice(),
        b"\x89PNG".as_slice(),
        b"GIF89a".as_slice(),
        b"ID3".as_slice(),
        b"OggS".as_slice(),
    ];
    for magic in magics {
        let mut blob = magic.to_vec();
        for _ in 0..1024 {
            blob.push((next() & 0xFF) as u8);
        }
        corpus.push(blob);
    }
    // Plain text.
    corpus.push(b"this is not media at all".to_vec());

    // Regression guard: at least one opened session that reports a video
    // stream must produce a non-EndOfMedia `next_frame` result. Before the
    // EOF-on-success fix, `grok_av_decode_video` left `status` at
    // GROK_AV_ERR_EOF on every successful frame, so every video session
    // returned EndOfMedia without ever decoding — the silent short-circuit
    // that let the decoder bug through validation.
    let mut video_decoder_exercised = false;

    for (idx, input) in corpus.iter().enumerate() {
        let limits = FfmpegLimits {
            max_source_bytes: 32 * 1024 * 1024,
            request_timeout_ms: 3_000,
            ..test_limits()
        };
        // Open may succeed (header looks plausible) or fail gracefully; it
        // must never panic.
        let Ok(session) = DecodeSession::open(&ffmpeg, input.clone(), limits) else {
            continue;
        };
        let probe = session.probe();
        let next = session.next_frame();
        if probe.as_ref().is_ok_and(|p| p.has_video) && next != Err(FfmpegError::EndOfMedia) {
            video_decoder_exercised = true;
        }
        let _ = session.frame_at_seconds(0);
        let _ = session.audio_pcm();
        session.close();
        let _ = idx;
    }

    assert!(
        video_decoder_exercised,
        "malformed corpus must reach the video decoder for at least one opened video \
         session; a universal EndOfMedia short-circuit means the decoder is broken"
    );
}

// ------------------------------------------------------------------
// Diagnostics
// ------------------------------------------------------------------

#[test]
fn error_category_mapping_is_stable() {
    assert_eq!(FfmpegError::EndOfMedia.category(), "eof");
    assert_eq!(FfmpegError::Cancelled.category(), "cancelled");
    assert_eq!(FfmpegError::Limit("x".to_string()).category(), "limit");
    assert_eq!(
        FfmpegError::Unavailable("x".to_string()).category(),
        "unavailable"
    );
    assert_eq!(
        FfmpegError::VersionMismatch {
            library: "libavutil".to_string(),
            expected: 60,
            found: 59,
        }
        .category(),
        "abi"
    );
}

#[test]
fn native_code_maps_to_typed_errors() {
    use super::error::from_native_code;
    assert_eq!(
        from_native_code(abi::GROK_AV_ERR_EOF, "eof"),
        FfmpegError::EndOfMedia
    );
    assert_eq!(
        from_native_code(abi::GROK_AV_ERR_CANCELLED, "cancelled"),
        FfmpegError::Cancelled
    );
    assert!(matches!(
        from_native_code(abi::GROK_AV_ERR_LIMIT, "pixel cap"),
        FfmpegError::Limit(_)
    ));
    assert!(matches!(
        from_native_code(abi::GROK_AV_ERR_OPEN, "bad container"),
        FfmpegError::OpenFailed(_)
    ));
}

#[test]
fn limits_round_trip_to_native() {
    let limits = test_limits();
    let native = limits.into_native();
    assert_eq!(native.max_source_bytes, limits.max_source_bytes);
    assert_eq!(native.max_pixels, limits.max_pixels);
    assert_eq!(native.max_video_frames, limits.max_video_frames);
}

#[test]
fn pcm_helpers_are_consistent() {
    let pcm = DecodedPcm {
        samples: vec![0.0, 0.5, 1.0, -1.0],
        sample_rate: 2,
        channels: 2,
        truncated: false,
    };
    assert_eq!(pcm.frames(), 2);
    assert_eq!(pcm.duration_secs(), 1.0);
    let mono = pcm.to_mono();
    assert_eq!(mono.len(), 2);
    assert_eq!(mono[0], 0.25);
    assert_eq!(mono[1], 0.0);
}

#[test]
fn diagnostics_never_panic() {
    let _ = loader::diagnostics();
    let _ = loader::is_loaded();
}

#[test]
fn diagnostics_library_names_are_redacted_to_base_names() {
    // Plan section 17: diagnostics must never expose full local paths. When
    // the backend is loaded, `FfmpegDiagnostics.libraries` must contain only
    // base file names (e.g. `libavutil.60.dylib`), never absolute paths.
    let _ = loader::load_once();
    let diag = loader::diagnostics();
    for name in &diag.libraries {
        assert!(
            !name.contains('/') && !name.contains('\\'),
            "diagnostics leaked a full path: {name:?}"
        );
        assert!(!name.is_empty(), "diagnostics entry must not be empty");
        assert!(
            name.starts_with("lib"),
            "diagnostics entry must be a library base name, got {name:?}"
        );
    }
}

// ------------------------------------------------------------------
// PR 10 hardening
// ------------------------------------------------------------------

/// Direct kill-switch test for `GROK_DISABLE_MEDIA_FFMPEG` (plan section 5.5).
///
/// The loader consults the env predicate on every call and before every
/// runtime load. This test proves the predicate itself is honored. It does
/// not call `load_once` after mutation (the process-wide `OnceLock` would
/// make that order-dependent under shared-process `cargo test`), so it is
/// safe under both nextest (one test per process) and cargo test.
#[test]
fn ffmpeg_kill_switch_env_var_directly_disables_loader() {
    let previous = std::env::var_os("GROK_DISABLE_MEDIA_FFMPEG");
    unsafe {
        std::env::set_var("GROK_DISABLE_MEDIA_FFMPEG", "1");
    }
    assert!(
        loader::disabled_by_env(),
        "GROK_DISABLE_MEDIA_FFMPEG=1 must disable the loader predicate"
    );
    unsafe {
        std::env::remove_var("GROK_DISABLE_MEDIA_FFMPEG");
    }
    assert!(
        !loader::disabled_by_env(),
        "unsetting GROK_DISABLE_MEDIA_FFMPEG must re-enable the loader predicate"
    );
    match previous {
        Some(value) => unsafe {
            std::env::set_var("GROK_DISABLE_MEDIA_FFMPEG", value);
        },
        None => {}
    }
}

/// Emitted trace fields for `media.ffmpeg.load` must never contain absolute
/// library paths or separators (plan section 17).
///
/// The `load_once` span fires on every call with only the non-secret
/// `disabled` field; whatever else appears in the captured trace must not
/// contain a path separator or a marker.
#[test]
fn ffmpeg_load_trace_never_leaks_paths() {
    #[derive(Clone)]
    struct CaptureWriter(Arc<std::sync::Mutex<Vec<u8>>>);
    impl std::io::Write for CaptureWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for CaptureWriter {
        type Writer = CaptureWriter;
        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    let captured = CaptureWriter(Arc::new(std::sync::Mutex::new(Vec::new())));
    let subscriber = tracing_subscriber::fmt()
        .with_writer(captured.clone())
        .with_max_level(tracing::Level::DEBUG)
        // `load_once` emits no events, only the `media.ffmpeg.load` span
        // lifecycle; synthesize span open/close events so the capture is
        // non-vacuous. The span's only field is the non-secret `disabled`
        // kill-switch state, so the output contains no paths or library
        // names.
        .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
        .finish();

    tracing::subscriber::with_default(subscriber, || {
        let _ = loader::diagnostics();
        let _ = loader::load_once();
    });

    let text = String::from_utf8_lossy(&captured.0.lock().unwrap()).to_string();
    assert!(
        !text.contains('/') && !text.contains('\\'),
        "media.ffmpeg.load trace leaked a path separator: {text}"
    );
    assert!(
        !text.contains(".dylib") && !text.contains(".so."),
        "media.ffmpeg.load trace leaked a library file name: {text}"
    );
    assert!(
        text.contains("media.ffmpeg.load"),
        "media.ffmpeg.load span must be emitted: {text}"
    );
}
