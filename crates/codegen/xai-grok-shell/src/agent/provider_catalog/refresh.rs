//! Multi-account catalog refresh with LKG retention and PR7 cache integration.
//!
//! Rules:
//! - Independent accounts may fetch concurrently.
//! - Partial-page / auth / malformed / cancel / bound exceedance never publish
//!   a truncated account; prior complete LKG is retained for that account.
//! - One account failure cannot hide or replace siblings.
//! - Failed first-time / identity-mismatch accounts are **omitted** (never
//!   publish empty fake Cache LKG).
//! - Stale registry (live ProviderService generation) / incarnation /
//!   credential-binding / publication generation completions are discarded.
//! - Credential replacement invalidates only that account.
//! - Bearer tokens are never stored in Debug-printable targets.

use super::bounds::CatalogFetchBounds;
use super::openai_adapter::fetch_openai_catalog;
use super::openrouter_adapter::fetch_openrouter_catalog;
use super::project::is_built_in_compatibility_instance;
use super::publish::{AccountRefreshOutcome, CatalogPublisher, merge_account_results};
use super::types::{
    CatalogAccountIdentity, CatalogAdapterError, CatalogFetchSource, CatalogTruncationReason,
    DiscoveredModel, InstanceCatalogResult,
};
use crate::provider_registry::{
    CATALOG_CACHE_VERSION, CacheOrigin, CatalogCacheEntry, ProviderCacheIdentity,
    ProviderCacheStore, ProviderKind, normalize_endpoint_origin,
};
use indexmap::IndexMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Secret-free credential handle bound to a known binding id.
///
/// The raw token is private and never appears in `Debug`. Callers pass it into
/// the adapter for the network hop only.
#[derive(Clone)]
pub struct CatalogCredential {
    binding_id: crate::provider_registry::CredentialBindingId,
    /// Opaque generation from the credential source (rotated on key replace).
    credential_generation: u64,
    token: String,
}

impl CatalogCredential {
    pub fn new(
        binding_id: crate::provider_registry::CredentialBindingId,
        credential_generation: u64,
        token: impl Into<String>,
    ) -> Result<Self, CatalogAdapterError> {
        let token = token.into();
        if token.trim().is_empty() {
            return Err(CatalogAdapterError::MissingCredential);
        }
        Ok(Self {
            binding_id,
            credential_generation,
            token,
        })
    }

    pub fn binding_id(&self) -> &crate::provider_registry::CredentialBindingId {
        &self.binding_id
    }

    pub fn credential_generation(&self) -> u64 {
        self.credential_generation
    }

    /// Borrow the token for an authenticated request only.
    pub fn token(&self) -> &str {
        &self.token
    }
}

impl fmt::Debug for CatalogCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CatalogCredential")
            .field("binding_id", &self.binding_id.as_str())
            .field("credential_generation", &self.credential_generation)
            .field("token", &"<redacted>")
            .finish()
    }
}

/// One catalog-capable account to refresh (secret-safe Debug).
#[derive(Clone)]
pub struct CatalogRefreshTarget {
    pub identity: CatalogAccountIdentity,
    pub models_list_url: String,
    pub credential: CatalogCredential,
    pub manual_capabilities: IndexMap<String, bool>,
    pub bounds: CatalogFetchBounds,
    /// OpenRouter `provider_preferences.zdr` for this instance only.
    /// `Some(true)` fetches `GET /models?zdr=true`. False/unset fetches the
    /// full list and still tags `supports_zdr` from `GET /endpoints/zdr`.
    pub zdr: Option<bool>,
}

impl fmt::Debug for CatalogRefreshTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CatalogRefreshTarget")
            .field("identity", &self.identity)
            .field("models_list_url", &"<redacted-url>")
            .field("credential", &self.credential)
            .field("manual_capabilities_len", &self.manual_capabilities.len())
            .field("bounds", &self.bounds)
            .field("zdr", &self.zdr)
            .finish()
    }
}

/// Live registry generation authority (typically `ProviderService::generation`).
pub type RegistryGenerationFn = Arc<dyn Fn() -> u64 + Send + Sync>;

