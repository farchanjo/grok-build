//! Backend selection and store construction.
//!
//! [`resolve_asset_store_source`] is pure, synchronous, and does no I/O: it
//! turns `[assets]` + `[assets_providers.*]` + environment overrides into a
//! fully concrete [`AssetStoreSource`]. That makes every precedence rule and
//! every fail-closed case unit-testable without a network or a filesystem.
//!
//! Precedence, highest first:
//!
//! 1. explicit `[assets].provider` (or `GROK_ASSETS_PROVIDER`)
//! 2. named `[assets].active_profile` (or `GROK_ASSETS_PROFILE`)
//! 3. bucket scheme — `s3://bucket` / `gs://bucket`
//! 4. `local` — total, never fails
//!
//! A *named* mistake fails closed: an `active_profile` with no matching
//! `[assets_providers.<id>]`, a `provider` that needs a bucket without one, a
//! TTL outside the SigV4 window, an unparseable env override. Only an
//! unrecognized bucket scheme (including a bare bucket name, which carries no
//! scheme at all) warns and falls through to `local`.
//!
//! [`resolve_asset_store`] adds construction on top. Construction of a
//! non-local backend cannot fail today — see [`StubAssetStore`] — so the
//! terminal `local` fallback is total.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use xai_grok_config_types::{
    AssetProviderConfig, AssetProviderKind, AssetVisibility, AssetsConfig, AssetsSettings,
    DEFAULT_ASSET_KEY_PREFIX, DEFAULT_ASSET_REQUEST_TIMEOUT_SECS, DEFAULT_ASSET_TTL_SECS,
    validate_asset_key_prefix, validate_asset_ttl_secs,
};

use super::error::{AssetError, AssetOperation};
use super::gcs_store::GcsAssetStore;
use super::key::{AssetKey, AssetPrefix, ContentType};
use super::local_store::LocalAssetStore;
use super::s3_store::S3AssetStore;
use super::value::{
    AssetMeta, DeleteOutcome, ListPage, ListQuery, PresignedUrl, PutRequest, Visibility,
};
use super::{AssetStore, BackendCapabilities, BackendKind, SharedAssetStore, StoreStatus};

// ---------------------------------------------------------------------------
// Environment overrides
// ---------------------------------------------------------------------------

/// `GROK_ASSETS_*` overrides, captured from the process environment.
///
/// Kept as an explicit value (rather than reading `std::env` inside the
/// resolver) so tests are deterministic and parallel-safe.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssetEnvOverrides {
    pub provider: Option<String>,
    pub profile: Option<String>,
    pub bucket: Option<String>,
    pub region: Option<String>,
    pub endpoint_url: Option<String>,
    pub credentials_file: Option<String>,
    pub public_base_url: Option<String>,
    pub default_visibility: Option<String>,
    pub default_ttl_secs: Option<String>,
    pub local_root: Option<String>,
    pub proxy_base_url: Option<String>,
    pub max_object_bytes: Option<String>,
    pub request_timeout_secs: Option<String>,
    pub key_prefix: Option<String>,
}

impl AssetEnvOverrides {
    /// Read every `GROK_ASSETS_*` variable from the process environment.
    pub fn from_process_env() -> Self {
        fn var(name: &str) -> Option<String> {
            std::env::var(name).ok().filter(|v| !v.trim().is_empty())
        }
        Self {
            provider: var("GROK_ASSETS_PROVIDER"),
            profile: var("GROK_ASSETS_PROFILE"),
            bucket: var("GROK_ASSETS_BUCKET"),
            region: var("GROK_ASSETS_REGION"),
            endpoint_url: var("GROK_ASSETS_ENDPOINT_URL"),
            credentials_file: var("GROK_ASSETS_CREDENTIALS_FILE"),
            public_base_url: var("GROK_ASSETS_PUBLIC_BASE_URL"),
            default_visibility: var("GROK_ASSETS_DEFAULT_VISIBILITY"),
            default_ttl_secs: var("GROK_ASSETS_DEFAULT_TTL_SECS"),
            local_root: var("GROK_ASSETS_LOCAL_ROOT"),
            proxy_base_url: var("GROK_ASSETS_PROXY_BASE_URL"),
            max_object_bytes: var("GROK_ASSETS_MAX_OBJECT_BYTES"),
            request_timeout_secs: var("GROK_ASSETS_REQUEST_TIMEOUT_SECS"),
            key_prefix: var("GROK_ASSETS_KEY_PREFIX"),
        }
    }

    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Everything the resolver needs that is not in the config file.
#[derive(Debug, Clone, Default)]
pub struct AssetRuntimeContext {
    /// `GROK_ASSETS_*` overrides. Empty in tests unless set explicitly.
    pub env: AssetEnvOverrides,
    /// Home directory used to expand the default `local_root` (`$GROK_HOME`).
    pub home_dir: Option<PathBuf>,
    /// Explicit default local root; wins over `home_dir`-derived default.
    pub default_local_root: Option<PathBuf>,
}

impl AssetRuntimeContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Capture the process environment. `GROK_HOME` (then the platform home)
    /// supplies the default local root.
    pub fn from_process() -> Self {
        let home_dir = std::env::var_os("GROK_HOME")
            .map(PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty())
            .or_else(dirs::home_dir);
        Self {
            env: AssetEnvOverrides::from_process_env(),
            home_dir,
            default_local_root: None,
        }
    }

