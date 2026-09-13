//! Local-directory [`AssetStore`] — the tool-test seam and the total fallback.
//!
//! Layout under the configured root:
//!
//! ```text
//! <root>/<key_prefix>/<key>                       objects
//! <root>/_meta/<key_prefix>/<key>.visibility.json visibility sidecars
//! ```
//!
//! `_meta/` is a reserved top-level namespace (a logical key may not start with
//! it), so sidecars can never collide with an object.
//!
//! Every write is atomic: bytes land in a sibling temp file and are moved into
//! place with `rename`, which is atomic within one filesystem. A reader
//! therefore never observes a half-written object.
//!
//! Nothing here blocks: all filesystem work goes through `tokio::fs`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

use super::error::{AssetError, AssetOperation};
use super::factory::AssetStoreSource;
use super::key::{AssetKey, AssetPrefix, ContentType, RESERVED_META_SEGMENT};
use super::value::{
    AssetMeta, DeleteOutcome, ListCursor, ListPage, ListQuery, PresignMethod, PresignedUrl,
    PutRequest, PutSource, Visibility,
};
use super::{AssetStore, BackendCapabilities, BackendKind, StoreStatus, validate_ttl};

/// Monotonic suffix so two concurrent writers in the same directory never
/// collide on a temp name.
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Suffix appended to a key to name its visibility sidecar.
const VISIBILITY_SIDECAR_SUFFIX: &str = ".visibility.json";

/// File name used for the writability probe in [`AssetStore::health`].
const HEALTH_PROBE_FILE: &str = ".grok-assets-health-probe";

/// Records the visibility of one object on a backend that cannot enforce it.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct VisibilitySidecar {
    key: String,
    visibility: String,
    enforced: bool,
}

/// An [`AssetStore`] backed by a directory on the local filesystem.
#[derive(Debug, Clone)]
pub struct LocalAssetStore {
    root: PathBuf,
    key_prefix: String,
    public_base_url: Option<String>,
    max_object_bytes: Option<u64>,
    request_timeout: Duration,
}

