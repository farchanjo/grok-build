//! S3 (and S3-compatible) [`AssetStore`].
//!
//! Built on the shared primitives in [`crate::s3`]: the client builder, the
//! credential parsing, the presign helpers, and the object operations added for
//! this adapter. No credential format or error classification is re-derived
//! here.
//!
//! Two behaviours deserve their own words:
//!
//! - **Visibility is emulated through object ACLs.** S3 has no "recorded
//!   visibility" concept, so `set_visibility` writes a canned ACL. On a
//!   bucket-owner-enforced bucket S3 rejects `x-amz-acl` with
//!   `AccessControlListNotSupported`; the adapter then flips
//!   `visibility_enforced` to `false`, retries the write without the ACL, and
//!   records the intent in the `_meta/<key>.visibility.json` sidecar. The flip
//!   is sticky for the store's lifetime, so only the first write pays the
//!   extra round trip.
//! - **Presigning needs static credentials.** The SigV4 helpers sign locally
//!   from an access-key pair; an ambient-chain client (instance role, SSO)
//!   cannot be re-signed by them, so `presign_*` returns `Unsupported` with a
//!   clear reason instead of a URL that would 403.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use aws_sdk_s3::primitives::{ByteStream, Length};
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart, ObjectCannedAcl};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

use super::error::{AssetError, AssetOperation};
use super::factory::AssetStoreSource;
use super::key::{AssetKey, ContentType, RESERVED_META_SEGMENT};
use super::progress::{ProgressHandle, copy_with_progress};
use super::value::{
    AssetMeta, DeleteOutcome, ListCursor, ListPage, ListQuery, PresignMethod, PresignedUrl,
    PutRequest, PutSource, Visibility,
};
use super::{AssetStore, BackendCapabilities, BackendKind, StoreStatus, validate_ttl};
use crate::s3::{S3Failure, S3StaticCredentials, classify_sdk_error};

/// Suffix appended to a key to name its visibility sidecar.
const VISIBILITY_SIDECAR_SUFFIX: &str = ".visibility.json";

/// Region assumed when `[assets].region` is unset. S3-compatible endpoints
/// require *a* region but almost never validate it.
const DEFAULT_REGION: &str = "us-east-1";

/// Key (under the reserved namespace) used for the writability probe.
const HEALTH_PROBE_KEY: &str = "_meta/.health-probe";

/// Monotonic suffix so two concurrent downloads to the same directory do not
/// collide on a temp name.
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Records the visibility of one object on a backend that cannot enforce it.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct VisibilitySidecar {
    key: String,
    visibility: String,
    enforced: bool,
}

/// An [`AssetStore`] backed by an S3-compatible bucket.
pub struct S3AssetStore {
    client: aws_sdk_s3::Client,
    bucket: String,
    region: String,
    endpoint_url: Option<String>,
    /// Present only when static credentials were configured; required for
    /// presigning.
    credentials: Option<S3StaticCredentials>,
    public_base_url: Option<String>,
    key_prefix: String,
    max_object_bytes: Option<u64>,
    request_timeout: Duration,
    /// `true` until the bucket rejects an ACL, then sticky `false`.
    acl_enforced: AtomicBool,
}

impl std::fmt::Debug for S3AssetStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("S3AssetStore")
            .field("bucket", &self.bucket)
            .field("region", &self.region)
            .field("endpoint_url", &self.endpoint_url)
            .field("key_prefix", &self.key_prefix)
            .field("acl_enforced", &self.acl_enforced.load(Ordering::Relaxed))
            .finish()
    }
}

impl S3AssetStore {
    /// Build from a resolved selection.
    ///
    /// Fails only on construction problems (unreadable credentials file,
    /// client build failure); no network call happens here.
    pub async fn from_source(source: &AssetStoreSource) -> Result<Self, AssetError> {
        let bucket = source
            .bucket
            .clone()
            .ok_or_else(|| AssetError::InvalidKey {
                key: "assets.bucket".to_owned(),
                reason: "the s3 backend needs a bucket".to_owned(),
            })?;
        let region = source
            .region
            .clone()
            .filter(|r| !r.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_REGION.to_owned());

        let content = resolve_credential_content(source).await?;
        let credentials = content
            .as_deref()
            .and_then(|c| crate::s3::static_credentials_from_content(c).ok());

        let client = crate::s3::build_s3_client(
            &region,
            content.as_deref(),
            None,
            source.endpoint_url.as_deref(),
        )
        .await
        .map_err(|e| AssetError::io("build_s3_client", &std::io::Error::other(e.to_string())))?;

        Ok(Self {
            client,
            bucket,
            region,
            endpoint_url: source.endpoint_url.clone(),
            credentials,
            public_base_url: source.public_base_url.clone(),
            key_prefix: source.key_prefix.as_str().to_owned(),
            max_object_bytes: source.max_object_bytes,
            request_timeout: source.request_timeout,
            acl_enforced: AtomicBool::new(true),
        })
    }

    /// Inject a pre-built client. Used by the adapter tests to point at the
    /// mock server without re-deriving credentials.
    pub fn with_client(mut self, client: aws_sdk_s3::Client) -> Self {
        self.client = client;
        self
    }

    pub fn bucket(&self) -> &str {
        &self.bucket
    }

    pub fn request_timeout(&self) -> Duration {
        self.request_timeout
    }

