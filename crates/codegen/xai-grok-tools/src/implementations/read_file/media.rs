//! Typed audio/video `read_file` outputs.
//!
//! Tools never call an inference client. They only classify media, optionally
//! probe metadata through the process-neutral ffmpeg helper, and return path-
//! centric typed content for the shell layer to understand.

use std::path::Path;

use crate::types::output::{AudioContent, ReadFileOutput, VideoContent};
use crate::util::ffmpeg::{
    self, DEFAULT_PROBE_TIMEOUT, SystemProcessRunner, mime_from_extension, mime_is_audio,
    mime_is_video,
};

/// Build a typed audio `read_file` result for `path`.
pub fn audio_read_output(path: &Path, mime_type: String, size_bytes: u64) -> ReadFileOutput {
    let probe = ffmpeg::probe_media(path, &SystemProcessRunner, DEFAULT_PROBE_TIMEOUT).ok();
    ReadFileOutput::AudioContent(AudioContent {
        absolute_path: path.to_path_buf(),
        mime_type,
        size_bytes,
        duration_secs: probe.as_ref().and_then(|p| p.duration_secs),
        sample_rate: probe.as_ref().and_then(|p| p.sample_rate),
        channels: probe.as_ref().and_then(|p| p.channels),
    })
}

/// Build a typed video `read_file` result for `path`.
pub fn video_read_output(path: &Path, mime_type: String, size_bytes: u64) -> ReadFileOutput {
    let probe = ffmpeg::probe_media(path, &SystemProcessRunner, DEFAULT_PROBE_TIMEOUT).ok();
    ReadFileOutput::VideoContent(VideoContent {
        absolute_path: path.to_path_buf(),
        mime_type,
        size_bytes,
        duration_secs: probe.as_ref().and_then(|p| p.duration_secs),
        width: probe.as_ref().and_then(|p| p.width),
        height: probe.as_ref().and_then(|p| p.height),
        has_audio: probe.as_ref().map(|p| p.has_audio).unwrap_or(false),
    })
}

/// Resolve audio/video typed output from magic MIME and/or extension.
///
/// Returns `None` when the file is not audio or video.
pub fn maybe_media_read_output(
    path: &Path,
    file_bytes: &[u8],
    extension: &str,
    magic_mime: Option<&str>,
) -> Option<ReadFileOutput> {
    let size_bytes = file_bytes.len() as u64;
    if let Some(mime) = magic_mime {
        if mime_is_audio(mime) {
            return Some(audio_read_output(path, mime.to_owned(), size_bytes));
        }
        if mime_is_video(mime) {
            return Some(video_read_output(path, mime.to_owned(), size_bytes));
        }
    }
    if ffmpeg::extension_is_audio(extension) {
        let mime = mime_from_extension(extension)
            .unwrap_or("audio/octet-stream")
            .to_owned();
        return Some(audio_read_output(path, mime, size_bytes));
    }
    if ffmpeg::extension_is_video(extension) {
        let mime = mime_from_extension(extension)
            .unwrap_or("video/octet-stream")
            .to_owned();
        return Some(video_read_output(path, mime, size_bytes));
    }
    None
}

/// Relative path label used only in model-facing prose (never absolute).
pub fn display_media_label(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| path.display().to_string())
}

/// Prompt-facing summary for an audio read (shell may replace with a transcript).
pub fn audio_prompt_summary(audio: &AudioContent) -> String {
    let name = display_media_label(&audio.absolute_path);
    let mut parts = vec![
        format!("Read audio file: {name}"),
        format!("mime={}", audio.mime_type),
        format!("size_bytes={}", audio.size_bytes),
    ];
    if let Some(duration) = audio.duration_secs {
        parts.push(format!("duration_secs={duration:.2}"));
    }
    if let Some(rate) = audio.sample_rate {
        parts.push(format!("sample_rate={rate}"));
    }
    if let Some(channels) = audio.channels {
        parts.push(format!("channels={channels}"));
    }
    parts.join("\n")
}

/// Prompt-facing summary for a video read (shell may replace with frame text).
pub fn video_prompt_summary(video: &VideoContent) -> String {
    let name = display_media_label(&video.absolute_path);
    let mut parts = vec![
        format!("Read video file: {name}"),
        format!("mime={}", video.mime_type),
        format!("size_bytes={}", video.size_bytes),
    ];
    if let Some(duration) = video.duration_secs {
        parts.push(format!("duration_secs={duration:.2}"));
    }
    if let (Some(w), Some(h)) = (video.width, video.height) {
        parts.push(format!("resolution={w}x{h}"));
    }
    if video.has_audio {
        parts.push("has_audio=true".to_owned());
    }
    parts.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maybe_media_classifies_by_mime_and_extension() {
        let dir = tempfile::tempdir().unwrap();
        let wav = dir.path().join("note.wav");
        std::fs::write(&wav, b"RIFF....WAVEfmt ").unwrap();
        let out = maybe_media_read_output(&wav, b"RIFF", "wav", Some("audio/wav")).unwrap();
        assert!(matches!(out, ReadFileOutput::AudioContent(_)));

        let mp4 = dir.path().join("clip.mp4");
        std::fs::write(&mp4, b"fake").unwrap();
        let out = maybe_media_read_output(&mp4, b"fake", "mp4", None).unwrap();
        assert!(matches!(out, ReadFileOutput::VideoContent(_)));

        assert!(maybe_media_read_output(Path::new("a.rs"), b"fn", "rs", None).is_none());
    }
}
