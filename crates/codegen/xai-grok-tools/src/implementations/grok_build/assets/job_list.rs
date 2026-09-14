//! `asset_job_list` — list the session's transfer jobs.
//!
//! Read-only. Oldest first, so a caller can find a job it started earlier
//! without keeping the id. `only_active` narrows the list to jobs that have not
//! reached a terminal state.

use super::{TransferJobView, format_bytes, require_jobs};
use crate::types::output::ToolOutput;
use crate::types::requirements::{Expr, ToolRequirement};
use crate::types::tool::{ToolKind, ToolNamespace};

/// Canonical tool name.
pub const ASSET_JOB_LIST_TOOL_NAME: &str = "asset_job_list";

/// Cap on jobs returned in one call.
pub const MAX_LISTED_JOBS: usize = 50;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct AssetJobListInput {
    #[serde(default)]
    #[schemars(
        description = "Only report jobs that have not finished (queued or running). Default: false."
    )]
    pub only_active: bool,
}

/// Structured result for the listing.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AssetJobListOutput {
    pub jobs: Vec<TransferJobView>,
    pub total: usize,
    pub active: usize,
    /// True when the listing was cut at [`MAX_LISTED_JOBS`].
    pub truncated: bool,
    /// Pre-formatted model-facing prose.
    pub text: String,
}

#[derive(Debug, Default)]
pub struct AssetJobListTool;

impl crate::types::tool_metadata::ToolMetadata for AssetJobListTool {
    fn kind(&self) -> ToolKind {
        ToolKind::AssetJobList
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    fn description_template(&self) -> &str {
        "List this session's asset transfer jobs, oldest first, with state and progress. \
         Read-only. Use `only_active` to see just the running ones."
    }

    fn requires_expr(&self) -> Expr<ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for AssetJobListTool {
    type Args = AssetJobListInput;
    type Output = ToolOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new(ASSET_JOB_LIST_TOOL_NAME).expect("valid tool id")
    }

    fn description(
        &self,
        _ctx: &xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            ASSET_JOB_LIST_TOOL_NAME,
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

    #[tracing::instrument(name = "tool.asset_job_list", skip_all, fields(only_active = input.only_active))]
    async fn run(
        &self,
        ctx: xai_tool_runtime::ToolCallContext,
        input: AssetJobListInput,
    ) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
        use crate::types::tool_metadata::shared_resources;

        let resources = shared_resources(&ctx)?;
        let registry = require_jobs(&resources).await?;

        let snapshots = registry.list().await;
        let active = snapshots
            .iter()
            .filter(|snapshot| !snapshot.is_terminal())
            .count();
        let total = snapshots.len();

        let mut views: Vec<TransferJobView> = snapshots
            .iter()
            .filter(|snapshot| !input.only_active || !snapshot.is_terminal())
            .map(TransferJobView::from_snapshot)
            .collect();
        let truncated = views.len() > MAX_LISTED_JOBS;
        views.truncate(MAX_LISTED_JOBS);

        let text = if views.is_empty() {
            if input.only_active {
                "No transfer jobs are running.".to_owned()
            } else {
                "No transfer jobs have been started in this session.".to_owned()
            }
        } else {
            let mut text = format!(
                "{} transfer job{} ({active} active, {} total):",
                views.len(),
                if views.len() == 1 { "" } else { "s" },
                total
            );
            for view in &views {
                text.push_str("\n- ");
                text.push_str(&view.line());
            }
            if truncated {
                let moved: u64 = views.iter().map(|view| view.bytes_transferred).sum();
                text.push_str(&format!(
                    "\n(truncated at {MAX_LISTED_JOBS} jobs; {} moved across the listed jobs)",
                    format_bytes(moved)
                ));
            }
            text
        };

        Ok(ToolOutput::AssetJobList(AssetJobListOutput {
            jobs: views,
            total,
            active,
            truncated,
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
    use xai_file_utils::assets::{AssetKey, ContentType, MockAssetStore, PutRequest, SharedAssetStore};

    async fn run(
        store: SharedAssetStore,
        cwd: &std::path::Path,
        input: AssetJobListInput,
    ) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
        xai_tool_runtime::Tool::run(
            &AssetJobListTool,
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
    fn tool_name_is_read_only() {
        let tool = AssetJobListTool;
        assert_eq!(
            xai_tool_runtime::Tool::id(&tool).as_str(),
            ASSET_JOB_LIST_TOOL_NAME
        );
        assert!(crate::types::tool_metadata::ToolMetadata::is_read_only(
            &tool
        ));
    }

    #[tokio::test]
    async fn empty_session_reports_no_jobs() {
        let dir = tempfile::TempDir::new().unwrap();
        let mock = Arc::new(MockAssetStore::new());
        let out = run(
            mock,
            dir.path(),
            AssetJobListInput { only_active: false },
        )
        .await
        .unwrap();
        match out {
            ToolOutput::AssetJobList(out) => {
                assert!(out.jobs.is_empty());
                assert_eq!(out.total, 0);
                assert!(out.text.contains("No transfer jobs"));
            }
            other => panic!("expected AssetJobList, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn lists_jobs_oldest_first_and_filters_active() {
        let dir = tempfile::TempDir::new().unwrap();
        let store: SharedAssetStore = Arc::new(MockAssetStore::new());
        let resources = resources_with_store(store.clone(), dir.path());
        let registry = Arc::new(xai_file_utils::assets::AssetJobRegistry::new());
        {
            let mut res = resources.lock().await;
            res.insert(Arc::clone(&registry));
        }
        let first = registry.spawn_upload(store.clone(), request("a.txt"));
        let second = registry.spawn_upload(store, request("b.txt"));
        registry
            .wait_for_completion(&first, Some(std::time::Duration::from_secs(5)))
            .await
            .unwrap();

        let out = xai_tool_runtime::Tool::run(
            &AssetJobListTool,
            test_ctx_with_call_id(resources.clone(), "test-call"),
            AssetJobListInput { only_active: false },
        )
        .await
        .unwrap();
        match out {
            ToolOutput::AssetJobList(out) => {
                assert_eq!(out.total, 2);
                assert_eq!(out.jobs[0].job_id, first, "oldest first");
                assert_eq!(out.jobs[1].job_id, second);
                assert!(out.active <= 2);
                assert!(out.text.contains("uploads/a.txt"));
            }
            other => panic!("expected AssetJobList, got {other:?}"),
        }

        registry
            .wait_for_completion(&second, Some(std::time::Duration::from_secs(5)))
            .await
            .unwrap();
        let active_only = xai_tool_runtime::Tool::run(
            &AssetJobListTool,
            test_ctx_with_call_id(resources, "test-call"),
            AssetJobListInput { only_active: true },
        )
        .await
        .unwrap();
        match active_only {
            ToolOutput::AssetJobList(out) => {
                assert!(out.jobs.is_empty());
                assert_eq!(out.total, 2);
            }
            other => panic!("expected AssetJobList, got {other:?}"),
        }
    }
}