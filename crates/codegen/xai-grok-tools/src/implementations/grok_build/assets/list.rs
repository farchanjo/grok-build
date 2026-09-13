//! `asset_list` — page through the objects held in the session asset store.
//!
//! Read-only. The reserved `_meta/` sidecar namespace stays hidden unless the
//! caller asks for it. Pagination is opaque: the tool hands back whatever
//! cursor the backend produced and accepts it verbatim on the next call.

use xai_file_utils::assets::{AssetOperation, AssetPrefix, ListCursor, ListQuery};

use super::{project_asset_error, require_capability, require_store};
use crate::types::output::ToolOutput;
use crate::types::requirements::{Expr, ToolRequirement};
use crate::types::tool::{ToolKind, ToolNamespace};

/// Canonical tool name.
pub const ASSET_LIST_TOOL_NAME: &str = "asset_list";

/// Page size used when the caller does not pass `limit`.
pub const DEFAULT_LIST_LIMIT: u32 = xai_file_utils::assets::DEFAULT_LIST_LIMIT;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct AssetListInput {
    #[serde(default)]
    #[schemars(
        description = "Key prefix to filter by, e.g. `reports/`. Omit to list every object."
    )]
    pub prefix: Option<String>,

    #[serde(default)]
    #[schemars(description = "Page size, 1..=1000. Defaults to 100.")]
    pub limit: Option<u32>,

    #[serde(default)]
    #[schemars(description = "Opaque cursor from a previous call's `next_cursor`.")]
    pub cursor: Option<String>,

    #[serde(default)]
    #[schemars(
        description = "Include the reserved `_meta/` visibility sidecars. Hidden by default."
    )]
    pub include_meta: bool,
}

/// One listed object.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AssetListItem {
    pub key: String,
    pub size_bytes: u64,
    pub content_type: String,
    pub visibility: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_at: Option<String>,
}

/// Structured result of one page.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AssetListOutput {
    pub backend: String,
    pub prefix: String,
    pub items: Vec<AssetListItem>,
    /// True when the backend stopped early; `next_cursor` continues the walk.
    pub truncated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// Pre-formatted model-facing prose.
    pub text: String,
}

#[derive(Debug, Default)]
pub struct AssetListTool;

impl crate::types::tool_metadata::ToolMetadata for AssetListTool {
    fn kind(&self) -> ToolKind {
        ToolKind::AssetList
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    fn description_template(&self) -> &str {
        "List stored assets, newest page first. Pass `next_cursor` back to continue. \
         Read-only: nothing is modified."
    }

    fn requires_expr(&self) -> Expr<ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for AssetListTool {
    type Args = AssetListInput;
    type Output = ToolOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new(ASSET_LIST_TOOL_NAME).expect("valid tool id")
    }

    fn description(
        &self,
        _ctx: &xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            ASSET_LIST_TOOL_NAME,
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

    #[tracing::instrument(
        name = "tool.asset_list",
        skip_all,
        fields(prefix = input.prefix.as_deref().unwrap_or(""), limit = input.limit.unwrap_or(DEFAULT_LIST_LIMIT))
    )]
    async fn run(
        &self,
        ctx: xai_tool_runtime::ToolCallContext,
        input: AssetListInput,
    ) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
        use crate::types::tool_metadata::shared_resources;

        let prefix = match input.prefix.as_deref().map(str::trim) {
            Some(raw) if !raw.is_empty() => AssetPrefix::parse(raw).map_err(|err| {
                xai_tool_runtime::ToolError::invalid_arguments(format!(
                    "invalid `prefix` `{raw}`: {err}"
                ))
            })?,
            _ => AssetPrefix::default(),
        };

        let mut query = ListQuery::new(prefix.clone())
            .with_limit(input.limit.unwrap_or(DEFAULT_LIST_LIMIT))
            .with_include_meta(input.include_meta);
        if let Some(cursor) = input
            .cursor
            .as_deref()
            .map(str::trim)
            .filter(|c| !c.is_empty())
        {
            query = query.with_cursor(ListCursor::new(cursor));
        }
        query.validate().map_err(project_asset_error)?;

