//! Machine-wide provider registry generation notifications.
//!
//! After durable management mutations (and external config reloads that bump
//! generation), emit `x.ai/providers/update` with only safe optional fields:
//! `schema_version`, `generation`, `changed_ids`, `changed_fields`.
//!
//! Delivery:
//! 1. Shared-file under `$GROK_HOME/state/provider_registry_notify.json`
//!    (written by management; reloader/clients poll or watch).
//! 2. Best-effort ACP gateway fanout when a sender is registered.

use std::sync::{Mutex, OnceLock};

use agent_client_protocol as acp;

type GatewayFn = Box<dyn Fn(acp::ExtNotification) + Send + Sync>;

static GATEWAY: OnceLock<Mutex<Option<GatewayFn>>> = OnceLock::new();

fn gateway_slot() -> &'static Mutex<Option<GatewayFn>> {
    GATEWAY.get_or_init(|| Mutex::new(None))
}

/// Register a process-wide forwarder (typically the ACP agent gateway).
pub fn set_providers_update_forwarder(forward: Option<GatewayFn>) {
    if let Ok(mut slot) = gateway_slot().lock() {
        *slot = forward;
    }
}

/// Fire-and-forget providers/update with a version-tolerant JSON payload.
pub fn try_forward_providers_update(params: &serde_json::Value) {
    let Ok(raw) = serde_json::value::to_raw_value(params) else {
        return;
    };
    let notif = acp::ExtNotification::new("x.ai/providers/update", raw.into());
    if let Ok(slot) = gateway_slot().lock() {
        if let Some(fwd) = slot.as_ref() {
            fwd(notif);
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn forwarder_receives_method() {
        let hits = Arc::new(AtomicUsize::new(0));
        let hits2 = hits.clone();
        set_providers_update_forwarder(Some(Box::new(move |n| {
            assert_eq!(n.method.as_ref(), "x.ai/providers/update");
            hits2.fetch_add(1, Ordering::SeqCst);
        })));
        try_forward_providers_update(&providers_update_params(3, &["lab"], &["enabled".into()]));
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        set_providers_update_forwarder(None);
    }
}
