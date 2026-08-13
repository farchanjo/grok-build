//! Internal multi-account rollout gate.
//!
//! Disabled by default. This is a pure helper/constant plus env-parsed
//! configuration. It is intentionally *not* consumed by any existing
//! composition path yet, so it cannot change current behavior and never
//! publishes duplicate-account models.

/// Environment variable that enables the internal multi-account rollout.
///
/// Any of `1`, `true`, `on`, `yes` enables it; everything else (including the
/// unset default) keeps it disabled.
pub const MULTI_ACCOUNT_ROLLOUT_ENV: &str = "GROK_MULTI_ACCOUNT_ROLLOUT";

/// Default rollout state: disabled until an operator opts in.
pub const MULTI_ACCOUNT_ROLLOUT_DEFAULT_ENABLED: bool = false;

/// Whether the internal multi-account rollout gate is open.
pub fn multi_account_rollout_enabled() -> bool {
    let Some(raw) = std::env::var(MULTI_ACCOUNT_ROLLOUT_ENV).ok() else {
        return MULTI_ACCOUNT_ROLLOUT_DEFAULT_ENABLED;
    };
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "on" | "yes"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    /// Process-wide serialization for tests that mutate a shared env var.
    /// Required so the mutations are safe both under nextest (per-test
    /// processes) and under shared-process `cargo test`.
    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    /// Restores an env var on drop, panic-safe: restores the previous value or
    /// removes the var if it was absent when captured.
    struct EnvRestore {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvRestore {
        fn capture(key: &'static str) -> Self {
            Self {
                key,
                previous: std::env::var(key).ok(),
            }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            match &self.previous {
                Some(v) => unsafe { std::env::set_var(self.key, v) },
                None => unsafe { std::env::remove_var(self.key) },
            }
        }
    }

    fn with_env_mutation<F: FnOnce()>(f: F) {
        let _guard = ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _restore = EnvRestore::capture(MULTI_ACCOUNT_ROLLOUT_ENV);
        f();
    }

    #[test]
    fn rollout_gate_is_disabled_by_default_and_opt_in() {
        with_env_mutation(|| {
            unsafe { std::env::remove_var(MULTI_ACCOUNT_ROLLOUT_ENV) };
            assert!(
                !multi_account_rollout_enabled(),
                "absent env stays disabled"
            );

            for v in ["1", "true", "on", "yes", "TRUE"] {
                unsafe { std::env::set_var(MULTI_ACCOUNT_ROLLOUT_ENV, v) };
                assert!(multi_account_rollout_enabled(), "`{v}` should enable");
            }
            for v in ["0", "false", "off", "no", ""] {
                unsafe { std::env::set_var(MULTI_ACCOUNT_ROLLOUT_ENV, v) };
                assert!(!multi_account_rollout_enabled(), "`{v}` should not enable");
            }
        });
    }

    #[test]
    fn constant_default_is_disabled() {
        assert!(!MULTI_ACCOUNT_ROLLOUT_DEFAULT_ENABLED);
    }
}
