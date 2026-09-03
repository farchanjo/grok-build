//! "Subscribed Tools" sheet session-state tests: the session subscription
//! registry (subscribe-time labels + idempotent re-subscribe), liveness
//! classification, and dead marking through the resource pump's client-event
//! tee when an MCP client is removed.
//!
//! Contract under test:
//! - Subscribing an already-subscribed `(server, uri)` is idempotent — the
//!   existing subscription and its label are kept, nothing stacks.
//! - A subscribe-time label round-trips from the client through the session
//!   registry into `x.ai/mcp/subscriptions` list responses.
//! - A client close / config removal marks that server's subscriptions dead
//!   (visible rows, not dropped); a fresh `Ready` revives them.

use super::support::*;
use super::*;

use std::sync::Arc;
use std::time::Instant;

use crate::extensions::mcp::McpSubscriptionState;
use crate::session::acp_session::McpSubscriptionRecord;
use xai_grok_mcp::servers::McpClientEvent;

use super::mcp_push::{
    set_server_subscriptions_dead, subscription_state, upsert_subscription_record,
};

type SubscriptionRegistry = std::collections::HashMap<(String, String), McpSubscriptionRecord>;

/// Poll a predicate to completion, yielding to the LocalSet so the spawned
/// pump can make progress. Panics with `message` when it never settles.
async fn wait_until(pred: impl Fn() -> bool, message: &str) {
    for _ in 0..10_000 {
        if pred() {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("{message}");
}

#[test]
fn subscription_registry_upsert_is_idempotent_and_keeps_label() {
    let mut registry = SubscriptionRegistry::new();
    let now = Instant::now();

    let inserted = upsert_subscription_record(
        &mut registry,
        "ssh",
        "command://1",
        Some("session output".into()),
        now,
    );
    assert!(inserted, "first subscribe must record the subscription");

    // Re-subscribing the same (server, uri) is an idempotent no-op: nothing
    // stacks and the existing label is kept (never clobbered).
    let inserted_again = upsert_subscription_record(
        &mut registry,
        "ssh",
        "command://1",
        Some("renamed stream".into()),
        now,
    );
    assert!(
        !inserted_again,
        "subscribing an already-subscribed (server, uri) must surface as already subscribed"
    );
    assert_eq!(
        registry.len(),
        1,
        "a duplicate subscribe must never stack a duplicate entry"
    );
    assert_eq!(
        registry[&("ssh".to_string(), "command://1".to_string())]
            .label
            .as_deref(),
        Some("session output"),
        "the existing subscription+label must be kept on re-subscribe"
    );

    // A subscription recorded without a label may adopt one later (the
    // resource only exposed a name on a subsequent listing), but an existing
    // label is still never overwritten.
    upsert_subscription_record(&mut registry, "ssh", "command://2", None, now);
    assert!(
        registry[&("ssh".to_string(), "command://2".to_string())]
            .label
            .is_none()
    );
    let adopted = upsert_subscription_record(
        &mut registry,
        "ssh",
        "command://2",
        Some("later name".into()),
        now,
    );
    assert!(!adopted, "second pass is still an already-subscribed no-op");
    assert_eq!(
        registry[&("ssh".to_string(), "command://2".to_string())]
            .label
            .as_deref(),
        Some("later name"),
        "a missing label may be adopted from a later subscribe/listing sync"
    );
}

#[test]
fn subscription_state_classification_thresholds() {
    let fresh = Instant::now();
    let old = Instant::now() - std::time::Duration::from_secs(6 * 60);
    let old_ms = Some(6 * 60 * 1000);
    let fresh_ms = Some(60 * 1000);

    // Fresh subscription, no pushes yet: still active (grace window).
    assert_eq!(
        subscription_state(false, None, None, Some(0)),
        McpSubscriptionState::Active
    );
    // Recent push: active.
    assert_eq!(
        subscription_state(false, Some(3), fresh_ms, Some(0)),
        McpSubscriptionState::Active
    );
    // Last push older than the idle window: idle.
    assert_eq!(
        subscription_state(false, Some(3), old_ms, None),
        McpSubscriptionState::Idle
    );
    // Never pushed, but subscribed long ago: idle ("never pushed").
    assert_eq!(
        subscription_state(
            false,
            Some(0),
            None,
            Some((Instant::now().duration_since(old).as_millis()) as u64)
        ),
        McpSubscriptionState::Idle
    );
    // Dead wins over every stat combination.
    assert_eq!(
        subscription_state(true, Some(3), fresh_ms, Some(0)),
        McpSubscriptionState::Dead
    );
    let _ = fresh;
    let _ = old;
}

#[tokio::test(flavor = "current_thread")]
async fn listing_keeps_label_and_marks_dead_when_client_removed() {
    // `create_test_actor_ex` internally `spawn_local`s (turn-task stubs),
    // so the body must run inside a LocalSet.
    tokio::task::LocalSet::new()
        .run_until(listing_keeps_label_and_marks_dead_when_client_removed_body())
        .await;
}

async fn listing_keeps_label_and_marks_dead_when_client_removed_body() {
    let (gateway_tx, _gateway_rx) = tokio::sync::mpsc::unbounded_channel();
    let (persistence_tx, _persistence_rx) = mpsc::unbounded_channel();
    let (actor, _ev) = create_test_actor_ex(0, 256_000, 85, gateway_tx, persistence_tx).await;
    let actor = Arc::new(actor);

    // Subscribe-time sync recorded a labeled subscription with pump stats;
    // afterwards the ssh client vanished from the session's MCP state
    // (process died / config removed) — the failure path.
    {
        let mut registry = actor.mcp_subscription_registry.lock();
        upsert_subscription_record(
            &mut registry,
            "ssh",
            "command://1",
            Some("session output".into()),
            Instant::now(),
        );
    }
    {
        let mut stats = actor.mcp_push_stats.lock();
        let entry = stats
            .entry(("ssh".to_string(), "command://1".to_string()))
            .or_default();
        entry.pushes = 3;
        entry.last_push = Some(Instant::now());
    }

    let rows = actor.list_mcp_subscriptions().await;
    assert_eq!(
        rows.len(),
        1,
        "the subscription must survive client removal instead of being dropped"
    );
    assert_eq!(rows[0].server, "ssh");
    assert_eq!(rows[0].uri, "command://1");
    assert_eq!(
        rows[0].label.as_deref(),
        Some("session output"),
        "the subscribe-time label must round-trip into the list response"
    );
    assert_eq!(rows[0].pushes_seen, Some(3), "pump stats must survive");
    assert_eq!(
        rows[0].state,
        Some(McpSubscriptionState::Dead),
        "a subscription whose client is gone must list as dead"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn pump_marks_subscriptions_dead_on_client_removal_and_revives_on_ready() {
    // Same LocalSet requirement as the sibling test above.
    tokio::task::LocalSet::new()
        .run_until(pump_marks_subscriptions_dead_on_client_removal_and_revives_on_ready_body())
        .await;
}

async fn pump_marks_subscriptions_dead_on_client_removal_and_revives_on_ready_body() {
    let (gateway_tx, _gateway_rx) = tokio::sync::mpsc::unbounded_channel();
    let (persistence_tx, _persistence_rx) = mpsc::unbounded_channel();
    let (actor, _ev) = create_test_actor_ex(0, 256_000, 85, gateway_tx, persistence_tx).await;
    let actor = Arc::new(actor);

    let key = ("ssh".to_string(), "command://1".to_string());
    {
        let mut registry = actor.mcp_subscription_registry.lock();
        upsert_subscription_record(
            &mut registry,
            "ssh",
            "command://1",
            Some("out".into()),
            Instant::now(),
        );
    }

    // Drive the pump's client-event tee like the run loop does.
    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel::<McpClientEvent>();
    crate::session::acp_session::spawn_mcp_resource_pump(actor.clone(), event_rx);

    // Client transport closed → the server's subscriptions go dead.
    event_tx
        .send(McpClientEvent::TransportClosed {
            server: "ssh".to_string(),
            client_id: 7,
        })
        .expect("send TransportClosed");
    wait_until(
        || {
            actor
                .mcp_subscription_registry
                .lock()
                .get(&key)
                .is_some_and(|record| record.dead)
        },
        "TransportClosed must mark the server's subscriptions dead",
    )
    .await;

    // A fresh Ready after a (re)handshake revives them.
    event_tx
        .send(McpClientEvent::Ready {
            server: "ssh".to_string(),
        })
        .expect("send Ready");
    wait_until(
        || {
            actor
                .mcp_subscription_registry
                .lock()
                .get(&key)
                .is_some_and(|record| !record.dead)
        },
        "Ready must revive the server's subscriptions",
    )
    .await;

    // Config removal marks dead again (the other removal path).
    event_tx
        .send(McpClientEvent::ConfigRemoved {
            server: "ssh".to_string(),
        })
        .expect("send ConfigRemoved");
    wait_until(
        || {
            actor
                .mcp_subscription_registry
                .lock()
                .get(&key)
                .is_some_and(|record| record.dead)
        },
        "ConfigRemoved must mark the server's subscriptions dead",
    )
    .await;

    // Other servers are untouched by ssh's removal.
    {
        let registry = actor.mcp_subscription_registry.lock();
        assert!(registry.keys().all(|(server, _)| server == "ssh"));
    }

    // Drop the tee sender: the pump exits its recv loop.
    drop(event_tx);
    wait_until(
        || {
            let registry = actor.mcp_subscription_registry.lock();
            registry.get(&key).is_some_and(|record| record.dead)
        },
        "registry state must remain stable after the pump exits",
    )
    .await;
}

#[test]
fn set_server_subscriptions_dead_only_touches_that_server() {
    let mut registry = SubscriptionRegistry::new();
    let now = Instant::now();
    upsert_subscription_record(&mut registry, "ssh", "command://1", None, now);
    upsert_subscription_record(&mut registry, "ssh", "command://2", None, now);
    upsert_subscription_record(&mut registry, "arithma", "res://x", None, now);

    set_server_subscriptions_dead(&mut registry, "ssh", true);
    assert!(
        registry
            .iter()
            .filter(|((server, _), _)| server == "ssh")
            .all(|(_, record)| record.dead),
        "all ssh subscriptions must be marked dead"
    );
    assert!(
        !registry[&("arithma".to_string(), "res://x".to_string())].dead,
        "other servers' subscriptions must be untouched"
    );

    set_server_subscriptions_dead(&mut registry, "ssh", false);
    assert!(
        registry.iter().all(|(_, record)| !record.dead),
        "revive must clear the dead marks it owns"
    );
}

#[test]
fn subscription_entry_wire_keeps_label_and_state_optional() {
    use crate::extensions::mcp::McpSubscriptionEntry;

    let labeled = McpSubscriptionEntry {
        server: "ssh".to_string(),
        uri: "command://1".to_string(),
        label: Some("session output".to_string()),
        pushes_seen: Some(3),
        last_push_ms_ago: Some(1200),
        unsubscribed: Some(false),
        state: Some(McpSubscriptionState::Idle),
    };
    let json = serde_json::to_value(&labeled).expect("serialize entry");
    assert_eq!(json["label"], "session output", "label rides the wire");
    assert_eq!(json["state"], "idle", "state serializes lowercase");
    assert_eq!(json["pushesSeen"], 3, "camelCase wire compat preserved");
    assert_eq!(
        json["lastPushMsAgo"], 1200,
        "camelCase wire compat preserved"
    );

    let bare = McpSubscriptionEntry {
        server: "ssh".to_string(),
        uri: "command://1".to_string(),
        label: None,
        pushes_seen: None,
        last_push_ms_ago: None,
        unsubscribed: None,
        state: None,
    };
    let json = serde_json::to_value(&bare).expect("serialize entry");
    assert!(
        json.get("label").is_none(),
        "label must be omitted when None so older wire consumers are unaffected"
    );
    assert!(
        json.get("state").is_none(),
        "state must be omitted when None (agent-level listing has no session stats)"
    );
}
