//! GCS [`AssetStore`].
//!
//! Built on the shared client builder in [`crate::gcs`].
//!
//! Two GCS-specific behaviours, both reported honestly rather than faked:
//!
//! - **No per-object ACL here.** `set_visibility` is `Unsupported` unless a
//!   `public_base_url` gives the object a public contract, in which case the
//!   intent is recorded in the `_meta/<key>.visibility.json` sidecar.
//!   `visibility_enforced` is therefore always `false`.
//! - **Presigning is emulated.** `gcloud-storage` signs a V4 URL locally, which
//!   needs a service-account private key. With ADC-only credentials the key is
//!   not reachable from here, so `presign_*` returns `Unsupported` with a clear
//!   reason instead of a URL that would 403.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use bytes::Bytes;
use futures::StreamExt;
use gcloud_storage::client::{Client, ClientConfig};
use gcloud_storage::http::objects::delete::DeleteObjectRequest;
use gcloud_storage::http::objects::download::Range;
use gcloud_storage::http::objects::get::GetObjectRequest;
use gcloud_storage::http::objects::list::ListObjectsRequest;
use gcloud_storage::http::objects::upload::{Media, UploadObjectRequest, UploadType};
use gcloud_storage::sign::{SignBy, SignedURLMethod, SignedURLOptions};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;

use super::error::{AssetError, AssetOperation};
use super::factory::AssetStoreSource;
use super::key::{AssetKey, ContentType, RESERVED_META_SEGMENT};
use super::progress::ProgressHandle;
use super::value::{
    AssetMeta, DeleteOutcome, ListCursor, ListPage, ListQuery, PresignMethod, PresignedUrl,
    PutRequest, PutSource, Visibility,
};
use super::{AssetStore, BackendCapabilities, BackendKind, StoreStatus, validate_ttl};

/// Suffix appended to a key to name its visibility sidecar.
const VISIBILITY_SIDECAR_SUFFIX: &str = ".visibility.json";

/// Key (under the reserved namespace) used for the writability probe.
const HEALTH_PROBE_KEY: &str = "_meta/.health-probe";

/// Monotonic suffix so two concurrent downloads do not collide on a temp name.
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Records the visibility of one object on a backend that cannot enforce it.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct VisibilitySidecar {
    key: String,
    visibility: String,
    enforced: bool,
}

/// The subset of a service-account JSON this adapter needs to sign.
#[derive(Debug, Clone, Deserialize)]
struct ServiceAccountKey {
    client_email: String,
    private_key: String,
}

/// How a GCS call failed, in the shape the adapter branches on.
#[derive(Debug, Clone, PartialEq, Eq)]
enum GcsFailure {
    NotFound,
    Unauthorized,
    AccessDenied,
    Transient(String),
    Other(String),
}

fn classify(err: &gcloud_storage::http::Error) -> GcsFailure {
    use gcloud_storage::http::Error;
    match err {
        Error::Response(response) => match response.code {
            404 => GcsFailure::NotFound,
            401 => GcsFailure::Unauthorized,
            403 => GcsFailure::AccessDenied,
            code if (500..600).contains(&code) || code == 408 || code == 429 => {
                GcsFailure::Transient(format!("HTTP {code}"))
            }
            code => GcsFailure::Other(format!("HTTP {code}")),
        },
        Error::HttpClient(e) if e.is_timeout() => GcsFailure::Transient("request timed out".into()),
        Error::HttpClient(e) if e.is_connect() => {
            GcsFailure::Transient(format!("connect failed: {e}"))
        }
        other => GcsFailure::Transient(other.to_string()),
    }
}

/// A resolved service-account signing pair.
struct GcsSigning {
    google_access_id: String,
    sign_by: SignBy,
}

/// An [`AssetStore`] backed by a Google Cloud Storage bucket.
pub struct GcsAssetStore {
    client: Client,
    bucket: String,
    public_base_url: Option<String>,
    key_prefix: String,
    max_object_bytes: Option<u64>,
    request_timeout: Duration,
    /// Present only with a service-account key; required for presigning.
    signing: Option<GcsSigning>,
}

impl std::fmt::Debug for GcsAssetStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GcsAssetStore")
            .field("bucket", &self.bucket)
            .field("key_prefix", &self.key_prefix)
            .field("has_signing_key", &self.signing.is_some())
            .finish()
    }
}

impl GcsAssetStore {
    /// Build from a resolved selection, authenticating through the service
    /// account key or ADC.
    pub async fn from_source(source: &AssetStoreSource) -> Result<Self, AssetError> {
        let key_json = resolve_service_account_key(source).await?;
        let client = crate::gcs::build_gcs_client(key_json.as_deref())
            .await
            // `{:#}` keeps the whole anyhow chain, not just the outer context.
            .map_err(|e| {
                AssetError::io("build_gcs_client", &std::io::Error::other(format!("{e:#}")))
            })?;
        Self::from_source_with_client(source, key_json.as_deref(), client).await
    }

