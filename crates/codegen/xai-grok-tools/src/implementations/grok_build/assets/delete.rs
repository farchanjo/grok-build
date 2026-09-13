//! `asset_delete` — remove an object from the session asset store.
//!
//! Idempotent by contract: deleting a key that is not there is a successful
//! `not_found` outcome, not an error, so a retry never has to branch.

use xai_file_utils::assets::AssetOperation;

use super::{parse_key, project_asset_error, require_capability, require_store};
use crate::types::output::ToolOutput;
use crate::types::requirements::{Expr, ToolRequirement};
use crate::types::tool::{ToolKind, ToolNamespace};

/// Canonical tool name.
pub const ASSET_DELETE_TOOL_NAME: &str = "asset_delete";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct AssetDeleteInput {
    #[schemars(description = "Key of the object to delete, e.g. `reports/q3.pdf`.")]
    pub key: String,
}

/// Structured result of one delete.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AssetDeleteOutput {
    pub key: String,
    /// True only when an object was actually removed.
    pub deleted: bool,
    /// `deleted` or `not_found`; deleting a missing key is not an error.
    pub outcome: String,
    pub backend: String,
    /// Pre-formatted model-facing prose.
    pub text: String,
}

#[derive(Debug, Default)]
pub struct AssetDeleteTool;

impl crate::types::tool_metadata::ToolMetadata for AssetDeleteTool {
    fn kind(&self) -> ToolKind {
        ToolKind::AssetDelete
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    fn description_template(&self) -> &str {
        "Delete a stored asset. Idempotent: deleting a key that does not exist succeeds \
         with `not_found` instead of erroring."
    }

    fn requires_expr(&self) -> Expr<ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for AssetDeleteTool {
    type Args = AssetDeleteInput;
    type Output = ToolOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new(ASSET_DELETE_TOOL_NAME).expect("valid tool id")
    }

    fn description(
        &self,
        _ctx: &xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            ASSET_DELETE_TOOL_NAME,
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

    #[tracing::instrument(name = "tool.asset_delete", skip_all, fields(key = %input.key))]
    async fn run(
        &self,
        ctx: xai_tool_runtime::ToolCallContext,
        input: AssetDeleteInput,
    ) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
        use crate::types::tool_metadata::shared_resources;

        let key = parse_key(&input.key)?;
        let resources = shared_resources(&ctx)?;
        let store = require_store(&resources).await?;
        require_capability(store.as_ref(), AssetOperation::Delete)?;

        let outcome = store.delete(&key).await.map_err(project_asset_error)?;
        let backend = store.backend();
        let deleted = outcome.is_deleted();

        let text = if deleted {
            format!("Deleted `{key}` from the {backend} asset store.")
        } else {
            format!("`{key}` was not present in the {backend} asset store; nothing to delete.")
        };

        Ok(ToolOutput::AssetDelete(AssetDeleteOutput {
            key: key.to_string(),
            deleted,
            outcome: if deleted { "deleted" } else { "not_found" }.to_owned(),
            backend: backend.to_string(),
            text,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::implementations::grok_build::assets::test_support::resources_with_store;
    use crate::types::tool_metadata::test_ctx_with_call_id;
    use std::path::Path;
    use std::sync::Arc;
    use xai_file_utils::assets::{
        AssetError, AssetKey, AssetStore, BackendKind, ContentType, MockAssetStore, PutRequest,
        SharedAssetStore,
    };

    async fn run(
        tool: &AssetDeleteTool,
        store: SharedAssetStore,
        key: &str,
    ) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
        xai_tool_runtime::Tool::run(
            tool,
            test_ctx_with_call_id(resources_with_store(store, Path::new("/tmp")), "test-call"),
            AssetDeleteInput {
                key: key.to_owned(),
            },
        )
        .await
    }

    async fn seeded() -> Arc<MockAssetStore> {
        let store = Arc::new(MockAssetStore::new());
        store
            .put(PutRequest::from_bytes(
                AssetKey::parse("uploads/a.txt").unwrap(),
                b"body".to_vec(),
                ContentType::default(),
            ))
            .await
            .unwrap();
        store
    }

    #[test]
    fn tool_name_and_classification() {
        let tool = AssetDeleteTool;
        assert_eq!(
            xai_tool_runtime::Tool::id(&tool).as_str(),
            ASSET_DELETE_TOOL_NAME
        );
        assert!(!crate::types::tool_metadata::ToolMetadata::is_read_only(
            &tool
        ));
    }

    #[tokio::test]
    async fn happy_path_deletes_the_object() {
        let store = seeded().await;
        let out = run(&AssetDeleteTool, store.clone(), "uploads/a.txt")
            .await
            .unwrap();

        match out {
            ToolOutput::AssetDelete(out) => {
                assert_eq!(out.key, "uploads/a.txt");
                assert!(out.deleted);
                assert_eq!(out.outcome, "deleted");
                assert!(out.text.contains("Deleted `uploads/a.txt`"));
            }
            other => panic!("expected AssetDelete, got {other:?}"),
        }
        assert_eq!(store.object_count(), 0);
    }

    #[tokio::test]
    async fn missing_key_is_an_idempotent_success() {
        let store = Arc::new(MockAssetStore::new());
        let out = run(&AssetDeleteTool, store, "uploads/none").await.unwrap();

        match out {
            ToolOutput::AssetDelete(out) => {
                assert!(!out.deleted);
                assert_eq!(out.outcome, "not_found");
                assert!(out.text.contains("nothing to delete"));
            }
            other => panic!("expected AssetDelete, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn capability_fast_fails_on_a_backend_without_delete() {
        let store = Arc::new(
            MockAssetStore::builder()
                .backend(BackendKind::Proxy)
                .build(),
        );
        let err = run(&AssetDeleteTool, store.clone(), "uploads/a.txt")
            .await
            .unwrap_err();

        assert_eq!(err.kind, xai_tool_runtime::ToolErrorKind::NotImplemented);
        let details = err.details.unwrap();
        assert_eq!(details["code"], "asset_unsupported");
        assert_eq!(details["operation"], "delete");
        assert!(!store.was_called(AssetOperation::Delete));
    }

    #[tokio::test]
    async fn invalid_key_is_rejected_before_any_call() {
        let store = seeded().await;
        let err = run(&AssetDeleteTool, store.clone(), "_meta/a")
            .await
            .unwrap_err();
        assert_eq!(err.kind, xai_tool_runtime::ToolErrorKind::InvalidArguments);
        assert!(!store.was_called(AssetOperation::Delete));
    }

    #[tokio::test]
    async fn transient_failure_is_flagged_retryable() {
        let store = Arc::new(
            MockAssetStore::builder()
                .fail_with(
                    AssetOperation::Delete,
                    AssetError::Transient {
                        backend: BackendKind::S3,
                        detail: "503".into(),
                    },
                )
                .build(),
        );
        let err = run(&AssetDeleteTool, store, "uploads/a.txt")
            .await
            .unwrap_err();
        assert_eq!(
            err.kind,
            xai_tool_runtime::ToolErrorKind::ServiceUnavailable
        );
        assert_eq!(err.details.unwrap()["retryable"], true);
    }
}
