//! Multi-backend object storage behind one async trait.
//!
//! The subsystem exists so that five `asset_*` tools can upload, share, list,
//! and delete objects without knowing whether the bytes land in S3, GCS, the
//! upload proxy, or a local directory. Everything here is async on tokio: the
//! trait's futures are `Send`, so an `Arc<dyn AssetStore>` moves into
//! `tokio::spawn` and lives in `Resources` without further ceremony.
//!
//! Design rules that the code encodes rather than documents:
//!
//! - **Private by default.** [`Visibility::Private`] is `#[default]`; public is
//!   always explicit.
//! - **`Unsupported` is explicit.** A backend that cannot perform an operation
//!   returns [`AssetError::Unsupported`] through
//!   [`BackendCapabilities::require`] *before* any network call — never a fake
//!   success.
//! - **Capabilities are data.** [`BackendCapabilities`] is the single source of
//!   truth for the support matrix; adapters consult it instead of hard-coding
//!   `match` arms.
//! - **Keys are path-safe by construction.** See [`key`].
//! - **Errors are owned and secret-free.** See [`error`].

pub mod error;
pub mod factory;
pub mod gcs_store;
pub mod jobs;
pub mod key;
pub mod local_store;
#[cfg(any(test, feature = "test-support"))]
pub mod mock;
pub mod progress;
pub mod s3_store;
pub mod value;

pub use error::{AssetError, AssetOperation};
pub use factory::{
    AssetEnvOverrides, AssetRuntimeContext, AssetStoreSource, SelectionOrigin, StubAssetStore,
    resolve_asset_store, resolve_asset_store_source,
};
pub use gcs_store::GcsAssetStore;
pub use jobs::{
    AssetJobRegistry, CancelOutcome, JobError, JobEvent, JobId, JobOutcome, JobSnapshot, JobState,
    JobSubscription, SubscribeOptions, TransferKind,
};
pub use key::{AssetKey, AssetPrefix, ContentType, ContentTypeError, KeyError};
pub use local_store::LocalAssetStore;
#[cfg(any(test, feature = "test-support"))]
pub use mock::{MockAssetStore, MockAssetStoreBuilder};
pub use progress::{PROGRESS_CHUNK_BYTES, ProgressHandle};
pub use s3_store::S3AssetStore;
pub use value::{
    AssetMeta, DeleteOutcome, ListCursor, ListPage, ListQuery, PresignMethod, PresignedUrl,
    PutRequest, PutSource, Visibility,
};

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use xai_grok_config_types::{DEFAULT_ASSET_LIST_LIMIT, MAX_ASSET_LIST_LIMIT, MIN_ASSET_LIST_LIMIT};

/// Default page size for [`ListQuery`].
pub const DEFAULT_LIST_LIMIT: u32 = DEFAULT_ASSET_LIST_LIMIT;
/// Smallest accepted page size.
pub const MIN_LIST_LIMIT: u32 = MIN_ASSET_LIST_LIMIT;
/// Largest accepted page size.
pub const MAX_LIST_LIMIT: u32 = MAX_ASSET_LIST_LIMIT;

/// The backend a store talks to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackendKind {
    S3,
    Gcs,
    Local,
    Proxy,
}

impl BackendKind {
    /// Stable snake_case name, used in errors, diagnostics, and tool output.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::S3 => "s3",
            Self::Gcs => "gcs",
            Self::Local => "local",
            Self::Proxy => "proxy",
        }
    }

    /// Map the config enum onto the runtime enum.
    pub const fn from_config(kind: xai_grok_config_types::AssetProviderKind) -> Self {
        match kind {
            xai_grok_config_types::AssetProviderKind::S3 => Self::S3,
            xai_grok_config_types::AssetProviderKind::Gcs => Self::Gcs,
            xai_grok_config_types::AssetProviderKind::Local => Self::Local,
            xai_grok_config_types::AssetProviderKind::Proxy => Self::Proxy,
        }
    }

    /// True when objects live in a remote object store.
    pub const fn is_remote(self) -> bool {
        matches!(self, Self::S3 | Self::Gcs)
    }
}

impl std::fmt::Display for BackendKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What one backend can actually do.
///
/// Encoded, not documented: adapters call [`Self::require`] at the top of an
/// operation so an unsupported call fails fast and identically everywhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendCapabilities {
    pub put: bool,
    pub put_file: bool,
    pub get: bool,
    pub download_to: bool,
    pub exists: bool,
    pub delete: bool,
    pub list: bool,
    pub presign_get: bool,
    pub presign_put: bool,
    pub set_visibility: bool,
    /// Whether a public marker is actually enforced by the backend.
    pub visibility_enforced: bool,
}