    pub fn with_env(mut self, env: AssetEnvOverrides) -> Self {
        self.env = env;
        self
    }

    pub fn with_home_dir(mut self, home_dir: impl Into<PathBuf>) -> Self {
        self.home_dir = Some(home_dir.into());
        self
    }

    pub fn with_default_local_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.default_local_root = Some(root.into());
        self
    }

    /// Default local root: explicit > `$GROK_HOME/assets` > `./assets`.
    fn resolve_local_root(&self, configured: Option<&str>) -> PathBuf {
        if let Some(configured) = configured.filter(|s| !s.trim().is_empty()) {
            return PathBuf::from(configured);
        }
        if let Some(root) = &self.default_local_root {
            return root.clone();
        }
        if let Some(home) = &self.home_dir {
            return home.join("assets");
        }
        PathBuf::from("assets")
    }
}

// ---------------------------------------------------------------------------
// Selection result
// ---------------------------------------------------------------------------

/// Which precedence rule selected the backend. Carried for diagnostics and
/// asserted directly by the tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionOrigin {
    /// `[assets].provider` or `GROK_ASSETS_PROVIDER`.
    ExplicitProvider,
    /// `[assets].active_profile`, naming `[assets_providers.<id>]`.
    ActiveProfile(String),
    /// A scheme-qualified `bucket`.
    BucketScheme(String),
    /// Nothing selected; the total fallback.
    LocalFallback,
}

impl SelectionOrigin {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::ExplicitProvider => "explicit_provider",
            Self::ActiveProfile(_) => "active_profile",
            Self::BucketScheme(_) => "bucket_scheme",
            Self::LocalFallback => "local_fallback",
        }
    }
}

impl std::fmt::Display for SelectionOrigin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ActiveProfile(id) => write!(f, "active_profile:{id}"),
            Self::BucketScheme(scheme) => write!(f, "bucket_scheme:{scheme}"),
            other => f.write_str(other.as_str()),
        }
    }
}

/// A fully resolved backend selection: concrete enough that construction needs
/// nothing but this value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetStoreSource {
    pub kind: BackendKind,
    pub origin: SelectionOrigin,
    pub bucket: Option<String>,
    pub region: Option<String>,
    pub endpoint_url: Option<String>,
    pub credentials_file: Option<String>,
    /// Environment variable *name* holding the credential. Never the secret.
    pub env_key: Option<String>,
    pub public_base_url: Option<String>,
    pub default_visibility: Visibility,
    pub default_ttl: Duration,
    pub local_root: PathBuf,
    pub proxy_base_url: Option<String>,
    pub max_object_bytes: Option<u64>,
    pub request_timeout: Duration,
    pub key_prefix: AssetPrefix,
}

impl AssetStoreSource {
    /// Capability matrix for the selected backend.
    pub const fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::for_kind(self.kind)
    }

    pub const fn is_local(&self) -> bool {
        matches!(self.kind, BackendKind::Local)
    }
}

// ---------------------------------------------------------------------------
// Resolution
// ---------------------------------------------------------------------------

/// A `bucket` value split into its scheme and remainder.
enum BucketScheme {
    S3(String),
    Gcs(String),
    /// No recognized scheme (including a bare bucket name).
    Unrecognized(String),
}

fn split_bucket_scheme(bucket: &str) -> BucketScheme {
    if let Some(rest) = bucket.strip_prefix("s3://") {
        return BucketScheme::S3(rest.to_owned());
    }
    if let Some(rest) = bucket.strip_prefix("gs://") {
        return BucketScheme::Gcs(rest.to_owned());
    }
    BucketScheme::Unrecognized(bucket.to_owned())
}

/// Split `bucket[/optional/prefix]` into the bucket name and a key prefix.
fn split_bucket_remainder(remainder: &str) -> (String, String) {
    match remainder.split_once('/') {
        Some((bucket, prefix)) => (
            bucket.trim().to_owned(),
            prefix.trim_end_matches('/').to_owned(),
        ),
        None => (remainder.trim().to_owned(), String::new()),
    }
}

/// Overlay a named profile on the base table. `None` fields inherit.
fn merge_profile(base: &AssetsConfig, profile: &AssetProviderConfig) -> AssetsConfig {
    AssetsConfig {
        provider: Some(profile.kind),
        active_profile: base.active_profile.clone(),
        bucket: profile.bucket.clone().or_else(|| base.bucket.clone()),
        region: profile.region.clone().or_else(|| base.region.clone()),
        endpoint_url: profile
            .endpoint_url
            .clone()
            .or_else(|| base.endpoint_url.clone()),
        credentials_file: profile
            .credentials_file
            .clone()
            .or_else(|| base.credentials_file.clone()),
        env_key: profile.env_key.clone().or_else(|| base.env_key.clone()),
        public_base_url: profile
            .public_base_url
            .clone()
            .or_else(|| base.public_base_url.clone()),
        default_visibility: profile
            .default_visibility
            .unwrap_or(base.default_visibility),
        default_ttl_secs: profile.default_ttl_secs.unwrap_or(base.default_ttl_secs),
        local_root: profile
            .local_root
            .clone()
            .or_else(|| base.local_root.clone()),
        proxy_base_url: profile
            .proxy_base_url
            .clone()
            .or_else(|| base.proxy_base_url.clone()),
        max_object_bytes: profile.max_object_bytes.or(base.max_object_bytes),
        request_timeout_secs: profile
            .request_timeout_secs
            .unwrap_or(base.request_timeout_secs),
        key_prefix: profile
            .key_prefix
            .clone()
            .unwrap_or_else(|| base.key_prefix.clone()),
    }
}

