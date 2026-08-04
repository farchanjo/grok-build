//! Errors for the native FFmpeg preprocessing layer.
//!
//! Every error is cloneable and `Send + Sync` so it can cross the decode
//! worker thread boundary and be reported by the caller.

/// Errors produced by the native FFmpeg preprocessing layer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FfmpegError {
    /// The feature was disabled by `GROK_DISABLE_MEDIA_FFMPEG` or compiled
    /// out because compatible FFmpeg 8 headers were unavailable.
    #[error("native FFmpeg preprocessing is not available: {0}")]
    Unavailable(String),
    /// A required FFmpeg runtime library could not be loaded.
    #[error("failed to load FFmpeg library `{library}`: {detail}")]
    LibraryLoad { library: String, detail: String },
    /// A loaded library's ABI major does not match the header major.
    #[error("FFmpeg ABI mismatch for `{library}`: expected major {expected}, found {found}")]
    VersionMismatch {
        library: String,
        expected: u32,
        found: u32,
    },
    /// A required symbol was missing from a loaded library.
    #[error("FFmpeg symbol `{symbol}` missing from `{library}`")]
    MissingSymbol { library: String, symbol: String },
    /// A native FFmpeg call failed.
    #[error("native FFmpeg operation failed: {0}")]
    Native(String),
    /// The input could not be opened or parsed as media.
    #[error("media input could not be opened or parsed: {0}")]
    OpenFailed(String),
    /// No matching stream exists in the media.
    #[error("no {0} stream in media")]
    NoStream(&'static str),
    /// The media is malformed or truncated and decoding failed.
    #[error("media decoding failed: {0}")]
    DecodeFailed(String),
    /// A seek to a requested timestamp failed.
    #[error("media seek failed: {0}")]
    Seek(String),
    /// The media or its layout is unsupported by this build.
    #[error("unsupported media: {0}")]
    Unsupported(String),
    /// A configured safety cap (bytes, pixels, duration, frames, PCM) was
    /// exceeded.
    #[error("media exceeded a safety limit: {0}")]
    Limit(String),
    /// The operation was cooperatively cancelled.
    #[error("operation was cancelled")]
    Cancelled,
    /// End of media reached while requesting the next frame.
    #[error("end of media reached")]
    EndOfMedia,
    /// The decode worker did not respond within the deadline. The native
    /// call may be stuck; the context is never freed or reused (plan 10.5).
    #[error("decode worker is unresponsive (possible native hang)")]
    WorkerUnresponsive,
    /// A decode session was used after it was closed.
    #[error("decode session is closed")]
    Closed,
    /// The decode worker thread exited unexpectedly.
    #[error("decode worker exited unexpectedly")]
    WorkerGone,
}

impl FfmpegError {
    /// Human-readable category used by PR 9 diagnostics.
    pub fn category(&self) -> &'static str {
        match self {
            FfmpegError::Unavailable(_) => "unavailable",
            FfmpegError::LibraryLoad { .. } => "load",
            FfmpegError::VersionMismatch { .. } => "abi",
            FfmpegError::MissingSymbol { .. } => "abi",
            FfmpegError::Native(_) => "native",
            FfmpegError::OpenFailed(_) => "open",
            FfmpegError::NoStream(_) => "nostream",
            FfmpegError::DecodeFailed(_) => "decode",
            FfmpegError::Seek(_) => "seek",
            FfmpegError::Unsupported(_) => "unsupported",
            FfmpegError::Limit(_) => "limit",
            FfmpegError::Cancelled => "cancelled",
            FfmpegError::EndOfMedia => "eof",
            FfmpegError::WorkerUnresponsive => "stuck",
            FfmpegError::Closed => "closed",
            FfmpegError::WorkerGone => "gone",
        }
    }
}

/// Converts a native `GrokAvError` code plus the C shim's last-error message
/// into a [`FfmpegError`].
pub(crate) fn from_native_code(code: i32, last_error: &str) -> FfmpegError {
    use crate::media::ffmpeg::abi;
    match code {
        abi::GROK_AV_OK => FfmpegError::Native("unexpected success".to_string()),
        abi::GROK_AV_ERR_NOMEM => FfmpegError::Native("out of memory".to_string()),
        abi::GROK_AV_ERR_INVALID_ARG => FfmpegError::Native("invalid argument".to_string()),
        abi::GROK_AV_ERR_LIBRARY => FfmpegError::Native(clean(last_error)),
        abi::GROK_AV_ERR_OPEN => FfmpegError::OpenFailed(clean(last_error)),
        abi::GROK_AV_ERR_NO_STREAM => FfmpegError::NoStream(if last_error.contains("audio") {
            "audio"
        } else {
            "video"
        }),
        abi::GROK_AV_ERR_DECODE => FfmpegError::DecodeFailed(clean(last_error)),
        abi::GROK_AV_ERR_EOF => FfmpegError::EndOfMedia,
        abi::GROK_AV_ERR_SEEK => FfmpegError::Seek(clean(last_error)),
        abi::GROK_AV_ERR_CANCELLED => FfmpegError::Cancelled,
        abi::GROK_AV_ERR_UNSUPPORTED => FfmpegError::Unsupported(clean(last_error)),
        abi::GROK_AV_ERR_LIMIT => FfmpegError::Limit(clean(last_error)),
        other => FfmpegError::Native(format!(
            "unexpected native error code {other}: {}",
            clean(last_error)
        )),
    }
}

fn clean(message: &str) -> String {
    let message = message.trim();
    if message.is_empty() {
        "no native diagnostic".to_string()
    } else {
        message.to_string()
    }
}
