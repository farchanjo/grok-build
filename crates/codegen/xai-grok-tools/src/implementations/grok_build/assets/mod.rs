//! `asset_*` tools — the model-facing surface of the session asset store.
//!
//! Five tools, one backend contract:
//!
//! - [`upload`] — `asset_upload`, stream a local file into the store.
//! - [`share`] — `asset_share`, mint a time-limited read URL.
//! - [`list`] — `asset_list`, page through stored objects.
//! - [`delete`] — `asset_delete`, remove an object (idempotent).
//! - [`set_visibility`] — `asset_set_visibility`, flip private/public.
//!
//! The store itself lives in `xai_file_utils::assets`; the tools never talk to
//! a backend directly. Every tool:
//!
//! 1. resolves the shared store from `Resources` (built once per session, never
//!    per call — contract §9),
//! 2. consults [`BackendCapabilities`] through [`require_capability`] so an
//!    unsupported operation fails *before* any network or filesystem call, and
//! 3. projects every [`AssetError`] through the single [`project_asset_error`]
//!    table, so `details.code` is stable across the five tools.
//!
//! Nothing here blocks: the store's futures are `Send` and the tools `await`
//! them directly. No `block_on`.
//!
//! [`BackendCapabilities`]: xai_file_utils::assets::BackendCapabilities

pub mod delete;
pub mod list;
pub mod set_visibility;
pub mod share;
pub mod upload;

pub use delete::{ASSET_DELETE_TOOL_NAME, AssetDeleteInput, AssetDeleteOutput, AssetDeleteTool};
pub use list::{
    ASSET_LIST_TOOL_NAME, AssetListInput, AssetListItem, AssetListOutput, AssetListTool,
};
pub use set_visibility::{
    ASSET_SET_VISIBILITY_TOOL_NAME, AssetSetVisibilityInput, AssetSetVisibilityOutput,
    AssetSetVisibilityTool,
};
pub use share::{ASSET_SHARE_TOOL_NAME, AssetShareInput, AssetShareOutput, AssetShareTool};
pub use upload::{ASSET_UPLOAD_TOOL_NAME, AssetUploadInput, AssetUploadOutput, AssetUploadTool};

use std::time::Duration;

use xai_file_utils::assets::{
    AssetError, AssetKey, AssetOperation, AssetStore, BackendKind, SharedAssetStore, Visibility,
    validate_ttl,
};
use xai_tool_runtime::{ToolError, ToolErrorKind};

use crate::types::resources::SharedResources;

/// Resolve the session asset store from `Resources`.
///
/// The registry builder inserts it once per session; a miss means the session
/// was finalized without one, which is a wiring bug rather than a user error.
pub(crate) async fn require_store(
    resources: &SharedResources,
) -> Result<SharedAssetStore, ToolError> {
    let res = resources.lock().await;
    Ok(res.require::<SharedAssetStore>()?.clone())
}

/// Fast-fail gate: reject an operation the backend cannot perform before any
/// call is made (contract §7). The adapter would reject it too; doing it here
/// keeps the error identical across all five tools.
pub(crate) fn require_capability(
    store: &dyn AssetStore,
    operation: AssetOperation,
) -> Result<(), ToolError> {
    store
        .capabilities()
        .require(store.backend(), operation)
        .map_err(project_asset_error)
}

