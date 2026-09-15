//! `asset_job_cancel` — stop a running transfer job.
//!
//! Cancellation is cooperative: the token is raised, the transfer task drops
//! the store future, and the job lands in `cancelled`. `wait_secs` observes
//! that transition inline, which is what makes the outcome deterministic for
//! the caller.

use xai_file_utils::assets::{CancelOutcome, JobState};

use super::{
    TransferJobView, parse_job_id, parse_wait_secs, require_jobs, unknown_job, wait_for_job,
};
use crate::types::output::ToolOutput;
use crate::types::requirements::{Expr, ToolRequirement};
use crate::types::tool::{ToolKind, ToolNamespace};

/// Canonical tool name.
pub const ASSET_JOB_CANCEL_TOOL_NAME: &str = "asset_job_cancel";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct AssetJobCancelInput {
    #[schemars(description = "Job id returned by `asset_upload` or `asset_download`.")]
    pub job_id: String,

    #[serde(default)]
    #[schemars(
        description = "Wait up to this many seconds for the cancellation to take effect (0..=600) so the reported state is final. Omit to report as soon as the token is raised."
    )]
    pub wait_secs: Option<u64>,
}

/// Structured result of one cancellation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AssetJobCancelOutput {
    pub job_id: String,
    /// `cancelled`, `already_finished`, or `not_found`.
    pub outcome: String,
    /// State observed after the cancel (and after `wait_secs`, when given).
    pub state: String,
    pub terminal: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes_transferred: Option<u64>,
    /// Pre-formatted model-facing prose.
    pub text: String,
}

#[derive(Debug, Default)]
pub struct AssetJobCancelTool;

impl crate::types::tool_metadata::ToolMetadata for AssetJobCancelTool {
    fn kind(&self) -> ToolKind {
        ToolKind::AssetJobCancel
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    fn description_template(&self) -> &str {
        "Cancel a running asset transfer job. Cooperative: pass `wait_secs` so the reported \
         state is the final one. Cancelling a finished job is not an error."
    }

    fn requires_expr(&self) -> Expr<ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for AssetJobCancelTool {
    type Args = AssetJobCancelInput;
    type Output = ToolOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new(ASSET_JOB_CANCEL_TOOL_NAME).expect("valid tool id")
    }

    fn description(
        &self,
        _ctx: &xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            ASSET_JOB_CANCEL_TOOL_NAME,
            crate::types::tool_metadata::ToolMetadata::description_template(self),
        )
    }

    fn capabilities(&self) -> xai_tool_protocol::ToolCapabilities {
        xai_tool_protocol::ToolCapabilities {
            is_read_only: false,
            tool_scope: Some(xai_tool_protocol::ToolScope::Write),
            ..Default::default()
        }
    }

    #[tracing::instrument(name = "tool.asset_job_cancel", skip_all, fields(job = %input.job_id))]
    async fn run(
        &self,
        ctx: xai_tool_runtime::ToolCallContext,
        input: AssetJobCancelInput,
    ) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
        use crate::types::tool_metadata::shared_resources;

        let resources = shared_resources(&ctx)?;
        let registry = require_jobs(&resources).await?;
        let job_id = parse_job_id(&input.job_id)?;
        let wait = parse_wait_secs(input.wait_secs)?;

        let outcome = registry.cancel(&job_id).await;
        if matches!(outcome, CancelOutcome::NotFound { .. }) {
            return Err(unknown_job(&job_id));
        }

        // Observe the transition so the reported state is final, not "cancel
        // requested but still running".
        let snapshot = match wait {
            Some(timeout) => wait_for_job(&registry, &job_id, Some(timeout)).await,
            None if outcome.is_cancelled() => {
                // A bounded look so a queued job is not reported as running
                // forever; the task observes the token on its first poll.
                wait_for_job(
                    &registry,
                    &job_id,
                    Some(std::time::Duration::from_millis(250)),
                )
                .await
            }
            None => registry.get(&job_id).await,
        };

        let (state, terminal, bytes_transferred, view) = match snapshot {
            Some(snapshot) => {
                let view = TransferJobView::from_snapshot(&snapshot);
                (
                    snapshot.state,
                    snapshot.is_terminal(),
                    Some(snapshot.bytes_transferred),
                    Some(view),
                )
            }
            None => (JobState::Cancelled, true, None, None),
        };

