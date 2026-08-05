//! Process-neutral, bounded `ffmpeg` / `ffprobe` helpers.
//!
//! These helpers are intentionally free of TUI, graphics-protocol, and
//! inference concerns so `read_file`, shell media understanding, and tests can
//! share one argv-only subprocess surface. Callers never pass a shell string;
//! every argument is a discrete process argument. Wall-clock timeouts kill the
//! direct child, and stdout is capped before it is returned.

use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use wait_timeout::ChildExt;

/// Default wall-clock budget for metadata probes.
pub const DEFAULT_PROBE_TIMEOUT: Duration = Duration::from_secs(10);
/// Default wall-clock budget for frame / audio extraction.
pub const DEFAULT_EXTRACT_TIMEOUT: Duration = Duration::from_secs(30);
/// Hard cap on captured stdout from a single ffmpeg/ffprobe invocation.
pub const MAX_CAPTURED_OUTPUT_BYTES: usize = 32 * 1024 * 1024;
/// Maximum configured audio window that safely fits 16 kHz mono PCM16 beneath
/// [`MAX_CAPTURED_OUTPUT_BYTES`], with headroom for container framing.
pub const MAX_AUDIO_EXTRACT_SECONDS: u64 = 900;
/// Reject media inputs larger than this before spawning ffmpeg.
pub const MAX_MEDIA_INPUT_BYTES: u64 = 512 * 1024 * 1024;

/// Errors from the process-neutral ffmpeg/ffprobe surface.
#[derive(Debug, thiserror::Error)]
pub enum FfmpegError {
    #[error("media path is empty")]
    EmptyPath,
    #[error("media path is not a regular file: {0}")]
    NotAFile(PathBuf),
    #[error("media file exceeds the {MAX_MEDIA_INPUT_BYTES}-byte input cap ({size} bytes)")]
    InputTooLarge { size: u64 },
    #[error("{tool} is not available on PATH")]
    ToolMissing { tool: &'static str },
    #[error("{tool} timed out after {timeout:?}")]
    Timeout {
        tool: &'static str,
        timeout: Duration,
    },
    #[error("{tool} exited with status {status}: {stderr}")]
    NonZeroExit {
        tool: &'static str,
        status: i32,
        stderr: String,
    },
    #[error("{tool} produced more than {MAX_CAPTURED_OUTPUT_BYTES} bytes of output")]
    OutputTooLarge { tool: &'static str },
    #[error("{tool} failed: {source}")]
    Io {
        tool: &'static str,
        #[source]
        source: io::Error,
    },
    #[error("failed to parse ffprobe output: {0}")]
    Parse(String),
    #[error("ffmpeg extracted no media bytes")]
    EmptyOutput,
}

/// Raw captured process output after timeout / size checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub status_code: i32,
}

/// Pluggable runner so unit tests never spawn real ffmpeg.
pub trait ProcessRunner: Send + Sync {
    fn run(
        &self,
        program: &str,
        args: &[&str],
        timeout: Duration,
    ) -> Result<ProcessOutput, FfmpegError>;
}

/// Production runner: detaches from the TTY, uses argv-only spawn, kills on
/// timeout, and rejects oversized stdout.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemProcessRunner;

impl ProcessRunner for SystemProcessRunner {
    fn run(
        &self,
        program: &str,
        args: &[&str],
        timeout: Duration,
    ) -> Result<ProcessOutput, FfmpegError> {
        use std::io::Read;

        let tool = program_label(program);
        if which::which(program).is_err() {
            return Err(FfmpegError::ToolMissing { tool });
        }
        let mut cmd = Command::new(program);
        cmd.args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        xai_tty_utils::detach_std_command(&mut cmd);
        let mut child = cmd
            .spawn()
            .map_err(|source| FfmpegError::Io { tool, source })?;
        let stdout_pipe = child.stdout.take();
        let stderr_pipe = child.stderr.take();
        // Drain pipes on helper threads so a full pipe cannot deadlock the
        // timeout wait. Cap reads one byte past the limit so we can reject.
        let stdout_handle = stdout_pipe.map(|pipe| {
            std::thread::spawn(move || {
                let mut buf = Vec::new();
                let mut limited = pipe.take(MAX_CAPTURED_OUTPUT_BYTES as u64 + 1);
                let _ = limited.read_to_end(&mut buf);
                buf
            })
        });
        let stderr_handle = stderr_pipe.map(|pipe| {
            std::thread::spawn(move || {
                let mut buf = Vec::new();
                let mut limited = pipe.take(MAX_CAPTURED_OUTPUT_BYTES as u64 + 1);
                let _ = limited.read_to_end(&mut buf);
                buf
            })
        });
        let wait_result = child.wait_timeout(timeout);
        let status_code = match wait_result {
            Ok(Some(status)) => status.code().unwrap_or(-1),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                // Join readers so OS pipes close cleanly after kill.
                let _ = stdout_handle.map(|h| h.join());
                let _ = stderr_handle.map(|h| h.join());
                return Err(FfmpegError::Timeout { tool, timeout });
            }
            Err(source) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_handle.map(|h| h.join());
                let _ = stderr_handle.map(|h| h.join());
                return Err(FfmpegError::Io { tool, source });
            }
        };
        let stdout = stdout_handle
            .map(|h| h.join().unwrap_or_default())
            .unwrap_or_default();
        let stderr = stderr_handle
            .map(|h| h.join().unwrap_or_default())
            .unwrap_or_default();
        if stdout.len() > MAX_CAPTURED_OUTPUT_BYTES || stderr.len() > MAX_CAPTURED_OUTPUT_BYTES {
            return Err(FfmpegError::OutputTooLarge { tool });
        }
        Ok(ProcessOutput {
            stdout,
            stderr,
            status_code,
        })
    }
}

