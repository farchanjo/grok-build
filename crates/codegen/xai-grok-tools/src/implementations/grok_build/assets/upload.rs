//! `asset_upload` — stream a local file into the session asset store.
//!
//! The object is uploaded with [`AssetStore::put_file`] so a multi-gigabyte
//! artifact is never materialized in memory (contract §9). The destination key
//! defaults to the sanitized file name, optionally under a caller prefix, and
//! an explicit `key` always wins.
//!
//! Overwriting is deliberate and reported: `put` is idempotent, so uploading a
//! second file to the same key replaces the first and the output says so.

use std::path::{Path, PathBuf};

use xai_file_utils::assets::{AssetKey, AssetOperation, AssetPrefix, ContentType, PutRequest};

use super::{
    parse_key, parse_visibility, project_asset_error, require_capability, require_store,
    visibility_hint,
};
use crate::types::output::ToolOutput;
use crate::types::requirements::{Expr, ToolRequirement};
use crate::types::tool::{ToolKind, ToolNamespace};

/// Canonical tool name.
pub const ASSET_UPLOAD_TOOL_NAME: &str = "asset_upload";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct AssetUploadInput {
    #[schemars(
        description = "Local file path to upload (absolute, or relative to the session cwd)."
    )]
    pub path: String,

    #[serde(default)]
    #[schemars(
        description = "Destination key inside the store, e.g. `reports/q3.pdf`. Defaults to the sanitized file name, optionally under `prefix`. Replaces any existing object at that key."
    )]
    pub key: Option<String>,

    #[serde(default)]
    #[schemars(
        description = "Optional key prefix for the derived key, e.g. `reports/`. Ignored when `key` is set."
    )]
    pub prefix: Option<String>,

    #[serde(default)]
    #[schemars(
        description = "Media type, e.g. `image/png`. Inferred from the file extension when omitted."
    )]
    pub content_type: Option<String>,

    #[serde(default)]
    #[schemars(description = "`private` (default) or `public`. Public is always explicit.")]
    pub visibility: Option<String>,
}

/// Structured result of one upload.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AssetUploadOutput {
    pub key: String,
    pub size_bytes: u64,
    pub content_type: String,
    pub visibility: String,
    /// True only when the backend actually enforces the recorded visibility.
    pub visibility_enforced: bool,
    pub backend: String,
    /// True when an object already existed at `key` and was replaced.
    pub overwritten: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_url: Option<String>,
    /// Present when the backend records visibility without enforcing it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// Pre-formatted model-facing prose.
    pub text: String,
}

#[derive(Debug, Default)]
pub struct AssetUploadTool;

impl crate::types::tool_metadata::ToolMetadata for AssetUploadTool {
    fn kind(&self) -> ToolKind {
        ToolKind::AssetUpload
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    fn description_template(&self) -> &str {
        "Upload a local file to the session asset store and return its key. \
         Use `asset_share` to mint a time-limited URL for a stored object. \
         The key defaults to the file name; an existing object at the same key is replaced."
    }

    fn requires_expr(&self) -> Expr<ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for AssetUploadTool {
    type Args = AssetUploadInput;
    type Output = ToolOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new(ASSET_UPLOAD_TOOL_NAME).expect("valid tool id")
    }

    fn description(
        &self,
        _ctx: &xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            ASSET_UPLOAD_TOOL_NAME,
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
        name = "tool.asset_upload",
        skip_all,
        fields(path = %input.path, key = input.key.as_deref().unwrap_or(""))
    )]
    async fn run(
        &self,
        ctx: xai_tool_runtime::ToolCallContext,
        input: AssetUploadInput,
    ) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
        use crate::types::tool_metadata::{resolve_cwd, shared_resources};

        let resources = shared_resources(&ctx)?;
        let store = require_store(&resources).await?;
        require_capability(store.as_ref(), AssetOperation::PutFile)?;

