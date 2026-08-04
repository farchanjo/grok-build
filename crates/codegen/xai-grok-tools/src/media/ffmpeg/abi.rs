//! Rust mirror of the `grok_av.h` C ABI.
//!
//! `GrokAvFns` must match the C `GrokAvFns` struct field-for-field. FFmpeg
//! types are erased to `*mut c_void` / integer enums: the table is only ever
//! *called* from C (which has the real headers), so Rust only needs the
//! layout. Every function pointer is `unsafe extern "C"` and is only invoked
//! by the compiled C shim.

#![allow(non_camel_case_types)]

use core::ffi::{c_char, c_int, c_uint, c_void};

/// `AVRational` (`libavutil/rational.h`), passed by value to `av_rescale_q`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrokAvRational {
    pub num: c_int,
    pub den: c_int,
}

// Native error codes (must match `enum GrokAvError` in grok_av.h).
pub const GROK_AV_OK: c_int = 0;
pub const GROK_AV_ERR_NOMEM: c_int = 1;
pub const GROK_AV_ERR_INVALID_ARG: c_int = 2;
pub const GROK_AV_ERR_LIBRARY: c_int = 3;
pub const GROK_AV_ERR_OPEN: c_int = 4;
pub const GROK_AV_ERR_NO_STREAM: c_int = 5;
pub const GROK_AV_ERR_DECODE: c_int = 6;
pub const GROK_AV_ERR_EOF: c_int = 7;
pub const GROK_AV_ERR_SEEK: c_int = 8;
pub const GROK_AV_ERR_CANCELLED: c_int = 9;
pub const GROK_AV_ERR_UNSUPPORTED: c_int = 10;
pub const GROK_AV_ERR_LIMIT: c_int = 11;

/// Opaque per-context state owned by the C shim. Never dereferenced from
/// Rust; created by `grok_av_open`, destroyed by `grok_av_close`.
#[repr(C)]
pub struct GrokAvContext {
    _opaque: [u8; 0],
}

/// Hard caps mirror of `GrokAvLimits`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrokAvLimits {
    pub max_source_bytes: usize,
    pub max_pixels: u64,
    pub max_width: c_int,
    pub max_height: c_int,
    pub max_duration_us: u64,
    pub max_audio_samples: u64,
    pub max_video_frames: c_int,
    pub max_frame_bytes: u64,
}

/// `GrokAvProbeResult` mirror.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrokAvProbeResult {
    pub has_video: c_int,
    pub has_audio: c_int,
    pub width: c_int,
    pub height: c_int,
    pub video_stream_index: c_int,
    pub audio_stream_index: c_int,
    pub duration_us: i64,
    pub start_time_us: i64,
    pub video_time_base_num: i64,
    pub video_time_base_den: i64,
    pub sample_rate: c_int,
    pub channels: c_int,
}

/// `GrokAvFrame` mirror. `data` is a context-owned RGB24 buffer allocated
/// with the loaded `libavutil`'s `av_malloc`; release via
/// `grok_av_frame_free`.
#[repr(C)]
#[derive(Debug)]
pub struct GrokAvFrame {
    pub data: *mut u8,
    pub width: c_int,
    pub height: c_int,
    pub stride: c_int,
    pub pts: i64,
    pub time_base_num: i64,
    pub time_base_den: i64,
}

/// `GrokAvPcm` mirror. `data` is context-owned interleaved float32 PCM;
/// release via `grok_av_pcm_free`.
#[repr(C)]
#[derive(Debug)]
pub struct GrokAvPcm {
    pub data: *mut f32,
    pub len: usize,
    pub sample_rate: c_int,
    pub channels: c_int,
    pub truncated: c_int,
}

unsafe impl Send for GrokAvContext {}
unsafe impl Sync for GrokAvContext {}

/// Send wrapper for the native context pointer. The C shim owns all native
/// memory; the decode worker thread moves the pointer across threads, so the
/// wrapper is `Send`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ContextHandle(pub(crate) *mut GrokAvContext);

