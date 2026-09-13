use std::collections::HashSet;
use std::path::Path;

use anyhow::Context;
use futures::StreamExt;

/// Map an S3 HeadObject error to NotFound / Unauthorized / ProbeFailed.
/// `ServiceError` and `ResponseError` both carry a status; everything else
/// (construction, dispatch, timeout) is transient.
fn classify_head_error<E>(err: &aws_sdk_s3::error::SdkError<E>) -> HeadOutcome
where
    E: std::fmt::Debug,
{
    use aws_sdk_s3::error::SdkError;
    let status = match err {
        SdkError::ServiceError(ctx) => Some(ctx.raw().status().as_u16()),
        SdkError::ResponseError(ctx) => Some(ctx.raw().status().as_u16()),
        _ => None,
    };
    match status {
        Some(404) => HeadOutcome::NotFound,
        Some(401) | Some(403) => HeadOutcome::Unauthorized,
        _ => HeadOutcome::ProbeFailed,
    }
}

#[derive(Debug)]
enum HeadOutcome {
    NotFound,
    Unauthorized,
    ProbeFailed,
}

/// Some S3-compatible endpoints reject single PutObject chunks above 16 MiB.
/// Use multipart upload with 8 MiB parts to stay within that limit.
pub(crate) const MULTIPART_THRESHOLD: usize = 8 * 1024 * 1024;
pub(crate) const MULTIPART_PART_SIZE: usize = 8 * 1024 * 1024;

/// Parse credential content (JSON or INI format) into the three AWS fields.
///
/// The single parsing entry point: `parse_aws_credentials` (SDK credentials)
/// and [`static_credentials_from_content`] (presign credentials) both build on
/// it, so the accepted formats cannot drift apart.
pub(crate) fn parse_credential_fields(
    content: &str,
) -> anyhow::Result<(String, String, Option<String>)> {
    #[derive(serde::Deserialize)]
    struct JsonCreds {
        aws_access_key_id: String,
        aws_secret_access_key: String,
        #[serde(default)]
        aws_session_token: Option<String>,
    }

    if let Ok(parsed) = serde_json::from_str::<JsonCreds>(content) {
        return Ok((
            parsed.aws_access_key_id,
            parsed.aws_secret_access_key,
            parsed.aws_session_token,
        ));
    }

    let strip_comment = |v: &str| {
        v.split_once('#')
            .map_or(v, |(before, _)| before)
            .trim()
            .to_owned()
    };
    let mut key_id = None;
    let mut secret = None;
    let mut token = None;
    for line in content.lines() {
        if let Some((k, v)) = line.split_once('=') {
            match k.trim() {
                "aws_access_key_id" => key_id = Some(strip_comment(v)),
                "aws_secret_access_key" => secret = Some(strip_comment(v)),
                "aws_session_token" => token = Some(strip_comment(v)),
                _ => {}
            }
        }
    }

    match (key_id, secret) {
        (Some(k), Some(s)) => Ok((k, s, token)),
        _ => anyhow::bail!(
            "AWS credentials are neither valid JSON \
             nor contain aws_access_key_id and aws_secret_access_key"
        ),
    }
}

/// Parse credential content (JSON or INI format) into AWS SDK credentials.
fn parse_aws_credentials(content: &str) -> anyhow::Result<aws_sdk_s3::config::Credentials> {
    let (key_id, secret, token) = parse_credential_fields(content)?;
    Ok(aws_sdk_s3::config::Credentials::new(
        &key_id,
        &secret,
        token,
        None,
        "grok-shell-trace-upload",
    ))
}

/// Build [`S3StaticCredentials`] from the same content formats
/// [`parse_aws_credentials`] accepts. Used to presign without rebuilding a
/// credential chain.
pub(crate) fn static_credentials_from_content(
    content: &str,
) -> anyhow::Result<S3StaticCredentials> {
    let (access_key_id, secret_access_key, _token) = parse_credential_fields(content)?;
    Ok(S3StaticCredentials {
        access_key_id,
        secret_access_key,
    })
}

/// Resolve credential *content* from the inline value or a file.
///
/// Inline wins; a missing file is an error; neither yields `None` so the SDK's
/// ambient chain stays in charge.
pub(crate) async fn resolve_credentials_content(
    credentials_content: Option<&str>,
    credentials_file: Option<&str>,
) -> anyhow::Result<Option<String>> {
    match (credentials_content, credentials_file) {
        (Some(inline), _) => Ok(Some(inline.to_owned())),
        (None, Some(path)) => {
            Ok(Some(tokio::fs::read_to_string(path).await.with_context(
                || format!("Failed to read AWS credentials file: {path}"),
            )?))
        }
        (None, None) => Ok(None),
    }
}

/// Build an S3 client. Uses path-style addressing when `endpoint_url` is set.
///
/// Reads `HTTPS_PROXY` / `HTTP_PROXY` / `ALL_PROXY` / `NO_PROXY` environment
/// variables so that S3 traffic can route through a corporate HTTP proxy when
/// the S3-compatible endpoint is not directly reachable.
pub(crate) async fn build_s3_client(
    region: &str,
    credentials_content: Option<&str>,
    credentials_file: Option<&str>,
    endpoint_url: Option<&str>,
) -> anyhow::Result<aws_sdk_s3::Client> {
    let proxy_config = aws_smithy_http_client::proxy::ProxyConfig::from_env();
    let http_client = aws_smithy_http_client::Builder::new().build_with_connector_fn(
        move |settings, _runtime_components| {
            let mut builder =
                aws_smithy_http_client::Connector::builder().proxy_config(proxy_config.clone());
            if let Some(s) = settings {
                builder.set_connector_settings(Some(s.clone()));
            }
            builder
                .tls_provider(aws_smithy_http_client::tls::Provider::Rustls(
                    aws_smithy_http_client::tls::rustls_provider::CryptoMode::Ring,
                ))
                .build()
        },
    );

    let mut config_loader = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .http_client(http_client)
        .region(aws_config::Region::new(region.to_owned()));

    let resolved_content =
        resolve_credentials_content(credentials_content, credentials_file).await?;

    if let Some(ref content) = resolved_content {
        config_loader = config_loader.credentials_provider(parse_aws_credentials(content)?);
    } else if endpoint_url.is_some() {
        config_loader = config_loader.credentials_provider(aws_sdk_s3::config::Credentials::new(
            "test",
            "test",
            None,
            None,
            "grok-shell-test",
        ));
    }

    let sdk_config = config_loader.load().await;
    let mut builder =
        aws_sdk_s3::config::Builder::from(&sdk_config).force_path_style(endpoint_url.is_some());
    if let Some(url) = endpoint_url {
        builder = builder.endpoint_url(url);
    }
    Ok(aws_sdk_s3::Client::from_conf(builder.build()))
}

/// Static access-key credentials for presigning S3 URLs.
///
/// `Debug` is intentionally redacted — the struct holds plaintext secrets.
#[derive(Clone)]
pub struct S3StaticCredentials {
    pub access_key_id: String,
    pub secret_access_key: String,
}

impl std::fmt::Debug for S3StaticCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("S3StaticCredentials")
            .field("access_key_id", &"[redacted]")
            .field("secret_access_key", &"[redacted]")
            .finish()
    }
}

impl S3StaticCredentials {
    fn to_credentials_content(&self) -> String {
        serde_json::json!({
            "aws_access_key_id": self.access_key_id,
            "aws_secret_access_key": self.secret_access_key,
        })
        .to_string()
    }
}

pub async fn presign_put_url(
    region: &str,
    endpoint_url: Option<&str>,
    creds: &S3StaticCredentials,
    bucket: &str,
    key: &str,
    content_type: &str,
    expires_in: std::time::Duration,
) -> anyhow::Result<String> {
    let content = creds.to_credentials_content();
    let client = build_s3_client(region, Some(&content), None, endpoint_url).await?;
    let presigning_config = aws_sdk_s3::presigning::PresigningConfig::expires_in(expires_in)?;
    let presigned = client
        .put_object()
        .bucket(bucket)
        .key(key)
        .content_type(content_type)
        .presigned(presigning_config)
        .await?;
    Ok(presigned.uri().to_string())
}

