//! Injectable monotonic clock for deterministic deadline/cooldown tests.
//!
//! Mirrors the circuit-breaker clock pattern without adding a crate dependency.

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Monotonic time source for budgets and cooldowns.
pub trait Clock: Send + Sync + 'static {
    fn now(&self) -> Instant;
}

/// Production clock backed by [`Instant::now`].
#[derive(Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// Controllable clock: starts at construction time; advance via [`MockClock::advance`].
#[derive(Debug)]
pub struct MockClock {
    now: Mutex<Instant>,
}

impl MockClock {
    pub fn new() -> Self {
        Self {
            now: Mutex::new(Instant::now()),
        }
    }

    /// Advance the mock clock by `d`.
    pub fn advance(&self, d: Duration) {
        let mut g = self.now.lock().unwrap_or_else(|e| e.into_inner());
        *g = g.checked_add(d).expect("MockClock overflow");
    }

    /// Absolute set (tests that need a known base).
    pub fn set(&self, instant: Instant) {
        let mut g = self.now.lock().unwrap_or_else(|e| e.into_inner());
        *g = instant;
    }
}

impl Default for MockClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for MockClock {
    fn now(&self) -> Instant {
        *self.now.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// No-op sleeper trait for backoff (PR16 owns transport backoff; pipeline
/// only needs cancellable wait hooks for tests).
pub trait Sleeper: Send + Sync + 'static {
    fn sleep(
        &self,
        duration: Duration,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>>;
}

/// Tokio sleeper (production).
#[derive(Debug, Default)]
pub struct TokioSleeper;

impl Sleeper for TokioSleeper {
    fn sleep(
        &self,
        duration: Duration,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            if !duration.is_zero() {
                tokio::time::sleep(duration).await;
            }
        })
    }
}

/// Immediate sleeper (tests): does not wait wall clock.
#[derive(Debug, Default)]
pub struct ImmediateSleeper;

impl Sleeper for ImmediateSleeper {
    fn sleep(
        &self,
        _duration: Duration,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
        Box::pin(async {})
    }
}