unsafe impl Send for ContextHandle {}
unsafe impl Sync for ContextHandle {}

/// Send wrapper for the immutable function-pointer table pointer.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FnsHandle(pub(crate) *const GrokAvFns);

unsafe impl Send for FnsHandle {}
unsafe impl Sync for FnsHandle {}

/// Immutable per-context FFmpeg function-pointer table. Field order and
/// layout must match `GrokAvFns` in grok_av.h exactly. `None`-able callback
/// params use `Option<extern "C" fn ...>` so C can pass NULL.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GrokAvFns {
    // libavutil
    pub avutil_version: unsafe extern "C" fn() -> c_uint,
    pub av_strerror: unsafe extern "C" fn(c_int, *mut c_char, usize) -> c_int,
    pub av_malloc: unsafe extern "C" fn(usize) -> *mut c_void,
    pub av_realloc: unsafe extern "C" fn(*mut c_void, usize) -> *mut c_void,
    pub av_free: unsafe extern "C" fn(*mut c_void),
    pub av_frame_alloc: unsafe extern "C" fn() -> *mut c_void,
    pub av_frame_free: unsafe extern "C" fn(*mut *mut c_void),
    pub av_frame_unref: unsafe extern "C" fn(*mut c_void),
    pub av_frame_get_buffer: unsafe extern "C" fn(*mut c_void, c_int) -> c_int,
    // AVPacket helpers are exported by libavcodec (avcodec.h), not
    // libavutil; the loader resolves them from libavcodec. Field position is
    // fixed by the C header and must not change.
    pub av_packet_alloc: unsafe extern "C" fn() -> *mut c_void,
    pub av_packet_free: unsafe extern "C" fn(*mut *mut c_void),
    pub av_packet_unref: unsafe extern "C" fn(*mut c_void),
    pub av_samples_get_buffer_size:
        unsafe extern "C" fn(*mut c_int, c_int, c_int, c_int, c_int) -> c_int,
    pub av_get_bytes_per_sample: unsafe extern "C" fn(c_int) -> c_int,
    pub av_channel_layout_default: unsafe extern "C" fn(*mut c_void, c_int),
    pub av_channel_layout_uninit: unsafe extern "C" fn(*mut c_void),
    pub av_rescale_q: unsafe extern "C" fn(i64, GrokAvRational, GrokAvRational) -> i64,

    // libavcodec
    pub avcodec_version: unsafe extern "C" fn() -> c_uint,
    pub avcodec_alloc_context3: unsafe extern "C" fn(*const c_void) -> *mut c_void,
    pub avcodec_free_context: unsafe extern "C" fn(*mut *mut c_void),
    pub avcodec_parameters_to_context: unsafe extern "C" fn(*mut c_void, *const c_void) -> c_int,
    pub avcodec_open2: unsafe extern "C" fn(*mut c_void, *const c_void, *mut c_void) -> c_int,
    pub avcodec_send_packet: unsafe extern "C" fn(*mut c_void, *const c_void) -> c_int,
    pub avcodec_receive_frame: unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_int,
    pub avcodec_flush_buffers: unsafe extern "C" fn(*mut c_void),

    // libavformat
    pub avformat_version: unsafe extern "C" fn() -> c_uint,
    pub avformat_alloc_context: unsafe extern "C" fn() -> *mut c_void,
    pub avformat_free_context: unsafe extern "C" fn(*mut c_void),
    pub avformat_open_input:
        unsafe extern "C" fn(*mut *mut c_void, *const c_char, *const c_void, *mut c_void) -> c_int,
    pub avformat_find_stream_info: unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_int,
    pub av_find_best_stream:
        unsafe extern "C" fn(*mut c_void, c_int, c_int, c_int, *mut *const c_void, c_int) -> c_int,
    pub av_read_frame: unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_int,
    pub avformat_seek_file: unsafe extern "C" fn(*mut c_void, c_int, i64, i64, i64, c_int) -> c_int,
    pub avio_alloc_context: unsafe extern "C" fn(
        *mut u8,
        c_int,
        c_int,
        *mut c_void,
        Option<unsafe extern "C" fn(*mut c_void, *mut u8, c_int) -> c_int>,
        Option<unsafe extern "C" fn(*mut c_void, *const u8, c_int) -> c_int>,
        Option<unsafe extern "C" fn(*mut c_void, i64, c_int) -> i64>,
    ) -> *mut c_void,
    pub avio_context_free: unsafe extern "C" fn(*mut *mut c_void),

    // libswscale
    pub swscale_version: unsafe extern "C" fn() -> c_uint,
    pub sws_get_context: unsafe extern "C" fn(
        c_int,
        c_int,
        c_int,
        c_int,
        c_int,
        c_int,
        c_int,
        *const c_void,
        *const c_void,
        *const f64,
    ) -> *mut c_void,
    pub sws_scale: unsafe extern "C" fn(
        *mut c_void,
        *const *const u8,
        *const c_int,
        c_int,
        c_int,
        *const *mut u8,
        *const c_int,
    ) -> c_int,
    pub sws_free_context: unsafe extern "C" fn(*mut c_void),

    // libswresample
    pub swresample_version: unsafe extern "C" fn() -> c_uint,
    pub swr_alloc: unsafe extern "C" fn() -> *mut c_void,
    pub swr_init: unsafe extern "C" fn(*mut c_void) -> c_int,
    pub swr_free: unsafe extern "C" fn(*mut *mut c_void),
    pub swr_alloc_set_opts2: unsafe extern "C" fn(
        *mut *mut c_void,
        *const c_void,
        c_int,
        c_int,
        *const c_void,
        c_int,
        c_int,
        c_int,
        *mut c_void,
    ) -> c_int,
    pub swr_convert:
        unsafe extern "C" fn(*mut c_void, *mut *mut u8, c_int, *const *const u8, c_int) -> c_int,
    pub swr_get_out_samples: unsafe extern "C" fn(*mut c_void, c_int) -> c_int,
}