pub async fn presign_get_url(
    region: &str,
    endpoint_url: Option<&str>,
    creds: &S3StaticCredentials,
    bucket: &str,
    key: &str,
    expires_in: std::time::Duration,
) -> anyhow::Result<String> {
    let content = creds.to_credentials_content();
    let client = build_s3_client(region, Some(&content), None, endpoint_url).await?;
    let presigning_config = aws_sdk_s3::presigning::PresigningConfig::expires_in(expires_in)?;
    let presigned = client
        .get_object()
        .bucket(bucket)
        .key(key)
        .presigned(presigning_config)
        .await?;
    Ok(presigned.uri().to_string())
}

// ---------------------------------------------------------------------------
// Object operations used by the asset store
// ---------------------------------------------------------------------------

/// How an S3 SDK call failed, in the shape the asset layer branches on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum S3Failure {
    NotFound,
    Unauthorized,
    AccessDenied,
    /// The bucket rejects `x-amz-acl` (bucket-owner-enforced).
    AclNotSupported,
    /// A retryable transport / 5xx failure.
    Transient(String),
    /// Anything else, already rendered secret-free by `Display`.
    Other(String),
}

/// Error code emitted by S3 when a bucket-owner-enforced bucket rejects an ACL.
pub(crate) const ACL_NOT_SUPPORTED_CODE: &str = "AccessControlListNotSupported";

/// Map an SDK error to an [`S3Failure`].
///
/// 404 → `NotFound`, 401 → `Unauthorized`, 403 → `AccessDenied`; everything
/// without a status (construction, dispatch, timeout) is `Transient`.
pub(crate) fn classify_sdk_error<E>(err: &aws_sdk_s3::error::SdkError<E>) -> S3Failure
where
    E: aws_sdk_s3::error::ProvideErrorMetadata + std::fmt::Debug,
{
    use aws_sdk_s3::error::SdkError;

    let status = match err {
        SdkError::ServiceError(ctx) => Some(ctx.raw().status().as_u16()),
        SdkError::ResponseError(ctx) => Some(ctx.raw().status().as_u16()),
        _ => None,
    };

    // The ACL rejection is a 400 with a specific code; check it before the
    // generic status mapping so the degrade path can trigger.
    if is_acl_not_supported(err) {
        return S3Failure::AclNotSupported;
    }

    match status {
        Some(404) => S3Failure::NotFound,
        Some(401) => S3Failure::Unauthorized,
        Some(403) => S3Failure::AccessDenied,
        Some(code) if (500..600).contains(&code) => S3Failure::Transient(format!("HTTP {code}")),
        Some(code) => S3Failure::Other(format!("HTTP {code}")),
        None => S3Failure::Transient(format!("{err:?}")),
    }
}

/// True when the error is S3's "this bucket does not support ACLs" rejection.
///
/// Checks the parsed error code first and falls back to the rendered error,
/// which carries the metadata even when the body could not be modeled.
pub(crate) fn is_acl_not_supported<E>(err: &aws_sdk_s3::error::SdkError<E>) -> bool
where
    E: aws_sdk_s3::error::ProvideErrorMetadata + std::fmt::Debug,
{
    if let Some(code) = err.as_service_error().and_then(|e| e.code())
        && code == ACL_NOT_SUPPORTED_CODE
    {
        return true;
    }
    format!("{err:?}").contains(ACL_NOT_SUPPORTED_CODE)
}

/// `PutObject` with an optional canned ACL.
///
/// `content_length` is required when the body cannot report its own size (a
/// streamed file); S3 rejects a chunked PUT without `Content-Length`.
pub(crate) async fn put_object(
    client: &aws_sdk_s3::Client,
    bucket: &str,
    key: &str,
    body: aws_sdk_s3::primitives::ByteStream,
    content_type: &str,
    content_length: Option<u64>,
    acl: Option<aws_sdk_s3::types::ObjectCannedAcl>,
) -> Result<(), S3Failure> {
    let mut request = client
        .put_object()
        .bucket(bucket)
        .key(key)
        .content_type(content_type)
        .body(body);
    if let Some(length) = content_length {
        request = request.content_length(length as i64);
    }
    if let Some(acl) = acl {
        request = request.acl(acl);
    }
    request.send().await.map(|_| ()).map_err(|e| {
        let failure = classify_sdk_error(&e);
        tracing::debug!(key, ?failure, "s3 put_object failed");
        failure
    })
}

/// `PutObjectAcl` — the emulated visibility path for S3.
pub(crate) async fn put_object_acl(
    client: &aws_sdk_s3::Client,
    bucket: &str,
    key: &str,
    acl: aws_sdk_s3::types::ObjectCannedAcl,
) -> Result<(), S3Failure> {
    client
        .put_object_acl()
        .bucket(bucket)
        .key(key)
        .acl(acl)
        .send()
        .await
        .map(|_| ())
        .map_err(|e| classify_sdk_error(&e))
}

/// `DeleteObject`. S3 returns 204 even for a missing key, so this is
/// idempotent by construction.
pub(crate) async fn delete_object(
    client: &aws_sdk_s3::Client,
    bucket: &str,
    key: &str,
) -> Result<(), S3Failure> {
    client
        .delete_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await
        .map(|_| ())
        .map_err(|e| classify_sdk_error(&e))
}

/// One page of `ListObjectsV2`.
#[derive(Debug, Default)]
pub(crate) struct ListObjectsPage {
    /// `(key, size_bytes)` pairs, in service order.
    pub keys: Vec<(String, u64)>,
    pub next_continuation_token: Option<String>,
    pub is_truncated: bool,
}

/// `ListObjectsV2` with prefix, page size, and continuation token.
pub(crate) async fn list_objects_v2(
    client: &aws_sdk_s3::Client,
    bucket: &str,
    prefix: Option<&str>,
    max_keys: i32,
    continuation_token: Option<&str>,
) -> Result<ListObjectsPage, S3Failure> {
    let mut request = client.list_objects_v2().bucket(bucket).max_keys(max_keys);
    if let Some(prefix) = prefix {
        request = request.prefix(prefix);
    }
    if let Some(token) = continuation_token {
        request = request.continuation_token(token);
    }
    let output = request.send().await.map_err(|e| classify_sdk_error(&e))?;

    let keys = output
        .contents()
        .iter()
        .filter_map(|object| {
            let key = object.key()?.to_owned();
            Some((key, object.size().unwrap_or(0).max(0) as u64))
        })
        .collect();

    Ok(ListObjectsPage {
        keys,
        next_continuation_token: output.next_continuation_token().map(str::to_owned),
        is_truncated: output.is_truncated().unwrap_or(false),
    })
}

/// `HeadBucket` — a cheap reachability probe that needs no list permission.
pub(crate) async fn head_bucket(
    client: &aws_sdk_s3::Client,
    bucket: &str,
) -> Result<(), S3Failure> {
    client
        .head_bucket()
        .bucket(bucket)
        .send()
        .await
        .map(|_| ())
        .map_err(|e| classify_sdk_error(&e))
}

/// Metadata returned by [`head_object`].
#[derive(Debug, Clone, Default)]
pub(crate) struct HeadObjectInfo {
    pub content_length: u64,
    pub content_type: Option<String>,
    pub etag: Option<String>,
    /// RFC 3339 timestamp, when the service reported one.
    pub last_modified: Option<String>,
}

/// `HeadObject` — existence plus metadata, without transferring the body.
pub(crate) async fn head_object(
    client: &aws_sdk_s3::Client,
    bucket: &str,
    key: &str,
) -> Result<HeadObjectInfo, S3Failure> {
    let output = client
        .head_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await
        .map_err(|e| classify_sdk_error(&e))?;

    Ok(HeadObjectInfo {
        content_length: output.content_length().unwrap_or(0).max(0) as u64,
        content_type: output.content_type().map(str::to_owned),
        etag: output.e_tag().map(str::to_owned),
        last_modified: output.last_modified().map(|t| t.to_string()),
    })
}

/// `GetObject`, returning the raw output so the caller can stream the body.
pub(crate) async fn get_object(
    client: &aws_sdk_s3::Client,
    bucket: &str,
    key: &str,
) -> Result<aws_sdk_s3::operation::get_object::GetObjectOutput, S3Failure> {
    client
        .get_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await
        .map_err(|e| classify_sdk_error(&e))
}

