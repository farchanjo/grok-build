//! Byte-level progress reporting for streaming transfers.
//!
//! An adapter that copies bytes in chunks reports each chunk into a
//! [`ProgressHandle`]; the handle is a cheap, cloneable, allocation-free sink
//! (`Arc` + two atomics) so a hot copy loop pays almost nothing. Nothing is
//! buffered here: the handle holds counters, and a consumer (a
//! [`JobSubscription`](super::jobs::JobSubscription), a status poll) reads them
//! whenever it wants.
//!
//! `None` and an inactive handle are both valid "no progress wanted" values, so
//! callers can `progress.unwrap_or_default()` and call [`ProgressHandle::add`]
//! unconditionally.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::Notify;

/// Chunk size adapters use when they report progress.
///
/// Also the smallest sensible coalescing window: a subscriber with
/// `buffer_bytes` at or above this value sees far fewer events than the copy
/// loop produced chunks.
pub const PROGRESS_CHUNK_BYTES: usize = 64 * 1024;

/// Shared counters behind a [`ProgressHandle`].
///
/// Kept private so the only way to move the counters is through a handle; the
/// `Notify` lets a subscriber wake on progress instead of polling blindly.
#[derive(Debug, Default)]
pub struct ProgressSink {
    transferred: AtomicU64,
    /// `0` means "not known yet".
    total: AtomicU64,
    wake: Notify,
}

impl ProgressSink {
    fn add(&self, delta: u64) {
        self.transferred.fetch_add(delta, Ordering::Relaxed);
        self.wake.notify_waiters();
    }

    fn transferred(&self) -> u64 {
        self.transferred.load(Ordering::Relaxed)
    }

    fn total(&self) -> Option<u64> {
        match self.total.load(Ordering::Relaxed) {
            0 => None,
            total => Some(total),
        }
    }
}

/// A cloneable progress sink handed to a streaming adapter.
///
/// `Default` is the inactive handle: every method is a branch, nothing is
/// allocated, and the adapter code stays free of `if let Some(..)` noise.
#[derive(Debug, Clone, Default)]
pub struct ProgressHandle {
    sink: Option<Arc<ProgressSink>>,
}

impl ProgressHandle {
    /// An inactive handle. Every report is a no-op.
    pub const fn inactive() -> Self {
        Self { sink: None }
    }

    /// A fresh, active handle with an unknown total.
    pub fn new() -> Self {
        Self {
            sink: Some(Arc::new(ProgressSink::default())),
        }
    }

    /// A fresh, active handle that already knows the payload size.
    pub fn with_total(total: u64) -> Self {
        let handle = Self::new();
        handle.set_total(total);
        handle
    }

    /// Whether anything is listening. An inactive handle drops every report.
    pub const fn is_active(&self) -> bool {
        self.sink.is_some()
    }

    /// Record `delta` more transferred bytes.
    pub fn add(&self, delta: u64) {
        if let Some(sink) = &self.sink {
            sink.add(delta);
        }
    }

    /// Declare the payload size once it is known (a `Content-Length`, a `stat`).
    ///
    /// Idempotent; a later call wins.
    pub fn set_total(&self, total: u64) {
        if let Some(sink) = &self.sink {
            sink.total.store(total, Ordering::Relaxed);
        }
    }

    /// Drop the byte count back to zero, keeping the total.
    ///
    /// Used when a transfer restarts (an ACL retry re-runs the multipart
    /// upload), so a subscriber never sees a count that exceeds the total.
    pub fn reset(&self) {
        if let Some(sink) = &self.sink {
            sink.transferred.store(0, Ordering::Relaxed);
            sink.wake.notify_waiters();
        }
    }

    /// Bytes transferred so far.
    pub fn transferred(&self) -> u64 {
        self.sink.as_ref().map(|s| s.transferred()).unwrap_or(0)
    }

    /// Payload size, when the adapter learned it.
    pub fn total(&self) -> Option<u64> {
        self.sink.as_ref().and_then(|s| s.total())
    }

    /// Completion in `0.0..=1.0`, when the total is known.
    pub fn fraction(&self) -> Option<f64> {
        let total = self.total()?;
        if total == 0 {
            return Some(1.0);
        }
        Some((self.transferred() as f64 / total as f64).clamp(0.0, 1.0))
    }

    /// Resolves on the next progress report.
    ///
    /// An inactive handle never resolves, which makes it a harmless extra
    /// `select!` arm.
    pub async fn changed(&self) {
        match &self.sink {
            Some(sink) => sink.wake.notified().await,
            None => std::future::pending().await,
        }
    }
}

