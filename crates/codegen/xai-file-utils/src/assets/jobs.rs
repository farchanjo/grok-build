//! Async transfer jobs: uploads and downloads the turn does not wait for.
//!
//! A multi-gigabyte upload must not block the turn, so every transfer becomes a
//! **job**: an id, an observable state, a cancellable token, and a throttled
//! event stream. The shape mirrors the terminal background-task registry
//! (`xai-grok-shell/src/terminal/background_task.rs`): `register`/`get`/`list`,
//! a completion `Notify`, a per-session map, and a hard cap on tracked jobs.
//!
//! Everything here is tokio-native:
//!
//! - the transfer itself is a `tokio::spawn`ed task, so `spawn_upload` returns
//!   immediately with a [`JobId`],
//! - cancellation is cooperative through [`CancellationToken`], and the task
//!   drops the store future on cancel (the trait's futures are
//!   cancellation-safe by contract),
//! - progress arrives from the adapter through a [`ProgressHandle`], never by
//!   buffering the payload,
//! - [`AssetJobRegistry::subscribe`] returns a `Stream` of coalesced
//!   [`JobEvent`]s, rate-limited by the same token bucket the monitor uses.
//!
//! Nothing here logs a secret: errors come from [`AssetError`], whose `Display`
//! already strips URL query strings.

use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex, RwLock};
use std::task::{Context, Poll};
use std::time::Duration;

use chrono::{DateTime, Utc};
use futures::Stream;
use tokio::sync::{Notify, mpsc};
use tokio_util::sync::CancellationToken;

use super::error::AssetError;
use super::key::AssetKey;
use super::progress::{PROGRESS_CHUNK_BYTES, ProgressHandle};
use super::value::{AssetMeta, PutRequest};
use super::{AssetStore, BackendKind, SharedAssetStore};
use crate::rate_limiter::TokenBucket;

/// Identifier of one transfer job (a v7 UUID, so ids sort chronologically).
pub type JobId = String;

/// Default subscriber throttle in milliseconds.
pub const DEFAULT_SUBSCRIBE_INTERVAL_MS: u64 = 500;
/// Smallest accepted throttle.
pub const MIN_SUBSCRIBE_INTERVAL_MS: u64 = 1;
/// Largest accepted throttle.
pub const MAX_SUBSCRIBE_INTERVAL_MS: u64 = 60_000;

/// Default coalescing window in bytes; one adapter chunk.
pub const DEFAULT_SUBSCRIBE_BUFFER_BYTES: usize = PROGRESS_CHUNK_BYTES;
/// Smallest accepted coalescing window.
pub const MIN_SUBSCRIBE_BUFFER_BYTES: usize = 1;
/// Largest accepted coalescing window.
pub const MAX_SUBSCRIBE_BUFFER_BYTES: usize = BUFFER_CAP_BYTES;

/// Default token-bucket capacity (events per refill window).
pub const DEFAULT_SUBSCRIBE_CAPACITY: u32 = 10;
/// Smallest accepted capacity.
pub const MIN_SUBSCRIBE_CAPACITY: u32 = 1;
/// Largest accepted capacity.
pub const MAX_SUBSCRIBE_CAPACITY: u32 = 1_000;

/// Default stream cutoff.
pub const DEFAULT_SUBSCRIBE_MAX_EVENTS: usize = 50;
/// Smallest accepted cutoff.
pub const MIN_SUBSCRIBE_MAX_EVENTS: usize = 1;
/// Largest accepted cutoff.
pub const MAX_SUBSCRIBE_MAX_EVENTS: usize = 1_000;

/// Refill window for the subscription token bucket (ms).
pub const SUBSCRIBE_REFILL_MS: u64 = 1_000;

/// Hard cap on a coalesced event payload, mirroring the monitor's
/// `BUFFER_CAP_BYTES`.
pub const BUFFER_CAP_BYTES: usize = 1_048_576;

/// Hard cap on one rendered event line, mirroring the monitor's
/// `MAX_RESULT_SIZE_CHARS`.
pub const MAX_EVENT_TEXT_CHARS: usize = 10_000;

/// Tracked jobs per session before the oldest are reclaimed.
pub const DEFAULT_MAX_JOBS: usize = 16;

/// A terminal job is reaped once it is this old.
pub const DEFAULT_JOB_TTL_SECS: u64 = 3_600;

/// Which direction the bytes move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransferKind {
    Upload,
    Download,
}

impl TransferKind {
    /// Stable snake_case name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Upload => "upload",
            Self::Download => "download",
        }
    }

    /// Whether this job writes into the store.
    pub const fn is_upload(self) -> bool {
        matches!(self, Self::Upload)
    }
}

impl std::fmt::Display for TransferKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Lifecycle of one job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JobState {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl JobState {
    /// Stable snake_case name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    /// Whether no further transition is possible.
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    /// Parse a wire name back into a state.
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "queued" => Some(Self::Queued),
            "running" => Some(Self::Running),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "cancelled" | "canceled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

impl std::fmt::Display for JobState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A point-in-time view of one job.
#[derive(Debug, Clone)]
pub struct JobSnapshot {
    pub job_id: JobId,
    pub kind: TransferKind,
    pub key: AssetKey,
    pub backend: BackendKind,
    pub state: JobState,
    pub bytes_transferred: u64,
    /// `None` until the adapter learns the payload size.
    pub bytes_total: Option<u64>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    /// Secret-free failure detail, present only in [`JobState::Failed`].
    pub error: Option<String>,
}

impl JobSnapshot {
    /// Whether the job reached a terminal state.
    pub const fn is_terminal(&self) -> bool {
        self.state.is_terminal()
    }

    /// Seconds since the job started, or its total runtime once finished.
    pub fn duration_secs(&self) -> f64 {
        let end = self.ended_at.unwrap_or_else(Utc::now);
        (end - self.started_at).num_milliseconds() as f64 / 1000.0
    }

    /// Completion in `0.0..=1.0`, when the total is known.
    pub fn fraction(&self) -> Option<f64> {
        let total = self.bytes_total?;
        if total == 0 {
            return Some(1.0);
        }
        Some((self.bytes_transferred as f64 / total as f64).clamp(0.0, 1.0))
    }

    /// Average throughput in bytes per second (0.0 before any measurable time).
    pub fn bytes_per_sec(&self) -> f64 {
        let secs = self.duration_secs();
        if secs <= 0.0 {
            return 0.0;
        }
        self.bytes_transferred as f64 / secs
    }

