//! Decode sessions: one dedicated worker thread owns one native
//! [`abi::GrokAvContext`], serializing all FFmpeg calls (plan sections 10.4
//! and 10.5).
//!
//! - Inputs are bounded byte slices; C accepts no paths.
//! - Outputs (RGB frames, normalized PCM) are copied immediately into
//!   Rust-owned buffers and the native buffers are freed in the worker.
//! - Cooperative cancellation is an out-of-band C11 atomic flag; `cancel`
//!   never blocks on the worker.
//! - On request timeout the caller cancels and waits a short grace period;
//!   if the worker is still unresponsive the session is marked poisoned and
//!   its context is never freed or reused (it may remain until process
//!   exit, which is the documented trade-off for not crash-isolating native
//!   FFmpeg).

use super::abi::{
    self, ContextHandle, FnsHandle, GrokAvContext, GrokAvFrame, GrokAvLimits, GrokAvPcm,
    GrokAvProbeResult,
};
use super::audio::DecodedPcm;
use super::error::{FfmpegError, from_native_code};
use super::loader::{LoadedFfmpeg, SessionLease};
use std::ptr;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

/// Default per-request deadline for native operations.
const DEFAULT_REQUEST_TIMEOUT_MS: u64 = 30_000;
/// Grace period after cancel before a session is declared unresponsive.
const CANCEL_GRACE_MS: u64 = 2_000;
/// Grace period when closing a session.
const CLOSE_GRACE_MS: u64 = 2_000;

/// Hard caps for one decode session. These are the primitive layer's
/// safety bounds (plan 10.5); PR 6 applies tighter policy limits on top.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FfmpegLimits {
    /// Maximum accepted source byte length (also enforced by the shim).
    pub max_source_bytes: usize,
    /// Maximum decoded frame `width * height`.
    pub max_pixels: u64,
    /// Maximum decoded frame width.
    pub max_width: i32,
    /// Maximum decoded frame height.
    pub max_height: i32,
    /// Maximum media duration in microseconds (enforced by the shim at
    /// open, once the probed duration is reliable).
    pub max_duration_us: u64,
    /// Maximum total output PCM samples (frames * channels).
    pub max_audio_samples: u64,
    /// Maximum sequential frame iterations per session.
    pub max_video_frames: i32,
    /// Maximum RGB bytes per frame.
    pub max_frame_bytes: u64,
    /// Per-request wall-clock deadline in milliseconds. On expiry the
    /// operation is cancelled; if the worker stays unresponsive the session
    /// is abandoned (its context is never freed or reused).
    pub request_timeout_ms: u64,
}

impl Default for FfmpegLimits {
    fn default() -> Self {
        FfmpegLimits {
            max_source_bytes: 256 * 1024 * 1024,
            max_pixels: 8192 * 8192,
            max_width: 8192,
            max_height: 8192,
            max_duration_us: 900 * 1_000_000,
            max_audio_samples: 900 * 48_000 * 2,
            max_video_frames: 64,
            max_frame_bytes: 64 * 1024 * 1024,
            request_timeout_ms: DEFAULT_REQUEST_TIMEOUT_MS,
        }
    }
}

impl FfmpegLimits {
    pub(crate) fn into_native(self) -> GrokAvLimits {
        GrokAvLimits {
            max_source_bytes: self.max_source_bytes,
            max_pixels: self.max_pixels,
            max_width: self.max_width,
            max_height: self.max_height,
            max_duration_us: self.max_duration_us,
            max_audio_samples: self.max_audio_samples,
            max_video_frames: self.max_video_frames,
            max_frame_bytes: self.max_frame_bytes,
        }
    }
}

/// Container metadata produced by a probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeResult {
    pub has_video: bool,
    pub has_audio: bool,
    pub width: u32,
    pub height: u32,
    pub video_stream_index: i32,
    pub audio_stream_index: i32,
    /// Duration in microseconds (`None` when unknown).
    pub duration_us: Option<i64>,
    /// Start time in microseconds (`None` when unknown).
    pub start_time_us: Option<i64>,
    /// `(num, den)` video stream time base.
    pub video_time_base: Option<(i32, i32)>,
    pub sample_rate: u32,
    pub channels: u32,
}

