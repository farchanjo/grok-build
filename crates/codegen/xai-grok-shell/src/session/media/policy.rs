//! Path containment, artifact-ref confinement, and request bounds for
//! media understanding (plan sections 7.2 and 17).
//!
//! The **first** host gate is filesystem/tool permission:
//!
//! - Read-only classification for every `MediaSource::Path`.
//! - Permission is requested BEFORE any path resolution.
//! - The path is canonicalized only AFTER permission approval, and symlink /
//!   workspace escapes are rejected.
//! - `MediaSource::ArtifactRef` is confined to the current session's store by
//!   construction: the store lives under `<session_dir>/assets/media/`, reads
//!   only blobs whose name is a valid BLAKE3 address, and can never reference
//!   an arbitrary session ID.
//!
//! The **second** gate (disclosure consent, [`super::consent`]) runs later,
//! immediately before any provider transmission.

use std::io::{self, Read};
use std::path::{Path, PathBuf};

use xai_grok_tools::media::backend::MediaUnderstandingRequest;
use xai_grok_tools::media::domain::MediaSource;

use super::artifacts::MediaArtifactStore;

/// Hard request bounds enforced by the policy engine.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MediaPolicyLimits {
    pub max_media_bytes: u64,
    pub max_audio_seconds: u64,
    pub max_video_seconds: u64,
    pub max_video_frames: u64,
    /// Instruction length bound (characters).
    pub max_instruction_chars: usize,
    /// Focus list size bound.
    pub max_focus_items: usize,
    /// Media items per request bound.
    pub max_media_items: usize,
}

impl Default for MediaPolicyLimits {
    fn default() -> Self {
        Self {
            max_media_bytes: 256 * 1024 * 1024,
            max_audio_seconds: 1_800,
            max_video_seconds: 900,
            max_video_frames: 32,
            max_instruction_chars: 20_000,
            max_focus_items: 16,
            max_media_items: 32,
        }
    }
}

/// Bounded bytes for one resolved media source.
#[derive(Debug, Clone)]
pub(crate) struct MediaItemBytes {
    pub source: MediaSource,
    pub bytes: Vec<u8>,
    /// Best-effort MIME sniff, `None` when unknown.
    pub mime: Option<String>,
    /// BLAKE3 hex digest of the source bytes.
    pub source_digest: String,
}

/// Policy violation surfaced while resolving media sources.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum PolicyError {
    #[error("permission denied for media path `{0}`")]
    PermissionDenied(String),
    #[error("media path not found: {0}")]
    NotFound(String),
    #[error("media path escapes the workspace: `{0}`")]
    WorkspaceEscape(String),
    #[error("media artifact `{0}` is not present in the current session store")]
    ArtifactNotFound(String),
    #[error("media source exceeds the byte cap: {0}")]
    TooLarge(String),
    #[error("invalid media input: {0}")]
    InvalidInput(String),
}

/// Validate request-level bounds (instruction length, focus list, item count).
pub(crate) fn validate_request(
    request: &MediaUnderstandingRequest,
    limits: &MediaPolicyLimits,
) -> Result<(), PolicyError> {
    if request.media.is_empty() {
        return Err(PolicyError::InvalidInput(
            "request carries no media items".to_string(),
        ));
    }
    if request.media.len() > limits.max_media_items {
        return Err(PolicyError::InvalidInput(format!(
            "request carries {} media items; cap is {}",
            request.media.len(),
            limits.max_media_items
        )));
    }
    if let Some(instruction) = &request.instruction {
        if instruction.chars().count() > limits.max_instruction_chars {
            return Err(PolicyError::InvalidInput(format!(
                "instruction exceeds {} characters",
                limits.max_instruction_chars
            )));
        }
    }
    if request.focus.len() > limits.max_focus_items {
        return Err(PolicyError::InvalidInput(format!(
            "request carries {} focus items; cap is {}",
            request.focus.len(),
            limits.max_focus_items
        )));
    }
    Ok(())
}

