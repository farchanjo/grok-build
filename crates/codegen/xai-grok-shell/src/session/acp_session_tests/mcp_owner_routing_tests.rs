//! Owner-based MCP push routing.
//!
//! Contract under test: a `resources/updated` push is delivered to the
//! session that *subscribed* the URI, not to the session holding the
//! transport. Subagents inherit the parent's `Arc<McpClient>` (their tool
//! calls run on it and their post-call refresh subscribes on it), so before
//! this the parent received every child-owned stream while the child got
//! nothing. Unknown or dead owners fall back to local delivery (adoption).
//!
//! The MCP client here is an in-process ACP server stub, so the test exercises
//! the real subscribe path (handshake → `resources/list` → `resources/subscribe`)
//! and the real pump without spawning a child process.

use super::support::*;
use super::*;

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use xai_grok_mcp::servers::{McpClient, McpClientEvent};

const SERVER: &str = "stub";
const URI: &str = "command://stub/output";

/// Minimal MCP server over the ACP reverse channel: advertises resource
/// subscribe, lists one stream, answers reads with a fixed payload.
struct StubResourceServer;

#[async_trait::async_trait]
impl xai_grok_mcp::acp_transport::AcpReverseInvoker for StubResourceServer {
    async fn invoke(
        &self,
        _server_id: &str,
        message: Value,
        _timeout: Duration,
    ) -> Result<Value, String> {
        let id = message.get("id").cloned().unwrap_or(Value::Null);
        let method = message.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let result = match method {
            "initialize" => json!({
                "protocolVersion": "2025-06-18",
                "capabilities": { "resources": { "subscribe": true, "listChanged": true } },
                "serverInfo": { "name": "stub", "version": "0.0.1" },
            }),
            "resources/list" => json!({ "resources": [{ "uri": URI, "name": "stub stream" }] }),
            "resources/read" => json!({ "contents": [{ "uri": URI, "text": "payload" }] }),
            _ => json!({}),
        };
        Ok(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
    }
}

type ChildQueue = tokio::sync::mpsc::UnboundedReceiver<crate::session::commands::SessionCommand>;

/// Poll a predicate, letting real time pass so the pump's quiet window can
/// elapse.
async fn wait_for(pred: impl Fn() -> bool, message: &str) {
    for _ in 0..400 {
        if pred() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("{message}");
}

fn key() -> (String, String) {
    (SERVER.to_string(), URI.to_string())
}

/// Wait for the next queued command: accounting happens at accept time, the
/// injection only after the pump's quiet window elapses.
async fn wait_for_command(rx: &mut ChildQueue) -> crate::session::commands::SessionCommand {
    for _ in 0..400 {
        match rx.try_recv() {
            Ok(command) => return command,
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            Err(error) => panic!("owner channel closed: {error}"),
        }
    }
    panic!("owner never received the parked notification");
}

/// Actor with the stub client installed as an *owned* server, plus the event
/// lane left unwired so a test can wire it itself.
async fn setup_unwired() -> (
    Arc<crate::session::acp_session::SessionActor>,
    Arc<McpClient>,
) {
    setup_unwired_with(false).await
}

/// Same, with the subagent hint applied before the actor is shared (the field
/// is plain, so it must be set while the actor is still owned).
async fn setup_unwired_with(
    is_subagent: bool,
) -> (
    Arc<crate::session::acp_session::SessionActor>,
    Arc<McpClient>,
) {
    let (gateway_tx, _gateway_rx) = tokio::sync::mpsc::unbounded_channel();
    let (persistence_tx, _persistence_rx) = mpsc::unbounded_channel();
    let (mut actor, _ev) = create_test_actor_ex(0, 256_000, 85, gateway_tx, persistence_tx).await;
    actor.startup_hints.is_subagent = is_subagent;
    let actor = Arc::new(actor);

    let client = Arc::new(McpClient::new_acp(
        SERVER.to_string(),
        "stub-server".to_string(),
        Arc::new(StubResourceServer),
        None,
        None,
    ));
    {
        let mut state = actor.mcp_state.lock().await;
        state
            .configs
            .push(acp::McpServer::Http(acp::McpServerHttp::new(
                SERVER.to_string(),
                "http://127.0.0.1:9/mcp".to_string(),
            )));
        state
            .owned_clients
            .insert(SERVER.to_string(), Arc::clone(&client));
    }
    (actor, client)
}

async fn setup() -> (
    Arc<crate::session::acp_session::SessionActor>,
    Arc<McpClient>,
    tokio::sync::mpsc::UnboundedSender<McpClientEvent>,
) {
    let (actor, client) = setup_unwired().await;
    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel::<McpClientEvent>();
    crate::session::acp_session::spawn_mcp_resource_pump(actor.clone(), event_rx);
    (actor, client, event_tx)
}

/// Register a fake sibling session and return its queue plus its target.
fn register_child(id: &str) -> (ChildQueue, crate::session::delivery::SessionDeliveryTarget) {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let (persistence_tx, _persistence_rx) = tokio::sync::mpsc::unbounded_channel();
    crate::session::delivery::register(crate::session::delivery::SessionDeliveryTarget {
        session_id: id.to_string(),
        cmd_tx: tx,
        persistence: crate::session::persistence::PersistenceHandle::from_sender_for_test(
            persistence_tx,
        ),
        mcp_state: std::sync::Weak::new(),
        push_stats: Arc::new(parking_lot::Mutex::new(Default::default())),
        subscription_registry: Arc::new(parking_lot::Mutex::new(Default::default())),
        task_completion_reservations: Default::default(),
    });
    let target = crate::session::delivery::resolve(id).expect("target just registered");
    (rx, target)
}

/// A push for a URI owned by a *live foreign* session is queued on that
/// session's channel and accounted there — never on the holder's.
#[tokio::test(flavor = "current_thread")]
async fn push_owned_by_another_session_routes_to_it() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (actor, client, event_tx) = setup().await;
            // The child's tool call subscribed on the shared client.
            client
                .subscribe_all_resources(Some("child-session"))
                .await
                .expect("stub handshake + subscribe must succeed");

            let (mut child_rx, target) = register_child("child-session");

            event_tx
                .send(McpClientEvent::ResourceUpdated {
                    server: SERVER.to_string(),
                    uri: URI.to_string(),
                })
                .expect("send push");

            wait_for(
                || {
                    target
                        .push_stats
                        .lock()
                        .get(&key())
                        .is_some_and(|stats| stats.pushes == 1)
                },
                "the routed push must be accounted on the owner",
            )
            .await;

            let command = wait_for_command(&mut child_rx).await;
            match command {
                crate::session::commands::SessionCommand::InjectNotification { source, .. } => {
                    assert!(
                        matches!(
                            source,
                            crate::session::commands::NotificationSource::McpResourceUpdated { .. }
                        ),
                        "routed notification must keep its MCP source: {source:?}"
                    );
                }
                _ => panic!("expected InjectNotification"),
            }
            assert!(
                actor.mcp_push_stats.lock().is_empty(),
                "the transport holder must not account a child-owned push"
            );
            assert!(
                target.subscription_registry.lock().contains_key(&key()),
                "the owner's sheet registry must carry the routed row"
            );
            crate::session::delivery::unregister("child-session");
        })
        .await;
}

