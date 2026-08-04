//! Lazy runtime loading of the five FFmpeg 8 libraries via `libloading`
//! (plan section 10.3).
//!
//! - Libraries are located starting from the exact link directories the
//!   build script discovered (`GROK_FFMPEG_*_LIBDIR` env vars), then
//!   standard Unix locations.
//! - Version functions are resolved and their ABI majors verified against
//!   the header majors the build used (avutil 60 / avcodec 62 / avformat 62 /
//!   swscale 9 / swresample 6).
//! - Every other required symbol is resolved eagerly so a partial or
//!   mismatched install fails fast with a descriptive error, never a crash.
//! - The resulting immutable `GrokAvFns` table is handed to the C shim.

use super::abi::GrokAvFns;
use super::error::FfmpegError;
use libloading::Library;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

/// FFmpeg libraries required by the shim, in load order:
/// `(short name, expected ABI major, version symbol name)`.
pub const FFMPEG_LIBS: [(&str, u32, &str); 5] = [
    ("libavutil", 60, "avutil_version"),
    ("libavcodec", 62, "avcodec_version"),
    ("libavformat", 62, "avformat_version"),
    ("libswscale", 9, "swscale_version"),
    ("libswresample", 6, "swresample_version"),
];

/// A successfully loaded and ABI-validated FFmpeg installation. The handles
/// stay alive for as long as this object exists, keeping every function
/// pointer in the table valid.
pub struct LoadedFfmpeg {
    _libraries: Vec<Library>,
    fns: Box<GrokAvFns>,
    /// Base file names of the loaded libraries (e.g. `libavutil.60.dylib`).
    /// Full absolute paths are intentionally not retained (plan section 17:
    /// diagnostics must never expose full local paths).
    library_files: Vec<String>,
}

impl std::fmt::Debug for LoadedFfmpeg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedFfmpeg")
            .field("libraries", &self.library_files)
            .finish_non_exhaustive()
    }
}

impl LoadedFfmpeg {
    /// Pointer to the immutable function-pointer table, passed to the shim.
    pub(crate) fn fns_ptr(&self) -> *const GrokAvFns {
        self.fns.as_ref() as *const GrokAvFns
    }

    /// Redacted base file names of the libraries that were loaded
    /// (diagnostics only; never full paths).
    pub fn library_files(&self) -> &[String] {
        &self.library_files
    }
}

/// Extract the ABI major from an FFmpeg `*_version()` return value.
pub(crate) fn major_from_version(value: u32) -> u32 {
    value >> 16
}

/// Whether `GROK_DISABLE_MEDIA_FFMPEG` is set at runtime (kill switch).
pub(crate) fn disabled_by_env() -> bool {
    env::var_os("GROK_DISABLE_MEDIA_FFMPEG").is_some_and(|v| !v.is_empty())
}

/// Candidate library file names for a library name and major, newest first.
fn candidate_file_names(name: &str, major: u32) -> Vec<String> {
    let mut names = Vec::new();
    if cfg!(target_os = "macos") {
        names.push(format!("{name}.{major}.dylib"));
        names.push(format!("{name}.dylib"));
    } else {
        names.push(format!("{name}.so.{major}"));
        names.push(format!("{name}.so"));
    }
    names
}

/// Directories to search for the FFmpeg libraries, in order.
fn search_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    // Build-time discovered link dirs from pkg-config (via build.rs env).
    for name in FFMPEG_LIBS {
        let key = format!("GROK_FFMPEG_{}_LIBDIR", name.0.to_uppercase());
        if let Some(value) = env::var_os(&key) {
            let path = PathBuf::from(value);
            if !dirs.contains(&path) {
                dirs.push(path);
            }
        }
    }
    // Standard locations.
    for standard in [
        "/opt/homebrew/lib",
        "/usr/local/lib",
        "/usr/lib",
        "/lib",
        "/usr/lib/x86_64-linux-gnu",
        "/usr/lib/aarch64-linux-gnu",
    ] {
        let path = PathBuf::from(standard);
        if !dirs.contains(&path) {
            dirs.push(path);
        }
    }
    dirs
}