impl LocalAssetStore {
    /// Create a store rooted at `root`. No I/O happens here.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            key_prefix: String::new(),
            public_base_url: None,
            max_object_bytes: None,
            request_timeout: Duration::from_secs(30),
        }
    }

    /// Build from a resolved selection.
    pub fn from_source(source: &AssetStoreSource) -> Self {
        Self {
            root: source.local_root.clone(),
            key_prefix: source.key_prefix.as_str().to_owned(),
            public_base_url: source.public_base_url.clone(),
            max_object_bytes: source.max_object_bytes,
            request_timeout: source.request_timeout,
        }
    }

    pub fn with_key_prefix(mut self, prefix: &str) -> Self {
        self.key_prefix = prefix.to_owned();
        self
    }

    pub fn with_public_base_url(mut self, url: impl Into<String>) -> Self {
        self.public_base_url = Some(url.into());
        self
    }

    pub fn with_max_object_bytes(mut self, max: u64) -> Self {
        self.max_object_bytes = Some(max);
        self
    }

    pub fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Configured public base URL, when any.
    pub fn public_base_url(&self) -> Option<&str> {
        self.public_base_url.as_deref()
    }

    // ---- path mapping ---------------------------------------------------

    /// Physical path of an object, relative to the root.
    fn relative_object_path(&self, key: &AssetKey) -> String {
        format!("{}{key}", self.key_prefix)
    }

    fn object_path(&self, key: &AssetKey) -> PathBuf {
        self.root.join(self.relative_object_path(key))
    }

    /// Path of the visibility sidecar for `key`.
    fn sidecar_path(&self, key: &AssetKey) -> PathBuf {
        self.root.join(RESERVED_META_SEGMENT).join(format!(
            "{}{VISIBILITY_SIDECAR_SUFFIX}",
            self.relative_object_path(key)
        ))
    }

    /// Map a root-relative path back to a key (sidecars keep their `_meta/`
    /// prefix so they round-trip through `get`).
    fn key_from_relative(&self, relative: &str) -> Option<AssetKey> {
        if relative.starts_with(&format!("{RESERVED_META_SEGMENT}/")) {
            return AssetKey::parse_physical(relative).ok();
        }
        let stripped = relative.strip_prefix(&self.key_prefix)?;
        if stripped.is_empty() {
            return None;
        }
        AssetKey::parse(stripped).ok()
    }

    /// The logical key a sidecar annotates, when `relative` is a sidecar.
    fn sidecar_target(&self, relative: &str) -> Option<AssetKey> {
        let stripped = relative
            .strip_prefix(&format!("{RESERVED_META_SEGMENT}/"))?
            .strip_suffix(VISIBILITY_SIDECAR_SUFFIX)?
            .strip_prefix(&self.key_prefix)?;
        if stripped.is_empty() {
            return None;
        }
        AssetKey::parse(stripped).ok()
    }

    // ---- I/O helpers ----------------------------------------------------

    /// Size check shared by both upload paths, before any bytes move.
    fn check_size(&self, key: &AssetKey, size: Option<u64>) -> Result<(), AssetError> {
        let (Some(max), Some(size)) = (self.max_object_bytes, size) else {
            return Ok(());
        };
        if size > max {
            return Err(AssetError::ObjectTooLarge {
                key: key.to_string(),
                size,
                max,
            });
        }
        Ok(())
    }

    async fn ensure_parent(&self, path: &Path) -> Result<(), AssetError> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| AssetError::io("create_dir_all", &e))?;
        }
        Ok(())
    }

    fn temp_path(&self, dest: &Path) -> PathBuf {
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let name = dest
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "object".to_owned());
        dest.with_file_name(format!(".{name}.tmp-{}-{counter}", std::process::id()))
    }

    /// Write `bytes` to `dest` atomically.
    async fn write_bytes_atomic(&self, dest: &Path, bytes: &[u8]) -> Result<(), AssetError> {
        self.ensure_parent(dest).await?;
        let temp = self.temp_path(dest);
        tokio::fs::write(&temp, bytes)
            .await
            .map_err(|e| AssetError::io("write", &e))?;
        match tokio::fs::rename(&temp, dest).await {
            Ok(()) => Ok(()),
            Err(e) => {
                let _ = tokio::fs::remove_file(&temp).await;
                Err(AssetError::io("rename", &e))
            }
        }
    }

    /// Stream `source` to `dest` atomically. Neither side is buffered whole.
    async fn stream_file_atomic(&self, dest: &Path, source: &Path) -> Result<u64, AssetError> {
        self.ensure_parent(dest).await?;
        let temp = self.temp_path(dest);

        let result = async {
            let mut reader = tokio::fs::File::open(source)
                .await
                .map_err(|e| AssetError::io("open", &e))?;
            let mut writer = tokio::fs::File::create(&temp)
                .await
                .map_err(|e| AssetError::io("create", &e))?;
            let copied = tokio::io::copy(&mut reader, &mut writer)
                .await
                .map_err(|e| AssetError::io("copy", &e))?;
            writer
                .flush()
                .await
                .map_err(|e| AssetError::io("flush", &e))?;
            drop(writer);
            tokio::fs::rename(&temp, dest)
                .await
                .map_err(|e| AssetError::io("rename", &e))?;
            Ok::<u64, AssetError>(copied)
        }
        .await;

        if result.is_err() {
            let _ = tokio::fs::remove_file(&temp).await;
        }
        result
    }

    /// Build metadata from what is on disk. `NotFound` when the object is gone.
    async fn meta_for(&self, key: &AssetKey) -> Result<AssetMeta, AssetError> {
        let path = self.object_path(key);
        let metadata = tokio::fs::metadata(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                AssetError::NotFound {
                    key: key.to_string(),
                }
            } else {
                AssetError::io("metadata", &e)
            }
        })?;
        if !metadata.is_file() {
            return Err(AssetError::NotFound {
                key: key.to_string(),
            });
        }
        let visibility = self.read_visibility(key).await;
        let modified_at = metadata.modified().ok().map(|t| {
            let dt: chrono::DateTime<chrono::Utc> = t.into();
            dt.to_rfc3339()
        });
        Ok(AssetMeta {
            key: key.clone(),
            size_bytes: metadata.len(),
            content_type: ContentType::infer_from_path(&path),
            visibility,
            // The local backend never enforces; `public_base_url` only makes
            // the object reachable through the configured public contract.
            visibility_enforced: false,
            backend: BackendKind::Local,
            modified_at,
            etag: None,
        })
    }

    /// Recorded visibility, or `Private` when no sidecar exists.
    async fn read_visibility(&self, key: &AssetKey) -> Visibility {
        let path = self.sidecar_path(key);
        let Ok(raw) = tokio::fs::read_to_string(&path).await else {
            return Visibility::Private;
        };
        let Ok(sidecar) = serde_json::from_str::<VisibilitySidecar>(&raw) else {
            tracing::warn!(
                path = %path.display(),
                "unreadable visibility sidecar; treating the object as private"
            );
            return Visibility::Private;
        };
        Visibility::parse(&sidecar.visibility).unwrap_or(Visibility::Private)
    }

    async fn write_visibility(
        &self,
        key: &AssetKey,
        visibility: Visibility,
    ) -> Result<(), AssetError> {
        let sidecar = VisibilitySidecar {
            key: key.to_string(),
            visibility: visibility.as_str().to_owned(),
            // Honest: recording is not enforcing.
            enforced: false,
        };
        let encoded = serde_json::to_vec(&sidecar).map_err(|e| {
            AssetError::io("serialize_sidecar", &std::io::Error::other(e.to_string()))
        })?;
        self.write_bytes_atomic(&self.sidecar_path(key), &encoded)
            .await
    }

    /// Root-relative paths of every file under `root`, sorted.
    async fn walk(&self) -> Result<Vec<String>, AssetError> {
        let mut found = Vec::new();
        let mut stack = vec![self.root.clone()];
        while let Some(dir) = stack.pop() {
            let mut entries = match tokio::fs::read_dir(&dir).await {
                Ok(entries) => entries,
                // A missing root is an empty store, not an error.
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(AssetError::io("read_dir", &e)),
            };
            while let Some(entry) = entries
                .next_entry()
                .await
                .map_err(|e| AssetError::io("read_dir", &e))?
            {
                let path = entry.path();
                let file_type = entry
                    .file_type()
                    .await
                    .map_err(|e| AssetError::io("file_type", &e))?;
                let is_dir = if file_type.is_symlink() {
                    tokio::fs::metadata(&path)
                        .await
                        .map(|m| m.is_dir())
                        .unwrap_or(false)
                } else {
                    file_type.is_dir()
                };
                if is_dir {
                    stack.push(path);
                } else if let Ok(relative) = path.strip_prefix(&self.root) {
                    let relative = relative.to_string_lossy().replace('\\', "/");
                    // Temp files are always written as `.<name>.tmp-<pid>-<n>`,
                    // so a leading dot on the file name identifies them
                    // without matching against real keys.
                    let name = relative.rsplit('/').next().unwrap_or(&relative);
                    if name.starts_with('.') {
                        continue;
                    }
                    found.push(relative);
                }
            }
        }
        found.sort();
        Ok(found)
    }
}