/// Coordinator for concurrent multi-account catalog refresh + atomic publish.
pub struct CatalogRefreshCoordinator {
    grok_home: PathBuf,
    publisher: Arc<CatalogPublisher>,
    last_known_good: parking_lot::RwLock<IndexMap<String, InstanceCatalogResult>>,
    /// Live registry generation callback; re-read before store and publish.
    registry_generation: RegistryGenerationFn,
}

impl fmt::Debug for CatalogRefreshCoordinator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CatalogRefreshCoordinator")
            .field("grok_home", &self.grok_home)
            .field("publisher_gen", &self.publisher.current_generation())
            .finish()
    }
}

impl CatalogRefreshCoordinator {
    pub fn new(
        grok_home: impl Into<PathBuf>,
        publisher: Arc<CatalogPublisher>,
        registry_generation: RegistryGenerationFn,
    ) -> Self {
        Self {
            grok_home: grok_home.into(),
            publisher,
            last_known_good: parking_lot::RwLock::new(IndexMap::new()),
            registry_generation,
        }
    }

    /// Convenience: fixed registry generation (tests only).
    pub fn with_fixed_registry(
        grok_home: impl Into<PathBuf>,
        publisher: Arc<CatalogPublisher>,
        registry_generation: u64,
    ) -> Self {
        Self::new(grok_home, publisher, Arc::new(move || registry_generation))
    }

    pub fn publisher(&self) -> Arc<CatalogPublisher> {
        self.publisher.clone()
    }

    pub fn last_known_good(&self) -> IndexMap<String, InstanceCatalogResult> {
        self.last_known_good.read().clone()
    }

    pub fn load_lkg_from_caches(&self, identities: &[CatalogAccountIdentity]) {
        let mut lkg = self.last_known_good.write();
        let reg = (self.registry_generation)();
        for identity in identities {
            if let Some(result) = load_cached_account(&self.grok_home, identity, reg, 0) {
                if result.is_complete_publishable() {
                    lkg.insert(identity.instance_id.as_str().to_owned(), result);
                }
            }
        }
    }

    /// Refresh all targets concurrently and publish one atomic generation.
    ///
    /// Pre-cancelled refresh does not bump publication generation.
    pub async fn refresh_all(
        &self,
        targets: Vec<CatalogRefreshTarget>,
        cancel: CancellationToken,
    ) -> Option<u64> {
        if cancel.is_cancelled() {
            return None;
        }
        let registry_at_start = (self.registry_generation)();
        let publication_generation = self.publisher.begin_generation();
        let prior = self.last_known_good.read().clone();
        let registry_fn = self.registry_generation.clone();
        let publisher_gen_check = self.publisher.clone();

        let mut handles = Vec::with_capacity(targets.len());
        for target in targets {
            // Credential binding must match identity.
            if target.credential.binding_id() != &target.identity.credential_binding_id {
                continue;
            }
            let home = self.grok_home.clone();
            let cancel = cancel.clone();
            let prior_for_account = prior.get(target.identity.instance_id.as_str()).cloned();
            let registry_fn = registry_fn.clone();
            let publisher_gen_check = publisher_gen_check.clone();
            handles.push(tokio::spawn(async move {
                refresh_one_account(
                    &home,
                    target,
                    registry_at_start,
                    publication_generation,
                    prior_for_account,
                    cancel,
                    registry_fn,
                    publisher_gen_check,
                )
                .await
            }));
        }

        let mut updates: IndexMap<String, AccountRefreshOutcome> = IndexMap::new();
        for handle in handles {
            match handle.await {
                Ok((id, outcome)) => {
                    updates.insert(id, outcome);
                }
                Err(_) => {}
            }
        }

        if self.publisher.current_generation() != publication_generation {
            return None;
        }
        if cancel.is_cancelled() {
            return None;
        }
        // Live registry generation must still match start.
        let registry_now = (self.registry_generation)();
        if registry_now != registry_at_start {
            return None;
        }

        let mut merged = merge_account_results(&prior, updates);
        // Only complete rows with matching live registry generation.
        merged.retain(|_, r| {
            r.is_complete_publishable() && r.registry_generation == registry_at_start
        });
        // Retag publication generation so retained LKG can join this snapshot.
        for result in merged.values_mut() {
            result.publication_generation = publication_generation;
        }

        let published = self.publisher.publish_if_current(
            publication_generation,
            registry_at_start,
            merged.clone(),
        );
        if published {
            *self.last_known_good.write() = merged;
            Some(publication_generation)
        } else {
            None
        }
    }

