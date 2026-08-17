//! Internal multi-account rollout gate (Gate D).
//!
//! Multi-account selection is **default enabled** after Gate D. The environment
//! variable remains an explicit rollback kill switch:
//! - absent → enabled
//! - `1` / `true` / `on` / `yes` → enabled
//! - `0` / `false` / `off` / `no` (and other non-enable values) → disabled

/// Environment variable that controls the multi-account rollout kill switch.
///
/// Any of `1`, `true`, `on`, `yes` enables it; any other set value disables it.
/// When unset, the default (enabled after Gate D) applies.
pub const MULTI_ACCOUNT_ROLLOUT_ENV: &str = "GROK_MULTI_ACCOUNT_ROLLOUT";

/// Default rollout state: enabled after Gate D acceptance.
pub const MULTI_ACCOUNT_ROLLOUT_DEFAULT_ENABLED: bool = true;

/// Whether the multi-account rollout gate is open.
pub fn multi_account_rollout_enabled() -> bool {
    let Some(raw) = std::env::var(MULTI_ACCOUNT_ROLLOUT_ENV).ok() else {
        return MULTI_ACCOUNT_ROLLOUT_DEFAULT_ENABLED;
    };
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "on" | "yes"
    )
}

// ---------------------------------------------------------------------------
// Shared-process env lock (tests only)
// ---------------------------------------------------------------------------
//
// Under shared-process `cargo test`, every mutation of
// `GROK_MULTI_ACCOUNT_ROLLOUT` — and every publish/refresh that *reads* the
// gate — must hold this lock. Nextest (one process per test) is fine either
// way; the lock is still required for `cargo test` compatibility.

#[cfg(test)]
mod env_lock {
    use std::sync::{Mutex, MutexGuard, OnceLock};

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    /// Acquire the process-wide multi-account rollout env lock.
    pub(crate) fn acquire() -> MutexGuard<'static, ()> {
        ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Process-wide lock for tests that mutate or depend on
/// [`MULTI_ACCOUNT_ROLLOUT_ENV`]. Hold across env set/restore **and** any
/// gate-reading publish/refresh.
#[cfg(test)]
pub(crate) fn multi_account_rollout_env_lock() -> std::sync::MutexGuard<'static, ()> {
    env_lock::acquire()
}

/// Run `f` while holding the multi-account rollout env lock and restoring the
/// previous env value on exit (panic-safe).
#[cfg(test)]
pub(crate) fn with_multi_account_rollout_env<F: FnOnce()>(f: F) {
    let _guard = multi_account_rollout_env_lock();
    let previous = std::env::var(MULTI_ACCOUNT_ROLLOUT_ENV).ok();
    struct Restore(Option<String>);
    impl Drop for Restore {
        fn drop(&mut self) {
            match &self.0 {
                Some(v) => unsafe { std::env::set_var(MULTI_ACCOUNT_ROLLOUT_ENV, v) },
                None => unsafe { std::env::remove_var(MULTI_ACCOUNT_ROLLOUT_ENV) },
            }
        }
    }
    let _restore = Restore(previous);
    f();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rollout_gate_is_enabled_by_default_with_explicit_kill_switch() {
        with_multi_account_rollout_env(|| {
            unsafe { std::env::remove_var(MULTI_ACCOUNT_ROLLOUT_ENV) };
            assert!(
                multi_account_rollout_enabled(),
                "absent env stays enabled after Gate D"
            );

            for v in ["1", "true", "on", "yes", "TRUE"] {
                unsafe { std::env::set_var(MULTI_ACCOUNT_ROLLOUT_ENV, v) };
                assert!(multi_account_rollout_enabled(), "`{v}` should enable");
            }
            for v in ["0", "false", "off", "no", ""] {
                unsafe { std::env::set_var(MULTI_ACCOUNT_ROLLOUT_ENV, v) };
                assert!(!multi_account_rollout_enabled(), "`{v}` should disable");
            }
        });
    }

    #[test]
    fn constant_default_is_enabled() {
        // Hold the shared lock so this read cannot race a concurrent mutator.
        let _guard = multi_account_rollout_env_lock();
        assert!(MULTI_ACCOUNT_ROLLOUT_DEFAULT_ENABLED);
    }
}