    /// Build with an injected client. The credential *file* is still read for
    /// the signing key, so this path exercises the same signing resolution
    /// without touching the ambient credential chain.
    pub async fn from_source_with_client(
        source: &AssetStoreSource,
        service_account_key: Option<&str>,
        client: Client,
    ) -> Result<Self, AssetError> {
        let bucket = source
            .bucket
            .clone()
            .ok_or_else(|| AssetError::InvalidKey {
                key: "assets.bucket".to_owned(),
                reason: "the gcs backend needs a bucket".to_owned(),
            })?;

        let signing = service_account_key.and_then(|json| {
            serde_json::from_str::<ServiceAccountKey>(json)
                .map(|key| GcsSigning {
                    google_access_id: key.client_email,
                    sign_by: SignBy::PrivateKey(key.private_key.into_bytes()),
                })
                .map_err(|e| {
                    tracing::warn!(
                        error = %e,
                        "assets: service account key is not JSON with \
                         `client_email`/`private_key`; presigning stays Unsupported"
                    );
                })
                .ok()
        });

        Ok(Self {
            client,
            bucket,
            public_base_url: source.public_base_url.clone(),
            key_prefix: source.key_prefix.as_str().to_owned(),
            max_object_bytes: source.max_object_bytes,
            request_timeout: source.request_timeout,
            signing,
        })
    }

    /// Inject a pre-built client (tests point this at a mock endpoint).
    pub fn with_client(mut self, client: Client) -> Self {
        self.client = client;
        self
    }

    pub fn with_public_base_url(mut self, url: impl Into<String>) -> Self {
        self.public_base_url = Some(url.into());
        self
    }

    pub fn bucket(&self) -> &str {
        &self.bucket
    }

    pub fn request_timeout(&self) -> Duration {
        self.request_timeout
    }

    /// Whether a signing key is available for presigning.
    pub fn can_presign(&self) -> bool {
        self.signing.is_some()
    }

    // ---- path mapping ---------------------------------------------------

    fn physical(&self, key: &AssetKey) -> String {
        format!("{}{key}", self.key_prefix)
    }

    fn sidecar(&self, key: &AssetKey) -> String {
        format!(
            "{RESERVED_META_SEGMENT}/{}{VISIBILITY_SIDECAR_SUFFIX}",
            self.physical(key)
        )
    }

    fn logical(&self, physical: &str) -> Option<AssetKey> {
        let stripped = physical.strip_prefix(&self.key_prefix)?;
        if stripped.is_empty() {
            return None;
        }
        AssetKey::parse(stripped).ok()
    }

    // ---- error plumbing --------------------------------------------------

    fn map_failure(
        &self,
        operation: AssetOperation,
        failure: GcsFailure,
        key: Option<&AssetKey>,
    ) -> AssetError {
        match failure {
            GcsFailure::NotFound => AssetError::NotFound {
                key: key.map(AssetKey::to_string).unwrap_or_default(),
            },
            GcsFailure::Unauthorized => AssetError::Unauthorized {
                backend: BackendKind::Gcs,
                detail: format!("{operation} rejected the credentials"),
            },
            GcsFailure::AccessDenied => AssetError::AccessDenied {
                backend: BackendKind::Gcs,
                detail: format!("{operation} is not permitted"),
            },
            GcsFailure::Transient(detail) | GcsFailure::Other(detail) => AssetError::Transient {
                backend: BackendKind::Gcs,
                detail,
            },
        }
    }

    async fn run_raw<T, F>(&self, fut: F) -> Result<T, GcsFailure>
    where
        F: std::future::Future<Output = Result<T, GcsFailure>>,
    {
        match tokio::time::timeout(self.request_timeout, fut).await {
            Ok(result) => result,
            Err(_) => Err(GcsFailure::Transient(format!(
                "timed out after {}s",
                self.request_timeout.as_secs()
            ))),
        }
    }

    async fn run<T, F>(
        &self,
        operation: AssetOperation,
        key: Option<&AssetKey>,
        fut: F,
    ) -> Result<T, AssetError>
    where
        F: std::future::Future<Output = Result<T, GcsFailure>>,
    {
        self.run_raw(fut)
            .await
            .map_err(|failure| self.map_failure(operation, failure, key))
    }

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

    // ---- object helpers --------------------------------------------------

    async fn put_bytes(
        &self,
        object: &str,
        bytes: Vec<u8>,
        content_type: &str,
    ) -> Result<(), GcsFailure> {
        let mut media = Media::new(object.to_owned());
        media.content_type = content_type.to_owned().into();
        media.content_length = Some(bytes.len() as u64);
        let request = UploadObjectRequest {
            bucket: self.bucket.clone(),
            ..Default::default()
        };
        self.client
            .upload_object(&request, bytes, &UploadType::Simple(media))
            .await
            .map(|_| ())
            .map_err(|e| classify(&e))
    }

    async fn get_bytes(&self, object: &str) -> Result<Vec<u8>, GcsFailure> {
        let request = GetObjectRequest {
            bucket: self.bucket.clone(),
            object: object.to_owned(),
            ..Default::default()
        };
        self.client
            .download_object(&request, &Range::default())
            .await
            .map_err(|e| classify(&e))
    }

    async fn head(
        &self,
        object: &str,
    ) -> Result<gcloud_storage::http::objects::Object, GcsFailure> {
        let request = GetObjectRequest {
            bucket: self.bucket.clone(),
            object: object.to_owned(),
            ..Default::default()
        };
        self.client
            .get_object(&request)
            .await
            .map_err(|e| classify(&e))
    }

    async fn record_visibility(
        &self,
        key: &AssetKey,
        visibility: Visibility,
    ) -> Result<(), AssetError> {
        let sidecar = VisibilitySidecar {
            key: key.to_string(),
            visibility: visibility.as_str().to_owned(),
            enforced: false,
        };
        let encoded = serde_json::to_vec(&sidecar).map_err(|e| {
            AssetError::io("serialize_sidecar", &std::io::Error::other(e.to_string()))
        })?;
        self.run(
            AssetOperation::SetVisibility,
            Some(key),
            self.put_bytes(&self.sidecar(key), encoded, "application/json"),
        )
        .await
    }