/// `GetObject`, buffering the whole body.
pub(crate) async fn get_object_bytes(
    client: &aws_sdk_s3::Client,
    bucket: &str,
    key: &str,
) -> Result<bytes::Bytes, S3Failure> {
    let output = get_object(client, bucket, key).await?;
    output
        .body
        .collect()
        .await
        .map(|aggregated| aggregated.into_bytes())
        .map_err(|e| S3Failure::Transient(e.to_string()))
}

/// Multipart upload for payloads that exceed [`MULTIPART_THRESHOLD`].
///
/// Splits `content` into [`MULTIPART_PART_SIZE`] chunks and uploads each as a
/// separate part via the S3 multipart upload API. Aborts the upload on any
/// part failure so we don't leak incomplete multipart uploads.
async fn multipart_upload_bytes(
    client: &aws_sdk_s3::Client,
    bucket: &str,
    object_path: &str,
    content: &[u8],
    content_type: &str,
) -> anyhow::Result<()> {
    let create = client
        .create_multipart_upload()
        .bucket(bucket)
        .key(object_path)
        .content_type(content_type)
        .send()
        .await
        .with_context(|| {
            format!("Failed to create multipart upload for s3://{bucket}/{object_path}")
        })?;

    let upload_id = create
        .upload_id()
        .context("CreateMultipartUpload response missing upload_id")?
        .to_owned();

    let mut completed_parts = Vec::new();
    let mut offset = 0usize;
    let mut part_number = 1i32;

    let result: anyhow::Result<()> = async {
        while offset < content.len() {
            let end = (offset + MULTIPART_PART_SIZE).min(content.len());
            let chunk = &content[offset..end];

            let upload_part = client
                .upload_part()
                .bucket(bucket)
                .key(object_path)
                .upload_id(&upload_id)
                .part_number(part_number)
                .body(aws_sdk_s3::primitives::ByteStream::from(chunk.to_vec()))
                .send()
                .await
                .with_context(|| {
                    format!("Failed to upload part {part_number} for s3://{bucket}/{object_path}")
                })?;

            let etag = upload_part
                .e_tag()
                .context("UploadPart response missing ETag")?
                .to_owned();

            completed_parts.push(
                aws_sdk_s3::types::CompletedPart::builder()
                    .part_number(part_number)
                    .e_tag(etag)
                    .build(),
            );

            offset = end;
            part_number += 1;
        }

        let completed = aws_sdk_s3::types::CompletedMultipartUpload::builder()
            .set_parts(Some(completed_parts))
            .build();

        client
            .complete_multipart_upload()
            .bucket(bucket)
            .key(object_path)
            .upload_id(&upload_id)
            .multipart_upload(completed)
            .send()
            .await
            .with_context(|| {
                format!("Failed to complete multipart upload for s3://{bucket}/{object_path}")
            })?;

        Ok(())
    }
    .await;

    if result.is_err() {
        let _ = client
            .abort_multipart_upload()
            .bucket(bucket)
            .key(object_path)
            .upload_id(&upload_id)
            .send()
            .await;
    }

    result
}

/// Upload bytes to an S3-compatible bucket.
pub async fn upload_bytes(
    bucket: &str,
    object_path: &str,
    content: &[u8],
    content_type: &str,
    region: &str,
    credentials_content: Option<&str>,
    credentials_file: Option<&str>,
    endpoint_url: Option<&str>,
) -> anyhow::Result<String> {
    let client =
        build_s3_client(region, credentials_content, credentials_file, endpoint_url).await?;
    if content.len() >= MULTIPART_THRESHOLD {
        multipart_upload_bytes(&client, bucket, object_path, content, content_type).await?;
    } else {
        client
            .put_object()
            .bucket(bucket)
            .key(object_path)
            .content_type(content_type)
            .body(aws_sdk_s3::primitives::ByteStream::from(content.to_vec()))
            .send()
            .await
            .with_context(|| format!("Failed to upload to s3://{bucket}/{object_path}"))?;
    }
    Ok(format!("s3://{bucket}/{object_path}"))
}

/// Upload a file to an S3-compatible bucket by streaming from disk.
pub async fn upload_file(
    bucket: &str,
    object_path: &str,
    file_path: &Path,
    content_type: &str,
    region: &str,
    credentials_content: Option<&str>,
    credentials_file: Option<&str>,
    endpoint_url: Option<&str>,
) -> anyhow::Result<String> {
    let client =
        build_s3_client(region, credentials_content, credentials_file, endpoint_url).await?;
    let file_size = tokio::fs::metadata(file_path)
        .await
        .map(|m| m.len() as usize)
        .unwrap_or(0);
    if file_size >= MULTIPART_THRESHOLD {
        let content = tokio::fs::read(file_path).await.with_context(|| {
            format!("Failed to read file for S3 upload: {}", file_path.display())
        })?;
        multipart_upload_bytes(&client, bucket, object_path, &content, content_type).await?;
    } else {
        let body = aws_sdk_s3::primitives::ByteStream::from_path(file_path)
            .await
            .with_context(|| {
                format!("Failed to open file for S3 upload: {}", file_path.display())
            })?;
        client
            .put_object()
            .bucket(bucket)
            .key(object_path)
            .content_type(content_type)
            .body(body)
            .send()
            .await
            .with_context(|| format!("Failed to upload to s3://{bucket}/{object_path}"))?;
    }
    Ok(format!("s3://{bucket}/{object_path}"))
}