        let cwd = resolve_cwd(&ctx, &resources).await?;
        let path = absolutize(&cwd, &input.path);
        let metadata = tokio::fs::metadata(&path).await.map_err(|err| {
            xai_tool_runtime::ToolError::invalid_arguments(format!(
                "cannot read `{}`: {err}",
                path.display()
            ))
        })?;
        if !metadata.is_file() {
            return Err(xai_tool_runtime::ToolError::invalid_arguments(format!(
                "`{}` is not a regular file",
                path.display()
            )));
        }

        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("asset");
        let key = resolve_key(&input, file_name)?;
        let content_type = match input.content_type.as_deref().map(str::trim) {
            Some(raw) if !raw.is_empty() => ContentType::parse(raw).map_err(|err| {
                xai_tool_runtime::ToolError::invalid_arguments(format!(
                    "invalid `content_type` `{raw}`: {err}"
                ))
            })?,
            _ => ContentType::infer_from_path(&path),
        };
        let visibility = parse_visibility(input.visibility.as_deref())?;

        require_capability(store.as_ref(), AssetOperation::Exists)?;
        let overwritten = store.exists(&key).await.map_err(project_asset_error)?;

        let request = PutRequest::from_file(key.clone(), path.clone(), content_type)
            .with_visibility(visibility)
            .with_expected_size(metadata.len());
        let meta = store.put_file(request).await.map_err(project_asset_error)?;

        let public_url = store.public_url(&key);
        let hint = visibility_hint(meta.backend, meta.visibility_enforced);
        let mut text = format!(
            "Uploaded `{}` ({} bytes, {}) to the {} asset store as {}; visibility {}",
            meta.key,
            meta.size_bytes,
            meta.content_type,
            meta.backend,
            if overwritten {
                "an overwrite"
            } else {
                "a new object"
            },
            meta.visibility,
        );
        if let Some(url) = &public_url {
            text.push_str(&format!(". Public URL: {url}"));
        } else {
            text.push('.');
        }
        if let Some(hint) = &hint {
            text.push(' ');
            text.push_str(hint);
        }

        tracing::info!(
            key = %meta.key,
            bytes = meta.size_bytes,
            backend = %meta.backend,
            "asset uploaded"
        );

        Ok(ToolOutput::AssetUpload(AssetUploadOutput {
            key: meta.key.to_string(),
            size_bytes: meta.size_bytes,
            content_type: meta.content_type.to_string(),
            visibility: meta.visibility.to_string(),
            visibility_enforced: meta.visibility_enforced,
            backend: meta.backend.to_string(),
            overwritten,
            public_url,
            hint,
            text,
        }))
    }
}

/// Absolute form of `raw`: the session cwd joins a relative path, an absolute
/// path is used as given.
fn absolutize(cwd: &Path, raw: &str) -> PathBuf {
    let path = Path::new(raw.trim());
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

/// Resolve the destination key: explicit `key` wins, otherwise the sanitized
/// file name under the optional `prefix`.
fn resolve_key(
    input: &AssetUploadInput,
    file_name: &str,
) -> Result<AssetKey, xai_tool_runtime::ToolError> {
    if let Some(raw) = input
        .key
        .as_deref()
        .map(str::trim)
        .filter(|k| !k.is_empty())
    {
        return parse_key(raw);
    }

    let prefix = match input
        .prefix
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        Some(raw) => AssetPrefix::parse(raw).map_err(|err| {
            xai_tool_runtime::ToolError::invalid_arguments(format!(
                "invalid `prefix` `{raw}`: {err}"
            ))
        })?,
        None => AssetPrefix::default(),
    };

    let derived = format!("{}{}", prefix.normalized(), sanitize_segment(file_name));
    AssetKey::parse(&derived).map_err(|err| {
        xai_tool_runtime::ToolError::invalid_arguments(format!(
            "derived key `{derived}` is not a valid asset key ({err}); pass an explicit `key`"
        ))
    })
}