/// The one [`AssetError`] → [`ToolError`] projection (contract §5).
///
/// `details.code` carries [`AssetError::code`] verbatim and `details.retryable`
/// mirrors [`AssetError::is_retryable`], so a caller branches on data instead of
/// re-deriving the retry policy from the message. 401 and 403 stay distinct:
/// 401 is [`ToolErrorKind::Unauthorized`] (the repository's attribution shape),
/// 403 is [`ToolErrorKind::PermissionDenied`].
///
/// The message is [`AssetError`]'s `Display`, which already strips URL query
/// strings — a presigned URL never leaks its signature into a tool result.
pub fn project_asset_error(err: AssetError) -> ToolError {
    let code = err.code();
    let retryable = err.is_retryable();
    let kind = match &err {
        AssetError::NotFound { .. } => ToolErrorKind::NotFound,
        AssetError::Unauthorized { .. } => ToolErrorKind::Unauthorized,
        AssetError::AccessDenied { .. } => ToolErrorKind::PermissionDenied,
        AssetError::Unsupported { .. } => ToolErrorKind::NotImplemented,
        AssetError::InvalidKey { .. }
        | AssetError::InvalidContentType { .. }
        | AssetError::TtlOutOfRange { .. }
        | AssetError::ObjectTooLarge { .. } => ToolErrorKind::InvalidArguments,
        AssetError::PreconditionFailed { .. } => ToolErrorKind::Execution,
        AssetError::Transient { .. } => ToolErrorKind::ServiceUnavailable,
        AssetError::Io { .. } => ToolErrorKind::Execution,
        // `AssetError` is `#[non_exhaustive]`; an unknown future variant is a
        // backend failure, never a validation error.
        _ => ToolErrorKind::Execution,
    };

    let mut details = serde_json::Map::new();
    details.insert("code".to_owned(), serde_json::Value::from(code));
    details.insert("retryable".to_owned(), serde_json::Value::from(retryable));
    if let Some(backend) = err.backend() {
        details.insert(
            "backend".to_owned(),
            serde_json::Value::from(backend.as_str()),
        );
    }
    if let AssetError::Unsupported { operation, .. } = &err {
        details.insert(
            "operation".to_owned(),
            serde_json::Value::from(operation.as_str()),
        );
    }

    ToolError::new(kind, err.to_string()).with_details(serde_json::Value::Object(details))
}

/// Parse a `key` argument, turning a bad key into a model-actionable error.
pub(crate) fn parse_key(raw: &str) -> Result<AssetKey, ToolError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(ToolError::invalid_arguments("`key` must not be empty"));
    }
    AssetKey::parse(raw).map_err(|err| {
        ToolError::invalid_arguments(format!(
            "invalid `key` `{raw}`: {err} (allowed: [A-Za-z0-9._~-] and `/`; no `..`, no \
             leading `_meta`)"
        ))
    })
}

/// Parse a `visibility` argument. Absent means [`Visibility::Private`] — public
/// is always explicit (contract I3).
pub(crate) fn parse_visibility(raw: Option<&str>) -> Result<Visibility, ToolError> {
    match raw.map(str::trim).filter(|v| !v.is_empty()) {
        None => Ok(Visibility::default()),
        Some(raw) => Visibility::parse(raw).ok_or_else(|| {
            ToolError::invalid_arguments(format!(
                "invalid `visibility` `{raw}`: expected `private` or `public`"
            ))
        }),
    }
}

/// Validate a caller-supplied presign TTL, defaulting to the configured
/// one-hour window. Out of range is an error, never a silent clamp (contract I4).
pub(crate) fn resolve_ttl(ttl_secs: Option<u64>, default_secs: u64) -> Result<Duration, ToolError> {
    let secs = ttl_secs.unwrap_or(default_secs);
    validate_ttl(Duration::from_secs(secs)).map_err(project_asset_error)
}

