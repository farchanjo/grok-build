//! In-memory [`AssetStore`] for downstream tests.
//!
//! Gated behind `cfg(test)` or the `test-support` feature so tool and TUI test
//! suites get a deterministic seam without a filesystem or a network.
//!
//! The mock is deliberately *not* a stub: it stores bytes, enforces the
//! capability matrix, records visibility, and can be told to fail specific
//! operations. It also records every call so a test can assert on ordering and
//! on "no network call happened" invariants.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use bytes::Bytes;

use super::error::{AssetError, AssetOperation};
use super::key::{AssetKey, AssetPrefix, ContentType};
use super::value::{
    AssetMeta, DeleteOutcome, ListCursor, ListPage, ListQuery, PresignMethod, PresignedUrl,
    PutRequest, PutSource, Visibility,
};
use super::{AssetStore, BackendCapabilities, BackendKind, StoreStatus, validate_ttl};

#[derive(Debug, Default)]
struct State {
    objects: HashMap<String, (Bytes, ContentType)>,
    visibility: HashMap<String, Visibility>,
    calls: Vec<AssetOperation>,
    failures: HashMap<AssetOperation, AssetError>,
}

/// Configurable in-memory store. Build with [`MockAssetStore::builder`].
#[derive(Debug)]
pub struct MockAssetStore {
    backend: BackendKind,
    capabilities: BackendCapabilities,
    public_base_url: Option<String>,
    key_prefix: String,
    max_object_bytes: Option<u64>,
    state: Mutex<State>,
}

impl Default for MockAssetStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MockAssetStore {
    pub fn new() -> Self {
        Self::builder().build()
    }

    pub fn builder() -> MockAssetStoreBuilder {
        MockAssetStoreBuilder::default()
    }

    /// Operations the store has been asked to perform, in order.
    pub fn calls(&self) -> Vec<AssetOperation> {
        self.state
            .lock()
            .expect("mock state poisoned")
            .calls
            .clone()
    }

    /// True when `operation` was never called.
    pub fn was_called(&self, operation: AssetOperation) -> bool {
        self.calls().contains(&operation)
    }

    pub fn object_count(&self) -> usize {
        self.state
            .lock()
            .expect("mock state poisoned")
            .objects
            .len()
    }

    pub fn visibility_of(&self, key: &AssetKey) -> Option<Visibility> {
        self.state
            .lock()
            .expect("mock state poisoned")
            .visibility
            .get(key.as_str())
            .copied()
    }

    /// Raw stored bytes, bypassing `get` so a failing `get` can still be
    /// inspected.
    pub fn raw(&self, key: &AssetKey) -> Option<Bytes> {
        self.state
            .lock()
            .expect("mock state poisoned")
            .objects
            .get(key.as_str())
            .map(|(bytes, _)| bytes.clone())
    }

    fn record(&self, operation: AssetOperation) -> Result<(), AssetError> {
        let mut state = self.state.lock().expect("mock state poisoned");
        state.calls.push(operation);
        match state.failures.get(&operation) {
            Some(err) => Err(err.clone()),
            None => Ok(()),
        }
    }

    fn physical(&self, key: &AssetKey) -> String {
        format!("{}{key}", self.key_prefix)
    }

    fn gate(&self, operation: AssetOperation) -> Result<(), AssetError> {
        self.capabilities.require(self.backend, operation)
    }

    fn check_size(&self, key: &AssetKey, size: u64) -> Result<(), AssetError> {
        match self.max_object_bytes {
            Some(max) if size > max => Err(AssetError::ObjectTooLarge {
                key: key.to_string(),
                size,
                max,
            }),
            _ => Ok(()),
        }
    }

    /// Build metadata while the caller already holds the state lock.
    ///
    /// `std::sync::Mutex` is not reentrant, so the locked variants matter.
    fn meta_locked(
        &self,
        state: &State,
        key: &AssetKey,
        size: u64,
        content_type: ContentType,
    ) -> AssetMeta {
        let visibility = state
            .visibility
            .get(&self.physical(key))
            .copied()
            .unwrap_or_default();
        AssetMeta {
            key: key.clone(),
            size_bytes: size,
            content_type,
            visibility,
            visibility_enforced: self.capabilities.visibility_enforced,
            backend: self.backend,
            modified_at: None,
            etag: Some(format!("mock-{}", key.as_str().len())),
        }
    }

    fn meta(&self, key: &AssetKey, size: u64, content_type: ContentType) -> AssetMeta {
        let state = self.state.lock().expect("mock state poisoned");
        self.meta_locked(&state, key, size, content_type)
    }
}

/// Builder for [`MockAssetStore`].
#[derive(Debug, Default)]
pub struct MockAssetStoreBuilder {
    backend: Option<BackendKind>,
    capabilities: Option<BackendCapabilities>,
    public_base_url: Option<String>,
    key_prefix: String,
    max_object_bytes: Option<u64>,
    failures: Vec<(AssetOperation, AssetError)>,
}

