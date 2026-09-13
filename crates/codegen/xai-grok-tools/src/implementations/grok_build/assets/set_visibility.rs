//! `asset_set_visibility` — flip a stored object between private and public.
//!
//! Not every backend can enforce visibility. GCS cannot change it at all (the
//! tool fast-fails with `Unsupported`), and every other backend *records* it —
//! so the output carries `enforced` and a hint instead of pretending the flag
//! is an access-control guarantee (contract §8).

use xai_file_utils::assets::AssetOperation;

use super::{
    parse_key, parse_visibility, project_asset_error, require_capability, require_store,
    visibility_hint,
};
use crate::types::output::ToolOutput;
use crate::types::requirements::{Expr, ToolRequirement};
use crate::types::tool::{ToolKind, ToolNamespace};

/// Canonical tool name.
pub const ASSET_SET_VISIBILITY_TOOL_NAME: &str = "asset_set_visibility";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct AssetSetVisibilityInput {
    #[schemars(description = "Key of the stored object, e.g. `reports/q3.pdf`.")]
    pub key: String,

    #[schemars(description = "`public` to publish, `private` to withdraw.")]
    pub visibility: String,
}

/// Structured result of one visibility change.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AssetSetVisibilityOutput {
    pub key: String,
    pub visibility: String,
    /// True only when the backend actually enforces the recorded visibility.
    pub enforced: bool,
    /// Stable URL when the store has a public base configured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_url: Option<String>,
    /// Present when the backend records without enforcing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// Pre-formatted model-facing prose.
    pub text: String,
}

#[derive(Debug, Default)]
pub struct AssetSetVisibilityTool;

impl crate::types::tool_metadata::ToolMetadata for AssetSetVisibilityTool {
    fn kind(&self) -> ToolKind {
        ToolKind::AssetSetVisibility
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    fn description_template(&self) -> &str {
        "Change a stored asset between `private` and `public`. Not every backend can \
         enforce visibility; the result says whether it is enforced."
    }

    fn requires_expr(&self) -> Expr<ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for AssetSetVisibilityTool {
    type Args = AssetSetVisibilityInput;
    type Output = ToolOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new(ASSET_SET_VISIBILITY_TOOL_NAME).expect("valid tool id")
    }

    fn description(
        &self,
        _ctx: &xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            ASSET_SET_VISIBILITY_TOOL_NAME,
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

    #[tracing::instrument(
        name = "tool.asset_set_visibility",
        skip_all,
        fields(key = %input.key, visibility = %input.visibility)
    )]
    async fn run(
        &self,
        ctx: xai_tool_runtime::ToolCallContext,
        input: AssetSetVisibilityInput,
    ) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
        use crate::types::tool_metadata::shared_resources;

        let key = parse_key(&input.key)?;
        let visibility = parse_visibility(Some(&input.visibility))?;

        let resources = shared_resources(&ctx)?;
        let store = require_store(&resources).await?;
        require_capability(store.as_ref(), AssetOperation::SetVisibility)?;

        let meta = store
            .set_visibility(&key, visibility)
            .await
            .map_err(project_asset_error)?;

        let public_url = store.public_url(&key);
        let hint = visibility_hint(meta.backend, meta.visibility_enforced);
        let mut text = format!(
            "`{}` is now {} on the {} backend.",
            meta.key,
            if meta.visibility.is_public() {
                "public"
            } else {
                "private"
            },
            meta.backend,
        );
        if meta.visibility.is_public() {
            match &public_url {
                Some(url) => text.push_str(&format!(" Public URL: {url}")),
                None => text.push_str(
                    " No public base URL is configured, so the object has no stable public URL yet.",
                ),
            }
        }
        if let Some(hint) = &hint {
            text.push(' ');
            text.push_str(hint);
        }