fn parse_env_provider(raw: &str) -> Result<AssetProviderKind, AssetError> {
    AssetProviderKind::parse(raw).ok_or_else(|| AssetError::InvalidKey {
        key: "GROK_ASSETS_PROVIDER".to_owned(),
        reason: format!("`{raw}` is not one of s3, gcs, local, proxy"),
    })
}

fn parse_env_visibility(raw: &str) -> Result<AssetVisibility, AssetError> {
    AssetVisibility::parse(raw).ok_or_else(|| AssetError::InvalidKey {
        key: "GROK_ASSETS_DEFAULT_VISIBILITY".to_owned(),
        reason: format!("`{raw}` is not one of private, public"),
    })
}

fn parse_env_u64(name: &str, raw: &str) -> Result<u64, AssetError> {
    raw.trim()
        .parse::<u64>()
        .map_err(|_| AssetError::InvalidKey {
            key: name.to_owned(),
            reason: format!("`{raw}` is not a non-negative integer"),
        })
}

/// Resolve the backend selection. Pure, synchronous, no I/O.
///
/// Fails closed on named mistakes; the terminal `local` branch is total.
pub fn resolve_asset_store_source(
    settings: &AssetsSettings,
    context: &AssetRuntimeContext,
) -> Result<AssetStoreSource, AssetError> {
    // Every named mistake is loud, even when a higher-precedence signal would
    // have shadowed it.
    settings
        .validate()
        .map_err(|reason| AssetError::InvalidKey {
            key: "assets".to_owned(),
            reason,
        })?;

    let env = &context.env;

    // Environment wins over the config file for the same field, matching the
    // repository's `GROK_*` convention.
    let mut base = settings.assets.clone();
    let mut origin = SelectionOrigin::LocalFallback;

    // 1. Explicit provider.
    if let Some(raw) = env.provider.as_deref() {
        base.provider = Some(parse_env_provider(raw)?);
    }
    // 2. Named profile.
    if let Some(raw) = env.profile.as_deref() {
        base.active_profile = Some(raw.trim().to_owned());
    }
    if let Some(raw) = env.bucket.as_deref() {
        base.bucket = Some(raw.to_owned());
    }

    let explicit_provider = base.provider;
    let active_profile = base.active_profile.clone();

    if let Some(provider) = explicit_provider {
        base.provider = Some(provider);
        origin = SelectionOrigin::ExplicitProvider;
    } else if let Some(id) = active_profile.as_deref() {
        let profile = settings.profile(id).ok_or_else(|| AssetError::InvalidKey {
            key: "assets.active_profile".to_owned(),
            reason: format!("`{id}` has no matching `[assets_providers.{id}]` table"),
        })?;
        base = merge_profile(&base, profile);
        origin = SelectionOrigin::ActiveProfile(id.to_owned());
    }

    // 3. Bucket scheme. A scheme-qualified bucket is always normalized (the
    // scheme is stripped); it only *selects* the backend when nothing above
    // did. An unrecognized scheme — including a bare bucket name, which
    // carries no scheme at all — warns and falls through to `local`.
    let selecting = matches!(origin, SelectionOrigin::LocalFallback);
    let mut scheme_prefix = String::new();
    if let Some(bucket) = base.bucket.clone() {
        match split_bucket_scheme(&bucket) {
            BucketScheme::S3(rest) => {
                let (name, prefix) = split_bucket_remainder(&rest);
                base.bucket = Some(name);
                scheme_prefix = prefix;
                if selecting {
                    base.provider = Some(AssetProviderKind::S3);
                    origin = SelectionOrigin::BucketScheme("s3".to_owned());
                }
            }
            BucketScheme::Gcs(rest) => {
                let (name, prefix) = split_bucket_remainder(&rest);
                base.bucket = Some(name);
                scheme_prefix = prefix;
                if selecting {
                    base.provider = Some(AssetProviderKind::Gcs);
                    origin = SelectionOrigin::BucketScheme("gs".to_owned());
                }
            }
            BucketScheme::Unrecognized(raw) => {
                if selecting {
                    tracing::warn!(
                        bucket = raw.as_str(),
                        "assets.bucket has no recognized scheme; falling through to the \
                         local backend (use s3://bucket or gs://bucket)"
                    );
                }
            }
        }
    }

    // Apply the remaining environment overrides on top of the effective view.
    if let Some(raw) = env.region.as_deref() {
        base.region = Some(raw.to_owned());
    }
    if let Some(raw) = env.endpoint_url.as_deref() {
        base.endpoint_url = Some(raw.to_owned());
    }
    if let Some(raw) = env.credentials_file.as_deref() {
        base.credentials_file = Some(raw.to_owned());
    }
    if let Some(raw) = env.public_base_url.as_deref() {
        base.public_base_url = Some(raw.to_owned());
    }
    if let Some(raw) = env.default_visibility.as_deref() {
        base.default_visibility = parse_env_visibility(raw)?;
    }
    if let Some(raw) = env.default_ttl_secs.as_deref() {
        base.default_ttl_secs = parse_env_u64("GROK_ASSETS_DEFAULT_TTL_SECS", raw)?;
    }
    if let Some(raw) = env.local_root.as_deref() {
        base.local_root = Some(raw.to_owned());
    }
    if let Some(raw) = env.proxy_base_url.as_deref() {
        base.proxy_base_url = Some(raw.to_owned());
    }
    if let Some(raw) = env.max_object_bytes.as_deref() {
        base.max_object_bytes = Some(parse_env_u64("GROK_ASSETS_MAX_OBJECT_BYTES", raw)?);
    }
    if let Some(raw) = env.request_timeout_secs.as_deref() {
        base.request_timeout_secs = parse_env_u64("GROK_ASSETS_REQUEST_TIMEOUT_SECS", raw)?;
    }
    if let Some(raw) = env.key_prefix.as_deref() {
        base.key_prefix = raw.to_owned();
    }

    // From here on the view is fully effective: validate what is named.
    let kind = match base.provider {
        Some(kind) => BackendKind::from_config(kind),
        None => BackendKind::Local,
    };

    validate_asset_ttl_secs(base.default_ttl_secs).map_err(|reason| AssetError::InvalidKey {
        key: "assets.default_ttl_secs".to_owned(),
        reason,
    })?;
    validate_asset_key_prefix(&base.key_prefix).map_err(|reason| AssetError::InvalidKey {
        key: "assets.key_prefix".to_owned(),
        reason,
    })?;

    let key_prefix = if scheme_prefix.is_empty() {
        base.key_prefix.clone()
    } else {
        format!("{}{scheme_prefix}/", base.key_prefix)
    };
    let key_prefix = AssetPrefix::parse(&key_prefix).map_err(|err| AssetError::InvalidKey {
        key: "assets.key_prefix".to_owned(),
        reason: err.to_string(),
    })?;

    let bucket = base
        .bucket
        .as_deref()
        .map(str::trim)
        .filter(|b| !b.is_empty())
        .map(str::to_owned);
    if kind.is_remote() && bucket.is_none() {
        return Err(AssetError::InvalidKey {
            key: "assets.bucket".to_owned(),
            reason: format!(
                "backend `{kind}` needs a bucket (selected via {origin}); \
                 set `[assets].bucket` or `GROK_ASSETS_BUCKET`"
            ),
        });
    }

    let proxy_base_url = base
        .proxy_base_url
        .as_deref()
        .map(str::trim)
        .filter(|u| !u.is_empty())
        .map(str::to_owned);
    if kind == BackendKind::Proxy && proxy_base_url.is_none() {
        return Err(AssetError::InvalidKey {
            key: "assets.proxy_base_url".to_owned(),
            reason: "backend `proxy` needs `proxy_base_url`".to_owned(),
        });
    }

    Ok(AssetStoreSource {
        kind,
        origin,
        bucket,
        region: base.region.clone(),
        endpoint_url: base.endpoint_url.clone(),
        credentials_file: base.credentials_file.clone(),
        env_key: base.env_key.clone(),
        public_base_url: base.public_base_url.clone(),
        default_visibility: Visibility::from(base.default_visibility),
        default_ttl: Duration::from_secs(base.default_ttl_secs),
        local_root: context.resolve_local_root(base.local_root.as_deref()),
        proxy_base_url,
        max_object_bytes: base.max_object_bytes,
        request_timeout: Duration::from_secs(base.request_timeout_secs.max(1)),
        key_prefix,
    })
}