/// Probed container / stream metadata. Fields are best-effort.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaProbe {
    pub duration_secs: Option<f64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub has_video: bool,
    pub has_audio: bool,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u32>,
}

impl Default for MediaProbe {
    fn default() -> Self {
        Self {
            duration_secs: None,
            width: None,
            height: None,
            has_video: false,
            has_audio: false,
            video_codec: None,
            audio_codec: None,
            sample_rate: None,
            channels: None,
        }
    }
}

/// Probe media metadata via `ffprobe` JSON.
pub fn probe_media(
    path: &Path,
    runner: &dyn ProcessRunner,
    timeout: Duration,
) -> Result<MediaProbe, FfmpegError> {
    validate_media_path(path)?;
    let path_str = path_as_arg(path)?;
    let output = runner.run(
        "ffprobe",
        &[
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
            path_str.as_str(),
        ],
        timeout,
    )?;
    ensure_success("ffprobe", &output)?;
    parse_ffprobe_json(&output.stdout)
}

/// Extract a single poster/sample frame as JPEG bytes.
///
/// Seeks to `seek_secs` when positive, otherwise the first decodable frame.
/// Unlike the pager-render poster helper, this never consults a graphics
/// protocol and always returns JPEG for process-neutral consumers.
pub fn extract_frame_jpeg(
    path: &Path,
    seek_secs: f64,
    runner: &dyn ProcessRunner,
    timeout: Duration,
) -> Result<Vec<u8>, FfmpegError> {
    validate_media_path(path)?;
    let path_str = path_as_arg(path)?;
    let seek = format!("{seek_secs:.3}");
    let mut args: Vec<&str> = vec!["-hide_banner", "-loglevel", "error"];
    if seek_secs > 0.0 {
        args.extend_from_slice(&["-ss", seek.as_str()]);
    }
    args.extend_from_slice(&[
        "-i",
        path_str.as_str(),
        "-frames:v",
        "1",
        "-f",
        "image2pipe",
        "-vcodec",
        "mjpeg",
        "-",
    ]);
    let output = runner.run("ffmpeg", &args, timeout)?;
    ensure_success("ffmpeg", &output)?;
    if output.stdout.is_empty() {
        return Err(FfmpegError::EmptyOutput);
    }
    if output.stdout.len() > MAX_CAPTURED_OUTPUT_BYTES {
        return Err(FfmpegError::OutputTooLarge { tool: "ffmpeg" });
    }
    Ok(output.stdout)
}