/// Honest, secret-free note for a backend that records visibility without
/// enforcing it (contract §8). `None` when the backend does enforce.
pub(crate) fn visibility_hint(backend: BackendKind, enforced: bool) -> Option<String> {
    if enforced {
        return None;
    }
    Some(format!(
        "`{backend}` records visibility without enforcing it: the stored value is a hint, \
         not an access-control guarantee."
    ))
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::path::Path;

    use super::*;
    use crate::types::resources::{Cwd, Resources};

    /// `Resources` holding `store` and a working directory, ready to dispatch a
    /// tool against via [`crate::types::tool_metadata::test_ctx_with_call_id`].
    pub(crate) fn resources_with_store(store: SharedAssetStore, cwd: &Path) -> SharedResources {
        let mut resources = Resources::new();
        resources.insert(store);
        resources.insert(Cwd(cwd.to_path_buf()));
        resources.into_shared()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xai_file_utils::assets::AssetKey;

    /// Every variant, with the projection the contract pins: stable `code()`,
    /// 401/403 kept apart, and `retryable` true for `Transient` only.
    #[test]
    fn error_projection_is_stable_and_complete() {
        let samples = [
            AssetError::NotFound {
                key: "uploads/a.png".into(),
            },
            AssetError::Unauthorized {
                backend: BackendKind::S3,
                detail: "GET https://b.s3.amazonaws.com/k?X-Amz-Signature=x".into(),
            },
            AssetError::AccessDenied {
                backend: BackendKind::Gcs,
                detail: "denied".into(),
            },
            AssetError::unsupported(BackendKind::Proxy, AssetOperation::Delete, "no delete"),
            AssetError::InvalidKey {
                key: "a b".into(),
                reason: "space".into(),
            },
            AssetError::InvalidContentType {
                value: "plain".into(),
                reason: "no slash".into(),
            },
            AssetError::TtlOutOfRange {
                secs: 0,
                min: 1,
                max: 604_800,
            },
            AssetError::ObjectTooLarge {
                key: "uploads/big".into(),
                size: 9,
                max: 1,
            },
            AssetError::PreconditionFailed {
                key: "uploads/a.png".into(),
                detail: "etag".into(),
            },
            AssetError::Transient {
                backend: BackendKind::S3,
                detail: "503".into(),
            },
            AssetError::Io {
                operation: "read",
                detail: "enoent".into(),
            },
        ];

        for err in samples {
            let expected_code = err.code();
            let expected_retryable = err.is_retryable();
            let expected_kind = match &err {
                AssetError::NotFound { .. } => ToolErrorKind::NotFound,
                AssetError::Unauthorized { .. } => ToolErrorKind::Unauthorized,
                AssetError::AccessDenied { .. } => ToolErrorKind::PermissionDenied,
                AssetError::Unsupported { .. } => ToolErrorKind::NotImplemented,
                AssetError::InvalidKey { .. }
                | AssetError::InvalidContentType { .. }
                | AssetError::TtlOutOfRange { .. }
                | AssetError::ObjectTooLarge { .. } => ToolErrorKind::InvalidArguments,
                AssetError::Transient { .. } => ToolErrorKind::ServiceUnavailable,
                AssetError::PreconditionFailed { .. } | AssetError::Io { .. } => {
                    ToolErrorKind::Execution
                }
                _ => ToolErrorKind::Execution,
            };

            let projected = project_asset_error(err.clone());
            assert_eq!(projected.kind, expected_kind, "{err:?}");
            assert_eq!(projected.detail, err.to_string(), "{err:?}");

            let details = projected.details.expect("details always set");
            assert_eq!(details["code"], expected_code);
            assert_eq!(details["retryable"], expected_retryable);
            assert!(
                details["code"].as_str().unwrap().starts_with("asset_"),
                "every code is namespaced: {details}"
            );
        }
    }

    #[test]
    fn transient_is_the_only_retryable_projection() {
        let transient = project_asset_error(AssetError::Transient {
            backend: BackendKind::S3,
            detail: "503".into(),
        });
        assert_eq!(transient.kind, ToolErrorKind::ServiceUnavailable);
        assert_eq!(transient.details.unwrap()["retryable"], true);

        let missing = project_asset_error(AssetError::NotFound { key: "k".into() });
        assert_eq!(missing.details.unwrap()["retryable"], false);
    }

    #[test]
    fn projection_is_secret_free() {
        let err = AssetError::Transient {
            backend: BackendKind::S3,
            detail: "GET https://b.s3.amazonaws.com/k?X-Amz-Signature=deadbeef".into(),
        };
        let projected = project_asset_error(err);
        assert!(
            !projected.detail.contains("X-Amz-Signature"),
            "{}",
            projected.detail
        );
        assert!(projected.detail.contains("https://b.s3.amazonaws.com/k"));
    }

    #[test]
    fn unsupported_projection_carries_the_operation() {
        let projected = project_asset_error(AssetError::unsupported(
            BackendKind::Proxy,
            AssetOperation::List,
            "no list",
        ));
        let details = projected.details.unwrap();
        assert_eq!(details["code"], "asset_unsupported");
        assert_eq!(details["backend"], "proxy");
        assert_eq!(details["operation"], "list");
    }

    #[test]
    fn key_parsing_rejects_unsafe_input() {
        assert_eq!(
            parse_key("s3://bucket/uploads/a.png").unwrap().as_str(),
            "uploads/a.png"
        );
        assert!(parse_key("uploads/../a.png").is_err());
        assert!(parse_key("").is_err());
        assert_eq!(
            parse_key("  uploads/a.png  ").unwrap(),
            AssetKey::parse("uploads/a.png").unwrap()
        );
    }

    #[test]
    fn visibility_defaults_to_private_and_rejects_junk() {
        assert_eq!(parse_visibility(None).unwrap(), Visibility::Private);
        assert_eq!(parse_visibility(Some("")).unwrap(), Visibility::Private);
        assert_eq!(
            parse_visibility(Some("PUBLIC")).unwrap(),
            Visibility::Public
        );
        assert!(parse_visibility(Some("shared")).is_err());
    }

    #[test]
    fn ttl_defaults_and_bounds_are_enforced() {
        assert_eq!(
            resolve_ttl(None, 3_600).unwrap(),
            Duration::from_secs(3_600)
        );
        assert_eq!(resolve_ttl(Some(1), 3_600).unwrap(), Duration::from_secs(1));
        assert_eq!(
            resolve_ttl(Some(604_800), 3_600).unwrap(),
            Duration::from_secs(604_800)
        );

        let zero = resolve_ttl(Some(0), 3_600).unwrap_err();
        assert_eq!(zero.details.unwrap()["code"], "asset_ttl_out_of_range");
        let over = resolve_ttl(Some(604_801), 3_600).unwrap_err();
        assert_eq!(over.kind, ToolErrorKind::InvalidArguments);
    }

    #[test]
    fn visibility_hint_only_fires_when_unenforced() {
        assert!(visibility_hint(BackendKind::S3, true).is_none());
        let hint = visibility_hint(BackendKind::Local, false).expect("hint");
        assert!(hint.contains("local"));
        assert!(hint.contains("not an access-control guarantee"));
    }

    /// The model-facing argument schema is part of the tool contract: pin the
    /// property set so a rename is a test failure, not a silent drift.
    #[test]
    fn tool_arg_schemas_expose_the_documented_fields() {
        fn properties<T: schemars::JsonSchema>() -> Vec<String> {
            let schema = crate::registry::types::generate_schema::<T>();
            let mut names: Vec<String> = schema["properties"]
                .as_object()
                .map(|props| props.keys().cloned().collect())
                .unwrap_or_default();
            names.sort();
            names
        }

        assert_eq!(
            properties::<upload::AssetUploadInput>(),
            ["content_type", "key", "path", "prefix", "visibility"]
        );
        assert_eq!(properties::<share::AssetShareInput>(), ["key", "ttl_secs"]);
        assert_eq!(
            properties::<list::AssetListInput>(),
            ["cursor", "include_meta", "limit", "prefix"]
        );
        assert_eq!(properties::<delete::AssetDeleteInput>(), ["key"]);
        assert_eq!(
            properties::<set_visibility::AssetSetVisibilityInput>(),
            ["key", "visibility"]
        );

        // Only the genuinely required fields are required.
        let upload_schema = crate::registry::types::generate_schema::<upload::AssetUploadInput>();
        assert_eq!(upload_schema["required"], serde_json::json!(["path"]));

        // Printed under `--nocapture` so the registered schemas can be reviewed.
        for (name, schema) in [
            (
                upload::ASSET_UPLOAD_TOOL_NAME,
                crate::registry::types::generate_schema::<upload::AssetUploadInput>(),
            ),
            (
                share::ASSET_SHARE_TOOL_NAME,
                crate::registry::types::generate_schema::<share::AssetShareInput>(),
            ),
            (
                list::ASSET_LIST_TOOL_NAME,
                crate::registry::types::generate_schema::<list::AssetListInput>(),
            ),
            (
                delete::ASSET_DELETE_TOOL_NAME,
                crate::registry::types::generate_schema::<delete::AssetDeleteInput>(),
            ),
            (
                set_visibility::ASSET_SET_VISIBILITY_TOOL_NAME,
                crate::registry::types::generate_schema::<set_visibility::AssetSetVisibilityInput>(
                ),
            ),
        ] {
            println!("{name}: {schema}");
        }
    }

    /// The full local round trip through the real adapter over a tempdir:
    /// upload → share → list → delete, with no mock in the path.
    #[tokio::test]
    async fn local_store_end_to_end_round_trip() {
        use super::test_support::resources_with_store;
        use crate::types::output::ToolOutput;
        use crate::types::tool_metadata::test_ctx_with_call_id;
        use std::sync::Arc;
        use xai_file_utils::assets::{LocalAssetStore, SharedAssetStore};

        let root = tempfile::TempDir::new().unwrap();
        let workspace = tempfile::TempDir::new().unwrap();
        let source = workspace.path().join("quarterly report.txt");
        tokio::fs::write(&source, b"revenue").await.unwrap();

        let store: SharedAssetStore =
            Arc::new(LocalAssetStore::new(root.path()).with_public_base_url("https://cdn.test"));
        let resources = resources_with_store(store.clone(), workspace.path());

        let call = |ctx_resources: SharedResources, call_id: &str| {
            test_ctx_with_call_id(ctx_resources, call_id)
        };

        // 1. upload
        let uploaded = xai_tool_runtime::Tool::run(
            &upload::AssetUploadTool,
            call(resources.clone(), "call-upload"),
            upload::AssetUploadInput {
                path: "quarterly report.txt".into(),
                key: None,
                prefix: Some("reports/".into()),
                content_type: None,
                visibility: Some("public".into()),
            },
        )
        .await
        .expect("upload");
        let key = match uploaded {
            ToolOutput::AssetUpload(out) => {
                assert_eq!(out.key, "reports/quarterly-report.txt");
                assert_eq!(out.size_bytes, 7);
                out.key
            }
            other => panic!("expected AssetUpload, got {other:?}"),
        };
        assert!(root.path().join("reports/quarterly-report.txt").exists());

        // 2. share
        let shared = xai_tool_runtime::Tool::run(
            &share::AssetShareTool,
            call(resources.clone(), "call-share"),
            share::AssetShareInput {
                key: key.clone(),
                ttl_secs: Some(120),
            },
        )
        .await
        .expect("share");
        match shared {
            ToolOutput::AssetShare(out) => {
                assert!(out.url.starts_with("file://"), "{}", out.url);
                assert_eq!(out.expires_in_secs, 120);
                assert!(out.emulated);
                assert_eq!(out.public_url, Some(format!("https://cdn.test/{key}")));
            }
            other => panic!("expected AssetShare, got {other:?}"),
        }

        // 3. list
        let listed = xai_tool_runtime::Tool::run(
            &list::AssetListTool,
            call(resources.clone(), "call-list"),
            list::AssetListInput {
                prefix: Some("reports/".into()),
                limit: None,
                cursor: None,
                include_meta: false,
            },
        )
        .await
        .expect("list");
        match listed {
            ToolOutput::AssetList(out) => {
                assert_eq!(out.items.len(), 1);
                assert_eq!(out.items[0].key, key);
                assert_eq!(out.items[0].visibility, "public");
            }
            other => panic!("expected AssetList, got {other:?}"),
        }

        // 4. delete
        let deleted = xai_tool_runtime::Tool::run(
            &delete::AssetDeleteTool,
            call(resources.clone(), "call-delete"),
            delete::AssetDeleteInput { key: key.clone() },
        )
        .await
        .expect("delete");
        match deleted {
            ToolOutput::AssetDelete(out) => assert!(out.deleted),
            other => panic!("expected AssetDelete, got {other:?}"),
        }
        assert!(!root.path().join("reports/quarterly-report.txt").exists());

        // A second delete is still a success.
        let again = xai_tool_runtime::Tool::run(
            &delete::AssetDeleteTool,
            call(resources, "call-delete-again"),
            delete::AssetDeleteInput { key },
        )
        .await
        .expect("second delete");
        match again {
            ToolOutput::AssetDelete(out) => assert!(!out.deleted),
            other => panic!("expected AssetDelete, got {other:?}"),
        }
    }
}
