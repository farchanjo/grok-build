//! Session-administration extension handlers.
//!
//! Methods grouped here are operational/admin endpoints that mutate
//! persistent or shared agent state but are not part of the per-turn prompt
//! lifecycle:
//!
//! - `x.ai/session/rename`                  rename a session locally + remote
//! - `x.ai/session/delete`                  delete a session locally + remote
//! - `x.ai/session/update_mcp_servers`      mid-session MCP server swap
//! - `x.ai/session/fork`                    fork a session into a new one
//! - `x.ai/internal/reload_all_mcp_servers` config hot-reload, all sessions
//! - `x.ai/internal/reload_project_mcp_servers` config hot-reload, cwd-scoped
//! - `x.ai/internal/reload_skills`          skills file watcher fan-out
//! - `x.ai/internal/reload_models`          model list hot-reload from config.toml
//! - `x.ai/internal/reload_models_cache`    model catalog hot-reload from disk cache
//! - `x.ai/internal/reload_compaction`      compaction policy fan-out
//! - `x.ai/internal/reload_language`        conversation-language fan-out
//! - `x.ai/internal/auth_cleared`           auth hot-clear cleanup
//! - `x.ai/plugins/reload`                  rebuild shared plugin registry
//! - `x.ai/commands/list`                   list slash commands

use std::path::Path;
use std::sync::Arc;

use agent_client_protocol as acp;
use agent_client_protocol::Client as _;
use serde::Deserialize;

use super::{ExtResult, parse_params, to_raw_response};
use crate::agent::MvpAgent;
use crate::session::persistence::list_summaries;
use crate::session::storage::StorageAdapter;
use crate::session::storage::jsonl::JsonlStorageAdapter;
use crate::session::unified_list::SessionKind;
use crate::session::{ExtMethodResult, SessionCommand};
use xai_grok_telemetry::id::agent_id;

#[tracing::instrument(skip_all, fields(method = %args.method))]
pub async fn handle(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    match args.method.as_ref() {
        "x.ai/session/rename" => handle_session_rename(agent, args).await,
        "x.ai/session/delete" => handle_session_delete(agent, args).await,
        "x.ai/session/update_mcp_servers" => handle_update_mcp_servers(agent, args).await,
        "x.ai/session/fork" => handle_session_fork(agent, args).await,
        "x.ai/internal/reload_all_mcp_servers" => handle_reload_all_mcp_servers(agent).await,
        "x.ai/internal/reload_project_mcp_servers" => {
            handle_reload_project_mcp_servers(agent, args).await
        }
        "x.ai/internal/reload_skills" => handle_reload_skills(agent),
        "x.ai/internal/reload_workflows" => handle_reload_workflows(agent),
        "x.ai/internal/reload_models" => handle_reload_models(agent, args),
        "x.ai/internal/reload_models_cache" => handle_reload_models_cache(agent),
        "x.ai/internal/reload_compaction" => handle_reload_compaction(agent, args),
        "x.ai/internal/reload_language" => handle_reload_language(agent, args),
        "x.ai/internal/auth_cleared" => handle_auth_cleared(agent),
        "x.ai/plugins/reload" => handle_plugins_reload(agent).await,
        "x.ai/commands/list" => handle_commands_list(agent, args).await,
        _ => Err(acp::Error::method_not_found()),
    }
}

// session/rename

/// Handles renaming a session.
async fn handle_session_rename(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct RenameRequest {
        session_id: String,
        title: String,
        #[serde(default)]
        cwd: Option<String>,
        #[serde(default)]
        kind: SessionKind,
    }

    let mut req: RenameRequest = parse_params(args)?;
    // Manual titles must be non-blank: `Summary.title_is_manual` binds to a
    // real `generated_title`, so reject whitespace-only input at the boundary.
    req.title = req.title.trim().to_string();
    if req.title.is_empty() {
        return Err(acp::Error::invalid_request().data("title must not be blank"));
    }

    if req.kind == SessionKind::Chat {
        return rename_chat_conversation(agent, &req.session_id, &req.title).await;
    }

    let session_id = acp::SessionId::new(Arc::from(req.session_id.as_str()));

    // Find the session info, scoping to cwd if provided
    let summaries = list_summaries(req.cwd.as_deref())
        .await
        .map_err(|e| acp::Error::internal_error().data(format!("failed to list sessions: {e}")))?;

    let summary = summaries
        .iter()
        .find(|s| s.info.id == session_id)
        .ok_or_else(|| {
            acp::Error::invalid_request().data(format!("session not found: {}", req.session_id))
        })?;

    let info = summary.info.clone();

    // Update the session title in local storage
    let storage = JsonlStorageAdapter::default();
    storage
        .update_session_title(&info, req.title.clone())
        .await
        .map_err(|e| {
            acp::Error::internal_error().data(format!("failed to update session title: {e}"))
        })?;

    // Update session search index with new title
    crate::session::storage::search::notify_session_updated(&info.id.to_string(), &info.cwd);

    // Send a SessionSummaryGenerated notification so the TUI updates its title
    notify_session_title(agent, session_id, &req.title).await;

    if agent.is_writeback_storage()
        && let Some(auth) = agent.current_auth()
        && !auth.is_zdr_team()
    {
        use crate::remote::client::BackendClient;
        use crate::session::export::ExportedMetadata;

        let mut metadata = ExportedMetadata::from_summary(summary);
        metadata.title = Some(req.title.clone());
        metadata.updated_at = Some(chrono::Utc::now().to_rfc3339());
        if let Err(e) = BackendClient::new()
            .with_auth_manager(agent.auth_manager.clone())
            .save_session_data(&req.session_id, &[], Some(&metadata))
            .await
        {
            tracing::warn!(?e, session_id = %req.session_id, "failed to sync renamed title to backend");
        }
    }

    // Hook 2: update session replica with summary (fire-and-forget)
    if let Some(client) = agent.session_registry_client() {
        let sid = req.session_id.to_string();
        let title = if agent
            .auth_manager
            .current_or_expired()
            .is_some_and(|a| a.is_zdr_team())
        {
            None
        } else {
            Some(req.title.clone())
        };
        tokio::spawn(async move {
            let update = crate::agent::session_registry_client::UpdateRequest {
                summary: title,
                first_prompt: None,
                last_turn_number: None,
                repo_head_at_end: None,
                restorable_turn_number: None,
            };
            if let Err(e) = client.update(&sid, &update).await {
                tracing::warn!(error = %e, "session registry summary update failed (non-fatal)");
            }
        });
    }

    tracing::info!(session_id = %req.session_id, title = %req.title, "Session renamed");

    to_raw_response(&serde_json::json!({ "success": true }))
}