/// Resolve one media source to bounded bytes.
///
/// For `MediaSource::Path`: requests read permission, canonicalizes AFTER
/// approval, verifies containment, then reads at most
/// `limits.max_media_bytes` bytes. For `MediaSource::ArtifactRef`: reads the
/// BLAKE3-addressed blob from the current session's store only.
pub(crate) async fn resolve_media_item(
    source: &MediaSource,
    workspace_root: &Path,
    session_dir: &Path,
    permission: Option<&xai_grok_workspace::permission::PermissionHandle>,
    session_id: Option<&str>,
    limits: &MediaPolicyLimits,
) -> Result<MediaItemBytes, PolicyError> {
    match source {
        MediaSource::ArtifactRef { blob_hash } => {
            let store = MediaArtifactStore::open(session_dir)
                .map_err(|e| PolicyError::InvalidInput(format!("artifact store: {e}")))?;
            let bytes = store
                .get_blob(blob_hash)
                .map_err(|e| PolicyError::InvalidInput(format!("artifact read: {e}")))?
                .ok_or_else(|| PolicyError::ArtifactNotFound(blob_hash.clone()))?;
            enforce_byte_cap(&bytes, limits)?;
            Ok(MediaItemBytes {
                source: source.clone(),
                mime: sniff_mime(&bytes),
                source_digest: blob_hash.clone(),
                bytes,
            })
        }
        MediaSource::Path { path } => {
            // PR 7: URL schemes are rejected by construction before any
            // permission request or path resolution. `MediaSource::Path`
            // carries only workspace-relative paths; an `http://`,
            // `https://`, or `file://` prefix is never a valid workspace
            // path and must fail up front.
            if is_url_like(path) {
                return Err(PolicyError::InvalidInput(format!(
                    "URLs are not accepted as media sources: `{path}`"
                )));
            }
            let decision = match permission {
                Some(handle) => {
                    let update = permission_update(path);
                    handle
                        .request(
                            xai_grok_workspace::permission::AccessKind::Read(Some(
                                path.to_string(),
                            )),
                            update,
                            session_id.map(str::to_string),
                            None,
                            None,
                        )
                        .await
                }
                None => xai_grok_workspace::permission::Decision::Allow,
            };
            if !matches!(decision, xai_grok_workspace::permission::Decision::Allow) {
                return Err(PolicyError::PermissionDenied(path.clone()));
            }
            let joined = workspace_root.join(path);
            let canonical = std::fs::canonicalize(&joined)
                .map_err(|e| PolicyError::NotFound(format!("{path}: {e}")))?;
            let canonical_root = std::fs::canonicalize(workspace_root)
                .unwrap_or_else(|_| workspace_root.to_path_buf());
            if !canonical.starts_with(&canonical_root) {
                return Err(PolicyError::WorkspaceEscape(path.clone()));
            }
            let bytes = read_bounded(&canonical, limits)?;
            Ok(MediaItemBytes {
                source: source.clone(),
                mime: sniff_mime(&bytes),
                source_digest: blake3::hash(&bytes).to_hex().to_string(),
                bytes,
            })
        }
    }
}

/// Persist the source bytes in the session store (idempotent) and return the
/// BLAKE3 digest. Callers use this so artifact refs can be reused by later
/// requests (replay/compaction/export).
pub(crate) fn persist_source_blob(session_dir: &Path, item: &MediaItemBytes) -> io::Result<String> {
    let store = MediaArtifactStore::open(session_dir)?;
    let digest = store.put_blob(&item.bytes)?;
    Ok(digest)
}

/// Read at most `limits.max_media_bytes` bytes from `path`.
fn read_bounded(path: &Path, limits: &MediaPolicyLimits) -> Result<Vec<u8>, PolicyError> {
    let max = usize::try_from(limits.max_media_bytes).unwrap_or(usize::MAX);
    let mut file = std::fs::File::open(path)
        .map_err(|e| PolicyError::NotFound(format!("{}: {e}", path.display())))?;
    let mut bytes = Vec::with_capacity(max.min(1024 * 1024));
    file.take((max as u64).saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| PolicyError::NotFound(format!("{}: {e}", path.display())))?;
    enforce_byte_cap(&bytes, limits)?;
    Ok(bytes)
}

