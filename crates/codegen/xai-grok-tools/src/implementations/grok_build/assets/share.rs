//! `asset_share` — mint a time-limited read URL for a stored asset.
//!
//! Read-only: the object is probed with `exists` (a presign is pure string work
//! on S3, so a missing key would otherwise hand back a URL that 404s), then a
//! `presign_get` URL is returned with its exact expiry.
//!
//! The TTL is validated against the SigV4 window *before* the store is called;
//! out of range is an error, never a silent clamp (contract I4).

use xai_file_utils::assets::{AssetOperation, BackendKind};

use super::{
    parse_key, project_asset_error, require_capability, require_store, resolve_ttl, visibility_hint,
};
use crate::types::output::ToolOutput;
use crate::types::requirements::{Expr, ToolRequirement};
use crate::types::tool::{ToolKind, ToolNamespace};

/// Canonical tool name.
pub const ASSET_SHARE_TOOL_NAME: &str = "asset_share";

/// Default share window when the caller does not pass `ttl_secs`.
pub const DEFAULT_SHARE_TTL_SECS: u64 = xai_grok_config_types::DEFAULT_ASSET_TTL_SECS;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct AssetShareInput {
    #[schemars(description = "Key of the stored object, e.g. `reports/q3.pdf`.")]
    pub key: String,

    #[serde(default)]
    #[schemars(
        description = "How long the URL stays valid, in seconds. Defaults to 3600; bounds are 1..=604800 (the SigV4 ceiling). Out of range is an error."
    )]
    pub ttl_secs: Option<u64>,
}

/// Structured result of one share.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AssetShareOutput {
    pub key: String,
    pub url: String,
    pub method: String,
    pub expires_in_secs: u64,
    pub expires_at: String,
    /// True when the URL is synthesized rather than cryptographically signed
    /// (the local `file://` form, a GCS emulation): it carries no authorization.
    pub emulated: bool,
    /// Stable URL when the store has a public base configured. Present even for
    /// a private object — it only resolves once visibility is `public`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_url: Option<String>,
    /// True only when the backend actually enforces visibility.
    pub visibility_enforced: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// Pre-formatted model-facing prose.
    pub text: String,
}

#[derive(Debug, Default)]
pub struct AssetShareTool;

impl crate::types::tool_metadata::ToolMetadata for AssetShareTool {
    fn kind(&self) -> ToolKind {
        ToolKind::AssetShare
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    fn description_template(&self) -> &str {
        "Mint a time-limited URL for a stored asset. The URL is valid for `ttl_secs` \
         (default 3600, maximum 604800). Read-only: nothing is modified."
    }

    fn requires_expr(&self) -> Expr<ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for AssetShareTool {
    type Args = AssetShareInput;
    type Output = ToolOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new(ASSET_SHARE_TOOL_NAME).expect("valid tool id")
    }

    fn description(
        &self,
        _ctx: &xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            ASSET_SHARE_TOOL_NAME,
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
        name = "tool.asset_share",
        skip_all,
        fields(key = %input.key, ttl_secs = input.ttl_secs.unwrap_or(DEFAULT_SHARE_TTL_SECS))
    )]
    async fn run(
        &self,
        ctx: xai_tool_runtime::ToolCallContext,
        input: AssetShareInput,
    ) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
        use crate::types::tool_metadata::shared_resources;

        let ttl = resolve_ttl(input.ttl_secs, DEFAULT_SHARE_TTL_SECS)?;
        let key = parse_key(&input.key)?;

        let resources = shared_resources(&ctx)?;
        let store = require_store(&resources).await?;
        require_capability(store.as_ref(), AssetOperation::Exists)?;
        require_capability(store.as_ref(), AssetOperation::PresignGet)?;

        if !store.exists(&key).await.map_err(project_asset_error)? {
            return Err(project_asset_error(
                xai_file_utils::assets::AssetError::NotFound {
                    key: key.to_string(),
                },
            ));
        }

