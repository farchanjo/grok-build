//! `asset_download` — start a background download job for a stored object.
//!
//! The object streams straight to disk through [`AssetStore::download_to`] on a
//! spawned task, so a multi-gigabyte media file neither blocks the turn nor
//! lands in memory (contract §9, §12). The tool returns a `job_id`
//! immediately; `wait_secs` lets a small object finish inline.
//!
//! `dest` defaults to the file name derived from the key, resolved against the
//! session cwd. Parent directories are created; an existing file at `dest` is
//! replaced atomically (the adapter renames into place).

use std::path::{Path, PathBuf};

use xai_file_utils::assets::{AssetOperation, JobState};

use super::{
    parse_key, parse_wait_secs, require_capability, require_jobs, require_store, state_note,
    wait_for_job,
};
use crate::types::output::ToolOutput;
use crate::types::requirements::{Expr, ToolRequirement};
use crate::types::tool::{ToolKind, ToolNamespace};

/// Canonical tool name.
pub const ASSET_DOWNLOAD_TOOL_NAME: &str = "asset_download";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct AssetDownloadInput {
    #[schemars(description = "Key of the stored object, e.g. `reports/q3.pdf`.")]
    pub key: String,

    #[serde(default)]
    #[schemars(
        description = "Destination file path (absolute, or relative to the session cwd). Defaults to the key's file name in the session cwd. Parent directories are created; an existing file is replaced."
    )]
    pub dest: Option<String>,

    #[serde(default)]
    #[schemars(
        description = "Wait up to this many seconds for the download to finish inline (0..=600). Omit to return as soon as the job is started; a large object is still running when the tool returns."
    )]
    pub wait_secs: Option<u64>,
}

/// Structured result of one download job.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AssetDownloadOutput {
    /// Job handle for `asset_job_status`, `asset_job_cancel`, `asset_job_subscribe`.
    pub job_id: String,
    /// `queued`, `running`, `completed`, `failed`, or `cancelled`.
    pub state: String,
    pub key: String,
    /// Absolute destination path the bytes land in.
    pub dest: String,
    pub backend: String,
    /// Payload size, once the backend reported it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    pub bytes_transferred: u64,
    /// True when `wait_secs` was supplied, so the state is an outcome.
    pub waited: bool,
    /// Secret-free failure detail, present when the job failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Pre-formatted model-facing prose.
    pub text: String,
}

#[derive(Debug, Default)]
pub struct AssetDownloadTool;

impl crate::types::tool_metadata::ToolMetadata for AssetDownloadTool {
    fn kind(&self) -> ToolKind {
        ToolKind::AssetDownload
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    fn description_template(&self) -> &str {
        "Download a stored asset to a local file as a background job and return its \
         `job_id`. `dest` defaults to the key's file name in the session cwd. \
         Pass `wait_secs` to wait for a small object to finish inline."
    }

    fn requires_expr(&self) -> Expr<ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for AssetDownloadTool {
    type Args = AssetDownloadInput;
    type Output = ToolOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new(ASSET_DOWNLOAD_TOOL_NAME).expect("valid tool id")
    }

    fn description(
        &self,
        _ctx: &xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            ASSET_DOWNLOAD_TOOL_NAME,
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
        name = "tool.asset_download",
        skip_all,
        fields(key = %input.key, dest = input.dest.as_deref().unwrap_or(""))
    )]
    async fn run(
        &self,
        ctx: xai_tool_runtime::ToolCallContext,
        input: AssetDownloadInput,
    ) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
        use crate::types::tool_metadata::{resolve_cwd, shared_resources};

        let resources = shared_resources(&ctx)?;
        let store = require_store(&resources).await?;
        require_capability(store.as_ref(), AssetOperation::DownloadTo)?;

        let cwd = resolve_cwd(&ctx, &resources).await?;
        let key = parse_key(&input.key)?;
        let dest = resolve_dest(&cwd, input.dest.as_deref(), &key);
        let wait = parse_wait_secs(input.wait_secs)?;

        let registry = require_jobs(&resources).await?;
        let job_id = registry.spawn_download(store, key.clone(), dest.clone());

        let snapshot = match wait {
            Some(timeout) => wait_for_job(&registry, &job_id, Some(timeout)).await,
            None => registry.get(&job_id).await,
        };