/// Notify connected clients of a session's new title via
/// `SessionSummaryGenerated`.
async fn notify_session_title(agent: &MvpAgent, session_id: acp::SessionId, title: &str) {
    use crate::extensions::notification::{SessionNotification, SessionUpdate};

    let notification = SessionNotification {
        session_id,
        update: SessionUpdate::SessionSummaryGenerated {
            session_summary: title.to_owned(),
        },
        meta: None,
    };
    if let Ok(params) = serde_json::value::to_raw_value(&notification) {
        let ext_notification =
            acp::ExtNotification::new("x.ai/session_notification", params.into());
        let _ = agent.gateway.ext_notification(ext_notification).await;
    }
}

async fn rename_chat_conversation(
    agent: &MvpAgent,
    conversation_id: &str,
    title: &str,
) -> ExtResult {
    use crate::remote::{ConvError, UpdateConversationBody};

    let Some(client) = agent.conversations_client() else {
        return Err(acp::Error::invalid_request()
            .data("chat session rename requires the conversations lane (OIDC + chat feature)"));
    };

    let body = UpdateConversationBody {
        title: Some(title.to_owned()),
        starred: None,
    };
    client
        .update_conversation(conversation_id, &body)
        .await
        .map_err(|e| match e {
            ConvError::NoOauth => acp::Error::invalid_request()
                .data("chat session rename requires xAI OAuth credentials"),
            ConvError::Http { status: 404 } => acp::Error::invalid_request()
                .data(format!("conversation not found: {conversation_id}")),
            other => acp::Error::internal_error()
                .data(format!("chat conversation rename failed: {other}")),
        })?;

    // If this conversation is open live, notify clients of the new title.
    let session_id = acp::SessionId::new(Arc::from(conversation_id));
    if agent.sessions.borrow().contains_key(&session_id) {
        notify_session_title(agent, session_id, title).await;
    }

    tracing::info!(
        session_id = %conversation_id,
        title = %title,
        "Chat conversation renamed"
    );

    to_raw_response(&serde_json::json!({ "success": true }))
}

// session/delete

/// Delete a session from history.
async fn handle_session_delete(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct DeleteRequest {
        session_id: String,
        #[serde(default)]
        cwd: Option<String>,
        #[serde(default)]
        kind: SessionKind,
    }

    let req: DeleteRequest = parse_params(args)?;

    if req.kind == SessionKind::Chat {
        return soft_delete_chat_conversation(agent, &req.session_id).await;
    }

    let session_id = acp::SessionId::new(Arc::from(req.session_id.as_str()));

    // For writeback storage (non-ZDR): remote delete is authoritative for
    // the cloud history and runs first; on failure no local bits are
    // touched so the pager does not remove the row or toast success.
    let needs_remote =
        agent.is_writeback_storage() && agent.current_auth().is_some_and(|a| !a.is_zdr_team());

    // Shared delete: remote-first, then local disk + FTS eviction.
    // Mirrored by the `grok sessions delete <id>` CLI path.
    crate::session::persistence::delete_session_history(
        &req.session_id,
        req.cwd.as_deref(),
        needs_remote,
        agent.auth_manager.clone(),
    )
    .await
    .map_err(|e| {
        if let crate::session::persistence::DeleteSessionError::Remote(_) = &e {
            tracing::warn!(?e, session_id = %req.session_id, "failed to delete remote session data");
        }
        acp::Error::internal_error().data(e.to_string())
    })?;

    // If an in-memory live session exists for this id (e.g. the user
    // deleted history for a session that is still open in another agent
    // or the current one), shut it down and drop the MvpAgent bookkeeping
    // so we don't leave a live actor whose on-disk/FTS state is gone.
    if agent.sessions.borrow().contains_key(&session_id) {
        agent.request_session_shutdown(&session_id);
        agent.remove_session(&session_id);
    }

    tracing::info!(session_id = %req.session_id, "Session deleted");

    to_raw_response(&serde_json::json!({ "success": true }))
}

async fn soft_delete_chat_conversation(agent: &MvpAgent, conversation_id: &str) -> ExtResult {
    use crate::remote::ConvError;

    let Some(client) = agent.conversations_client() else {
        return Err(acp::Error::invalid_request()
            .data("chat session delete requires the conversations lane (OIDC + chat feature)"));
    };

    client
        .soft_delete_conversation(conversation_id)
        .await
        .map_err(|e| match e {
            ConvError::NoOauth => acp::Error::invalid_request()
                .data("chat session delete requires xAI OAuth credentials"),
            other => acp::Error::internal_error()
                .data(format!("chat conversation soft-delete failed: {other}")),
        })?;

    let session_id = acp::SessionId::new(Arc::from(conversation_id));
    if agent.sessions.borrow().contains_key(&session_id) {
        agent.request_session_shutdown(&session_id);
        agent.remove_session(&session_id);
    }

    tracing::info!(session_id = %conversation_id, "Chat conversation soft-deleted");

    to_raw_response(&serde_json::json!({ "success": true }))
}