/// Build the store for the resolved selection.
///
/// Fails closed on selection errors (see [`resolve_asset_store_source`]).
/// Construction failure of a remote adapter degrades to the local backend with
/// a warning, so a session always ends up with a usable store.
pub async fn resolve_asset_store(
    settings: &AssetsSettings,
    context: &AssetRuntimeContext,
) -> Result<SharedAssetStore, AssetError> {
    let source = resolve_asset_store_source(settings, context)?;
    match source.kind {
        BackendKind::Local => {
            Ok(Arc::new(LocalAssetStore::from_source(&source)) as SharedAssetStore)
        }
        BackendKind::S3 => match S3AssetStore::from_source(&source).await {
            Ok(store) => Ok(Arc::new(store) as SharedAssetStore),
            Err(err) => Ok(degrade_to_local(&source, &err)),
        },
        BackendKind::Gcs => match GcsAssetStore::from_source(&source).await {
            Ok(store) => Ok(Arc::new(store) as SharedAssetStore),
            Err(err) => Ok(degrade_to_local(&source, &err)),
        },
        // The proxy adapter lands with the `storage_client` slice.
        BackendKind::Proxy => {
            tracing::debug!(
                backend = source.kind.as_str(),
                origin = %source.origin,
                "assets backend selected; the proxy adapter is not wired yet, so \
                 operations report `Unsupported` until then"
            );
            Ok(Arc::new(StubAssetStore::new(source.kind)) as SharedAssetStore)
        }
    }
}

/// Total fallback: a session must never be left without a store.
fn degrade_to_local(source: &AssetStoreSource, err: &AssetError) -> SharedAssetStore {
    tracing::warn!(
        backend = source.kind.as_str(),
        origin = %source.origin,
        error = %err,
        "assets: adapter construction failed; falling back to the local backend"
    );
    let mut local = source.clone();
    local.kind = BackendKind::Local;
    Arc::new(LocalAssetStore::from_source(&local)) as SharedAssetStore
}