unsafe impl Send for GrokAvFns {}
unsafe impl Sync for GrokAvFns {}

/// Entry points compiled into `libgrok_av.a` (see `grok_av.h`).
#[allow(unsafe_op_in_unsafe_fn)]
pub mod shim {
    use super::{
        GrokAvContext, GrokAvFns, GrokAvFrame, GrokAvLimits, GrokAvPcm, GrokAvProbeResult,
    };
    use core::ffi::{c_char, c_int};

    unsafe extern "C" {
        pub fn grok_av_open(
            out_ctx: *mut *mut GrokAvContext,
            fns: *const GrokAvFns,
            data: *const u8,
            len: usize,
            limits: *const GrokAvLimits,
        ) -> c_int;
        pub fn grok_av_probe(ctx: *mut GrokAvContext, out: *mut GrokAvProbeResult) -> c_int;
        pub fn grok_av_next_frame(ctx: *mut GrokAvContext, out: *mut GrokAvFrame) -> c_int;
        pub fn grok_av_frame_at_seconds(
            ctx: *mut GrokAvContext,
            seconds: i64,
            out: *mut GrokAvFrame,
        ) -> c_int;
        pub fn grok_av_audio_pcm(ctx: *mut GrokAvContext, out: *mut GrokAvPcm) -> c_int;
        pub fn grok_av_cancel(ctx: *mut GrokAvContext) -> c_int;
        pub fn grok_av_last_error(ctx: *mut GrokAvContext) -> *const c_char;
        pub fn grok_av_frame_free(frame: *mut GrokAvFrame);
        pub fn grok_av_pcm_free(pcm: *mut GrokAvPcm);
        pub fn grok_av_close(ctx: *mut GrokAvContext);
    }
}