// session/update_mcp_servers

async fn handle_update_mcp_servers(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Params {
        session_id: acp::SessionId,
        mcp_servers: Vec<acp::McpServer>,
    }

    let params: Params = parse_params(args)?;

    let (handle, cwd) = {
        let sessions = agent.sessions.borrow();
        let h = sessions
            .get(&params.session_id)
            .cloned()
            .ok_or_else(|| acp::Error::invalid_params().data("unknown session id"))?;
        let cwd = std::path::PathBuf::from(&h.info.cwd);
        (h, cwd)
    };

    let managed = agent.get_managed_mcp_configs().await;
    let merged = crate::session::managed_mcp::merge_managed_mcp_servers(
        params.mcp_servers.clone(),
        &cwd,
        &managed,
        agent.plugin_registry_handle().snapshot().as_deref(),
        &agent.cfg.borrow().compat_resolved,
    );

    let (tx, rx) = tokio::sync::oneshot::channel();
    handle
        .cmd_tx
        .send(SessionCommand::UpdateMcpServers {
            mcp_servers: merged,
            respond_to: tx,
        })
        .map_err(|_| acp::Error::internal_error().data("session closed"))?;

    // Wait for the session actor to finish MCP re-initialization.
    rx.await
        .map_err(|_| acp::Error::internal_error().data("session closed"))?
        .map_err(|e| acp::Error::internal_error().data(e.to_string()))?;

    // Persist the new client set on the handle so config hot-reloads
    // (`reload_all_mcp_servers` / `reload_project_mcp_servers`) re-merge from
    // the client's latest intent rather than the `session/new` snapshot —
    // otherwise a reload would resurrect servers the client just removed
    // (or drop ones it just added).
    if let Some(h) = agent.sessions.borrow_mut().get_mut(&params.session_id) {
        h.initial_client_mcp_servers = params.mcp_servers;
    }

    ExtMethodResult::success(serde_json::json!({ "ok": true }))
        .to_ext_response()
        .map_err(|e| acp::Error::internal_error().data(e.to_string()))
}

// internal/reload_skills

/// Reload skills for ALL active sessions. Called by the skills file watcher
fn handle_reload_skills(agent: &MvpAgent) -> ExtResult {
    let reloaded = agent.reload_skills_all_sessions();
    ExtMethodResult::success(serde_json::json!({ "reloaded": reloaded }))
        .to_ext_response()
        .map_err(|e| acp::Error::internal_error().data(e.to_string()))
}

fn handle_reload_workflows(agent: &MvpAgent) -> ExtResult {
    let reloaded = agent.advertise_commands_all_sessions();
    ExtMethodResult::success(serde_json::json!({ "reloaded": reloaded }))
        .to_ext_response()
        .map_err(|e| acp::Error::internal_error().data(e.to_string()))
}

// internal/reload_all_mcp_servers

/// Reload MCP servers for ALL active sessions. Called by the config
/// hot-reload watcher when `[mcp_servers]` changes in config.toml.
async fn handle_reload_all_mcp_servers(agent: &MvpAgent) -> ExtResult {
    let session_ids: Vec<acp::SessionId> = agent.sessions.borrow().keys().cloned().collect();

    if session_ids.is_empty() {
        return ExtMethodResult::success(serde_json::json!({ "updated": 0 }))
            .to_ext_response()
            .map_err(|e| acp::Error::internal_error().data(e.to_string()));
    }

    let managed = agent.get_managed_mcp_configs().await;
    let mut updated = 0u32;
    for session_id in &session_ids {
        let Some(handle) = agent.sessions.borrow().get(session_id).cloned() else {
            continue;
        };
        let cwd = std::path::PathBuf::from(&handle.info.cwd);
        let compat = agent.cfg.borrow().compat_resolved;
        // Re-seed the merge with the session's original client-provided MCP
        // servers (e.g. a managed connector injected at `session/new` by a
        // client session binding). `merge_managed_mcp_servers` already
        // re-reads every disk source (config.toml, plugins, ~/.claude.json,
        // ~/.cursor/mcp.json, .mcp.json) internally, so passing
        // `load_mcp_servers()` output here was redundant — and silently
        // dropped client servers that exist in no on-disk config, tearing
        // them down on every config hot-reload.
        let merged = crate::session::managed_mcp::merge_managed_mcp_servers(
            handle.initial_client_mcp_servers.clone(),
            &cwd,
            &managed,
            agent.plugin_registry_handle().snapshot().as_deref(),
            &compat,
        );

        let (tx, _rx) = tokio::sync::oneshot::channel();
        if handle
            .cmd_tx
            .send(SessionCommand::UpdateMcpServers {
                mcp_servers: merged,
                respond_to: tx,
            })
            .is_ok()
        {
            updated += 1;
        }
    }

    tracing::info!(
        updated,
        total = session_ids.len(),
        "reloaded MCP servers for active sessions"
    );
    ExtMethodResult::success(serde_json::json!({ "updated": updated }))
        .to_ext_response()
        .map_err(|e| acp::Error::internal_error().data(e.to_string()))
}

// internal/reload_project_mcp_servers