        let resources = shared_resources(&ctx)?;
        let store = require_store(&resources).await?;
        require_capability(store.as_ref(), AssetOperation::List)?;

        let page = store.list(query).await.map_err(project_asset_error)?;
        let backend = store.backend();

        let items: Vec<AssetListItem> = page
            .items
            .iter()
            .map(|meta| AssetListItem {
                key: meta.key.to_string(),
                size_bytes: meta.size_bytes,
                content_type: meta.content_type.to_string(),
                visibility: meta.visibility.to_string(),
                modified_at: meta.modified_at.clone(),
            })
            .collect();

        let next_cursor = page
            .next_cursor
            .as_ref()
            .map(ListCursor::as_str)
            .map(str::to_owned);
        let text = if items.is_empty() {
            if prefix.is_empty() {
                format!("No assets stored on the {backend} backend.")
            } else {
                format!("No assets under `{prefix}` on the {backend} backend.")
            }
        } else {
            let mut lines = vec![format!(
                "{} asset(s) on the {backend} backend{}:",
                items.len(),
                if prefix.is_empty() {
                    String::new()
                } else {
                    format!(" under `{prefix}`")
                }
            )];
            for item in &items {
                lines.push(format!(
                    "- {} ({} bytes, {}, {})",
                    item.key, item.size_bytes, item.content_type, item.visibility
                ));
            }
            if page.truncated {
                match &next_cursor {
                    Some(cursor) => lines.push(format!(
                        "More results available — call asset_list again with cursor `{cursor}`."
                    )),
                    None => lines.push("More results available.".to_owned()),
                }
            }
            lines.join("\n")
        };