impl ProbeResult {
    fn from_native(c: &GrokAvProbeResult) -> Self {
        ProbeResult {
            has_video: c.has_video != 0,
            has_audio: c.has_audio != 0,
            width: c.width.max(0) as u32,
            height: c.height.max(0) as u32,
            video_stream_index: c.video_stream_index,
            audio_stream_index: c.audio_stream_index,
            duration_us: (c.duration_us >= 0).then_some(c.duration_us),
            start_time_us: (c.start_time_us >= 0).then_some(c.start_time_us),
            video_time_base: (c.video_time_base_num != 0 && c.video_time_base_den != 0)
                .then_some((c.video_time_base_num as i32, c.video_time_base_den as i32)),
            sample_rate: c.sample_rate.max(0) as u32,
            channels: c.channels.max(0) as u32,
        }
    }
}

/// One decoded RGB24 frame, copied out of the native context.
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedFrame {
    /// RGB24 packed rows; bytes per row is `stride`.
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub stride: i32,
    /// PTS in stream time-base units (`None` when unknown).
    pub pts: Option<i64>,
    /// `(num, den)` stream time base for converting PTS.
    pub time_base: Option<(i32, i32)>,
}

impl DecodedFrame {
    fn from_native(c: &GrokAvFrame) -> Self {
        let stride = if c.stride > 0 {
            c.stride as usize
        } else {
            (c.width.max(0) as usize) * 3
        };
        let total = stride.saturating_mul(c.height.max(0) as usize);
        let data = if total > 0 && !c.data.is_null() {
            let mut bytes = vec![0u8; total];
            // SAFETY: the shim guarantees `data` points to at least
            // `height * stride` contiguous bytes for a successful decode.
            unsafe { ptr::copy_nonoverlapping(c.data, bytes.as_mut_ptr(), total) };
            bytes
        } else {
            Vec::new()
        };
        DecodedFrame {
            data,
            width: c.width.max(0) as u32,
            height: c.height.max(0) as u32,
            stride: stride as i32,
            pts: (c.pts != i64::MIN).then_some(c.pts),
            time_base: (c.time_base_num != 0 && c.time_base_den != 0)
                .then_some((c.time_base_num as i32, c.time_base_den as i32)),
        }
    }

    /// Convert PTS to microseconds when both PTS and time base are known.
    pub fn pts_us(&self) -> Option<i64> {
        let pts = self.pts?;
        let (num, den) = self.time_base?;
        if num == 0 || den == 0 {
            return None;
        }
        let seconds = (pts as i128) * (num as i128) / (den as i128);
        (seconds * 1_000_000).try_into().ok()
    }
}

enum WorkerMsg {
    Open {
        source: Arc<[u8]>,
        limits: GrokAvLimits,
        reply: mpsc::Sender<Result<(), FfmpegError>>,
    },
    Probe {
        reply: mpsc::Sender<Result<ProbeResult, FfmpegError>>,
    },
    NextFrame {
        reply: mpsc::Sender<Result<DecodedFrame, FfmpegError>>,
    },
    FrameAt {
        seconds: i64,
        reply: mpsc::Sender<Result<DecodedFrame, FfmpegError>>,
    },
    AudioPcm {
        reply: mpsc::Sender<Result<DecodedPcm, FfmpegError>>,
    },
    Close,
}

struct SessionState {
    closed: bool,
    poisoned: bool,
}

struct Inner {
    tx: Mutex<mpsc::Sender<WorkerMsg>>,
    state: Mutex<SessionState>,
    /// Native context handle, written once by the worker after Open.
    ctx: Mutex<Option<ContextHandle>>,
    exit: Mutex<Option<mpsc::Receiver<()>>>,
    join: Mutex<Option<JoinHandle<()>>>,
    request_timeout_ms: u64,
}

