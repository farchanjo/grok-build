//! MCP resource-push pump — delivers async subscription results to the model.
//!
//! An MCP server that advertised `resources.subscribe` pushes
//! `notifications/resources/updated` for subscribed URIs (for example the ssh
//! server's live `command://<id>/output` stream after the model calls
//! `sub_open`). Until now those pushes only reached the ACP client
//! (`x.ai/mcp/resource_updated`, dropped by the pager) — the model never saw
//! them, so it fell back to polling tools like `ssh_exec_output`.
//!
//! This pump taps the same client-event stream as the MCP status dispatcher
//! (via a tee in the run loop), reads the fresh resource content host-side,
//! coalesces bursts per `(server, uri)`, and parks the result as an
//! [`NotificationSource::McpResourceUpdated`] pending notification through
//! `SessionCommand::InjectNotification`. The existing idle / end-of-turn
//! notification drain then delivers it to the model as a synthetic
//! `NotificationDrain` turn — the "result arrives in async mode" contract:
//! after `sub_open` the model simply stops, and the output shows up on its own.

use super::*;
use xai_grok_mcp::servers::McpClientEvent;

/// Minimum gap between two accepted pushes for the same `(server, uri)`.
/// Mirrors the dispatcher's flood guard; chatty servers are throttled here
/// before any read is issued.
const MIN_PUSH_GAP: std::time::Duration = std::time::Duration::from_millis(150);

/// Quiet window: a flush fires this long after the LAST accepted push for a
/// `(server, uri)`. Deltas arriving inside the window keep accumulating.
const QUIET_WINDOW: std::time::Duration = std::time::Duration::from_millis(1200);

/// Absolute cap on how long a burst may keep accumulating before it is
/// flushed mid-stream (a continuously-printing command must not buffer
/// forever).
const MAX_WINDOW: std::time::Duration = std::time::Duration::from_secs(10);

/// Per-pending accumulated text cap. Beyond this, further text is dropped and
/// a truncation marker is recorded.
const MAX_PENDING_BYTES: usize = 16 * 1024;

/// One accumulating burst for a `(server, uri)` pair.
struct PendingUpdate {
    text: String,
    truncated: bool,
    first_at: std::time::Instant,
    last_at: std::time::Instant,
}

impl PendingUpdate {
    fn append(&mut self, text: &str) {
        if self.truncated {
            return;
        }
        let room = MAX_PENDING_BYTES.saturating_sub(self.text.len());
        if text.len() <= room {
            self.text.push_str(text);
            return;
        }
        let cut = floor_char_boundary(text, room);
        self.text.push_str(&text[..cut]);
        self.text.push_str("\n…(stream truncated at limit)\n");
        self.truncated = true;
    }

    fn ready(&self, now: std::time::Instant) -> bool {
        now.duration_since(self.last_at) >= QUIET_WINDOW
            || now.duration_since(self.first_at) >= MAX_WINDOW
    }
}

/// Largest byte index `<= end` that lies on a UTF-8 character boundary of
/// `text` (`str::floor_char_boundary` is unstable). `text` is always valid
/// UTF-8, so the walk terminates at 0.
fn floor_char_boundary(text: &str, end: usize) -> usize {
    let mut cut = end.min(text.len());
    while cut > 0 && !text.is_char_boundary(cut) {
        cut -= 1;
    }
    cut
}

/// Per-`(server, uri)` push stats for the "Subscribed Tools" sheet.
#[derive(Debug, Default, Clone)]
pub(crate) struct McpPushStats {
    /// Accepted (post-flood-guard) pushes seen by this session's pump.
    pub(crate) pushes: u64,
    /// Last accepted push, for an age readout.
    pub(crate) last_push: Option<std::time::Instant>,
    /// Set by the user-driven unsubscribe; the pump drops any buffered
    /// burst for the uri instead of injecting it after the fact.
    pub(crate) unsubscribed: bool,
}