        let mut text = match (&outcome, state) {
            (CancelOutcome::Cancelled { .. }, JobState::Cancelled) => {
                format!("Cancelled job `{job_id}`.")
            }
            (CancelOutcome::Cancelled { .. }, _) => format!(
                "Cancellation requested for job `{job_id}`; it is still {} — check again with `asset_job_status`.",
                state
            ),
            (CancelOutcome::AlreadyFinished { state, .. }, _) => {
                format!("Job `{job_id}` had already finished as `{state}`; nothing to cancel.")
            }
            (CancelOutcome::NotFound { .. }, _) => {
                format!("No transfer job `{job_id}` in this session.")
            }
        };
        if let Some(bytes) = bytes_transferred
            && bytes > 0
            && state == JobState::Cancelled
        {
            text.push_str(&format!(
                " {} had already moved before the cancel.",
                super::format_bytes(bytes)
            ));
        }
        if let Some(view) = &view
            && let Some(error) = &view.error
        {
            text.push_str(&format!(" Error: {error}"));
        }

        Ok(ToolOutput::AssetJobCancel(AssetJobCancelOutput {
            job_id,
            outcome: outcome.as_str().to_owned(),
            state: state.as_str().to_owned(),
            terminal,
            bytes_transferred,
            text,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::implementations::grok_build::assets::test_support::resources_with_store;
    use crate::types::tool_metadata::test_ctx_with_call_id;
    use std::sync::Arc;
    use xai_file_utils::assets::{
        AssetJobRegistry, AssetKey, ContentType, MockAssetStore, PutRequest, SharedAssetStore,
    };

    async fn run(
        store: SharedAssetStore,
        cwd: &std::path::Path,
        input: AssetJobCancelInput,
    ) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
        xai_tool_runtime::Tool::run(
            &AssetJobCancelTool,
            test_ctx_with_call_id(resources_with_store(store, cwd), "test-call"),
            input,
        )
        .await
    }

    fn request(name: &str) -> PutRequest {
        PutRequest::from_bytes(
            AssetKey::parse(&format!("uploads/{name}")).unwrap(),
            b"x".to_vec(),
            ContentType::default(),
        )
    }

    #[test]
    fn tool_name_is_not_read_only() {
        let tool = AssetJobCancelTool;
        assert_eq!(
            xai_tool_runtime::Tool::id(&tool).as_str(),
            ASSET_JOB_CANCEL_TOOL_NAME
        );
        assert!(!crate::types::tool_metadata::ToolMetadata::is_read_only(
            &tool
        ));
    }

    #[tokio::test]
    async fn cancelling_a_finished_job_is_not_an_error() {
        let dir = tempfile::TempDir::new().unwrap();
        let store: SharedAssetStore = Arc::new(MockAssetStore::new());
        let resources = resources_with_store(store.clone(), dir.path());
        let registry = Arc::new(AssetJobRegistry::new());
        {
            let mut res = resources.lock().await;
            res.insert(Arc::clone(&registry));
        }
        let job_id = registry.spawn_upload(store, request("a.txt"));
        registry
            .wait_for_completion(&job_id, Some(std::time::Duration::from_secs(5)))
            .await
            .unwrap();

        let out = xai_tool_runtime::Tool::run(
            &AssetJobCancelTool,
            test_ctx_with_call_id(resources, "test-call"),
            AssetJobCancelInput {
                job_id: job_id.clone(),
                wait_secs: None,
            },
        )
        .await
        .unwrap();

        match out {
            ToolOutput::AssetJobCancel(out) => {
                assert_eq!(out.outcome, "already_finished");
                assert_eq!(out.state, "completed");
                assert!(out.terminal);
                assert!(out.text.contains("had already finished"));
            }
            other => panic!("expected AssetJobCancel, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn unknown_job_is_a_not_found_error() {
        let dir = tempfile::TempDir::new().unwrap();
        let mock = Arc::new(MockAssetStore::new());
        let err = run(
            mock,
            dir.path(),
            AssetJobCancelInput {
                job_id: "nope".into(),
                wait_secs: None,
            },
        )
        .await
        .unwrap_err();
        assert_eq!(err.kind, xai_tool_runtime::ToolErrorKind::NotFound);
    }

    #[tokio::test]
    async fn blank_job_id_is_an_argument_error() {
        let dir = tempfile::TempDir::new().unwrap();
        let mock = Arc::new(MockAssetStore::new());
        let err = run(
            mock,
            dir.path(),
            AssetJobCancelInput {
                job_id: "  ".into(),
                wait_secs: None,
            },
        )
        .await
        .unwrap_err();
        assert_eq!(err.kind, xai_tool_runtime::ToolErrorKind::InvalidArguments);
    }
}
