//! Request/response value types for the asset store.
//!
//! Everything here is plain data: no I/O, no backend knowledge. The types are
//! deliberately `Clone` so a caller can retry or fan out without rebuilding.

use std::path::{Path, PathBuf};
use std::time::Duration;

use bytes::Bytes;
use chrono::{DateTime, Utc};

use super::error::AssetError;
use super::key::{AssetKey, AssetPrefix, ContentType};
use super::progress::ProgressHandle;
use super::{BackendKind, MAX_LIST_LIMIT, MIN_LIST_LIMIT};

/// Whether an object is publicly readable.
///
/// Private is the default and public is always explicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Visibility {
    #[default]
    Private,
    Public,
}

impl Visibility {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Private => "private",
            Self::Public => "public",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "private" => Some(Self::Private),
            "public" => Some(Self::Public),
            _ => None,
        }
    }

    pub const fn is_public(self) -> bool {
        matches!(self, Self::Public)
    }
}

impl std::fmt::Display for Visibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<xai_grok_config_types::AssetVisibility> for Visibility {
    fn from(value: xai_grok_config_types::AssetVisibility) -> Self {
        match value {
            xai_grok_config_types::AssetVisibility::Private => Self::Private,
            xai_grok_config_types::AssetVisibility::Public => Self::Public,
        }
    }
}

impl From<Visibility> for xai_grok_config_types::AssetVisibility {
    fn from(value: Visibility) -> Self {
        match value {
            Visibility::Private => Self::Private,
            Visibility::Public => Self::Public,
        }
    }
}

/// Metadata describing a stored object.
///
/// `visibility_enforced` is the honest field: it is `false` whenever the
/// backend only *records* visibility without enforcing it, so callers never
/// present a recorded flag as an access-control guarantee.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetMeta {
    pub key: AssetKey,
    pub size_bytes: u64,
    pub content_type: ContentType,
    pub visibility: Visibility,
    pub visibility_enforced: bool,
    pub backend: BackendKind,
    pub modified_at: Option<String>,
    pub etag: Option<String>,
}

impl AssetMeta {
    /// Build metadata with the defaults a backend almost always wants.
    pub fn new(
        key: AssetKey,
        size_bytes: u64,
        content_type: ContentType,
        backend: BackendKind,
    ) -> Self {
        Self {
            key,
            size_bytes,
            content_type,
            visibility: Visibility::Private,
            visibility_enforced: false,
            backend,
            modified_at: None,
            etag: None,
        }
    }

    pub fn with_visibility(mut self, visibility: Visibility, enforced: bool) -> Self {
        self.visibility = visibility;
        self.visibility_enforced = enforced;
        self
    }
}

/// HTTP method a presigned URL is bound to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PresignMethod {
    Get,
    Put,
}

impl PresignMethod {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Put => "PUT",
        }
    }
}

impl std::fmt::Display for PresignMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A time-limited URL for one object.
///
/// `emulated` marks URLs the backend synthesized rather than signed (the local
/// `file://` path, a GCS emulation): a caller may need to know that the URL
/// carries no cryptographic authorization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresignedUrl {
    pub key: AssetKey,
    pub url: String,
    pub method: PresignMethod,
    pub expires_in: Duration,
    pub expires_at: DateTime<Utc>,
    pub content_type: Option<String>,
    pub emulated: bool,
}

impl PresignedUrl {
    /// Build a URL that expires `ttl` from now.
    pub fn new(
        key: AssetKey,
        url: String,
        method: PresignMethod,
        ttl: Duration,
        emulated: bool,
    ) -> Self {
        let expires_at = Utc::now()
            + chrono::Duration::from_std(ttl).unwrap_or_else(|_| chrono::Duration::seconds(0));
        Self {
            key,
            url,
            method,
            expires_in: ttl,
            expires_at,
            content_type: None,
            emulated,
        }
    }

    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    /// True when the URL is already past its expiry.
    pub fn is_expired(&self) -> bool {
        self.expires_at <= Utc::now()
    }
}

/// Where the bytes of a [`PutRequest`] come from.
///
/// `put_file` streams a `File` source; `put` buffers whatever it is given.
#[derive(Debug, Clone)]
pub enum PutSource {
    Bytes(Bytes),
    File(PathBuf),
}

impl PutSource {
    pub fn is_file(&self) -> bool {
        matches!(self, Self::File(_))
    }

    pub fn file_path(&self) -> Option<&Path> {
        match self {
            Self::File(path) => Some(path),
            Self::Bytes(_) => None,
        }
    }

    /// Size when it is known without touching the filesystem.
    pub fn buffered_len(&self) -> Option<u64> {
        match self {
            Self::Bytes(bytes) => Some(bytes.len() as u64),
            Self::File(_) => None,
        }
    }

    /// The buffered bytes, or `None` for a file source.
    pub fn as_bytes(&self) -> Option<&Bytes> {
        match self {
            Self::Bytes(bytes) => Some(bytes),
            Self::File(_) => None,
        }
    }
}

impl From<Bytes> for PutSource {
    fn from(value: Bytes) -> Self {
        Self::Bytes(value)
    }
}