#[async_trait::async_trait]
impl AssetStore for LocalAssetStore {
    fn backend(&self) -> BackendKind {
        BackendKind::Local
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            // Visibility is only recordable when there is a public contract to
            // point at; without one, `set_visibility` is honestly unsupported.
            set_visibility: self.public_base_url.is_some(),
            ..BackendCapabilities::LOCAL
        }
    }

    async fn put(&self, request: PutRequest) -> Result<AssetMeta, AssetError> {
        self.capabilities()
            .require(BackendKind::Local, AssetOperation::Put)?;
        let dest = self.object_path(&request.key);
        // Buffering entry point: a file source is read whole, then written.
        // Use `put_file` when the payload must not be materialized.
        let bytes = match &request.source {
            PutSource::Bytes(bytes) => bytes.clone(),
            PutSource::File(path) => Bytes::from(
                tokio::fs::read(path)
                    .await
                    .map_err(|e| AssetError::io("read", &e))?,
            ),
        };
        self.check_size(&request.key, Some(bytes.len() as u64))?;
        self.write_bytes_atomic(&dest, &bytes).await?;
        self.write_visibility(&request.key, request.visibility)
            .await?;
        self.meta_for(&request.key).await
    }

    async fn put_file(&self, request: PutRequest) -> Result<AssetMeta, AssetError> {
        self.capabilities()
            .require(BackendKind::Local, AssetOperation::PutFile)?;
        let dest = self.object_path(&request.key);
        match &request.source {
            PutSource::Bytes(bytes) => {
                self.check_size(&request.key, Some(bytes.len() as u64))?;
                self.write_bytes_atomic(&dest, bytes).await?;
            }
            PutSource::File(path) => {
                let len = tokio::fs::metadata(path)
                    .await
                    .map(|m| m.len())
                    .map_err(|e| AssetError::io("metadata", &e))?;
                self.check_size(&request.key, Some(len))?;
                self.stream_file_atomic(&dest, path).await?;
            }
        }
        // The requested visibility is recorded, not enforced. Recording on
        // every write (rather than only for `public`) keeps a re-upload with
        // the default from leaving a stale `public` sidecar behind.
        self.write_visibility(&request.key, request.visibility)
            .await?;
        self.meta_for(&request.key).await
    }

    async fn get(&self, key: &AssetKey) -> Result<Bytes, AssetError> {
        self.capabilities()
            .require(BackendKind::Local, AssetOperation::Get)?;
        let path = self.object_path(key);
        match tokio::fs::read(&path).await {
            Ok(bytes) => Ok(Bytes::from(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(AssetError::NotFound {
                key: key.to_string(),
            }),
            Err(e) => Err(AssetError::io("read", &e)),
        }
    }

    async fn download_to(&self, key: &AssetKey, dest: &Path) -> Result<AssetMeta, AssetError> {
        self.capabilities()
            .require(BackendKind::Local, AssetOperation::DownloadTo)?;
        let source = self.object_path(key);
        if !self.exists(key).await? {
            return Err(AssetError::NotFound {
                key: key.to_string(),
            });
        }
        self.stream_file_atomic(dest, &source).await?;
        self.meta_for(key).await
    }

    async fn exists(&self, key: &AssetKey) -> Result<bool, AssetError> {
        self.capabilities()
            .require(BackendKind::Local, AssetOperation::Exists)?;
        match tokio::fs::metadata(self.object_path(key)).await {
            Ok(meta) => Ok(meta.is_file()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(AssetError::io("metadata", &e)),
        }
    }

    async fn delete(&self, key: &AssetKey) -> Result<DeleteOutcome, AssetError> {
        self.capabilities()
            .require(BackendKind::Local, AssetOperation::Delete)?;
        let outcome = match tokio::fs::remove_file(self.object_path(key)).await {
            Ok(()) => DeleteOutcome::Deleted,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => DeleteOutcome::NotFound,
            Err(e) => return Err(AssetError::io("remove_file", &e)),
        };
        // A dangling sidecar is noise; drop it best-effort.
        let _ = tokio::fs::remove_file(self.sidecar_path(key)).await;
        Ok(outcome)
    }

    async fn list(&self, query: ListQuery) -> Result<ListPage, AssetError> {
        self.capabilities()
            .require(BackendKind::Local, AssetOperation::List)?;
        query.validate()?;

        let prefix = query.prefix.as_str();
        let cursor = query.cursor.as_ref().map(ListCursor::as_str);
        let mut items = Vec::new();
        let mut last_relative: Option<String> = None;

        for relative in self.walk().await? {
            let is_meta = relative.starts_with(&format!("{RESERVED_META_SEGMENT}/"));
            if is_meta && !query.include_meta {
                continue;
            }
            // Sidecars are matched on the logical key they annotate, so a
            // prefix query stays meaningful with `include_meta`.
            let match_target = if is_meta {
                relative
                    .strip_prefix(&format!("{RESERVED_META_SEGMENT}/"))
                    .and_then(|r| r.strip_suffix(VISIBILITY_SIDECAR_SUFFIX))
                    .unwrap_or(&relative)
                    .strip_prefix(&self.key_prefix)
                    .unwrap_or(relative.as_str())
            } else {
                relative.as_str()
            };
            if !prefix.is_empty() && !match_target.starts_with(prefix) {
                continue;
            }
            if let Some(cursor) = cursor
                && relative.as_str() <= cursor
            {
                continue;
            }

            let Some(key) = self.key_from_relative(&relative) else {
                continue;
            };
            let path = self.root.join(&relative);
            let Ok(metadata) = tokio::fs::metadata(&path).await else {
                continue;
            };
            let visibility = match self.sidecar_target(&relative) {
                Some(target) => self.read_visibility(&target).await,
                None => self.read_visibility(&key).await,
            };
            last_relative = Some(relative.clone());
            items.push(AssetMeta {
                key,
                size_bytes: metadata.len(),
                content_type: ContentType::infer_from_path(&path),
                visibility,
                visibility_enforced: false,
                backend: BackendKind::Local,
                modified_at: metadata.modified().ok().map(|t| {
                    let dt: chrono::DateTime<chrono::Utc> = t.into();
                    dt.to_rfc3339()
                }),
                etag: None,
            });
            if items.len() as u32 >= query.limit {
                break;
            }
        }

        // A full page is reported as truncated: whether more exists is only
        // knowable by asking again, and a stale `next_cursor` is cheap.
        let next_cursor = if items.len() as u32 >= query.limit {
            last_relative.map(ListCursor::new)
        } else {
            None
        };

        Ok(ListPage {
            items,
            truncated: next_cursor.is_some(),
            next_cursor,
        })
    }

    async fn presign_get(&self, key: &AssetKey, ttl: Duration) -> Result<PresignedUrl, AssetError> {
        self.capabilities()
            .require(BackendKind::Local, AssetOperation::PresignGet)?;
        let ttl = validate_ttl(ttl)?;
        // `tokio::fs::canonicalize` is banned repo-wide (it returns `\\?\`
        // verbatim paths on Windows), so canonicalize through `dunce` on the
        // blocking pool. A missing object still presigns; only the absolute
        // form is best-effort.
        let joined = self.object_path(key);
        let canonical = {
            let path = joined.clone();
            tokio::task::spawn_blocking(move || dunce::canonicalize(&path))
                .await
                .ok()
                .and_then(Result::ok)
        };
        let absolute = canonical.unwrap_or(joined);
        Ok(PresignedUrl::new(
            key.clone(),
            format!("file://{}", absolute.display()),
            PresignMethod::Get,
            ttl,
            // No signature: a `file://` URL carries no authorization.
            true,
        ))
    }

    async fn presign_put(
        &self,
        key: &AssetKey,
        _content_type: &ContentType,
        _ttl: Duration,
    ) -> Result<PresignedUrl, AssetError> {
        Err(self
            .capabilities()
            .require(BackendKind::Local, AssetOperation::PresignPut)
            .err()
            .unwrap_or_else(|| {
                AssetError::unsupported(
                    BackendKind::Local,
                    AssetOperation::PresignPut,
                    "a local path needs no signature",
                )
            }))
    }

    async fn set_visibility(
        &self,
        key: &AssetKey,
        visibility: Visibility,
    ) -> Result<AssetMeta, AssetError> {
        self.capabilities()
            .require(BackendKind::Local, AssetOperation::SetVisibility)?;
        if !self.exists(key).await? {
            return Err(AssetError::NotFound {
                key: key.to_string(),
            });
        }
        self.write_visibility(key, visibility).await?;
        self.meta_for(key).await
    }

    fn public_url(&self, key: &AssetKey) -> Option<String> {
        let base = self.public_base_url.as_deref()?;
        Some(format!(
            "{}/{}",
            base.trim_end_matches('/'),
            self.relative_object_path(key)
        ))
    }

    async fn health(&self) -> Result<StoreStatus, AssetError> {
        if let Err(e) = tokio::fs::metadata(&self.root).await {
            return Ok(StoreStatus::degraded(
                BackendKind::Local,
                false,
                format!("{} is not readable: {e}", self.root.display()),
            ));
        }
        let probe = self.root.join(HEALTH_PROBE_FILE);
        if let Err(e) = tokio::fs::write(&probe, b"ok").await {
            return Ok(StoreStatus::degraded(
                BackendKind::Local,
                true,
                format!("{} is not writable: {e}", self.root.display()),
            ));
        }
        let _ = tokio::fs::remove_file(&probe).await;
        Ok(StoreStatus::healthy(BackendKind::Local))
    }
}

impl LocalAssetStore {
    /// Effective request timeout, exposed for adapters that need it.
    pub fn request_timeout(&self) -> Duration {
        self.request_timeout
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::AssetKey;

    fn key(s: &str) -> AssetKey {
        AssetKey::parse(s).unwrap()
    }

    fn store(dir: &tempfile::TempDir) -> LocalAssetStore {
        LocalAssetStore::new(dir.path())
    }

    #[tokio::test]
    async fn put_get_round_trip_is_private_by_default() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);

        let meta = store
            .put(PutRequest::from_bytes(
                key("uploads/a.txt"),
                b"hello".to_vec(),
                ContentType::parse("text/plain").unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(meta.size_bytes, 5);
        assert_eq!(meta.visibility, Visibility::Private);
        assert!(!meta.visibility_enforced);
        assert_eq!(meta.backend, BackendKind::Local);
        assert!(meta.modified_at.is_some());

        let bytes = store.get(&key("uploads/a.txt")).await.unwrap();
        assert_eq!(&bytes[..], b"hello");

        // The object really is at <root>/uploads/a.txt.
        assert!(dir.path().join("uploads/a.txt").is_file());
    }

    #[tokio::test]
    async fn put_is_atomic_and_leaves_no_temp_files() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);
        store
            .put(PutRequest::from_bytes(
                key("uploads/a.bin"),
                vec![7u8; 4096],
                ContentType::default(),
            ))
            .await
            .unwrap();

        let entries: Vec<String> = std::fs::read_dir(dir.path().join("uploads"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(entries, vec!["a.bin".to_owned()]);
    }

    #[tokio::test]
    async fn put_file_streams_and_download_to_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);

        let source = dir.path().join("source.bin");
        let payload: Vec<u8> = (0..(512 * 1024)).map(|i| (i % 251) as u8).collect();
        std::fs::write(&source, &payload).unwrap();

        let meta = store
            .put_file(PutRequest::from_file(
                key("uploads/big.bin"),
                source.clone(),
                ContentType::default(),
            ))
            .await
            .unwrap();
        assert_eq!(meta.size_bytes, payload.len() as u64);

        let dest = dir.path().join("nested/deeper/out.bin");
        let meta = store
            .download_to(&key("uploads/big.bin"), &dest)
            .await
            .unwrap();
        assert_eq!(meta.size_bytes, payload.len() as u64);
        assert_eq!(std::fs::read(&dest).unwrap(), payload);

        let missing = dir.path().join("nested/missing.bin");
        assert!(
            store
                .download_to(&key("uploads/nope.bin"), &missing)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn put_rejects_objects_over_max_object_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir).with_max_object_bytes(4);
        let err = store
            .put(PutRequest::from_bytes(
                key("uploads/a.bin"),
                vec![0u8; 5],
                ContentType::default(),
            ))
            .await
            .unwrap_err();
        assert_eq!(err.code(), "asset_object_too_large");
        assert!(matches!(
            err,
            AssetError::ObjectTooLarge {
                size: 5,
                max: 4,
                ..
            }
        ));

        store
            .put(PutRequest::from_bytes(
                key("uploads/a.bin"),
                vec![0u8; 4],
                ContentType::default(),
            ))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn get_and_exists_report_missing_objects() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);
        assert!(!store.exists(&key("uploads/none.txt")).await.unwrap());
        let err = store.get(&key("uploads/none.txt")).await.unwrap_err();
        assert_eq!(err.code(), "asset_not_found");
        assert!(matches!(err, AssetError::NotFound { .. }));
    }

    #[tokio::test]
    async fn delete_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);
        store
            .put(PutRequest::from_bytes(
                key("uploads/a.txt"),
                b"x".to_vec(),
                ContentType::default(),
            ))
            .await
            .unwrap();

        assert_eq!(
            store.delete(&key("uploads/a.txt")).await.unwrap(),
            DeleteOutcome::Deleted
        );
        assert_eq!(
            store.delete(&key("uploads/a.txt")).await.unwrap(),
            DeleteOutcome::NotFound
        );
        assert!(!store.exists(&key("uploads/a.txt")).await.unwrap());
    }

    #[tokio::test]
    async fn list_paginates_with_a_stable_cursor_and_hides_meta() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);
        for name in ["a", "b", "c", "d", "e"] {
            store
                .put(PutRequest::from_bytes(
                    key(&format!("uploads/{name}.txt")),
                    name.as_bytes().to_vec(),
                    ContentType::default(),
                ))
                .await
                .unwrap();
        }
        store
            .put(PutRequest::from_bytes(
                key("other/z.txt"),
                b"z".to_vec(),
                ContentType::default(),
            ))
            .await
            .unwrap();

        let page = store
            .list(ListQuery::new(AssetPrefix::parse("uploads/").unwrap()).with_limit(2))
            .await
            .unwrap();
        assert_eq!(page.items.len(), 2);
        assert!(page.truncated);
        assert_eq!(page.items[0].key.as_str(), "uploads/a.txt");
        assert_eq!(page.items[1].key.as_str(), "uploads/b.txt");

        let cursor = page.next_cursor.clone().expect("cursor");
        let next = store
            .list(
                ListQuery::new(AssetPrefix::parse("uploads/").unwrap())
                    .with_limit(2)
                    .with_cursor(cursor.clone()),
            )
            .await
            .unwrap();
        assert_eq!(next.items[0].key.as_str(), "uploads/c.txt");
        assert_eq!(next.items[1].key.as_str(), "uploads/d.txt");

        let rest = store
            .list(
                ListQuery::new(AssetPrefix::parse("uploads/").unwrap())
                    .with_limit(10)
                    .with_cursor(next.next_cursor.clone().expect("cursor")),
            )
            .await
            .unwrap();
        assert_eq!(rest.items.len(), 1);
        assert_eq!(rest.items[0].key.as_str(), "uploads/e.txt");
        assert!(!rest.truncated);

        // Unprefixed list sees `other/` too.
        let all = store
            .list(ListQuery::default().with_limit(100))
            .await
            .unwrap();
        assert_eq!(all.items.len(), 6);

        // A bad limit fails before touching the filesystem.
        assert!(
            store
                .list(ListQuery::default().with_limit(0))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn list_can_include_meta_sidecars() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir).with_public_base_url("https://cdn.test");
        store
            .put(PutRequest::from_bytes(
                key("uploads/a.txt"),
                b"a".to_vec(),
                ContentType::default(),
            ))
            .await
            .unwrap();
        store
            .set_visibility(&key("uploads/a.txt"), Visibility::Public)
            .await
            .unwrap();

        let hidden = store
            .list(ListQuery::new(AssetPrefix::parse("uploads/").unwrap()))
            .await
            .unwrap();
        assert_eq!(hidden.items.len(), 1);

        let shown = store
            .list(ListQuery::new(AssetPrefix::parse("uploads/").unwrap()).with_include_meta(true))
            .await
            .unwrap();
        assert_eq!(shown.items.len(), 2);
        assert!(shown.items.iter().any(|m| m.key.is_physical_meta()));
    }

    #[tokio::test]
    async fn presign_get_returns_a_file_url_and_never_signs() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);
        store
            .put(PutRequest::from_bytes(
                key("uploads/a.txt"),
                b"a".to_vec(),
                ContentType::default(),
            ))
            .await
            .unwrap();

        let url = store
            .presign_get(&key("uploads/a.txt"), Duration::from_secs(60))
            .await
            .unwrap();
        assert!(url.url.starts_with("file://"), "{}", url.url);
        assert!(url.url.ends_with("uploads/a.txt"), "{}", url.url);
        assert!(url.emulated);
        assert_eq!(url.method, PresignMethod::Get);
        assert!(!url.is_expired());

        // TTL is validated, never clamped.
        assert_eq!(
            store
                .presign_get(&key("uploads/a.txt"), Duration::from_secs(0))
                .await
                .unwrap_err()
                .code(),
            "asset_ttl_out_of_range"
        );
    }

    #[tokio::test]
    async fn presign_put_is_unsupported_and_capability_driven() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);
        assert!(!store.capabilities().presign_put);
        let err = store
            .presign_put(
                &key("uploads/a.txt"),
                &ContentType::default(),
                Duration::from_secs(60),
            )
            .await
            .unwrap_err();
        assert_eq!(err.code(), "asset_unsupported");
        assert!(matches!(
            err,
            AssetError::Unsupported {
                backend: BackendKind::Local,
                operation: AssetOperation::PresignPut,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn set_visibility_records_a_sidecar_only_with_a_public_base_url() {
        let dir = tempfile::tempdir().unwrap();

        let without = store(&dir);
        assert!(!without.capabilities().set_visibility);
        without
            .put(PutRequest::from_bytes(
                key("uploads/a.txt"),
                b"a".to_vec(),
                ContentType::default(),
            ))
            .await
            .unwrap();
        let err = without
            .set_visibility(&key("uploads/a.txt"), Visibility::Public)
            .await
            .unwrap_err();
        assert_eq!(err.code(), "asset_unsupported");

        let with = store(&dir).with_public_base_url("https://cdn.test/");
        assert!(with.capabilities().set_visibility);
        let meta = with
            .set_visibility(&key("uploads/a.txt"), Visibility::Public)
            .await
            .unwrap();
        assert_eq!(meta.visibility, Visibility::Public);
        // Recorded, not enforced.
        assert!(!meta.visibility_enforced);
        assert!(
            dir.path()
                .join("_meta/uploads/a.txt.visibility.json")
                .is_file()
        );

        assert_eq!(
            with.public_url(&key("uploads/a.txt")).as_deref(),
            Some("https://cdn.test/uploads/a.txt")
        );
        assert!(without.public_url(&key("uploads/a.txt")).is_none());

        // Setting visibility on a missing object is NotFound.
        assert!(
            with.set_visibility(&key("uploads/none.txt"), Visibility::Public)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn key_prefix_namespaces_objects_and_sidecars() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir).with_key_prefix("uploads/");
        assert!(store.public_url(&key("a.txt")).is_none());
        store
            .put(PutRequest::from_bytes(
                key("a.txt"),
                b"a".to_vec(),
                ContentType::default(),
            ))
            .await
            .unwrap();
        assert!(dir.path().join("uploads/a.txt").is_file());
        assert!(store.exists(&key("a.txt")).await.unwrap());

        let listed = store.list(ListQuery::default()).await.unwrap();
        assert_eq!(listed.items[0].key.as_str(), "a.txt");
    }

    #[tokio::test]
    async fn health_probes_read_and_write() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);
        let status = store.health().await.unwrap();
        assert!(status.is_healthy());
        assert_eq!(status.backend, BackendKind::Local);
        assert!(!dir.path().join(HEALTH_PROBE_FILE).exists());

        let missing = LocalAssetStore::new(dir.path().join("nope"));
        let status = missing.health().await.unwrap();
        assert!(!status.is_healthy());
        assert!(!status.reachable);
        assert!(status.detail.unwrap().contains("not readable"));
    }

    #[tokio::test]
    async fn store_is_send_and_shareable() {
        let dir = tempfile::tempdir().unwrap();
        let store: crate::assets::SharedAssetStore = std::sync::Arc::new(store(&dir));
        let handle = tokio::spawn(async move {
            store
                .put(PutRequest::from_bytes(
                    key("uploads/task.txt"),
                    b"t".to_vec(),
                    ContentType::default(),
                ))
                .await
                .unwrap()
                .size_bytes
        });
        assert_eq!(handle.await.unwrap(), 1);
    }

    #[tokio::test]
    async fn from_source_maps_every_configured_field() {
        use crate::assets::factory::{AssetStoreSource, SelectionOrigin};

        let dir = tempfile::tempdir().unwrap();
        let source = AssetStoreSource {
            kind: BackendKind::Local,
            origin: SelectionOrigin::LocalFallback,
            bucket: None,
            region: None,
            endpoint_url: None,
            credentials_file: None,
            env_key: None,
            public_base_url: Some("https://cdn.test".into()),
            default_visibility: Visibility::Private,
            default_ttl: Duration::from_secs(60),
            local_root: dir.path().to_path_buf(),
            proxy_base_url: None,
            max_object_bytes: Some(8),
            request_timeout: Duration::from_secs(3),
            key_prefix: crate::assets::AssetPrefix::parse("uploads/").unwrap(),
        };
        let store = LocalAssetStore::from_source(&source);
        assert_eq!(store.root(), dir.path());
        assert_eq!(store.public_base_url(), Some("https://cdn.test"));
        assert_eq!(store.request_timeout(), Duration::from_secs(3));
        assert!(store.capabilities().set_visibility);
        assert!(
            store
                .put(PutRequest::from_bytes(
                    key("a.bin"),
                    vec![0u8; 9],
                    ContentType::default(),
                ))
                .await
                .is_err()
        );
    }
}
