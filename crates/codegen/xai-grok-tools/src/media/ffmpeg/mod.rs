//! Native FFmpeg 8 preprocessing primitives (plan section 10).
//!
//! This module is compiled only when `cfg(media_ffmpeg)` is set by
//! `build.rs`, which happens when compatible FFmpeg 8 headers (avutil 60 /
//! avcodec 62 / avformat 62 / swscale 9 / swresample 6) are discovered on a
//! supported target and `GROK_DISABLE_MEDIA_FFMPEG` is unset. Without
//! headers the module compiles out cleanly with a build-time diagnostic.
//!
//! The Grok binary has no link-time FFmpeg dependency: the Grok-owned C
//! shim (`ffmpeg/grok_av.c`) calls FFmpeg exclusively through an immutable
//! per-context function-pointer table populated lazily by [`loader`] via
//! `libloading`. Rust reads bounded bytes first, the shim accepts no paths,
//! outputs are copied immediately into Rust, cancellation is a C-owned C11
//! atomic, and decode sessions run on dedicated worker threads.
//!
//! Safety model (plan 10.5): in-process FFmpeg is NOT crash-isolated. A
//! native segfault/abort/OOM may terminate the process. The layer enforces
//! hard caps (bytes, pixels, dimensions, duration, frames, PCM), bounded
//! concurrency ([`loader`]'s session lease), cooperative cancellation, and
//! ABI validation at load time; stuck contexts are never freed or reused.

pub mod abi;
pub mod audio;
pub mod decode;
pub mod error;
pub mod loader;

pub use audio::DecodedPcm;
pub use decode::{DecodeSession, DecodedFrame, FfmpegLimits, ProbeResult};
pub use error::FfmpegError;
pub use loader::{FfmpegDiagnostics, LoadedFfmpeg, diagnostics, is_loaded, try_load};

#[cfg(test)]
mod tests;