    /// Whether visibility is currently enforced through object ACLs.
    pub fn visibility_enforced(&self) -> bool {
        self.acl_enforced.load(Ordering::SeqCst)
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

    /// Map a physical object key back to a logical key.
    fn logical(&self, physical: &str) -> Option<AssetKey> {
        let stripped = physical.strip_prefix(&self.key_prefix)?;
        if stripped.is_empty() {
            return None;
        }
        AssetKey::parse(stripped).ok()
    }

    fn canned_acl(visibility: Visibility) -> ObjectCannedAcl {
        match visibility {
            Visibility::Public => ObjectCannedAcl::PublicRead,
            Visibility::Private => ObjectCannedAcl::Private,
        }
    }

    /// ACL to send for this write, or `None` once the bucket has degraded.
    fn acl_for(&self, visibility: Visibility) -> Option<ObjectCannedAcl> {
        self.visibility_enforced()
            .then(|| Self::canned_acl(visibility))
    }

    // ---- error plumbing --------------------------------------------------

    fn map_failure(
        &self,
        operation: AssetOperation,
        failure: S3Failure,
        key: Option<&AssetKey>,
    ) -> AssetError {
        match failure {
            S3Failure::NotFound => AssetError::NotFound {
                key: key.map(AssetKey::to_string).unwrap_or_default(),
            },
            S3Failure::Unauthorized => AssetError::Unauthorized {
                backend: BackendKind::S3,
                detail: format!("{operation} rejected the credentials"),
            },
            S3Failure::AccessDenied => AssetError::AccessDenied {
                backend: BackendKind::S3,
                detail: format!("{operation} is not permitted"),
            },
            S3Failure::AclNotSupported => AssetError::unsupported(
                BackendKind::S3,
                operation,
                "the bucket rejects x-amz-acl (bucket-owner-enforced)",
            ),
            // Everything the classifier could not name is treated as retryable
            // transport noise; a 4xx that is not auth/404 lands here too.
            S3Failure::Transient(detail) | S3Failure::Other(detail) => AssetError::Transient {
                backend: BackendKind::S3,
                detail,
            },
        }
    }

    /// Run one SDK call under the configured request timeout, without mapping
    /// the failure — used where the caller must see `AclNotSupported`.
    async fn run_raw<T, F>(&self, fut: F) -> Result<T, S3Failure>
    where
        F: std::future::Future<Output = Result<T, S3Failure>>,
    {
        match tokio::time::timeout(self.request_timeout, fut).await {
            Ok(result) => result,
            Err(_) => Err(self.timeout_failure("call")),
        }
    }

    /// [`S3Failure::Transient`] naming the call that exceeded
    /// `request_timeout`.
    ///
    /// Used on the multipart path, where the timeout must apply **per SDK
    /// call**: a single deadline over the whole transfer would cap an upload
    /// at `request_timeout * throughput` regardless of how healthy the
    /// connection is.
    fn timeout_failure(&self, what: &str) -> S3Failure {
        S3Failure::Transient(format!(
            "{what} timed out after {}s",
            self.request_timeout.as_secs()
        ))
    }

    /// Budget for `CompleteMultipartUpload`.
    ///
    /// The call does not stream: the server assembles the object from the
    /// already-uploaded parts, and that takes longer than one part upload
    /// (measured: >30s for 25 parts on R2). Reusing the per-request timeout
    /// here fails a transfer whose bytes all landed. Four times the request
    /// timeout, floored at 2 minutes.
    fn complete_timeout(&self) -> std::time::Duration {
        self.request_timeout
            .saturating_mul(4)
            .max(std::time::Duration::from_secs(120))
    }

    async fn run<T, F>(
        &self,
        operation: AssetOperation,
        key: Option<&AssetKey>,
        fut: F,
    ) -> Result<T, AssetError>
    where
        F: std::future::Future<Output = Result<T, S3Failure>>,
    {
        self.run_raw(fut)
            .await
            .map_err(|failure| self.map_failure(operation, failure, key))
    }

    // ---- size + visibility helpers ---------------------------------------

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

    /// Sticky degrade: stop sending ACLs and start recording visibility.
    fn degrade_acl(&self, key: &AssetKey) {
        if self.acl_enforced.swap(false, Ordering::SeqCst) {
            tracing::warn!(
                bucket = self.bucket.as_str(),
                key = key.as_str(),
                "bucket rejects x-amz-acl; degrading to recorded visibility \
                 (visibility_enforced=false)"
            );
        }
    }

    /// Record visibility in the sidecar.
    ///
    /// Written on both paths: it is the only way `list`/`meta` can report
    /// visibility without an extra `GetObjectAcl` round trip, and `enforced`
    /// keeps the sidecar honest about whether the ACL actually landed.
    async fn record_visibility(
        &self,
        key: &AssetKey,
        visibility: Visibility,
        enforced: bool,
    ) -> Result<(), AssetError> {
        let sidecar = VisibilitySidecar {
            key: key.to_string(),
            visibility: visibility.as_str().to_owned(),
            enforced,
        };
        let encoded = serde_json::to_vec(&sidecar).map_err(|e| {
            AssetError::io("serialize_sidecar", &std::io::Error::other(e.to_string()))
        })?;
        let encoded_len = encoded.len() as u64;
        self.run(
            AssetOperation::SetVisibility,
            Some(key),
            crate::s3::put_object(
                &self.client,
                &self.bucket,
                &self.sidecar(key),
                ByteStream::from(encoded),
                "application/json",
                Some(encoded_len),
                None,
            ),
        )
        .await
    }

    /// Recorded visibility, or `Private` when there is no sidecar.
    async fn read_visibility(&self, key: &AssetKey) -> Visibility {
        match crate::s3::get_object_bytes(&self.client, &self.bucket, &self.sidecar(key)).await {
            Ok(bytes) => serde_json::from_slice::<VisibilitySidecar>(&bytes)
                .ok()
                .and_then(|s| Visibility::parse(&s.visibility))
                .unwrap_or(Visibility::Private),
            Err(S3Failure::NotFound) => Visibility::Private,
            Err(_) => Visibility::Private,
        }
    }

    async fn meta_for(&self, key: &AssetKey) -> Result<AssetMeta, AssetError> {
        let info = self
            .run(
                AssetOperation::Exists,
                Some(key),
                crate::s3::head_object(&self.client, &self.bucket, &self.physical(key)),
            )
            .await?;
        let visibility = self.read_visibility(key).await;
        Ok(AssetMeta {
            key: key.clone(),
            size_bytes: info.content_length,
            content_type: info
                .content_type
                .as_deref()
                .and_then(|ct| ContentType::parse(ct).ok())
                .unwrap_or_default(),
            visibility,
            visibility_enforced: self.visibility_enforced(),
            backend: BackendKind::S3,
            modified_at: info.last_modified,
            etag: info.etag,
        })
    }

    // ---- upload paths ----------------------------------------------------

    /// Build the request body. `ByteStream` is not `Clone`, so the ACL
    /// degrade retry rebuilds it: from refcounted `Bytes` for the buffered
    /// path, by reopening the file for the streamed one.
    async fn body_for(source: &PutSource) -> Result<ByteStream, AssetError> {
        match source {
            PutSource::Bytes(bytes) => Ok(ByteStream::from(bytes.clone())),
            PutSource::File(path) => ByteStream::from_path(path)
                .await
                .map_err(|e| AssetError::io("open", &std::io::Error::other(e.to_string()))),
        }
    }

    /// `PutObject` with an ACL, degrading to a recorded visibility on
    /// `AccessControlListNotSupported`.
    async fn put_body(
        &self,
        key: &AssetKey,
        source: &PutSource,
        content_type: &str,
        content_length: Option<u64>,
        visibility: Visibility,
    ) -> Result<(), AssetError> {
        let physical = self.physical(key);
        let body = Self::body_for(source).await?;
        match self
            .run_raw(crate::s3::put_object(
                &self.client,
                &self.bucket,
                &physical,
                body,
                content_type,
                content_length,
                self.acl_for(visibility),
            ))
            .await
        {
            Ok(()) => Ok(()),
            Err(S3Failure::AclNotSupported) => {
                self.degrade_acl(key);
                let body = Self::body_for(source).await?;
                self.run(
                    AssetOperation::Put,
                    Some(key),
                    crate::s3::put_object(
                        &self.client,
                        &self.bucket,
                        &physical,
                        body,
                        content_type,
                        content_length,
                        None,
                    ),
                )
                .await?;
                self.record_visibility(key, visibility, false).await
            }
            Err(failure) => Err(self.map_failure(AssetOperation::Put, failure, Some(key))),
        }
    }

    /// Streaming multipart upload: each part is read from disk on demand, so a
    /// multi-gigabyte file never lands in memory. Every uploaded part is
    /// reported into `progress`.
    async fn put_file_multipart(
        &self,
        key: &AssetKey,
        path: &Path,
        size: u64,
        content_type: &str,
        acl: Option<ObjectCannedAcl>,
        progress: Option<&ProgressHandle>,
    ) -> Result<(), S3Failure> {
        let physical = self.physical(key);
        let mut create = self
            .client
            .create_multipart_upload()
            .bucket(&self.bucket)
            .key(&physical)
            .content_type(content_type);
        if let Some(acl) = acl {
            create = create.acl(acl);
        }
        let created = tokio::time::timeout(self.request_timeout, create.send())
            .await
            .map_err(|_| self.timeout_failure("create multipart upload"))?
            .map_err(|e| classify_sdk_error(&e))?;
        let upload_id = match created.upload_id() {
            Some(id) => id.to_owned(),
            None => return Err(S3Failure::Transient("missing upload_id".to_owned())),
        };

        let result = async {
            let mut completed: Vec<CompletedPart> = Vec::new();
            let mut offset: u64 = 0;
            let mut part_number: i32 = 1;
            while offset < size {
                let length = crate::s3::MULTIPART_PART_SIZE.min((size - offset) as usize) as u64;
                let body = ByteStream::read_from()
                    .path(path)
                    .offset(offset)
                    .length(Length::Exact(length))
                    .build()
                    .await
                    .map_err(|e| S3Failure::Transient(e.to_string()))?;
                let part = tokio::time::timeout(
                    self.request_timeout,
                    self.client
                        .upload_part()
                        .bucket(&self.bucket)
                        .key(&physical)
                        .upload_id(&upload_id)
                        .part_number(part_number)
                        .content_length(length as i64)
                        .body(body)
                        .send(),
                )
                .await
                .map_err(|_| self.timeout_failure(&format!("part {part_number}")))?
                .map_err(|e| classify_sdk_error(&e))?;
                let etag = part
                    .e_tag()
                    .ok_or_else(|| S3Failure::Transient("missing part ETag".to_owned()))?
                    .to_owned();
                completed.push(
                    CompletedPart::builder()
                        .part_number(part_number)
                        .e_tag(etag)
                        .build(),
                );
                offset += length;
                part_number += 1;
                if let Some(progress) = progress {
                    progress.add(length);
                }
            }

            let upload = CompletedMultipartUpload::builder()
                .set_parts(Some(completed))
                .build();
            self.client
                .complete_multipart_upload()
                .bucket(&self.bucket)
                .key(&physical)
                .upload_id(&upload_id)
                .multipart_upload(upload)
                .send()
                .await
                .map(|_| ())
                .map_err(|e| classify_sdk_error(&e))
        };
        let result = match tokio::time::timeout(self.complete_timeout(), result).await {
            Ok(inner) => inner,
            Err(_) => Err(S3Failure::Transient(format!(
                "complete multipart upload timed out after {}s",
                self.complete_timeout().as_secs()
            ))),
        };

        if result.is_err() {
            // Best effort: never leak an incomplete upload.
            let _ = self
                .client
                .abort_multipart_upload()
                .bucket(&self.bucket)
                .key(&physical)
                .upload_id(&upload_id)
                .send()
                .await;
        }
        result
    }

    async fn put_file_streaming(
        &self,
        request: &PutRequest,
        path: &Path,
        size: u64,
    ) -> Result<(), AssetError> {
        let key = &request.key;
        let content_type = request.content_type.as_str();
        let progress = request.progress.as_ref();
        if let Some(progress) = progress {
            progress.set_total(size);
        }

        if size < crate::s3::MULTIPART_THRESHOLD as u64 {
            let result = self
                .put_body(
                    key,
                    &request.source,
                    content_type,
                    Some(size),
                    request.visibility,
                )
                .await;
            if result.is_ok()
                && let Some(progress) = progress
            {
                progress.add(size);
            }
            return result;
        }

        // No `run_raw` here: the multipart path applies `request_timeout` per SDK
        // call, so a large transfer is not capped by one global deadline.
        match self
            .put_file_multipart(
                key,
                path,
                size,
                content_type,
                self.acl_for(request.visibility),
                progress,
            )
            .await
        {
            Ok(()) => Ok(()),
            Err(S3Failure::AclNotSupported) => {
                self.degrade_acl(key);
                // The retry restarts the multipart upload, so the byte count
                // restarts with it.
                if let Some(progress) = progress {
                    progress.reset();
                }
                // Same as above: no global deadline on the multipart retry either.
                self.put_file_multipart(key, path, size, content_type, None, progress)
                    .await
                    .map_err(|failure| {
                        self.map_failure(AssetOperation::PutFile, failure, Some(key))
                    })?;
                self.record_visibility(key, request.visibility, false).await
            }
            Err(failure) => Err(self.map_failure(AssetOperation::PutFile, failure, Some(key))),
        }
    }

    fn require_presign_credentials(
        &self,
        operation: AssetOperation,
    ) -> Result<&S3StaticCredentials, AssetError> {
        self.credentials.as_ref().ok_or_else(|| {
            AssetError::unsupported(
                BackendKind::S3,
                operation,
                "presigning needs static credentials; set `env_key` or `credentials_file` \
                 (an ambient-chain client cannot be re-signed locally)",
            )
        })
    }
}

/// Resolve credential content: an inline env value wins, then the env value as
/// a file path, then `credentials_file`. `None` leaves the SDK's ambient chain
/// in charge.
async fn resolve_credential_content(
    source: &AssetStoreSource,
) -> Result<Option<String>, AssetError> {
    if let Some(name) = source.env_key.as_deref()
        && let Ok(value) = std::env::var(name)
        && !value.trim().is_empty()
    {
        if crate::s3::parse_credential_fields(&value).is_ok() {
            return Ok(Some(value));
        }
        match tokio::fs::read_to_string(&value).await {
            Ok(file) => return Ok(Some(file)),
            Err(e) => {
                tracing::warn!(
                    env_key = name,
                    path = value.as_str(),
                    error = %e,
                    "assets: env_key is neither inline credentials nor a readable file; \
                     falling back to credentials_file"
                );
            }
        }
    }
    crate::s3::resolve_credentials_content(None, source.credentials_file.as_deref())
        .await
        .map_err(|e| AssetError::io("read_credentials", &std::io::Error::other(e.to_string())))
}

#[async_trait::async_trait]
impl AssetStore for S3AssetStore {
    fn backend(&self) -> BackendKind {
        BackendKind::S3
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            // Flipped by the ACL degrade; every operation stays supported.
            visibility_enforced: self.visibility_enforced(),
            ..BackendCapabilities::S3
        }
    }