/// A single media decode session backed by a dedicated worker thread.
pub struct DecodeSession {
    _ffmpeg: Arc<LoadedFfmpeg>,
    _lease: SessionLease,
    inner: Arc<Inner>,
}

impl DecodeSession {
    /// Open a bounded byte slice and spawn its decode worker.
    ///
    /// `source` is copied into an `Arc` shared with the worker, so the
    /// native AVIO callbacks read from memory that outlives the session.
    ///
    /// Traced as `media.ffmpeg.decode` (plan section 17) with byte counts
    /// only — never media bytes, paths, or decoder error bodies.
    #[tracing::instrument(
        name = "media.ffmpeg.decode",
        level = "debug",
        skip_all,
        fields(
            source_bytes = source.len(),
            max_source_bytes = limits.max_source_bytes,
        )
    )]
    pub fn open(
        ffmpeg: &Arc<LoadedFfmpeg>,
        source: Vec<u8>,
        limits: FfmpegLimits,
    ) -> Result<Self, FfmpegError> {
        if source.is_empty() {
            return Err(FfmpegError::OpenFailed("empty input".to_string()));
        }
        if source.len() > limits.max_source_bytes {
            return Err(FfmpegError::Limit(format!(
                "source is {} bytes; cap is {}",
                source.len(),
                limits.max_source_bytes
            )));
        }
        let _lease = SessionLease::acquire()?;

        let fns = FnsHandle(ffmpeg.fns_ptr());
        let request_timeout_ms = limits.request_timeout_ms;
        let (tx, rx) = mpsc::channel();
        let (exit_tx, exit_rx) = mpsc::channel();
        let inner = Arc::new(Inner {
            tx: Mutex::new(tx),
            state: Mutex::new(SessionState {
                closed: false,
                poisoned: false,
            }),
            ctx: Mutex::new(None),
            exit: Mutex::new(Some(exit_rx)),
            join: Mutex::new(None),
            request_timeout_ms,
        });

        let worker_inner = Arc::clone(&inner);
        let handle = std::thread::Builder::new()
            .name("grok-ffmpeg-decode".to_string())
            .spawn(move || worker_main(rx, exit_tx, worker_inner, fns))
            .map_err(|e| FfmpegError::Native(format!("failed to spawn decode worker: {e}")))?;
        *inner.join.lock().unwrap() = Some(handle);

        let session = DecodeSession {
            _ffmpeg: Arc::clone(ffmpeg),
            _lease,
            inner,
        };

        session
            .request(move |reply| WorkerMsg::Open {
                source: Arc::from(source),
                limits: limits.into_native(),
                reply,
            })
            .map(|()| session)
    }

    fn request<T>(
        &self,
        make: impl FnOnce(mpsc::Sender<Result<T, FfmpegError>>) -> WorkerMsg,
    ) -> Result<T, FfmpegError> {
        self.check_usable()?;
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send(make(reply_tx))?;
        let timeout = self.timeout();
        match reply_rx.recv_timeout(timeout) {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Cancel cooperatively, then wait a short grace period for
                // the in-flight operation to notice and finish.
                self.cancel();
                match reply_rx.recv_timeout(Duration::from_millis(CANCEL_GRACE_MS)) {
                    Ok(result) => result,
                    Err(_) => {
                        self.mark_poisoned();
                        Err(FfmpegError::WorkerUnresponsive)
                    }
                }
            }
            Err(_) => Err(FfmpegError::WorkerGone),
        }
    }

    fn timeout(&self) -> Duration {
        Duration::from_millis(self.inner.request_timeout_ms.max(1))
    }

    fn send(&self, msg: WorkerMsg) -> Result<(), FfmpegError> {
        self.inner
            .tx
            .lock()
            .unwrap()
            .send(msg)
            .map_err(|_| FfmpegError::WorkerGone)
    }

    fn check_usable(&self) -> Result<(), FfmpegError> {
        let state = self.inner.state.lock().unwrap();
        if state.closed {
            return Err(FfmpegError::Closed);
        }
        if state.poisoned {
            return Err(FfmpegError::WorkerUnresponsive);
        }
        Ok(())
    }

    fn mark_poisoned(&self) {
        let mut state = self.inner.state.lock().unwrap();
        state.poisoned = true;
    }

    /// Container metadata (streams, dimensions, duration).
    pub fn probe(&self) -> Result<ProbeResult, FfmpegError> {
        self.request(|reply| WorkerMsg::Probe { reply })
    }

    /// Decode the next video frame to RGB24, or [`FfmpegError::EndOfMedia`].
    pub fn next_frame(&self) -> Result<DecodedFrame, FfmpegError> {
        self.request(|reply| WorkerMsg::NextFrame { reply })
    }

    /// Deterministic frame at (or just after) `seconds` into the video.
    pub fn frame_at_seconds(&self, seconds: i64) -> Result<DecodedFrame, FfmpegError> {
        self.request(|reply| WorkerMsg::FrameAt { seconds, reply })
    }

    /// Decode the whole audio stream to bounded normalized PCM.
    pub fn audio_pcm(&self) -> Result<DecodedPcm, FfmpegError> {
        self.request(|reply| WorkerMsg::AudioPcm { reply })
    }

    /// Cooperative cancellation. Safe to call from any thread while the
    /// session is open; only the C-owned atomic flag is touched.
    pub fn cancel(&self) {
        let state = self.inner.state.lock().unwrap();
        if state.closed {
            return;
        }
        if let Some(handle) = *self.inner.ctx.lock().unwrap() {
            // SAFETY: `handle.0` is alive (the worker only frees it while
            // holding `state`, which we hold now).
            unsafe {
                abi::shim::grok_av_cancel(handle.0);
            }
        }
    }

    /// Close the session: request the worker to free the context and wait a
    /// short grace period. If the worker is stuck in a native call, it is
    /// abandoned (its context is never freed or reused).
    pub fn close(&self) {
        {
            let mut state = self.inner.state.lock().unwrap();
            if state.closed {
                return;
            }
            state.closed = true;
        }
        let _ = self.inner.tx.lock().unwrap().send(WorkerMsg::Close);
        let finished = {
            if let Some(exit_rx) = self.inner.exit.lock().unwrap().take() {
                exit_rx
                    .recv_timeout(Duration::from_millis(CLOSE_GRACE_MS))
                    .is_ok()
            } else {
                true
            }
        };
        if finished {
            if let Some(handle) = self.inner.join.lock().unwrap().take() {
                let _ = handle.join();
            }
        }
        // Not finished: worker is stuck; the JoinHandle stays in Inner, and
        // the worker thread + context may remain until process exit.
    }
}