    async fn read_visibility(&self, key: &AssetKey) -> Visibility {
        match self.get_bytes(&self.sidecar(key)).await {
            Ok(bytes) => serde_json::from_slice::<VisibilitySidecar>(&bytes)
                .ok()
                .and_then(|s| Visibility::parse(&s.visibility))
                .unwrap_or(Visibility::Private),
            Err(_) => Visibility::Private,
        }
    }

    async fn meta_for(&self, key: &AssetKey) -> Result<AssetMeta, AssetError> {
        let object = self
            .run(
                AssetOperation::Exists,
                Some(key),
                self.head(&self.physical(key)),
            )
            .await?;
        let visibility = self.read_visibility(key).await;
        Ok(AssetMeta {
            key: key.clone(),
            size_bytes: object.size.max(0) as u64,
            content_type: object
                .content_type
                .as_deref()
                .and_then(|ct| ContentType::parse(ct).ok())
                .unwrap_or_default(),
            visibility,
            // GCS has no per-object ACL here; recording is never enforcing.
            visibility_enforced: false,
            backend: BackendKind::Gcs,
            modified_at: object.updated.map(|t| t.to_string()),
            etag: Some(object.etag),
        })
    }

    /// Sign a V4 URL locally. `emulated` in the contract's sense: no
    /// server-side presign API is involved.
    async fn signed_url(
        &self,
        key: &AssetKey,
        method: SignedURLMethod,
        ttl: Duration,
        content_type: Option<&str>,
    ) -> Result<String, AssetError> {
        let operation = match method {
            SignedURLMethod::PUT => AssetOperation::PresignPut,
            _ => AssetOperation::PresignGet,
        };
        let signing = self.signing.as_ref().ok_or_else(|| {
            AssetError::unsupported(
                BackendKind::Gcs,
                operation,
                "GCS presigning signs locally and needs a service-account key; \
                 ADC-only credentials cannot sign without the IAM Credentials API",
            )
        })?;

        let options = SignedURLOptions {
            method,
            expires: ttl,
            content_type: content_type.map(str::to_owned),
            // The endpoint is https in production; the local mock is not.
            insecure: self
                .public_base_url
                .as_deref()
                .is_none_or(|b| b.starts_with("http://")),
            ..Default::default()
        };
        self.client
            .signed_url(
                &self.bucket,
                &self.physical(key),
                Some(signing.google_access_id.clone()),
                Some(signing.sign_by.clone()),
                options,
            )
            .await
            .map_err(|e| AssetError::io("signed_url", &std::io::Error::other(e.to_string())))
    }
}

/// Resolve the service-account key JSON: inline env value first, then the env
/// value as a file path, then `credentials_file`.
async fn resolve_service_account_key(
    source: &AssetStoreSource,
) -> Result<Option<String>, AssetError> {
    if let Some(name) = source.env_key.as_deref()
        && let Ok(value) = std::env::var(name)
        && !value.trim().is_empty()
    {
        if value.trim_start().starts_with('{') {
            return Ok(Some(value));
        }
        match tokio::fs::read_to_string(&value).await {
            Ok(file) => return Ok(Some(file)),
            Err(e) => {
                tracing::warn!(
                    env_key = name,
                    path = value.as_str(),
                    error = %e,
                    "assets: env_key is neither inline JSON nor a readable file; \
                     falling back to credentials_file"
                );
            }
        }
    }
    match source.credentials_file.as_deref() {
        Some(path) => tokio::fs::read_to_string(path)
            .await
            .map(Some)
            .map_err(|e| AssetError::io("read_credentials", &e)),
        None => Ok(None),
    }
}

/// Sibling temp path used to make a download atomic.
fn temp_sibling(dest: &Path) -> PathBuf {
    let name = dest
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "object".to_owned());
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    dest.with_file_name(format!(".{name}.partial-{}-{counter}", std::process::id()))
}