/// Upload an async reader to S3.
///
/// Buffers the reader into memory before uploading because S3 PutObject
/// requires a known Content-Length. For the primary caller (zstd-compressed
/// dedup blobs from the upload queue), the compressed output is typically
/// small enough that buffering is acceptable.
pub async fn upload_stream<R: tokio::io::AsyncRead + Send + Sync + 'static>(
    bucket: &str,
    object_path: &str,
    reader: R,
    content_type: &str,
    region: &str,
    credentials_content: Option<&str>,
    credentials_file: Option<&str>,
    endpoint_url: Option<&str>,
) -> anyhow::Result<String> {
    use tokio::io::AsyncReadExt;

    let mut buf = Vec::new();
    tokio::pin!(reader);
    reader.read_to_end(&mut buf).await?;

    upload_bytes(
        bucket,
        object_path,
        &buf,
        content_type,
        region,
        credentials_content,
        credentials_file,
        endpoint_url,
    )
    .await
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // Used once the S3 storage backend is wired up.
pub struct S3ExistsResponse {
    pub bucket: String,
    pub path: String,
    pub size: i64,
}

/// S3-native storage client providing batch operations via concurrent SDK calls.
///
/// Caches the AWS SDK `Client` for the lifetime of the struct, avoiding the
/// per-call `build_s3_client()` overhead.
#[allow(dead_code)] // Used once the S3 storage backend is wired up.
pub struct S3StorageClient {
    client: aws_sdk_s3::Client,
    bucket: String,
}

#[allow(dead_code)] // Used once the S3 storage backend is wired up.
impl S3StorageClient {
    pub fn bucket_name(&self) -> &str {
        &self.bucket
    }

    pub async fn new(
        bucket: String,
        region: &str,
        credentials_content: Option<&str>,
        credentials_file: Option<&str>,
        endpoint_url: Option<&str>,
    ) -> anyhow::Result<Self> {
        let client =
            build_s3_client(region, credentials_content, credentials_file, endpoint_url).await?;
        tracing::debug!(bucket = %bucket, region, endpoint = ?endpoint_url, "S3StorageClient created");
        Ok(Self { client, bucket })
    }

    /// Check existence of multiple S3 objects via concurrent HeadObject calls.
    ///
    /// Aggregates per-key outcomes worst-first: 401/403 → `Unauthorized`,
    /// non-404 transient → `ProbeFailed`, all-404 → `NotFound`, else `Found`.
    pub async fn batch_check_exists<S: AsRef<str>>(
        &self,
        paths: &[S],
    ) -> crate::storage_client::ExistsResult<HashSet<String>> {
        use crate::storage_client::ExistsResult;
        let client = &self.client;
        let bucket = &*self.bucket;

        // Collect into owned strings up front: HeadObject requires owned keys
        // and this keeps closures HRTB-clean for callers passing both
        // `&[String]` and `&[&str]` from the same async fn.
        let owned_paths: Vec<String> = paths
            .iter()
            .map(|p| <S as AsRef<str>>::as_ref(p).to_string())
            .collect();
        let total = owned_paths.len();

        #[derive(Debug)]
        enum PathOutcome {
            Exists(String),
            NotFound,
            Unauthorized,
            ProbeFailed,
        }

        let outcomes: Vec<PathOutcome> = futures::stream::iter(owned_paths)
            .map(|path| async move {
                match client.head_object().bucket(bucket).key(&path).send().await {
                    Ok(_) => PathOutcome::Exists(path),
                    Err(e) => match classify_head_error(&e) {
                        HeadOutcome::NotFound => PathOutcome::NotFound,
                        HeadOutcome::Unauthorized => PathOutcome::Unauthorized,
                        HeadOutcome::ProbeFailed => PathOutcome::ProbeFailed,
                    },
                }
            })
            .buffer_unordered(32)
            .collect()
            .await;

        let mut found = HashSet::new();
        let mut not_found_count: usize = 0;
        let mut any_unauthorized = false;
        let mut any_transient = false;
        for o in outcomes {
            match o {
                PathOutcome::Exists(p) => {
                    found.insert(p);
                }
                PathOutcome::NotFound => not_found_count += 1,
                PathOutcome::Unauthorized => any_unauthorized = true,
                PathOutcome::ProbeFailed => any_transient = true,
            }
        }
        if any_unauthorized {
            ExistsResult::Unauthorized
        } else if any_transient {
            ExistsResult::ProbeFailed
        } else if total > 0 && not_found_count == total {
            // Symmetric with the proxy: all-404 batch surfaces as NotFound,
            // not Found(empty_set).
            ExistsResult::NotFound
        } else {
            ExistsResult::Found(found)
        }
    }

    /// Upload multiple small files via concurrent PutObject calls.
    ///
    /// Returns the proxy-compatible `BatchUploadResult` type directly so
    /// downstream result-handling code stays unchanged.
    pub async fn batch_upload(
        &self,
        files: Vec<(String, Vec<u8>, String)>,
    ) -> Option<Vec<prod_mc_cli_chat_proxy_types::BatchUploadResult>> {
        let client = &self.client;
        let bucket = &*self.bucket;
        let results: Vec<prod_mc_cli_chat_proxy_types::BatchUploadResult> = futures::stream::iter(
            files,
        )
        .map(|(path, content, content_type)| async move {
            let size = content.len() as i64;
            let bucket_owned = bucket.to_string();
            let upload_result = if content.len() >= MULTIPART_THRESHOLD {
                multipart_upload_bytes(client, bucket, &path, &content, &content_type).await
            } else {
                client
                    .put_object()
                    .bucket(bucket)
                    .key(&path)
                    .content_type(&content_type)
                    .body(aws_sdk_s3::primitives::ByteStream::from(content))
                    .send()
                    .await
                    .map(|_| ())
                    .map_err(|e| anyhow::anyhow!(e))
            };
            match upload_result {
                Ok(_) => prod_mc_cli_chat_proxy_types::BatchUploadResult {
                    path,
                    bucket: Some(bucket_owned),
                    status: prod_mc_cli_chat_proxy_types::BatchUploadStatus::Ok,
                    size: Some(size),
                    generation: None,
                    error: None,
                },
                Err(e) => {
                    let error_msg = format!("{:#}", e);
                    tracing::warn!(path = %path, error = %error_msg, "S3 batch upload item failed");
                    prod_mc_cli_chat_proxy_types::BatchUploadResult {
                        path,
                        bucket: Some(bucket_owned),
                        status: prod_mc_cli_chat_proxy_types::BatchUploadStatus::Error,
                        size: None,
                        generation: None,
                        error: Some(error_msg),
                    }
                }
            }
        })
        .buffer_unordered(16)
        .collect()
        .await;

        Some(results)
    }

    /// Check if a single object exists via HeadObject.
    pub async fn check_exists(&self, path: &str) -> Option<S3ExistsResponse> {
        match self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(path)
            .send()
            .await
        {
            Ok(output) => Some(S3ExistsResponse {
                bucket: self.bucket.clone(),
                path: path.to_string(),
                size: output.content_length().unwrap_or(0),
            }),
            Err(_) => None,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use axum::{
        Router,
        extract::Path as AxumPath,
        http::StatusCode,
        response::IntoResponse,
        routing::{get, head, put},
    };
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tokio::sync::RwLock;

    /// In-progress multipart uploads: upload_id -> (key, parts: part_number -> bytes).
    type MultipartUploads = HashMap<String, (String, HashMap<i32, Vec<u8>>)>;

    /// Shared state for the mock S3 server.
    pub(crate) struct MockS3State {
        pub(crate) objects: RwLock<HashMap<String, Vec<u8>>>,
        pub(crate) multipart_uploads: RwLock<MultipartUploads>,
        /// Canned ACLs written through `PutObjectAcl`, keyed by object key.
        pub(crate) acls: RwLock<HashMap<String, String>>,
        /// `x-amz-acl` header observed on the last `PutObject` per key.
        pub(crate) put_acl_headers: RwLock<HashMap<String, Option<String>>>,
        /// Content type stored with each object, echoed by HEAD/GET.
        pub(crate) content_types: RwLock<HashMap<String, String>>,
        /// When `false`, ACL-bearing requests are rejected with
        /// `AccessControlListNotSupported` — the bucket-owner-enforced bucket.
        pub(crate) acl_supported: AtomicBool,
    }

    impl MockS3State {
        fn new() -> Self {
            Self {
                objects: RwLock::new(HashMap::new()),
                multipart_uploads: RwLock::new(HashMap::new()),
                acls: RwLock::new(HashMap::new()),
                put_acl_headers: RwLock::new(HashMap::new()),
                content_types: RwLock::new(HashMap::new()),
                acl_supported: AtomicBool::new(true),
            }
        }

        /// Model a bucket that rejects `x-amz-acl`.
        pub(crate) fn reject_acls(&self) {
            self.acl_supported.store(false, Ordering::SeqCst);
        }

        /// `x-amz-acl` sent with the last `PutObject` for `key`.
        pub(crate) async fn put_acl_header(&self, key: &str) -> Option<String> {
            self.put_acl_headers
                .read()
                .await
                .get(key)
                .cloned()
                .flatten()
        }

        /// Canned ACL recorded for `key` through `PutObjectAcl`.
        pub(crate) async fn acl(&self, key: &str) -> Option<String> {
            self.acls.read().await.get(key).cloned()
        }

        /// Every stored object key, sorted.
        pub(crate) async fn object_keys(&self) -> Vec<String> {
            let mut keys: Vec<String> = self.objects.read().await.keys().cloned().collect();
            keys.sort();
            keys
        }
    }

    fn xml_response(status: u16, body: String) -> axum::response::Response {
        axum::http::Response::builder()
            .status(status)
            .header("content-type", "application/xml")
            .body(axum::body::Body::from(body))
            .unwrap()
            .into_response()
    }

    /// The ACL rejection a bucket-owner-enforced bucket returns.
    fn acl_not_supported_response() -> axum::response::Response {
        xml_response(
            400,
            format!(
                "<Error><Code>{ACL_NOT_SUPPORTED_CODE}</Code>\
                 <Message>The bucket does not support ACLs</Message></Error>"
            ),
        )
    }

    /// Decode `aws-chunked` framing, which the SDK uses when it streams an
    /// unknown-length body. Real S3 does the same when
    /// `x-amz-decoded-content-length` is present.
    fn decode_aws_chunked(body: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(body.len());
        let mut pos = 0usize;
        while pos < body.len() {
            let Some(newline) = body[pos..].iter().position(|b| *b == b'\n') else {
                break;
            };
            let line = String::from_utf8_lossy(&body[pos..pos + newline])
                .trim()
                .to_owned();
            pos += newline + 1;
            // Blank separators and trailer headers are not chunk sizes.
            let Some(size) = line
                .split(';')
                .next()
                .and_then(|s| usize::from_str_radix(s, 16).ok())
            else {
                continue;
            };
            if size == 0 {
                break;
            }
            if pos + size > body.len() {
                out.extend_from_slice(&body[pos..]);
                break;
            }
            out.extend_from_slice(&body[pos..pos + size]);
            pos += size;
        }
        out
    }

    /// True when the request carries `x-amz-acl` on a bucket that rejects it.
    fn acl_rejected(state: &MockS3State, headers: &axum::http::HeaderMap) -> bool {
        !state.acl_supported.load(Ordering::SeqCst) && headers.contains_key("x-amz-acl")
    }

    /// Mock S3 server with PUT/GET/HEAD/POST/DELETE, multipart, ACL, and
    /// ListObjectsV2 support.
    pub(crate) fn mock_s3_router() -> (Router, Arc<MockS3State>) {
        let state = Arc::new(MockS3State::new());

        let s = state.clone();
        let head_handler = move |AxumPath((_, key)): AxumPath<(String, String)>| {
            let state = s.clone();
            async move {
                match state.objects.read().await.get(&key) {
                    Some(data) => {
                        let content_type = state
                            .content_types
                            .read()
                            .await
                            .get(&key)
                            .cloned()
                            .unwrap_or_else(|| "application/octet-stream".to_owned());
                        (
                            StatusCode::OK,
                            [
                                ("content-length", data.len().to_string()),
                                ("content-type", content_type),
                            ],
                        )
                            .into_response()
                    }
                    None => StatusCode::NOT_FOUND.into_response(),
                }
            }
        };

        let s = state.clone();
        let put_handler = move |AxumPath((_, key)): AxumPath<(String, String)>,
                                query: axum::extract::Query<HashMap<String, String>>,
                                headers: axum::http::HeaderMap,
                                body: axum::body::Bytes| {
            let state = s.clone();
            async move {
                if query.contains_key("acl") {
                    if acl_rejected(&state, &headers) {
                        return acl_not_supported_response();
                    }
                    let acl = headers
                        .get("x-amz-acl")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("private")
                        .to_owned();
                    state.acls.write().await.insert(key, acl);
                    return StatusCode::OK.into_response();
                }

                if let (Some(pn), Some(uid)) = (query.get("partNumber"), query.get("uploadId")) {
                    let part_num: i32 = pn.parse().unwrap_or(0);
                    let mut uploads = state.multipart_uploads.write().await;
                    if let Some((_, parts)) = uploads.get_mut(uid) {
                        let stored = if headers.contains_key("x-amz-decoded-content-length") {
                            decode_aws_chunked(&body)
                        } else {
                            body.to_vec()
                        };
                        parts.insert(part_num, stored);
                        // UploadPart must echo an ETag; the SDK needs it for
                        // the CompleteMultipartUpload manifest.
                        return axum::http::Response::builder()
                            .status(200)
                            .header("content-type", "application/xml")
                            .header("etag", format!("\"part-{part_num}-etag\""))
                            .body(axum::body::Body::from("<UploadPartResult/>"))
                            .unwrap()
                            .into_response();
                    }
                    return StatusCode::NOT_FOUND.into_response();
                }

                let acl_header = headers
                    .get("x-amz-acl")
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_owned);
                state
                    .put_acl_headers
                    .write()
                    .await
                    .insert(key.clone(), acl_header);

                if acl_rejected(&state, &headers) {
                    return acl_not_supported_response();
                }
                if body.len() > 16 * 1024 * 1024 {
                    return (StatusCode::BAD_REQUEST, "chunk too big").into_response();
                }
                let content_type = headers
                    .get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("application/octet-stream")
                    .to_owned();
                state
                    .content_types
                    .write()
                    .await
                    .insert(key.clone(), content_type);
                let stored = if headers.contains_key("x-amz-decoded-content-length") {
                    decode_aws_chunked(&body)
                } else {
                    body.to_vec()
                };
                state.objects.write().await.insert(key, stored);
                StatusCode::OK.into_response()
            }
        };

        let s = state.clone();
        let get_handler = move |AxumPath((_, key)): AxumPath<(String, String)>| {
            let state = s.clone();
            async move {
                match state.objects.read().await.get(&key) {
                    Some(data) => (StatusCode::OK, data.clone()).into_response(),
                    None => StatusCode::NOT_FOUND.into_response(),
                }
            }
        };

        async fn head_bucket_handler() -> StatusCode {
            StatusCode::OK
        }

        let s = state.clone();
        let list_handler =
            move |AxumPath(_bucket): AxumPath<String>,
                  query: axum::extract::Query<HashMap<String, String>>| {
                let state = s.clone();
                async move {
                    let prefix = query.get("prefix").cloned().unwrap_or_default();
                    let max_keys: usize = query
                        .get("max-keys")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(1_000);
                    let start: usize = query
                        .get("continuation-token")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0);

                    let mut keys: Vec<String> = state
                        .objects
                        .read()
                        .await
                        .keys()
                        .filter(|k| k.starts_with(&prefix))
                        .cloned()
                        .collect();
                    keys.sort();

                    let page: Vec<String> =
                        keys.iter().skip(start).take(max_keys).cloned().collect();
                    let next = start + page.len();
                    let truncated = next < keys.len();

                    let mut body = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
                    body.push_str("<ListBucketResult>");
                    body.push_str(&format!("<Name>bucket</Name><Prefix>{prefix}</Prefix>"));
                    body.push_str(&format!(
                        "<KeyCount>{}</KeyCount><MaxKeys>{max_keys}</MaxKeys>",
                        page.len()
                    ));
                    body.push_str(&format!("<IsTruncated>{truncated}</IsTruncated>"));
                    for key in &page {
                        let size = state
                            .objects
                            .read()
                            .await
                            .get(key)
                            .map(|d| d.len())
                            .unwrap_or(0);
                        body.push_str(&format!(
                            "<Contents><Key>{key}</Key><Size>{size}</Size>\
                         <LastModified>2026-01-01T00:00:00.000Z</LastModified>\
                         <ETag>\"etag\"</ETag><StorageClass>STANDARD</StorageClass></Contents>"
                        ));
                    }
                    if truncated {
                        body.push_str(&format!(
                            "<NextContinuationToken>{next}</NextContinuationToken>"
                        ));
                    }
                    body.push_str("</ListBucketResult>");
                    xml_response(200, body)
                }
            };

        let s = state.clone();
        let post_handler = move |AxumPath((_, key)): AxumPath<(String, String)>,
                                 query: axum::extract::Query<HashMap<String, String>>,
                                 headers: axum::http::HeaderMap,
                                 body: axum::body::Bytes| {
            let state = s.clone();
            async move {
                if query.contains_key("uploads") {
                    if acl_rejected(&state, &headers) {
                        return acl_not_supported_response();
                    }
                    use std::sync::atomic::AtomicU64;
                    static CTR: AtomicU64 = AtomicU64::new(0);
                    let uid = format!("upload-{}", CTR.fetch_add(1, Ordering::Relaxed));
                    state
                        .multipart_uploads
                        .write()
                        .await
                        .insert(uid.clone(), (key.clone(), HashMap::new()));
                    return xml_response(
                        200,
                        format!(
                            "<InitiateMultipartUploadResult><UploadId>{uid}</UploadId></InitiateMultipartUploadResult>"
                        ),
                    );
                }
                if let Some(uid) = query.get("uploadId") {
                    let mut uploads = state.multipart_uploads.write().await;
                    if let Some((stored_key, parts)) = uploads.remove(uid) {
                        let mut sorted: Vec<_> = parts.into_iter().collect();
                        sorted.sort_by_key(|(n, _)| *n);
                        let combined: Vec<u8> = sorted.into_iter().flat_map(|(_, d)| d).collect();
                        state.objects.write().await.insert(stored_key, combined);
                        return xml_response(200, "<CompleteMultipartUploadResult/>".into());
                    }
                    return StatusCode::NOT_FOUND.into_response();
                }
                let _ = body;
                StatusCode::BAD_REQUEST.into_response()
            }
        };

        let s = state.clone();
        let delete_handler =
            move |AxumPath((_, key)): AxumPath<(String, String)>,
                  query: axum::extract::Query<HashMap<String, String>>| {
                let state = s.clone();
                async move {
                    if let Some(uid) = query.get("uploadId") {
                        state.multipart_uploads.write().await.remove(uid);
                    } else {
                        state.objects.write().await.remove(&key);
                        state.acls.write().await.remove(&key);
                    }
                    StatusCode::NO_CONTENT
                }
            };

        let router = Router::new()
            .route(
                "/{bucket}/{*key}",
                head(head_handler)
                    .put(put_handler)
                    .get(get_handler)
                    .post(post_handler)
                    .delete(delete_handler),
            )
            .route(
                "/{bucket}",
                get(list_handler.clone()).head(head_bucket_handler),
            )
            .route("/{bucket}/", get(list_handler))
            // 8 MiB multipart parts exceed axum's 2 MiB default body limit.
            .layer(axum::extract::DefaultBodyLimit::max(16 * 1024 * 1024));
        (router, state)
    }

    /// Start the mock server and return (endpoint_url, state).
    pub(crate) async fn start_mock_server() -> (String, Arc<MockS3State>) {
        let (router, state) = mock_s3_router();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        (format!("http://{addr}"), state)
    }

    /// A raw `aws_sdk_s3::Client` pointed at the mock, for adapter tests.
    pub(crate) async fn make_raw_test_client(endpoint_url: &str) -> aws_sdk_s3::Client {
        build_s3_client(
            "us-east-1",
            Some(r#"{"aws_access_key_id":"test","aws_secret_access_key":"test"}"#),
            None,
            Some(endpoint_url),
        )
        .await
        .unwrap()
    }

    /// Static credentials matching [`make_raw_test_client`].
    pub(crate) fn test_static_credentials() -> S3StaticCredentials {
        S3StaticCredentials {
            access_key_id: "test".to_owned(),
            secret_access_key: "test".to_owned(),
        }
    }

    async fn make_test_client(endpoint_url: &str) -> S3StorageClient {
        S3StorageClient::new(
            "test-bucket".to_string(),
            "us-east-1",
            Some(r#"{"aws_access_key_id":"test","aws_secret_access_key":"test"}"#),
            None,
            Some(endpoint_url),
        )
        .await
        .unwrap()
    }

    /// Start a mock server that rejects PUT for keys containing "fail".
    async fn start_mock_server_rejecting_fail_keys() -> String {
        let put_handler = move |AxumPath((_, key)): AxumPath<(String, String)>,
                                _body: axum::body::Bytes| async move {
            if key.contains("fail") {
                StatusCode::FORBIDDEN.into_response()
            } else {
                StatusCode::OK.into_response()
            }
        };

        let router = Router::new().route("/{bucket}/{*key}", put(put_handler));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        format!("http://{addr}")
    }

    fn unwrap_found(r: crate::storage_client::ExistsResult<HashSet<String>>) -> HashSet<String> {
        match r {
            crate::storage_client::ExistsResult::Found(s) => s,
            other => panic!("expected Found, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn batch_check_exists_returns_existing_paths() {
        let (endpoint, state) = start_mock_server().await;
        {
            let mut objects = state.objects.write().await;
            objects.insert("file-a.txt".into(), b"hello".to_vec());
            objects.insert("file-c.txt".into(), b"world".to_vec());
        }

        let client = make_test_client(&endpoint).await;
        let paths: Vec<String> = vec![
            "file-a.txt".into(),
            "file-b.txt".into(),
            "file-c.txt".into(),
        ];

        let result = unwrap_found(client.batch_check_exists(&paths).await);
        assert_eq!(result.len(), 2);
        assert!(result.contains("file-a.txt"));
        assert!(result.contains("file-c.txt"));
        assert!(!result.contains("file-b.txt"));
    }

    #[tokio::test]
    async fn batch_check_exists_empty_input() {
        let (endpoint, _state) = start_mock_server().await;
        let client = make_test_client(&endpoint).await;

        let paths: &[String] = &[];
        let result = unwrap_found(client.batch_check_exists(paths).await);
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn batch_check_exists_all_missing() {
        let (endpoint, _state) = start_mock_server().await;
        let client = make_test_client(&endpoint).await;
        let paths: Vec<String> = vec!["missing-1".into(), "missing-2".into()];

        // All-404 collapses to top-level NotFound, symmetric with the proxy.
        let result = client.batch_check_exists(&paths).await;
        assert!(
            matches!(result, crate::storage_client::ExistsResult::NotFound),
            "all-404 batch must map to NotFound, got {:?}",
            result
        );
    }

    /// Build a HEAD-only mock router that returns `responder(key)` per key.
    async fn start_head_mock<F>(responder: F) -> String
    where
        F: Fn(String) -> axum::http::StatusCode + Clone + Send + Sync + 'static,
    {
        use axum::routing::head as axum_head;

        let h = move |AxumPath((_, key)): AxumPath<(String, String)>| {
            let responder = responder.clone();
            async move {
                let code = responder(key);
                code.into_response()
            }
        };
        let router = Router::new().route("/{bucket}/{*key}", axum_head(h));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn batch_check_exists_403_maps_to_unauthorized() {
        let endpoint = start_head_mock(|_| axum::http::StatusCode::FORBIDDEN).await;
        let client = make_test_client(&endpoint).await;
        let paths: Vec<String> = vec!["any.txt".into()];
        let result = client.batch_check_exists(&paths).await;
        assert!(
            matches!(result, crate::storage_client::ExistsResult::Unauthorized),
            "403 HEAD must map to Unauthorized, got {:?}",
            result
        );
    }

    #[tokio::test]
    async fn batch_check_exists_401_maps_to_unauthorized() {
        let endpoint = start_head_mock(|_| axum::http::StatusCode::UNAUTHORIZED).await;
        let client = make_test_client(&endpoint).await;
        let paths: Vec<String> = vec!["any.txt".into()];
        let result = client.batch_check_exists(&paths).await;
        assert!(
            matches!(result, crate::storage_client::ExistsResult::Unauthorized),
            "401 HEAD must map to Unauthorized, got {:?}",
            result
        );
    }

    /// Mixed `[Found, NotFound, 5xx]` → `ProbeFailed`; transient dominates.
    #[tokio::test]
    async fn batch_check_exists_mixed_with_5xx_maps_to_probe_failed() {
        let endpoint = start_head_mock(|key| {
            if key == "exists.txt" {
                axum::http::StatusCode::OK
            } else if key == "missing.txt" {
                axum::http::StatusCode::NOT_FOUND
            } else {
                axum::http::StatusCode::INTERNAL_SERVER_ERROR
            }
        })
        .await;
        let client = make_test_client(&endpoint).await;
        let paths: Vec<String> = vec!["exists.txt".into(), "missing.txt".into(), "boom.txt".into()];
        let result = client.batch_check_exists(&paths).await;
        assert!(
            matches!(result, crate::storage_client::ExistsResult::ProbeFailed),
            "any 5xx must dominate, got {:?}",
            result
        );
    }

    /// Mixed `[Found, 5xx, 401]` → `Unauthorized`; auth outranks transient.
    #[tokio::test]
    async fn batch_check_exists_mixed_with_401_maps_to_unauthorized() {
        let endpoint = start_head_mock(|key| {
            if key == "exists.txt" {
                axum::http::StatusCode::OK
            } else if key == "transient.txt" {
                axum::http::StatusCode::BAD_GATEWAY
            } else {
                axum::http::StatusCode::UNAUTHORIZED
            }
        })
        .await;
        let client = make_test_client(&endpoint).await;
        let paths: Vec<String> = vec![
            "exists.txt".into(),
            "transient.txt".into(),
            "no-auth.txt".into(),
        ];
        let result = client.batch_check_exists(&paths).await;
        assert!(
            matches!(result, crate::storage_client::ExistsResult::Unauthorized),
            "any 401 must dominate transient, got {:?}",
            result
        );
    }

    #[tokio::test]
    async fn batch_check_exists_transient_error_maps_to_probe_failed() {
        // Spin up a server whose HEAD returns 500 for everything.
        use axum::http::StatusCode;
        use axum::routing::head as axum_head;

        let head_handler = move |AxumPath((_, _key)): AxumPath<(String, String)>| async move {
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        };
        let router = Router::new().route("/{bucket}/{*key}", axum_head(head_handler));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let endpoint = format!("http://{addr}");

        let client = make_test_client(&endpoint).await;
        let paths: Vec<String> = vec!["x".into()];
        let result = client.batch_check_exists(&paths).await;
        assert!(
            matches!(result, crate::storage_client::ExistsResult::ProbeFailed),
            "5xx HEAD must map to ProbeFailed, got {:?}",
            result
        );
    }

    #[tokio::test]
    async fn batch_upload_all_succeed() {
        let (endpoint, state) = start_mock_server().await;
        let client = make_test_client(&endpoint).await;

        let files = vec![
            (
                "upload-a.txt".into(),
                b"content-a".to_vec(),
                "text/plain".into(),
            ),
            (
                "upload-b.bin".into(),
                b"content-b".to_vec(),
                "application/octet-stream".into(),
            ),
        ];

        let results = client.batch_upload(files).await.unwrap();
        assert_eq!(results.len(), 2);
        for r in &results {
            assert_eq!(
                r.status,
                prod_mc_cli_chat_proxy_types::BatchUploadStatus::Ok
            );
            assert!(r.error.is_none());
            assert_eq!(r.bucket.as_deref(), Some("test-bucket"));
        }

        let objects = state.objects.read().await;
        assert_eq!(objects.get("upload-a.txt").unwrap(), b"content-a");
        assert_eq!(objects.get("upload-b.bin").unwrap(), b"content-b");
    }

    #[tokio::test]
    async fn batch_upload_reports_size() {
        let (endpoint, _state) = start_mock_server().await;
        let client = make_test_client(&endpoint).await;

        let content = b"twelve chars".to_vec();
        let expected_size = content.len() as i64;
        let files = vec![("sized.txt".into(), content, "text/plain".into())];

        let results = client.batch_upload(files).await.unwrap();
        assert_eq!(results[0].size, Some(expected_size));
    }

    #[tokio::test]
    async fn batch_upload_empty_input() {
        let (endpoint, _state) = start_mock_server().await;
        let client = make_test_client(&endpoint).await;

        let results = client.batch_upload(vec![]).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn check_exists_found() {
        let (endpoint, state) = start_mock_server().await;
        state
            .objects
            .write()
            .await
            .insert("existing.txt".into(), b"data123".to_vec());

        let client = make_test_client(&endpoint).await;
        let resp = client.check_exists("existing.txt").await.unwrap();
        assert_eq!(resp.bucket, "test-bucket");
        assert_eq!(resp.path, "existing.txt");
        assert_eq!(resp.size, 7);
    }

    #[tokio::test]
    async fn check_exists_not_found() {
        let (endpoint, _state) = start_mock_server().await;
        let client = make_test_client(&endpoint).await;

        assert!(client.check_exists("nonexistent.txt").await.is_none());
    }

    #[tokio::test]
    async fn batch_upload_then_batch_exists_roundtrip() {
        let (endpoint, _state) = start_mock_server().await;
        let client = make_test_client(&endpoint).await;

        let files = vec![
            ("rt-a.txt".into(), b"a".to_vec(), "text/plain".into()),
            ("rt-b.txt".into(), b"b".to_vec(), "text/plain".into()),
        ];
        client.batch_upload(files).await.unwrap();

        let paths: Vec<String> = vec!["rt-a.txt".into(), "rt-b.txt".into(), "rt-c.txt".into()];
        let existing = unwrap_found(client.batch_check_exists(&paths).await);
        assert_eq!(existing.len(), 2);
        assert!(existing.contains("rt-a.txt"));
        assert!(existing.contains("rt-b.txt"));
    }

    #[tokio::test]
    async fn batch_upload_partial_failure() {
        let endpoint = start_mock_server_rejecting_fail_keys().await;
        let client = make_test_client(&endpoint).await;

        let files = vec![
            ("good.txt".into(), b"ok".to_vec(), "text/plain".into()),
            ("fail-item.txt".into(), b"bad".to_vec(), "text/plain".into()),
            (
                "also-good.txt".into(),
                b"fine".to_vec(),
                "text/plain".into(),
            ),
        ];

        let results = client.batch_upload(files).await.unwrap();
        assert_eq!(results.len(), 3);

        let by_path: HashMap<&str, &prod_mc_cli_chat_proxy_types::BatchUploadResult> =
            results.iter().map(|r| (r.path.as_str(), r)).collect();

        let good = by_path["good.txt"];
        assert_eq!(
            good.status,
            prod_mc_cli_chat_proxy_types::BatchUploadStatus::Ok
        );
        assert!(good.size.is_some());
        assert!(good.error.is_none());

        let fail = by_path["fail-item.txt"];
        assert_eq!(
            fail.status,
            prod_mc_cli_chat_proxy_types::BatchUploadStatus::Error
        );
        assert!(fail.size.is_none());
        assert!(fail.error.is_some());

        let also_good = by_path["also-good.txt"];
        assert_eq!(
            also_good.status,
            prod_mc_cli_chat_proxy_types::BatchUploadStatus::Ok
        );
    }

    #[tokio::test]
    async fn new_returns_error_for_invalid_credentials_file() {
        let result = S3StorageClient::new(
            "test-bucket".to_string(),
            "us-east-1",
            None,
            Some("/nonexistent/path/to/credentials"),
            None,
        )
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn upload_stream_sends_content() {
        let (endpoint, state) = start_mock_server().await;
        let content = b"streamed content here";
        let reader = std::io::Cursor::new(content.to_vec());

        let result = upload_stream(
            "test-bucket",
            "stream-test.txt",
            reader,
            "text/plain",
            "us-east-1",
            Some(r#"{"aws_access_key_id":"test","aws_secret_access_key":"test"}"#),
            None,
            Some(&endpoint),
        )
        .await
        .unwrap();

        assert_eq!(result, "s3://test-bucket/stream-test.txt");
        let objects = state.objects.read().await;
        assert_eq!(objects.get("stream-test.txt").unwrap(), content);
    }

    #[tokio::test]
    async fn upload_stream_large_content() {
        let (endpoint, state) = start_mock_server().await;

        // 100 KB — larger payload exercising multi-chunk ReaderStream reads
        let content: Vec<u8> = (0..100_000).map(|i| (i % 256) as u8).collect();
        let reader = std::io::Cursor::new(content.clone());

        let result = upload_stream(
            "test-bucket",
            "large-stream.bin",
            reader,
            "application/octet-stream",
            "us-east-1",
            Some(r#"{"aws_access_key_id":"test","aws_secret_access_key":"test"}"#),
            None,
            Some(&endpoint),
        )
        .await
        .unwrap();

        assert_eq!(result, "s3://test-bucket/large-stream.bin");
        let objects = state.objects.read().await;
        assert_eq!(objects.get("large-stream.bin").unwrap(), &content);
    }

    #[test]
    fn multipart_threshold_is_under_16mib() {
        const MAX_CHUNK: usize = 16 * 1024 * 1024;
        const _: () = assert!(MULTIPART_THRESHOLD <= MAX_CHUNK);
        const _: () = assert!(MULTIPART_PART_SIZE <= MAX_CHUNK);
    }

    #[tokio::test]
    async fn upload_bytes_uses_single_put_for_small_content() {
        let (endpoint, state) = start_mock_server().await;

        let content = b"small payload".to_vec();
        upload_bytes(
            "test-bucket",
            "small-single.txt",
            &content,
            "text/plain",
            "us-east-1",
            Some(r#"{"aws_access_key_id":"test","aws_secret_access_key":"test"}"#),
            None,
            Some(&endpoint),
        )
        .await
        .unwrap();

        let objects = state.objects.read().await;
        assert_eq!(objects.get("small-single.txt").unwrap(), &content);
        assert!(state.multipart_uploads.read().await.is_empty());
    }

    /// Full multipart roundtrip against a real S3-compatible endpoint.
    #[tokio::test]
    #[ignore]
    async fn integration_multipart_upload_roundtrip() {
        let (endpoint, bucket, access_key, secret_key, region) = match integration_test_config() {
            Some(c) => c,
            None => return,
        };
        let creds = integration_creds(&access_key, &secret_key);
        let raw = make_raw_sdk_client(&endpoint, &access_key, &secret_key, &region).await;

        let prefix = unique_prefix();
        let key = format!("{prefix}/multipart-large.bin");

        // 20 MiB of patterned data, exceeds MULTIPART_THRESHOLD
        let content: Vec<u8> = (0..20 * 1024 * 1024).map(|i| (i % 251) as u8).collect();

        let result = upload_bytes(
            &bucket,
            &key,
            &content,
            "application/octet-stream",
            &region,
            Some(&creds),
            None,
            Some(&endpoint),
        )
        .await
        .unwrap();

        assert_eq!(result, format!("s3://{bucket}/{key}"));

        let obj = raw
            .get_object()
            .bucket(&bucket)
            .key(&key)
            .send()
            .await
            .unwrap();
        let body = obj.body.collect().await.unwrap().into_bytes();
        assert_eq!(&body[..], &content[..]);
    }

    // --- Integration tests (require S3_TEST_ENDPOINT env var pointing at MinIO or similar) ---

    fn integration_test_config() -> Option<(String, String, String, String, String)> {
        Some((
            std::env::var("S3_TEST_ENDPOINT").ok()?,
            std::env::var("S3_TEST_BUCKET").ok()?,
            std::env::var("S3_TEST_ACCESS_KEY").ok()?,
            std::env::var("S3_TEST_SECRET_KEY").ok()?,
            std::env::var("S3_TEST_REGION").unwrap_or_else(|_| "us-east-1".to_string()),
        ))
    }

    fn integration_creds(access_key: &str, secret_key: &str) -> String {
        serde_json::json!({
            "aws_access_key_id": access_key,
            "aws_secret_access_key": secret_key,
        })
        .to_string()
    }

    async fn make_integration_client(
        endpoint: &str,
        bucket: &str,
        access_key: &str,
        secret_key: &str,
        region: &str,
    ) -> S3StorageClient {
        let creds = integration_creds(access_key, secret_key);
        S3StorageClient::new(
            bucket.to_string(),
            region,
            Some(&creds),
            None,
            Some(endpoint),
        )
        .await
        .unwrap()
    }

    async fn make_raw_sdk_client(
        endpoint: &str,
        access_key: &str,
        secret_key: &str,
        region: &str,
    ) -> aws_sdk_s3::Client {
        let creds = integration_creds(access_key, secret_key);
        build_s3_client(region, Some(&creds), None, Some(endpoint))
            .await
            .unwrap()
    }

    fn unique_prefix() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("test-{nanos}-{}", std::process::id())
    }

    #[tokio::test]
    #[ignore]
    async fn integration_batch_upload_and_verify_content() {
        let (endpoint, bucket, access_key, secret_key, region) = match integration_test_config() {
            Some(c) => c,
            None => return,
        };
        let client =
            make_integration_client(&endpoint, &bucket, &access_key, &secret_key, &region).await;
        let raw = make_raw_sdk_client(&endpoint, &access_key, &secret_key, &region).await;

        let prefix = unique_prefix();
        let key_a = format!("{prefix}/a.txt");
        let key_b = format!("{prefix}/b.bin");

        let files = vec![
            (key_a.clone(), b"hello-a".to_vec(), "text/plain".to_string()),
            (
                key_b.clone(),
                b"\x00\x01\x02binary".to_vec(),
                "application/octet-stream".to_string(),
            ),
        ];

        let results = client.batch_upload(files).await.unwrap();
        assert_eq!(results.len(), 2);
        for r in &results {
            assert_eq!(
                r.status,
                prod_mc_cli_chat_proxy_types::BatchUploadStatus::Ok
            );
        }

        let obj_a = raw
            .get_object()
            .bucket(&bucket)
            .key(&key_a)
            .send()
            .await
            .unwrap();
        let body_a = obj_a.body.collect().await.unwrap().into_bytes();
        assert_eq!(&body_a[..], b"hello-a");

        let obj_b = raw
            .get_object()
            .bucket(&bucket)
            .key(&key_b)
            .send()
            .await
            .unwrap();
        let body_b = obj_b.body.collect().await.unwrap().into_bytes();
        assert_eq!(&body_b[..], b"\x00\x01\x02binary");
    }

    #[tokio::test]
    #[ignore]
    async fn integration_batch_check_exists_partitions_correctly() {
        let (endpoint, bucket, access_key, secret_key, region) = match integration_test_config() {
            Some(c) => c,
            None => return,
        };
        let client =
            make_integration_client(&endpoint, &bucket, &access_key, &secret_key, &region).await;

        let prefix = unique_prefix();
        let uploaded_1 = format!("{prefix}/exists-1.txt");
        let uploaded_2 = format!("{prefix}/exists-2.txt");
        let missing = format!("{prefix}/does-not-exist.txt");

        let files = vec![
            (
                uploaded_1.clone(),
                b"data1".to_vec(),
                "text/plain".to_string(),
            ),
            (
                uploaded_2.clone(),
                b"data2".to_vec(),
                "text/plain".to_string(),
            ),
        ];
        client.batch_upload(files).await.unwrap();

        let paths = vec![uploaded_1.clone(), missing.clone(), uploaded_2.clone()];
        let existing = unwrap_found(client.batch_check_exists(&paths).await);
        assert_eq!(existing.len(), 2);
        assert!(existing.contains(&uploaded_1));
        assert!(existing.contains(&uploaded_2));
        assert!(!existing.contains(&missing));

        // All-missing query collapses to top-level NotFound (symmetric with proxy).
        let all_missing = vec![format!("{prefix}/nope-1"), format!("{prefix}/nope-2")];
        assert!(matches!(
            client.batch_check_exists(&all_missing).await,
            crate::storage_client::ExistsResult::NotFound
        ));
    }

    #[tokio::test]
    #[ignore]
    async fn integration_upload_stream_roundtrip() {
        let (endpoint, bucket, access_key, secret_key, region) = match integration_test_config() {
            Some(c) => c,
            None => return,
        };
        let creds = integration_creds(&access_key, &secret_key);
        let raw = make_raw_sdk_client(&endpoint, &access_key, &secret_key, &region).await;

        let prefix = unique_prefix();
        let key = format!("{prefix}/streamed.bin");

        // 50 KB of patterned data to exercise multi-chunk ReaderStream reads
        let content: Vec<u8> = (0..50_000).map(|i| (i % 251) as u8).collect();
        let reader = std::io::Cursor::new(content.clone());

        let result = upload_stream(
            &bucket,
            &key,
            reader,
            "application/octet-stream",
            &region,
            Some(&creds),
            None,
            Some(&endpoint),
        )
        .await
        .unwrap();

        assert_eq!(result, format!("s3://{bucket}/{key}"));

        let obj = raw
            .get_object()
            .bucket(&bucket)
            .key(&key)
            .send()
            .await
            .unwrap();
        let body = obj.body.collect().await.unwrap().into_bytes();
        assert_eq!(&body[..], &content[..]);
    }

    #[tokio::test]
    #[ignore]
    async fn integration_check_exists_single_roundtrip() {
        let (endpoint, bucket, access_key, secret_key, region) = match integration_test_config() {
            Some(c) => c,
            None => return,
        };
        let client =
            make_integration_client(&endpoint, &bucket, &access_key, &secret_key, &region).await;

        let prefix = unique_prefix();
        let key = format!("{prefix}/single.txt");

        assert!(client.check_exists(&key).await.is_none());

        client
            .batch_upload(vec![(
                key.clone(),
                b"present".to_vec(),
                "text/plain".to_string(),
            )])
            .await
            .unwrap();

        let resp = client.check_exists(&key).await.unwrap();
        assert_eq!(resp.path, key);
        assert_eq!(resp.bucket, bucket);
    }

    #[tokio::test]
    #[ignore]
    async fn integration_empty_file_upload() {
        let (endpoint, bucket, access_key, secret_key, region) = match integration_test_config() {
            Some(c) => c,
            None => return,
        };
        let client =
            make_integration_client(&endpoint, &bucket, &access_key, &secret_key, &region).await;
        let raw = make_raw_sdk_client(&endpoint, &access_key, &secret_key, &region).await;

        let prefix = unique_prefix();
        let key = format!("{prefix}/empty.bin");

        let results = client
            .batch_upload(vec![(
                key.clone(),
                vec![],
                "application/octet-stream".to_string(),
            )])
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].status,
            prod_mc_cli_chat_proxy_types::BatchUploadStatus::Ok
        );
        assert_eq!(results[0].size, Some(0));

        let obj = raw
            .get_object()
            .bucket(&bucket)
            .key(&key)
            .send()
            .await
            .unwrap();
        let body = obj.body.collect().await.unwrap().into_bytes();
        assert!(body.is_empty());

        assert!(client.check_exists(&key).await.is_some());
    }
}
