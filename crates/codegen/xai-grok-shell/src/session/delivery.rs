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
use std::sync::{Arc, OnceLock, Weak};

use parking_lot::Mutex;
use tokio::sync::mpsc::UnboundedSender;

use crate::session::acp_session::{McpPushStats, McpSubscriptionRecord};
use crate::session::commands::SessionCommand;
use crate::session::mcp_servers::{McpClient, McpState};
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
    /// The owner's MCP state, so a shared transport that reconnects under the
    /// holder can be re-pointed here instead of leaving this session on the
    /// dead client. `Weak`: a target never keeps a session alive.
    pub(crate) mcp_state: Weak<tokio::sync::Mutex<McpState>>,
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

/// Drop a session's target only when it still belongs to `cmd_tx`.
///
/// Registration is keyed by session id, and an id can be reused (a caller
/// pinning `task_id`, a client re-sending `session/new` with the same
/// `_meta.sessionId`). A plain [`unregister`] from the *old* actor's teardown
/// would then delete the entry the new actor just published, silently dropping
/// that session's routing back to the transport holder. `same_channel`
/// distinguishes the two actors.
pub(crate) fn unregister_if_same(session_id: &str, cmd_tx: &UnboundedSender<SessionCommand>) {
    targets().rcu(|current| {
        match current.get(session_id) {
            Some(target) if target.cmd_tx.same_channel(cmd_tx) => {
                let mut next = DeliveryTargetMap::clone(current);
                next.remove(session_id);
                Arc::new(next)
            }
            // Absent, or already replaced by a newer actor for this id.
            _ => Arc::clone(current),
        }
    });
}

/// Hand a freshly connected shared client to every live session that imported
/// it.
///
/// A respawn builds a new `McpClient` and installs it in the *respawning*
/// session's `owned_clients`; sessions that imported the old one through
/// `SharedMcpPool` keep the dead `Arc` and would run their next tool call on a
/// closed transport. Only `shared_clients` entries are touched — the respawner
/// already holds it as owned.
pub(crate) async fn broadcast_shared_client(server: &str, client: &Arc<McpClient>) {
    let registered: Vec<SessionDeliveryTarget> = targets().load().values().cloned().collect();
    for target in registered {
        let Some(state) = target.mcp_state.upgrade() else {
            continue;
        };
        let mut state = state.lock().await;
        if state.shared_clients.contains_key(server) {
            state
                .shared_clients
                .insert(server.to_string(), Arc::clone(client));
            tracing::info!(
                server,
                session_id = %target.session_id,
                "shared MCP client re-pointed at the fresh transport"
            );
        }
    }
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
            mcp_state: Weak::new(),
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

    /// A session id reused by a newer actor must survive the older actor's
    /// teardown: the old cleanup removes only its own entry.
    #[test]
    fn unregister_if_same_spares_a_newer_actor_for_the_same_id() {
        let (older, _older_rx) = target("reused-id");
        register(older.clone());
        let (newer, _newer_rx) = target("reused-id");
        register(newer.clone());

        unregister_if_same("reused-id", &older.cmd_tx);
        assert!(
            matches!(route(Some("reused-id"), "parent"), Delivery::Routed(_)),
            "the newer actor's target must survive the older actor's teardown"
        );

        unregister_if_same("reused-id", &newer.cmd_tx);
        assert!(matches!(
            route(Some("reused-id"), "parent"),
            Delivery::Local
        ));
    }

    /// An unknown id (or one whose entry was already replaced) is a no-op.
    #[test]
    fn unregister_if_same_ignores_absent_and_foreign_entries() {
        let (mine, _mine_rx) = target("absent-probe");
        unregister_if_same("absent-probe", &mine.cmd_tx);
        assert!(matches!(
            route(Some("absent-probe"), "parent"),
            Delivery::Local
        ));

        let (owner, _owner_rx) = target("kept-id");
        register(owner.clone());
        let (foreign, _foreign_rx) = target("foreign-id");
        unregister_if_same("kept-id", &foreign.cmd_tx);
        assert!(
            matches!(route(Some("kept-id"), "parent"), Delivery::Routed(_)),
            "a mismatched channel must not delete the entry"
        );
        unregister("kept-id");
    }

    /// A respawned shared client is handed to the sessions that imported it,
    /// and only to them.
    #[tokio::test]
    async fn broadcast_shared_client_repoints_only_importers() {
        let importer_state = Arc::new(tokio::sync::Mutex::new(
            crate::session::mcp_servers::McpState::new(vec![]),
        ));
        let holder_state = Arc::new(tokio::sync::Mutex::new(
            crate::session::mcp_servers::McpState::new(vec![]),
        ));
        let bystander_state = Arc::new(tokio::sync::Mutex::new(
            crate::session::mcp_servers::McpState::new(vec![]),
        ));
        let dead = Arc::new(crate::session::mcp_servers::McpClient::stub("srv"));
        importer_state
            .lock()
            .await
            .shared_clients
            .insert("srv".to_string(), Arc::clone(&dead));
        holder_state
            .lock()
            .await
            .owned_clients
            .insert("srv".to_string(), Arc::clone(&dead));

        for (id, state) in [
            ("importer", &importer_state),
            ("holder", &holder_state),
            ("bystander", &bystander_state),
        ] {
            let (mut t, _rx) = target(id);
            t.mcp_state = Arc::downgrade(state);
            register(t);
        }

        let fresh = Arc::new(crate::session::mcp_servers::McpClient::stub("srv"));
        broadcast_shared_client("srv", &fresh).await;

        assert!(
            Arc::ptr_eq(&importer_state.lock().await.shared_clients["srv"], &fresh),
            "an importer must be re-pointed at the fresh transport"
        );
        assert!(
            Arc::ptr_eq(&holder_state.lock().await.owned_clients["srv"], &dead),
            "the respawner already installed it as owned; broadcast must not touch it"
        );
        assert!(
            bystander_state.lock().await.shared_clients.is_empty(),
            "a session without the client stays untouched"
        );

        for id in ["importer", "holder", "bystander"] {
            unregister(id);
        }
    }

    /// A target whose session is gone (dropped `Arc`) is skipped, not fatal.
    #[tokio::test]
    async fn broadcast_shared_client_skips_dropped_sessions() {
        let (mut t, _rx) = target("gone");
        t.mcp_state = Weak::new();
        register(t);
        let fresh = Arc::new(crate::session::mcp_servers::McpClient::stub("srv"));
        broadcast_shared_client("srv", &fresh).await;
        unregister("gone");
    }
}