    /// One secret-free summary line.
    pub fn summary(&self) -> String {
        let mut line = format!(
            "{} {} ({}) is {}",
            self.kind,
            self.key,
            self.backend,
            self.state
        );
        match self.bytes_total {
            Some(total) => line.push_str(&format!(
                " — {} / {}",
                format_bytes(self.bytes_transferred),
                format_bytes(total)
            )),
            None => line.push_str(&format!(" — {}", format_bytes(self.bytes_transferred))),
        }
        if let Some(error) = &self.error {
            line.push_str(&format!(": {error}"));
        }
        line
    }
}

/// Result of [`AssetJobRegistry::cancel`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CancelOutcome {
    /// The token was raised; the task lands in [`JobState::Cancelled`].
    Cancelled { job_id: JobId },
    /// The job had already reached a terminal state.
    AlreadyFinished { job_id: JobId, state: JobState },
    /// No job with that id is tracked.
    NotFound { job_id: JobId },
}

impl CancelOutcome {
    /// Stable snake_case name.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Cancelled { .. } => "cancelled",
            Self::AlreadyFinished { .. } => "already_finished",
            Self::NotFound { .. } => "not_found",
        }
    }

    /// Whether this call is what stopped the job.
    pub const fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled { .. })
    }

    pub fn job_id(&self) -> &str {
        match self {
            Self::Cancelled { job_id }
            | Self::AlreadyFinished { job_id, .. }
            | Self::NotFound { job_id } => job_id,
        }
    }
}

/// Invalid subscription or lookup input.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum JobError {
    /// An option fell outside its accepted window.
    InvalidOption {
        field: &'static str,
        value: u64,
        min: u64,
        max: u64,
    },
}

impl JobError {
    /// Stable snake_case code.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidOption { .. } => "asset_job_invalid_option",
        }
    }
}

impl std::fmt::Display for JobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidOption {
                field,
                value,
                min,
                max,
            } => write!(f, "`{field}` must be {min}..={max}, got {value}"),
        }
    }
}

impl std::error::Error for JobError {}

/// Knobs for [`AssetJobRegistry::subscribe`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubscribeOptions {
    /// Throttle between emissions (ms).
    pub interval_ms: u64,
    /// Progress coalescing window in bytes: a progress event is only emitted
    /// once at least this many more bytes have moved.
    pub buffer_bytes: usize,
    /// Token-bucket capacity (emissions per [`SUBSCRIBE_REFILL_MS`] window).
    pub capacity: u32,
    /// Progress-event cutoff before the stream closes.
    pub max_events: usize,
    /// Keep the stream open past `max_events` until the job ends.
    pub until_complete: bool,
}

impl Default for SubscribeOptions {
    fn default() -> Self {
        Self {
            interval_ms: DEFAULT_SUBSCRIBE_INTERVAL_MS,
            buffer_bytes: DEFAULT_SUBSCRIBE_BUFFER_BYTES,
            capacity: DEFAULT_SUBSCRIBE_CAPACITY,
            max_events: DEFAULT_SUBSCRIBE_MAX_EVENTS,
            until_complete: false,
        }
    }
}

impl SubscribeOptions {
    /// Reject out-of-range knobs instead of silently clamping them.
    pub fn validate(&self) -> Result<(), JobError> {
        check_range(
            "interval_ms",
            self.interval_ms,
            MIN_SUBSCRIBE_INTERVAL_MS,
            MAX_SUBSCRIBE_INTERVAL_MS,
        )?;
        check_range(
            "buffer_bytes",
            self.buffer_bytes as u64,
            MIN_SUBSCRIBE_BUFFER_BYTES as u64,
            MAX_SUBSCRIBE_BUFFER_BYTES as u64,
        )?;
        check_range(
            "capacity",
            self.capacity as u64,
            MIN_SUBSCRIBE_CAPACITY as u64,
            MAX_SUBSCRIBE_CAPACITY as u64,
        )?;
        check_range(
            "max_events",
            self.max_events as u64,
            MIN_SUBSCRIBE_MAX_EVENTS as u64,
            MAX_SUBSCRIBE_MAX_EVENTS as u64,
        )?;
        Ok(())
    }

    /// Same as [`Self::validate`], clamped per field.
    ///
    /// Used by the registry so `subscribe` can stay total while the tool
    /// surfaces a precise error first.
    fn sanitized(self) -> Self {
        Self {
            interval_ms: self
                .interval_ms
                .clamp(MIN_SUBSCRIBE_INTERVAL_MS, MAX_SUBSCRIBE_INTERVAL_MS),
            buffer_bytes: self
                .buffer_bytes
                .clamp(MIN_SUBSCRIBE_BUFFER_BYTES, MAX_SUBSCRIBE_BUFFER_BYTES),
            capacity: self
                .capacity
                .clamp(MIN_SUBSCRIBE_CAPACITY, MAX_SUBSCRIBE_CAPACITY),
            max_events: self
                .max_events
                .clamp(MIN_SUBSCRIBE_MAX_EVENTS, MAX_SUBSCRIBE_MAX_EVENTS),
            until_complete: self.until_complete,
        }
    }

    /// Channel depth for one subscription.
    fn channel_depth(self) -> usize {
        self.max_events.clamp(1, 64)
    }
}

fn check_range(field: &'static str, value: u64, min: u64, max: u64) -> Result<(), JobError> {
    if value < min || value > max {
        return Err(JobError::InvalidOption {
            field,
            value,
            min,
            max,
        });
    }
    Ok(())
}

/// One coalesced progress or state event on a [`JobSubscription`].
///
/// Plain data with owned strings: the shell serializes it straight onto the
/// `x.ai/asset_job_event` wire without a second projection, and nothing here
/// carries a secret.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JobEvent {
    pub job_id: JobId,
    /// `upload` or `download`.
    pub kind: String,
    pub key: String,
    pub backend: String,
    pub state: String,
    pub bytes_transferred: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes_total: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percent: Option<u8>,
    pub elapsed_ms: u64,
    /// Progress updates merged into this event since the previous one.
    pub coalesced_events: u64,
    /// True on the event that carries a terminal state.
    pub terminal: bool,
    /// True on the final event when the stream closed on `max_events` first.
    pub cutoff: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Pre-formatted, secret-free one-line summary.
    pub text: String,
}

impl JobEvent {
    /// Build an event from a snapshot.
    ///
    /// `coalesced` counts suppressed updates merged into this one; `cutoff`
    /// marks the closing event of a stream truncated by `max_events`.
    pub fn from_snapshot(snapshot: &JobSnapshot, coalesced: u64, cutoff: bool) -> Self {
        let percent = snapshot
            .fraction()
            .map(|fraction| (fraction * 100.0).round() as u8);
        Self {
            job_id: snapshot.job_id.clone(),
            kind: snapshot.kind.as_str().to_owned(),
            key: snapshot.key.to_string(),
            backend: snapshot.backend.as_str().to_owned(),
            state: snapshot.state.as_str().to_owned(),
            bytes_transferred: snapshot.bytes_transferred,
            bytes_total: snapshot.bytes_total,
            percent,
            elapsed_ms: (snapshot.duration_secs() * 1000.0).round() as u64,
            coalesced_events: coalesced,
            terminal: snapshot.is_terminal(),
            cutoff,
            error: snapshot.error.clone(),
            text: truncate_chars(&snapshot.summary(), MAX_EVENT_TEXT_CHARS),
        }
    }

    /// Whether the event carries a terminal state.
    pub const fn is_terminal(&self) -> bool {
        self.terminal
    }
}