/// Reload MCP servers for sessions whose `cwd` matches (or sits beneath)
/// the project root passed in `params.cwd`. Called by the config
/// hot-reload watcher when `<cwd>/.grok/config.toml`,
/// `<cwd>/.mcp.json`, or `<cwd>/.claude.json` changes.
///
/// Sessions in unrelated cwds are intentionally NOT touched — that is
/// the whole point of [`crate::config::reloader::ConfigUpdate::
/// ProjectMcpServersChanged`] being a per-cwd variant. The legacy
/// [`handle_reload_all_mcp_servers`] is still the fan-out for global
/// `~/.grok/config.toml` edits.
async fn handle_reload_project_mcp_servers(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    #[derive(Deserialize)]
    struct Params {
        cwd: String,
    }

    let params: Params = parse_params(args)?;
    let target_cwd = std::path::PathBuf::from(&params.cwd);

    // Collect (session_id, cwd) pairs once so we don't hold the
    // `sessions` RefCell borrow across `.await` points.
    let session_ids: Vec<(acp::SessionId, std::path::PathBuf)> = agent
        .sessions
        .borrow()
        .iter()
        .map(|(sid, h)| (sid.clone(), std::path::PathBuf::from(&h.info.cwd)))
        .filter(|(_, cwd)| cwd_matches(cwd, &target_cwd))
        .collect();

    if session_ids.is_empty() {
        return ExtMethodResult::success(serde_json::json!({ "updated": 0 }))
            .to_ext_response()
            .map_err(|e| acp::Error::internal_error().data(e.to_string()));
    }

    let managed = agent.get_managed_mcp_configs().await;
    let mut updated = 0u32;
    for (session_id, cwd) in &session_ids {
        let Some(handle) = agent.sessions.borrow().get(session_id).cloned() else {
            continue;
        };
        // See `handle_reload_all_mcp_servers`: seed with the session's
        // client-provided servers, not `load_mcp_servers()` — the merge
        // re-reads all disk sources itself, and client-provided servers
        // (session bindings) must survive config hot-reloads.
        let merged = crate::session::managed_mcp::merge_managed_mcp_servers(
            handle.initial_client_mcp_servers.clone(),
            cwd,
            &managed,
            agent.plugin_registry_handle().snapshot().as_deref(),
            &agent.cfg.borrow().compat_resolved,
        );

        let (tx, _rx) = tokio::sync::oneshot::channel();
        if handle
            .cmd_tx
            .send(SessionCommand::UpdateMcpServers {
                mcp_servers: merged,
                respond_to: tx,
            })
            .is_ok()
        {
            updated += 1;
        }
    }

    tracing::info!(
        updated,
        total = session_ids.len(),
        cwd = %target_cwd.display(),
        "reloaded project MCP servers for matching sessions"
    );
    ExtMethodResult::success(serde_json::json!({ "updated": updated }))
        .to_ext_response()
        .map_err(|e| acp::Error::internal_error().data(e.to_string()))
}

/// Returns `true` iff `session_cwd` equals `target_cwd` or sits
/// beneath it (so a `<repo>/` edit reloads `<repo>/subdir/` sessions
/// too).
///
/// This uses `Path::starts_with`, which is
/// **component-aware** — `/repo-test` does NOT match `/repo` even
/// though the byte prefix matches. That is the desired behavior
/// (component-aware avoids the `/foo-bar` ⊂ `/foo` foot-gun). Paths
/// come from `SessionInfo::cwd` (always absolute) and the watcher's
/// emitted path (also absolute), so no canonicalization is needed
/// here. The `==` short-circuit is redundant (`Path::starts_with` is
/// reflexive) but kept for an explicit zero-allocation fast path.
fn cwd_matches(session_cwd: &std::path::Path, target_cwd: &std::path::Path) -> bool {
    session_cwd == target_cwd || session_cwd.starts_with(target_cwd)
}

// internal/reload_models

/// Reload lanes. A bare request retains the legacy model/catalog behavior.
#[derive(Deserialize)]
#[serde(from = "ReloadModelLaneFlagsWire")]
struct ReloadModelLaneFlags {
    models: bool,
    providers: bool,
    retrieval: bool,
    cache: bool,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ReloadModelLaneFlagsWire {
    models: Option<bool>,
    providers: Option<bool>,
    retrieval: Option<bool>,
    cache: Option<bool>,
}

impl From<ReloadModelLaneFlagsWire> for ReloadModelLaneFlags {
    fn from(wire: ReloadModelLaneFlagsWire) -> Self {
        let bare = wire.models.is_none()
            && wire.providers.is_none()
            && wire.retrieval.is_none()
            && wire.cache.is_none();
        Self {
            // Explicit partial requests must not widen omitted lanes.
            models: wire.models.unwrap_or(bare),
            providers: wire.providers.unwrap_or(false),
            retrieval: wire.retrieval.unwrap_or(false),
            cache: wire.cache.unwrap_or(false),
        }
    }
}

const MAX_RELOAD_RETRIEVAL_WARNINGS: usize = 8;
const MAX_RELOAD_RETRIEVAL_WARNING_CHARS: usize = 160;

fn safe_retrieval_load_warning(error: &str) -> &'static str {
    if error.starts_with("config.toml parse error:") {
        "config_parse_error"
    } else if error.starts_with("read config.toml:") {
        "config_read_error"
    } else {
        "retrieval_unavailable"
    }
}

fn safe_retrieval_validation_warnings(reasons: Vec<String>) -> Vec<String> {
    let mut warnings: Vec<String> = reasons
        .into_iter()
        .take(MAX_RELOAD_RETRIEVAL_WARNINGS)
        .map(|reason| {
            let mut chars = reason.chars();
            let mut bounded: String = chars
                .by_ref()
                .take(MAX_RELOAD_RETRIEVAL_WARNING_CHARS)
                .map(|ch| if ch.is_control() { ' ' } else { ch })
                .collect();
            if chars.next().is_some() {
                bounded.push('\u{2026}');
            }
            if bounded.trim().is_empty() {
                "invalid".to_owned()
            } else {
                bounded
            }
        })
        .collect();
    if warnings.is_empty() {
        warnings.push("invalid".to_owned());
    }
    warnings
}