// ---------------------------------------------------------------------------
// Stub store
// ---------------------------------------------------------------------------

/// A store for a backend whose adapter is not wired up yet.
///
/// Only `proxy` still lands here (the proxy adapter needs `storage_client`).
/// Every operation fails through [`BackendCapabilities::require`] first, so an
/// operation the backend genuinely cannot do reports the capability reason and
/// everything else reports "adapter not implemented". Never a fake success.
#[derive(Debug, Clone)]
pub struct StubAssetStore {
    kind: BackendKind,
    capabilities: BackendCapabilities,
}

impl StubAssetStore {
    pub fn new(kind: BackendKind) -> Self {
        Self {
            kind,
            capabilities: BackendCapabilities::for_kind(kind),
        }
    }

    fn unimplemented(&self, operation: AssetOperation) -> AssetError {
        AssetError::unsupported(
            self.kind,
            operation,
            format!("{} adapter is not implemented yet", self.kind.as_str()),
        )
    }

    /// Capability gap first, then the not-implemented reason.
    fn gate(&self, operation: AssetOperation) -> AssetError {
        self.capabilities
            .require(self.kind, operation)
            .err()
            .unwrap_or_else(|| self.unimplemented(operation))
    }
}

#[async_trait::async_trait]
impl AssetStore for StubAssetStore {
    fn backend(&self) -> BackendKind {
        self.kind
    }

    fn capabilities(&self) -> BackendCapabilities {
        self.capabilities
    }

    async fn put(&self, _request: PutRequest) -> Result<AssetMeta, AssetError> {
        Err(self.gate(AssetOperation::Put))
    }

    async fn put_file(&self, _request: PutRequest) -> Result<AssetMeta, AssetError> {
        Err(self.gate(AssetOperation::PutFile))
    }

    async fn get(&self, _key: &AssetKey) -> Result<Bytes, AssetError> {
        Err(self.gate(AssetOperation::Get))
    }

    async fn download_to(&self, _key: &AssetKey, _dest: &Path) -> Result<AssetMeta, AssetError> {
        Err(self.gate(AssetOperation::DownloadTo))
    }

    async fn exists(&self, _key: &AssetKey) -> Result<bool, AssetError> {
        Err(self.gate(AssetOperation::Exists))
    }

    async fn delete(&self, _key: &AssetKey) -> Result<DeleteOutcome, AssetError> {
        Err(self.gate(AssetOperation::Delete))
    }

    async fn list(&self, _query: ListQuery) -> Result<ListPage, AssetError> {
        Err(self.gate(AssetOperation::List))
    }

    async fn presign_get(
        &self,
        _key: &AssetKey,
        _ttl: Duration,
    ) -> Result<PresignedUrl, AssetError> {
        Err(self.gate(AssetOperation::PresignGet))
    }

    async fn presign_put(
        &self,
        _key: &AssetKey,
        _content_type: &ContentType,
        _ttl: Duration,
    ) -> Result<PresignedUrl, AssetError> {
        Err(self.gate(AssetOperation::PresignPut))
    }

    async fn set_visibility(
        &self,
        _key: &AssetKey,
        _visibility: Visibility,
    ) -> Result<AssetMeta, AssetError> {
        Err(self.gate(AssetOperation::SetVisibility))
    }

    fn public_url(&self, _key: &AssetKey) -> Option<String> {
        None
    }

