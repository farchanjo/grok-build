//! Process-wide one-way kill switch for the local session-search index.
//!
//! Once any hosted workspace turns search off, the process cannot reopen it:
//! updates skipped while closed would make an existing completion marker stale.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{LazyLock, RwLock};

use xai_grok_config_types::Resolved;

const UNAPPLIED: u8 = 0;
const OPEN: u8 = 1;
const CLOSED: u8 = 2;

static GATE: AtomicU8 = AtomicU8::new(UNAPPLIED);
static MUTATION_GATE: LazyLock<RwLock<()>> = LazyLock::new(|| RwLock::new(()));

pub(crate) fn apply(setting: &Resolved<bool>) {
    if !setting.value {
        // Serialize close with the final process-local mutation checks. Once
        // this returns, no write that observed the gate open can still start.
        let _write = MUTATION_GATE.write().unwrap_or_else(|e| e.into_inner());
        if GATE.swap(CLOSED, Ordering::AcqRel) != CLOSED {
            tracing::info!(source = %setting.source, "session search index turned off for this process");
        }
        return;
    }
    if GATE.compare_exchange(UNAPPLIED, OPEN, Ordering::AcqRel, Ordering::Acquire) == Err(CLOSED) {
        tracing::info!(source = %setting.source, "session search remains off until the next launch");
    }
}

pub(crate) fn is_enabled() -> bool {
    // Normal agent and standalone CLI startup apply the resolved gate before
    // reaching search. Default-open preserves compatibility for library users
    // that call the search API directly.
    GATE.load(Ordering::Acquire) != CLOSED
}

/// Hold a shared process-local mutation lease and check the gate at the final
/// write boundary. Gate closure takes the exclusive side, so a mutation that
/// returns from this closure cannot start after `apply(false)` returns.
pub(crate) fn with_mutation_gate<T>(mutate: impl FnOnce() -> T) -> Option<T> {
    let _read = MUTATION_GATE.read().unwrap_or_else(|e| e.into_inner());
    is_enabled().then(mutate)
}

#[cfg(test)]
#[must_use]
pub(crate) struct TestGateGuard(u8);

#[cfg(test)]
impl TestGateGuard {
    pub(crate) fn snapshot() -> Self {
        Self(GATE.load(Ordering::Acquire))
    }

    pub(crate) fn force_open() -> Self {
        let guard = Self::snapshot();
        GATE.store(OPEN, Ordering::Release);
        guard
    }
}

#[cfg(test)]
impl Drop for TestGateGuard {
    fn drop(&mut self) {
        GATE.store(self.0, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xai_grok_config_types::{ConfigSource, Resolved};

    #[test]
    #[serial_test::serial]
    fn closed_gate_never_reopens_in_process() {
        let _guard = TestGateGuard::snapshot();
        GATE.store(UNAPPLIED, Ordering::Release);
        apply(&Resolved::new(true, ConfigSource::Default));
        assert!(is_enabled());
        apply(&Resolved::new(false, ConfigSource::Env));
        apply(&Resolved::new(true, ConfigSource::Requirement));
        assert!(!is_enabled());
    }

    #[test]
    #[serial_test::serial]
    fn close_waits_for_in_flight_mutation_and_blocks_later_mutations() {
        let _guard = TestGateGuard::force_open();
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let (closed_tx, closed_rx) = std::sync::mpsc::channel();
        let mutation = std::thread::spawn(move || {
            assert_eq!(
                with_mutation_gate(|| {
                    entered_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    1
                }),
                Some(1)
            );
        });
        entered_rx.recv().unwrap();
        let close = std::thread::spawn(move || {
            apply(&Resolved::new(false, ConfigSource::Env));
            closed_tx.send(()).unwrap();
        });
        assert!(closed_rx.try_recv().is_err());
        release_tx.send(()).unwrap();
        mutation.join().unwrap();
        close.join().unwrap();
        closed_rx.recv().unwrap();
        assert_eq!(with_mutation_gate(|| 2), None);
    }
}
