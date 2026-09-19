//! Cross-session delivery targets for owner-stamped notifications.
//!
//! Several producers stamp an `owner_session_id` on their payloads (MCP
//! resource pushes, task/monitor completions, scheduled fires) so a push
//! created by one session can be delivered to that session even when the
//! underlying transport is shared. Sharing is real: a subagent inherits the
//! parent's `Arc<McpClient>`s through `SharedMcpPool` and reuses the parent's
//! terminal backend, while the client's single `notify_tx` slot keeps
//! pointing at the parent.
//!
//! This module is the resolution step: given an owner id and the id of the
//! session holding the event, it answers "deliver here" or "hand it to that
//! session's command channel". Owners that are unknown (legacy payloads
//! without a stamp) or already gone fall back to local delivery, which is the
//! same adoption rule the task and wait paths already use.
//!
//! Registration is by session spawn; unregistration happens on session exit.
//! A stale entry is harmless: the send fails and the caller falls back.
//!
//! The registry itself is a lock-free `ArcSwap` map, matching
//! `session::media_descriptors`: `resolve` never blocks the push path, and the
//! rare spawn/teardown writes clone-on-write through `rcu` so two sessions
//! spawning on their own threads merge instead of overwriting.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;
use tokio::sync::mpsc::UnboundedSender;

use crate::session::acp_session::{McpPushStats, McpSubscriptionRecord};
use crate::session::commands::SessionCommand;
use crate::session::persistence::PersistenceMsg;
use xai_grok_tools::reminders::task_completion::TaskCompletionReservations;

/// Live handle to another session's push-facing state.
///
/// Holds the owner's command channel plus the shared registries the resource
/// pump keeps push accounting in, so a routed push lands in the owner's
/// pending-notification queue *and* in the owner's "Subscribed Tools" sheet
/// with the same fidelity as a locally delivered one.
#[derive(Clone)]
pub(crate) struct SessionDeliveryTarget {
    pub(crate) session_id: String,
    pub(crate) cmd_tx: UnboundedSender<SessionCommand>,
    /// The owner's persistence channel: a routed frame is written to the
    /// owner's transcript, not to the transport holder's.
    pub(crate) persistence_tx: UnboundedSender<PersistenceMsg>,
    pub(crate) push_stats: Arc<Mutex<HashMap<(String, String), McpPushStats>>>,
    pub(crate) subscription_registry: Arc<Mutex<HashMap<(String, String), McpSubscriptionRecord>>>,
    /// The owner's task-completion reservations. A routed completion reserves
    /// its task id here so the owner's per-tool-call reminder does not surface
    /// the same completion a second time.
    pub(crate) task_completion_reservations: TaskCompletionReservations,
}

/// Where an owner-stamped notification must land.
pub(crate) enum Delivery {
    /// This session owns it, or the owner is unknown or gone.
    Local,
    /// A live sibling session owns it.
    Routed(SessionDeliveryTarget),
}

/// Snapshot map type of the registry. Readers load the current generation
/// without blocking; writers clone-on-write and publish through `rcu`.
pub(crate) type DeliveryTargetMap = HashMap<String, SessionDeliveryTarget>;

fn targets() -> &'static arc_swap::ArcSwap<DeliveryTargetMap> {
    static TARGETS: OnceLock<arc_swap::ArcSwap<DeliveryTargetMap>> = OnceLock::new();
    TARGETS.get_or_init(|| arc_swap::ArcSwap::from_pointee(HashMap::new()))
}

/// Register (or replace) the delivery target of one session.
///
/// `rcu` (not `store`) because sessions spawn from their own threads: two
/// concurrent registrations must merge, not overwrite each other.
pub(crate) fn register(target: SessionDeliveryTarget) {
    targets().rcu(|current| {
        let mut next = DeliveryTargetMap::clone(current);
        next.insert(target.session_id.clone(), target.clone());
        Arc::new(next)
    });
}

/// Drop a session's target. Idempotent.
pub(crate) fn unregister(session_id: &str) {
    targets().rcu(|current| {
        if !current.contains_key(session_id) {
            return Arc::clone(current);
        }
        let mut next = DeliveryTargetMap::clone(current);
        next.remove(session_id);
        Arc::new(next)
    });
}

/// Resolve one owner id to its target, if it is still registered.
pub(crate) fn resolve(owner: &str) -> Option<SessionDeliveryTarget> {
    targets().load().get(owner).cloned()
}

/// Decide where a notification owned by `owner` must be delivered, given the
/// session that currently holds the event.
///
/// Local when the owner is the current session, absent, or no longer
/// registered — a dead owner must not swallow the payload.
pub(crate) fn route(owner: Option<&str>, me: &str) -> Delivery {
    match owner {
        Some(owner) if owner != me => match resolve(owner) {
            Some(target) => Delivery::Routed(target),
            None => Delivery::Local,
        },
        _ => Delivery::Local,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(
        id: &str,
    ) -> (
        SessionDeliveryTarget,
        tokio::sync::mpsc::UnboundedReceiver<SessionCommand>,
    ) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (persistence_tx, _persistence_rx) = tokio::sync::mpsc::unbounded_channel();
        let target = SessionDeliveryTarget {
            session_id: id.to_string(),
            cmd_tx: tx,
            persistence_tx,
            push_stats: Arc::new(Mutex::new(HashMap::new())),
            subscription_registry: Arc::new(Mutex::new(HashMap::new())),
            task_completion_reservations: TaskCompletionReservations::default(),
        };
        (target, rx)
    }

    #[test]
    fn owner_is_self_delivers_locally() {
        register(target("a").0);
        assert!(matches!(route(Some("a"), "a"), Delivery::Local));
    }

    #[test]
    fn owner_is_unregistered_unknown_delivers_locally() {
        assert!(matches!(route(Some("ghost"), "a"), Delivery::Local));
    }

    #[test]
    fn missing_owner_delivers_locally() {
        assert!(matches!(route(None, "a"), Delivery::Local));
    }

    #[test]
    fn live_foreign_owner_routes_to_its_command_channel() {
        let (t, _rx) = target("child-delivery-test");
        register(t);
        match route(Some("child-delivery-test"), "parent") {
            Delivery::Routed(target) => {
                assert_eq!(target.session_id, "child-delivery-test");
                target
                    .cmd_tx
                    .send(SessionCommand::Shutdown)
                    .expect("routed target must accept a command");
            }
            Delivery::Local => panic!("live foreign owner must route"),
        }
        unregister("child-delivery-test");
    }

    #[test]
    fn concurrent_registrations_merge_instead_of_overwriting() {
        // Sessions spawn on their own threads; a plain `store` would drop one.
        let handles: Vec<_> = (0..8)
            .map(|i| {
                std::thread::spawn(move || {
                    let (target, _rx) = target(&format!("concurrent-{i}"));
                    register(target);
                })
            })
            .collect();
        for handle in handles {
            handle.join().expect("registration thread must not panic");
        }
        for i in 0..8 {
            let id = format!("concurrent-{i}");
            assert!(
                matches!(route(Some(&id), "parent"), Delivery::Routed(_)),
                "{id} must still be registered after concurrent writes"
            );
            unregister(&id);
        }
    }

    #[test]
    fn unregister_makes_the_owner_local_again() {
        let (t, _rx) = target("child-unregister-test");
        register(t);
        unregister("child-unregister-test");
        assert!(matches!(
            route(Some("child-unregister-test"), "parent"),
            Delivery::Local
        ));
    }
}
