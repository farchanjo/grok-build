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

async fn setup() -> (
    Arc<crate::session::acp_session::SessionActor>,
    Arc<McpClient>,
    tokio::sync::mpsc::UnboundedSender<McpClientEvent>,
) {
    let (gateway_tx, _gateway_rx) = tokio::sync::mpsc::unbounded_channel();
    let (persistence_tx, _persistence_rx) = mpsc::unbounded_channel();
    let (actor, _ev) = create_test_actor_ex(0, 256_000, 85, gateway_tx, persistence_tx).await;
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
        persistence_tx,
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

/// A push owned by a session that already exited is delivered locally and
/// re-stamped onto the holder, so delivery and the sheet stay consistent
/// (adoption, matching the task/wait paths).
#[tokio::test(flavor = "current_thread")]
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
