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
                Ok(Some(McpClientEvent::ResourceUpdated { server, uri })) => {
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
                Ok(Some(_)) => {
                    // Other client events belong to the status dispatcher.
                }
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

impl SessionActor {
    /// Snapshot of this session's active MCP resource subscriptions with
    /// pump stats, for the pager's "Subscribed Tools" sheet. Servers whose
    /// clients are missing (disconnected mid-list) contribute no rows.
    pub(crate) async fn list_mcp_subscriptions(
        &self,
    ) -> Vec<crate::extensions::mcp::McpSubscriptionEntry> {
        let clients: Vec<(
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
        let mut entries = Vec::new();
        for (server, client) in clients {
            for uri in client.subscribed_uris() {
                let stats = self
                    .mcp_push_stats
                    .lock()
                    .get(&(server.clone(), uri.clone()))
                    .cloned();
                let (pushes_seen, last_push_ms_ago, unsubscribed) = match stats {
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
                    uri,
                    pushes_seen,
                    last_push_ms_ago,
                    unsubscribed,
                });
            }
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