        let (state, bytes_transferred, size_bytes, error, backend) = match snapshot {
            Some(snapshot) => (
                snapshot.state,
                snapshot.bytes_transferred,
                snapshot.bytes_total,
                snapshot.error,
                snapshot.backend.to_string(),
            ),
            // The registry only drops jobs at capacity; treat a vanished entry
            // as "queued and gone" rather than failing the call.
            None => (
                JobState::Queued,
                0,
                None,
                None,
                store_backend(&resources).await,
            ),
        };

        let mut text = match state {
            JobState::Completed => format!(
                "Downloaded `{key}` ({} bytes) from the {backend} asset store to `{}` \
                 (job `{job_id}`).",
                bytes_transferred,
                dest.display()
            ),
            _ => {
                let mut text = format!(
                    "Download `{key}` from the {backend} asset store to `{}` (job `{job_id}`): {}",
                    dest.display(),
                    state_note(state, wait.is_some()),
                );
                match &error {
                    Some(error) => text.push_str(&format!(": {error}")),
                    None => text
                        .push_str(". Follow it with `asset_job_status` or `asset_job_subscribe`."),
                }
                text
            }
        };
        if state == JobState::Failed
            && let Some(error) = &error
        {
            text.push_str(&format!(" ({error})"));
        }

        tracing::info!(
            job = %job_id,
            key = %key,
            dest = %dest.display(),
            state = %state,
            "asset download job started"
        );

        Ok(ToolOutput::AssetDownload(AssetDownloadOutput {
            job_id,
            state: state.as_str().to_owned(),
            key: key.to_string(),
            dest: dest.display().to_string(),
            backend,
            size_bytes,
            bytes_transferred,
            waited: wait.is_some(),
            error,
            text,
        }))
    }
}

/// Backend name of the session store, for the fallback path.
async fn store_backend(resources: &crate::types::resources::SharedResources) -> String {
    match require_store(resources).await {
        Ok(store) => store.backend().to_string(),
        Err(_) => "unknown".to_owned(),
    }
}