fn enforce_byte_cap(bytes: &[u8], limits: &MediaPolicyLimits) -> Result<(), PolicyError> {
    if bytes.len() as u64 > limits.max_media_bytes {
        return Err(PolicyError::TooLarge(format!(
            "{} bytes exceeds the {} byte cap",
            bytes.len(),
            limits.max_media_bytes
        )));
    }
    Ok(())
}

/// Build the ACP tool-call update for a read-permission request.
fn permission_update(path: &str) -> agent_client_protocol::ToolCallUpdate {
    use agent_client_protocol::{ToolCallId, ToolCallUpdateFields, ToolKind};
    agent_client_protocol::ToolCallUpdate::new(
        ToolCallId::new(format!("analyze-media-{}", uuid::Uuid::new_v4())),
        ToolCallUpdateFields::new()
            .title(Some("Analyze a media file".to_string()))
            .kind(Some(ToolKind::Read))
            .raw_input(Some(serde_json::json!({ "path": path }))),
    )
}

/// Whether `path` looks like a URL scheme the tool must reject up front
/// (PR 7): `http://`, `https://`, or `file://`. `MediaSource::Path` is
/// workspace-relative by contract, so these prefixes are never valid inputs.
///
/// Leading whitespace is trimmed before the prefix check: a model can attempt
/// to smuggle a URL past the check with `"   http://..."`, and such an input
/// must still fail up front as `InvalidInput` rather than falling through to
/// path resolution.
fn is_url_like(path: &str) -> bool {
    let trimmed = path.trim_start();
    trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("file://")
}