        let presigned = store
            .presign_get(&key, ttl)
            .await
            .map_err(project_asset_error)?;
        let backend = store.backend();
        let enforced = store.capabilities().visibility_enforced;
        let public_url = store.public_url(&key);
        let hint = visibility_hint(backend, enforced);

        let mut text = format!(
            "Share URL for `{}` on the {} backend — valid for {}s (expires {}): {}",
            presigned.key,
            backend,
            presigned.expires_in.as_secs(),
            presigned.expires_at.to_rfc3339(),
            presigned.url,
        );
        if presigned.emulated {
            text.push_str(
                " The URL is emulated (no signature): the reader needs access to the backend.",
            );
        }
        if let Some(url) = &public_url {
            text.push_str(&format!(" Stable public URL: {url}."));
        }
        if let Some(hint) = &hint {
            text.push(' ');
            text.push_str(hint);
        }

        Ok(ToolOutput::AssetShare(AssetShareOutput {
            key: presigned.key.to_string(),
            url: presigned.url,
            method: presigned.method.to_string(),
            expires_in_secs: presigned.expires_in.as_secs(),
            expires_at: presigned.expires_at.to_rfc3339(),
            emulated: presigned.emulated,
            public_url,
            visibility_enforced: enforced,
            hint,
            text,
        }))
    }
}