/// Resolve the absolute path of one FFmpeg library, or `None`.
///
/// Exposed for tests: passing empty dirs returns `None` so environments
/// without FFmpeg degrade gracefully instead of failing.
pub(crate) fn resolve_library_path(name: &str, major: u32, dirs: &[PathBuf]) -> Option<PathBuf> {
    for file in candidate_file_names(name, major) {
        for dir in dirs {
            let candidate = dir.join(&file);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        // Also allow the bare name (dlopen default search path).
        let bare = Path::new(&file);
        if bare.is_file() {
            return Some(bare.to_path_buf());
        }
    }
    None
}

/// Reduce a resolved library path to its base file name for diagnostics
/// (plan section 17: never expose full local paths). Falls back to the
/// library short name when the path has no usable file-name component.
fn redact_path(path: &Path, fallback: &str) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| fallback.to_string())
}

/// Load one library, returning the handle and its redacted base file name.
fn load_library(
    name: &str,
    major: u32,
    dirs: &[PathBuf],
) -> Result<(Library, String), FfmpegError> {
    let path = resolve_library_path(name, major, dirs).ok_or_else(|| FfmpegError::LibraryLoad {
        library: name.to_string(),
        detail: format!(
            "no runtime library found (searched for major {major} in {} directories)",
            dirs.len()
        ),
    })?;
    let file_name = redact_path(&path, name);
    // SAFETY: the path was just verified to exist; loading a shared library
    // and resolving symbols is the documented use of libloading.
    let library = unsafe { Library::new(&path) }.map_err(|e| FfmpegError::LibraryLoad {
        library: name.to_string(),
        detail: format!("dlopen failed for {file_name}: {e}"),
    })?;
    Ok((library, file_name))
}