impl BackendCapabilities {
    /// Every operation supported; the baseline all four backends share.
    const ALL_OBJECT_OPS: Self = Self {
        put: true,
        put_file: true,
        get: true,
        download_to: true,
        exists: true,
        delete: false,
        list: false,
        presign_get: false,
        presign_put: false,
        set_visibility: false,
        visibility_enforced: false,
    };

    /// S3: everything native except visibility, which is emulated through
    /// object ACLs and may degrade to recorded-only on
    /// bucket-owner-enforced buckets.
    pub const S3: Self = Self {
        delete: true,
        list: true,
        presign_get: true,
        presign_put: true,
        set_visibility: true,
        visibility_enforced: true,
        ..Self::ALL_OBJECT_OPS
    };

    /// GCS: delete and list are native; presigning needs a service-account
    /// key, so it is reported as emulated support; visibility is unsupported.
    pub const GCS: Self = Self {
        delete: true,
        list: true,
        presign_get: true,
        presign_put: true,
        set_visibility: false,
        visibility_enforced: false,
        ..Self::ALL_OBJECT_OPS
    };

    /// Local directory: `presign_get` returns an emulated `file://` URL,
    /// `presign_put` is unsupported, and visibility is recorded when a
    /// `public_base_url` is configured.
    pub const LOCAL: Self = Self {
        delete: true,
        list: true,
        presign_get: true,
        presign_put: false,
        set_visibility: true,
        visibility_enforced: false,
        ..Self::ALL_OBJECT_OPS
    };

    /// Upload proxy: no delete and no list on the wire, everything else native
    /// or recorded.
    pub const PROXY: Self = Self {
        delete: false,
        list: false,
        presign_get: true,
        presign_put: true,
        set_visibility: true,
        visibility_enforced: false,
        ..Self::ALL_OBJECT_OPS
    };

    /// The support matrix for `kind`.
    pub const fn for_kind(kind: BackendKind) -> Self {
        match kind {
            BackendKind::S3 => Self::S3,
            BackendKind::Gcs => Self::GCS,
            BackendKind::Local => Self::LOCAL,
            BackendKind::Proxy => Self::PROXY,
        }
    }

    /// Whether `operation` is supported.
    pub const fn supports(&self, operation: AssetOperation) -> bool {
        match operation {
            AssetOperation::Put => self.put,
            AssetOperation::PutFile => self.put_file,
            AssetOperation::Get => self.get,
            AssetOperation::DownloadTo => self.download_to,
            AssetOperation::Exists => self.exists,
            AssetOperation::Delete => self.delete,
            AssetOperation::List => self.list,
            AssetOperation::PresignGet => self.presign_get,
            AssetOperation::PresignPut => self.presign_put,
            AssetOperation::SetVisibility => self.set_visibility,
            // Health is a local probe and is always answerable.
            AssetOperation::Health => true,
        }
    }

    /// Fast-fail gate: turn an unsupported operation into
    /// [`AssetError::Unsupported`] before any network call.
    pub fn require(
        &self,
        backend: BackendKind,
        operation: AssetOperation,
    ) -> Result<(), AssetError> {
        if self.supports(operation) {
            return Ok(());
        }
        Err(AssetError::unsupported(
            backend,
            operation,
            format!("{} does not implement {operation}", backend.as_str()),
        ))
    }

    /// Human-readable summary for diagnostics and tool output.
    pub fn unsupported_operations(&self) -> Vec<AssetOperation> {
        [
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
        ]
        .into_iter()
        .filter(|op| !self.supports(*op))
        .collect()
    }
}

/// Result of a [`AssetStore::health`] probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreStatus {
    pub backend: BackendKind,
    /// The store answered a cheap probe (stat / head).
    pub reachable: bool,
    /// A write probe succeeded.
    pub writable: bool,
    /// Secret-free detail, present when something failed.
    pub detail: Option<String>,
}

impl StoreStatus {
    pub fn healthy(backend: BackendKind) -> Self {
        Self {
            backend,
            reachable: true,
            writable: true,
            detail: None,
        }
    }

    pub fn degraded(backend: BackendKind, reachable: bool, detail: impl Into<String>) -> Self {
        Self {
            backend,
            reachable,
            writable: false,
            detail: Some(detail.into()),
        }
    }

    /// True only when the store is both reachable and writable.
    pub const fn is_healthy(&self) -> bool {
        self.reachable && self.writable
    }
}

/// A shared, type-erased store. Stored in `Resources` and handed to tools.
pub type SharedAssetStore = Arc<dyn AssetStore>;

/// The one storage abstraction the asset tools speak to.
///
/// `#[async_trait]` (rather than AFIT) because the store is always behind
/// `dyn`: it is `TypeId`-keyed in `Resources` and shared across sessions,
/// matching every other `Arc<dyn _>` trait in the repository.
///
/// Every method that touches the network must be cancellation-safe: dropping
/// the future must not leak a multipart upload.
#[async_trait::async_trait]
pub trait AssetStore: Send + Sync {
    /// Which backend this store talks to.
    fn backend(&self) -> BackendKind;