/// Re-resolve the agent model list from config.toml. Called by the config
/// hot-reload watcher when `[model.*]` or `[models]` changes.
///
/// Re-reads config from disk, re-runs the same resolution logic as
/// `new_with_models()` for user TOML config entries, and swaps the model list
/// in-place. Prefetched (API) and default models are NOT re-fetched -- only
/// BYOK entries from config are updated.
///
/// The ACP-serialized owner for atomic provider/catalog/retrieval publication.
fn handle_reload_models(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    let flags: ReloadModelLaneFlags = parse_params(args)?;

    // Cache-only events must not advance provider or graph generations.
    if flags.cache && !flags.models && !flags.providers && !flags.retrieval {
        agent.models_manager.reload_from_disk_cache();
        agent.sync_process_static_api_key(None);
        let count = agent.models_manager.models().len();
        return ExtMethodResult::success(serde_json::json!({
            "reloaded": true,
            "models": count,
        }))
        .to_ext_response()
        .map_err(|e| acp::Error::internal_error().data(e.to_string()));
    }

    let home = crate::util::grok_home::grok_home();
    // Validate retrieval before catalog mutation; model/media-only batches
    // remain independent of unrelated graph validity.
    let retrieval_candidate = if flags.retrieval {
        let input = match crate::retrieval::load_build_input_from_home(&home) {
            Ok(input) => input,
            Err(error) => {
                let warning = safe_retrieval_load_warning(&error);
                tracing::warn!(
                    warning,
                    "reload batch rejected; retrieval candidate unavailable"
                );
                if flags.cache {
                    agent.models_manager.reload_from_disk_cache();
                }
                agent.sync_process_static_api_key(None);
                return ExtMethodResult::success(serde_json::json!({
                    "models": agent.models_manager.models().len(),
                    "retainedLastKnownGood": true,
                    "retrievalWarnings": [warning],
                }))
                .to_ext_response()
                .map_err(|e| acp::Error::internal_error().data(e.to_string()));
            }
        };
        match crate::retrieval::build_snapshot(input.clone(), 0) {
            Ok(_) => crate::retrieval::registry_for_home(&home).map(|registry| (registry, input)),
            Err(error) => {
                let warnings = safe_retrieval_validation_warnings(error.reasons);
                tracing::warn!(
                    ?warnings,
                    "reload batch rejected; retrieval candidate invalid"
                );
                if flags.cache {
                    agent.models_manager.reload_from_disk_cache();
                }
                agent.sync_process_static_api_key(None);
                return ExtMethodResult::success(serde_json::json!({
                    "models": agent.models_manager.models().len(),
                    "retainedLastKnownGood": true,
                    "retrievalWarnings": warnings,
                }))
                .to_ext_response()
                .map_err(|e| acp::Error::internal_error().data(e.to_string()));
            }
        }
    } else {
        None
    };

    let mut updated_media_sessions = 0;
    if flags.models {
        let prior_agent_config = agent.cfg.borrow().clone();
        let disk_config = crate::config::load_effective_config()
            .map_err(|e| acp::Error::internal_error().data(e.to_string()))?;

        let toml_config = crate::agent::config::Config::new_from_toml_cfg(&disk_config)
            .map_err(|e| acp::Error::internal_error().data(e))?;

        // Merge TOML-derived model fields into the agent's in-memory config so
        // runtime-only fields (#[serde(skip)]: remote_settings, endpoints, CLI
        // flags) are preserved. Only model-related TOML fields are refreshed.
        {
            let agent_config = agent.cfg.borrow();
            let overrides = crate::config::ModelOverrideConfig::resolve(
                agent_config.web_search_model_override.as_deref(),
                agent_config.session_summary_model_override.as_deref(),
                &disk_config,
                agent_config.remote_settings.as_ref(),
            );
            drop(agent_config);
            let mut agent_config = agent.cfg.borrow_mut();
            agent_config.models = toml_config.models.clone();
            agent_config.config_models = toml_config.config_models.clone();
            agent_config.config_warnings = toml_config.config_warnings.clone();
            if flags.providers {
                agent_config.model_providers = toml_config.model_providers.clone();
            }
            agent_config.endpoints = toml_config.endpoints.clone();
            agent_config.voice = toml_config.voice.clone();
            agent_config.web_search_model = overrides.web_search;
            agent_config.session_summary_model = overrides.session_summary;
            agent_config.media_config = crate::config::MediaConfig::resolve(
                &disk_config,
                agent_config.remote_settings.as_ref(),
            );
            agent_config.image_description_model = agent_config.media_config.image_model.clone();
            agent_config.prompt_suggest_model_pin = overrides.prompt_suggestion;
        }
        let media_config = agent.cfg.borrow().media_config.clone();
        let sessions = agent.sessions.borrow();
        updated_media_sessions = sessions
            .values()
            .filter(|session| {
                session
                    .cmd_tx
                    .send(SessionCommand::UpdateMediaConfig {
                        media: Box::new(media_config.clone()),
                    })
                    .is_ok()
            })
            .count();
        drop(sessions);
        // Recompute the campaign overlay + `pre_campaign_default` (the
        // catalog-miss fallback) so reload matches spawn; `new_from_toml_cfg`
        // reset it to None.
        {
            let mut agent_config = agent.cfg.borrow_mut();
            crate::util::config::sync_campaign_fields(&mut agent_config);
        }
        let merged_config = agent.cfg.borrow().clone();

        let outcome = agent
            .models_manager
            .apply_config_with_outcome_gated(merged_config, || {
                match retrieval_candidate.as_ref() {
                    Some((registry, input)) => matches!(
                        registry.reload_from_input(input.clone()),
                        crate::retrieval::ReloadOutcome::Published { .. }
                            | crate::retrieval::ReloadOutcome::Unchanged { .. }
                            | crate::retrieval::ReloadOutcome::Disabled { .. }
                    ),
                    None => true,
                }
            });

        let catalog_ok = !matches!(
            outcome,
            crate::agent::models::ModelReloadOutcome::RetainedLastKnownGood
        );
        if !catalog_ok {
            // Restore config so a rejected catalog cannot leak partial policy.
            *agent.cfg.borrow_mut() = prior_agent_config.clone();
            let prior_media = prior_agent_config.media_config.clone();
            let sessions = agent.sessions.borrow();
            updated_media_sessions = sessions
                .values()
                .filter(|session| {
                    session
                        .cmd_tx
                        .send(SessionCommand::UpdateMediaConfig {
                            media: Box::new(prior_media.clone()),
                        })
                        .is_ok()
                })
                .count();
            drop(sessions);
            tracing::warn!(
                "model config reload rejected; agent config restored and retrieval registry not rebuilt (LKG retained)"
            );
            if flags.cache {
                agent.models_manager.reload_from_disk_cache();
            }
            agent.sync_process_static_api_key(None);
            return ExtMethodResult::success(serde_json::json!({
                "models": agent.models_manager.models().len(),
                "updated_media_sessions": updated_media_sessions,
                "retainedLastKnownGood": true,
            }))
            .to_ext_response()
            .map_err(|e| acp::Error::internal_error().data(e.to_string()));
        }
    }

    // Model batches publish this candidate inside the catalog gate.
    if !flags.models
        && let Some((registry, input)) = retrieval_candidate
    {
        let outcome = registry.reload_from_input(input);
        tracing::info!(?home, ?outcome, "retrieval-only candidate published");
    }

    // Apply cache last so it cannot precede an accepted graph candidate.
    if flags.cache {
        agent.models_manager.reload_from_disk_cache();
    }
    agent.sync_process_static_api_key(None);

    let sessions = agent.sessions.borrow();
    let updated_context_windows = fan_out_catalog_context_windows(
        sessions
            .values()
            .map(|session| (&session.model_id, &session.cmd_tx)),
        &agent.models_manager,
    );
    drop(sessions);

    let count = agent.models_manager.models().len();
    tracing::info!(
        count,
        updated_media_sessions,
        updated_context_windows,
        "model list and media policy reloaded from config.toml"
    );
    ExtMethodResult::success(serde_json::json!({
        "models": count,
        "updated_media_sessions": updated_media_sessions,
        "updated_context_windows": updated_context_windows,
    }))
    .to_ext_response()
    .map_err(|e| acp::Error::internal_error().data(e.to_string()))
}