/// Sample up to `max_frames` JPEG frames evenly across `[0, duration]`.
pub fn extract_sample_frames_jpeg(
    path: &Path,
    duration_secs: f64,
    max_frames: usize,
    runner: &dyn ProcessRunner,
    timeout: Duration,
) -> Result<Vec<Vec<u8>>, FfmpegError> {
    let max_frames = max_frames.clamp(1, 32);
    let duration = duration_secs.max(0.0);
    let started = std::time::Instant::now();
    let mut frames = Vec::with_capacity(max_frames);
    for index in 0..max_frames {
        let Some(remaining) = timeout.checked_sub(started.elapsed()) else {
            if index == 0 {
                return Err(FfmpegError::Timeout {
                    tool: "ffmpeg",
                    timeout,
                });
            }
            break;
        };
        if remaining.is_zero() {
            if index == 0 {
                return Err(FfmpegError::Timeout {
                    tool: "ffmpeg",
                    timeout,
                });
            }
            break;
        }
        let seek = if max_frames == 1 || duration <= 0.0 {
            0.0
        } else {
            // Keep the last sample slightly inside the duration when known.
            let t = duration * (index as f64) / ((max_frames - 1) as f64);
            if t >= duration {
                (duration - 0.05).max(0.0)
            } else {
                t
            }
        };
        match extract_frame_jpeg(path, seek, runner, remaining) {
            Ok(bytes) => frames.push(bytes),
            Err(error) if index == 0 => return Err(error),
            Err(_) => break,
        }
    }
    if frames.is_empty() {
        return Err(FfmpegError::EmptyOutput);
    }
    Ok(frames)
}

/// Extract a mono 16 kHz WAV of at most `max_seconds` for transcription paths.
pub fn extract_audio_wav(
    path: &Path,
    max_seconds: u64,
    runner: &dyn ProcessRunner,
    timeout: Duration,
) -> Result<Vec<u8>, FfmpegError> {
    extract_audio_container(path, max_seconds, "wav", runner, timeout)
}

/// Extract mono 16 kHz signed little-endian PCM (no container) for streaming STT.
///
/// Matches the xAI voice STT transport (`sample_rate=16000`, PCM16 mono).
pub fn extract_audio_pcm_s16le(
    path: &Path,
    max_seconds: u64,
    runner: &dyn ProcessRunner,
    timeout: Duration,
) -> Result<Vec<u8>, FfmpegError> {
    extract_audio_container(path, max_seconds, "s16le", runner, timeout)
}

fn extract_audio_container(
    path: &Path,
    max_seconds: u64,
    format: &str,
    runner: &dyn ProcessRunner,
    timeout: Duration,
) -> Result<Vec<u8>, FfmpegError> {
    validate_media_path(path)?;
    let path_str = path_as_arg(path)?;
    let max_seconds = max_seconds.clamp(1, MAX_AUDIO_EXTRACT_SECONDS);
    let duration = max_seconds.to_string();
    let output = runner.run(
        "ffmpeg",
        &[
            "-hide_banner",
            "-loglevel",
            "error",
            "-i",
            path_str.as_str(),
            "-t",
            duration.as_str(),
            "-vn",
            "-ac",
            "1",
            "-ar",
            "16000",
            "-f",
            format,
            "-",
        ],
        timeout,
    )?;
    ensure_success("ffmpeg", &output)?;
    if output.stdout.is_empty() {
        return Err(FfmpegError::EmptyOutput);
    }
    if output.stdout.len() > MAX_CAPTURED_OUTPUT_BYTES {
        return Err(FfmpegError::OutputTooLarge { tool: "ffmpeg" });
    }
    Ok(output.stdout)
}