#[async_trait::async_trait]
impl AssetStore for GcsAssetStore {
    fn backend(&self) -> BackendKind {
        BackendKind::Gcs
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            // Recording needs somewhere public to point at; without it the
            // operation is honestly unsupported.
            set_visibility: self.public_base_url.is_some(),
            ..BackendCapabilities::GCS
        }
    }

    async fn put(&self, request: PutRequest) -> Result<AssetMeta, AssetError> {
        self.capabilities()
            .require(BackendKind::Gcs, AssetOperation::Put)?;
        let bytes = match &request.source {
            PutSource::Bytes(bytes) => bytes.to_vec(),
            PutSource::File(path) => tokio::fs::read(path)
                .await
                .map_err(|e| AssetError::io("read", &e))?,
        };
        self.check_size(&request.key, Some(bytes.len() as u64))?;
        let uploaded = bytes.len() as u64;
        self.run(
            AssetOperation::Put,
            Some(&request.key),
            self.put_bytes(
                &self.physical(&request.key),
                bytes,
                request.content_type.as_str(),
            ),
        )
        .await?;
        if let Some(progress) = &request.progress {
            progress.set_total(uploaded);
            progress.add(uploaded);
        }
        self.meta_for(&request.key).await
    }

    async fn put_file(&self, request: PutRequest) -> Result<AssetMeta, AssetError> {
        self.capabilities()
            .require(BackendKind::Gcs, AssetOperation::PutFile)?;
        let path = match &request.source {
            PutSource::File(path) => path.clone(),
            PutSource::Bytes(bytes) => {
                self.check_size(&request.key, Some(bytes.len() as u64))?;
                let uploaded = bytes.len() as u64;
                self.run(
                    AssetOperation::PutFile,
                    Some(&request.key),
                    self.put_bytes(
                        &self.physical(&request.key),
                        bytes.to_vec(),
                        request.content_type.as_str(),
                    ),
                )
                .await?;
                if let Some(progress) = &request.progress {
                    progress.set_total(uploaded);
                    progress.add(uploaded);
                }
                return self.meta_for(&request.key).await;
            }
        };

        let size = tokio::fs::metadata(&path)
            .await
            .map(|m| m.len())
            .map_err(|e| AssetError::io("metadata", &e))?;
        self.check_size(&request.key, Some(size))?;
        if let Some(progress) = &request.progress {
            progress.set_total(size);
        }

        let object = self.physical(&request.key);
        let content_type = request.content_type.as_str();
        self.run(AssetOperation::PutFile, Some(&request.key), async {
            let file = tokio::fs::File::open(&path)
                .await
                .map_err(|e| GcsFailure::Other(e.to_string()))?;
            let mut media = Media::new(object.clone());
            media.content_type = content_type.to_owned().into();
            media.content_length = Some(size);
            let upload = UploadObjectRequest {
                bucket: self.bucket.clone(),
                ..Default::default()
            };
            self.client
                .upload_streamed_object(
                    &upload,
                    ReaderStream::new(file),
                    &UploadType::Simple(media),
                )
                .await
                .map(|_| ())
                .map_err(|e| classify(&e))
        })
        .await?;
        // The GCS SDK streams the body itself, so the payload is reported as
        // one completed unit once the upload returns.
        if let Some(progress) = &request.progress {
            progress.add(size);
        }
        self.meta_for(&request.key).await
    }

    async fn get(&self, key: &AssetKey) -> Result<Bytes, AssetError> {
        self.capabilities()
            .require(BackendKind::Gcs, AssetOperation::Get)?;
        let bytes = self
            .run(
                AssetOperation::Get,
                Some(key),
                self.get_bytes(&self.physical(key)),
            )
            .await?;
        Ok(Bytes::from(bytes))
    }

    async fn download_to(
        &self,
        key: &AssetKey,
        dest: &Path,
        progress: Option<ProgressHandle>,
    ) -> Result<AssetMeta, AssetError> {
        self.capabilities()
            .require(BackendKind::Gcs, AssetOperation::DownloadTo)?;
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| AssetError::io("create_dir_all", &e))?;
        }

        let request = GetObjectRequest {
            bucket: self.bucket.clone(),
            object: self.physical(key),
            ..Default::default()
        };
        let stream = self
            .run(AssetOperation::DownloadTo, Some(key), async {
                self.client
                    .download_streamed_object(&request, &Range::default())
                    .await
                    .map_err(|e| classify(&e))
            })
            .await?;

        let temp = temp_sibling(dest);
        let write = async {
            let mut file = tokio::fs::File::create(&temp)
                .await
                .map_err(|e| AssetError::io("create", &e))?;
            tokio::pin!(stream);
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|e| {
                    AssetError::io("download", &std::io::Error::other(e.to_string()))
                })?;
                file.write_all(&chunk)
                    .await
                    .map_err(|e| AssetError::io("write", &e))?;
                if let Some(progress) = &progress {
                    progress.add(chunk.len() as u64);
                }
            }
            file.flush()
                .await
                .map_err(|e| AssetError::io("flush", &e))?;
            Ok::<(), AssetError>(())
        }
        .await;

        if let Err(err) = write {
            let _ = tokio::fs::remove_file(&temp).await;
            return Err(err);
        }
        tokio::fs::rename(&temp, dest)
            .await
            .map_err(|e| AssetError::io("rename", &e))?;
        self.meta_for(key).await
    }

    async fn exists(&self, key: &AssetKey) -> Result<bool, AssetError> {
        self.capabilities()
            .require(BackendKind::Gcs, AssetOperation::Exists)?;
        match self.run_raw(self.head(&self.physical(key))).await {
            Ok(_) => Ok(true),
            Err(GcsFailure::NotFound) => Ok(false),
            Err(failure) => Err(self.map_failure(AssetOperation::Exists, failure, Some(key))),
        }
    }

    async fn delete(&self, key: &AssetKey) -> Result<DeleteOutcome, AssetError> {
        self.capabilities()
            .require(BackendKind::Gcs, AssetOperation::Delete)?;
        let request = DeleteObjectRequest {
            bucket: self.bucket.clone(),
            object: self.physical(key),
            ..Default::default()
        };
        let outcome = match self
            .run_raw(async {
                self.client
                    .delete_object(&request)
                    .await
                    .map_err(|e| classify(&e))
            })
            .await
        {
            Ok(()) => DeleteOutcome::Deleted,
            Err(GcsFailure::NotFound) => DeleteOutcome::NotFound,
            Err(failure) => {
                return Err(self.map_failure(AssetOperation::Delete, failure, Some(key)));
            }
        };

        let sidecar = DeleteObjectRequest {
            bucket: self.bucket.clone(),
            object: self.sidecar(key),
            ..Default::default()
        };
        let _ = self.client.delete_object(&sidecar).await;
        Ok(outcome)
    }

    async fn list(&self, query: ListQuery) -> Result<ListPage, AssetError> {
        self.capabilities()
            .require(BackendKind::Gcs, AssetOperation::List)?;
        query.validate()?;

        let physical_prefix = format!("{}{}", self.key_prefix, query.prefix.as_str());
        let request = ListObjectsRequest {
            bucket: self.bucket.clone(),
            prefix: (!physical_prefix.is_empty()).then(|| physical_prefix.clone()),
            max_results: Some(query.limit as i32),
            page_token: query
                .cursor
                .as_ref()
                .map(ListCursor::as_str)
                .map(str::to_owned),
            ..Default::default()
        };

        let response = self
            .run(AssetOperation::List, None, async {
                self.client
                    .list_objects(&request)
                    .await
                    .map_err(|e| classify(&e))
            })
            .await?;

        let mut items = Vec::new();
        for object in response.items.unwrap_or_default() {
            let is_meta = object
                .name
                .starts_with(&format!("{RESERVED_META_SEGMENT}/"));
            if is_meta && !query.include_meta {
                continue;
            }
            let key = if is_meta {
                AssetKey::parse_physical(&object.name).ok()
            } else {
                self.logical(&object.name)
            };
            let Some(key) = key else { continue };
            let visibility = self.read_visibility(&key).await;
            items.push(AssetMeta {
                key,
                size_bytes: object.size.max(0) as u64,
                content_type: object
                    .content_type
                    .as_deref()
                    .and_then(|ct| ContentType::parse(ct).ok())
                    .unwrap_or_default(),
                visibility,
                visibility_enforced: false,
                backend: BackendKind::Gcs,
                modified_at: object.updated.map(|t| t.to_string()),
                etag: Some(object.etag),
            });
        }

        let next_cursor = response.next_page_token.map(ListCursor::new);
        Ok(ListPage {
            truncated: next_cursor.is_some(),
            items,
            next_cursor,
        })
    }

    async fn presign_get(&self, key: &AssetKey, ttl: Duration) -> Result<PresignedUrl, AssetError> {
        self.capabilities()
            .require(BackendKind::Gcs, AssetOperation::PresignGet)?;
        let ttl = validate_ttl(ttl)?;
        let url = self
            .signed_url(key, SignedURLMethod::GET, ttl, None)
            .await?;
        Ok(PresignedUrl::new(
            key.clone(),
            url,
            PresignMethod::Get,
            ttl,
            true,
        ))
    }

    async fn presign_put(
        &self,
        key: &AssetKey,
        content_type: &ContentType,
        ttl: Duration,
    ) -> Result<PresignedUrl, AssetError> {
        self.capabilities()
            .require(BackendKind::Gcs, AssetOperation::PresignPut)?;
        let ttl = validate_ttl(ttl)?;
        let url = self
            .signed_url(key, SignedURLMethod::PUT, ttl, Some(content_type.as_str()))
            .await?;
        Ok(
            PresignedUrl::new(key.clone(), url, PresignMethod::Put, ttl, true)
                .with_content_type(content_type.as_str()),
        )
    }

    async fn set_visibility(
        &self,
        key: &AssetKey,
        visibility: Visibility,
    ) -> Result<AssetMeta, AssetError> {
        self.capabilities()
            .require(BackendKind::Gcs, AssetOperation::SetVisibility)?;
        if !self.exists(key).await? {
            return Err(AssetError::NotFound {
                key: key.to_string(),
            });
        }
        self.record_visibility(key, visibility).await?;
        self.meta_for(key).await
    }

    fn public_url(&self, key: &AssetKey) -> Option<String> {
        let path = self.physical(key);
        Some(match self.public_base_url.as_deref() {
            Some(base) => format!("{}/{}", base.trim_end_matches('/'), path),
            None => format!("https://storage.googleapis.com/{}/{}", self.bucket, path),
        })
    }

    async fn health(&self) -> Result<StoreStatus, AssetError> {
        let list_request = ListObjectsRequest {
            bucket: self.bucket.clone(),
            max_results: Some(1),
            ..Default::default()
        };
        if let Err(failure) = self
            .run_raw(async {
                self.client
                    .list_objects(&list_request)
                    .await
                    .map_err(|e| classify(&e))
            })
            .await
        {
            return Ok(StoreStatus::degraded(
                BackendKind::Gcs,
                false,
                describe(&failure),
            ));
        }

        let probe = format!("{}{HEALTH_PROBE_KEY}", self.key_prefix);
        match self
            .run_raw(self.put_bytes(&probe, b"ok".to_vec(), "text/plain"))
            .await
        {
            Ok(()) => {
                let _ = self
                    .client
                    .delete_object(&DeleteObjectRequest {
                        bucket: self.bucket.clone(),
                        object: probe,
                        ..Default::default()
                    })
                    .await;
                Ok(StoreStatus::healthy(BackendKind::Gcs))
            }
            Err(failure) => Ok(StoreStatus::degraded(
                BackendKind::Gcs,
                true,
                describe(&failure),
            )),
        }
    }
}