/// A live, throttled event stream for one job.
///
/// Implements [`Stream`]; dropping it ends the polling task. The stream always
/// ends when the job ends, and ends early — with `cutoff: true` on the last
/// event — when `max_events` is reached and `until_complete` is false.
#[derive(Debug)]
pub struct JobSubscription {
    job_id: JobId,
    receiver: mpsc::Receiver<JobEvent>,
}

impl JobSubscription {
    /// Id of the job this stream follows.
    pub fn job_id(&self) -> &str {
        &self.job_id
    }

    /// Await the next event without pulling in `StreamExt`.
    pub async fn next(&mut self) -> Option<JobEvent> {
        self.receiver.recv().await
    }
}

impl Stream for JobSubscription {
    type Item = JobEvent;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<JobEvent>> {
        self.receiver.poll_recv(cx)
    }
}

/// Rich result of a job that finished, beyond what [`JobSnapshot`] carries.
#[derive(Debug, Clone)]
pub struct JobOutcome {
    pub job_id: JobId,
    /// Metadata the store returned on success.
    pub meta: Option<AssetMeta>,
    /// Local destination, for a download.
    pub dest: Option<PathBuf>,
}

/// Internal, mutable state of one job.
#[derive(Debug)]
struct JobRecord {
    job_id: JobId,
    kind: TransferKind,
    key: AssetKey,
    backend: BackendKind,
    state: JobState,
    started_at: DateTime<Utc>,
    ended_at: Option<DateTime<Utc>>,
    error: Option<String>,
    /// Size known at spawn (upload) — the adapter may refine it later.
    known_total: Option<u64>,
}

/// One tracked job: its record, its sink, and its completion signal.
#[derive(Debug)]
struct JobEntry {
    record: RwLock<JobRecord>,
    outcome: RwLock<Option<JobOutcome>>,
    progress: ProgressHandle,
    cancel: CancellationToken,
    done: Notify,
}

impl JobEntry {
    fn new(record: JobRecord, progress: ProgressHandle) -> Self {
        Self {
            record: RwLock::new(record),
            outcome: RwLock::new(None),
            progress,
            cancel: CancellationToken::new(),
            done: Notify::new(),
        }
    }

    fn job_id(&self) -> JobId {
        self.record
            .read()
            .expect("job record poisoned")
            .job_id
            .clone()
    }

    fn state(&self) -> JobState {
        self.record.read().expect("job record poisoned").state
    }

    /// Materialized view, with live byte counters folded in.
    fn snapshot(&self) -> JobSnapshot {
        let record = self.record.read().expect("job record poisoned");
        JobSnapshot {
            job_id: record.job_id.clone(),
            kind: record.kind,
            key: record.key.clone(),
            backend: record.backend,
            state: record.state,
            bytes_transferred: self.progress.transferred(),
            bytes_total: self.progress.total().or(record.known_total),
            started_at: record.started_at,
            ended_at: record.ended_at,
            error: record.error.clone(),
        }
    }

    fn set_state(&self, state: JobState) {
        self.record.write().expect("job record poisoned").state = state;
    }

    /// Move to a terminal state and wake every waiter.
    fn finish(&self, state: JobState, error: Option<String>) {
        debug_assert!(state.is_terminal(), "{state} is not terminal");
        {
            let mut record = self.record.write().expect("job record poisoned");
            record.state = state;
            record.ended_at = Some(Utc::now());
            record.error = error;
        }
        self.done.notify_waiters();
    }

    fn set_outcome(&self, outcome: JobOutcome) {
        *self.outcome.write().expect("job outcome poisoned") = Some(outcome);
    }

    fn outcome(&self) -> Option<JobOutcome> {
        self.outcome
            .read()
            .expect("job outcome poisoned")
            .clone()
    }

    /// Terminal timestamp, when finished.
    fn ended_at(&self) -> Option<DateTime<Utc>> {
        self.record.read().expect("job record poisoned").ended_at
    }
}

/// Per-session registry of transfer jobs.
///
/// Lives beside the store (in `Resources`), so job ids only need to be unique
/// within a session and session teardown drops the whole map. The store is
/// never held by the registry: each spawn takes an owned `Arc` clone.
#[derive(Debug)]
pub struct AssetJobRegistry {
    jobs: Mutex<HashMap<JobId, Arc<JobEntry>>>,
    max_jobs: usize,
    ttl: Duration,
}

impl Default for AssetJobRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetJobRegistry {
    /// A registry with the default cap and TTL.
    pub fn new() -> Self {
        Self::with_max_jobs(DEFAULT_MAX_JOBS)
    }

    /// A registry tracking at most `max_jobs` jobs.
    pub fn with_max_jobs(max_jobs: usize) -> Self {
        Self {
            jobs: Mutex::new(HashMap::new()),
            max_jobs: max_jobs.max(1),
            ttl: Duration::from_secs(DEFAULT_JOB_TTL_SECS),
        }
    }

    /// Override how long a terminal job is retained.
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    /// Configured cap on tracked jobs.
    pub fn max_jobs(&self) -> usize {
        self.max_jobs
    }

    // ---- spawning -------------------------------------------------------