/// Best-effort MIME sniff: validated image MIME first, then magic-byte
/// inference. `None` when unknown.
fn sniff_mime(bytes: &[u8]) -> Option<String> {
    if let Ok((_, _, mime)) = xai_grok_tools::util::image_validate::validate_image_bytes(bytes) {
        return Some(mime.to_string());
    }
    let kind = infer::get(bytes)?;
    Some(kind.mime_type().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use xai_grok_workspace::permission::types::{
        PatternMode, PermissionConfig, PermissionRule, RuleAction, ToolFilter,
    };
    use xai_grok_workspace::permission::{ClientType, spawn_permission_manager};

    fn limits() -> MediaPolicyLimits {
        MediaPolicyLimits {
            max_media_bytes: 1024,
            ..Default::default()
        }
    }

    fn path_source(path: &str) -> MediaSource {
        MediaSource::Path {
            path: path.to_string(),
        }
    }

    fn request(path: &str, extra: Option<(&str, Vec<String>)>) -> MediaUnderstandingRequest {
        let (instruction, focus) = extra
            .map(|(instruction, focus)| (instruction.to_string(), focus))
            .unwrap_or((String::new(), Vec::new()));
        MediaUnderstandingRequest {
            media: vec![path_source(path)],
            category: xai_grok_tools::media::domain::MediaCategory::Image,
            instruction: (!instruction.is_empty()).then_some(instruction),
            detail: Default::default(),
            focus,
        }
    }

    fn png_bytes(width: u32, height: u32) -> Vec<u8> {
        use image::{ImageBuffer, Rgba};
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(width, height, Rgba([128, 64, 32, 255]));
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        buf
    }

    #[tokio::test]
    async fn media_policy_resolves_path_inside_workspace() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let png = png_bytes(16, 16);
        std::fs::write(root.join("photo.png"), &png).unwrap();
        let item = resolve_media_item(
            &path_source("photo.png"),
            root,
            root,
            Some(&xai_grok_workspace::permission::PermissionHandle::allow_all()),
            Some("session-1"),
            &limits(),
        )
        .await
        .unwrap();
        assert_eq!(item.bytes, png);
        assert_eq!(item.source_digest, blake3::hash(&png).to_hex().to_string());
        assert_eq!(item.mime.as_deref(), Some("image/png"));
    }

    #[tokio::test]
    async fn media_policy_rejects_path_escape_via_parent_components() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let outside =
            std::env::temp_dir().join(format!("media-policy-outside-{}", uuid::Uuid::new_v4()));
        std::fs::write(&outside, b"secret").unwrap();
        let result = resolve_media_item(
            &path_source(&format!(
                "../{}",
                outside.file_name().unwrap().to_string_lossy()
            )),
            root,
            root,
            Some(&xai_grok_workspace::permission::PermissionHandle::allow_all()),
            None,
            &limits(),
        )
        .await;
        let _ = std::fs::remove_file(&outside);
        assert!(
            matches!(result, Err(PolicyError::WorkspaceEscape(_))),
            "escape via `..` must be rejected, got {result:?}"
        );
    }

    #[tokio::test]
    async fn media_policy_rejects_symlink_escape() {
        #[cfg(unix)]
        {
            let workspace = tempfile::tempdir().unwrap();
            let outside = tempfile::tempdir().unwrap();
            std::fs::write(outside.path().join("secret.txt"), b"secret").unwrap();
            let link = workspace.path().join("link");
            std::os::unix::fs::symlink(outside.path().join("secret.txt"), &link).unwrap();
            let result = resolve_media_item(
                &path_source("link"),
                workspace.path(),
                workspace.path(),
                Some(&xai_grok_workspace::permission::PermissionHandle::allow_all()),
                None,
                &limits(),
            )
            .await;
            assert!(
                matches!(result, Err(PolicyError::WorkspaceEscape(_))),
                "symlink escape must be rejected, got {result:?}"
            );
        }
    }

    #[tokio::test]
    async fn media_policy_rejects_missing_and_oversized_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::write(root.join("big.bin"), vec![0u8; 4096]).unwrap();
        let result = resolve_media_item(
            &path_source("big.bin"),
            root,
            root,
            Some(&xai_grok_workspace::permission::PermissionHandle::allow_all()),
            None,
            &limits(),
        )
        .await;
        assert!(matches!(result, Err(PolicyError::TooLarge(_))));

        let result = resolve_media_item(
            &path_source("missing.bin"),
            root,
            root,
            Some(&xai_grok_workspace::permission::PermissionHandle::allow_all()),
            None,
            &limits(),
        )
        .await;
        assert!(matches!(result, Err(PolicyError::NotFound(_))));
    }

    #[tokio::test]
    async fn media_policy_rejects_url_schemes_before_permission() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        // `http://`, `https://`, and `file://` prefixes must fail up front
        // with an InvalidInput error, before any permission request or path
        // resolution (PR 7 content-only tool contract).
        for url in [
            "http://evil.example/x.png",
            "https://evil.example/x.png",
            "file:///etc/passwd",
            "   http://evil.example/x.png",
        ] {
            let result = resolve_media_item(
                &path_source(url),
                root,
                root,
                Some(&xai_grok_workspace::permission::PermissionHandle::allow_all()),
                None,
                &limits(),
            )
            .await;
            assert!(
                matches!(result, Err(PolicyError::InvalidInput(_))),
                "URL-like source `{url}` must be rejected up front, got {result:?}"
            );
        }
    }

    #[tokio::test]
    async fn media_policy_permission_deny_blocks_path() {
        use agent_client_protocol::SessionId;
        use std::sync::Arc;
        use xai_acp_lib::AcpAgentGatewaySender as GatewaySender;
        use xai_grok_paths::AbsPathBuf;

        // The permission manager's actor runs `spawn_local`, so the whole
        // body must execute inside a `task::LocalSet`.
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let tmp = tempfile::tempdir().unwrap();
                let root = tmp.path();
                std::fs::write(root.join("secret.txt"), b"secret").unwrap();
                let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
                let gateway = GatewaySender::new(tx);
                let config = PermissionConfig::new(vec![PermissionRule {
                    action: RuleAction::Deny,
                    tool: ToolFilter::Read,
                    pattern: Some("**/secret.txt".to_string()),
                    pattern_mode: PatternMode::Glob,
                }]);
                let (handle, _ev) = spawn_permission_manager(
                    SessionId::new(Arc::from("media-policy-test")),
                    gateway,
                    AbsPathBuf::new(root.to_path_buf()).unwrap(),
                    ClientType::Generic,
                    Some(config),
                    vec![],
                    vec![],
                    false,
                    None,
                );

                let result = resolve_media_item(
                    &path_source("secret.txt"),
                    root,
                    root,
                    Some(&handle),
                    Some("media-policy-test"),
                    &limits(),
                )
                .await;
                assert!(
                    matches!(result, Err(PolicyError::PermissionDenied(_))),
                    "policy deny must surface as PermissionDenied, got {result:?}"
                );
            })
            .await;
    }

    #[tokio::test]
    async fn media_policy_artifact_ref_confined_to_current_session() {
        let tmp = tempfile::tempdir().unwrap();
        let session_dir = tmp.path().join("session-a");
        let store = MediaArtifactStore::open(&session_dir).unwrap();
        let digest = store.put_blob(b"artifact-bytes").unwrap();

        // Present in the current session store.
        let item = resolve_media_item(
            &MediaSource::ArtifactRef {
                blob_hash: digest.clone(),
            },
            tmp.path(),
            &session_dir,
            None,
            Some("session-a"),
            &limits(),
        )
        .await
        .unwrap();
        assert_eq!(item.bytes, b"artifact-bytes");

        // A different session dir has no such artifact: the ref cannot reach
        // across sessions.
        let other_session = tmp.path().join("session-b");
        let result = resolve_media_item(
            &MediaSource::ArtifactRef { blob_hash: digest },
            tmp.path(),
            &other_session,
            None,
            Some("session-b"),
            &limits(),
        )
        .await;
        assert!(matches!(result, Err(PolicyError::ArtifactNotFound(_))));

        // An invalid BLAKE3 address is rejected before any read.
        let result = resolve_media_item(
            &MediaSource::ArtifactRef {
                blob_hash: "../escape".to_string(),
            },
            tmp.path(),
            &session_dir,
            None,
            None,
            &limits(),
        )
        .await;
        assert!(result.is_err());
    }

    #[test]
    fn media_policy_validate_request_bounds() {
        let limits = limits();

        let ok = request(
            "a.png",
            Some(("short instruction", vec!["text".to_string()])),
        );
        assert!(validate_request(&ok, &limits).is_ok());

        let empty = MediaUnderstandingRequest {
            media: vec![],
            category: xai_grok_tools::media::domain::MediaCategory::Image,
            instruction: None,
            detail: Default::default(),
            focus: vec![],
        };
        assert!(validate_request(&empty, &limits).is_err());

        let too_many_items = MediaUnderstandingRequest {
            media: (0..40).map(|_| path_source("a.png")).collect(),
            category: xai_grok_tools::media::domain::MediaCategory::Image,
            instruction: None,
            detail: Default::default(),
            focus: vec![],
        };
        assert!(validate_request(&too_many_items, &limits).is_err());

        let long_instruction = MediaUnderstandingRequest {
            media: vec![path_source("a.png")],
            category: xai_grok_tools::media::domain::MediaCategory::Image,
            instruction: Some("x".repeat(limits.max_instruction_chars + 1)),
            detail: Default::default(),
            focus: vec![],
        };
        assert!(validate_request(&long_instruction, &limits).is_err());

        let too_many_focus = MediaUnderstandingRequest {
            media: vec![path_source("a.png")],
            category: xai_grok_tools::media::domain::MediaCategory::Image,
            instruction: None,
            detail: Default::default(),
            focus: (0..(limits.max_focus_items + 1))
                .map(|i| format!("f{i}"))
                .collect(),
        };
        assert!(validate_request(&too_many_focus, &limits).is_err());
    }

    #[test]
    fn media_policy_persist_blob_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let session_dir = tmp.path().join("s");
        let item = MediaItemBytes {
            source: path_source("a.png"),
            bytes: b"persist-me".to_vec(),
            mime: None,
            source_digest: blake3::hash(b"persist-me").to_hex().to_string(),
        };
        let digest1 = persist_source_blob(&session_dir, &item).unwrap();
        let digest2 = persist_source_blob(&session_dir, &item).unwrap();
        assert_eq!(digest1, digest2);
        let store = MediaArtifactStore::open(&session_dir).unwrap();
        assert_eq!(store.get_blob(&digest1).unwrap().unwrap(), b"persist-me");
    }
}