impl Drop for DecodeSession {
    fn drop(&mut self) {
        self.close();
    }
}

/// Read the native last-error message for a failed operation.
fn last_error(code: i32, ctx: *mut GrokAvContext) -> FfmpegError {
    if ctx.is_null() {
        return FfmpegError::Native("null native context".to_string());
    }
    // SAFETY: ctx is alive when the worker calls this.
    let message = unsafe {
        let ptr = abi::shim::grok_av_last_error(ctx);
        if ptr.is_null() {
            "no native diagnostic".to_string()
        } else {
            std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned()
        }
    };
    from_native_code(code, &message)
}

/// The worker thread: owns the native context and processes requests
/// serially. Runs until `Close` or until the channel disconnects.
fn worker_main(
    rx: mpsc::Receiver<WorkerMsg>,
    exit_tx: mpsc::Sender<()>,
    inner: Arc<Inner>,
    fns: FnsHandle,
) {
    let mut ctx: Option<ContextHandle> = None;
    while let Ok(msg) = rx.recv() {
        match msg {
            WorkerMsg::Open {
                source,
                limits,
                reply,
            } => {
                let mut raw: *mut GrokAvContext = ptr::null_mut();
                // SAFETY: `fns` points at the immutable table owned by the
                // LoadedFfmpeg the session holds an Arc to.
                let rc = unsafe {
                    abi::shim::grok_av_open(&mut raw, fns.0, source.as_ptr(), source.len(), &limits)
                };
                if rc == abi::GROK_AV_OK {
                    let handle = ContextHandle(raw);
                    ctx = Some(handle);
                    *inner.ctx.lock().unwrap() = Some(handle);
                    let _ = reply.send(Ok(()));
                } else {
                    let err = last_error(rc, raw);
                    // SAFETY: the shim promises *out_ctx is set on any return.
                    unsafe { abi::shim::grok_av_close(raw) };
                    let _ = reply.send(Err(err));
                }
            }
            WorkerMsg::Probe { reply } => {
                let result = match ctx {
                    Some(handle) => {
                        let mut out: GrokAvProbeResult = unsafe { std::mem::zeroed() };
                        // SAFETY: the context is live.
                        let rc = unsafe { abi::shim::grok_av_probe(handle.0, &mut out) };
                        if rc == abi::GROK_AV_OK {
                            Ok(ProbeResult::from_native(&out))
                        } else {
                            Err(last_error(rc, handle.0))
                        }
                    }
                    None => Err(FfmpegError::Closed),
                };
                let _ = reply.send(result);
            }
            WorkerMsg::NextFrame { reply } => {
                let result = match ctx {
                    Some(handle) => {
                        let mut out: GrokAvFrame = unsafe { std::mem::zeroed() };
                        // SAFETY: the context is live.
                        let rc = unsafe { abi::shim::grok_av_next_frame(handle.0, &mut out) };
                        if rc == abi::GROK_AV_OK {
                            let frame = DecodedFrame::from_native(&out);
                            // SAFETY: `out.data` was allocated by the shim.
                            unsafe { abi::shim::grok_av_frame_free(&mut out) };
                            Ok(frame)
                        } else {
                            Err(last_error(rc, handle.0))
                        }
                    }
                    None => Err(FfmpegError::Closed),
                };
                let _ = reply.send(result);
            }
            WorkerMsg::FrameAt { seconds, reply } => {
                let result = match ctx {
                    Some(handle) => {
                        let mut out: GrokAvFrame = unsafe { std::mem::zeroed() };
                        // SAFETY: the context is live.
                        let rc = unsafe {
                            abi::shim::grok_av_frame_at_seconds(handle.0, seconds, &mut out)
                        };
                        if rc == abi::GROK_AV_OK {
                            let frame = DecodedFrame::from_native(&out);
                            // SAFETY: `out.data` was allocated by the shim.
                            unsafe { abi::shim::grok_av_frame_free(&mut out) };
                            Ok(frame)
                        } else {
                            Err(last_error(rc, handle.0))
                        }
                    }
                    None => Err(FfmpegError::Closed),
                };
                let _ = reply.send(result);
            }
            WorkerMsg::AudioPcm { reply } => {
                let result = match ctx {
                    Some(handle) => {
                        let mut out: GrokAvPcm = unsafe { std::mem::zeroed() };
                        // SAFETY: the context is live.
                        let rc = unsafe { abi::shim::grok_av_audio_pcm(handle.0, &mut out) };
                        if rc == abi::GROK_AV_OK {
                            let pcm = DecodedPcm::from_native(&out);
                            // SAFETY: `out.data` was allocated by the shim.
                            unsafe { abi::shim::grok_av_pcm_free(&mut out) };
                            Ok(pcm)
                        } else {
                            Err(last_error(rc, handle.0))
                        }
                    }
                    None => Err(FfmpegError::Closed),
                };
                let _ = reply.send(result);
            }
            WorkerMsg::Close => break,
        }
    }

    // Thread exit path: free the context under `state` so an in-flight
    // `cancel` can never race a free.
    {
        let _guard = inner.state.lock().unwrap();
        if let Some(handle) = ctx {
            // SAFETY: the worker owns the context and no request is in
            // flight anymore.
            unsafe { abi::shim::grok_av_close(handle.0) };
        }
    }
    let _ = exit_tx.send(());
}