    /// Start an upload and return its id immediately.
    ///
    /// Must be called from within a tokio runtime: the transfer runs on a
    /// spawned task.
    pub fn spawn_upload(&self, store: SharedAssetStore, request: PutRequest) -> JobId {
        self.reap_expired();
        let key = request.key.clone();
        // An externally supplied handle is honored, so a caller can watch bytes
        // it already has a sink for. `Default` is deliberately the *inactive*
        // handle, so `ProgressHandle::new()` is not the same as the default.
        #[expect(
            clippy::unwrap_or_default,
            reason = "ProgressHandle::default() is the inactive handle; new() allocates a sink"
        )]
        let progress = request
            .progress
            .clone()
            .unwrap_or_else(ProgressHandle::new);
        let mut request = request;
        request.progress = Some(progress.clone());

        let entry = self.insert(
            JobRecord {
                job_id: new_job_id(),
                kind: TransferKind::Upload,
                key,
                backend: store.backend(),
                state: JobState::Queued,
                started_at: Utc::now(),
                ended_at: None,
                error: None,
                known_total: request.known_size(),
            },
            progress,
        );
        let job_id = entry.job_id();
        let entry = Arc::clone(&entry);

        tokio::spawn(async move {
            run_upload(store, request, entry).await;
        });
        job_id
    }

    /// Start a download to `dest` and return its id immediately.
    ///
    /// Must be called from within a tokio runtime.
    pub fn spawn_download(&self, store: SharedAssetStore, key: AssetKey, dest: PathBuf) -> JobId {
        self.reap_expired();
        let entry = self.insert(
            JobRecord {
                job_id: new_job_id(),
                kind: TransferKind::Download,
                key,
                backend: store.backend(),
                state: JobState::Queued,
                started_at: Utc::now(),
                ended_at: None,
                error: None,
                // The adapter learns the size from the backend; until then the
                // total is honestly unknown.
                known_total: None,
            },
            ProgressHandle::new(),
        );
        let job_id = entry.job_id();
        let entry = Arc::clone(&entry);

        tokio::spawn(async move {
            run_download(store, entry, dest).await;
        });
        job_id
    }

    // ---- observation ----------------------------------------------------

    /// Current snapshot of one job.
    pub async fn get(&self, id: &str) -> Option<JobSnapshot> {
        self.entry(id).map(|entry| entry.snapshot())
    }

    /// Every tracked job, oldest first (v7 ids sort chronologically).
    pub async fn list(&self) -> Vec<JobSnapshot> {
        let entries = self.entries();
        let mut snapshots: Vec<JobSnapshot> = entries.iter().map(|e| e.snapshot()).collect();
        snapshots.sort_by(|a, b| a.job_id.cmp(&b.job_id));
        snapshots
    }

    /// Number of jobs that have not reached a terminal state.
    pub async fn active_count(&self) -> usize {
        self.entries()
            .iter()
            .filter(|entry| !entry.state().is_terminal())
            .count()
    }

    /// Rich result of a finished job, when one was recorded.
    pub async fn outcome(&self, id: &str) -> Option<JobOutcome> {
        self.entry(id).and_then(|entry| entry.outcome())
    }

    /// Wait for a job to finish, with an optional timeout.
    ///
    /// Returns `None` only when the id is unknown; a timeout returns the
    /// still-running snapshot, mirroring the background-task registry.
    pub async fn wait_for_completion(
        &self,
        id: &str,
        timeout: Option<Duration>,
    ) -> Option<JobSnapshot> {
        let entry = self.entry(id)?;

        // Register interest before re-checking state, so a completion that
        // lands in between cannot lose the notification.
        let notified = entry.done.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();

        if entry.state().is_terminal() {
            return Some(entry.snapshot());
        }
        match timeout {
            Some(timeout) => {
                let _ = tokio::time::timeout(timeout, notified).await;
            }
            None => notified.await,
        }
        Some(entry.snapshot())
    }

    /// Raise the cancellation token for one job.
    ///
    /// Cooperative: the task observes the token and lands in
    /// [`JobState::Cancelled`] shortly after. Use
    /// [`Self::wait_for_completion`] to observe that transition.
    pub async fn cancel(&self, id: &str) -> CancelOutcome {
        let job_id = id.to_owned();
        let Some(entry) = self.entry(id) else {
            return CancelOutcome::NotFound { job_id };
        };
        let state = entry.state();
        if state.is_terminal() {
            return CancelOutcome::AlreadyFinished { job_id, state };
        }
        entry.cancel.cancel();
        CancelOutcome::Cancelled { job_id }
    }

    /// Follow a job's progress as a coalesced, rate-limited stream.
    ///
    /// An unknown id yields a stream that ends immediately with no events; the
    /// caller decides whether that is an error. Out-of-range knobs are clamped
    /// here (the tool validates them first for a precise message).
    pub fn subscribe(&self, id: &str, opts: SubscribeOptions) -> JobSubscription {
        let opts = opts.sanitized();
        let (sender, receiver) = mpsc::channel(opts.channel_depth());
        let job_id = id.to_owned();

        match self.entry(id) {
            Some(entry) => {
                let poller_job_id = job_id.clone();
                tokio::spawn(async move {
                    poll_job(entry, opts, sender, poller_job_id).await;
                });
            }
            None => {
                // Nothing to poll: the receiver closes as soon as the sender
                // is dropped, which happens when this task ends.
                tokio::spawn(async move {
                    drop(sender);
                });
            }
        }

        JobSubscription { job_id, receiver }
    }

    /// Drop terminal jobs older than the TTL. Returns how many were removed.
    pub fn reap_expired(&self) -> usize {
        let now = Utc::now();
        let ttl = chrono::Duration::from_std(self.ttl).unwrap_or_else(|_| chrono::Duration::hours(1));
        let mut jobs = self.jobs.lock().expect("job registry poisoned");
        let before = jobs.len();
        jobs.retain(|_, entry| match entry.ended_at() {
            Some(ended) => now - ended < ttl,
            None => true,
        });
        before - jobs.len()
    }

    // ---- internals ------------------------------------------------------

    fn entry(&self, id: &str) -> Option<Arc<JobEntry>> {
        self.jobs
            .lock()
            .expect("job registry poisoned")
            .get(id)
            .cloned()
    }

    fn entries(&self) -> Vec<Arc<JobEntry>> {
        self.jobs
            .lock()
            .expect("job registry poisoned")
            .values()
            .cloned()
            .collect()
    }

    /// Insert a job, making room first.
    fn insert(&self, record: JobRecord, progress: ProgressHandle) -> Arc<JobEntry> {
        let job_id = record.job_id.clone();
        let entry = Arc::new(JobEntry::new(record, progress));
        let mut jobs = self.jobs.lock().expect("job registry poisoned");
        make_room(&mut jobs, self.max_jobs);
        jobs.insert(job_id, Arc::clone(&entry));
        entry
    }
}

/// Reclaim finished jobs, then the oldest, until one slot is free.
///
/// Job ids are v7 UUIDs, so a lexical sort is chronological.
fn make_room(jobs: &mut HashMap<JobId, Arc<JobEntry>>, max_jobs: usize) {
    if jobs.len() < max_jobs {
        return;
    }
    let mut finished: Vec<JobId> = jobs
        .values()
        .filter(|entry| entry.state().is_terminal())
        .map(|entry| entry.job_id())
        .collect();
    finished.sort();
    for id in finished {
        if jobs.len() < max_jobs {
            return;
        }
        jobs.remove(&id);
    }
    if jobs.len() >= max_jobs
        && let Some(oldest) = jobs.keys().min().cloned()
    {
        jobs.remove(&oldest);
    }
}

async fn run_upload(store: SharedAssetStore, request: PutRequest, entry: Arc<JobEntry>) {
    if entry.cancel.is_cancelled() {
        entry.finish(JobState::Cancelled, None);
        return;
    }
    entry.set_state(JobState::Running);
    let cancel = entry.cancel.clone();
    let job_id = entry.job_id();

    let result = tokio::select! {
        biased;
        () = cancel.cancelled() => None,
        result = store.put_file(request) => Some(result),
    };

    match result {
        None => {
            tracing::debug!(job = %job_id, "asset upload job cancelled");
            entry.finish(JobState::Cancelled, None);
        }
        Some(Ok(meta)) => {
            entry.progress.set_total(meta.size_bytes);
            entry.set_outcome(JobOutcome {
                job_id: job_id.clone(),
                meta: Some(meta.clone()),
                dest: None,
            });
            entry.finish(JobState::Completed, None);
            tracing::info!(
                job = %job_id,
                key = %meta.key,
                bytes = meta.size_bytes,
                "asset upload job completed"
            );
        }
        Some(Err(err)) => {
            let detail = err.to_string();
            entry.finish(JobState::Failed, Some(detail.clone()));
            tracing::warn!(job = %job_id, error = %detail, "asset upload job failed");
        }
    }
}