/// Reduce one file name to the accepted asset-key alphabet
/// (`[A-Za-z0-9._~-]`), collapsing runs of rejected characters into a single
/// `-`. Non-ASCII bytes and spaces are replaced, so `my report (1).pdf` becomes
/// `my-report-1.pdf` and the extension survives.
fn sanitize_segment(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut pending_dash = false;
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '~' | '-') {
            // A separator run immediately before an extension dot is dropped,
            // so `report (1).pdf` does not become `report-1-.pdf`.
            if pending_dash && !out.is_empty() && ch != '.' {
                out.push('-');
            }
            pending_dash = false;
            out.push(ch);
        } else {
            pending_dash = true;
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "asset".to_owned()
    } else {
        trimmed.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::implementations::grok_build::assets::test_support::resources_with_store;
    use crate::types::tool_metadata::test_ctx_with_call_id;
    use std::sync::Arc;
    use xai_file_utils::assets::{BackendKind, MockAssetStore, SharedAssetStore};

    fn input(path: &str) -> AssetUploadInput {
        AssetUploadInput {
            path: path.to_owned(),
            key: None,
            prefix: None,
            content_type: None,
            visibility: None,
        }
    }

    async fn run(
        tool: &AssetUploadTool,
        store: SharedAssetStore,
        cwd: &Path,
        input: AssetUploadInput,
    ) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
        xai_tool_runtime::Tool::run(
            tool,
            test_ctx_with_call_id(resources_with_store(store, cwd), "test-call"),
            input,
        )
        .await
    }

    #[test]
    fn tool_name_and_description() {
        let tool = AssetUploadTool;
        assert_eq!(
            xai_tool_runtime::Tool::id(&tool).as_str(),
            ASSET_UPLOAD_TOOL_NAME
        );
        assert!(
            crate::types::tool_metadata::ToolMetadata::description_template(&tool)
                .contains("Upload a local file")
        );
        assert!(!crate::types::tool_metadata::ToolMetadata::is_read_only(
            &tool
        ));
    }

    #[test]
    fn sanitize_segment_keeps_the_extension() {
        assert_eq!(sanitize_segment("report.pdf"), "report.pdf");
        assert_eq!(sanitize_segment("my report (1).pdf"), "my-report-1.pdf");
        assert_eq!(sanitize_segment("café.png"), "caf.png");
        assert_eq!(sanitize_segment("a___b"), "a___b");
        assert_eq!(sanitize_segment("---"), "asset");
    }

    #[test]
    fn derived_keys_are_valid_and_prefixable() {
        let mut input = input("/tmp/my report (1).pdf");
        assert_eq!(
            resolve_key(&input, "my report (1).pdf").unwrap().as_str(),
            "my-report-1.pdf"
        );

        input.prefix = Some("reports/2026".into());
        assert_eq!(
            resolve_key(&input, "my report (1).pdf").unwrap().as_str(),
            "reports/2026/my-report-1.pdf"
        );

        input.key = Some("explicit/key.pdf".into());
        assert_eq!(
            resolve_key(&input, "ignored.pdf").unwrap().as_str(),
            "explicit/key.pdf"
        );

        input.key = Some("a b/c.pdf".into());
        assert!(resolve_key(&input, "x.pdf").is_err());
    }

    #[tokio::test]
    async fn happy_path_streams_the_file_into_the_store() {
        let dir = tempfile::TempDir::new().unwrap();
        let source = dir.path().join("my report.pdf");
        tokio::fs::write(&source, b"%PDF-1.4 body").await.unwrap();

        let mock = Arc::new(MockAssetStore::new());
        let out = run(
            &AssetUploadTool,
            mock.clone(),
            dir.path(),
            input(source.to_str().unwrap()),
        )
        .await
        .expect("upload succeeds");

        match out {
            ToolOutput::AssetUpload(out) => {
                assert_eq!(out.key, "my-report.pdf");
                assert_eq!(out.size_bytes, 13);
                assert_eq!(out.content_type, "application/pdf");
                assert_eq!(out.visibility, "private");
                assert!(!out.visibility_enforced);
                assert!(!out.overwritten);
                assert_eq!(out.backend, "local");
                assert!(out.hint.is_some(), "local records without enforcing");
                assert!(out.text.contains("Uploaded `my-report.pdf`"));
            }
            other => panic!("expected AssetUpload, got {other:?}"),
        }

        assert_eq!(
            &mock
                .raw(&AssetKey::parse("my-report.pdf").unwrap())
                .unwrap()[..],
            b"%PDF-1.4 body"
        );
        assert!(!mock.was_called(AssetOperation::Put), "put_file streams");
    }

    #[tokio::test]
    async fn re_upload_reports_the_overwrite() {
        let dir = tempfile::TempDir::new().unwrap();
        let source = dir.path().join("a.txt");
        tokio::fs::write(&source, b"one").await.unwrap();

        let mock = Arc::new(MockAssetStore::new());
        let _ = run(
            &AssetUploadTool,
            mock.clone(),
            dir.path(),
            input(source.to_str().unwrap()),
        )
        .await
        .unwrap();
        tokio::fs::write(&source, b"two").await.unwrap();
        let out = run(
            &AssetUploadTool,
            mock.clone(),
            dir.path(),
            input(source.to_str().unwrap()),
        )
        .await
        .unwrap();

        match out {
            ToolOutput::AssetUpload(out) => assert!(out.overwritten),
            other => panic!("expected AssetUpload, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn missing_source_file_is_an_argument_error() {
        let dir = tempfile::TempDir::new().unwrap();
        let mock = Arc::new(MockAssetStore::new());
        let err = run(
            &AssetUploadTool,
            mock.clone(),
            dir.path(),
            input("nope.txt"),
        )
        .await
        .unwrap_err();

        assert_eq!(err.kind, xai_tool_runtime::ToolErrorKind::InvalidArguments);
        assert!(!mock.was_called(AssetOperation::Exists));
    }

    #[tokio::test]
    async fn directory_source_is_rejected() {
        let dir = tempfile::TempDir::new().unwrap();
        let mock = Arc::new(MockAssetStore::new());
        let err = run(&AssetUploadTool, mock.clone(), dir.path(), input("."))
            .await
            .unwrap_err();
        assert!(err.detail.contains("not a regular file"), "{}", err.detail);
    }

    #[tokio::test]
    async fn capability_fast_fails_before_any_call() {
        let dir = tempfile::TempDir::new().unwrap();
        let source = dir.path().join("a.txt");
        tokio::fs::write(&source, b"x").await.unwrap();

        let mock = Arc::new(
            MockAssetStore::builder()
                .backend(BackendKind::Proxy)
                .capabilities(xai_file_utils::assets::BackendCapabilities {
                    put_file: false,
                    ..xai_file_utils::assets::BackendCapabilities::PROXY
                })
                .build(),
        );
        let err = run(
            &AssetUploadTool,
            mock.clone(),
            dir.path(),
            input(source.to_str().unwrap()),
        )
        .await
        .unwrap_err();

        assert_eq!(err.details.unwrap()["code"], "asset_unsupported");
        assert!(!mock.was_called(AssetOperation::PutFile));
        assert!(!mock.was_called(AssetOperation::Exists));
    }

    #[tokio::test]
    async fn store_errors_are_projected_with_the_shared_table() {
        let dir = tempfile::TempDir::new().unwrap();
        let source = dir.path().join("a.txt");
        tokio::fs::write(&source, b"x").await.unwrap();

        let mock = Arc::new(
            MockAssetStore::builder()
                .fail_with(
                    AssetOperation::PutFile,
                    xai_file_utils::assets::AssetError::Transient {
                        backend: BackendKind::S3,
                        detail: "503".into(),
                    },
                )
                .build(),
        );
        let err = run(
            &AssetUploadTool,
            mock.clone(),
            dir.path(),
            input(source.to_str().unwrap()),
        )
        .await
        .unwrap_err();

        assert_eq!(
            err.kind,
            xai_tool_runtime::ToolErrorKind::ServiceUnavailable
        );
        assert_eq!(err.details.unwrap()["retryable"], true);
    }

    #[tokio::test]
    async fn missing_store_resource_is_reported() {
        let resources = crate::types::resources::Resources::new().into_shared();
        let err = xai_tool_runtime::Tool::run(
            &AssetUploadTool,
            test_ctx_with_call_id(resources, "test-call"),
            input("a.txt"),
        )
        .await
        .unwrap_err();
        assert!(
            err.detail.contains("missing required resource"),
            "{}",
            err.detail
        );
    }
}
