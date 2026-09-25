//! `x.ai/plan/update` — client-driven todo status changes.
//!
//! The todo pane is a *view* of the session plan, so closing an item there must
//! land in the same `State<TodoState>` the model's `todo_write` mutates.
//! Mutating the pane's local copy alone would diverge: the model would still see
//! the item as pending, and its next `todo_write` would resurrect it.
//!
//! The request is addressed by plan *position* because the ACP `Plan` payload
//! carries no identifier (content, priority, status and meta only). The plan is
//! emitted in state order and the pane renders in that order, so a position is
//! stable between two emissions.
//!
//! The actor performs the change and re-emits the Plan; this handler only
//! forwards and reports how many positions matched.

use agent_client_protocol as acp;
use serde::Deserialize;

use super::super::mvp_agent::MvpAgent;
use crate::session::commands::{SessionCommand, TodoStatusChange};
use crate::session::result::ExtMethodResult;

/// `x.ai/plan/update` params. `sessionId` is optional for the same reason as
/// `x.ai/session/info`: a single-session client may omit it.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlanUpdateRequest {
    session_id: Option<String>,
    changes: Vec<TodoStatusChange>,
}

/// Router entry for `x.ai/plan/update`.
pub async fn handle(
    agent: &MvpAgent,
    args: &acp::ExtRequest,
) -> Result<acp::ExtResponse, acp::Error> {
    let req: PlanUpdateRequest = serde_json::from_str(args.params.get())
        .map_err(|e| acp::Error::invalid_params().data(format!("invalid params: {e}")))?;

    let session_id = req.session_id.or_else(|| {
        agent
            .sessions
            .borrow()
            .keys()
            .next()
            .map(|id| id.0.to_string())
    });
    let Some(session_id) = session_id else {
        return ExtMethodResult::<serde_json::Value>::failure("no resident session")
            .to_ext_response()
            .map_err(to_acp_error);
    };

    let sid = acp::SessionId::new(session_id);
    let Some(session) = agent.sessions.borrow().get(&sid).cloned() else {
        return ExtMethodResult::<serde_json::Value>::failure(format!(
            "unknown session `{}`",
            sid.0
        ))
        .to_ext_response()
        .map_err(to_acp_error);
    };

    let (tx, rx) = tokio::sync::oneshot::channel();
    if session
        .cmd_tx
        .send(SessionCommand::SetTodoStatuses {
            changes: req.changes,
            responds_to: tx,
        })
        .is_err()
    {
        return ExtMethodResult::<serde_json::Value>::failure("session actor is gone")
            .to_ext_response()
            .map_err(to_acp_error);
    }
    let Ok(applied) = rx.await else {
        return ExtMethodResult::<serde_json::Value>::failure("session actor dropped the request")
            .to_ext_response()
            .map_err(to_acp_error);
    };

    ExtMethodResult::<serde_json::Value>::success(serde_json::json!({ "applied": applied }))
        .to_ext_response()
        .map_err(to_acp_error)
}

fn to_acp_error(error: anyhow::Error) -> acp::Error {
    acp::Error::internal_error().data(error.to_string())
}