    async fn put(&self, request: PutRequest) -> Result<AssetMeta, AssetError> {
        self.capabilities()
            .require(BackendKind::S3, AssetOperation::Put)?;
        // Buffering entry point: a file source is read whole, then written.
        let source = match &request.source {
            PutSource::Bytes(bytes) => {
                self.check_size(&request.key, Some(bytes.len() as u64))?;
                PutSource::Bytes(bytes.clone())
            }
            PutSource::File(path) => {
                let size = tokio::fs::metadata(path)
                    .await
                    .map(|m| m.len())
                    .map_err(|e| AssetError::io("metadata", &e))?;
                self.check_size(&request.key, Some(size))?;
                PutSource::Bytes(Bytes::from(
                    tokio::fs::read(path)
                        .await
                        .map_err(|e| AssetError::io("read", &e))?,
                ))
            }
        };
        let length = source.buffered_len();
        self.put_body(
            &request.key,
            &source,
            request.content_type.as_str(),
            length,
            request.visibility,
        )
        .await?;
        // Buffered path: the whole payload is already in memory.
        if let (Some(progress), Some(length)) = (&request.progress, length) {
            progress.set_total(length);
            progress.add(length);
        }
        self.meta_for(&request.key).await
    }

    async fn put_file(&self, request: PutRequest) -> Result<AssetMeta, AssetError> {
        self.capabilities()
            .require(BackendKind::S3, AssetOperation::PutFile)?;
        match &request.source {
            PutSource::File(path) => {
                let size = tokio::fs::metadata(path)
                    .await
                    .map(|m| m.len())
                    .map_err(|e| AssetError::io("metadata", &e))?;
                self.check_size(&request.key, Some(size))?;
                self.put_file_streaming(&request, path, size).await?;
            }
            PutSource::Bytes(bytes) => {
                self.check_size(&request.key, Some(bytes.len() as u64))?;
                self.put_body(
                    &request.key,
                    &request.source,
                    request.content_type.as_str(),
                    Some(bytes.len() as u64),
                    request.visibility,
                )
                .await?;
                if let Some(progress) = &request.progress {
                    progress.set_total(bytes.len() as u64);
                    progress.add(bytes.len() as u64);
                }
            }
        }
        self.meta_for(&request.key).await
    }