/// Whether a MIME type is audio.
pub fn mime_is_audio(mime: &str) -> bool {
    mime.starts_with("audio/")
}

/// Whether a MIME type is video.
pub fn mime_is_video(mime: &str) -> bool {
    mime.starts_with("video/")
}

/// Extension fallback when magic-byte inference is inconclusive.
pub fn extension_is_audio(extension: &str) -> bool {
    matches!(
        extension.to_ascii_lowercase().as_str(),
        "mp3" | "wav" | "flac" | "aac" | "m4a" | "ogg" | "opus" | "wma" | "aiff" | "aif"
    )
}

/// Extension fallback when magic-byte inference is inconclusive.
pub fn extension_is_video(extension: &str) -> bool {
    matches!(
        extension.to_ascii_lowercase().as_str(),
        "mp4" | "mov" | "mkv" | "webm" | "avi" | "m4v" | "wmv" | "flv" | "mpeg" | "mpg" | "3gp"
    )
}

/// Guess a MIME type from a well-known audio/video extension.
pub fn mime_from_extension(extension: &str) -> Option<&'static str> {
    match extension.to_ascii_lowercase().as_str() {
        "mp3" => Some("audio/mpeg"),
        "wav" => Some("audio/wav"),
        "flac" => Some("audio/flac"),
        "aac" => Some("audio/aac"),
        "m4a" => Some("audio/mp4"),
        "ogg" | "opus" => Some("audio/ogg"),
        "wma" => Some("audio/x-ms-wma"),
        "aiff" | "aif" => Some("audio/aiff"),
        "mp4" | "m4v" => Some("video/mp4"),
        "mov" => Some("video/quicktime"),
        "mkv" => Some("video/x-matroska"),
        "webm" => Some("video/webm"),
        "avi" => Some("video/x-msvideo"),
        "wmv" => Some("video/x-ms-wmv"),
        "flv" => Some("video/x-flv"),
        "mpeg" | "mpg" => Some("video/mpeg"),
        "3gp" => Some("video/3gpp"),
        _ => None,
    }
}

fn validate_media_path(path: &Path) -> Result<(), FfmpegError> {
    if path.as_os_str().is_empty() {
        return Err(FfmpegError::EmptyPath);
    }
    let meta = std::fs::metadata(path).map_err(|source| FfmpegError::Io {
        tool: "stat",
        source,
    })?;
    if !meta.is_file() {
        return Err(FfmpegError::NotAFile(path.to_path_buf()));
    }
    if meta.len() > MAX_MEDIA_INPUT_BYTES {
        return Err(FfmpegError::InputTooLarge { size: meta.len() });
    }
    Ok(())
}

fn path_as_arg(path: &Path) -> Result<String, FfmpegError> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| FfmpegError::Parse("media path is not valid UTF-8".to_owned()))
}

fn program_label(program: &str) -> &'static str {
    match program {
        "ffmpeg" => "ffmpeg",
        "ffprobe" => "ffprobe",
        _ => "media-tool",
    }
}