    /// What this store can do. Consulted by [`BackendCapabilities::require`].
    fn capabilities(&self) -> BackendCapabilities;

    /// Buffer-then-write upload. Use [`Self::put_file`] for large payloads.
    ///
    /// Reports progress through [`PutRequest::progress`] when set.
    async fn put(&self, request: PutRequest) -> Result<AssetMeta, AssetError>;

    /// Streaming upload. Must not materialize the whole object in memory.
    ///
    /// Reports progress through [`PutRequest::progress`] when set.
    async fn put_file(&self, request: PutRequest) -> Result<AssetMeta, AssetError>;

    /// Read the whole object into memory.
    async fn get(&self, key: &AssetKey) -> Result<Bytes, AssetError>;

    /// Stream the object to `dest`, creating parent directories.
    ///
    /// `progress`, when active, receives one report per copied chunk; the whole
    /// object is never materialized.
    async fn download_to(
        &self,
        key: &AssetKey,
        dest: &Path,
        progress: Option<ProgressHandle>,
    ) -> Result<AssetMeta, AssetError>;

    /// Cheap existence probe.
    async fn exists(&self, key: &AssetKey) -> Result<bool, AssetError>;

    /// Delete, idempotently: a missing key is [`DeleteOutcome::NotFound`].
    async fn delete(&self, key: &AssetKey) -> Result<DeleteOutcome, AssetError>;

    /// List one page. `_meta/` sidecars stay hidden unless asked for.
    async fn list(&self, query: ListQuery) -> Result<ListPage, AssetError>;

    /// Time-limited read URL.
    async fn presign_get(&self, key: &AssetKey, ttl: Duration) -> Result<PresignedUrl, AssetError>;

    /// Time-limited write URL bound to `content_type`.
    async fn presign_put(
        &self,
        key: &AssetKey,
        content_type: &ContentType,
        ttl: Duration,
    ) -> Result<PresignedUrl, AssetError>;

    /// Record (and, where the backend can, enforce) object visibility.
    async fn set_visibility(
        &self,
        key: &AssetKey,
        visibility: Visibility,
    ) -> Result<AssetMeta, AssetError>;

    /// Stable public URL, when the store has one configured.
    ///
    /// Synchronous because it is pure string work. `None` means "no public
    /// contract configured", not "object missing".
    fn public_url(&self, key: &AssetKey) -> Option<String>;

    /// Cheap reachability plus writability probe.
    async fn health(&self) -> Result<StoreStatus, AssetError>;
}

