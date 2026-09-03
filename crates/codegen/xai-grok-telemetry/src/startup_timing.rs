//! Startup phase timing helpers for the unified log.
//!
//! Startup instrumentation emits one cheap info event per phase
//! (config load, session-history scan, model catalog, plugin/marketplace
//! load, auth init, ...) with a context object carrying:
//!
//! - `elapsed_ms`: milliseconds since the process-start anchor, so a
//!   timeline can be reconstructed from `unified.jsonl` alone;
//! - `duration_ms`: wall time spent inside the instrumented phase.
//!
//! Events carry counters only — never payloads (no config contents, no
//! session rows). The anchor is installed by [`anchor()`] at the top of the
//! composition root's `main()`; before that, the epoch falls back to first
//! use, which stays monotonic within the process.

use std::sync::LazyLock;
use std::time::Instant;

static PROCESS_START: LazyLock<Instant> = LazyLock::new(Instant::now);

/// Anchor the process-start epoch. Call once, as early as possible in
/// `main()`; later calls are no-ops (the first anchor wins).
pub fn anchor() {
    LazyLock::force(&PROCESS_START);
}

/// Milliseconds since the process-start anchor (or first use, pre-anchor).
pub fn elapsed_ms() -> u64 {
    PROCESS_START.elapsed().as_millis() as u64
}

/// Unified-log context for one finished startup phase: process elapsed time
/// plus the phase's own duration. `started` must be an `Instant` taken when
/// the phase began.
pub fn phase_ctx(started: &Instant) -> serde_json::Value {
    serde_json::json!({
        "elapsed_ms": elapsed_ms(),
        "duration_ms": started.elapsed().as_millis() as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_ctx_carries_elapsed_and_duration() {
        let started = Instant::now();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let ctx = phase_ctx(&started);
        let duration = ctx["duration_ms"].as_u64().expect("duration_ms u64");
        let elapsed = ctx["elapsed_ms"].as_u64().expect("elapsed_ms u64");
        assert!(elapsed >= duration, "elapsed must include the phase");
        // A 5 ms sleep must not measure as zero on any supported platform.
        assert!(duration < 5_000, "duration must be small, got {duration}");
    }

    #[test]
    fn elapsed_ms_is_monotonic() {
        let first = elapsed_ms();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let second = elapsed_ms();
        assert!(second >= first, "elapsed_ms must never go backwards");
    }

    #[test]
    fn phase_ctx_keys_are_stable() {
        let started = Instant::now();
        let ctx = phase_ctx(&started);
        let obj = ctx.as_object().expect("ctx is a JSON object");
        let mut keys: Vec<_> = obj.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, vec!["duration_ms", "elapsed_ms"]);
    }
}