impl MockAssetStoreBuilder {
    /// Select the backend (and its default capabilities).
    pub fn backend(mut self, backend: BackendKind) -> Self {
        self.backend = Some(backend);
        self
    }

    /// Override the capability matrix, e.g. to model a degraded backend.
    pub fn capabilities(mut self, capabilities: BackendCapabilities) -> Self {
        self.capabilities = Some(capabilities);
        self
    }

    pub fn public_base_url(mut self, url: impl Into<String>) -> Self {
        self.public_base_url = Some(url.into());
        self
    }

    pub fn key_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.key_prefix = prefix.into();
        self
    }

    pub fn max_object_bytes(mut self, max: u64) -> Self {
        self.max_object_bytes = Some(max);
        self
    }

    /// Make `operation` fail with `error` (after recording the call).
    pub fn fail_with(mut self, operation: AssetOperation, error: AssetError) -> Self {
        self.failures.push((operation, error));
        self
    }

    pub fn build(self) -> MockAssetStore {
        let backend = self.backend.unwrap_or(BackendKind::Local);
        let capabilities = self.capabilities.unwrap_or_else(|| {
            let mut caps = BackendCapabilities::for_kind(backend);
            // Local needs a public base URL to record visibility; mirror the
            // real store so tests exercise the same gate.
            if backend == BackendKind::Local && self.public_base_url.is_none() {
                caps.set_visibility = false;
            }
            caps
        });
        MockAssetStore {
            backend,
            capabilities,
            public_base_url: self.public_base_url,
            key_prefix: self.key_prefix,
            max_object_bytes: self.max_object_bytes,
            state: Mutex::new(State {
                failures: self.failures.into_iter().collect(),
                ..State::default()
            }),
        }
    }
}

#[async_trait::async_trait]
impl AssetStore for MockAssetStore {
    fn backend(&self) -> BackendKind {
        self.backend
    }

    fn capabilities(&self) -> BackendCapabilities {
        self.capabilities
    }

    async fn put(&self, request: PutRequest) -> Result<AssetMeta, AssetError> {
        self.gate(AssetOperation::Put)?;
        self.record(AssetOperation::Put)?;
        let bytes = match &request.source {
            PutSource::Bytes(bytes) => bytes.clone(),
            PutSource::File(path) => Bytes::from(
                tokio::fs::read(path)
                    .await
                    .map_err(|e| AssetError::io("read", &e))?,
            ),
        };
        self.check_size(&request.key, bytes.len() as u64)?;
        let mut state = self.state.lock().expect("mock state poisoned");
        state.objects.insert(
            self.physical(&request.key),
            (bytes.clone(), request.content_type.clone()),
        );
        state
            .visibility
            .insert(self.physical(&request.key), request.visibility);
        drop(state);
        Ok(self.meta(&request.key, bytes.len() as u64, request.content_type))
    }

    async fn put_file(&self, request: PutRequest) -> Result<AssetMeta, AssetError> {
        self.gate(AssetOperation::PutFile)?;
        self.record(AssetOperation::PutFile)?;
        let bytes = match &request.source {
            PutSource::Bytes(bytes) => bytes.clone(),
            PutSource::File(path) => Bytes::from(
                tokio::fs::read(path)
                    .await
                    .map_err(|e| AssetError::io("read", &e))?,
            ),
        };
        self.check_size(&request.key, bytes.len() as u64)?;
        let mut state = self.state.lock().expect("mock state poisoned");
        state.objects.insert(
            self.physical(&request.key),
            (bytes.clone(), request.content_type.clone()),
        );
        state
            .visibility
            .insert(self.physical(&request.key), request.visibility);
        drop(state);
        Ok(self.meta(&request.key, bytes.len() as u64, request.content_type))
    }

    async fn get(&self, key: &AssetKey) -> Result<Bytes, AssetError> {
        self.gate(AssetOperation::Get)?;
        self.record(AssetOperation::Get)?;
        self.raw(key).ok_or_else(|| AssetError::NotFound {
            key: key.to_string(),
        })
    }

