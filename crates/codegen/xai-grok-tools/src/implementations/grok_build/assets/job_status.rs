//! `asset_job_status` — inspect one transfer job.
//!
//! Read-only. Returns the job's current snapshot, or waits up to `wait_secs`
//! for it to finish first, so a caller can poll without a sleep loop.

use super::{
    TransferJobView, parse_job_id, parse_wait_secs, require_jobs, state_note, unknown_job,
    wait_for_job,
};
use crate::types::output::ToolOutput;
use crate::types::requirements::{Expr, ToolRequirement};
use crate::types::tool::{ToolKind, ToolNamespace};

/// Canonical tool name.
pub const ASSET_JOB_STATUS_TOOL_NAME: &str = "asset_job_status";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct AssetJobStatusInput {
    #[schemars(description = "Job id returned by `asset_upload` or `asset_download`.")]
    pub job_id: String,

    #[serde(default)]
    #[schemars(
        description = "Wait up to this many seconds for the job to reach a terminal state (0..=600) before reporting. Omit to report the current state immediately."
    )]
    pub wait_secs: Option<u64>,
}

/// Structured result for one job.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AssetJobStatusOutput {
    pub job_id: String,
    pub state: String,
    pub kind: String,
    pub key: String,
    pub backend: String,
    pub bytes_transferred: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes_total: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percent: Option<u8>,
    pub elapsed_ms: u64,
    pub bytes_per_sec: u64,
    pub terminal: bool,
    pub waited: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Pre-formatted model-facing prose.
    pub text: String,
}

#[derive(Debug, Default)]
pub struct AssetJobStatusTool;

impl crate::types::tool_metadata::ToolMetadata for AssetJobStatusTool {
    fn kind(&self) -> ToolKind {
        ToolKind::AssetJobStatus
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    fn description_template(&self) -> &str {
        "Report the state, progress, and throughput of one transfer job. \
         Read-only. Pass `wait_secs` to block until the job finishes instead of polling."
    }

    fn requires_expr(&self) -> Expr<ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for AssetJobStatusTool {
    type Args = AssetJobStatusInput;
    type Output = ToolOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new(ASSET_JOB_STATUS_TOOL_NAME).expect("valid tool id")
    }

    fn description(
        &self,
        _ctx: &xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            ASSET_JOB_STATUS_TOOL_NAME,
            crate::types::tool_metadata::ToolMetadata::description_template(self),
        )
    }

    fn capabilities(&self) -> xai_tool_protocol::ToolCapabilities {
        xai_tool_protocol::ToolCapabilities {
            is_read_only: true,
            tool_scope: Some(xai_tool_protocol::ToolScope::Read),
            ..Default::default()
        }
    }

    #[tracing::instrument(name = "tool.asset_job_status", skip_all, fields(job = %input.job_id))]
    async fn run(
        &self,
        ctx: xai_tool_runtime::ToolCallContext,
        input: AssetJobStatusInput,
    ) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
        use crate::types::tool_metadata::shared_resources;

        let resources = shared_resources(&ctx)?;
        let registry = require_jobs(&resources).await?;
        let job_id = parse_job_id(&input.job_id)?;
        let wait = parse_wait_secs(input.wait_secs)?;

        let snapshot = match wait {
            Some(timeout) => wait_for_job(&registry, &job_id, Some(timeout)).await,
            None => registry.get(&job_id).await,
        }
        .ok_or_else(|| unknown_job(&job_id))?;

        let view = TransferJobView::from_snapshot(&snapshot);
        let mut text = format!("Job {}.", view.line());
        if !view.terminal {
            text.push_str(&format!(
                " It {}.",
                state_note(snapshot.state, wait.is_some())
            ));
        }
        if let Some(error) = &view.error {
            text.push_str(&format!(" Error: {error}"));
        }

        Ok(ToolOutput::AssetJobStatus(AssetJobStatusOutput {
            job_id: view.job_id,
            state: view.state,
            kind: view.kind,
            key: view.key,
            backend: view.backend,
            bytes_transferred: view.bytes_transferred,
            bytes_total: view.bytes_total,
            percent: view.percent,
            elapsed_ms: view.elapsed_ms,
            bytes_per_sec: view.bytes_per_sec,
            terminal: view.terminal,
            waited: wait.is_some(),
            error: view.error,
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
        input: AssetJobStatusInput,
    ) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
        xai_tool_runtime::Tool::run(
            &AssetJobStatusTool,
            test_ctx_with_call_id(resources_with_store(store, cwd), "test-call"),
            input,
        )
        .await
    }

    #[test]
    fn tool_name_is_read_only() {
        let tool = AssetJobStatusTool;
        assert_eq!(
            xai_tool_runtime::Tool::id(&tool).as_str(),
            ASSET_JOB_STATUS_TOOL_NAME
        );
        assert!(crate::types::tool_metadata::ToolMetadata::is_read_only(
            &tool
        ));
    }

    #[tokio::test]
    async fn unknown_job_is_a_not_found_error() {
        let dir = tempfile::TempDir::new().unwrap();
        let mock = Arc::new(MockAssetStore::new());
        let err = run(
            mock,
            dir.path(),
            AssetJobStatusInput {
                job_id: "nope".into(),
                wait_secs: None,
            },
        )
        .await
        .unwrap_err();
        assert_eq!(err.kind, xai_tool_runtime::ToolErrorKind::NotFound);
        assert_eq!(err.details.unwrap()["code"], "asset_job_not_found");
    }

    #[tokio::test]
    async fn reports_a_completed_job_with_waited_flag() {
        let dir = tempfile::TempDir::new().unwrap();
        let workspace = tempfile::TempDir::new().unwrap();
        let source = workspace.path().join("a.txt");
        tokio::fs::write(&source, b"hello").await.unwrap();

        let store: SharedAssetStore = Arc::new(MockAssetStore::new());
        let resources = resources_with_store(store.clone(), workspace.path());
        let registry = Arc::new(AssetJobRegistry::new());
        {
            let mut res = resources.lock().await;
            res.insert(Arc::clone(&registry));
        }
        let job_id = registry.spawn_upload(
            store,
            PutRequest::from_bytes(
                AssetKey::parse("uploads/a.txt").unwrap(),
                b"hello".to_vec(),
                ContentType::default(),
            ),
        );

        let out = xai_tool_runtime::Tool::run(
            &AssetJobStatusTool,
            test_ctx_with_call_id(resources, "test-call"),
            AssetJobStatusInput {
                job_id: job_id.clone(),
                wait_secs: Some(10),
            },
        )
        .await
        .unwrap();

        match out {
            ToolOutput::AssetJobStatus(out) => {
                assert_eq!(out.state, "completed");
                assert!(out.terminal);
                assert!(out.waited);
                assert_eq!(out.bytes_transferred, 5);
                assert!(out.text.contains("completed"));
            }
            other => panic!("expected AssetJobStatus, got {other:?}"),
        }
        let _ = dir;
    }
}