async fn run_download(store: SharedAssetStore, entry: Arc<JobEntry>, dest: PathBuf) {
    if entry.cancel.is_cancelled() {
        entry.finish(JobState::Cancelled, None);
        return;
    }
    entry.set_state(JobState::Running);
    let key = entry
        .record
        .read()
        .expect("job record poisoned")
        .key
        .clone();
    let cancel = entry.cancel.clone();
    let job_id = entry.job_id();

    let result = tokio::select! {
        biased;
        () = cancel.cancelled() => None,
        result = store.download_to(&key, &dest, Some(entry.progress.clone())) => Some(result),
    };

    match result {
        None => {
            tracing::debug!(job = %job_id, "asset download job cancelled");
            entry.finish(JobState::Cancelled, None);
        }
        Some(Ok(meta)) => {
            entry.progress.set_total(meta.size_bytes);
            entry.set_outcome(JobOutcome {
                job_id: job_id.clone(),
                meta: Some(meta.clone()),
                dest: Some(dest),
            });
            entry.finish(JobState::Completed, None);
            tracing::info!(
                job = %job_id,
                key = %meta.key,
                bytes = meta.size_bytes,
                "asset download job completed"
            );
        }
        Some(Err(err)) => {
            let detail = err.to_string();
            entry.finish(JobState::Failed, Some(detail.clone()));
            tracing::warn!(job = %job_id, error = %detail, "asset download job failed");
        }
    }
}

/// Coalesce, throttle, and forward job updates until the stream should close.
///
/// Three gates decide whether a sampled update becomes an event:
///
/// 1. a state change always emits, so a subscriber never misses a transition,
/// 2. a progress event only emits once `buffer_bytes` more bytes moved, which
///    is what makes the event count far lower than the adapter's chunk count,
/// 3. a token bucket caps the emission rate; denied progress is folded into the
///    next event and reported as `coalesced_events`.
///
/// The stream ends on the job's terminal event, and early — with `cutoff: true`
/// — when `max_events` is reached and `until_complete` is false.
async fn poll_job(
    entry: Arc<JobEntry>,
    opts: SubscribeOptions,
    sender: mpsc::Sender<JobEvent>,
    job_id: JobId,
) {
    let mut bucket = TokenBucket::new(opts.capacity, SUBSCRIBE_REFILL_MS);
    let mut last_state = JobState::Queued;
    let mut emitted_bytes = 0u64;
    let mut observed_bytes = 0u64;
    let mut pending = 0u64;
    let mut emitted = 0usize;
    let mut past_cutoff = false;
    let mut first = true;

    loop {
        let snapshot = entry.snapshot();
        let terminal = snapshot.is_terminal();
        let state_changed = first || snapshot.state != last_state;
        if snapshot.bytes_transferred > observed_bytes {
            pending += 1;
        }
        observed_bytes = snapshot.bytes_transferred;

        let mut emit = state_changed;
        if !emit && !past_cutoff {
            let window_filled =
                snapshot.bytes_transferred.saturating_sub(emitted_bytes) >= opts.buffer_bytes as u64;
            if window_filled {
                emit = bucket.try_consume();
            }
        }

        if emit {
            let event = JobEvent::from_snapshot(&snapshot, pending, false);
            if sender.send(event).await.is_err() {
                tracing::debug!(job = %job_id, "asset job subscriber went away");
                return;
            }
            emitted += 1;
            last_state = snapshot.state;
            emitted_bytes = snapshot.bytes_transferred;
            pending = 0;
        }
        first = false;

        if terminal {
            tracing::debug!(job = %job_id, "asset job subscription reached a terminal state");
            return;
        }
        if emitted >= opts.max_events {
            if opts.until_complete {
                past_cutoff = true;
            } else {
                let event = JobEvent::from_snapshot(&snapshot, pending, true);
                let _ = sender.send(event).await;
                tracing::debug!(job = %job_id, "asset job subscription hit max_events");
                return;
            }
        }

        tokio::select! {
            () = tokio::time::sleep(Duration::from_millis(opts.interval_ms)) => {}
            () = entry.progress.changed() => {}
            () = entry.done.notified() => {}
        }
    }
}

/// A v7 UUID string: time-ordered, so `JobId` sorting is chronological.
fn new_job_id() -> JobId {
    uuid::Uuid::now_v7().to_string()
}

/// Human-readable byte count (binary units, one decimal).
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    format!("{:.1} {}", value, UNITS[unit])
}