// internal/reload_models_cache

/// Hot-reload the model catalog from `~/.grok/models_cache.json` after an
/// external write detected by the config watcher.
///
/// Legacy cache-only entry point for external callers.
fn handle_reload_models_cache(agent: &MvpAgent) -> ExtResult {
    agent.models_manager.reload_from_disk_cache();
    agent.sync_process_static_api_key(None);
    ExtMethodResult::success(serde_json::json!({ "reloaded": true }))
        .to_ext_response()
        .map_err(|e| acp::Error::internal_error().data(e.to_string()))
}

fn fan_out_catalog_context_windows<'a>(
    sessions: impl Iterator<
        Item = (
            &'a acp::ModelId,
            &'a tokio::sync::mpsc::UnboundedSender<SessionCommand>,
        ),
    >,
    models_manager: &crate::agent::models::ModelsManager,
) -> usize {
    sessions
        .filter(|(model_id, sender)| {
            let Some(context_window) = models_manager.context_window_for(model_id.0.as_ref())
            else {
                return false;
            };
            sender
                .send(SessionCommand::RefreshCatalogContextWindow { context_window })
                .is_ok()
        })
        .count()
}

fn fan_out_compaction_config<'a>(
    command_senders: impl Iterator<Item = &'a tokio::sync::mpsc::UnboundedSender<SessionCommand>>,
    config: &crate::agent::config::CompactionConfig,
) -> usize {
    command_senders
        .filter(|sender| {
            sender
                .send(SessionCommand::UpdateCompactionConfig {
                    compaction: Box::new(config.clone()),
                })
                .is_ok()
        })
        .count()
}

fn handle_reload_language(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    #[derive(Deserialize)]
    struct LanguageReload {
        conversation: Option<String>,
    }
    let params: LanguageReload = parse_params(args)?;
    let conversation = params
        .conversation
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != "off")
        .map(str::to_owned);
    let sessions = agent.sessions.borrow();
    let total = sessions.len();
    let updated = sessions
        .values()
        .filter(|session| {
            session
                .cmd_tx
                .send(SessionCommand::SetConversationLanguage {
                    conversation_language: conversation.clone(),
                })
                .is_ok()
        })
        .count();
    tracing::info!(
        updated,
        total,
        "reloaded conversation language for active sessions"
    );
    ExtMethodResult::success(serde_json::json!({ "reloaded": true }))
        .to_ext_response()
        .map_err(|error| acp::Error::internal_error().data(error.to_string()))
}

fn handle_reload_compaction(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    let config: crate::agent::config::CompactionConfig = parse_params(args)?;
    config
        .normalize_validate()
        .map_err(|error| acp::Error::invalid_params().data(error.to_string()))?;
    agent.cfg.borrow_mut().compaction = config.clone();
    let sessions = agent.sessions.borrow();
    let total = sessions.len();
    let updated =
        fan_out_compaction_config(sessions.values().map(|session| &session.cmd_tx), &config);
    tracing::info!(
        updated,
        total,
        "reloaded compaction policy for active sessions"
    );
    ExtMethodResult::success(serde_json::json!({ "reloaded": true }))
        .to_ext_response()
        .map_err(|error| acp::Error::internal_error().data(error.to_string()))
}

fn handle_auth_cleared(agent: &MvpAgent) -> ExtResult {
    agent.disable_managed_gateway_tools_and_refresh_sessions();
    ExtMethodResult::success(serde_json::json!({ "ok": true }))
        .to_ext_response()
        .map_err(|e| acp::Error::internal_error().data(e.to_string()))
}