/// Get a function pointer from a library by name.
fn get_symbol<T>(library: &Library, name: &str, symbol: &str) -> Result<T, FfmpegError>
where
    T: Copy,
{
    let bytes = std::ffi::CString::new(symbol).map_err(|_| FfmpegError::MissingSymbol {
        library: name.to_string(),
        symbol: symbol.to_string(),
    })?;
    // SAFETY: the symbol must match the expected type; the loader is the
    // only place that establishes this contract against real FFmpeg headers.
    unsafe { library.get(bytes.as_bytes_with_nul()) }
        .map(|sym: libloading::Symbol<'_, T>| *sym)
        .map_err(|e| FfmpegError::MissingSymbol {
            library: name.to_string(),
            symbol: format!("{symbol} ({e})"),
        })
}

/// Verify one library's ABI major matches the header major the build used.
fn check_version(
    name: &str,
    expected_major: u32,
    version_fn: unsafe extern "C" fn() -> u32,
) -> Result<(), FfmpegError> {
    // SAFETY: the symbol was resolved from a real FFmpeg library.
    let value = unsafe { version_fn() };
    let found = major_from_version(value);
    if found != expected_major {
        return Err(FfmpegError::VersionMismatch {
            library: name.to_string(),
            expected: expected_major,
            found,
        });
    }
    Ok(())
}

/// Build the full `GrokAvFns` table by loading and validating all five
/// libraries. Testable with explicit search dirs; the global loader uses the
/// default search path.
pub(crate) fn load_from_dirs(dirs: &[PathBuf]) -> Result<LoadedFfmpeg, FfmpegError> {
    let mut libraries = Vec::new();
    let mut library_files = Vec::new();

    for (name, major, version_symbol) in FFMPEG_LIBS {
        let (library, file) = load_library(name, major, dirs)?;

        // Version symbol and ABI major check.
        let version_fn: unsafe extern "C" fn() -> u32 = get_symbol(&library, name, version_symbol)?;
        check_version(name, major, version_fn)?;

        libraries.push(library);
        library_files.push(file);
    }

    let fns = build_fns_table(&libraries)?;

    Ok(LoadedFfmpeg {
        _libraries: libraries,
        fns: Box::new(fns),
        library_files,
    })
}

/// Resolve every function pointer into the immutable table. The expected
/// field types of `GrokAvFns` drive inference for each `get_symbol` call.
fn build_fns_table(libraries: &[Library]) -> Result<GrokAvFns, FfmpegError> {
    let names = FFMPEG_LIBS.map(|(n, _, _)| n);

    // Indices into FFMPEG_LIBS: 0 avutil, 1 avcodec, 2 avformat, 3 swscale,
    // 4 swresample.
    Ok(GrokAvFns {
        // libavutil
        avutil_version: get_symbol(&libraries[0], names[0], "avutil_version")?,
        av_strerror: get_symbol(&libraries[0], names[0], "av_strerror")?,
        av_malloc: get_symbol(&libraries[0], names[0], "av_malloc")?,
        av_realloc: get_symbol(&libraries[0], names[0], "av_realloc")?,
        av_free: get_symbol(&libraries[0], names[0], "av_free")?,
        av_frame_alloc: get_symbol(&libraries[0], names[0], "av_frame_alloc")?,
        av_frame_free: get_symbol(&libraries[0], names[0], "av_frame_free")?,
        av_frame_unref: get_symbol(&libraries[0], names[0], "av_frame_unref")?,
        av_frame_get_buffer: get_symbol(&libraries[0], names[0], "av_frame_get_buffer")?,
        // AVPacket helpers live in libavcodec (avcodec.h) and are exported by
        // libavcodec only; libavutil does not re-export them in FFmpeg 8.
        av_packet_alloc: get_symbol(&libraries[1], names[1], "av_packet_alloc")?,
        av_packet_free: get_symbol(&libraries[1], names[1], "av_packet_free")?,
        av_packet_unref: get_symbol(&libraries[1], names[1], "av_packet_unref")?,
        av_samples_get_buffer_size: get_symbol(
            &libraries[0],
            names[0],
            "av_samples_get_buffer_size",
        )?,
        av_get_bytes_per_sample: get_symbol(&libraries[0], names[0], "av_get_bytes_per_sample")?,
        av_channel_layout_default: get_symbol(
            &libraries[0],
            names[0],
            "av_channel_layout_default",
        )?,
        av_channel_layout_uninit: get_symbol(&libraries[0], names[0], "av_channel_layout_uninit")?,
        av_rescale_q: get_symbol(&libraries[0], names[0], "av_rescale_q")?,
        // libavcodec
        avcodec_version: get_symbol(&libraries[1], names[1], "avcodec_version")?,
        avcodec_alloc_context3: get_symbol(&libraries[1], names[1], "avcodec_alloc_context3")?,
        avcodec_free_context: get_symbol(&libraries[1], names[1], "avcodec_free_context")?,
        avcodec_parameters_to_context: get_symbol(
            &libraries[1],
            names[1],
            "avcodec_parameters_to_context",
        )?,
        avcodec_open2: get_symbol(&libraries[1], names[1], "avcodec_open2")?,
        avcodec_send_packet: get_symbol(&libraries[1], names[1], "avcodec_send_packet")?,
        avcodec_receive_frame: get_symbol(&libraries[1], names[1], "avcodec_receive_frame")?,
        avcodec_flush_buffers: get_symbol(&libraries[1], names[1], "avcodec_flush_buffers")?,
        // libavformat
        avformat_version: get_symbol(&libraries[2], names[2], "avformat_version")?,
        avformat_alloc_context: get_symbol(&libraries[2], names[2], "avformat_alloc_context")?,
        avformat_free_context: get_symbol(&libraries[2], names[2], "avformat_free_context")?,
        avformat_open_input: get_symbol(&libraries[2], names[2], "avformat_open_input")?,
        avformat_find_stream_info: get_symbol(
            &libraries[2],
            names[2],
            "avformat_find_stream_info",
        )?,
        av_find_best_stream: get_symbol(&libraries[2], names[2], "av_find_best_stream")?,
        av_read_frame: get_symbol(&libraries[2], names[2], "av_read_frame")?,
        avformat_seek_file: get_symbol(&libraries[2], names[2], "avformat_seek_file")?,
        avio_alloc_context: get_symbol(&libraries[2], names[2], "avio_alloc_context")?,
        avio_context_free: get_symbol(&libraries[2], names[2], "avio_context_free")?,
        // libswscale
        swscale_version: get_symbol(&libraries[3], names[3], "swscale_version")?,
        sws_get_context: get_symbol(&libraries[3], names[3], "sws_getContext")?,
        sws_scale: get_symbol(&libraries[3], names[3], "sws_scale")?,
        sws_free_context: get_symbol(&libraries[3], names[3], "sws_freeContext")?,
        // libswresample
        swresample_version: get_symbol(&libraries[4], names[4], "swresample_version")?,
        swr_alloc: get_symbol(&libraries[4], names[4], "swr_alloc")?,
        swr_init: get_symbol(&libraries[4], names[4], "swr_init")?,
        swr_free: get_symbol(&libraries[4], names[4], "swr_free")?,
        swr_alloc_set_opts2: get_symbol(&libraries[4], names[4], "swr_alloc_set_opts2")?,
        swr_convert: get_symbol(&libraries[4], names[4], "swr_convert")?,
        swr_get_out_samples: get_symbol(&libraries[4], names[4], "swr_get_out_samples")?,
    })
}

/// Process-wide result of attempting to load FFmpeg.
#[derive(Debug, Clone)]
pub enum FfmpegLoadOutcome {
    Loaded(Arc<LoadedFfmpeg>),
    Failed(FfmpegError),
}

/// Bounded concurrency for decode sessions (plan 10.5): a fixed cap of live
/// native contexts per process so hostile or buggy media cannot spawn an
/// unbounded number of worker threads/contexts.
const MAX_CONCURRENT_SESSIONS: usize = 16;

struct SessionLeaseGuard;

impl Drop for SessionLeaseGuard {
    fn drop(&mut self) {
        SESSION_COUNT.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }
}

/// RAII lease for one decode session slot.
pub(crate) struct SessionLease {
    _guard: SessionLeaseGuard,
}

impl SessionLease {
    pub(crate) fn acquire() -> Result<Self, FfmpegError> {
        let previous = SESSION_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if previous >= MAX_CONCURRENT_SESSIONS {
            SESSION_COUNT.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            return Err(FfmpegError::Limit(format!(
                "too many concurrent decode sessions (max {MAX_CONCURRENT_SESSIONS})"
            )));
        }
        Ok(SessionLease {
            _guard: SessionLeaseGuard,
        })
    }
}

static SESSION_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

static LOADED: OnceLock<FfmpegLoadOutcome> = OnceLock::new();

/// Load FFmpeg once per process and cache the outcome. Honors the
/// `GROK_DISABLE_MEDIA_FFMPEG` kill switch at runtime.
///
/// Traced as `media.ffmpeg.load` (plan section 17). Fields are non-secret:
/// only the kill-switch state. Library paths are never logged (diagnostics
/// redact them to base file names) and FFmpeg error bodies are never logged.
#[tracing::instrument(
    name = "media.ffmpeg.load",
    fields(disabled = %disabled_by_env())
)]
pub(crate) fn load_once() -> &'static FfmpegLoadOutcome {
    LOADED.get_or_init(|| {
        if disabled_by_env() {
            return FfmpegLoadOutcome::Failed(FfmpegError::Unavailable(
                "GROK_DISABLE_MEDIA_FFMPEG is set".to_string(),
            ));
        }
        match load_from_dirs(&search_dirs()) {
            Ok(loaded) => FfmpegLoadOutcome::Loaded(Arc::new(loaded)),
            Err(e) => FfmpegLoadOutcome::Failed(e),
        }
    })
}