/// One tracked subscription in the session's registry
/// ([`SessionActor::mcp_subscription_registry`]). Recorded at subscribe time
/// (label + first-seen) and mutated by the resource pump's client-event tee
/// (dead marks); survives client removal so the "Subscribed Tools" sheet can
/// keep showing the stream as `dead` instead of silently dropping the row.
#[derive(Debug, Default, Clone)]
pub(crate) struct McpSubscriptionRecord {
    /// Display label captured at subscribe time (resource title/name).
    pub(crate) label: Option<String>,
    /// The server's MCP client disconnected or its config was removed —
    /// the stream can no longer push.
    pub(crate) dead: bool,
    /// When the subscription was first recorded; feeds the never-pushed
    /// idle grace window.
    pub(crate) first_seen: Option<std::time::Instant>,
}

/// A subscription with no push for longer than this is `idle`; a
/// never-pushed subscription younger than this is still `active`
/// (waiting for the first push).
pub(crate) const SUBSCRIPTION_IDLE_AFTER: std::time::Duration =
    std::time::Duration::from_secs(5 * 60);

/// Idle threshold in milliseconds, derived from [`SUBSCRIPTION_IDLE_AFTER`].
const SUBSCRIPTION_IDLE_AFTER_MS: u64 = SUBSCRIPTION_IDLE_AFTER.as_millis() as u64;

/// Upsert one subscription into the session registry. Idempotent per
/// `(server, uri)`: an existing entry is kept as-is — its label is never
/// overwritten (only a missing label may be adopted) — and `false` is
/// returned to signal "already subscribed" to the caller.
pub(crate) fn upsert_subscription_record(
    registry: &mut std::collections::HashMap<(String, String), McpSubscriptionRecord>,
    server: &str,
    uri: &str,
    label: Option<String>,
    now: std::time::Instant,
) -> bool {
    match registry.get_mut(&(server.to_string(), uri.to_string())) {
        Some(record) => {
            if record.label.is_none() {
                record.label = label;
            }
            false
        }
        None => {
            registry.insert(
                (server.to_string(), uri.to_string()),
                McpSubscriptionRecord {
                    label,
                    dead: false,
                    first_seen: Some(now),
                },
            );
            true
        }
    }
}

/// Mark every subscription of `server` dead (client disconnected / config
/// removed) or revive it (a fresh `Ready` after a restart proves the
/// client can push again).
pub(crate) fn set_server_subscriptions_dead(
    registry: &mut std::collections::HashMap<(String, String), McpSubscriptionRecord>,
    server: &str,
    dead: bool,
) {
    for ((entry_server, _uri), record) in registry.iter_mut() {
        if entry_server == server {
            record.dead = dead;
        }
    }
}

/// Classify one subscription row's liveness. Pure so the thresholds are
/// unit-testable: `dead` wins, then last-push age over the idle window,
/// then never-pushed past the grace window; everything else is `active`.
pub(crate) fn subscription_state(
    dead: bool,
    pushes_seen: Option<u64>,
    last_push_ms_ago: Option<u64>,
    first_seen_age_ms: Option<u64>,
) -> crate::extensions::mcp::McpSubscriptionState {
    use crate::extensions::mcp::McpSubscriptionState;
    if dead {
        return McpSubscriptionState::Dead;
    }
    if last_push_ms_ago.is_some_and(|ms| ms > SUBSCRIPTION_IDLE_AFTER_MS) {
        return McpSubscriptionState::Idle;
    }
    if pushes_seen.unwrap_or(0) == 0
        && first_seen_age_ms.is_some_and(|ms| ms > SUBSCRIPTION_IDLE_AFTER_MS)
    {
        return McpSubscriptionState::Idle;
    }
    McpSubscriptionState::Active
}