/// A push with no owner stamp (legacy subscription) stays local.
#[tokio::test(flavor = "current_thread")]
async fn push_without_owner_stays_local() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (actor, client, event_tx) = setup().await;
            client
                .subscribe_all_resources(None)
                .await
                .expect("stub handshake + subscribe must succeed");

            event_tx
                .send(McpClientEvent::ResourceUpdated {
                    server: SERVER.to_string(),
                    uri: URI.to_string(),
                })
                .expect("send push");

            wait_for(
                || {
                    actor
                        .mcp_push_stats
                        .lock()
                        .get(&key())
                        .is_some_and(|stats| stats.pushes == 1)
                },
                "an unstamped push must be accounted locally",
            )
            .await;
        })
        .await;
}

/// A subagent's own MCP server has no other consumer for its client events.
///
/// Shared clients keep the parent's sender, so the child's *own* clients need
/// the session's own lane; a client with no sender drops every
/// `ResourceUpdated` in `emit`, and the child's own streams would never arrive.
#[tokio::test(flavor = "current_thread")]
async fn subagent_own_client_events_reach_its_pump() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (actor, client) = setup_unwired_with(true).await;
            assert!(
                actor.mcp_state.lock().await.client_event_tx().is_none(),
                "no lane before wiring: the client would drop its events"
            );

            actor.wire_mcp_client_event_lane().await;
            let lane = actor
                .mcp_state
                .lock()
                .await
                .client_event_tx()
                .expect("a subagent must get its own client event lane");
            client
                .subscribe_all_resources(Some(actor.session_info.id.0.as_ref()))
                .await
                .expect("stub handshake + subscribe must succeed");

            // Emitted through the lane the wiring installed, not a test channel.
            lane.send(McpClientEvent::ResourceUpdated {
                server: SERVER.to_string(),
                uri: URI.to_string(),
            })
            .expect("send push");

            let stats = Arc::clone(&actor.mcp_push_stats);
            wait_for(
                || stats.lock().get(&key()).is_some_and(|s| s.pushes == 1),
                "the subagent's own pump must accept its client's push",
            )
            .await;
        })
        .await;
}

