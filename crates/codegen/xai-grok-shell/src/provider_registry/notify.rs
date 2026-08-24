//! Machine-wide provider registry generation notifications.
//!
//! After durable management mutations (and external config reloads that bump
//! generation), emit `x.ai/providers/update` with only safe optional fields:
//! `schema_version`, `generation`, `changed_ids`, `changed_fields`.
//!
//! # Delivery architecture
//!
//! 1. **Atomic notify file** under `$GROK_HOME/state/provider_registry_notify.json`
//!    (unique-temp + rename). Written by `ProviderManagementService` after every
//!    durable mutation, partial force-remove, and external fingerprint reconcile.
//! 2. **Multi-subscriber gateway fanout** via
//!    [`register_providers_update_forwarder`] / [`try_forward_providers_update`].
//!    The MvpAgent composition root registers the ACP gateway so leader clients
//!    receive the ext notification when the mutation runs in the agent process.
//! 3. **File-poll for TUI / `--no-leader`**: pager mutations run
//!    `ProviderManagementService` **in the pager process**. Peer agents and
//!    local self-refresh observe advances by polling the notify file
//!    ([`poll_notify_file_if_newer`] / [`poll_notify_file_with_cursor`]).
//!    Each consumer keeps an independent generation cursor so the same write
//!    is delivered once per consumer and coalesced until the generation bumps.
//!
//! Writes use unique-temp + rename. Pollers skip redelivery when the payload
//! generation is not newer than the last observed generation (coalescing).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use agent_client_protocol as acp;

/// Arc forwarders so fanout can snapshot under the lock and invoke after release.
type GatewayFn = std::sync::Arc<dyn Fn(acp::ExtNotification) + Send + Sync>;

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
pub fn set_providers_update_forwarder(
    forward: Option<Box<dyn Fn(acp::ExtNotification) + Send + Sync>>,
) {
    if let Ok(mut slot) = gateways().lock() {
        slot.clear();
        if let Some(f) = forward {
            slot.push(std::sync::Arc::from(f));
        }
    }
}

/// Register an additional process-wide forwarder (does not replace existing).
pub fn register_providers_update_forwarder(
    forward: Box<dyn Fn(acp::ExtNotification) + Send + Sync>,
) {
    if let Ok(mut slot) = gateways().lock() {
        slot.push(std::sync::Arc::from(forward));
    }
}

/// Clear all registered forwarders (tests / shutdown).
pub fn clear_providers_update_forwarders() {
    if let Ok(mut slot) = gateways().lock() {
        slot.clear();
    }
}

/// Fire-and-forget providers/update to every registered forwarder.
///
/// Snapshots the list under the lock and invokes after release so subscribers
/// may perform disk I/O without holding the gateway mutex.
pub fn try_forward_providers_update(params: &serde_json::Value) {
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

/// Independent consumer poll: compare file generation against a caller-owned
/// cursor (not process-global). Two clients each keep their own `cursor` and
/// both observe the same mutation write once, with generation coalescing.
///
/// This is the multi-client file-poll contract for pager-local mutations under
/// `--no-leader` and for tests that model two isolated consumers.
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

    /// Two independent consumers (separate cursors) both observe a pager-local
    /// style mutation write with generation coalescing — Gate D multi-client proof.
    #[test]
    fn two_independent_poll_consumers_observe_same_mutation_write() {
        use crate::provider_registry::management::ProviderManagementService;
        use crate::provider_registry::management::dto::ProviderAddRequest;

        let dir = TempDir::new().unwrap();
        let home = dir.path();
        // Consumer A and B start with independent cursors (not process-global).
        let mut cursor_a: u64 = 0;
        let mut cursor_b: u64 = 0;
        assert!(
            poll_notify_file_with_cursor(home, &mut cursor_a).is_none(),
            "no file yet"
        );
        assert!(poll_notify_file_with_cursor(home, &mut cursor_b).is_none());

        // Mutation in this "pager process" (management service writes notify file).
        let svc = ProviderManagementService::new(home);
        let g0 = svc.current_generation();
        assert!(
            svc.add(ProviderAddRequest {
                id: "lab".into(),
                kind: "openai_compatible".into(),
                base_url: "http://127.0.0.1:9/v1".into(),
                display_name: None,
                admin_base_url: None,
                enabled: true,
                expected_generation: g0,
            })
            .ok
        );

        let a = poll_notify_file_with_cursor(home, &mut cursor_a)
            .expect("consumer A must observe mutation");
        let b = poll_notify_file_with_cursor(home, &mut cursor_b)
            .expect("consumer B must observe mutation");
        assert_eq!(a["generation"], b["generation"]);
        assert!(a["generation"].as_u64().unwrap_or(0) > 0);
        // Coalesce: same generation not redelivered to either cursor.
        assert!(poll_notify_file_with_cursor(home, &mut cursor_a).is_none());
        assert!(poll_notify_file_with_cursor(home, &mut cursor_b).is_none());

        // Second mutation: both independent consumers observe the bump once.
        let g1 = svc.current_generation();
        assert!(svc.set_enabled("lab", false, g1).ok);
        let a2 = poll_notify_file_with_cursor(home, &mut cursor_a).expect("A sees disable");
        let b2 = poll_notify_file_with_cursor(home, &mut cursor_b).expect("B sees disable");
        assert_eq!(a2["generation"], b2["generation"]);
        assert!(a2["generation"].as_u64().unwrap() > a["generation"].as_u64().unwrap());
    }
}