        Ok(ToolOutput::AssetList(AssetListOutput {
            backend: backend.to_string(),
            prefix: prefix.to_string(),
            items,
            truncated: page.truncated,
            next_cursor,
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

    fn input() -> AssetListInput {
        AssetListInput {
            prefix: None,
            limit: None,
            cursor: None,
            include_meta: false,
        }
    }

    async fn run(
        tool: &AssetListTool,
        store: SharedAssetStore,
        input: AssetListInput,
    ) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
        xai_tool_runtime::Tool::run(
            tool,
            test_ctx_with_call_id(resources_with_store(store, Path::new("/tmp")), "test-call"),
            input,
        )
        .await
    }

    async fn seeded() -> Arc<MockAssetStore> {
        let store = Arc::new(MockAssetStore::new());
        for name in ["a.txt", "b.txt", "c.txt"] {
            store
                .put(PutRequest::from_bytes(
                    AssetKey::parse(&format!("uploads/{name}")).unwrap(),
                    name.as_bytes().to_vec(),
                    ContentType::default(),
                ))
                .await
                .unwrap();
        }
        store
    }

    #[test]
    fn tool_name_and_read_only_classification() {
        let tool = AssetListTool;
        assert_eq!(
            xai_tool_runtime::Tool::id(&tool).as_str(),
            ASSET_LIST_TOOL_NAME
        );
        assert!(crate::types::tool_metadata::ToolMetadata::is_read_only(
            &tool
        ));
    }

    #[tokio::test]
    async fn happy_path_lists_the_requested_prefix() {
        let store = seeded().await;
        let out = run(&AssetListTool, store.clone(), input()).await.unwrap();

        match out {
            ToolOutput::AssetList(out) => {
                assert_eq!(out.items.len(), 3);
                assert!(!out.truncated);
                assert!(out.next_cursor.is_none());
                assert_eq!(out.items[0].key, "uploads/a.txt");
                assert_eq!(out.items[0].size_bytes, 5);
                assert_eq!(out.items[0].visibility, "private");
                assert!(out.text.contains("3 asset(s)"));
            }
            other => panic!("expected AssetList, got {other:?}"),
        }
        assert!(store.was_called(AssetOperation::List));
    }

    #[tokio::test]
    async fn empty_result_is_not_an_error() {
        let store = Arc::new(MockAssetStore::new());
        let out = run(
            &AssetListTool,
            store,
            AssetListInput {
                prefix: Some("nothing/".into()),
                ..input()
            },
        )
        .await
        .unwrap();

        match out {
            ToolOutput::AssetList(out) => {
                assert!(out.items.is_empty());
                assert!(out.text.contains("No assets under `nothing/`"));
            }
            other => panic!("expected AssetList, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn pagination_cursor_round_trips() {
        let store = seeded().await;
        let first = run(
            &AssetListTool,
            store.clone(),
            AssetListInput {
                limit: Some(2),
                ..input()
            },
        )
        .await
        .unwrap();

        let cursor = match first {
            ToolOutput::AssetList(out) => {
                assert!(out.truncated);
                out.next_cursor.expect("cursor when truncated")
            }
            other => panic!("expected AssetList, got {other:?}"),
        };

        let second = run(
            &AssetListTool,
            store,
            AssetListInput {
                limit: Some(2),
                cursor: Some(cursor),
                ..input()
            },
        )
        .await
        .unwrap();

        match second {
            ToolOutput::AssetList(out) => {
                assert_eq!(out.items.len(), 1);
                assert_eq!(out.items[0].key, "uploads/c.txt");
                assert!(!out.truncated);
            }
            other => panic!("expected AssetList, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn limit_bounds_fail_before_any_call() {
        let store = seeded().await;
        for limit in [0_u32, 1_001] {
            let err = run(
                &AssetListTool,
                store.clone(),
                AssetListInput {
                    limit: Some(limit),
                    ..input()
                },
            )
            .await
            .unwrap_err();
            assert_eq!(
                err.kind,
                xai_tool_runtime::ToolErrorKind::InvalidArguments,
                "limit={limit}"
            );
        }
        assert!(!store.was_called(AssetOperation::List));
    }

    #[tokio::test]
    async fn capability_fast_fails_on_a_backend_without_list() {
        let store = Arc::new(
            MockAssetStore::builder()
                .backend(BackendKind::Proxy)
                .build(),
        );
        let err = run(&AssetListTool, store.clone(), input())
            .await
            .unwrap_err();

        assert_eq!(err.kind, xai_tool_runtime::ToolErrorKind::NotImplemented);
        assert_eq!(err.details.unwrap()["operation"], "list");
        assert!(!store.was_called(AssetOperation::List));
    }

    #[tokio::test]
    async fn access_denied_projects_to_permission_denied() {
        let store = Arc::new(
            MockAssetStore::builder()
                .fail_with(
                    AssetOperation::List,
                    AssetError::AccessDenied {
                        backend: BackendKind::S3,
                        detail: "403".into(),
                    },
                )
                .build(),
        );
        let err = run(&AssetListTool, store, input()).await.unwrap_err();
        assert_eq!(err.kind, xai_tool_runtime::ToolErrorKind::PermissionDenied);
        assert_eq!(err.details.unwrap()["code"], "asset_access_denied");
    }

    #[tokio::test]
    async fn invalid_prefix_is_rejected_before_any_call() {
        let store = seeded().await;
        let err = run(
            &AssetListTool,
            store.clone(),
            AssetListInput {
                prefix: Some("../x".into()),
                ..input()
            },
        )
        .await
        .unwrap_err();
        assert_eq!(err.kind, xai_tool_runtime::ToolErrorKind::InvalidArguments);
        assert!(!store.was_called(AssetOperation::List));
    }

    #[test]
    fn store_errors_keep_the_shared_code_table() {
        // A store-level `InvalidKey` (list limit) still arrives as a validation
        // error through the single projection table.
        let projected = crate::implementations::grok_build::assets::project_asset_error(
            AssetError::InvalidKey {
                key: "uploads/".into(),
                reason: "limit".into(),
            },
        );
        assert_eq!(
            projected.kind,
            xai_tool_runtime::ToolErrorKind::InvalidArguments
        );
    }
}