/// Spawn the per-session pump on the session's local task set. The pump exits
/// when `rx` closes (session teardown); later `InjectNotification` sends fail
/// harmlessly against the closed command channel.
pub(crate) fn spawn_mcp_resource_pump(
    session: std::sync::Arc<SessionActor>,
    mut rx: tokio::sync::mpsc::UnboundedReceiver<McpClientEvent>,
) {
    tokio::task::spawn_local(async move {
        let mut pending: std::collections::HashMap<(String, String), PendingUpdate> =
            std::collections::HashMap::new();
        loop {
            let now = std::time::Instant::now();
            let wait = pending
                .values()
                .map(|p| p.last_at + QUIET_WINDOW)
                .min()
                .map_or(std::time::Duration::MAX, |deadline| {
                    deadline.saturating_duration_since(now)
                });
            match tokio::time::timeout(wait, rx.recv()).await {
                Ok(Some(event)) => match event {
                    McpClientEvent::ResourceUpdated { server, uri } => {
                        let now = std::time::Instant::now();
                        let key = (server.clone(), uri.clone());
                        // Per-key flood guard: skip reads that arrive too fast
                        // after the previous accepted push for the same stream.
                        if let Some(pending_update) = pending.get(&key)
                            && now.duration_since(pending_update.last_at) < MIN_PUSH_GAP
                        {
                            tracing::debug!(
                                server = %server,
                                uri = %uri,
                                "mcp push throttled (min-gap)"
                            );
                            continue;
                        }
                        match read_resource_text(&session, &server, &uri).await {
                            Ok(text) => {
                                record_push(&session, &key);
                                let entry = pending.entry(key).or_insert_with(|| PendingUpdate {
                                    text: String::new(),
                                    truncated: false,
                                    first_at: now,
                                    last_at: now,
                                });
                                entry.append(&text);
                                entry.last_at = now;
                            }
                            Err(error) => {
                                tracing::debug!(
                                    server = %server,
                                    uri = %uri,
                                    %error,
                                    "mcp resource read after push failed; skipping"
                                );
                            }
                        }
                    }
                    // Client-event tee: a closed transport or a removed
                    // config means the server's streams can no longer push.
                    // Mark its subscriptions dead instead of dropping them so
                    // the sheet explains what happened to the row.
                    McpClientEvent::TransportClosed { server, .. } => {
                        mark_server_subscriptions_dead(&session, server.as_str());
                    }
                    McpClientEvent::ConfigRemoved { server } => {
                        mark_server_subscriptions_dead(&session, server.as_str());
                    }
                    McpClientEvent::ConfigDiff { removed, .. } => {
                        for server in removed {
                            mark_server_subscriptions_dead(&session, server.as_str());
                        }
                    }
                    // A (re-)initialized client can push again — clear any
                    // stale dead mark left by a prior disconnect.
                    McpClientEvent::Ready { server } => {
                        revive_server_subscriptions(&session, server.as_str());
                    }
                    other => {
                        // Other client events belong to the status dispatcher.
                        let _ = other;
                    }
                },
                Ok(None) => break,
                Err(_elapsed) => {
                    // Quiet window elapsed for at least one burst: flush every
                    // ready pending update as one pending notification each.
                    let now = std::time::Instant::now();
                    let ready_keys: Vec<(String, String)> = pending
                        .iter()
                        .filter(|(_, p)| p.ready(now))
                        .map(|(k, _)| k.clone())
                        .collect();
                    for (server, uri) in ready_keys {
                        // A user unsubscribe parked mid-burst wins: drop the
                        // buffered text instead of injecting after the fact.
                        if session
                            .mcp_push_stats
                            .lock()
                            .get(&(server.clone(), uri.clone()))
                            .is_some_and(|stats| stats.unsubscribed)
                        {
                            pending.remove(&(server, uri));
                            continue;
                        }
                        if let Some(update) = pending.remove(&(server.clone(), uri.clone())) {
                            inject_resource_update(&session, &server, &uri, &update);
                        }
                    }
                }
            }
        }
    });
}

/// Record an accepted push in the session's stats map (feeds the
/// "Subscribed Tools" sheet's status column).
fn record_push(session: &SessionActor, key: &(String, String)) {
    let mut stats = session.mcp_push_stats.lock();
    let entry = stats.entry(key.clone()).or_default();
    entry.pushes = entry.pushes.saturating_add(1);
    entry.last_push = Some(std::time::Instant::now());
}

/// Pump client-event tee: the server's MCP client is gone (transport closed
/// or config removed) — flag its subscriptions dead so the sheet shows a
/// `dead` row instead of silently dropping the stream.
fn mark_server_subscriptions_dead(session: &SessionActor, server: &str) {
    let mut registry = session.mcp_subscription_registry.lock();
    let marked = registry
        .keys()
        .filter(|(entry_server, _)| entry_server == server)
        .count();
    set_server_subscriptions_dead(&mut registry, server, true);
    drop(registry);
    if marked > 0 {
        tracing::info!(
            server = %server,
            subscriptions = marked,
            "marked MCP subscriptions dead after client removal"
        );
    } else {
        tracing::debug!(
            server = %server,
            "MCP client removed; no tracked subscriptions to mark dead"
        );
    }
}