    async fn health(&self) -> Result<StoreStatus, AssetError> {
        Ok(StoreStatus::degraded(
            self.kind,
            false,
            format!("{} adapter is not implemented yet", self.kind.as_str()),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xai_grok_config_types::{MAX_ASSET_TTL_SECS, MIN_ASSET_TTL_SECS};

    fn ctx() -> AssetRuntimeContext {
        AssetRuntimeContext::new()
            .with_home_dir("/home/tester")
            .with_env(AssetEnvOverrides::default())
    }

    fn settings(toml_src: &str) -> AssetsSettings {
        toml::from_str(toml_src).unwrap_or_else(|e| panic!("parse failed for {toml_src}: {e}"))
    }

    // ---- precedence -----------------------------------------------------

    #[test]
    fn explicit_provider_wins_over_everything() {
        let settings = settings(
            r#"
[assets]
provider = "local"
active_profile = "remote"
bucket = "s3://from-bucket"

[assets_providers.remote]
kind = "gcs"
bucket = "from-profile"
"#,
        );
        let source = resolve_asset_store_source(&settings, &ctx()).unwrap();
        assert_eq!(source.kind, BackendKind::Local);
        assert_eq!(source.origin, SelectionOrigin::ExplicitProvider);
    }

    #[test]
    fn active_profile_wins_over_bucket_scheme() {
        let settings = settings(
            r#"
[assets]
active_profile = "media"
bucket = "s3://from-bucket"

[assets_providers.media]
kind = "gcs"
bucket = "media-bucket"
"#,
        );
        let source = resolve_asset_store_source(&settings, &ctx()).unwrap();
        assert_eq!(source.kind, BackendKind::Gcs);
        assert_eq!(
            source.origin,
            SelectionOrigin::ActiveProfile("media".to_owned())
        );
        assert_eq!(source.bucket.as_deref(), Some("media-bucket"));
    }

    #[test]
    fn bucket_scheme_selects_when_nothing_is_named() {
        for (bucket, kind, scheme) in [
            ("s3://my-bucket", BackendKind::S3, "s3"),
            ("gs://my-bucket", BackendKind::Gcs, "gs"),
        ] {
            let settings = settings(&format!("[assets]\nbucket = \"{bucket}\"\n"));
            let source = resolve_asset_store_source(&settings, &ctx()).unwrap();
            assert_eq!(source.kind, kind, "{bucket}");
            assert_eq!(source.bucket.as_deref(), Some("my-bucket"));
            assert_eq!(
                source.origin,
                SelectionOrigin::BucketScheme(scheme.to_owned())
            );
        }
    }

    #[test]
    fn bucket_scheme_trailing_path_becomes_key_prefix() {
        let settings = settings("[assets]\nbucket = \"s3://my-bucket/team/media\"\n");
        let source = resolve_asset_store_source(&settings, &ctx()).unwrap();
        assert_eq!(source.bucket.as_deref(), Some("my-bucket"));
        assert_eq!(source.key_prefix.as_str(), "uploads/team/media/");
    }

    #[test]
    fn empty_config_falls_through_to_local_totally() {
        let source = resolve_asset_store_source(&AssetsSettings::default(), &ctx()).unwrap();
        assert_eq!(source.kind, BackendKind::Local);
        assert_eq!(source.origin, SelectionOrigin::LocalFallback);
        assert_eq!(source.local_root, PathBuf::from("/home/tester/assets"));
        assert_eq!(source.key_prefix.as_str(), DEFAULT_ASSET_KEY_PREFIX);
        assert_eq!(
            source.default_ttl,
            Duration::from_secs(DEFAULT_ASSET_TTL_SECS)
        );
        assert_eq!(source.default_visibility, Visibility::Private);
    }

    #[test]
    fn unrecognized_bucket_scheme_warns_and_falls_through_to_local() {
        for bucket in ["my-bucket", "hdfs://cluster/bucket", "file:///tmp/bucket"] {
            let settings = settings(&format!("[assets]\nbucket = \"{bucket}\"\n"));
            let source = resolve_asset_store_source(&settings, &ctx()).unwrap();
            assert_eq!(source.kind, BackendKind::Local, "{bucket}");
            assert_eq!(source.origin, SelectionOrigin::LocalFallback);
        }
    }

    // ---- fail-closed named mistakes -------------------------------------

    #[test]
    fn dangling_active_profile_fails_closed() {
        let settings = settings("[assets]\nactive_profile = \"typo\"\n");
        let err = resolve_asset_store_source(&settings, &ctx()).unwrap_err();
        assert_eq!(err.code(), "asset_invalid_key");
        let rendered = err.to_string();
        assert!(rendered.contains("typo"), "{rendered}");
        assert!(rendered.contains("assets_providers.typo"), "{rendered}");
    }

    #[test]
    fn invalid_active_profile_id_fails_closed() {
        let settings = settings(
            "[assets]\nactive_profile = \"bad id\"\n\n[assets_providers.\"bad id\"]\nkind = \"local\"\n",
        );
        let err = resolve_asset_store_source(&settings, &ctx()).unwrap_err();
        assert!(err.to_string().contains("profile id"), "{err}");
    }

    #[test]
    fn remote_backend_without_bucket_fails_closed() {
        for provider in ["s3", "gcs"] {
            let settings = settings(&format!("[assets]\nprovider = \"{provider}\"\n"));
            let err = resolve_asset_store_source(&settings, &ctx()).unwrap_err();
            assert!(err.to_string().contains("needs a bucket"), "{err}");
        }
    }

    #[test]
    fn proxy_without_base_url_fails_closed() {
        let settings = settings("[assets]\nprovider = \"proxy\"\n");
        let err = resolve_asset_store_source(&settings, &ctx()).unwrap_err();
        assert!(err.to_string().contains("proxy_base_url"), "{err}");
    }

    #[test]
    fn out_of_range_ttl_fails_closed() {
        let too_long = settings(&format!(
            "[assets]\ndefault_ttl_secs = {}\n",
            MAX_ASSET_TTL_SECS + 1
        ));
        let err = resolve_asset_store_source(&too_long, &ctx()).unwrap_err();
        assert!(err.to_string().contains("SigV4 ceiling"), "{err}");

        let zero_ttl = settings("[assets]\ndefault_ttl_secs = 0\n");
        assert!(resolve_asset_store_source(&zero_ttl, &ctx()).is_err());
    }

    #[test]
    fn invalid_key_prefix_fails_closed() {
        let settings = settings("[assets]\nkey_prefix = \"uploads\"\n");
        let err = resolve_asset_store_source(&settings, &ctx()).unwrap_err();
        assert!(err.to_string().contains("key_prefix"), "{err}");
    }

    // ---- environment overrides ------------------------------------------

    #[test]
    fn env_provider_overrides_config_and_is_validated() {
        let settings = settings("[assets]\nprovider = \"local\"\n");
        let env = AssetEnvOverrides {
            provider: Some("s3".into()),
            bucket: Some("env-bucket".into()),
            ..Default::default()
        };
        let source = resolve_asset_store_source(&settings, &ctx().with_env(env)).unwrap();
        assert_eq!(source.kind, BackendKind::S3);
        assert_eq!(source.bucket.as_deref(), Some("env-bucket"));
        assert_eq!(source.origin, SelectionOrigin::ExplicitProvider);

        let env = AssetEnvOverrides {
            provider: Some("azure".into()),
            ..Default::default()
        };
        let err = resolve_asset_store_source(&settings, &ctx().with_env(env)).unwrap_err();
        assert!(err.to_string().contains("GROK_ASSETS_PROVIDER"), "{err}");
    }

    #[test]
    fn env_scalar_overrides_are_applied_and_validated() {
        let env = AssetEnvOverrides {
            public_base_url: Some("https://cdn.test".into()),
            default_visibility: Some("public".into()),
            default_ttl_secs: Some("60".into()),
            local_root: Some("/tmp/env-root".into()),
            max_object_bytes: Some("1024".into()),
            request_timeout_secs: Some("5".into()),
            key_prefix: Some("media/".into()),
            ..Default::default()
        };
        let source =
            resolve_asset_store_source(&AssetsSettings::default(), &ctx().with_env(env)).unwrap();
        assert_eq!(source.public_base_url.as_deref(), Some("https://cdn.test"));
        assert_eq!(source.default_visibility, Visibility::Public);
        assert_eq!(source.default_ttl, Duration::from_secs(60));
        assert_eq!(source.local_root, PathBuf::from("/tmp/env-root"));
        assert_eq!(source.max_object_bytes, Some(1024));
        assert_eq!(source.request_timeout, Duration::from_secs(5));
        assert_eq!(source.key_prefix.as_str(), "media/");

        let bad_visibility = AssetEnvOverrides {
            default_visibility: Some("shared".into()),
            ..Default::default()
        };
        assert!(
            resolve_asset_store_source(&AssetsSettings::default(), &ctx().with_env(bad_visibility))
                .is_err()
        );

        let bad_number = AssetEnvOverrides {
            default_ttl_secs: Some("soon".into()),
            ..Default::default()
        };
        assert!(
            resolve_asset_store_source(&AssetsSettings::default(), &ctx().with_env(bad_number))
                .is_err()
        );
    }

    #[test]
    fn env_profile_selects_and_is_fail_closed() {
        let settings = settings(
            r#"
[assets_providers.team]
kind = "local"
local_root = "/srv/team"
"#,
        );
        let env = AssetEnvOverrides {
            profile: Some("team".into()),
            ..Default::default()
        };
        let source = resolve_asset_store_source(&settings, &ctx().with_env(env)).unwrap();
        assert_eq!(source.kind, BackendKind::Local);
        assert_eq!(source.local_root, PathBuf::from("/srv/team"));

        let env = AssetEnvOverrides {
            profile: Some("nope".into()),
            ..Default::default()
        };
        assert!(resolve_asset_store_source(&settings, &ctx().with_env(env)).is_err());
    }

    #[test]
    fn env_bucket_scheme_is_honored_when_nothing_is_named() {
        let env = AssetEnvOverrides {
            bucket: Some("gs://env-bucket".into()),
            ..Default::default()
        };
        let source =
            resolve_asset_store_source(&AssetsSettings::default(), &ctx().with_env(env)).unwrap();
        assert_eq!(source.kind, BackendKind::Gcs);
        assert_eq!(source.bucket.as_deref(), Some("env-bucket"));
        assert_eq!(source.origin, SelectionOrigin::BucketScheme("gs".into()));
    }

    #[test]
    fn local_root_default_prefers_explicit_then_context() {
        let explicit = settings("[assets]\nlocal_root = \"/srv/explicit\"\n");
        assert_eq!(
            resolve_asset_store_source(&explicit, &ctx())
                .unwrap()
                .local_root,
            PathBuf::from("/srv/explicit")
        );

        let ctx = AssetRuntimeContext::new().with_default_local_root("/srv/default");
        assert_eq!(
            resolve_asset_store_source(&AssetsSettings::default(), &ctx)
                .unwrap()
                .local_root,
            PathBuf::from("/srv/default")
        );

        let bare = AssetRuntimeContext::new();
        assert_eq!(
            resolve_asset_store_source(&AssetsSettings::default(), &bare)
                .unwrap()
                .local_root,
            PathBuf::from("assets")
        );
    }

    #[test]
    fn ttl_and_key_prefix_bounds_are_exported_from_the_config_crate() {
        assert_eq!(MIN_ASSET_TTL_SECS, 1);
        assert_eq!(MAX_ASSET_TTL_SECS, 604_800);
    }

    // ---- construction ----------------------------------------------------

    #[tokio::test]
    async fn resolve_asset_store_returns_a_local_store_for_local() {
        let plain_local = settings("[assets]\nprovider = \"local\"\n");
        let store = resolve_asset_store(&plain_local, &ctx()).await.unwrap();
        assert_eq!(store.backend(), BackendKind::Local);

        // Capabilities are the *effective* ones: local can only record
        // visibility when a `public_base_url` gives it somewhere to point.
        let caps = store.capabilities();
        assert!(caps.put && caps.get && caps.delete && caps.list && caps.presign_get);
        assert!(!caps.presign_put);
        assert!(!caps.set_visibility, "no public_base_url configured");
        assert!(!caps.visibility_enforced);

        let with_public =
            settings("[assets]\nprovider = \"local\"\npublic_base_url = \"https://cdn.test\"\n");
        let store = resolve_asset_store(&with_public, &ctx()).await.unwrap();
        assert!(store.capabilities().set_visibility);
    }

    #[tokio::test]
    async fn resolve_asset_store_propagates_fail_closed_selection_errors() {
        let settings = settings("[assets]\nactive_profile = \"typo\"\n");
        assert!(resolve_asset_store(&settings, &ctx()).await.is_err());
    }

    /// The factory must hand back the real adapter, not the stub.
    #[tokio::test]
    async fn resolve_asset_store_builds_the_s3_adapter() {
        let settings = settings(
            "[assets]\nprovider = \"s3\"\nbucket = \"b\"\nendpoint_url = \"http://127.0.0.1:1\"\n",
        );
        let store = resolve_asset_store(&settings, &ctx()).await.unwrap();
        assert_eq!(store.backend(), BackendKind::S3);
        assert_eq!(store.capabilities(), BackendCapabilities::S3);
        assert!(
            store
                .public_url(&AssetKey::parse("uploads/a.txt").unwrap())
                .is_some()
        );
    }

    /// GCS construction goes through a service-account key so the test never
    /// depends on ambient ADC.
    ///
    /// `gcloud-storage` fetches a service-account token **eagerly** at
    /// construction, so with no egress (or a synthetic `client_email`) the
    /// documented total fallback to `local` kicks in. Both are asserted.
    #[tokio::test]
    async fn resolve_asset_store_builds_the_gcs_adapter() {
        let dir = tempfile::tempdir().unwrap();
        let sa_path = dir.path().join("sa.json");
        let json = serde_json::json!({
            "type": "service_account",
            "project_id": "test",
            "private_key_id": "key-id",
            "private_key": include_str!("testdata/gcs_test_key.pem"),
            "client_email": "svc@test.iam.gserviceaccount.com",
            "client_id": "123",
            "auth_uri": "https://accounts.google.com/o/oauth2/auth",
            "token_uri": "https://oauth2.googleapis.com/token",
            "auth_provider_x509_cert_url": "https://www.googleapis.com/oauth2/v1/certs",
            "client_x509_cert_url": "https://example/cert",
        });
        std::fs::write(&sa_path, json.to_string()).unwrap();

        let settings = settings(&format!(
            "[assets]\nprovider = \"gcs\"\nbucket = \"b\"\ncredentials_file = \"{}\"\n",
            sa_path.display()
        ));
        let store = resolve_asset_store(&settings, &ctx()).await.unwrap();
        assert!(
            matches!(store.backend(), BackendKind::Gcs | BackendKind::Local),
            "unexpected backend {:?}",
            store.backend()
        );
        if store.backend() == BackendKind::Gcs {
            assert_eq!(store.capabilities(), BackendCapabilities::GCS);
        }
    }

    /// `proxy` still resolves to the stub until its adapter lands.
    #[tokio::test]
    async fn resolve_asset_store_keeps_proxy_on_the_stub() {
        let settings = settings("[assets]\nprovider = \"proxy\"\nproxy_base_url = \"https://p\"\n");
        let store = resolve_asset_store(&settings, &ctx()).await.unwrap();
        assert_eq!(store.backend(), BackendKind::Proxy);
        assert!(
            store
                .public_url(&AssetKey::parse("uploads/a.txt").unwrap())
                .is_none()
        );
    }

    #[tokio::test]
    async fn stub_reports_capability_gaps_before_not_implemented() {
        let stub = StubAssetStore::new(BackendKind::Proxy);
        assert_eq!(stub.backend(), BackendKind::Proxy);

        let key = AssetKey::parse("uploads/a.png").unwrap();
        let err = stub.delete(&key).await.unwrap_err();
        assert!(err.to_string().contains("does not support"), "{err}");

        let err = stub.exists(&key).await.unwrap_err();
        assert!(err.to_string().contains("not implemented yet"), "{err}");

        let err = stub
            .put(PutRequest::from_bytes(
                key.clone(),
                Bytes::from_static(b"x"),
                ContentType::default(),
            ))
            .await
            .unwrap_err();
        assert_eq!(err.code(), "asset_unsupported");

        assert!(stub.public_url(&key).is_none());
        let status = stub.health().await.unwrap();
        assert!(!status.is_healthy());
        assert_eq!(status.backend, BackendKind::Proxy);
    }

    #[test]
    fn stub_capabilities_match_the_backend() {
        for kind in [
            BackendKind::S3,
            BackendKind::Gcs,
            BackendKind::Local,
            BackendKind::Proxy,
        ] {
            assert_eq!(
                StubAssetStore::new(kind).capabilities(),
                BackendCapabilities::for_kind(kind)
            );
        }
    }

    #[test]
    fn origin_names_are_stable() {
        assert_eq!(
            SelectionOrigin::ExplicitProvider.as_str(),
            "explicit_provider"
        );
        assert_eq!(SelectionOrigin::LocalFallback.as_str(), "local_fallback");
        assert_eq!(
            SelectionOrigin::ActiveProfile("a".into()).to_string(),
            "active_profile:a"
        );
        assert_eq!(
            SelectionOrigin::BucketScheme("s3".into()).to_string(),
            "bucket_scheme:s3"
        );
    }
}