fn describe(failure: &GcsFailure) -> String {
    match failure {
        GcsFailure::Transient(d) | GcsFailure::Other(d) => d.clone(),
        other => format!("{other:?}"),
    }
}

/// Build a client pointed at an arbitrary endpoint (used by the mock tests).
pub(crate) fn client_for_endpoint(endpoint: &str) -> Client {
    Client::new(
        ClientConfig {
            storage_endpoint: endpoint.to_owned(),
            ..Default::default()
        }
        .anonymous(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Router,
        extract::Path as AxumPath,
        http::StatusCode,
        response::IntoResponse,
        routing::{delete, get, post},
    };
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    /// A real (2048-bit) PKCS#8 RSA key, so presigning exercises real signing.
    const TEST_SA_PRIVATE_KEY: &str = include_str!("testdata/gcs_test_key.pem");

    #[derive(Default)]
    struct MockGcsState {
        objects: RwLock<HashMap<String, Vec<u8>>>,
        denied: std::sync::atomic::AtomicBool,
    }

    impl MockGcsState {
        async fn keys(&self) -> Vec<String> {
            let mut keys: Vec<String> = self.objects.read().await.keys().cloned().collect();
            keys.sort();
            keys
        }
    }

    /// A GCS object resource, with the fields `gcloud-storage` requires.
    fn object_json(name: &str, size: usize) -> String {
        serde_json::json!({
            "name": name,
            "size": size.to_string(),
            "contentType": "application/octet-stream",
            "etag": "etag",
            "id": name,
            "bucket": "test-bucket",
            "generation": "1",
            "metageneration": "1",
            "selfLink": format!("https://example/{name}"),
            "mediaLink": format!("https://example/{name}?alt=media"),
        })
        .to_string()
    }

    /// `ErrorResponse` requires `errors` and `message`.
    fn error_body(code: u16, message: &str) -> String {
        serde_json::json!({ "error": { "code": code, "message": message, "errors": [] } })
            .to_string()
    }

    fn json(status: StatusCode, body: String) -> axum::response::Response {
        (
            status,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            body,
        )
            .into_response()
    }

    /// Minimal GCS JSON API mock: upload, get, download, delete, list.
    async fn start_mock_gcs() -> (String, Arc<MockGcsState>) {
        let state = Arc::new(MockGcsState::default());

        let s = state.clone();
        let upload =
            move |axum::extract::Query(query): axum::extract::Query<HashMap<String, String>>,
                  body: axum::body::Bytes| {
                let state = s.clone();
                async move {
                    let name = query.get("name").cloned().unwrap_or_default();
                    let size = body.len();
                    state
                        .objects
                        .write()
                        .await
                        .insert(name.clone(), body.to_vec());
                    json(StatusCode::OK, object_json(&name, size))
                }
            };

        let s = state.clone();
        let get_or_download =
            move |AxumPath((_, object)): AxumPath<(String, String)>,
                  query: axum::extract::Query<HashMap<String, String>>| {
                let state = s.clone();
                async move {
                    let media = query.get("alt").map(String::as_str) == Some("media");
                    if state.denied.load(Ordering::SeqCst) && !media {
                        return json(StatusCode::FORBIDDEN, error_body(403, "denied"));
                    }
                    match state.objects.read().await.get(&object) {
                        Some(data) if media => (StatusCode::OK, data.clone()).into_response(),
                        Some(data) => json(StatusCode::OK, object_json(&object, data.len())),
                        None if media => StatusCode::NOT_FOUND.into_response(),
                        None => json(StatusCode::NOT_FOUND, error_body(404, "not found")),
                    }
                }
            };

        let s = state.clone();
        let delete_object = move |AxumPath((_, object)): AxumPath<(String, String)>| {
            let state = s.clone();
            async move {
                // GCS returns 404 for a missing object, unlike S3.
                match state.objects.write().await.remove(&object) {
                    Some(_) => StatusCode::NO_CONTENT.into_response(),
                    None => json(StatusCode::NOT_FOUND, error_body(404, "not found")),
                }
            }
        };

        let s = state.clone();
        let list = move |axum::extract::Query(query): axum::extract::Query<
            HashMap<String, String>,
        >| {
            let state = s.clone();
            async move {
                if state.denied.load(Ordering::SeqCst) {
                    return json(StatusCode::FORBIDDEN, error_body(403, "denied"));
                }
                let prefix = query.get("prefix").cloned().unwrap_or_default();
                let mut keys: Vec<String> = state
                    .objects
                    .read()
                    .await
                    .keys()
                    .filter(|k| k.starts_with(&prefix))
                    .cloned()
                    .collect();
                keys.sort();
                let items: Vec<serde_json::Value> = keys
                    .iter()
                    .map(|k| {
                        serde_json::from_str(&object_json(k, 1)).unwrap_or(serde_json::Value::Null)
                    })
                    .collect();
                json(
                    StatusCode::OK,
                    serde_json::json!({ "items": items }).to_string(),
                )
            }
        };

        let router = Router::new()
            .route("/upload/storage/v1/b/{bucket}/o", post(upload))
            .route("/storage/v1/b/{bucket}/o", get(list))
            .route(
                "/storage/v1/b/{bucket}/o/{*object}",
                get(get_or_download).delete(delete_object),
            );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        (format!("http://{addr}"), state)
    }

    fn key(s: &str) -> AssetKey {
        AssetKey::parse(s).unwrap()
    }

    /// A service-account JSON carrying the embedded test key.
    fn write_service_account(dir: &Path) -> PathBuf {
        let path = dir.join("sa.json");
        let json = serde_json::json!({
            "type": "service_account",
            "project_id": "test",
            "private_key_id": "key-id",
            "private_key": TEST_SA_PRIVATE_KEY,
            "client_email": "svc@test.iam.gserviceaccount.com",
            "client_id": "123",
            "auth_uri": "https://accounts.google.com/o/oauth2/auth",
            "token_uri": "https://oauth2.googleapis.com/token",
            "auth_provider_x509_cert_url": "https://www.googleapis.com/oauth2/v1/certs",
            "client_x509_cert_url": "https://example/cert",
        });
        std::fs::write(&path, json.to_string()).unwrap();
        path
    }

    fn source(
        credentials_file: Option<String>,
        public_base_url: Option<String>,
    ) -> AssetStoreSource {
        AssetStoreSource {
            kind: BackendKind::Gcs,
            origin: super::super::SelectionOrigin::ExplicitProvider,
            bucket: Some("test-bucket".to_owned()),
            region: None,
            endpoint_url: None,
            credentials_file,
            env_key: None,
            public_base_url,
            default_visibility: Visibility::Private,
            default_ttl: Duration::from_secs(3_600),
            local_root: PathBuf::from("/tmp/unused"),
            proxy_base_url: None,
            max_object_bytes: None,
            request_timeout: Duration::from_secs(10),
            key_prefix: crate::assets::AssetPrefix::default(),
        }
    }

    /// Store over the mock with a signing key but no ambient credentials.
    async fn signed_store(dir: &Path, endpoint: &str) -> GcsAssetStore {
        let sa = write_service_account(dir);
        let json = std::fs::read_to_string(&sa).unwrap();
        GcsAssetStore::from_source_with_client(
            &source(Some(sa.to_string_lossy().into_owned()), None),
            Some(&json),
            client_for_endpoint(endpoint),
        )
        .await
        .unwrap()
    }

    async fn store(endpoint: &str) -> GcsAssetStore {
        GcsAssetStore::from_source_with_client(
            &source(None, None),
            None,
            client_for_endpoint(endpoint),
        )
        .await
        .unwrap()
    }

    /// Exercises the real `from_source` path (client builder included).
    ///
    /// `gcloud-storage` has no lazy variant: `with_credentials` performs an
    /// **eager** service-account token fetch at construction. So this test
    /// needs egress *and* a real account; the embedded key has a synthetic
    /// `client_email`, which the token endpoint rejects with `invalid_grant`.
    /// Both outcomes are asserted explicitly rather than papered over.
    #[tokio::test]
    async fn from_source_with_a_service_account_key_builds_a_client() {
        let dir = tempfile::tempdir().unwrap();
        let sa = write_service_account(dir.path());
        let result =
            GcsAssetStore::from_source(&source(Some(sa.to_string_lossy().into_owned()), None))
                .await;
        match result {
            Ok(store) => assert_eq!(store.bucket(), "test-bucket"),
            Err(err) => {
                let rendered = err.to_string();
                assert!(
                    rendered.contains("token") || rendered.contains("invalid_grant"),
                    "unexpected construction failure: {rendered}"
                );
            }
        }
    }

    #[tokio::test]
    async fn put_get_exists_delete_round_trip() {
        let (endpoint, state) = start_mock_gcs().await;
        let store = store(&endpoint).await;

        let meta = store
            .put(PutRequest::from_bytes(
                key("uploads/a.txt"),
                b"hello".to_vec(),
                ContentType::parse("text/plain").unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(meta.size_bytes, 5);
        assert_eq!(meta.backend, BackendKind::Gcs);
        assert_eq!(meta.visibility, Visibility::Private);
        assert!(!meta.visibility_enforced);

        assert_eq!(
            &store.get(&key("uploads/a.txt")).await.unwrap()[..],
            b"hello"
        );
        assert!(store.exists(&key("uploads/a.txt")).await.unwrap());
        assert!(!store.exists(&key("uploads/missing.txt")).await.unwrap());
        assert_eq!(state.keys().await, vec!["uploads/a.txt".to_owned()]);

        assert_eq!(
            store.delete(&key("uploads/a.txt")).await.unwrap(),
            DeleteOutcome::Deleted
        );
        assert_eq!(
            store.delete(&key("uploads/a.txt")).await.unwrap(),
            DeleteOutcome::NotFound
        );
    }

    #[tokio::test]
    async fn put_file_streams_and_download_to_writes_to_disk() {
        let (endpoint, _state) = start_mock_gcs().await;
        let store = store(&endpoint).await;
        let dir = tempfile::tempdir().unwrap();

        let source = dir.path().join("in.bin");
        let payload: Vec<u8> = (0..(256 * 1024)).map(|i| (i % 251) as u8).collect();
        std::fs::write(&source, &payload).unwrap();
        let meta = store
            .put_file(PutRequest::from_file(
                key("uploads/blob.bin"),
                source,
                ContentType::default(),
            ))
            .await
            .unwrap();
        assert_eq!(meta.size_bytes, payload.len() as u64);

        let dest = dir.path().join("out.bin");
        let meta = store
            .download_to(&key("uploads/blob.bin"), &dest, None)
            .await
            .unwrap();
        assert_eq!(meta.size_bytes, payload.len() as u64);
        assert_eq!(std::fs::read(&dest).unwrap(), payload);
    }

    #[tokio::test]
    async fn list_filters_by_prefix_and_hides_meta() {
        let (endpoint, _state) = start_mock_gcs().await;
        let store = store(&endpoint).await;
        for name in ["a", "b"] {
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
            .list(ListQuery::new(
                crate::assets::AssetPrefix::parse("uploads/").unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.items[0].key.as_str(), "uploads/a.txt");
        assert!(!page.truncated);
    }

    #[tokio::test]
    async fn set_visibility_needs_a_public_base_url_then_records() {
        let (endpoint, state) = start_mock_gcs().await;
        let plain = store(&endpoint).await;
        assert!(!plain.capabilities().set_visibility);
        plain
            .put(PutRequest::from_bytes(
                key("uploads/a.txt"),
                b"a".to_vec(),
                ContentType::default(),
            ))
            .await
            .unwrap();
        let err = plain
            .set_visibility(&key("uploads/a.txt"), Visibility::Public)
            .await
            .unwrap_err();
        assert_eq!(err.code(), "asset_unsupported");

        let with_public = plain.with_public_base_url("https://cdn.test");
        assert!(with_public.capabilities().set_visibility);
        let meta = with_public
            .set_visibility(&key("uploads/a.txt"), Visibility::Public)
            .await
            .unwrap();
        assert!(meta.visibility.is_public());
        assert!(!meta.visibility_enforced);
        assert!(
            state
                .keys()
                .await
                .contains(&"_meta/uploads/a.txt.visibility.json".to_owned())
        );
        assert_eq!(
            with_public.public_url(&key("uploads/a.txt")).as_deref(),
            Some("https://cdn.test/uploads/a.txt")
        );
    }

    #[tokio::test]
    async fn presign_requires_a_service_account_key() {
        let (endpoint, _state) = start_mock_gcs().await;
        let store = store(&endpoint).await;
        assert!(!store.can_presign());

        let err = store
            .presign_get(&key("uploads/a.txt"), Duration::from_secs(60))
            .await
            .unwrap_err();
        assert_eq!(err.code(), "asset_unsupported");
        assert!(err.to_string().contains("service-account key"), "{err}");
        assert!(
            store
                .presign_put(
                    &key("uploads/a.txt"),
                    &ContentType::default(),
                    Duration::from_secs(60),
                )
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn presign_with_a_service_account_key_produces_a_signed_url() {
        let (endpoint, _state) = start_mock_gcs().await;
        let dir = tempfile::tempdir().unwrap();
        let store = signed_store(dir.path(), &endpoint).await;
        assert!(store.can_presign());

        let url = store
            .presign_get(&key("uploads/a.txt"), Duration::from_secs(60))
            .await
            .unwrap();
        assert!(url.emulated, "GCS presigning is emulated");
        assert_eq!(url.method, PresignMethod::Get);
        assert!(url.url.contains("X-Goog-Signature"), "{}", url.url);
        assert!(
            url.url.contains("X-Goog-Algorithm=GOOG4-RSA-SHA256"),
            "{}",
            url.url
        );

        let put = store
            .presign_put(
                &key("uploads/a.txt"),
                &ContentType::parse("text/plain").unwrap(),
                Duration::from_secs(60),
            )
            .await
            .unwrap();
        assert_eq!(put.method, PresignMethod::Put);
        assert!(put.url.contains("X-Goog-Signature"), "{}", put.url);
    }

    #[tokio::test]
    async fn ttl_bounds_are_enforced_before_signing() {
        let (endpoint, _state) = start_mock_gcs().await;
        let dir = tempfile::tempdir().unwrap();
        let store = signed_store(dir.path(), &endpoint).await;
        assert_eq!(
            store
                .presign_get(&key("uploads/a.txt"), Duration::from_secs(0))
                .await
                .unwrap_err()
                .code(),
            "asset_ttl_out_of_range"
        );
        assert_eq!(
            store
                .presign_get(&key("uploads/a.txt"), Duration::from_secs(604_801))
                .await
                .unwrap_err()
                .code(),
            "asset_ttl_out_of_range"
        );
    }

    #[tokio::test]
    async fn capability_matrix_matches_the_contract() {
        let (endpoint, _state) = start_mock_gcs().await;
        let store = store(&endpoint).await;
        assert_eq!(store.capabilities(), BackendCapabilities::GCS);
        let caps = store.capabilities();
        assert!(caps.delete && caps.list && caps.presign_get && caps.presign_put);
        assert!(!caps.set_visibility, "no public_base_url configured");
        assert!(!caps.visibility_enforced);
    }

    #[tokio::test]
    async fn auth_failures_are_reported_distinctly() {
        let (endpoint, state) = start_mock_gcs().await;
        let store = store(&endpoint).await;
        state.denied.store(true, Ordering::SeqCst);
        let err = store.exists(&key("uploads/a.txt")).await.unwrap_err();
        assert_eq!(err.code(), "asset_access_denied");
        assert!(matches!(
            err,
            AssetError::AccessDenied {
                backend: BackendKind::Gcs,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn health_probes_and_cleans_up() {
        let (endpoint, state) = start_mock_gcs().await;
        let store = store(&endpoint).await;
        let status = store.health().await.unwrap();
        assert!(status.is_healthy());
        assert_eq!(status.backend, BackendKind::Gcs);
        assert!(
            state.keys().await.is_empty(),
            "probe object must be removed"
        );
    }

    #[tokio::test]
    async fn store_is_send_and_shareable() {
        let (endpoint, _state) = start_mock_gcs().await;
        let store: crate::assets::SharedAssetStore = Arc::new(store(&endpoint).await);
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
}
