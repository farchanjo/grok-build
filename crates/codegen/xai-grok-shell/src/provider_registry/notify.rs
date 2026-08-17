//! Machine-wide provider registry generation notifications.
//!
//! After durable management mutations (and external config reloads that bump
//! generation), emit `x.ai/providers/update` with only safe optional fields:
//! `schema_version`, `generation`, `changed_ids`, `changed_fields`.
//!
//! Delivery:
//! 1. Atomic shared-file under `$GROK_HOME/state/provider_registry_notify.json`
//!    (written by management; generation-coalescing poller/watchers read it).
//! 2. Multi-subscriber ACP gateway fanout when forwarders are registered
//!    (leader composition root + optional local self-refresh for `--no-leader`).
//!
//! Writes use unique-temp + rename. Pollers skip redelivery when the payload
//! generation is not newer than the last observed generation (coalescing).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use agent_client_protocol as acp;

type GatewayFn = Box<dyn Fn(acp::ExtNotification) + Send + Sync>;

static GATEWAYS: OnceLock<Mutex<Vec<GatewayFn>>> = OnceLock::new();
static POLL_STATE: OnceLock<Mutex<HashMap<PathBuf, u64>>> = OnceLock::new();

fn gateways() -> &'static Mutex<Vec<GatewayFn>> {
    GATEWAYS.get_or_init(|| Mutex::new(Vec::new()))
}

fn poll_state() -> &'static Mutex<HashMap<PathBuf, u64>> {
    POLL_STATE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Replace all registered forwarders with a single one (or clear when `None`).
///
/// Prefer [`register_providers_update_forwarder`] when multiple consumers
/// (e.g. two connected clients / local self-refresh + gateway) must both hear
/// the same mutation.
pub fn set_providers_update_forwarder(forward: Option<GatewayFn>) {
    if let Ok(mut slot) = gateways().lock() {
        slot.clear();
        if let Some(f) = forward {
            slot.push(f);
        }
    }
}

/// Register an additional process-wide forwarder (does not replace existing).
pub fn register_providers_update_forwarder(forward: GatewayFn) {
    if let Ok(mut slot) = gateways().lock() {
        slot.push(forward);
    }
}

/// Clear all registered forwarders (tests / shutdown).
pub fn clear_providers_update_forwarders() {
    if let Ok(mut slot) = gateways().lock() {
        slot.clear();
    }
}

/// Fire-and-forget providers/update to every registered forwarder.
pub fn try_forward_providers_update(params: &serde_json::Value) {
    let Ok(slot) = gateways().lock() else {
        return;
    };
    for fwd in slot.iter() {
        let Ok(raw) = serde_json::value::to_raw_value(params) else {
            continue;
        };
        let notif = acp::ExtNotification::new("x.ai/providers/update", raw.into());
        fwd(notif);
    }
}

/// Build the safe wire params object (never secrets).
pub fn providers_update_params(
    generation: u64,
    changed_ids: &[&str],
    changed_fields: &[String],
) -> serde_json::Value {
    serde_json::json!({
        "schema_version": 1,
        "generation": generation,
        "changed_ids": changed_ids,
        "changed_fields": changed_fields,
    })
}

/// Path of the generation-coalescing notify file for `home`.
pub fn notify_file_path(home: &Path) -> PathBuf {
    home.join("state/provider_registry_notify.json")
}

/// Read the notify file when its generation is newer than `last_seen`.
///
/// Returns `Some(params)` and advances the process poll cursor for `home` so
/// the same generation is not redelivered. Used by `--no-leader` self-refresh
/// and dual-client file bridges.
pub fn poll_notify_file_if_newer(home: &Path) -> Option<serde_json::Value> {
    let path = notify_file_path(home);
    let raw = std::fs::read_to_string(&path).ok()?;
    let val: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let registry_gen = val.get("generation").and_then(|g| g.as_u64()).unwrap_or(0);
    if registry_gen == 0 {
        return None;
    }
    let home_key = home.to_path_buf();
    if let Ok(mut map) = poll_state().lock() {
        let prev = map.get(&home_key).copied().unwrap_or(0);
        if registry_gen <= prev {
            return None;
        }
        map.insert(home_key, registry_gen);
    }
    Some(val)
}

/// Reset poll cursor (tests).
#[cfg(test)]
pub fn reset_poll_state_for_tests() {
    if let Ok(mut map) = poll_state().lock() {
        map.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::TempDir;

    #[test]
    fn forwarder_receives_method() {
        clear_providers_update_forwarders();
        let hits = Arc::new(AtomicUsize::new(0));
        let hits2 = hits.clone();
        set_providers_update_forwarder(Some(Box::new(move |n| {
            assert_eq!(n.method.as_ref(), "x.ai/providers/update");
            hits2.fetch_add(1, Ordering::SeqCst);
        })));
        try_forward_providers_update(&providers_update_params(3, &["lab"], &["enabled".into()]));
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        clear_providers_update_forwarders();
    }

    #[test]
    fn two_client_forwarders_both_receive_mutation_broadcast() {
        clear_providers_update_forwarders();
        let a = Arc::new(AtomicUsize::new(0));
        let b = Arc::new(AtomicUsize::new(0));
        let a2 = a.clone();
        let b2 = b.clone();
        register_providers_update_forwarder(Box::new(move |n| {
            assert_eq!(n.method.as_ref(), "x.ai/providers/update");
            a2.fetch_add(1, Ordering::SeqCst);
        }));
        register_providers_update_forwarder(Box::new(move |n| {
            assert_eq!(n.method.as_ref(), "x.ai/providers/update");
            b2.fetch_add(1, Ordering::SeqCst);
        }));
        try_forward_providers_update(&providers_update_params(9, &["lab"], &["enabled".into()]));
        assert_eq!(
            a.load(Ordering::SeqCst),
            1,
            "client A must receive broadcast"
        );
        assert_eq!(
            b.load(Ordering::SeqCst),
            1,
            "client B must receive broadcast"
        );
        clear_providers_update_forwarders();
    }

    #[test]
    fn notify_file_poll_coalesces_same_generation() {
        reset_poll_state_for_tests();
        let dir = TempDir::new().unwrap();
        let home = dir.path();
        let path = notify_file_path(home);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let payload = providers_update_params(4, &["lab"], &["enabled".into()]);
        let tmp = path.with_extension("tmp.test");
        std::fs::write(&tmp, serde_json::to_vec_pretty(&payload).unwrap()).unwrap();
        std::fs::rename(&tmp, &path).unwrap();
        let first = poll_notify_file_if_newer(home).expect("first poll delivers");
        assert_eq!(first["generation"], 4);
        assert!(
            poll_notify_file_if_newer(home).is_none(),
            "same generation must coalesce"
        );
        // Bump generation.
        let payload2 = providers_update_params(5, &["lab"], &["enabled".into()]);
        let tmp2 = path.with_extension("tmp.test2");
        std::fs::write(&tmp2, serde_json::to_vec_pretty(&payload2).unwrap()).unwrap();
        std::fs::rename(&tmp2, &path).unwrap();
        let second = poll_notify_file_if_newer(home).expect("newer generation delivers");
        assert_eq!(second["generation"], 5);
    }
}