// plugins/reload

async fn handle_plugins_reload(agent: &MvpAgent) -> ExtResult {
    // Rebuild the shared registry so future/new sessions clone the latest.
    let session_cwd = agent
        .sessions
        .borrow()
        .values()
        .next()
        .map(|h| std::path::PathBuf::from(&h.info.cwd));
    let mut plugins = agent.cfg.borrow().plugins.clone();
    plugins.merge_claude_enabled_plugins(session_cwd.as_deref());
    let disk_cfg = plugins.to_discovery_config();
    // Folder-trust gates repo-local project plugins (hooks/MCP). Resolve and
    // record the verdict for this cwd (honoring the real remote), then gate
    // plugins on it.
    let project_trusted = session_cwd.as_deref().is_some_and(|c| {
        let remote_settings = agent.cfg.borrow().remote_settings.clone();
        crate::agent::folder_trust::resolve_and_record(c, remote_settings.as_ref(), false)
    });
    // Explicit desktop `x.ai/plugins/reload`: force a full local-install re-copy.
    agent
        .plugin_registry_handle()
        .reload(session_cwd.as_deref(), &disk_cfg, project_trusted, true);

    // Eagerly fan out the new registry to every live session: each adopts a
    // cwd-correct snapshot (hooks + MCP + skills + client slash-command
    // catalog), the same refresh the originating session of a reload gets.
    agent.broadcast_plugin_registry_to_sessions(None);

    super::to_ext_response(Ok(serde_json::json!({"ok": true})))
}

// commands/list

async fn handle_commands_list(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    let req: crate::session::slash_commands::ListCommandsRequest = parse_params(args)?;

    if let Some(session_id) = req.session_id.as_ref() {
        let Some(handle) = agent.session_handle_waiting_for_load(session_id).await else {
            return Err(
                acp::Error::invalid_request().data(format!("unknown session id: {}", session_id.0))
            );
        };
        let response = handle.list_available_commands().await;
        return Ok(acp::ExtResponse::new(Arc::from(
            serde_json::value::to_raw_value(&response)?,
        )));
    }

    let skills_config = agent.cfg.borrow().skills.clone();
    let compat = agent.cfg.borrow().compat_resolved;
    let availability = agent.command_availability();

    // For a given cwd, compute the plugin registry the same way a session would
    // at spawn time (via build_for_cwd) and the same way reload_plugins_impl does
    // (ancestor project config walk + vendor compat merge). This is required so
    // that `x.ai/commands/list` (the pull used by grok-desktop after session
    // start) returns plugin-provided slash commands for the target cwd.
    //
    // The shared snapshot is only populated at agent boot (using process CWD)
    // and by explicit reloads. In desktop<->docker (and ssh) setups the agent's
    // launch CWD is unrelated to the user's chosen workspace dir, so relying on
    // snapshot() alone meant the post-start pull returned no project plugin
    // skills until the user manually reloaded.
    let plugin_reg = if let Some(cwd_str) = &req.cwd {
        let cwd = Path::new(cwd_str);

        // Folder-trust gates repo-local project plugins (hooks/MCP). Resolve and
        // record the verdict for this cwd (honoring the real remote) BEFORE the
        // plugins-config read below: that read gates its project-paths merge on
        // the recorded verdict, and a cold cwd (client-supplied, no session
        // resolve yet) must not first take the gate's remote-less backstop —
        // that would record a kill-switch-blind deny no later resolve can lift.
        let remote_settings = agent.cfg.borrow().remote_settings.clone();
        let project_trusted =
            crate::agent::folder_trust::resolve_and_record(cwd, remote_settings.as_ref(), false);

        // Effective [plugins] config (global + ancestor project configs +
        // vendor compat merge), shared with reload_plugins_impl and the eager
        // fan-out so the menu agrees with each session's registry for this cwd.
        let disk_cfg = crate::config::resolve_effective_plugins_config(cwd).to_discovery_config();

        // Fresh discovery for *this* cwd (includes .grok/plugins under it, plus
        // the cli --plugin-dir dirs). Does not mutate the shared snapshot.
        agent
            .plugin_registry_handle()
            .build_for_cwd(cwd, &disk_cfg, &[], project_trusted)
    } else {
        // No cwd: global/user skills only (pre-session case). Use the boot snapshot.
        agent.plugin_registry_handle().snapshot()
    };

    let response = crate::session::slash_commands::list_commands(
        req.cwd.as_deref(),
        &skills_config,
        plugin_reg.as_deref(),
        availability,
        compat,
        false,
    )
    .await;
    Ok(acp::ExtResponse::new(Arc::from(
        serde_json::value::to_raw_value(&response)?,
    )))
}

// session/fork

async fn handle_session_fork(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    use crate::session::fork::{ForkSessionRequest, fork_session};

    let request: ForkSessionRequest = parse_params(args)?;

    let agent_id = agent_id();
    let response = fork_session(request, &agent_id, Some(agent.auth_manager.clone()))
        .await
        .map_err(|e| acp::Error::internal_error().data(e.to_string()))?;

    to_raw_response(&response)
}

#[cfg(test)]
mod tests {
    use super::ReloadModelLaneFlags;
    use super::{fan_out_catalog_context_windows, fan_out_compaction_config};
    use super::{safe_retrieval_load_warning, safe_retrieval_validation_warnings};
    use crate::agent::config::ModelEntry;
    use crate::agent::models::ModelsManager;
    use crate::session::SessionCommand;
    use agent_client_protocol as acp;
    use std::num::NonZeroU64;