    async fn download_to(&self, key: &AssetKey, dest: &Path) -> Result<AssetMeta, AssetError> {
        self.gate(AssetOperation::DownloadTo)?;
        self.record(AssetOperation::DownloadTo)?;
        let bytes = self.raw(key).ok_or_else(|| AssetError::NotFound {
            key: key.to_string(),
        })?;
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| AssetError::io("create_dir_all", &e))?;
        }
        tokio::fs::write(dest, &bytes)
            .await
            .map_err(|e| AssetError::io("write", &e))?;
        Ok(self.meta(key, bytes.len() as u64, ContentType::infer_from_path(dest)))
    }

    async fn exists(&self, key: &AssetKey) -> Result<bool, AssetError> {
        self.gate(AssetOperation::Exists)?;
        self.record(AssetOperation::Exists)?;
        Ok(self.raw(key).is_some())
    }

    async fn delete(&self, key: &AssetKey) -> Result<DeleteOutcome, AssetError> {
        self.gate(AssetOperation::Delete)?;
        self.record(AssetOperation::Delete)?;
        let mut state = self.state.lock().expect("mock state poisoned");
        let removed = state.objects.remove(&self.physical(key)).is_some();
        state.visibility.remove(&self.physical(key));
        Ok(if removed {
            DeleteOutcome::Deleted
        } else {
            DeleteOutcome::NotFound
        })
    }

    async fn list(&self, query: ListQuery) -> Result<ListPage, AssetError> {
        self.gate(AssetOperation::List)?;
        self.record(AssetOperation::List)?;
        query.validate()?;

        let prefix = query.prefix.as_str();
        let after = query.cursor.as_ref().map(ListCursor::as_str);
        let state = self.state.lock().expect("mock state poisoned");
        let mut names: Vec<String> = state
            .objects
            .keys()
            .filter(|name| {
                let logical = name.strip_prefix(&self.key_prefix).unwrap_or(name);
                (prefix.is_empty() || logical.starts_with(prefix))
                    && after.is_none_or(|cursor| name.as_str() > cursor)
            })
            .cloned()
            .collect();
        names.sort();

        let take = query.limit as usize;
        let truncated = names.len() > take;
        let items: Vec<AssetMeta> = names
            .iter()
            .take(take)
            .filter_map(|name| {
                let logical = name.strip_prefix(&self.key_prefix)?;
                let key = AssetKey::parse(logical).ok()?;
                let (bytes, content_type) = state.objects.get(name)?;
                Some(self.meta_locked(&state, &key, bytes.len() as u64, content_type.clone()))
            })
            .collect();
        let next_cursor = if truncated {
            names.get(take.saturating_sub(1)).map(ListCursor::new)
        } else {
            None
        };

        Ok(ListPage {
            items,
            next_cursor,
            truncated,
        })
    }

    async fn presign_get(&self, key: &AssetKey, ttl: Duration) -> Result<PresignedUrl, AssetError> {
        self.gate(AssetOperation::PresignGet)?;
        self.record(AssetOperation::PresignGet)?;
        let ttl = validate_ttl(ttl)?;
        Ok(PresignedUrl::new(
            key.clone(),
            format!("mock://{}/{}", self.backend, self.physical(key)),
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
        self.gate(AssetOperation::PresignPut)?;
        self.record(AssetOperation::PresignPut)?;
        let ttl = validate_ttl(ttl)?;
        Ok(PresignedUrl::new(
            key.clone(),
            format!("mock://{}/{}", self.backend, self.physical(key)),
            PresignMethod::Put,
            ttl,
            true,
        )
        .with_content_type(content_type.as_str()))
    }

    async fn set_visibility(
        &self,
        key: &AssetKey,
        visibility: Visibility,
    ) -> Result<AssetMeta, AssetError> {
        self.gate(AssetOperation::SetVisibility)?;
        self.record(AssetOperation::SetVisibility)?;
        let mut state = self.state.lock().expect("mock state poisoned");
        let Some((bytes, content_type)) = state.objects.get(&self.physical(key)).cloned() else {
            return Err(AssetError::NotFound {
                key: key.to_string(),
            });
        };
        state.visibility.insert(self.physical(key), visibility);
        drop(state);
        Ok(self.meta(key, bytes.len() as u64, content_type))
    }

    fn public_url(&self, key: &AssetKey) -> Option<String> {
        let base = self.public_base_url.as_deref()?;
        Some(format!(
            "{}/{}",
            base.trim_end_matches('/'),
            self.physical(key)
        ))
    }

    async fn health(&self) -> Result<StoreStatus, AssetError> {
        self.record(AssetOperation::Health)?;
        Ok(StoreStatus::healthy(self.backend))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(s: &str) -> AssetKey {
        AssetKey::parse(s).unwrap()
    }

    fn put_request(s: &str, body: &[u8]) -> PutRequest {
        PutRequest::from_bytes(key(s), body.to_vec(), ContentType::default())
    }

    #[tokio::test]
    async fn stores_and_returns_bytes() {
        let store = MockAssetStore::new();
        store
            .put(put_request("uploads/a.txt", b"hello"))
            .await
            .unwrap();

        assert_eq!(store.object_count(), 1);
        assert_eq!(
            &store.get(&key("uploads/a.txt")).await.unwrap()[..],
            b"hello"
        );
        assert!(store.exists(&key("uploads/a.txt")).await.unwrap());
        assert!(store.was_called(AssetOperation::Put));
    }

    #[tokio::test]
    async fn missing_keys_report_not_found() {
        let store = MockAssetStore::new();
        assert!(!store.exists(&key("uploads/none")).await.unwrap());
        assert_eq!(
            store.get(&key("uploads/none")).await.unwrap_err().code(),
            "asset_not_found"
        );
    }

    #[tokio::test]
    async fn delete_is_idempotent() {
        let store = MockAssetStore::new();
        store.put(put_request("uploads/a.txt", b"x")).await.unwrap();
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
    async fn builder_can_force_a_failure_without_touching_other_ops() {
        let store = MockAssetStore::builder()
            .fail_with(
                AssetOperation::Put,
                AssetError::Transient {
                    backend: BackendKind::S3,
                    detail: "503".into(),
                },
            )
            .build();

        let err = store
            .put(put_request("uploads/a.txt", b"x"))
            .await
            .unwrap_err();
        assert!(err.is_retryable());
        assert!(store.was_called(AssetOperation::Put));
        assert_eq!(store.object_count(), 0);

        // A different op still works.
        store
            .put_file(put_request("uploads/b.txt", b"y"))
            .await
            .unwrap();
        assert_eq!(store.object_count(), 1);
    }

    #[tokio::test]
    async fn capability_gate_precedes_the_call_record() {
        let store = MockAssetStore::builder()
            .backend(BackendKind::Proxy)
            .build();
        let err = store.delete(&key("uploads/a.txt")).await.unwrap_err();
        assert_eq!(err.code(), "asset_unsupported");
        assert!(matches!(
            err,
            AssetError::Unsupported {
                operation: AssetOperation::Delete,
                ..
            }
        ));
        assert!(!store.was_called(AssetOperation::Delete));
    }

    #[tokio::test]
    async fn set_visibility_is_gated_on_a_public_base_url_for_local() {
        let without = MockAssetStore::new();
        assert!(!without.capabilities().set_visibility);
        without
            .put(put_request("uploads/a.txt", b"x"))
            .await
            .unwrap();
        assert!(
            without
                .set_visibility(&key("uploads/a.txt"), Visibility::Public)
                .await
                .is_err()
        );

        let with = MockAssetStore::builder()
            .public_base_url("https://cdn.test")
            .build();
        with.put(put_request("uploads/a.txt", b"x")).await.unwrap();
        let meta = with
            .set_visibility(&key("uploads/a.txt"), Visibility::Public)
            .await
            .unwrap();
        assert!(meta.visibility.is_public());
        assert!(!meta.visibility_enforced);
        assert_eq!(
            with.visibility_of(&key("uploads/a.txt")),
            Some(Visibility::Public)
        );
        assert_eq!(
            with.public_url(&key("uploads/a.txt")).as_deref(),
            Some("https://cdn.test/uploads/a.txt")
        );
    }

    #[tokio::test]
    async fn max_object_bytes_is_enforced() {
        let store = MockAssetStore::builder().max_object_bytes(2).build();
        assert_eq!(
            store
                .put(put_request("uploads/a.txt", b"abc"))
                .await
                .unwrap_err()
                .code(),
            "asset_object_too_large"
        );
    }

    #[tokio::test]
    async fn list_paginates() {
        let store = MockAssetStore::new();
        for name in ["a", "b", "c"] {
            store
                .put(put_request(&format!("uploads/{name}.txt"), name.as_bytes()))
                .await
                .unwrap();
        }
        let page = store
            .list(ListQuery::new(AssetPrefix::parse("uploads/").unwrap()).with_limit(2))
            .await
            .unwrap();
        assert_eq!(page.items.len(), 2);
        assert!(page.truncated);
        let next = store
            .list(
                ListQuery::new(AssetPrefix::parse("uploads/").unwrap())
                    .with_limit(2)
                    .with_cursor(page.next_cursor.unwrap()),
            )
            .await
            .unwrap();
        assert_eq!(next.items.len(), 1);
        assert!(!next.truncated);
    }

    #[tokio::test]
    async fn presign_helpers_validate_ttl() {
        let store = MockAssetStore::new();
        let url = store
            .presign_get(&key("uploads/a.txt"), Duration::from_secs(60))
            .await
            .unwrap();
        assert!(url.emulated);
        assert_eq!(url.method, PresignMethod::Get);
        assert!(
            store
                .presign_get(&key("uploads/a.txt"), Duration::from_secs(0))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn is_send_and_shareable() {
        let store: crate::assets::SharedAssetStore = std::sync::Arc::new(MockAssetStore::new());
        tokio::spawn(async move {
            store.put(put_request("uploads/a.txt", b"x")).await.unwrap();
        })
        .await
        .unwrap();
    }
}