impl From<Vec<u8>> for PutSource {
    fn from(value: Vec<u8>) -> Self {
        Self::Bytes(Bytes::from(value))
    }
}

impl From<PathBuf> for PutSource {
    fn from(value: PathBuf) -> Self {
        Self::File(value)
    }
}

/// One upload.
#[derive(Debug, Clone)]
pub struct PutRequest {
    pub key: AssetKey,
    pub source: PutSource,
    pub content_type: ContentType,
    pub visibility: Visibility,
    /// Size the caller already knows; lets a backend reject early.
    pub expected_size: Option<u64>,
    /// Byte sink the adapter reports copied chunks into, when anyone listens.
    pub progress: Option<ProgressHandle>,
}

impl PutRequest {
    pub fn from_bytes(key: AssetKey, bytes: impl Into<Bytes>, content_type: ContentType) -> Self {
        let bytes = bytes.into();
        Self {
            key,
            source: PutSource::Bytes(bytes),
            content_type,
            visibility: Visibility::default(),
            expected_size: None,
            progress: None,
        }
    }

    pub fn from_file(key: AssetKey, path: impl Into<PathBuf>, content_type: ContentType) -> Self {
        Self {
            key,
            source: PutSource::File(path.into()),
            content_type,
            visibility: Visibility::default(),
            expected_size: None,
            progress: None,
        }
    }

    pub fn with_visibility(mut self, visibility: Visibility) -> Self {
        self.visibility = visibility;
        self
    }

    pub fn with_expected_size(mut self, size: u64) -> Self {
        self.expected_size = Some(size);
        self
    }

    /// Attach a progress sink. The handle also carries the payload total when
    /// the caller already knows it.
    pub fn with_progress(mut self, progress: ProgressHandle) -> Self {
        self.progress = Some(progress);
        self
    }

    /// Size when known without I/O: the declared size, else the buffered length.
    pub fn known_size(&self) -> Option<u64> {
        self.expected_size.or_else(|| self.source.buffered_len())
    }
}

/// Opaque pagination cursor.
///
/// The wire form is backend-defined; callers pass it back verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ListCursor(String);

impl ListCursor {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl std::fmt::Display for ListCursor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// One page request.
#[derive(Debug, Clone)]
pub struct ListQuery {
    pub prefix: AssetPrefix,
    pub limit: u32,
    pub cursor: Option<ListCursor>,
    /// Include the reserved `_meta/` sidecar namespace. Hidden by default.
    pub include_meta: bool,
}

impl Default for ListQuery {
    fn default() -> Self {
        Self {
            prefix: AssetPrefix::default(),
            limit: super::DEFAULT_LIST_LIMIT,
            cursor: None,
            include_meta: false,
        }
    }
}

impl ListQuery {
    pub fn new(prefix: AssetPrefix) -> Self {
        Self {
            prefix,
            ..Self::default()
        }
    }

    pub fn with_limit(mut self, limit: u32) -> Self {
        self.limit = limit;
        self
    }

    pub fn with_cursor(mut self, cursor: ListCursor) -> Self {
        self.cursor = Some(cursor);
        self
    }

    pub fn with_include_meta(mut self, include_meta: bool) -> Self {
        self.include_meta = include_meta;
        self
    }