/// Copy `reader` into `writer` one [`PROGRESS_CHUNK_BYTES`] step at a time,
/// reporting every step.
///
/// Shared by the streaming adapters so none of them materializes the payload
/// and all of them report the same way. Returns the copied byte count.
pub(crate) async fn copy_with_progress<R, W>(
    reader: &mut R,
    writer: &mut W,
    progress: Option<&ProgressHandle>,
) -> std::io::Result<u64>
where
    R: AsyncRead + Unpin + ?Sized,
    W: AsyncWrite + Unpin + ?Sized,
{
    let mut buffer = vec![0u8; PROGRESS_CHUNK_BYTES];
    let mut copied = 0u64;
    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        writer.write_all(&buffer[..read]).await?;
        copied += read as u64;
        if let Some(progress) = progress {
            progress.add(read as u64);
        }
    }
    Ok(copied)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inactive_handle_swallows_every_report() {
        let handle = ProgressHandle::inactive();
        assert!(!handle.is_active());
        handle.add(10);
        handle.set_total(10);
        assert_eq!(handle.transferred(), 0);
        assert_eq!(handle.total(), None);
        assert_eq!(handle.fraction(), None);
        assert_eq!(ProgressHandle::default().transferred(), 0);
    }

    #[test]
    fn active_handle_counts_and_reports_fraction() {
        let handle = ProgressHandle::with_total(200);
        assert!(handle.is_active());
        assert_eq!(handle.total(), Some(200));
        assert_eq!(handle.fraction(), Some(0.0));
        handle.add(50);
        handle.add(150);
        assert_eq!(handle.transferred(), 200);
        assert_eq!(handle.fraction(), Some(1.0));
    }

    #[test]
    fn unknown_total_stays_none_until_set() {
        let handle = ProgressHandle::new();
        handle.add(7);
        assert_eq!(handle.transferred(), 7);
        assert_eq!(handle.total(), None);
        assert_eq!(handle.fraction(), None);
        handle.set_total(14);
        assert_eq!(handle.fraction(), Some(0.5));
    }

    #[test]
    fn zero_total_reads_as_unknown() {
        // `0` is the "unknown" sentinel in the sink, so an empty payload is
        // reported as "total not known" rather than as a nan fraction.
        let handle = ProgressHandle::with_total(0);
        assert_eq!(handle.total(), None);
        assert_eq!(handle.fraction(), None);
    }

    #[test]
    fn fraction_clamps_when_the_adapter_over_reports() {
        let handle = ProgressHandle::with_total(10);
        handle.add(25);
        assert_eq!(handle.fraction(), Some(1.0));
    }

    #[test]
    fn clones_share_one_sink() {
        let handle = ProgressHandle::new();
        let clone = handle.clone();
        clone.add(3);
        assert_eq!(handle.transferred(), 3);
    }

    #[tokio::test]
    async fn changed_wakes_on_report() {
        let handle = ProgressHandle::new();
        let reporter = handle.clone();
        let waiter = tokio::spawn(async move {
            reporter.add(1);
        });
        tokio::time::timeout(std::time::Duration::from_secs(1), handle.changed())
            .await
            .expect("woken by the report");
        waiter.await.unwrap();
        assert_eq!(handle.transferred(), 1);
    }

    #[tokio::test]
    async fn inactive_changed_is_pending() {
        let handle = ProgressHandle::inactive();
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(20), handle.changed())
                .await
                .is_err()
        );
    }

    #[test]
    fn handle_is_send_sync_static() {
        fn assert_bounds<T: Send + Sync + 'static>() {}
        assert_bounds::<ProgressHandle>();
    }

    #[tokio::test]
    async fn copy_reports_one_chunk_per_step() {
        // Two full chunks plus a partial one.
        let payload = vec![7u8; PROGRESS_CHUNK_BYTES * 2 + 5];
        let mut reader = payload.as_slice();
        let mut sink = Vec::new();
        let handle = ProgressHandle::with_total(payload.len() as u64);

        let copied = copy_with_progress(&mut reader, &mut sink, Some(&handle))
            .await
            .unwrap();
        assert_eq!(copied, payload.len() as u64);
        assert_eq!(sink, payload);
        assert_eq!(handle.transferred(), payload.len() as u64);
        assert_eq!(handle.fraction(), Some(1.0));
    }

    #[tokio::test]
    async fn copy_without_a_handle_still_copies() {
        let payload = vec![1u8; 32];
        let mut reader = payload.as_slice();
        let mut sink = Vec::new();
        let copied = copy_with_progress(&mut reader, &mut sink, None)
            .await
            .unwrap();
        assert_eq!(copied, 32);
        assert_eq!(sink, payload);
    }
}