/// Pump client-event tee: the server's client reached `Ready` again after a
/// (re)handshake — clear stale dead marks so the sheet stops showing the
/// streams as dead.
fn revive_server_subscriptions(session: &SessionActor, server: &str) {
    let mut registry = session.mcp_subscription_registry.lock();
    set_server_subscriptions_dead(&mut registry, server, false);
}

/// Read a resource via the session's MCP state and join its text contents.
async fn read_resource_text(
    session: &SessionActor,
    server: &str,
    uri: &str,
) -> Result<String, String> {
    let response =
        crate::extensions::mcp::read_mcp_resource(&session.mcp_state, server, uri).await?;
    let text = response
        .contents
        .into_iter()
        .filter_map(|c| c.text)
        .collect::<Vec<_>>()
        .join("\n");
    if text.is_empty() {
        return Err("resource has no text content".to_string());
    }
    Ok(text)
}

/// Park the coalesced burst as a pending notification. The drain delivers it
/// as (part of) the next synthetic turn when the session is idle or the turn
/// ends — `Later` priority defers to turn end and never interrupts mid-turn.
fn inject_resource_update(session: &SessionActor, server: &str, uri: &str, update: &PendingUpdate) {
    let body = format!(
        "MCP push from server `{server}` (subscribed resource `{uri}`) — output arrived \
         automatically via the subscription; do NOT poll for it with other tools. If the \
         stream is finished, close the subscription:\n\n{}",
        update.text
    );
    let message = xai_grok_tools::reminders::wrap_reminder(&body);
    let sent = session
        .session_cmd_tx
        .send(SessionCommand::InjectNotification {
            prompt_id: format!("mcp-resource-{}", uuid::Uuid::now_v7()),
            prompt_blocks: vec![acp::ContentBlock::Text(acp::TextContent::new(message))],
            priority: crate::session::commands::NotificationPriority::Later,
            source: NotificationSource::McpResourceUpdated {
                server: server.to_string(),
                uri: uri.to_string(),
            },
        });
    if let Err(error) = sent {
        tracing::debug!(%error, "mcp push injection skipped (session command channel closed)");
    }
}

/// Record subscribe-time metadata (display label, first-seen) for one
/// server's subscriptions into a session registry. Free function so the
/// background MCP-init task can record labels through its `Arc` handle
/// without holding the actor. Idempotent per `(server, uri)`: an
/// already-subscribed stream keeps its existing label and is never stacked;
/// a successful subscribe also clears stale dead marks (the transport
/// demonstrably works).
pub(crate) fn sync_mcp_subscription_registry_into(
    registry: &parking_lot::Mutex<
        std::collections::HashMap<(String, String), McpSubscriptionRecord>,
    >,
    server_name: &str,
    client: &crate::session::mcp_servers::McpClient,
) {
    let now = std::time::Instant::now();
    let mut registry = registry.lock();
    for (uri, label) in client.subscribed_resources() {
        let inserted = upsert_subscription_record(&mut registry, server_name, &uri, label, now);
        if let Some(record) = registry.get_mut(&(server_name.to_string(), uri.clone())) {
            record.dead = false;
        }
        if !inserted {
            tracing::debug!(
                server = %server_name,
                uri = %uri,
                "already subscribed; kept existing subscription and label"
            );
        }
    }
}

impl SessionActor {
    /// Record subscribe-time metadata (display label, first-seen) for the
    /// given server's subscriptions into the session registry. Idempotent
    /// per `(server, uri)`: an already-subscribed stream keeps its existing
    /// label and is never stacked. A successful `resources/subscribe` proves
    /// the client's transport works, so stale dead marks are cleared for the
    /// URIs the client now tracks.
    pub(crate) fn sync_mcp_subscription_registry(
        &self,
        server_name: &str,
        client: &crate::session::mcp_servers::McpClient,
    ) {
        sync_mcp_subscription_registry_into(&self.mcp_subscription_registry, server_name, client);
    }