    pub fn publish_lkg(&self) -> u64 {
        let lkg = self.last_known_good.read().clone();
        let reg = (self.registry_generation)();
        self.publisher.publish_complete(reg, lkg)
    }
}

#[allow(clippy::too_many_arguments)]
async fn refresh_one_account(
    grok_home: &Path,
    target: CatalogRefreshTarget,
    registry_at_start: u64,
    publication_generation: u64,
    prior: Option<InstanceCatalogResult>,
    cancel: CancellationToken,
    registry_fn: RegistryGenerationFn,
    publisher: Arc<CatalogPublisher>,
) -> (String, AccountRefreshOutcome) {
    let id = target.identity.instance_id.as_str().to_owned();

    // Matching complete prior only (full PR7 identity including fingerprint).
    let prior = prior
        .filter(|p| account_identity_matches(p, &target.identity) && p.is_complete_publishable());

    if cancel.is_cancelled() {
        return (
            id,
            prior
                .map(AccountRefreshOutcome::RetainLkg)
                .unwrap_or(AccountRefreshOutcome::Omit),
        );
    }

    let live = match target.identity.kind {
        ProviderKind::OpenAi | ProviderKind::OpenAiCompatible | ProviderKind::Zai => {
            fetch_openai_catalog(
                &target.models_list_url,
                target.credential.token(),
                &target.identity,
                &target.manual_capabilities,
                target.bounds,
                registry_at_start,
                publication_generation,
                &cancel,
            )
            .await
        }
        ProviderKind::OpenRouter => {
            fetch_openrouter_catalog(
                &target.models_list_url,
                target.credential.token(),
                &target.identity,
                &target.manual_capabilities,
                target.zdr,
                target.bounds,
                registry_at_start,
                publication_generation,
                &cancel,
            )
            .await
        }
        _ => {
            return (
                id,
                prior
                    .map(AccountRefreshOutcome::RetainLkg)
                    .unwrap_or(AccountRefreshOutcome::Omit),
            );
        }
    };

    match live {
        Ok(mut result) if result.is_complete_live() => {
            // Revalidate live registry + publication generation before store.
            if (registry_fn)() != registry_at_start
                || publisher.current_generation() != publication_generation
                || cancel.is_cancelled()
            {
                return (
                    id,
                    prior
                        .map(AccountRefreshOutcome::RetainLkg)
                        .unwrap_or(AccountRefreshOutcome::Omit),
                );
            }
            // Revalidate credential binding still matches identity.
            if target.credential.binding_id() != &target.identity.credential_binding_id {
                return (
                    id,
                    prior
                        .map(AccountRefreshOutcome::RetainLkg)
                        .unwrap_or(AccountRefreshOutcome::Omit),
                );
            }
            if let Ok(catalog_generation) =
                store_account_catalog(grok_home, &target.identity, &result)
            {
                result.catalog_generation = catalog_generation;
            }
            // Final gen check after store: if stale, still don't publish this
            // as Complete under a superseded generation.
            if publisher.current_generation() != publication_generation
                || (registry_fn)() != registry_at_start
            {
                return (
                    id,
                    prior
                        .map(AccountRefreshOutcome::RetainLkg)
                        .unwrap_or(AccountRefreshOutcome::Omit),
                );
            }
            result.registry_generation = registry_at_start;
            result.publication_generation = publication_generation;
            (id, AccountRefreshOutcome::Complete(result))
        }
        Ok(_) | Err(_) => {
            // Prefer matching complete disk LKG when in-memory prior missing.
            let lkg = prior.or_else(|| {
                load_cached_account(
                    grok_home,
                    &target.identity,
                    registry_at_start,
                    publication_generation,
                )
                .filter(|r| r.is_complete_publishable())
            });
            (
                id,
                lkg.map(AccountRefreshOutcome::RetainLkg)
                    .unwrap_or(AccountRefreshOutcome::Omit),
            )
        }
    }
}