/// Validate a caller-supplied presign TTL against the SigV4 bounds.
///
/// Out of range is an error, never a silent clamp.
pub fn validate_ttl(ttl: Duration) -> Result<Duration, AssetError> {
    let secs = ttl.as_secs();
    if !xai_grok_config_types::asset_ttl_secs_in_range(secs) {
        return Err(AssetError::TtlOutOfRange {
            secs,
            min: xai_grok_config_types::MIN_ASSET_TTL_SECS,
            max: xai_grok_config_types::MAX_ASSET_TTL_SECS,
        });
    }
    Ok(ttl)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_BACKENDS: [BackendKind; 4] = [
        BackendKind::S3,
        BackendKind::Gcs,
        BackendKind::Local,
        BackendKind::Proxy,
    ];

    #[test]
    fn backend_names_are_stable() {
        assert_eq!(BackendKind::S3.as_str(), "s3");
        assert_eq!(BackendKind::Gcs.as_str(), "gcs");
        assert_eq!(BackendKind::Local.as_str(), "local");
        assert_eq!(BackendKind::Proxy.as_str(), "proxy");
        assert_eq!(BackendKind::S3.to_string(), "s3");
        assert!(BackendKind::S3.is_remote());
        assert!(!BackendKind::Local.is_remote());
    }

    #[test]
    fn backend_maps_from_config_kind() {
        use xai_grok_config_types::AssetProviderKind;
        assert_eq!(
            BackendKind::from_config(AssetProviderKind::S3),
            BackendKind::S3
        );
        assert_eq!(
            BackendKind::from_config(AssetProviderKind::Gcs),
            BackendKind::Gcs
        );
        assert_eq!(
            BackendKind::from_config(AssetProviderKind::Local),
            BackendKind::Local
        );
        assert_eq!(
            BackendKind::from_config(AssetProviderKind::Proxy),
            BackendKind::Proxy
        );
    }

    /// The capability matrix, asserted op by op for all four backends.
    #[test]
    fn capability_matrix_matches_the_contract() {
        let native = [
            AssetOperation::Put,
            AssetOperation::PutFile,
            AssetOperation::Get,
            AssetOperation::DownloadTo,
            AssetOperation::Exists,
        ];
        for backend in ALL_BACKENDS {
            let caps = BackendCapabilities::for_kind(backend);
            for op in native {
                assert!(caps.supports(op), "{backend} must support {op}");
            }
        }

        let delete_and_list = [AssetOperation::Delete, AssetOperation::List];
        for backend in ALL_BACKENDS {
            let caps = BackendCapabilities::for_kind(backend);
            let expected = !matches!(backend, BackendKind::Proxy);
            for op in delete_and_list {
                assert_eq!(
                    caps.supports(op),
                    expected,
                    "{backend} support for {op} must be {expected}"
                );
            }
        }

        for backend in ALL_BACKENDS {
            let caps = BackendCapabilities::for_kind(backend);
            assert!(caps.supports(AssetOperation::PresignGet), "{backend}");
            assert_eq!(
                caps.supports(AssetOperation::PresignPut),
                backend != BackendKind::Local,
                "{backend} presign_put"
            );
            assert_eq!(
                caps.supports(AssetOperation::SetVisibility),
                backend != BackendKind::Gcs,
                "{backend} set_visibility"
            );
            assert_eq!(
                caps.visibility_enforced,
                backend == BackendKind::S3,
                "{backend} visibility_enforced"
            );
            assert!(caps.supports(AssetOperation::Health));
        }
    }

    #[test]
    fn require_fast_fails_unsupported_operations() {
        let caps = BackendCapabilities::for_kind(BackendKind::Proxy);
        assert!(
            caps.require(BackendKind::Proxy, AssetOperation::Delete)
                .is_err()
        );
        assert!(
            caps.require(BackendKind::Proxy, AssetOperation::List)
                .is_err()
        );
        caps.require(BackendKind::Proxy, AssetOperation::Put)
            .unwrap();

        let err = caps
            .require(BackendKind::Proxy, AssetOperation::Delete)
            .unwrap_err();
        match err {
            AssetError::Unsupported {
                backend,
                operation,
                reason,
            } => {
                assert_eq!(backend, BackendKind::Proxy);
                assert_eq!(operation, AssetOperation::Delete);
                assert!(reason.contains("proxy"), "{reason}");
            }
            other => panic!("expected Unsupported, got {other:?}"),
        }

        // `presign_put` is the one gap for local.
        let caps = BackendCapabilities::for_kind(BackendKind::Local);
        assert!(
            caps.require(BackendKind::Local, AssetOperation::PresignPut)
                .is_err()
        );
        caps.require(BackendKind::Local, AssetOperation::PresignGet)
            .unwrap();

        // GCS cannot set visibility.
        let caps = BackendCapabilities::for_kind(BackendKind::Gcs);
        assert!(
            caps.require(BackendKind::Gcs, AssetOperation::SetVisibility)
                .is_err()
        );
        caps.require(BackendKind::Gcs, AssetOperation::PresignPut)
            .unwrap();
    }

    #[test]
    fn unsupported_operations_lists_every_gap() {
        assert_eq!(
            BackendCapabilities::for_kind(BackendKind::Proxy).unsupported_operations(),
            vec![AssetOperation::Delete, AssetOperation::List]
        );
        assert_eq!(
            BackendCapabilities::for_kind(BackendKind::S3).unsupported_operations(),
            Vec::<AssetOperation>::new()
        );
        assert_eq!(
            BackendCapabilities::for_kind(BackendKind::Local).unsupported_operations(),
            vec![AssetOperation::PresignPut]
        );
    }

    #[test]
    fn ttl_validation_rejects_out_of_range() {
        assert_eq!(
            validate_ttl(Duration::from_secs(1)).unwrap(),
            Duration::from_secs(1)
        );
        assert!(validate_ttl(Duration::from_secs(0)).is_err());
        let err = validate_ttl(Duration::from_secs(604_801)).unwrap_err();
        assert_eq!(err.code(), "asset_ttl_out_of_range");
        assert!(matches!(
            err,
            AssetError::TtlOutOfRange {
                secs: 604_801,
                min: 1,
                max: 604_800
            }
        ));
    }

    #[test]
    fn store_status_reports_health_honestly() {
        let healthy = StoreStatus::healthy(BackendKind::Local);
        assert!(healthy.is_healthy());
        assert!(healthy.detail.is_none());

        let degraded = StoreStatus::degraded(BackendKind::Local, true, "read-only mount");
        assert!(!degraded.is_healthy());
        assert!(degraded.reachable);
        assert!(!degraded.writable);
        assert_eq!(degraded.detail.as_deref(), Some("read-only mount"));

        let down = StoreStatus::degraded(BackendKind::S3, false, "no route");
        assert!(!down.is_healthy());
    }
}
