//! Machine-wide retrieval graph generation notifications.
//!
//! After durable management mutations, emit `x.ai/retrieval/update` with only
//! safe optional fields: `schema_version`, `generation`, `changed_fields`.
//! No raw config or secrets.
//!
//! Delivery mirrors provider notify:
//! 1. Atomic notify file under `$GROK_HOME/state/retrieval_graph_notify.json`
//! 2. Multi-subscriber gateway fanout
//! 3. File-poll for TUI / `--no-leader` with independent cursors

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use agent_client_protocol as acp;

/// Process-wide forwarder (Arc so fanout can snapshot under the lock and
/// invoke after release — no disk I/O while holding the gateway list).
type GatewayFn = std::sync::Arc<dyn Fn(acp::ExtNotification) + Send + Sync>;

static GATEWAYS: OnceLock<Mutex<Vec<GatewayFn>>> = OnceLock::new();
static POLL_STATE: OnceLock<Mutex<HashMap<PathBuf, u64>>> = OnceLock::new();

fn gateways() -> &'static Mutex<Vec<GatewayFn>> {
    GATEWAYS.get_or_init(|| Mutex::new(Vec::new()))
}

fn poll_state() -> &'static Mutex<HashMap<PathBuf, u64>> {
    POLL_STATE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn set_retrieval_update_forwarder(
    forward: Option<Box<dyn Fn(acp::ExtNotification) + Send + Sync>>,
) {
    if let Ok(mut slot) = gateways().lock() {
        slot.clear();
        if let Some(f) = forward {
            slot.push(std::sync::Arc::from(f));
        }
    }
}

/// Register an additional forwarder without replacing existing subscribers.
pub fn register_retrieval_update_forwarder(
    forward: Box<dyn Fn(acp::ExtNotification) + Send + Sync>,
) {
    if let Ok(mut slot) = gateways().lock() {
        slot.push(std::sync::Arc::from(forward));
    }
}

pub fn clear_retrieval_update_forwarders() {
    if let Ok(mut slot) = gateways().lock() {
        slot.clear();
    }
}

/// Snapshot forwarders under the lock, then invoke **after** release so
/// subscribers may perform disk I/O / registry rebuild without holding the
/// gateway list mutex (H2).
pub fn try_forward_retrieval_update(params: &serde_json::Value) {
    let snapshot: Vec<GatewayFn> = {
        let Ok(slot) = gateways().lock() else {
            return;
        };
        slot.iter().cloned().collect()
    };
    for fwd in snapshot {
        let Ok(raw) = serde_json::value::to_raw_value(params) else {
            continue;
        };
        let notif = acp::ExtNotification::new("x.ai/retrieval/update", raw.into());
        fwd(notif);
    }
}

pub fn retrieval_update_params(generation: u64, changed_fields: &[String]) -> serde_json::Value {
    serde_json::json!({
        "schema_version": 1,
        "generation": generation,
        "changed_fields": changed_fields,
    })
}

pub fn notify_file_path(home: &Path) -> PathBuf {
    home.join("state/retrieval_graph_notify.json")
}

/// Write notify file (unique-temp + rename) and fan out to gateways.
pub fn publish_retrieval_update(home: &Path, generation: u64, changed_fields: &[String]) {
    let params = retrieval_update_params(generation, changed_fields);
    let path = notify_file_path(home);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    if let Ok(bytes) = serde_json::to_vec_pretty(&params)
        && std::fs::write(&tmp, &bytes).is_ok()
    {
        let _ = std::fs::rename(&tmp, &path);
    } else {
        let _ = std::fs::remove_file(&tmp);
    }
    try_forward_retrieval_update(&params);
}

pub fn poll_notify_file_if_newer(home: &Path) -> Option<serde_json::Value> {
    let home_key = home.to_path_buf();
    let mut cursor = poll_state()
        .lock()
        .ok()
        .and_then(|map| map.get(&home_key).copied())
        .unwrap_or(0);
    let val = poll_notify_file_with_cursor(home, &mut cursor)?;
    if let Ok(mut map) = poll_state().lock() {
        map.insert(home_key, cursor);
    }
    Some(val)
}

pub fn poll_notify_file_with_cursor(home: &Path, last_seen: &mut u64) -> Option<serde_json::Value> {
    let path = notify_file_path(home);
    let raw = std::fs::read_to_string(&path).ok()?;
    let val: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let registry_gen = val.get("generation").and_then(|g| g.as_u64()).unwrap_or(0);
    if registry_gen == 0 || registry_gen <= *last_seen {
        return None;
    }
    *last_seen = registry_gen;
    Some(val)
}

#[cfg(test)]
pub fn reset_poll_state_for_tests() {
    if let Ok(mut map) = poll_state().lock() {
        map.clear();
    }
}

/// Test helper: whether the gateway list mutex can be acquired without blocking
/// (proves release-before-callback fanout when called from a forwarder body).
#[cfg(test)]
pub fn gateway_mutex_try_lock_ok_for_test() -> bool {
    match gateways().try_lock() {
        Ok(_) => true,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::TempDir;

    #[test]
    #[serial]
    fn forwarder_receives_method() {
        clear_retrieval_update_forwarders();
        let hits = Arc::new(AtomicUsize::new(0));
        let hits2 = hits.clone();
        set_retrieval_update_forwarder(Some(Box::new(move |n| {
            assert_eq!(n.method.as_ref(), "x.ai/retrieval/update");
            hits2.fetch_add(1, Ordering::SeqCst);
        })));
        try_forward_retrieval_update(&retrieval_update_params(3, &["prime".into()]));
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        clear_retrieval_update_forwarders();
    }

    #[test]
    #[serial]
    fn notify_file_poll_coalesces() {
        reset_poll_state_for_tests();
        let dir = TempDir::new().unwrap();
        publish_retrieval_update(dir.path(), 4, &["embedding_models".into()]);
        let first = poll_notify_file_if_newer(dir.path()).expect("first");
        assert_eq!(first["generation"], 4);
        assert!(poll_notify_file_if_newer(dir.path()).is_none());
        publish_retrieval_update(dir.path(), 5, &["prime".into()]);
        let second = poll_notify_file_if_newer(dir.path()).expect("second");
        assert_eq!(second["generation"], 5);
    }

    #[test]
    #[serial]
    fn forwarder_runs_after_gateway_mutex_release() {
        clear_retrieval_update_forwarders();
        let unlocked = Arc::new(AtomicUsize::new(0));
        let unlocked2 = unlocked.clone();
        register_retrieval_update_forwarder(Box::new(move |_n| {
            // While this body runs the fanout must have released the list lock.
            if gateway_mutex_try_lock_ok_for_test() {
                unlocked2.fetch_add(1, Ordering::SeqCst);
            }
        }));
        try_forward_retrieval_update(&retrieval_update_params(1, &["prime".into()]));
        assert_eq!(
            unlocked.load(Ordering::SeqCst),
            1,
            "forwarder must run after gateway mutex release"
        );
        clear_retrieval_update_forwarders();
    }
}