    #[test]
    fn reload_lane_flags_parse_combined_intent() {
        // The app config-update task always sends explicit flags.
        let combined: ReloadModelLaneFlags = serde_json::from_value(serde_json::json!({
            "models": true,
            "providers": true,
            "retrieval": true,
            "cache": true,
        }))
        .unwrap();
        assert!(combined.models && combined.providers && combined.retrieval && combined.cache);

        let cache_only: ReloadModelLaneFlags = serde_json::from_value(serde_json::json!({
            "models": false,
            "retrieval": false,
            "cache": true,
        }))
        .unwrap();
        assert!(
            !cache_only.models
                && !cache_only.providers
                && !cache_only.retrieval
                && cache_only.cache
        );

        let models_only: ReloadModelLaneFlags = serde_json::from_value(serde_json::json!({
            "models": true,
            "retrieval": false,
            "cache": false,
        }))
        .unwrap();
        assert!(
            models_only.models
                && !models_only.providers
                && !models_only.retrieval
                && !models_only.cache
        );

        // A bare request preserves the historical model/catalog reload.
        let bare: ReloadModelLaneFlags = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(bare.models);
        assert!(!bare.retrieval && !bare.cache);

        let partial: ReloadModelLaneFlags =
            serde_json::from_value(serde_json::json!({ "cache": true })).unwrap();
        assert!(!partial.models && !partial.retrieval && partial.cache);
    }

    #[test]
    fn retrieval_reload_warnings_are_bounded_and_hide_raw_config_errors() {
        assert_eq!(
            safe_retrieval_load_warning(
                "config.toml parse error: secret = \"literal that must not escape\""
            ),
            "config_parse_error"
        );
        assert_eq!(
            safe_retrieval_load_warning("read config.toml: permission denied"),
            "config_read_error"
        );
        assert_eq!(
            safe_retrieval_load_warning("unexpected loader failure"),
            "retrieval_unavailable"
        );

        let warnings = safe_retrieval_validation_warnings(vec![
            "line one\nline two".to_owned(),
            "x".repeat(200),
        ]);
        assert_eq!(warnings[0], "line one line two");
        assert_eq!(warnings[1].chars().count(), 161);
        assert!(warnings[1].ends_with('\u{2026}'));
        assert_eq!(safe_retrieval_validation_warnings(Vec::new()), ["invalid"]);
    }

    #[test]
    fn compaction_reload_fans_out_to_every_live_session() {
        let (first_tx, mut first_rx) = tokio::sync::mpsc::unbounded_channel();
        let (second_tx, mut second_rx) = tokio::sync::mpsc::unbounded_channel();
        let config = crate::agent::config::CompactionConfig {
            models: vec![
                crate::agent::config::CompactionModelRef::new("@session".to_owned()).unwrap(),
                crate::agent::config::CompactionModelRef::new("custom-compactor".to_owned())
                    .unwrap(),
            ],
            strategy: Some(crate::agent::config::CompactionStrategy::Rolling),
            trigger_policy: Some(crate::agent::config::CompactionTriggerPolicy::Dynamic),
            rolling_band_count: Some(6),
            ..Default::default()
        };

        assert_eq!(
            fan_out_compaction_config([&first_tx, &second_tx].into_iter(), &config),
            2
        );
        for receiver in [&mut first_rx, &mut second_rx] {
            let SessionCommand::UpdateCompactionConfig { compaction } = receiver
                .try_recv()
                .expect("each session receives one update")
            else {
                panic!("expected compaction config update");
            };
            assert_eq!(*compaction, config);
            assert!(receiver.try_recv().is_err(), "must send exactly one update");
        }
    }

    #[test]
    fn compaction_reload_ignores_closed_session_mailboxes() {
        let (closed_tx, closed_rx) = tokio::sync::mpsc::unbounded_channel();
        drop(closed_rx);
        let (live_tx, mut live_rx) = tokio::sync::mpsc::unbounded_channel();
        let config = crate::agent::config::CompactionConfig::default();

        assert_eq!(
            fan_out_compaction_config([&closed_tx, &live_tx].into_iter(), &config),
            1
        );
        assert!(matches!(
            live_rx.try_recv(),
            Ok(SessionCommand::UpdateCompactionConfig { .. })
        ));
    }

    #[test]
    fn catalog_context_window_reload_fans_out_current_model_windows() {
        let tmp = tempfile::tempdir().unwrap();
        let auth_manager = std::sync::Arc::new(crate::auth::AuthManager::new(
            tmp.path(),
            crate::auth::GrokComConfig::default(),
        ));
        let mut catalog = indexmap::IndexMap::new();
        let mut sol = ModelEntry::fallback(
            "gpt-5.6-sol",
            &crate::agent::config::EndpointsConfig::default(),
        );
        sol.info.context_window = NonZeroU64::new(1_000_000).unwrap();
        catalog.insert("chatgpt-gpt-5.6-sol".to_owned(), sol);
        let mgr = ModelsManager::new(
            None,
            catalog,
            acp::ModelId::new("chatgpt-gpt-5.6-sol"),
            auth_manager,
            crate::agent::config::Config::default(),
        );

        let (matching_tx, mut matching_rx) = tokio::sync::mpsc::unbounded_channel();
        let (other_tx, mut other_rx) = tokio::sync::mpsc::unbounded_channel();
        let matching_id = acp::ModelId::new("chatgpt-gpt-5.6-sol");
        let other_id = acp::ModelId::new("grok-4");

        assert_eq!(
            fan_out_catalog_context_windows(
                [(&matching_id, &matching_tx), (&other_id, &other_tx)].into_iter(),
                &mgr,
            ),
            1
        );
        let Ok(SessionCommand::RefreshCatalogContextWindow { context_window }) =
            matching_rx.try_recv()
        else {
            panic!("expected context window refresh");
        };
        assert_eq!(context_window.get(), 1_000_000);
        assert!(
            other_rx.try_recv().is_err(),
            "sessions whose model is missing from the catalog must not be updated"
        );
    }
}