/// Truncate to `max` characters without splitting a UTF-8 sequence.
fn truncate_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::mock::MockAssetStore;
    use crate::assets::value::{DeleteOutcome, ListPage, ListQuery, PresignedUrl};
    use crate::assets::{AssetOperation, BackendCapabilities, ContentType, StoreStatus, Visibility};
    use bytes::Bytes;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn key(raw: &str) -> AssetKey {
        AssetKey::parse(raw).unwrap()
    }

    fn upload_request(raw: &str, body: &[u8]) -> PutRequest {
        PutRequest::from_bytes(key(raw), body.to_vec(), ContentType::default())
    }

    /// Wait until the job leaves `Queued`, with a bounded window.
    async fn wait_until_running(registry: &AssetJobRegistry, job_id: &str) {
        let _ = tokio::time::timeout(Duration::from_secs(2), async {
            while registry
                .get(job_id)
                .await
                .is_some_and(|snapshot| snapshot.state == JobState::Queued)
            {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await;
    }

    /// A store whose `put_file` reports progress in `chunk`-sized steps and can
    /// be made slow, so a subscriber sees many more chunks than events.
    #[derive(Debug)]
    struct ChunkedStore {
        inner: MockAssetStore,
        chunks: u64,
        chunk: u64,
        steps: u64,
        calls: AtomicU64,
    }

    impl ChunkedStore {
        fn new(total: u64, chunk: u64, steps: u64) -> Self {
            Self {
                inner: MockAssetStore::new(),
                chunks: total.div_ceil(chunk),
                chunk,
                steps,
                calls: AtomicU64::new(0),
            }
        }
    }

    #[async_trait::async_trait]
    impl AssetStore for ChunkedStore {
        fn backend(&self) -> BackendKind {
            self.inner.backend()
        }

        fn capabilities(&self) -> BackendCapabilities {
            self.inner.capabilities()
        }

        async fn put(&self, request: PutRequest) -> Result<AssetMeta, AssetError> {
            self.inner.put(request).await
        }

        async fn put_file(&self, request: PutRequest) -> Result<AssetMeta, AssetError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            let total = request.known_size();
            if let Some(progress) = &request.progress {
                for _ in 0..self.chunks {
                    progress.add(self.chunk);
                    for _ in 0..self.steps {
                        tokio::task::yield_now().await;
                    }
                }
                if let Some(total) = total {
                    progress.set_total(total);
                }
            }
            self.inner.put_file(request).await
        }

        async fn get(&self, key: &AssetKey) -> Result<Bytes, AssetError> {
            self.inner.get(key).await
        }

        async fn download_to(
            &self,
            key: &AssetKey,
            dest: &std::path::Path,
            progress: Option<ProgressHandle>,
        ) -> Result<AssetMeta, AssetError> {
            if let Some(progress) = &progress {
                for _ in 0..self.chunks {
                    progress.add(self.chunk);
                }
            }
            self.inner.download_to(key, dest, progress).await
        }

        async fn exists(&self, key: &AssetKey) -> Result<bool, AssetError> {
            self.inner.exists(key).await
        }

        async fn delete(&self, key: &AssetKey) -> Result<DeleteOutcome, AssetError> {
            self.inner.delete(key).await
        }

        async fn list(&self, query: ListQuery) -> Result<ListPage, AssetError> {
            self.inner.list(query).await
        }

        async fn presign_get(&self, key: &AssetKey, ttl: Duration) -> Result<PresignedUrl, AssetError> {
            self.inner.presign_get(key, ttl).await
        }

        async fn presign_put(
            &self,
            key: &AssetKey,
            content_type: &ContentType,
            ttl: Duration,
        ) -> Result<PresignedUrl, AssetError> {
            self.inner.presign_put(key, content_type, ttl).await
        }

        async fn set_visibility(
            &self,
            key: &AssetKey,
            visibility: Visibility,
        ) -> Result<AssetMeta, AssetError> {
            self.inner.set_visibility(key, visibility).await
        }

        fn public_url(&self, key: &AssetKey) -> Option<String> {
            self.inner.public_url(key)
        }

        async fn health(&self) -> Result<StoreStatus, AssetError> {
            self.inner.health().await
        }
    }

    /// A store whose transfers never finish, so the job stays `Running`.
    #[derive(Debug)]
    struct HangingStore;

    #[async_trait::async_trait]
    impl AssetStore for HangingStore {
        fn backend(&self) -> BackendKind {
            BackendKind::S3
        }

        fn capabilities(&self) -> BackendCapabilities {
            BackendCapabilities::S3
        }

        async fn put(&self, _request: PutRequest) -> Result<AssetMeta, AssetError> {
            std::future::pending().await
        }

        async fn put_file(&self, request: PutRequest) -> Result<AssetMeta, AssetError> {
            if let Some(progress) = &request.progress {
                progress.add(4096);
            }
            std::future::pending().await
        }

        async fn get(&self, _key: &AssetKey) -> Result<Bytes, AssetError> {
            std::future::pending().await
        }

        async fn download_to(
            &self,
            _key: &AssetKey,
            _dest: &std::path::Path,
            _progress: Option<ProgressHandle>,
        ) -> Result<AssetMeta, AssetError> {
            std::future::pending().await
        }

        async fn exists(&self, _key: &AssetKey) -> Result<bool, AssetError> {
            Ok(false)
        }

        async fn delete(&self, _key: &AssetKey) -> Result<DeleteOutcome, AssetError> {
            Ok(DeleteOutcome::NotFound)
        }

        async fn list(&self, _query: ListQuery) -> Result<ListPage, AssetError> {
            Ok(ListPage::empty())
        }

        async fn presign_get(&self, _key: &AssetKey, _ttl: Duration) -> Result<PresignedUrl, AssetError> {
            Err(AssetError::unsupported(
                BackendKind::S3,
                AssetOperation::PresignGet,
                "unused",
            ))
        }

        async fn presign_put(
            &self,
            _key: &AssetKey,
            _content_type: &ContentType,
            _ttl: Duration,
        ) -> Result<PresignedUrl, AssetError> {
            Err(AssetError::unsupported(
                BackendKind::S3,
                AssetOperation::PresignPut,
                "unused",
            ))
        }

        async fn set_visibility(
            &self,
            _key: &AssetKey,
            _visibility: Visibility,
        ) -> Result<AssetMeta, AssetError> {
            Err(AssetError::unsupported(
                BackendKind::S3,
                AssetOperation::SetVisibility,
                "unused",
            ))
        }

        fn public_url(&self, _key: &AssetKey) -> Option<String> {
            None
        }

        async fn health(&self) -> Result<StoreStatus, AssetError> {
            Ok(StoreStatus::healthy(BackendKind::S3))
        }
    }

    fn store() -> SharedAssetStore {
        Arc::new(MockAssetStore::new())
    }

    #[tokio::test]
    async fn upload_job_runs_to_completion() {
        let registry = AssetJobRegistry::new();
        let mock = Arc::new(MockAssetStore::new());
        let job_id = registry.spawn_upload(
            mock.clone(),
            upload_request("uploads/a.txt", b"hello"),
        );

        let snapshot = registry
            .wait_for_completion(&job_id, Some(Duration::from_secs(5)))
            .await
            .expect("job exists");
        assert_eq!(snapshot.state, JobState::Completed);
        assert!(snapshot.ended_at.is_some());
        assert_eq!(snapshot.bytes_transferred, 5);
        assert_eq!(snapshot.bytes_total, Some(5));
        assert_eq!(snapshot.fraction(), Some(1.0));
        assert_eq!(snapshot.kind, TransferKind::Upload);
        assert_eq!(snapshot.backend, BackendKind::Local);
        assert!(snapshot.error.is_none());

        let outcome = registry.outcome(&job_id).await.expect("outcome recorded");
        assert_eq!(outcome.meta.expect("meta").size_bytes, 5);
        assert!(mock.was_called(AssetOperation::PutFile));
    }

    #[tokio::test]
    async fn download_job_runs_to_completion() {
        let dir = tempfile::TempDir::new().unwrap();
        let registry = AssetJobRegistry::new();
        let mock = Arc::new(MockAssetStore::new());
        mock.put(upload_request("uploads/a.bin", b"payload"))
            .await
            .unwrap();

        let dest = dir.path().join("nested/out.bin");
        let job_id = registry.spawn_download(mock.clone(), key("uploads/a.bin"), dest.clone());

        let snapshot = registry
            .wait_for_completion(&job_id, Some(Duration::from_secs(5)))
            .await
            .expect("job exists");
        assert_eq!(snapshot.state, JobState::Completed);
        assert_eq!(snapshot.kind, TransferKind::Download);
        assert_eq!(std::fs::read(&dest).unwrap(), b"payload");

        let outcome = registry.outcome(&job_id).await.expect("outcome");
        assert_eq!(outcome.dest.as_deref(), Some(dest.as_path()));
    }

    #[tokio::test]
    async fn failing_job_records_a_secret_free_error() {
        let registry = AssetJobRegistry::new();
        let mock = Arc::new(
            MockAssetStore::builder()
                .fail_with(
                    AssetOperation::PutFile,
                    AssetError::Transient {
                        backend: BackendKind::S3,
                        detail: "GET https://b.s3.amazonaws.com/k?X-Amz-Signature=deadbeef".into(),
                    },
                )
                .build(),
        );
        let job_id = registry.spawn_upload(mock, upload_request("uploads/a.txt", b"x"));

        let snapshot = registry
            .wait_for_completion(&job_id, Some(Duration::from_secs(5)))
            .await
            .unwrap();
        assert_eq!(snapshot.state, JobState::Failed);
        let error = snapshot.error.expect("error recorded");
        assert!(error.contains("s3 transient failure"), "{error}");
        assert!(!error.contains("X-Amz-Signature"), "{error}");
        assert!(registry.outcome(&job_id).await.is_none());
    }

    #[tokio::test]
    async fn cancel_stops_a_running_job() {
        let store: SharedAssetStore = Arc::new(HangingStore);
        let registry = AssetJobRegistry::new();
        let job_id = registry.spawn_upload(store, upload_request("uploads/big.bin", b"x"));
        wait_until_running(&registry, &job_id).await;

        assert_eq!(
            registry.cancel(&job_id).await,
            CancelOutcome::Cancelled {
                job_id: job_id.clone()
            }
        );
        let snapshot = registry
            .wait_for_completion(&job_id, Some(Duration::from_secs(5)))
            .await
            .unwrap();
        assert_eq!(snapshot.state, JobState::Cancelled);
        assert!(snapshot.ended_at.is_some());
        assert!(snapshot.error.is_none());
    }

    #[tokio::test]
    async fn cancel_outcomes_cover_missing_and_finished() {
        let registry = AssetJobRegistry::new();
        assert!(matches!(
            registry.cancel("nope").await,
            CancelOutcome::NotFound { .. }
        ));

        let mock = Arc::new(MockAssetStore::new());
        let job_id = registry.spawn_upload(mock, upload_request("uploads/a.txt", b"x"));
        registry
            .wait_for_completion(&job_id, Some(Duration::from_secs(5)))
            .await
            .unwrap();

        let outcome = registry.cancel(&job_id).await;
        assert!(matches!(
            outcome,
            CancelOutcome::AlreadyFinished {
                state: JobState::Completed,
                ..
            }
        ));
        assert_eq!(outcome.as_str(), "already_finished");
        assert!(!outcome.is_cancelled());
    }

    #[tokio::test]
    async fn get_list_and_active_count_agree() {
        let registry = AssetJobRegistry::new();
        let mock = Arc::new(MockAssetStore::new());
        let first = registry.spawn_upload(mock.clone(), upload_request("uploads/a.txt", b"a"));
        let second = registry.spawn_upload(mock, upload_request("uploads/b.txt", b"b"));

        let listed = registry.list().await;
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].job_id, first, "oldest first");
        assert_eq!(listed[1].job_id, second);
        assert!(registry.get(&first).await.is_some());
        assert!(registry.get("missing").await.is_none());
        assert!(registry.active_count().await <= 2);

        registry
            .wait_for_completion(&first, Some(Duration::from_secs(5)))
            .await
            .unwrap();
        registry
            .wait_for_completion(&second, Some(Duration::from_secs(5)))
            .await
            .unwrap();
        assert_eq!(registry.active_count().await, 0);
    }

    #[tokio::test]
    async fn wait_for_completion_times_out_without_finishing() {
        let store: SharedAssetStore = Arc::new(HangingStore);
        let registry = AssetJobRegistry::new();
        let job_id = registry.spawn_upload(store, upload_request("uploads/a.bin", b"x"));

        let snapshot = registry
            .wait_for_completion(&job_id, Some(Duration::from_millis(20)))
            .await
            .unwrap();
        assert!(!snapshot.is_terminal());
        assert!(
            registry
                .wait_for_completion("missing", Some(Duration::from_millis(1)))
                .await
                .is_none()
        );
    }

    #[tokio::test]
    async fn subscribe_coalesces_progress_far_below_the_chunk_count() {
        // 256 chunks of 4 KiB, and the copy loop yields between chunks, so the
        // raw update count is 256 while the subscriber must see far fewer.
        let total = 256 * 4096;
        let store: SharedAssetStore = Arc::new(ChunkedStore::new(total, 4096, 2));
        let registry = AssetJobRegistry::new();
        let job_id = registry.spawn_upload(store, upload_request("uploads/big.bin", b"x"));

        let mut subscription = registry.subscribe(
            &job_id,
            SubscribeOptions {
                interval_ms: 1,
                buffer_bytes: 64 * 1024,
                capacity: 100,
                max_events: 10_000,
                until_complete: true,
            },
        );

        let mut events = Vec::new();
        while let Some(event) = subscription.next().await {
            events.push(event);
        }

        assert!(
            events.len() < 256,
            "subscriber saw {} events for 256 chunks",
            events.len()
        );
        let last = events.last().expect("terminal event");
        assert!(last.terminal);
        assert_eq!(last.state, "completed");
        assert!(
            events.iter().any(|e| e.coalesced_events > 0),
            "some events must report coalescing"
        );
    }

    #[tokio::test]
    async fn subscribe_cutoff_closes_early_unless_until_complete() {
        let store: SharedAssetStore = Arc::new(ChunkedStore::new(1024 * 1024, 1024, 1));
        let registry = AssetJobRegistry::new();
        let job_id = registry.spawn_upload(store, upload_request("uploads/big.bin", b"x"));

        let mut subscription = registry.subscribe(
            &job_id,
            SubscribeOptions {
                interval_ms: 1,
                buffer_bytes: 1,
                capacity: 10_000,
                max_events: 3,
                until_complete: false,
            },
        );
        let mut events = Vec::new();
        while let Some(event) = subscription.next().await {
            events.push(event);
        }
        assert!(events.len() <= 4, "cutoff must bound the stream");
        assert!(events.last().unwrap().cutoff, "last event marks the cutoff");
        assert!(!events.last().unwrap().terminal, "job is still running");
    }

    #[tokio::test]
    async fn subscribe_unknown_job_yields_no_events() {
        let registry = AssetJobRegistry::new();
        let mut subscription = registry.subscribe("missing", SubscribeOptions::default());
        assert_eq!(subscription.job_id(), "missing");
        assert!(subscription.next().await.is_none());
    }

    #[tokio::test]
    async fn registry_reclaims_the_oldest_job_at_capacity() {
        let registry = AssetJobRegistry::with_max_jobs(2);
        let mock = Arc::new(MockAssetStore::new());
        let first = registry.spawn_upload(mock.clone(), upload_request("uploads/a.txt", b"a"));
        registry
            .wait_for_completion(&first, Some(Duration::from_secs(5)))
            .await
            .unwrap();

        let second = registry.spawn_upload(mock.clone(), upload_request("uploads/b.txt", b"b"));
        // The first is finished, so it is reclaimed rather than the second.
        let third = registry.spawn_upload(mock, upload_request("uploads/c.txt", b"c"));

        assert_eq!(registry.list().await.len(), 2);
        assert!(registry.get(&first).await.is_none());
        assert!(registry.get(&second).await.is_some());
        assert!(registry.get(&third).await.is_some());
    }

    #[tokio::test]
    async fn ttl_reaps_terminal_jobs_only() {
        let registry = AssetJobRegistry::new().with_ttl(Duration::from_millis(0));
        let mock = Arc::new(MockAssetStore::new());
        let job_id = registry.spawn_upload(mock, upload_request("uploads/a.txt", b"a"));
        registry
            .wait_for_completion(&job_id, Some(Duration::from_secs(5)))
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(5)).await;
        assert_eq!(registry.reap_expired(), 1);
        assert!(registry.get(&job_id).await.is_none());

        // A running job survives its own TTL.
        let registry = AssetJobRegistry::new().with_ttl(Duration::from_millis(0));
        let store: SharedAssetStore = Arc::new(HangingStore);
        let running = registry.spawn_upload(store, upload_request("uploads/a.bin", b"x"));
        assert_eq!(registry.reap_expired(), 0);
        assert!(registry.get(&running).await.is_some());
    }

    #[test]
    fn subscribe_options_validate_bounds() {
        SubscribeOptions::default().validate().unwrap();

        let cases = [
            SubscribeOptions {
                interval_ms: 0,
                ..Default::default()
            },
            SubscribeOptions {
                interval_ms: MAX_SUBSCRIBE_INTERVAL_MS + 1,
                ..Default::default()
            },
            SubscribeOptions {
                buffer_bytes: 0,
                ..Default::default()
            },
            SubscribeOptions {
                buffer_bytes: MAX_SUBSCRIBE_BUFFER_BYTES + 1,
                ..Default::default()
            },
            SubscribeOptions {
                capacity: 0,
                ..Default::default()
            },
            SubscribeOptions {
                max_events: 0,
                ..Default::default()
            },
            SubscribeOptions {
                max_events: MAX_SUBSCRIBE_MAX_EVENTS + 1,
                ..Default::default()
            },
        ];
        for options in cases {
            let err = options.validate().unwrap_err();
            assert_eq!(err.code(), "asset_job_invalid_option");
            assert!(matches!(err, JobError::InvalidOption { .. }));
        }

        let err = SubscribeOptions {
            interval_ms: 0,
            ..Default::default()
        }
        .validate()
        .unwrap_err();
        assert!(err.to_string().contains("`interval_ms` must be 1..=60000"));
    }

    #[test]
    fn invalid_options_are_clamped_by_the_registry_not_rejected() {
        let clamped = SubscribeOptions {
            interval_ms: 0,
            buffer_bytes: 0,
            capacity: 0,
            max_events: 0,
            until_complete: true,
        }
        .sanitized();
        assert_eq!(clamped.interval_ms, MIN_SUBSCRIBE_INTERVAL_MS);
        assert_eq!(clamped.buffer_bytes, MIN_SUBSCRIBE_BUFFER_BYTES);
        assert_eq!(clamped.capacity, MIN_SUBSCRIBE_CAPACITY);
        assert_eq!(clamped.max_events, MIN_SUBSCRIBE_MAX_EVENTS);
        assert!(clamped.until_complete);
    }

    #[test]
    fn job_state_and_kind_names_are_stable() {
        assert_eq!(TransferKind::Upload.as_str(), "upload");
        assert_eq!(TransferKind::Download.to_string(), "download");
        assert!(TransferKind::Upload.is_upload());
        for state in [
            JobState::Queued,
            JobState::Running,
            JobState::Completed,
            JobState::Failed,
            JobState::Cancelled,
        ] {
            assert_eq!(JobState::parse(state.as_str()), Some(state));
            assert_eq!(state.to_string(), state.as_str());
        }
        assert_eq!(JobState::parse("canceled"), Some(JobState::Cancelled));
        assert_eq!(JobState::parse("nope"), None);
        assert!(JobState::Completed.is_terminal());
        assert!(!JobState::Running.is_terminal());
    }

    #[test]
    fn event_payload_is_plain_serializable_data() {
        let snapshot = JobSnapshot {
            job_id: "job-1".into(),
            kind: TransferKind::Upload,
            key: key("uploads/a.bin"),
            backend: BackendKind::S3,
            state: JobState::Running,
            bytes_transferred: 512,
            bytes_total: Some(1024),
            started_at: Utc::now(),
            ended_at: None,
            error: None,
        };
        let event = JobEvent::from_snapshot(&snapshot, 7, false);
        assert_eq!(event.percent, Some(50));
        assert_eq!(event.coalesced_events, 7);
        assert!(!event.terminal);
        assert!(event.text.contains("upload uploads/a.bin (s3) is running"));

        let encoded = serde_json::to_value(&event).unwrap();
        assert_eq!(encoded["kind"], "upload");
        assert_eq!(encoded["state"], "running");
        assert_eq!(encoded["key"], "uploads/a.bin");
        let decoded: JobEvent = serde_json::from_value(encoded).unwrap();
        assert_eq!(decoded, event);
    }

    #[test]
    fn event_text_is_truncated_on_a_char_boundary() {
        let long = "é".repeat(MAX_EVENT_TEXT_CHARS + 10);
        let truncated = truncate_chars(&long, MAX_EVENT_TEXT_CHARS);
        assert_eq!(truncated.chars().count(), MAX_EVENT_TEXT_CHARS);
        assert!(truncated.ends_with('…'));
    }

    #[test]
    fn format_bytes_is_human_readable() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1024), "1.0 KiB");
        assert_eq!(format_bytes(1024 * 1024 * 3 / 2), "1.5 MiB");
    }

    #[test]
    fn snapshot_reports_throughput_and_summary() {
        let snapshot = JobSnapshot {
            job_id: "job-1".into(),
            kind: TransferKind::Download,
            key: key("uploads/a.bin"),
            backend: BackendKind::Local,
            state: JobState::Failed,
            bytes_transferred: 1024,
            bytes_total: Some(2048),
            started_at: Utc::now() - chrono::Duration::seconds(2),
            ended_at: Some(Utc::now()),
            error: Some("boom".into()),
        };
        assert!(snapshot.bytes_per_sec() > 0.0);
        assert!(snapshot.duration_secs() >= 1.9);
        let summary = snapshot.summary();
        assert!(summary.contains("download uploads/a.bin (local) is failed"));
        assert!(summary.contains("boom"));
    }

    #[test]
    fn registry_is_send_sync_and_shareable() {
        fn assert_bounds<T: Send + Sync + 'static>() {}
        assert_bounds::<AssetJobRegistry>();
        assert_bounds::<JobSubscription>();
        assert_bounds::<JobSnapshot>();

        let registry = Arc::new(AssetJobRegistry::new());
        assert_eq!(registry.max_jobs(), DEFAULT_MAX_JOBS);
        assert!(registry.jobs.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_subscription_that_is_dropped_does_not_leak_the_job() {
        let store: SharedAssetStore = Arc::new(ChunkedStore::new(64 * 1024, 1024, 0));
        let registry = AssetJobRegistry::new();
        let job_id = registry.spawn_upload(store, upload_request("uploads/a.bin", b"x"));
        {
            let mut subscription = registry.subscribe(&job_id, SubscribeOptions::default());
            let _ = subscription.next().await;
        }
        let snapshot = registry
            .wait_for_completion(&job_id, Some(Duration::from_secs(5)))
            .await
            .unwrap();
        assert_eq!(snapshot.state, JobState::Completed);
    }
}