    /// Enforce the documented limit window before any backend call.
    pub fn validate(&self) -> Result<(), AssetError> {
        if self.limit < MIN_LIST_LIMIT || self.limit > MAX_LIST_LIMIT {
            return Err(AssetError::InvalidKey {
                key: self.prefix.to_string(),
                reason: format!(
                    "list limit {} is outside {MIN_LIST_LIMIT}..={MAX_LIST_LIMIT}",
                    self.limit
                ),
            });
        }
        Ok(())
    }
}

/// One page of results.
#[derive(Debug, Clone)]
pub struct ListPage {
    pub items: Vec<AssetMeta>,
    pub next_cursor: Option<ListCursor>,
    /// True when the backend stopped early — there may be more items.
    pub truncated: bool,
}

impl ListPage {
    pub fn empty() -> Self {
        Self {
            items: Vec::new(),
            next_cursor: None,
            truncated: false,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// True when [`Self::next_cursor`] can be followed.
    pub fn has_more(&self) -> bool {
        self.next_cursor.is_some()
    }
}

/// Result of a delete. Idempotent: a missing key is not an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeleteOutcome {
    Deleted,
    NotFound,
}

impl DeleteOutcome {
    pub const fn is_deleted(self) -> bool {
        matches!(self, Self::Deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> AssetKey {
        AssetKey::parse("uploads/a.png").unwrap()
    }

    #[test]
    fn visibility_defaults_to_private_and_maps_to_config() {
        assert_eq!(Visibility::default(), Visibility::Private);
        assert!(!Visibility::default().is_public());
        assert_eq!(Visibility::parse("Public"), Some(Visibility::Public));
        assert_eq!(Visibility::parse("nope"), None);

        let config: xai_grok_config_types::AssetVisibility = Visibility::Public.into();
        assert_eq!(config, xai_grok_config_types::AssetVisibility::Public);
        assert_eq!(Visibility::from(config), Visibility::Public);
    }

    #[test]
    fn put_request_helpers_set_defaults() {
        let request = PutRequest::from_bytes(key(), b"hello".to_vec(), ContentType::default());
        assert_eq!(request.visibility, Visibility::Private);
        assert_eq!(request.known_size(), Some(5));
        assert!(!request.source.is_file());
        assert!(request.progress.is_none());

        let request = request.with_visibility(Visibility::Public);
        assert!(request.visibility.is_public());

        let handle = ProgressHandle::with_total(5);
        let request = request.with_progress(handle.clone());
        handle.add(5);
        assert_eq!(
            request.progress.as_ref().map(ProgressHandle::transferred),
            Some(5)
        );

        let file = PutRequest::from_file(key(), "/tmp/a.png", ContentType::default());
        assert!(file.source.is_file());
        assert_eq!(file.source.file_path(), Some(Path::new("/tmp/a.png")));
        assert_eq!(file.known_size(), None);
        assert_eq!(file.with_expected_size(9).known_size(), Some(9));
    }

    #[test]
    fn list_query_defaults_and_validation() {
        let query = ListQuery::default();
        assert!(query.prefix.is_empty());
        assert_eq!(query.limit, super::super::DEFAULT_LIST_LIMIT);
        assert!(query.cursor.is_none());
        assert!(!query.include_meta);
        query.validate().unwrap();

        assert!(ListQuery::default().with_limit(0).validate().is_err());
        assert!(
            ListQuery::default()
                .with_limit(super::super::MAX_LIST_LIMIT + 1)
                .validate()
                .is_err()
        );
        ListQuery::default()
            .with_limit(super::super::MIN_LIST_LIMIT)
            .validate()
            .unwrap();
        ListQuery::default()
            .with_limit(super::super::MAX_LIST_LIMIT)
            .validate()
            .unwrap();

        let cursor = ListCursor::new("opaque:1");
        let query = ListQuery::new(AssetPrefix::parse("uploads/").unwrap())
            .with_cursor(cursor.clone())
            .with_include_meta(true);
        assert_eq!(query.cursor.as_ref(), Some(&cursor));
        assert!(query.include_meta);
        assert_eq!(cursor.as_str(), "opaque:1");
    }

    #[test]
    fn list_page_reports_more_items() {
        let empty = ListPage::empty();
        assert!(empty.is_empty());
        assert!(!empty.has_more());

        let page = ListPage {
            items: vec![AssetMeta::new(
                key(),
                3,
                ContentType::default(),
                BackendKind::Local,
            )],
            next_cursor: Some(ListCursor::new("next")),
            truncated: true,
        };
        assert!(!page.is_empty());
        assert!(page.has_more());
        assert!(page.truncated);
    }

    #[test]
    fn delete_outcome_is_explicit() {
        assert!(DeleteOutcome::Deleted.is_deleted());
        assert!(!DeleteOutcome::NotFound.is_deleted());
    }

    #[test]
    fn presigned_url_reports_expiry_and_emulation() {
        let url = PresignedUrl::new(
            key(),
            "file:///tmp/assets/uploads/a.png".into(),
            PresignMethod::Get,
            Duration::from_secs(60),
            true,
        );
        assert!(url.emulated);
        assert!(!url.is_expired());
        assert_eq!(url.method.as_str(), "GET");
        assert_eq!(url.method.to_string(), "GET");
        assert!(url.content_type.is_none());
        assert_eq!(
            url.clone()
                .with_content_type("image/png")
                .content_type
                .as_deref(),
            Some("image/png")
        );

        let expired = PresignedUrl::new(
            key(),
            "file:///x".into(),
            PresignMethod::Put,
            Duration::from_secs(0),
            false,
        );
        assert!(expired.is_expired());
        assert_eq!(PresignMethod::Put.as_str(), "PUT");
    }

    #[test]
    fn asset_meta_is_private_by_default_and_honest_about_enforcement() {
        let meta = AssetMeta::new(key(), 12, ContentType::default(), BackendKind::S3);
        assert_eq!(meta.visibility, Visibility::Private);
        assert!(!meta.visibility_enforced);
        assert_eq!(meta.backend, BackendKind::S3);
        assert!(meta.modified_at.is_none());
        assert!(meta.etag.is_none());

        let public = meta.with_visibility(Visibility::Public, true);
        assert!(public.visibility.is_public());
        assert!(public.visibility_enforced);
    }

    #[test]
    fn put_source_conversions_are_available() {
        let from_bytes: PutSource = Bytes::from_static(b"x").into();
        assert!(matches!(from_bytes, PutSource::Bytes(_)));
        let from_vec: PutSource = vec![1u8, 2].into();
        assert_eq!(from_vec.buffered_len(), Some(2));
        assert!(from_vec.as_bytes().is_some());
        let from_path: PutSource = PathBuf::from("/tmp/a").into();
        assert!(from_path.is_file());
        assert!(from_path.as_bytes().is_none());
    }
}