        Ok(ToolOutput::AssetSetVisibility(AssetSetVisibilityOutput {
            key: meta.key.to_string(),
            visibility: meta.visibility.to_string(),
            enforced: meta.visibility_enforced,
            public_url,
            hint,
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
        SharedAssetStore, Visibility,
    };

    async fn run(
        tool: &AssetSetVisibilityTool,
        store: SharedAssetStore,
        key: &str,
        visibility: &str,
    ) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
        xai_tool_runtime::Tool::run(
            tool,
            test_ctx_with_call_id(resources_with_store(store, Path::new("/tmp")), "test-call"),
            AssetSetVisibilityInput {
                key: key.to_owned(),
                visibility: visibility.to_owned(),
            },
        )
        .await
    }

    /// A store that can record visibility (a public base URL is configured).
    async fn seeded() -> Arc<MockAssetStore> {
        let store = Arc::new(
            MockAssetStore::builder()
                .public_base_url("https://cdn.test")
                .build(),
        );
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
        let tool = AssetSetVisibilityTool;
        assert_eq!(
            xai_tool_runtime::Tool::id(&tool).as_str(),
            ASSET_SET_VISIBILITY_TOOL_NAME
        );
        assert!(!crate::types::tool_metadata::ToolMetadata::is_read_only(
            &tool
        ));
    }

    #[tokio::test]
    async fn happy_path_publishes_and_reports_enforcement_honestly() {
        let store = seeded().await;
        let out = run(
            &AssetSetVisibilityTool,
            store.clone(),
            "uploads/a.txt",
            "public",
        )
        .await
        .unwrap();

        match out {
            ToolOutput::AssetSetVisibility(out) => {
                assert_eq!(out.key, "uploads/a.txt");
                assert_eq!(out.visibility, "public");
                assert!(!out.enforced, "local records without enforcing");
                assert!(out.hint.is_some());
                assert_eq!(
                    out.public_url.as_deref(),
                    Some("https://cdn.test/uploads/a.txt")
                );
                assert!(out.text.contains("is now public"));
            }
            other => panic!("expected AssetSetVisibility, got {other:?}"),
        }
        assert_eq!(
            store.visibility_of(&AssetKey::parse("uploads/a.txt").unwrap()),
            Some(Visibility::Public)
        );
    }

    #[tokio::test]
    async fn private_is_always_explicit() {
        let store = seeded().await;
        let out = run(
            &AssetSetVisibilityTool,
            store.clone(),
            "uploads/a.txt",
            "PRIVATE",
        )
        .await
        .unwrap();

        match out {
            ToolOutput::AssetSetVisibility(out) => {
                assert_eq!(out.visibility, "private");
                assert!(out.text.contains("is now private"));
                assert!(!out.text.contains("Public URL"));
            }
            other => panic!("expected AssetSetVisibility, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn enforced_backend_drops_the_hint() {
        let store = Arc::new(MockAssetStore::builder().backend(BackendKind::S3).build());
        store
            .put(PutRequest::from_bytes(
                AssetKey::parse("uploads/a.txt").unwrap(),
                b"body".to_vec(),
                ContentType::default(),
            ))
            .await
            .unwrap();

        let out = run(&AssetSetVisibilityTool, store, "uploads/a.txt", "public")
            .await
            .unwrap();

        match out {
            ToolOutput::AssetSetVisibility(out) => {
                assert!(out.enforced);
                assert!(out.hint.is_none());
            }
            other => panic!("expected AssetSetVisibility, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn capability_fast_fails_on_gcs() {
        let store = Arc::new(MockAssetStore::builder().backend(BackendKind::Gcs).build());
        let err = run(
            &AssetSetVisibilityTool,
            store.clone(),
            "uploads/a.txt",
            "public",
        )
        .await
        .unwrap_err();

        assert_eq!(err.kind, xai_tool_runtime::ToolErrorKind::NotImplemented);
        let details = err.details.unwrap();
        assert_eq!(details["code"], "asset_unsupported");
        assert_eq!(details["backend"], "gcs");
        assert_eq!(details["operation"], "set_visibility");
        assert!(!store.was_called(AssetOperation::SetVisibility));
    }

    #[tokio::test]
    async fn missing_object_is_not_found() {
        let store = Arc::new(
            MockAssetStore::builder()
                .public_base_url("https://cdn.test")
                .build(),
        );
        let err = run(&AssetSetVisibilityTool, store, "uploads/none", "public")
            .await
            .unwrap_err();
        assert_eq!(err.kind, xai_tool_runtime::ToolErrorKind::NotFound);
    }

    #[tokio::test]
    async fn invalid_visibility_is_rejected_before_any_call() {
        let store = seeded().await;
        let err = run(
            &AssetSetVisibilityTool,
            store.clone(),
            "uploads/a.txt",
            "shared",
        )
        .await
        .unwrap_err();
        assert_eq!(err.kind, xai_tool_runtime::ToolErrorKind::InvalidArguments);
        assert!(!store.was_called(AssetOperation::SetVisibility));
    }

    #[tokio::test]
    async fn store_failure_keeps_the_shared_projection() {
        let store = Arc::new(
            MockAssetStore::builder()
                .public_base_url("https://cdn.test")
                .fail_with(
                    AssetOperation::SetVisibility,
                    AssetError::PreconditionFailed {
                        key: "uploads/a.txt".into(),
                        detail: "etag".into(),
                    },
                )
                .build(),
        );
        let err = run(&AssetSetVisibilityTool, store, "uploads/a.txt", "public")
            .await
            .unwrap_err();
        assert_eq!(err.details.unwrap()["code"], "asset_precondition_failed");
    }
}
