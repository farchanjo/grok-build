//! Process-wide Claude CLI probe status cache.
//!
//! Catalog visibility consults this without spawning. Probe success/failure
//! updates the cache. Default is [`ClaudeCliProbeCacheState::NotProbed`]
//! (hidden / unselectable).

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long a successful probe remains trusted without re-check.
pub const PROBE_OK_TTL: Duration = Duration::from_secs(60);
/// How long a failed probe stays sticky before allowing a retry to flip UI.
pub const PROBE_FAIL_TTL: Duration = Duration::from_secs(15);

/// Cached probe outcome for catalog / status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaudeCliProbeCacheState {
    /// No successful probe yet (or cache cleared).
    NotProbed,
    /// Last probe succeeded.
    Ok { version: String },
    /// Last probe failed (gates open but binary missing/old/hang/etc.).
    Failed { message: String },
}

#[derive(Debug, Clone)]
struct CacheEntry {
    state: ClaudeCliProbeCacheState,
    at: Instant,
}

static PROBE_CACHE: Mutex<Option<CacheEntry>> = Mutex::new(None);

/// Snapshot of the cache for UI / catalog (applies TTL).
pub fn probe_cache_state() -> ClaudeCliProbeCacheState {
    let guard = PROBE_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    match guard.as_ref() {
        None => ClaudeCliProbeCacheState::NotProbed,
        Some(entry) => {
            let age = entry.at.elapsed();
            match &entry.state {
                ClaudeCliProbeCacheState::Ok { .. } if age > PROBE_OK_TTL => {
                    ClaudeCliProbeCacheState::NotProbed
                }
                ClaudeCliProbeCacheState::Failed { .. } if age > PROBE_FAIL_TTL => {
                    ClaudeCliProbeCacheState::NotProbed
                }
                other => other.clone(),
            }
        }
    }
}

/// `true` only when the last non-expired probe was successful.
pub fn probe_cache_ok() -> bool {
    matches!(probe_cache_state(), ClaudeCliProbeCacheState::Ok { .. })
}

/// Record a successful probe (version string).
pub fn record_probe_ok(version: impl Into<String>) {
    let mut guard = PROBE_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    *guard = Some(CacheEntry {
        state: ClaudeCliProbeCacheState::Ok {
            version: version.into(),
        },
        at: Instant::now(),
    });
}

/// Record a failed probe with a short user-facing message (no secrets).
pub fn record_probe_failed(message: impl Into<String>) {
    let mut msg = message.into();
    if msg.len() > 256 {
        msg.truncate(256);
        msg.push('…');
    }
    let mut guard = PROBE_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    *guard = Some(CacheEntry {
        state: ClaudeCliProbeCacheState::Failed { message: msg },
        at: Instant::now(),
    });
}

/// Clear the cache (tests / binary path change).
pub fn clear_probe_cache() {
    let mut guard = PROBE_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    *guard = None;
}

/// Whether catalog may show Claude CLI as selectable: gates + successful probe.
pub fn claude_cli_catalog_selectable() -> bool {
    crate::agent::external_runtime::gates::claude_cli_both_gates_open() && probe_cache_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[serial_test::serial(claude_cli_env)]
    fn default_not_probed_not_selectable() {
        clear_probe_cache();
        assert_eq!(probe_cache_state(), ClaudeCliProbeCacheState::NotProbed);
        assert!(!probe_cache_ok());
        // Without a successful probe, catalog stays non-selectable.
        assert!(!claude_cli_catalog_selectable());
        clear_probe_cache();
    }

    #[test]
    #[serial_test::serial(claude_cli_env)]
    fn success_then_fail_updates_cache() {
        clear_probe_cache();
        record_probe_ok("2.1.250");
        assert!(probe_cache_ok());
        assert!(matches!(
            probe_cache_state(),
            ClaudeCliProbeCacheState::Ok { version } if version == "2.1.250"
        ));
        record_probe_failed("missing binary");
        assert!(!probe_cache_ok());
        assert!(matches!(
            probe_cache_state(),
            ClaudeCliProbeCacheState::Failed { message } if message.contains("missing")
        ));
        clear_probe_cache();
    }
}
