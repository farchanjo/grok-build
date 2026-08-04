//! `x.ai/media/*` extension methods.
//!
//! `x.ai/media/test_route` runs a **real** consented sample-media route test
//! against one configured route. The shell performs the permission,
//! disclosure-consent, ZDR, credential, transport, and budget gates through
//! the session's media-understanding backend before any bytes leave; the
//! client only supplies the workspace-relative path. A one-token no-media
//! request is never presented as a modality test.

use agent_client_protocol as acp;
use serde::Deserialize;

use super::{ExtResult, parse_params, to_raw_response};
use crate::agent::MvpAgent;

/// Inbound params for `x.ai/media/test_route`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestMediaRouteParams {
    /// Session whose media-understanding backend runs the test.
    session_id: String,
    /// Category whose route list holds `route_index`.
    category: xai_grok_tools::media::domain::MediaCategory,
    /// Index into the category's configured route list (`0` = primary).
    route_index: usize,
    /// Workspace-relative media path the user selected.
    path: String,
}

/// Dispatch for the `x.ai/media/*` namespace.
pub async fn handle(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    match args.method.as_ref() {
        "x.ai/media/test_route" => handle_test_route(agent, args).await,
        _ => Err(acp::Error::method_not_found()),
    }
}

/// Run a consented sample-media route test through the session's backend.
async fn handle_test_route(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    let params: TestMediaRouteParams = parse_params(args)?;

    // Content-only contract: workspace-relative paths only. The shell's
    // policy layer re-verifies containment after permission approval; this
    // rejects obviously invalid forms before any session work.
    if params.path.trim().is_empty() {
        return Err(acp::Error::invalid_params().data("media route test path must not be empty"));
    }
    if params.path.contains("://") || params.path.starts_with('/') {
        return Err(
            acp::Error::invalid_params().data("media route test path must be workspace-relative")
        );
    }

    let session_id = acp::SessionId::new(params.session_id.as_str());
    let Some(handle) = agent.session_handle_waiting_for_load(&session_id).await else {
        return Err(acp::Error::resource_not_found(Some(format!(
            "session not found: {}",
            params.session_id
        ))));
    };

    let result = handle
        .test_media_route(params.category, params.route_index, params.path)
        .await;
    match result {
        Ok(summary) => to_raw_response(&serde_json::json!({
            "ok": true,
            "summary": summary,
        })),
        Err(error) => to_raw_response(&serde_json::json!({
            "error": error,
        })),
    }
}