    /// Snapshot of this session's active MCP resource subscriptions with
    /// pump stats, for the pager's "Subscribed Tools" sheet. Live rows come
    /// from each client's subscribed set (config order, as before). Streams
    /// whose MCP client is gone (disconnected mid-list, config removed) stay
    /// visible and are marked `dead` instead of being dropped; user-tombstoned
    /// rows keep staying hidden.
    pub(crate) async fn list_mcp_subscriptions(
        &self,
    ) -> Vec<crate::extensions::mcp::McpSubscriptionEntry> {
        let ordered_clients: Vec<(
            String,
            std::sync::Arc<crate::session::mcp_servers::McpClient>,
        )> = {
            let state = self.mcp_state.lock().await;
            state
                .configs
                .iter()
                .filter_map(|config| {
                    let name = xai_grok_mcp::servers::mcp_server_name(config).to_string();
                    state
                        .get_client(&name)
                        .cloned()
                        .map(|client| (name, client))
                })
                .collect()
        };
        let now = std::time::Instant::now();

        // Subscribe-time label plumbing catch-all: sync live clients into
        // the registry so labels/first-seen exist for subscriptions recorded
        // outside the explicit subscribe call sites (e.g. the post-tool-call
        // refresh inside the MCP layer). Never revives — liveness is the
        // pump's call, client presence alone is not proof.
        {
            let mut registry = self.mcp_subscription_registry.lock();
            for (server, client) in &ordered_clients {
                for (uri, label) in client.subscribed_resources() {
                    upsert_subscription_record(&mut registry, server, &uri, label, now);
                }
            }
        }

        let registry = self.mcp_subscription_registry.lock();
        let stats = self.mcp_push_stats.lock();
        let live_uris: std::collections::HashSet<(String, String)> = ordered_clients
            .iter()
            .flat_map(|(server, client)| {
                client
                    .subscribed_uris()
                    .into_iter()
                    .map(move |uri| (server.clone(), uri))
            })
            .collect();

        let mut entries = Vec::new();
        // Live rows in config order (unchanged source of truth for live servers).
        for (server, client) in &ordered_clients {
            for uri in client.subscribed_uris() {
                let key = (server.clone(), uri.clone());
                let record = registry.get(&key);
                let stat = stats.get(&key);
                // Tombstoned rows keep staying out of the listing (the user
                // dropped them; refreshes will not re-subscribe).
                if stat.is_some_and(|s| s.unsubscribed) {
                    continue;
                }
                let (pushes_seen, last_push_ms_ago, unsubscribed) = match stat {
                    Some(stats) => (
                        Some(stats.pushes),
                        stats
                            .last_push
                            .map(|at| now.duration_since(at).as_millis() as u64),
                        Some(stats.unsubscribed),
                    ),
                    None => (None, None, None),
                };
                let dead = record.is_some_and(|r| r.dead);
                let first_seen_age_ms = record
                    .and_then(|r| r.first_seen)
                    .map(|at| now.duration_since(at).as_millis() as u64);
                let state =
                    subscription_state(dead, pushes_seen, last_push_ms_ago, first_seen_age_ms);
                entries.push(crate::extensions::mcp::McpSubscriptionEntry {
                    server: server.clone(),
                    uri,
                    label: record.and_then(|r| r.label.clone()),
                    pushes_seen,
                    last_push_ms_ago,
                    unsubscribed,
                    state: Some(state),
                });
            }
        }
        // Dead rows: registry entries no longer covered by a live client —
        // the client for that server disconnected or was removed. Kept
        // visible (marked dead) instead of dropped. Registry URIs still
        // tracked by a live client but missing from its subscribed set were
        // unsubscribed server-side and stay dropped, matching the old
        // client-driven listing.
        for ((server, uri), record) in registry.iter() {
            if live_uris.contains(&(server.clone(), uri.clone())) {
                continue;
            }
            if ordered_clients.iter().any(|(name, _)| name == server) {
                // Client alive but this URI left its subscribed set (or was
                // already listed above) — not a dead-client case.
                continue;
            }
            let stat = stats.get(&(server.clone(), uri.clone()));
            if stat.is_some_and(|s| s.unsubscribed) {
                continue;
            }
            let (pushes_seen, last_push_ms_ago, unsubscribed) = match stat {
                Some(stats) => (
                    Some(stats.pushes),
                    stats
                        .last_push
                        .map(|at| now.duration_since(at).as_millis() as u64),
                    Some(stats.unsubscribed),
                ),
                None => (None, None, None),
            };
            entries.push(crate::extensions::mcp::McpSubscriptionEntry {
                server: server.clone(),
                uri: uri.clone(),
                label: record.label.clone(),
                pushes_seen,
                last_push_ms_ago,
                unsubscribed,
                state: Some(crate::extensions::mcp::McpSubscriptionState::Dead),
            });
        }
        entries
    }