/// Clone the process-wide loaded FFmpeg, or return the cached load error.
pub fn try_load() -> Result<Arc<LoadedFfmpeg>, FfmpegError> {
    match load_once() {
        FfmpegLoadOutcome::Loaded(loaded) => Ok(Arc::clone(loaded)),
        FfmpegLoadOutcome::Failed(e) => Err(e.clone()),
    }
}

/// Whether the native FFmpeg backend loaded successfully. Used by
/// availability snapshots and PR 9 diagnostics.
pub fn is_loaded() -> bool {
    matches!(load_once(), FfmpegLoadOutcome::Loaded(_))
}

/// Non-secret diagnostics about the FFmpeg load state (libraries, versions).
pub fn diagnostics() -> FfmpegDiagnostics {
    match load_once() {
        FfmpegLoadOutcome::Loaded(loaded) => FfmpegDiagnostics {
            loaded: true,
            libraries: loaded.library_files().to_vec(),
            error: None,
        },
        FfmpegLoadOutcome::Failed(e) => FfmpegDiagnostics {
            loaded: false,
            libraries: Vec::new(),
            error: Some(e.to_string()),
        },
    }
}

/// Non-secret FFmpeg load diagnostics for tool availability and TUI badges.
#[derive(Debug, Clone, Default)]
pub struct FfmpegDiagnostics {
    pub loaded: bool,
    pub libraries: Vec<String>,
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_major_extraction_is_stable() {
        // avutil 60.26.100 -> major 60
        assert_eq!(major_from_version(60 << 16 | 26 << 8 | 100), 60);
        // swscale 9.5.100
        assert_eq!(major_from_version(9 << 16 | 5 << 8 | 100), 9);
    }