    async fn get(&self, key: &AssetKey) -> Result<Bytes, AssetError> {
        self.capabilities()
            .require(BackendKind::S3, AssetOperation::Get)?;
        self.run(
            AssetOperation::Get,
            Some(key),
            crate::s3::get_object_bytes(&self.client, &self.bucket, &self.physical(key)),
        )
        .await
    }

    async fn download_to(
        &self,
        key: &AssetKey,
        dest: &Path,
        progress: Option<ProgressHandle>,
    ) -> Result<AssetMeta, AssetError> {
        self.capabilities()
            .require(BackendKind::S3, AssetOperation::DownloadTo)?;
        let output = self
            .run(
                AssetOperation::DownloadTo,
                Some(key),
                crate::s3::get_object(&self.client, &self.bucket, &self.physical(key)),
            )
            .await?;

        // `Content-Length` is the honest total; a chunked response leaves it
        // unknown and the snapshot reports that.
        if let Some(progress) = &progress
            && let Some(length) = output.content_length
            && length > 0
        {
            progress.set_total(length as u64);
        }

        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| AssetError::io("create_dir_all", &e))?;
        }
        let temp = temp_sibling(dest);
        let mut reader = output.body.into_async_read();
        let write = async {
            let mut file = tokio::fs::File::create(&temp)
                .await
                .map_err(|e| AssetError::io("create", &e))?;
            copy_with_progress(&mut reader, &mut file, progress.as_ref())
                .await
                .map_err(|e| AssetError::io("copy", &e))?;
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
            .require(BackendKind::S3, AssetOperation::Exists)?;
        match self
            .run_raw(crate::s3::head_object(
                &self.client,
                &self.bucket,
                &self.physical(key),
            ))
            .await
        {
            Ok(_) => Ok(true),
            Err(S3Failure::NotFound) => Ok(false),
            Err(failure) => Err(self.map_failure(AssetOperation::Exists, failure, Some(key))),
        }
    }

    async fn delete(&self, key: &AssetKey) -> Result<DeleteOutcome, AssetError> {
        self.capabilities()
            .require(BackendKind::S3, AssetOperation::Delete)?;
        self.run(
            AssetOperation::Delete,
            Some(key),
            crate::s3::delete_object(&self.client, &self.bucket, &self.physical(key)),
        )
        .await?;
        // A dangling sidecar is noise; drop it best-effort.
        let _ = crate::s3::delete_object(&self.client, &self.bucket, &self.sidecar(key)).await;
        // S3 returns 204 for a missing key, so a delete is always "deleted".
        Ok(DeleteOutcome::Deleted)
    }

    async fn list(&self, query: ListQuery) -> Result<ListPage, AssetError> {
        self.capabilities()
            .require(BackendKind::S3, AssetOperation::List)?;
        query.validate()?;

        let physical_prefix = format!("{}{}", self.key_prefix, query.prefix.as_str());
        // `_meta/` sits beside `key_prefix`, so including sidecars means
        // listing from the bucket root and filtering client-side.
        let prefix = if query.include_meta && !physical_prefix.is_empty() {
            ""
        } else {
            physical_prefix.as_str()
        };

        let page = self
            .run(
                AssetOperation::List,
                None,
                crate::s3::list_objects_v2(
                    &self.client,
                    &self.bucket,
                    (!prefix.is_empty()).then_some(prefix),
                    query.limit as i32,
                    query.cursor.as_ref().map(ListCursor::as_str),
                ),
            )
            .await?;

        let mut items = Vec::with_capacity(page.keys.len());
        for (physical, size) in page.keys {
            let is_meta = physical.starts_with(&format!("{RESERVED_META_SEGMENT}/"));
            let key = if is_meta {
                AssetKey::parse_physical(&physical).ok()
            } else {
                self.logical(&physical)
            };
            let Some(key) = key else { continue };
            let visibility = self.read_visibility(&key).await;
            items.push(AssetMeta {
                key,
                size_bytes: size,
                content_type: ContentType::from_extension(
                    physical.rsplit_once('.').map(|(_, e)| e).unwrap_or(""),
                ),
                visibility,
                visibility_enforced: self.visibility_enforced(),
                backend: BackendKind::S3,
                modified_at: None,
                etag: None,
            });
        }

        let next_cursor = page.next_continuation_token.map(ListCursor::new);
        Ok(ListPage {
            items,
            truncated: page.is_truncated || next_cursor.is_some(),
            next_cursor,
        })
    }

    async fn presign_get(&self, key: &AssetKey, ttl: Duration) -> Result<PresignedUrl, AssetError> {
        self.capabilities()
            .require(BackendKind::S3, AssetOperation::PresignGet)?;
        let ttl = validate_ttl(ttl)?;
        let creds = self.require_presign_credentials(AssetOperation::PresignGet)?;
        let url = crate::s3::presign_get_url(
            &self.region,
            self.endpoint_url.as_deref(),
            creds,
            &self.bucket,
            &self.physical(key),
            ttl,
        )
        .await
        .map_err(|e| AssetError::io("presign_get", &std::io::Error::other(e.to_string())))?;
        Ok(PresignedUrl::new(
            key.clone(),
            url,
            PresignMethod::Get,
            ttl,
            false,
        ))
    }

    async fn presign_put(
        &self,
        key: &AssetKey,
        content_type: &ContentType,
        ttl: Duration,
    ) -> Result<PresignedUrl, AssetError> {
        self.capabilities()
            .require(BackendKind::S3, AssetOperation::PresignPut)?;
        let ttl = validate_ttl(ttl)?;
        let creds = self.require_presign_credentials(AssetOperation::PresignPut)?;
        let url = crate::s3::presign_put_url(
            &self.region,
            self.endpoint_url.as_deref(),
            creds,
            &self.bucket,
            &self.physical(key),
            content_type.as_str(),
            ttl,
        )
        .await
        .map_err(|e| AssetError::io("presign_put", &std::io::Error::other(e.to_string())))?;
        Ok(
            PresignedUrl::new(key.clone(), url, PresignMethod::Put, ttl, false)
                .with_content_type(content_type.as_str()),
        )
    }

    async fn set_visibility(
        &self,
        key: &AssetKey,
        visibility: Visibility,
    ) -> Result<AssetMeta, AssetError> {
        self.capabilities()
            .require(BackendKind::S3, AssetOperation::SetVisibility)?;
        if !self.exists(key).await? {
            return Err(AssetError::NotFound {
                key: key.to_string(),
            });
        }

        if !self.visibility_enforced() {
            self.record_visibility(key, visibility, false).await?;
            return self.meta_for(key).await;
        }

        let physical = self.physical(key);
        match self
            .run_raw(crate::s3::put_object_acl(
                &self.client,
                &self.bucket,
                &physical,
                Self::canned_acl(visibility),
            ))
            .await
        {
            Ok(()) => {
                self.record_visibility(key, visibility, true).await?;
                self.meta_for(key).await
            }
            Err(S3Failure::AclNotSupported) => {
                self.degrade_acl(key);
                self.record_visibility(key, visibility, false).await?;
                self.meta_for(key).await
            }
            Err(failure) => {
                Err(self.map_failure(AssetOperation::SetVisibility, failure, Some(key)))
            }
        }
    }

    fn public_url(&self, key: &AssetKey) -> Option<String> {
        let path = self.physical(key);
        if let Some(base) = self.public_base_url.as_deref() {
            return Some(format!("{}/{}", base.trim_end_matches('/'), path));
        }
        match self.endpoint_url.as_deref() {
            // Path-style endpoint: the bucket is a path segment.
            Some(endpoint) => Some(format!(
                "{}/{}/{}",
                endpoint.trim_end_matches('/'),
                self.bucket,
                path
            )),
            // Virtual-hosted bucket URL.
            None => Some(format!(
                "https://{}.s3.{}.amazonaws.com/{}",
                self.bucket, self.region, path
            )),
        }
    }

    async fn health(&self) -> Result<StoreStatus, AssetError> {
        if let Err(failure) = self
            .run_raw(crate::s3::head_bucket(&self.client, &self.bucket))
            .await
        {
            let detail = match failure {
                S3Failure::Transient(d) | S3Failure::Other(d) => d,
                other => format!("{other:?}"),
            };
            return Ok(StoreStatus::degraded(BackendKind::S3, false, detail));
        }

        // Writability: a tiny object that is written and removed again.
        let physical = format!("{}{HEALTH_PROBE_KEY}", self.key_prefix);
        match self
            .run_raw(crate::s3::put_object(
                &self.client,
                &self.bucket,
                &physical,
                ByteStream::from_static(b"ok"),
                "text/plain",
                Some(2),
                None,
            ))
            .await
        {
            Ok(()) => {
                let _ = crate::s3::delete_object(&self.client, &self.bucket, &physical).await;
                Ok(StoreStatus::healthy(BackendKind::S3))
            }
            Err(failure) => {
                let detail = match failure {
                    S3Failure::Transient(d) | S3Failure::Other(d) => d,
                    other => format!("{other:?}"),
                };
                Ok(StoreStatus::degraded(BackendKind::S3, true, detail))
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::s3::tests::{make_raw_test_client, start_mock_server};
    use crate::s3::{ACL_NOT_SUPPORTED_CODE, S3StaticCredentials};

    fn key(s: &str) -> AssetKey {
        AssetKey::parse(s).unwrap()
    }

    async fn store(endpoint: &str) -> S3AssetStore {
        let source = AssetStoreSource {
            kind: BackendKind::S3,
            origin: super::super::SelectionOrigin::ExplicitProvider,
            bucket: Some("test-bucket".to_owned()),
            region: Some("us-east-1".to_owned()),
            endpoint_url: Some(endpoint.to_owned()),
            credentials_file: None,
            env_key: None,
            public_base_url: None,
            default_visibility: Visibility::Private,
            default_ttl: Duration::from_secs(3_600),
            local_root: PathBuf::from("/tmp/unused"),
            proxy_base_url: None,
            max_object_bytes: None,
            request_timeout: Duration::from_secs(10),
            key_prefix: crate::assets::AssetPrefix::default(),
        };
        // The mock needs no credential chain; inject a client built for it so
        // the test does not depend on the ambient AWS environment.
        let client = make_raw_test_client(endpoint).await;
        let mut store = S3AssetStore::from_source(&source).await.unwrap();
        store.credentials = Some(S3StaticCredentials {
            access_key_id: "test".to_owned(),
            secret_access_key: "test".to_owned(),
        });
        store.with_client(client)
    }

    async fn put_text(store: &S3AssetStore, key_str: &str, body: &[u8]) {
        store
            .put(PutRequest::from_bytes(
                key(key_str),
                body.to_vec(),
                ContentType::parse("text/plain").unwrap(),
            ))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn put_get_exists_delete_round_trip() {
        let (endpoint, state) = start_mock_server().await;
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
        assert_eq!(meta.backend, BackendKind::S3);
        assert_eq!(meta.content_type.as_str(), "text/plain");
        assert_eq!(meta.visibility, Visibility::Private);

        assert_eq!(
            &store.get(&key("uploads/a.txt")).await.unwrap()[..],
            b"hello"
        );
        assert!(store.exists(&key("uploads/a.txt")).await.unwrap());
        assert!(!store.exists(&key("uploads/missing.txt")).await.unwrap());
        assert_eq!(
            store
                .get(&key("uploads/missing.txt"))
                .await
                .unwrap_err()
                .code(),
            "asset_not_found"
        );

        assert_eq!(state.object_keys().await, vec!["uploads/a.txt".to_owned()]);
        assert_eq!(
            store.delete(&key("uploads/a.txt")).await.unwrap(),
            DeleteOutcome::Deleted
        );
        assert!(!store.exists(&key("uploads/a.txt")).await.unwrap());
    }

    /// Request shaping: a public put must carry `x-amz-acl: public-read`.
    #[tokio::test]
    async fn put_sends_the_canned_acl_header() {
        let (endpoint, state) = start_mock_server().await;
        let store = store(&endpoint).await;

        store
            .put(
                PutRequest::from_bytes(
                    key("uploads/pub.txt"),
                    b"p".to_vec(),
                    ContentType::default(),
                )
                .with_visibility(Visibility::Public),
            )
            .await
            .unwrap();
        assert_eq!(
            state.put_acl_header("uploads/pub.txt").await.as_deref(),
            Some("public-read")
        );

        store
            .put(PutRequest::from_bytes(
                key("uploads/priv.txt"),
                b"q".to_vec(),
                ContentType::default(),
            ))
            .await
            .unwrap();
        assert_eq!(
            state.put_acl_header("uploads/priv.txt").await.as_deref(),
            Some("private")
        );
    }

    /// F18: a bucket that rejects `x-amz-acl` must degrade, not fail.
    #[tokio::test]
    async fn acl_rejection_degrades_to_recorded_visibility() {
        let (endpoint, state) = start_mock_server().await;
        state.reject_acls();
        let store = store(&endpoint).await;
        assert!(store.visibility_enforced());
        assert!(store.capabilities().visibility_enforced);

        let meta = store
            .put(
                PutRequest::from_bytes(
                    key("uploads/pub.txt"),
                    b"p".to_vec(),
                    ContentType::default(),
                )
                .with_visibility(Visibility::Public),
            )
            .await
            .unwrap();

        // The object landed anyway...
        assert_eq!(&store.get(&key("uploads/pub.txt")).await.unwrap()[..], b"p");
        // ...visibility is now recorded, not enforced...
        assert_eq!(meta.visibility, Visibility::Public);
        assert!(!meta.visibility_enforced);
        assert!(!store.visibility_enforced());
        assert!(!store.capabilities().visibility_enforced);
        // ...and the sidecar is what carries it.
        let sidecar = state
            .objects
            .read()
            .await
            .get("_meta/uploads/pub.txt.visibility.json")
            .cloned()
            .expect("sidecar written");
        assert!(String::from_utf8_lossy(&sidecar).contains("\"public\""));
    }

    #[tokio::test]
    async fn set_visibility_uses_put_object_acl_and_degrades_once() {
        let (endpoint, state) = start_mock_server().await;
        let healthy = store(&endpoint).await;
        put_text(&healthy, "uploads/a.txt", b"a").await;

        let meta = healthy
            .set_visibility(&key("uploads/a.txt"), Visibility::Public)
            .await
            .unwrap();
        assert!(meta.visibility.is_public());
        assert!(meta.visibility_enforced);
        assert_eq!(
            state.acl("uploads/a.txt").await.as_deref(),
            Some("public-read")
        );

        // Second call on a degraded bucket records instead of ACLing.
        let (endpoint, state) = start_mock_server().await;
        state.reject_acls();
        let degraded = store(&endpoint).await;
        put_text(&degraded, "uploads/b.txt", b"b").await;

        let meta = degraded
            .set_visibility(&key("uploads/b.txt"), Visibility::Public)
            .await
            .unwrap();
        assert!(meta.visibility.is_public());
        assert!(!meta.visibility_enforced);
        assert!(state.acl("uploads/b.txt").await.is_none());

        // And a subsequent set is recorded too, without touching the ACL path.
        let meta = degraded
            .set_visibility(&key("uploads/b.txt"), Visibility::Private)
            .await
            .unwrap();
        assert_eq!(meta.visibility, Visibility::Private);
        assert!(!meta.visibility_enforced);
    }

    #[tokio::test]
    async fn set_visibility_on_missing_object_is_not_found() {
        let (endpoint, _state) = start_mock_server().await;
        let store = store(&endpoint).await;
        assert_eq!(
            store
                .set_visibility(&key("uploads/none.txt"), Visibility::Public)
                .await
                .unwrap_err()
                .code(),
            "asset_not_found"
        );
    }

    #[tokio::test]
    async fn put_file_streams_small_and_multipart_paths() {
        let (endpoint, state) = start_mock_server().await;
        let store = store(&endpoint).await;
        let dir = tempfile::tempdir().unwrap();

        let small = dir.path().join("small.bin");
        std::fs::write(&small, vec![7u8; 4096]).unwrap();
        let meta = store
            .put_file(PutRequest::from_file(
                key("uploads/small.bin"),
                small.clone(),
                ContentType::default(),
            ))
            .await
            .unwrap();
        assert_eq!(meta.size_bytes, 4096);

        // Above MULTIPART_THRESHOLD the adapter must use the multipart API.
        let big = dir.path().join("big.bin");
        std::fs::write(&big, vec![9u8; crate::s3::MULTIPART_THRESHOLD + 1]).unwrap();
        let meta = store
            .put_file(PutRequest::from_file(
                key("uploads/big.bin"),
                big,
                ContentType::default(),
            ))
            .await
            .unwrap();
        assert_eq!(meta.size_bytes, (crate::s3::MULTIPART_THRESHOLD + 1) as u64);
        assert!(state.multipart_uploads.read().await.is_empty(), "no leak");
    }

    #[tokio::test]
    async fn download_to_streams_to_disk() {
        let (endpoint, _state) = start_mock_server().await;
        let store = store(&endpoint).await;
        let payload: Vec<u8> = (0..(64 * 1024)).map(|i| (i % 251) as u8).collect();
        put_text(&store, "uploads/blob.bin", &payload).await;

        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("nested/out.bin");
        let meta = store
            .download_to(&key("uploads/blob.bin"), &dest, None)
            .await
            .unwrap();
        assert_eq!(meta.size_bytes, payload.len() as u64);
        assert_eq!(std::fs::read(&dest).unwrap(), payload);
        let leftovers: Vec<String> = std::fs::read_dir(dir.path().join("nested"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with('.'))
            .collect();
        assert!(leftovers.is_empty(), "temp files left: {leftovers:?}");
    }

    #[tokio::test]
    async fn list_returns_a_page_with_a_continuation_cursor() {
        let (endpoint, _state) = start_mock_server().await;
        let store = store(&endpoint).await;
        for name in ["a", "b", "c"] {
            put_text(&store, &format!("uploads/{name}.txt"), name.as_bytes()).await;
        }
        put_text(&store, "other/z.txt", b"z").await;

        let page = store
            .list(
                ListQuery::new(crate::assets::AssetPrefix::parse("uploads/").unwrap())
                    .with_limit(2),
            )
            .await
            .unwrap();
        assert_eq!(page.items.len(), 2);
        assert!(page.truncated);
        assert_eq!(page.items[0].key.as_str(), "uploads/a.txt");

        let next = store
            .list(
                ListQuery::new(crate::assets::AssetPrefix::parse("uploads/").unwrap())
                    .with_limit(2)
                    .with_cursor(page.next_cursor.unwrap()),
            )
            .await
            .unwrap();
        assert_eq!(next.items.len(), 1);
        assert_eq!(next.items[0].key.as_str(), "uploads/c.txt");
        assert!(!next.truncated);
    }

    #[tokio::test]
    async fn list_with_include_meta_returns_sidecars() {
        let (endpoint, _state) = start_mock_server().await;
        let store = store(&endpoint).await;
        put_text(&store, "uploads/a.txt", b"a").await;
        store
            .set_visibility(&key("uploads/a.txt"), Visibility::Public)
            .await
            .unwrap();

        let hidden = store
            .list(ListQuery::new(
                crate::assets::AssetPrefix::parse("uploads/").unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(hidden.items.len(), 1);

        let shown = store
            .list(
                ListQuery::new(crate::assets::AssetPrefix::parse("uploads/").unwrap())
                    .with_include_meta(true),
            )
            .await
            .unwrap();
        assert!(
            shown.items.iter().any(|m| m.key.is_physical_meta()),
            "sidecar missing from {:?}",
            shown
                .items
                .iter()
                .map(|m| m.key.to_string())
                .collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn key_prefix_namespaces_objects() {
        let (endpoint, state) = start_mock_server().await;
        let mut store = store(&endpoint).await;
        store.key_prefix = "team/".to_owned();

        put_text(&store, "a.txt", b"a").await;
        assert_eq!(state.object_keys().await, vec!["team/a.txt".to_owned()]);
        assert!(store.exists(&key("a.txt")).await.unwrap());
        assert_eq!(
            store.public_url(&key("a.txt")).as_deref(),
            Some(format!("{endpoint}/test-bucket/team/a.txt").as_str())
        );
    }

    #[tokio::test]
    async fn max_object_bytes_is_enforced_before_upload() {
        let (endpoint, state) = start_mock_server().await;
        let mut store = store(&endpoint).await;
        store.max_object_bytes = Some(4);
        let err = store
            .put(PutRequest::from_bytes(
                key("uploads/a.bin"),
                vec![0u8; 5],
                ContentType::default(),
            ))
            .await
            .unwrap_err();
        assert_eq!(err.code(), "asset_object_too_large");
        assert!(state.object_keys().await.is_empty());
    }

    #[tokio::test]
    async fn presign_bounds_and_shape() {
        let (endpoint, _state) = start_mock_server().await;
        let store = store(&endpoint).await;

        let url = store
            .presign_get(&key("uploads/a.txt"), Duration::from_secs(60))
            .await
            .unwrap();
        assert!(!url.emulated);
        assert_eq!(url.method, PresignMethod::Get);
        assert!(url.url.contains("X-Amz-Signature"), "{}", url.url);
        assert!(url.url.contains("uploads/a.txt"), "{}", url.url);

        let put = store
            .presign_put(
                &key("uploads/a.txt"),
                &ContentType::parse("text/plain").unwrap(),
                Duration::from_secs(60),
            )
            .await
            .unwrap();
        assert_eq!(put.method, PresignMethod::Put);
        assert_eq!(put.content_type.as_deref(), Some("text/plain"));

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
    async fn presign_without_static_credentials_is_unsupported() {
        let (endpoint, _state) = start_mock_server().await;
        let mut store = store(&endpoint).await;
        store.credentials = None;
        let err = store
            .presign_get(&key("uploads/a.txt"), Duration::from_secs(60))
            .await
            .unwrap_err();
        assert_eq!(err.code(), "asset_unsupported");
        assert!(err.to_string().contains("static credentials"), "{err}");
    }

    #[tokio::test]
    async fn health_probes_and_cleans_up() {
        let (endpoint, state) = start_mock_server().await;
        let store = store(&endpoint).await;
        let status = store.health().await.unwrap();
        assert!(status.is_healthy(), "{status:?}");
        assert_eq!(status.backend, BackendKind::S3);
        assert!(
            state.object_keys().await.is_empty(),
            "probe object must be removed"
        );
    }

    #[tokio::test]
    async fn capability_matrix_matches_the_contract() {
        let (endpoint, _state) = start_mock_server().await;
        let store = store(&endpoint).await;
        let caps = store.capabilities();
        assert_eq!(caps, BackendCapabilities::S3);
        for op in [
            AssetOperation::Put,
            AssetOperation::PutFile,
            AssetOperation::Get,
            AssetOperation::DownloadTo,
            AssetOperation::Exists,
            AssetOperation::Delete,
            AssetOperation::List,
            AssetOperation::PresignGet,
            AssetOperation::PresignPut,
            AssetOperation::SetVisibility,
        ] {
            assert!(caps.supports(op), "s3 must support {op}");
        }
        assert!(caps.visibility_enforced);
    }

    #[test]
    fn acl_not_supported_code_is_the_service_code() {
        assert_eq!(ACL_NOT_SUPPORTED_CODE, "AccessControlListNotSupported");
    }

    #[tokio::test]
    async fn store_is_send_and_shareable() {
        let (endpoint, _state) = start_mock_server().await;
        let store: crate::assets::SharedAssetStore = std::sync::Arc::new(store(&endpoint).await);
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