/// Resolve the destination: an explicit path (absolute or cwd-relative), else
/// the key's last segment under the session cwd.
fn resolve_dest(cwd: &Path, raw: Option<&str>, key: &xai_file_utils::assets::AssetKey) -> PathBuf {
    match raw.map(str::trim).filter(|value| !value.is_empty()) {
        Some(raw) => {
            let path = Path::new(raw);
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                cwd.join(path)
            }
        }
        None => {
            let name = key
                .as_str()
                .rsplit('/')
                .next()
                .filter(|name| !name.is_empty())
                .unwrap_or("asset");
            cwd.join(name)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::implementations::grok_build::assets::test_support::resources_with_store;
    use crate::types::tool_metadata::test_ctx_with_call_id;
    use std::sync::Arc;
    use xai_file_utils::assets::{
        AssetKey, AssetOperation, AssetStore, BackendKind, ContentType, MockAssetStore, PutRequest,
        SharedAssetStore,
    };

    fn key(raw: &str) -> AssetKey {
        AssetKey::parse(raw).unwrap()
    }

    async fn seeded_store() -> Arc<MockAssetStore> {
        let mock = Arc::new(MockAssetStore::new());
        mock.put(PutRequest::from_bytes(
            key("uploads/report.pdf"),
            b"%PDF-1.4 payload".to_vec(),
            ContentType::default(),
        ))
        .await
        .unwrap();
        mock
    }

    async fn run(
        store: SharedAssetStore,
        cwd: &Path,
        input: AssetDownloadInput,
    ) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
        xai_tool_runtime::Tool::run(
            &AssetDownloadTool,
            test_ctx_with_call_id(resources_with_store(store, cwd), "test-call"),
            input,
        )
        .await
    }

    fn input(key: &str) -> AssetDownloadInput {
        AssetDownloadInput {
            key: key.to_owned(),
            dest: None,
            wait_secs: None,
        }
    }

    #[test]
    fn tool_name_and_description() {
        let tool = AssetDownloadTool;
        assert_eq!(
            xai_tool_runtime::Tool::id(&tool).as_str(),
            ASSET_DOWNLOAD_TOOL_NAME
        );
        assert!(!crate::types::tool_metadata::ToolMetadata::is_read_only(
            &tool
        ));
    }

    #[test]
    fn dest_defaults_to_the_key_file_name_under_the_cwd() {
        let cwd = Path::new("/work");
        assert_eq!(
            resolve_dest(cwd, None, &key("reports/2026/q3.pdf")),
            PathBuf::from("/work/q3.pdf")
        );
        assert_eq!(
            resolve_dest(cwd, Some("out/q3.pdf"), &key("reports/q3.pdf")),
            PathBuf::from("/work/out/q3.pdf")
        );
        assert_eq!(
            resolve_dest(cwd, Some("/tmp/q3.pdf"), &key("reports/q3.pdf")),
            PathBuf::from("/tmp/q3.pdf")
        );
        // An all-whitespace dest behaves like an absent one.
        assert_eq!(
            resolve_dest(cwd, Some("   "), &key("reports/q3.pdf")),
            PathBuf::from("/work/q3.pdf")
        );
    }

    #[tokio::test]
    async fn returns_a_job_id_immediately_and_writes_the_file() {
        let dir = tempfile::TempDir::new().unwrap();
        let mock = seeded_store().await;

        let out = run(mock.clone(), dir.path(), input("uploads/report.pdf"))
            .await
            .expect("download starts");

        let job_id = match out {
            ToolOutput::AssetDownload(out) => {
                assert_eq!(out.key, "uploads/report.pdf");
                assert_eq!(out.dest, dir.path().join("report.pdf").display().to_string());
                assert_eq!(out.backend, "local");
                assert!(!out.waited);
                assert!(matches!(out.state.as_str(), "queued" | "running" | "completed"));
                assert!(out.text.contains("job `"));
                out.job_id
            }
            other => panic!("expected AssetDownload, got {other:?}"),
        };

        let dest = dir.path().join("report.pdf");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !dest.exists() && std::time::Instant::now() < deadline {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        assert_eq!(std::fs::read(&dest).unwrap(), b"%PDF-1.4 payload");
        assert!(!job_id.is_empty());
        assert!(mock.was_called(AssetOperation::DownloadTo));
    }

    #[tokio::test]
    async fn wait_secs_completes_inline_and_reports_the_size() {
        let dir = tempfile::TempDir::new().unwrap();
        let mock = seeded_store().await;

        let mut request = input("uploads/report.pdf");
        request.dest = Some("nested/out.pdf".into());
        request.wait_secs = Some(10);

        let out = run(mock, dir.path(), request).await.expect("download");
        match out {
            ToolOutput::AssetDownload(out) => {
                assert_eq!(out.state, "completed");
                assert!(out.waited);
                assert_eq!(out.bytes_transferred, 16);
                assert_eq!(out.size_bytes, Some(16));
                assert!(out.text.contains("Downloaded `uploads/report.pdf`"));
                assert!(out.error.is_none());
            }
            other => panic!("expected AssetDownload, got {other:?}"),
        }
        assert!(dir.path().join("nested/out.pdf").exists());
    }

    #[tokio::test]
    async fn missing_object_fails_the_job_with_a_projected_error() {
        let dir = tempfile::TempDir::new().unwrap();
        let mock = Arc::new(MockAssetStore::new());

        let mut request = input("uploads/nope.bin");
        request.wait_secs = Some(10);
        let out = run(mock, dir.path(), request).await.expect("job starts");

        match out {
            ToolOutput::AssetDownload(out) => {
                assert_eq!(out.state, "failed");
                assert!(out.error.unwrap().contains("not found"));
            }
            other => panic!("expected AssetDownload, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn bad_key_is_an_argument_error() {
        let dir = tempfile::TempDir::new().unwrap();
        let mock = Arc::new(MockAssetStore::new());
        let err = run(mock.clone(), dir.path(), input("a b/c.bin"))
            .await
            .unwrap_err();
        assert_eq!(err.kind, xai_tool_runtime::ToolErrorKind::InvalidArguments);
        assert!(!mock.was_called(AssetOperation::DownloadTo));
    }

    #[tokio::test]
    async fn capability_fast_fails_before_any_call() {
        let dir = tempfile::TempDir::new().unwrap();
        let mock = Arc::new(
            MockAssetStore::builder()
                .backend(BackendKind::Proxy)
                .capabilities(xai_file_utils::assets::BackendCapabilities {
                    download_to: false,
                    ..xai_file_utils::assets::BackendCapabilities::PROXY
                })
                .build(),
        );
        let err = run(mock.clone(), dir.path(), input("uploads/a.bin"))
            .await
            .unwrap_err();
        assert_eq!(err.details.unwrap()["code"], "asset_unsupported");
        assert!(!mock.was_called(AssetOperation::DownloadTo));
    }

    #[tokio::test]
    async fn empty_job_id_helper_rejects_blank() {
        assert_eq!(super::super::parse_job_id(" job-1 ").unwrap(), "job-1");
        assert!(super::super::parse_job_id("   ").is_err());
    }
}