fn account_identity_matches(
    prior: &InstanceCatalogResult,
    identity: &CatalogAccountIdentity,
) -> bool {
    if prior.provider_instance_id != identity.instance_id.as_str() {
        return false;
    }
    if prior.provider_kind != identity.kind {
        return false;
    }
    if prior.api_surface != identity.api_surface {
        return false;
    }
    if prior.credential_route != identity.credential_route {
        return false;
    }
    if prior.endpoint_origin != identity.endpoint_origin {
        return false;
    }
    if prior.org_project_fingerprint != identity.org_project_fingerprint {
        return false;
    }
    if let Some(inc) = &prior.incarnation
        && inc != &identity.incarnation
    {
        return false;
    }
    if let Some(binding) = &prior.credential_binding_id
        && binding != &identity.credential_binding_id
    {
        return false;
    }
    true
}

fn store_account_catalog(
    grok_home: &Path,
    identity: &CatalogAccountIdentity,
    result: &InstanceCatalogResult,
) -> Result<u64, CatalogAdapterError> {
    let cache_identity = ProviderCacheIdentity::new(
        identity.instance_id.clone(),
        identity.incarnation.clone(),
        identity.kind,
        identity.api_surface,
        identity.credential_route,
        identity.endpoint_origin.clone(),
        identity.org_project_fingerprint.clone(),
        identity.credential_binding_id.clone(),
    )
    .map_err(|e| CatalogAdapterError::InvalidOrigin {
        detail: e.to_string(),
    })?;

    let models: Vec<serde_json::Value> = result
        .models
        .iter()
        .map(|m| {
            serde_json::json!({
                "id": m.upstream_model_id,
                "canonical_id": m.canonical_selection_id,
                "display_name": m.display_name,
                "description": m.description,
                "context_window": m.context_window,
                "max_completion_tokens": m.max_completion_tokens,
                "max_output_ceiling": m.max_output_ceiling,
                "capabilities": m.capabilities,
            })
        })
        .collect();

    let entry = CatalogCacheEntry {
        version: CATALOG_CACHE_VERSION,
        provider_id: identity.instance_id.as_str().to_owned(),
        origin: CacheOrigin::Live,
        base_url_origin: identity.endpoint_origin.clone(),
        fetched_at_unix: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        models,
        baseline_version: None,
        incarnation: Some(identity.incarnation.clone()),
        provider_kind: Some(identity.kind),
        api_surface: Some(identity.api_surface),
        credential_route: Some(identity.credential_route),
        credential_binding_id: Some(identity.credential_binding_id.clone()),
        org_project_fingerprint: if identity.org_project_fingerprint.is_empty() {
            None
        } else {
            Some(identity.org_project_fingerprint.clone())
        },
        catalog_generation: 0,
        lifecycle_generation: None,
    };

    ProviderCacheStore::store_catalog(grok_home, &cache_identity, &entry).map_err(|e| {
        CatalogAdapterError::Transport {
            detail: CatalogAdapterError::sanitize_detail(&e.to_string()),
        }
    })?;

    let catalog_gen = ProviderCacheStore::load_state(grok_home, &identity.instance_id)
        .ok()
        .flatten()
        .map(|s| s.catalog_generation)
        .unwrap_or(1);
    Ok(catalog_gen)
}