    #[test]
    fn expected_majors_match_plan() {
        // plan section 10.2: avutil 60, avcodec 62, avformat 62, swscale 9,
        // swresample 6.
        let actual: Vec<u32> = FFMPEG_LIBS.iter().map(|(_, m, _)| *m).collect();
        assert_eq!(actual, vec![60, 62, 62, 9, 6]);
    }

    #[test]
    fn resolve_with_empty_dirs_is_graceful() {
        assert!(resolve_library_path("libavutil", 60, &[]).is_none());
    }

    #[test]
    fn candidate_names_include_matching_major() {
        let names = candidate_file_names("libavutil", 60);
        if cfg!(target_os = "macos") {
            assert!(names.contains(&"libavutil.60.dylib".to_string()));
        } else {
            assert!(names.contains(&"libavutil.so.60".to_string()));
        }
    }

    #[test]
    fn redact_path_keeps_only_the_base_file_name() {
        assert_eq!(
            redact_path(
                Path::new("/opt/homebrew/lib/libavutil.60.dylib"),
                "libavutil"
            ),
            "libavutil.60.dylib"
        );
        assert_eq!(
            redact_path(Path::new("/usr/lib/libavcodec.so.62"), "libavcodec"),
            "libavcodec.so.62"
        );
        assert_eq!(
            redact_path(Path::new("libswscale.9.dylib"), "libswscale"),
            "libswscale.9.dylib"
        );
        // A path with no file-name component falls back to the short name.
        assert_eq!(redact_path(Path::new("/"), "libavutil"), "libavutil");
        assert_eq!(redact_path(Path::new(""), "libavutil"), "libavutil");
    }

    #[test]
    fn load_failure_detail_never_leaks_full_paths() {
        // `load_from_dirs` with an empty search list cannot resolve any
        // library; the reported detail must contain no path components.
        let err = load_from_dirs(&[]).expect_err("empty search dirs must fail");
        match &err {
            FfmpegError::LibraryLoad { library, detail } => {
                assert_eq!(library, "libavutil");
                assert!(!detail.contains('/'), "detail leaked a path: {detail}");
            }
            other => panic!("expected LibraryLoad, got {other:?}"),
        }
    }

    #[test]
    fn session_lease_cap_is_bounded() {
        // The real cap is exercised at the session boundary, but the counter
        // must round-trip without underflow.
        let lease = SessionLease::acquire().expect("first lease ok");
        drop(lease);
        let _ = SessionLease::acquire().expect("lease after drop ok");
    }
}