/// A respawn builds a fresh client with an empty owner map; the re-subscribe
/// sweep then stamps the *respawning* session on every URI. The recorded owners
/// must be put back, or a shared transport migrates a child's streams to the
/// parent on every reconnect.
#[tokio::test(flavor = "current_thread")]
async fn respawn_restores_previous_subscription_owners() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (actor, client) = setup_unwired().await;
            // The child's tool call subscribed on the shared transport.
            client
                .subscribe_all_resources(Some("child-session"))
                .await
                .expect("stub handshake + subscribe must succeed");
            let previous = actor.subscription_owners_of(SERVER).await;
            assert_eq!(previous.get(URI).map(String::as_str), Some("child-session"));

            // Respawn: a fresh client, re-subscribed by the holder.
            let fresh = McpClient::new_acp(
                SERVER.to_string(),
                "stub-server".to_string(),
                Arc::new(StubResourceServer),
                None,
                None,
            );
            fresh
                .subscribe_all_resources(Some(actor.session_info.id.0.as_ref()))
                .await
                .expect("fresh transport subscribes");
            assert_ne!(
                fresh.subscription_owner(URI).as_deref(),
                Some("child-session"),
                "the sweep alone stamps the respawner"
            );

            crate::session::acp_session::SessionActor::restore_subscription_owners(
                &fresh, &previous,
            );
            assert_eq!(
                fresh.subscription_owner(URI).as_deref(),
                Some("child-session"),
                "the child's stream must survive the transport respawn"
            );
        })
        .await;
}

/// The dispatcher evicts a dead client *before* the auto-restart builds the
/// replacement, so the owners must survive the eviction.
///
/// This is the failure a live respawn exposed: with the capture reading only the
/// installed client, the map was already empty by respawn time, the sweep
/// stamped every URI with the parent, and the child's stream silently migrated.
#[tokio::test(flavor = "current_thread")]
async fn dead_client_eviction_keeps_owners_for_the_respawn() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (actor, client) = setup_unwired().await;
            client
                .subscribe_all_resources(Some("child-session"))
                .await
                .expect("stub handshake + subscribe must succeed");

            // The dispatcher observes TransportClosed and evicts the client.
            crate::session::mcp_dispatcher::drop_dead_clients(
                &actor.mcp_state,
                &[crate::session::mcp_dispatcher::DeadClient {
                    server: SERVER.to_string(),
                    closed: [client.client_id()].into_iter().collect(),
                }],
            )
            .await;
            assert!(
                actor.mcp_state.lock().await.get_client(SERVER).is_none(),
                "the dead client must be evicted"
            );

            let previous = actor.subscription_owners_of(SERVER).await;
            assert_eq!(
                previous.get(URI).map(String::as_str),
                Some("child-session"),
                "the owners must survive the eviction so the respawn can restore them"
            );

            // Respawn: fresh client, sweep by the holder, then the restore.
            let fresh = McpClient::new_acp(
                SERVER.to_string(),
                "stub-server".to_string(),
                Arc::new(StubResourceServer),
                None,
                None,
            );
            fresh
                .subscribe_all_resources(Some(actor.session_info.id.0.as_ref()))
                .await
                .expect("fresh transport subscribes");
            assert_ne!(
                fresh.subscription_owner(URI).as_deref(),
                Some("child-session"),
                "the sweep alone stamps the respawner"
            );
            crate::session::acp_session::SessionActor::restore_subscription_owners(
                &fresh, &previous,
            );
            assert_eq!(
                fresh.subscription_owner(URI).as_deref(),
                Some("child-session"),
                "the child's stream must survive the respawn"
            );

            // The stash is consumed, so a later capture does not resurrect it.
            assert!(
                actor.subscription_owners_of(SERVER).await.is_empty(),
                "taking the stash must clear it"
            );
        })
        .await;
}