/// Backends that report `visibility_enforced` are the only ones where the
/// recorded flag is a guarantee. Kept here so the test can assert the set.
pub const ENFORCING_BACKENDS: [BackendKind; 1] = [BackendKind::S3];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::implementations::grok_build::assets::test_support::resources_with_store;
    use crate::types::tool_metadata::test_ctx_with_call_id;
    use std::path::Path;
    use std::sync::Arc;
    use xai_file_utils::assets::{
        AssetError, AssetKey, AssetStore, BackendCapabilities, ContentType, MockAssetStore,
        PutRequest, SharedAssetStore,
    };

    fn input(key: &str) -> AssetShareInput {
        AssetShareInput {
            key: key.to_owned(),
            ttl_secs: None,
        }
    }

    async fn run(
        tool: &AssetShareTool,
        store: SharedAssetStore,
        input: AssetShareInput,
    ) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
        xai_tool_runtime::Tool::run(
            tool,
            test_ctx_with_call_id(resources_with_store(store, Path::new("/tmp")), "test-call"),
            input,
        )
        .await
    }

    async fn store_with_object() -> Arc<MockAssetStore> {
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
    fn tool_name_and_read_only_classification() {
        let tool = AssetShareTool;
        assert_eq!(
            xai_tool_runtime::Tool::id(&tool).as_str(),
            ASSET_SHARE_TOOL_NAME
        );
        assert!(crate::types::tool_metadata::ToolMetadata::is_read_only(
            &tool
        ));
        assert!(
            xai_tool_runtime::Tool::capabilities(&tool).is_read_only,
            "capabilities must agree with the kind"
        );
    }

    #[tokio::test]
    async fn happy_path_returns_a_bounded_url() {
        let store = store_with_object().await;
        let out = run(&AssetShareTool, store.clone(), input("uploads/a.txt"))
            .await
            .expect("share succeeds");

        match out {
            ToolOutput::AssetShare(out) => {
                assert_eq!(out.key, "uploads/a.txt");
                assert_eq!(out.method, "GET");
                assert_eq!(out.expires_in_secs, DEFAULT_SHARE_TTL_SECS);
                assert!(out.emulated, "the mock synthesizes URLs");
                assert!(!out.visibility_enforced);
                assert!(out.hint.is_some());
                assert!(out.text.contains("Share URL for `uploads/a.txt`"));
                assert!(!out.expires_at.is_empty());
            }
            other => panic!("expected AssetShare, got {other:?}"),
        }

        assert!(store.was_called(AssetOperation::Exists));
        assert!(store.was_called(AssetOperation::PresignGet));
        assert!(!store.was_called(AssetOperation::SetVisibility));
    }

    #[tokio::test]
    async fn custom_ttl_is_honored() {
        let store = store_with_object().await;
        let out = run(
            &AssetShareTool,
            store,
            AssetShareInput {
                key: "uploads/a.txt".into(),
                ttl_secs: Some(60),
            },
        )
        .await
        .unwrap();

        match out {
            ToolOutput::AssetShare(out) => assert_eq!(out.expires_in_secs, 60),
            other => panic!("expected AssetShare, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn ttl_bounds_fail_before_any_store_call() {
        let store = store_with_object().await;
        for (ttl, expected) in [
            (0_u64, "asset_ttl_out_of_range"),
            (604_801, "asset_ttl_out_of_range"),
        ] {
            let err = run(
                &AssetShareTool,
                store.clone(),
                AssetShareInput {
                    key: "uploads/a.txt".into(),
                    ttl_secs: Some(ttl),
                },
            )
            .await
            .unwrap_err();
            assert_eq!(err.details.unwrap()["code"], expected, "ttl={ttl}");
        }
        assert!(!store.was_called(AssetOperation::Exists));
        assert!(!store.was_called(AssetOperation::PresignGet));
    }

    #[tokio::test]
    async fn missing_object_is_not_found() {
        let store = Arc::new(MockAssetStore::new());
        let err = run(&AssetShareTool, store.clone(), input("uploads/none"))
            .await
            .unwrap_err();
        assert_eq!(err.kind, xai_tool_runtime::ToolErrorKind::NotFound);
        assert_eq!(err.details.unwrap()["code"], "asset_not_found");
        assert!(!store.was_called(AssetOperation::PresignGet));
    }

    #[tokio::test]
    async fn invalid_key_is_rejected_before_any_call() {
        let store = store_with_object().await;
        let err = run(&AssetShareTool, store.clone(), input("a b"))
            .await
            .unwrap_err();
        assert_eq!(err.kind, xai_tool_runtime::ToolErrorKind::InvalidArguments);
        assert!(!store.was_called(AssetOperation::Exists));
    }

    #[tokio::test]
    async fn capability_fast_fails_when_presign_is_unsupported() {
        let store = Arc::new(
            MockAssetStore::builder()
                .capabilities(BackendCapabilities {
                    presign_get: false,
                    ..BackendCapabilities::LOCAL
                })
                .build(),
        );
        let err = run(&AssetShareTool, store.clone(), input("uploads/a.txt"))
            .await
            .unwrap_err();
        assert_eq!(err.kind, xai_tool_runtime::ToolErrorKind::NotImplemented);
        assert_eq!(err.details.unwrap()["code"], "asset_unsupported");
        assert!(!store.was_called(AssetOperation::PresignGet));
    }

    #[tokio::test]
    async fn unauthorized_projects_to_the_auth_kind() {
        let store = Arc::new(
            MockAssetStore::builder()
                .fail_with(
                    AssetOperation::PresignGet,
                    AssetError::Unauthorized {
                        backend: BackendKind::S3,
                        detail: "401".into(),
                    },
                )
                .build(),
        );
        store
            .put(PutRequest::from_bytes(
                AssetKey::parse("uploads/a.txt").unwrap(),
                b"x".to_vec(),
                ContentType::default(),
            ))
            .await
            .unwrap();

        let err = run(&AssetShareTool, store, input("uploads/a.txt"))
            .await
            .unwrap_err();
        assert_eq!(err.kind, xai_tool_runtime::ToolErrorKind::Unauthorized);
        assert_eq!(err.details.unwrap()["retryable"], false);
    }

    #[test]
    fn only_s3_enforces_visibility_today() {
        for backend in [
            BackendKind::S3,
            BackendKind::Gcs,
            BackendKind::Local,
            BackendKind::Proxy,
        ] {
            let enforced = BackendCapabilities::for_kind(backend).visibility_enforced;
            assert_eq!(enforced, ENFORCING_BACKENDS.contains(&backend), "{backend}");
        }
    }
}