/// Load a complete cached account catalog when identity validates.
pub fn load_cached_account(
    grok_home: &Path,
    identity: &CatalogAccountIdentity,
    registry_generation: u64,
    publication_generation: u64,
) -> Option<InstanceCatalogResult> {
    let cache_identity = ProviderCacheIdentity::new(
        identity.instance_id.clone(),
        identity.incarnation.clone(),
        identity.kind,
        identity.api_surface,
        identity.credential_route,
        identity.endpoint_origin.clone(),
        identity.org_project_fingerprint.clone(),
        identity.credential_binding_id.clone(),
    )
    .ok()?;
    let entry = ProviderCacheStore::load_catalog(grok_home, &cache_identity)
        .ok()
        .flatten()?;
    let kind = entry.provider_kind.unwrap_or(identity.kind);
    let surface = entry.api_surface.unwrap_or(identity.api_surface);
    let route = entry.credential_route.unwrap_or(identity.credential_route);
    let models = models_from_cache_entry(&entry, identity, kind, surface, route);
    Some(InstanceCatalogResult {
        provider_instance_id: identity.instance_id.as_str().to_owned(),
        provider_kind: kind,
        api_surface: surface,
        credential_route: route,
        endpoint_origin: entry.base_url_origin,
        org_project_fingerprint: entry
            .org_project_fingerprint
            .clone()
            .unwrap_or_else(|| identity.org_project_fingerprint.clone()),
        incarnation: entry
            .incarnation
            .or_else(|| Some(identity.incarnation.clone())),
        credential_binding_id: entry
            .credential_binding_id
            .or_else(|| Some(identity.credential_binding_id.clone())),
        registry_generation,
        catalog_generation: entry.catalog_generation,
        publication_generation,
        source: CatalogFetchSource::Cache,
        truncation: CatalogTruncationReason::Complete,
        models,
        diagnostic: None,
    })
}

fn models_from_cache_entry(
    entry: &CatalogCacheEntry,
    identity: &CatalogAccountIdentity,
    kind: ProviderKind,
    surface: crate::provider_registry::ApiSurface,
    route: crate::provider_registry::CredentialRoute,
) -> Vec<DiscoveredModel> {
    use super::project::canonical_selection_id;
    entry
        .models
        .iter()
        .filter_map(|m| {
            let upstream = m.get("id")?.as_str()?;
            let canonical = m
                .get("canonical_id")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
                .unwrap_or_else(|| canonical_selection_id(identity, upstream));
            let capabilities: super::types::ProjectedCapabilities = m
                .get("capabilities")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            Some(DiscoveredModel {
                canonical_selection_id: canonical,
                upstream_model_id: upstream.to_owned(),
                display_name: m
                    .get("display_name")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned),
                description: m
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned),
                context_window: m.get("context_window").and_then(|v| v.as_u64()),
                max_completion_tokens: m
                    .get("max_completion_tokens")
                    .and_then(|v| v.as_u64())
                    .and_then(|n| u32::try_from(n).ok()),
                max_output_ceiling: m
                    .get("max_output_ceiling")
                    .and_then(|v| v.as_u64())
                    .and_then(|n| u32::try_from(n).ok())
                    .or(capabilities.max_output_ceiling),
                capabilities,
                provider_instance_id: identity.instance_id.as_str().to_owned(),
                provider_kind: kind,
                api_surface: surface,
                credential_route: route,
                endpoint_origin: entry.base_url_origin.clone(),
            })
        })
        .collect()
}

/// Build a [`CatalogAccountIdentity`] from service-facing fields.
pub fn build_account_identity(
    instance_id: &str,
    kind: ProviderKind,
    api_surface: crate::provider_registry::ApiSurface,
    credential_route: crate::provider_registry::CredentialRoute,
    base_url: &str,
    org: Option<&str>,
    project: Option<&str>,
    incarnation: crate::provider_registry::ProviderIncarnation,
    binding: crate::provider_registry::CredentialBindingId,
) -> Result<CatalogAccountIdentity, CatalogAdapterError> {
    let pid = crate::provider_registry::ProviderId::new(instance_id).map_err(|e| {
        CatalogAdapterError::InvalidOrigin {
            detail: e.to_string(),
        }
    })?;
    let origin =
        normalize_endpoint_origin(base_url).map_err(|e| CatalogAdapterError::InvalidOrigin {
            detail: e.to_string(),
        })?;
    let fingerprint = crate::provider_registry::org_project_fingerprint(org, project);
    Ok(CatalogAccountIdentity {
        instance_id: pid,
        kind,
        api_surface,
        credential_route,
        endpoint_origin: origin,
        org_project_fingerprint: fingerprint,
        incarnation,
        credential_binding_id: binding,
        is_built_in_compatibility: is_built_in_compatibility_instance(instance_id, kind),
    })
}

/// Models list URL for an instance base URL (OpenAI-family: `{base}/models`).
pub fn models_list_url_from_base(base_url: &str) -> String {
    format!("{}/models", base_url.trim_end_matches('/'))
}