/// A live client's owners win over a stale stash from an earlier eviction.
#[tokio::test(flavor = "current_thread")]
async fn live_owners_supersede_a_stale_stash() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (actor, client) = setup_unwired().await;
            {
                let mut state = actor.mcp_state.lock().await;
                state.stash_subscription_owners(
                    SERVER,
                    [("res://stale/1".to_string(), "ghost".to_string())].into(),
                );
            }
            client
                .subscribe_all_resources(Some("child-session"))
                .await
                .expect("subscribe");

            let owners = actor.subscription_owners_of(SERVER).await;
            assert_eq!(owners.get(URI).map(String::as_str), Some("child-session"));
            assert!(
                !owners.contains_key("res://stale/1"),
                "the live client supersedes the stash"
            );
            assert!(
                actor
                    .mcp_state
                    .lock()
                    .await
                    .pending_restore_owners
                    .is_empty(),
                "the superseded stash is cleared"
            );
        })
        .await;
}

/// A push owned by a session that already exited is delivered locally and
/// re-stamped onto the holder, so delivery and the sheet stay consistent
/// (adoption, matching the task/wait paths).
async fn push_with_dead_owner_is_adopted_locally() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (actor, client, event_tx) = setup().await;
            client
                .subscribe_all_resources(Some("dead-child"))
                .await
                .expect("stub handshake + subscribe must succeed");
            let me = actor.session_info.id.0.to_string();

            event_tx
                .send(McpClientEvent::ResourceUpdated {
                    server: SERVER.to_string(),
                    uri: URI.to_string(),
                })
                .expect("send push");

            wait_for(
                || {
                    actor
                        .mcp_push_stats
                        .lock()
                        .get(&key())
                        .is_some_and(|stats| stats.pushes == 1)
                },
                "a dead owner must not swallow the push",
            )
            .await;
            assert_eq!(
                client.subscription_owner(URI).as_deref(),
                Some(me.as_str()),
                "an adopted stream is re-stamped onto the holder"
            );
        })
        .await;
}

/// The sheet lists a session's own streams only: a row owned by another
/// session on the shared client is not the holder's row.
#[tokio::test(flavor = "current_thread")]
async fn sheet_filters_rows_owned_by_another_session() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (actor, client, _event_tx) = setup().await;
            client
                .subscribe_all_resources(Some("someone-else"))
                .await
                .expect("stub handshake + subscribe must succeed");

            let rows = actor.list_mcp_subscriptions().await;
            assert!(
                rows.is_empty(),
                "a foreign-owned row must not list in this session's sheet: {rows:?}"
            );
            assert_eq!(
                client.subscribed_uris(),
                vec![URI.to_string()],
                "sanity: the shared client does hold the subscription"
            );

            // Same client, owner = this session → the row appears.
            let own = actor.session_info.id.0.to_string();
            client.reassign_subscription_owner(URI, &own);
            let rows = actor.list_mcp_subscriptions().await;
            assert_eq!(rows.len(), 1, "own rows must list: {rows:?}");
            assert_eq!(rows[0].uri, URI);
            assert_eq!(rows[0].owner_session_id.as_deref(), Some(own.as_str()));
        })
        .await;
}
