//! Worker-thread and blocking-pool policy for Tokio runtimes.
//!
//! Grok processes are I/O-bound. Capping each runtime avoids exhausting shared
//! host thread ceilings, while pre-warming a retained blocking pool keeps the
//! first mid-turn `spawn_blocking` from taking Tokio's empty-pool thread-create
//! failure path.

use std::io;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Maximum runtime worker threads for any Grok process.
pub const MAX_WORKER_THREADS: NonZeroUsize = NonZeroUsize::new(8).unwrap();

/// Maximum Tokio blocking threads per process-lifetime runtime.
pub const MAX_BLOCKING_THREADS: usize = 16;

/// Retain idle blocking workers for the process lifetime.
pub const BLOCKING_THREAD_KEEP_ALIVE: Duration = Duration::MAX;

const PREWARM_THREAD_WAIT: Duration = Duration::from_secs(5);

/// Pure policy function: `min(cores, MAX_WORKER_THREADS)`.
pub fn cap_worker_threads(cores: NonZeroUsize) -> NonZeroUsize {
    cores.min(MAX_WORKER_THREADS)
}

/// Read host parallelism and apply the worker cap. `GROK_WORKER_THREADS`
/// may lower the count but can never exceed the host or policy ceiling.
pub fn capped_worker_threads() -> NonZeroUsize {
    let cores = std::thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);
    worker_threads_from(std::env::var("GROK_WORKER_THREADS").ok().as_deref(), cores)
}

fn worker_threads_from(value: Option<&str>, cores: NonZeroUsize) -> NonZeroUsize {
    let ceiling = cap_worker_threads(cores);
    let Some(value) = value else {
        return ceiling;
    };
    match value
        .trim()
        .parse::<usize>()
        .ok()
        .and_then(NonZeroUsize::new)
    {
        Some(requested) => requested.min(ceiling),
        // Invalid or zero overrides fail closed to the host/policy ceiling.
        None => ceiling,
    }
}

/// Cap the blocking pool and keep idle workers for the runtime lifetime.
pub fn apply_blocking_pool(builder: &mut tokio::runtime::Builder) -> &mut tokio::runtime::Builder {
    builder
        .max_blocking_threads(MAX_BLOCKING_THREADS)
        .thread_keep_alive(BLOCKING_THREAD_KEEP_ALIVE)
}

/// Apply the blocking-pool policy, build the runtime, and pre-warm its pool.
///
/// Use this for process-lifetime runtimes. Short-lived or per-session runtimes
/// should call [`apply_blocking_pool`] without pre-warming sixteen threads.
pub fn build_with_blocking_pool(
    builder: &mut tokio::runtime::Builder,
) -> io::Result<tokio::runtime::Runtime> {
    let runtime = apply_blocking_pool(builder).build()?;
    prewarm_blocking_pool(runtime.handle())?;
    Ok(runtime)
}

/// Create [`MAX_BLOCKING_THREADS`] overlapping blocking workers.
pub fn prewarm_blocking_pool(handle: &tokio::runtime::Handle) -> io::Result<()> {
    prewarm_blocking_pool_n(handle, MAX_BLOCKING_THREADS, PREWARM_THREAD_WAIT)
}

/// Create `n` overlapping blocking workers, waiting at most `wait` in total.
/// Already-started workers are always released if pre-warming times out.
pub fn prewarm_blocking_pool_n(
    handle: &tokio::runtime::Handle,
    n: usize,
    wait: Duration,
) -> io::Result<()> {
    let release = Arc::new(AtomicBool::new(false));
    let workers = park_blocking_workers(handle, n, &release, wait)?;
    release_parked_workers(&release, &workers);
    Ok(())
}

fn park_blocking_workers(
    handle: &tokio::runtime::Handle,
    n: usize,
    release: &Arc<AtomicBool>,
    wait: Duration,
) -> io::Result<Vec<std::thread::Thread>> {
    if n == 0 {
        return Ok(Vec::new());
    }
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    for _ in 0..n {
        let ready_tx = ready_tx.clone();
        let release = Arc::clone(release);
        handle.spawn_blocking(move || {
            let _ = ready_tx.send(std::thread::current());
            while !release.load(Ordering::Acquire) {
                std::thread::park();
            }
        });
    }
    drop(ready_tx);

    let deadline = Instant::now().checked_add(wait);
    let mut workers = Vec::with_capacity(n);
    for started in 0..n {
        let worker = match deadline {
            Some(deadline) => ready_rx
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .ok(),
            None => ready_rx.recv().ok(),
        };
        match worker {
            Some(worker) => workers.push(worker),
            None => {
                release_parked_workers(release, &workers);
                while let Ok(worker) = ready_rx.try_recv() {
                    worker.unpark();
                }
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("blocking pool pre-warm stalled after {started} of {n} threads"),
                ));
            }
        }
    }
    Ok(workers)
}

fn release_parked_workers(release: &AtomicBool, workers: &[std::thread::Thread]) {
    release.store(true, Ordering::Release);
    for worker in workers {
        worker.unpark();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nonzero(value: usize) -> NonZeroUsize {
        NonZeroUsize::new(value).unwrap()
    }

    #[test]
    fn worker_cap_is_identity_below_max_and_clamps_above_it() {
        assert_eq!(cap_worker_threads(nonzero(1)), nonzero(1));
        assert_eq!(cap_worker_threads(nonzero(8)), nonzero(8));
        assert_eq!(cap_worker_threads(nonzero(360)), nonzero(8));
    }

    #[test]
    fn worker_override_can_lower_but_never_raise_the_cap() {
        let cores = nonzero(360);
        assert_eq!(worker_threads_from(None, cores), nonzero(8));
        assert_eq!(worker_threads_from(Some(" 4 "), cores), nonzero(4));
        assert_eq!(worker_threads_from(Some("16"), cores), nonzero(8));
        assert_eq!(worker_threads_from(Some("0"), cores), nonzero(8));
        assert_eq!(worker_threads_from(Some("invalid"), cores), nonzero(8));
        assert_eq!(worker_threads_from(Some("8"), nonzero(4)), nonzero(4));
    }

    #[test]
    fn blocking_pool_builds_prewarms_and_runs_work() {
        let mut builder = tokio::runtime::Builder::new_current_thread();
        let runtime = build_with_blocking_pool(&mut builder).expect("runtime build");
        runtime.block_on(async {
            tokio::task::spawn_blocking(|| 42)
                .await
                .expect("spawn_blocking")
        });
    }

    #[test]
    fn prewarm_timeout_releases_started_workers() {
        let mut builder = tokio::runtime::Builder::new_current_thread();
        builder.max_blocking_threads(1);
        let runtime = builder.build().expect("runtime build");
        let error = prewarm_blocking_pool_n(
            runtime.handle(),
            MAX_BLOCKING_THREADS,
            Duration::from_millis(80),
        )
        .expect_err("one worker cannot prewarm sixteen overlapping workers");
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);

        let (done_tx, done_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            drop(runtime);
            let _ = done_tx.send(());
        });
        assert!(
            done_rx.recv_timeout(Duration::from_secs(2)).is_ok(),
            "runtime drop hung on a parked prewarm worker"
        );
    }
}
