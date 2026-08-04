//! Media-understanding seam for text-only session and compaction models.
//!
//! PR 1 establishes the inference-free foundation: domain vocabulary
//! ([`domain`]), the request/result/error types and backend trait
//! ([`backend`]), and the thin `analyze_media` tool ([`analyze_media`]) which
//! is **not registered** yet — model-visible registration is deferred to PR 7.
//!
//! This module must remain inference-free. `xai-grok-inference-types`
//! depends on `xai-grok-tools`, so any tools-to-inference edge here would
//! create a dependency cycle.

pub mod analyze_media;
pub mod backend;
// PR 4: native FFmpeg 8 preprocessing primitives. Compiled in only when
// build.rs discovers compatible FFmpeg 8 headers on a supported target and
// `GROK_DISABLE_MEDIA_FFMPEG` is unset (cfg(media_ffmpeg)). Without headers
// the module compiles out cleanly with a build-time diagnostic.
pub mod domain;
#[cfg(media_ffmpeg)]
pub mod ffmpeg;
// Always-compiled public seam over the native backend. Reports
// compile-out / runtime-load availability and decodes audio/video without
// requiring consumers to duplicate the `media_ffmpeg` build cfg.
pub mod ffmpeg_api;

pub use analyze_media::{
    ANALYZE_MEDIA_TOOL_NAME, AnalyzeMediaInput, AnalyzeMediaOutput, AnalyzeMediaTool,
};
pub use backend::{
    MediaAttemptSummary, MediaBackendAvailability, MediaProvenance, MediaSemantics,
    MediaUnderstandingBackend, MediaUnderstandingError, MediaUnderstandingRequest,
    MediaUnderstandingResult,
};
pub use domain::{
    MediaCapabilities, MediaCategory, MediaCategoryStrategy, MediaDetailLevel,
    MediaModalitySupport, MediaRouteMetadata, MediaSource, MediaTransportCapabilities,
};