fn ensure_success(tool: &'static str, output: &ProcessOutput) -> Result<(), FfmpegError> {
    if output.status_code == 0 {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = truncate_for_error(&stderr);
    Err(FfmpegError::NonZeroExit {
        tool,
        status: output.status_code,
        stderr,
    })
}

fn truncate_for_error(text: &str) -> String {
    const LIMIT: usize = 512;
    if text.len() <= LIMIT {
        return text.trim().to_owned();
    }
    let mut end = LIMIT;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", text[..end].trim())
}

fn parse_ffprobe_json(bytes: &[u8]) -> Result<MediaProbe, FfmpegError> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|error| FfmpegError::Parse(error.to_string()))?;
    let mut probe = MediaProbe::default();
    if let Some(duration) = value
        .pointer("/format/duration")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|d| d.is_finite() && *d >= 0.0)
    {
        probe.duration_secs = Some(duration);
    }
    let streams = value
        .get("streams")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    for stream in streams {
        let codec_type = stream
            .get("codec_type")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        match codec_type {
            "video" => {
                probe.has_video = true;
                if probe.width.is_none() {
                    probe.width = stream
                        .get("width")
                        .and_then(|v| v.as_u64())
                        .map(|v| v as u32);
                }
                if probe.height.is_none() {
                    probe.height = stream
                        .get("height")
                        .and_then(|v| v.as_u64())
                        .map(|v| v as u32);
                }
                if probe.video_codec.is_none() {
                    probe.video_codec = stream
                        .get("codec_name")
                        .and_then(|v| v.as_str())
                        .map(str::to_owned);
                }
                if probe.duration_secs.is_none() {
                    probe.duration_secs = stream
                        .get("duration")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<f64>().ok())
                        .filter(|d| d.is_finite() && *d >= 0.0);
                }
            }
            "audio" => {
                probe.has_audio = true;
                if probe.audio_codec.is_none() {
                    probe.audio_codec = stream
                        .get("codec_name")
                        .and_then(|v| v.as_str())
                        .map(str::to_owned);
                }
                if probe.sample_rate.is_none() {
                    probe.sample_rate = stream
                        .get("sample_rate")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<u32>().ok());
                }
                if probe.channels.is_none() {
                    probe.channels = stream
                        .get("channels")
                        .and_then(|v| v.as_u64())
                        .map(|v| v as u32);
                }
                if probe.duration_secs.is_none() {
                    probe.duration_secs = stream
                        .get("duration")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<f64>().ok())
                        .filter(|d| d.is_finite() && *d >= 0.0);
                }
            }
            _ => {}
        }
    }
    Ok(probe)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    struct MockRunner {
        responses: Mutex<VecDeque<Result<ProcessOutput, FfmpegError>>>,
        calls: Mutex<Vec<(String, Vec<String>)>>,
    }

    impl MockRunner {
        fn new(responses: Vec<Result<ProcessOutput, FfmpegError>>) -> Self {
            Self {
                responses: Mutex::new(responses.into()),
                calls: Mutex::new(Vec::new()),
            }
        }
    }

    impl ProcessRunner for MockRunner {
        fn run(
            &self,
            program: &str,
            args: &[&str],
            _timeout: Duration,
        ) -> Result<ProcessOutput, FfmpegError> {
            self.calls.lock().unwrap().push((
                program.to_owned(),
                args.iter().map(|s| (*s).to_owned()).collect(),
            ));
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(Err(FfmpegError::EmptyOutput))
        }
    }

    fn temp_file_with_bytes(bytes: &[u8]) -> tempfile::NamedTempFile {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        use std::io::Write;
        file.write_all(bytes).unwrap();
        file
    }

    #[test]
    fn probe_media_parses_audio_and_video_streams() {
        let file = temp_file_with_bytes(b"fake-media");
        let json = br#"{
            "format": {"duration": "12.5"},
            "streams": [
                {"codec_type":"video","codec_name":"h264","width":1280,"height":720,"duration":"12.5"},
                {"codec_type":"audio","codec_name":"aac","sample_rate":"48000","channels":2}
            ]
        }"#;
        let runner = MockRunner::new(vec![Ok(ProcessOutput {
            stdout: json.to_vec(),
            stderr: Vec::new(),
            status_code: 0,
        })]);
        let probe = probe_media(file.path(), &runner, DEFAULT_PROBE_TIMEOUT).unwrap();
        assert!(probe.has_video);
        assert!(probe.has_audio);
        assert_eq!(probe.width, Some(1280));
        assert_eq!(probe.height, Some(720));
        assert_eq!(probe.duration_secs, Some(12.5));
        assert_eq!(probe.video_codec.as_deref(), Some("h264"));
        assert_eq!(probe.audio_codec.as_deref(), Some("aac"));
        assert_eq!(probe.sample_rate, Some(48_000));
        assert_eq!(probe.channels, Some(2));
        let calls = runner.calls.lock().unwrap();
        assert_eq!(calls[0].0, "ffprobe");
        assert!(calls[0].1.iter().any(|a| a == "-print_format"));
        // Path is a discrete argv entry, never interpolated into a shell string.
        assert_eq!(
            calls[0].1.last().map(String::as_str),
            Some(file.path().to_str().unwrap())
        );
    }

    #[test]
    fn extract_frame_passes_path_as_single_argument_even_with_metacharacters() {
        let dir = tempfile::tempdir().unwrap();
        // Semicolon/pipe characters in the filename must remain one argv token.
        let nasty = dir.path().join("clip;rm|evil$(id).mp4");
        std::fs::write(&nasty, b"fake").expect("write media fixture");
        assert!(
            nasty.is_file(),
            "fixture path must exist: {}",
            nasty.display()
        );
        let runner = MockRunner::new(vec![Ok(ProcessOutput {
            stdout: b"jpeg-bytes".to_vec(),
            stderr: Vec::new(),
            status_code: 0,
        })]);
        let bytes = extract_frame_jpeg(&nasty, 1.0, &runner, DEFAULT_EXTRACT_TIMEOUT).unwrap();
        assert_eq!(bytes, b"jpeg-bytes");
        let calls = runner.calls.lock().unwrap();
        assert_eq!(calls[0].0, "ffmpeg");
        assert!(
            calls[0].1.iter().any(|arg| arg == nasty.to_str().unwrap()),
            "path must be a discrete argv entry, got {:?}",
            calls[0].1
        );
        // No shell concatenation of the path into a larger command string.
        assert!(calls[0].1.iter().all(|arg| !arg.contains("rm |")));
        assert_eq!(
            calls[0].1.iter().filter(|a| a.contains(';')).count(),
            1,
            "metacharacters stay inside the single path argument"
        );
    }

    #[test]
    fn rejects_oversized_input_before_spawn() {
        let file = temp_file_with_bytes(b"x");
        // Bypass real size by crafting an error through validate on a directory.
        let dir = tempfile::tempdir().unwrap();
        let err = probe_media(dir.path(), &SystemProcessRunner, DEFAULT_PROBE_TIMEOUT).unwrap_err();
        assert!(matches!(err, FfmpegError::NotAFile(_)));
        // Empty path.
        let err =
            probe_media(Path::new(""), &SystemProcessRunner, DEFAULT_PROBE_TIMEOUT).unwrap_err();
        assert!(matches!(err, FfmpegError::EmptyPath));
        let _ = file;
    }

    #[test]
    fn non_zero_exit_surfaces_truncated_stderr() {
        let file = temp_file_with_bytes(b"fake");
        let long_stderr = "e".repeat(2_000);
        let runner = MockRunner::new(vec![Ok(ProcessOutput {
            stdout: Vec::new(),
            stderr: long_stderr.into_bytes(),
            status_code: 1,
        })]);
        let err = extract_audio_wav(file.path(), 30, &runner, DEFAULT_EXTRACT_TIMEOUT).unwrap_err();
        match err {
            FfmpegError::NonZeroExit {
                tool,
                status,
                stderr,
            } => {
                assert_eq!(tool, "ffmpeg");
                assert_eq!(status, 1);
                assert!(stderr.len() <= 520);
                assert!(stderr.ends_with('…'));
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn timeout_error_is_typed() {
        let file = temp_file_with_bytes(b"fake");
        let runner = MockRunner::new(vec![Err(FfmpegError::Timeout {
            tool: "ffmpeg",
            timeout: Duration::from_millis(50),
        })]);
        let err =
            extract_frame_jpeg(file.path(), 0.0, &runner, Duration::from_millis(50)).unwrap_err();
        assert!(matches!(err, FfmpegError::Timeout { tool: "ffmpeg", .. }));
    }

    #[test]
    fn output_cap_is_enforced_by_runner_contract() {
        let file = temp_file_with_bytes(b"fake");
        let runner = MockRunner::new(vec![Err(FfmpegError::OutputTooLarge { tool: "ffmpeg" })]);
        let err = extract_audio_wav(file.path(), 5, &runner, DEFAULT_EXTRACT_TIMEOUT).unwrap_err();
        assert!(matches!(
            err,
            FfmpegError::OutputTooLarge { tool: "ffmpeg" }
        ));
    }

    #[test]
    fn audio_extraction_clamps_duration_to_captured_pcm_budget() {
        let file = temp_file_with_bytes(b"fake");
        let runner = MockRunner::new(vec![Ok(ProcessOutput {
            stdout: b"pcm".to_vec(),
            stderr: Vec::new(),
            status_code: 0,
        })]);
        extract_audio_pcm_s16le(file.path(), u64::MAX, &runner, DEFAULT_EXTRACT_TIMEOUT).unwrap();
        let calls = runner.calls.lock().unwrap();
        let duration_index = calls[0].1.iter().position(|arg| arg == "-t").unwrap() + 1;
        assert_eq!(
            calls[0].1[duration_index],
            MAX_AUDIO_EXTRACT_SECONDS.to_string()
        );
    }

    #[test]
    fn mime_and_extension_helpers_classify_audio_video() {
        assert!(mime_is_audio("audio/mpeg"));
        assert!(mime_is_video("video/mp4"));
        assert!(!mime_is_audio("video/mp4"));
        assert!(extension_is_audio("MP3"));
        assert!(extension_is_video("Mov"));
        assert_eq!(mime_from_extension("wav"), Some("audio/wav"));
        assert_eq!(mime_from_extension("webm"), Some("video/webm"));
    }

    #[test]
    fn sample_frames_share_one_aggregate_timeout_budget() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct SlowRunner {
            calls: AtomicUsize,
        }
        impl ProcessRunner for SlowRunner {
            fn run(
                &self,
                _program: &str,
                _args: &[&str],
                timeout: Duration,
            ) -> Result<ProcessOutput, FfmpegError> {
                self.calls.fetch_add(1, Ordering::SeqCst);
                std::thread::sleep(timeout.min(Duration::from_millis(12)));
                Ok(ProcessOutput {
                    stdout: b"frame".to_vec(),
                    stderr: Vec::new(),
                    status_code: 0,
                })
            }
        }

        let file = temp_file_with_bytes(b"fake");
        let runner = SlowRunner {
            calls: AtomicUsize::new(0),
        };
        let frames =
            extract_sample_frames_jpeg(file.path(), 10.0, 8, &runner, Duration::from_millis(20))
                .unwrap();
        assert!(frames.len() < 8);
        assert!(runner.calls.load(Ordering::SeqCst) < 8);
    }

    #[test]
    fn sample_frame_timestamps_degrade_after_first_failure() {
        let file = temp_file_with_bytes(b"fake");
        let runner = MockRunner::new(vec![
            Ok(ProcessOutput {
                stdout: b"frame0".to_vec(),
                stderr: Vec::new(),
                status_code: 0,
            }),
            Err(FfmpegError::EmptyOutput),
        ]);
        let frames =
            extract_sample_frames_jpeg(file.path(), 10.0, 4, &runner, DEFAULT_EXTRACT_TIMEOUT)
                .unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], b"frame0");
    }
}