    /// User-driven unsubscribe from the TUI. Tombstones the uri (no refresh
    /// re-subscribe) and marks the pump stats so a buffered burst is dropped.
    pub(crate) async fn unsubscribe_mcp_resource(
        &self,
        server_name: &str,
        uri: &str,
    ) -> Result<bool, String> {
        let client = {
            let state = self.mcp_state.lock().await;
            state
                .get_client(server_name)
                .cloned()
                .ok_or_else(|| format!("server '{server_name}' not found"))?
        };
        {
            let mut stats = self.mcp_push_stats.lock();
            let entry = stats
                .entry((server_name.to_string(), uri.to_string()))
                .or_default();
            entry.unsubscribed = true;
        }
        let acknowledged = client
            .unsubscribe_resource(uri)
            .await
            .map_err(|e| e.to_string())?;
        tracing::info!(
            server = %server_name,
            uri = %uri,
            acknowledged,
            "user unsubscribed from MCP resource via TUI"
        );
        Ok(acknowledged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pending() -> PendingUpdate {
        PendingUpdate {
            text: String::new(),
            truncated: false,
            first_at: std::time::Instant::now(),
            last_at: std::time::Instant::now(),
        }
    }

    #[test]
    fn append_accumulates_and_marks_truncation_at_cap() {
        let mut update = pending();
        update.append("hello ");
        update.append("world");
        assert_eq!(update.text, "hello world");
        assert!(!update.truncated);

        // Oversized append: fills the remaining room, marks truncated, and
        // ignores further appends.
        let filler = "x".repeat(MAX_PENDING_BYTES);
        update.append(&filler);
        assert!(update.truncated);
        assert!(update.text.len() <= MAX_PENDING_BYTES + 40);
        assert!(update.text.contains("(stream truncated at limit)"));
        let before = update.text.clone();
        update.append("more");
        assert_eq!(update.text, before, "no growth after truncation");
    }

    #[test]
    fn ready_requires_quiet_window_or_absolute_cap() {
        let mut update = pending();
        // Fresh burst: not ready (quiet window has not elapsed).
        assert!(!update.ready(std::time::Instant::now()));
        // Simulate an old burst: last push older than the quiet window.
        update.last_at = std::time::Instant::now() - QUIET_WINDOW;
        assert!(update.ready(std::time::Instant::now()));
        // Continuously-printing stream: quiet window never elapses, but the
        // absolute window cap forces a flush.
        let mut streaming = PendingUpdate {
            text: String::new(),
            truncated: false,
            first_at: std::time::Instant::now() - MAX_WINDOW,
            last_at: std::time::Instant::now(),
        };
        assert!(streaming.ready(std::time::Instant::now()));
        let _ = &mut streaming;
    }

    #[test]
    fn floor_char_boundary_never_splits_utf8() {
        let s = "é".repeat(4); // 8 bytes; 'é' occupies bytes 0..2, 2..4, …
        assert_eq!(floor_char_boundary(&s, 1), 0, "mid-char cut falls back");
        assert_eq!(floor_char_boundary(&s, 2), 2, "on-boundary cut is kept");
        assert_eq!(floor_char_boundary(&s, 5), 4);
        assert_eq!(floor_char_boundary(&s, 99), 8, "clamps to len");
        assert_eq!(floor_char_boundary("abc", 2), 2, "ascii cuts stay");
    }
}
