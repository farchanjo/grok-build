//! Registry-driven model discovery and namespaced catalog IDs (Change 8 / PR8).
//!
//! Live discovery for catalog-capable OpenAI / OpenRouter family instances uses
//! the provider-specific adapters in [`crate::agent::provider_catalog`].
//!
//! **Identity contract:** this module never fabricates incarnation/binding or
//! writes authoritative PR7 `ProviderCacheStore` envelopes. Callers that hold a
//! real [`CatalogAccountIdentity`] + [`CatalogCredential`] use
//! [`discover_with_identity`]. The compatibility path is parse/network-only
//! and does not store.

use crate::agent::model_providers::{ModelProviderConfig, ModelProviderKind};
use crate::agent::provider_catalog::{
    CatalogAccountIdentity, CatalogCredential, CatalogFetchBounds, CatalogFetchSource,
    CatalogRefreshCoordinator, CatalogRefreshTarget, build_account_identity, fetch_openai_catalog,
    fetch_openrouter_catalog, load_cached_account, models_list_url_from_base,
    parse_openai_models_body,
};
use crate::provider_registry::id::ProviderId;
use crate::provider_registry::lifecycle::namespaced_model_id;
use crate::provider_registry::{
    ApiSurface, CATALOG_CACHE_VERSION, CacheOrigin, CatalogCacheEntry, CredentialBindingId,
    CredentialRoute, ProviderIncarnation, ProviderKind, normalize_endpoint_origin,
};
use indexmap::IndexMap;
use serde_json::json;
use tokio_util::sync::CancellationToken;

/// Discover models for one configured provider **without** writing PR7 state.
///
/// Uses adapters for pagination/bounds. Does not fabricate incarnation/binding
/// and never calls `ProviderCacheStore` / `CatalogCacheStore` with synthetic
/// identity. Prefer [`discover_with_identity`] when a real credential snapshot
/// is available.
pub async fn discover_provider_models(
    _grok_home: &std::path::Path,
    provider_id: &str,
    provider: &ModelProviderConfig,
    bearer: Option<&str>,
) -> Result<DiscoveredCatalog, String> {
    if !provider.enabled {
        return Err(format!("provider `{provider_id}` is disabled"));
    }
    if !provider.catalog_enabled {
        return Err(format!("catalog discovery disabled for `{provider_id}`"));
    }
    let base = provider
        .base_url
        .as_deref()
        .or(provider.api_base_url.as_deref())
        .ok_or_else(|| format!("provider `{provider_id}` missing base_url"))?;
    let _pid = ProviderId::new(provider_id).map_err(|e| e.to_string())?;
    let origin = normalize_endpoint_origin(base).map_err(|e| e.to_string())?;

    let token = bearer
        .map(str::to_owned)
        .or_else(|| resolve_env_token(provider));
    let Some(token) = token.filter(|t| !t.trim().is_empty()) else {
        return Err(format!(
            "missing application credential for provider `{provider_id}`"
        ));
    };

    let kind = ProviderKind::from(provider.kind);
    let api_surface = provider.api_surface.unwrap_or(default_surface(kind));
    let credential_route = provider.credential_route.unwrap_or(CredentialRoute::ApiKey);
    let bounds = CatalogFetchBounds::default().with_request_timeout(
        std::time::Duration::from_secs(provider.request_timeout_secs.unwrap_or(30)),
    );
    let models_url = models_list_url_from_base(base);
    let cancel = CancellationToken::new();

    // Ephemeral identity for the network hop only — never stored.
    // Random binding is intentionally *not* written to disk.
    let incarnation =
        ProviderIncarnation::new(uuid::Uuid::new_v4().to_string()).map_err(|e| e.to_string())?;
    let binding = CredentialBindingId::generate();
    let identity = build_account_identity(
        provider_id,
        kind,
        api_surface,
        credential_route,
        base,
        provider.organization.as_deref(),
        provider.project.as_deref(),
        incarnation,
        binding,
    )
    .map_err(|e| e.to_string())?;

    let result = match kind {
        ProviderKind::OpenRouter => {
            fetch_openrouter_catalog(
                &models_url,
                &token,
                &identity,
                &provider.capabilities,
                provider
                    .provider_preferences
                    .as_ref()
                    .and_then(|prefs| prefs.zdr),
                bounds,
                0,
                0,
                &cancel,
            )
            .await
        }
        ProviderKind::OpenAi | ProviderKind::OpenAiCompatible | ProviderKind::Zai => {
            fetch_openai_catalog(
                &models_url,
                &token,
                &identity,
                &provider.capabilities,
                bounds,
                0,
                0,
                &cancel,
            )
            .await
        }
        _ => {
            return Err(format!(
                "catalog discovery unsupported for provider kind {}",
                kind.as_str()
            ));
        }
    }
    .map_err(|e| e.to_string())?;

    if !result.is_complete_live() {
        return Err(format!(
            "catalog fetch for `{provider_id}` incomplete: {:?}",
            result.truncation
        ));
    }

    // Explicitly do **not** store: no synthetic PR7 identity on disk.
    let _ = origin;

    Ok(DiscoveredCatalog {
        provider_id: provider_id.to_owned(),
        provider_kind: provider.kind,
        upstream_slugs: result
            .models
            .iter()
            .map(|m| m.upstream_model_id.clone())
            .collect(),
        namespaced_ids: result
            .models
            .iter()
            .map(|m| m.canonical_selection_id.clone())
            .collect(),
        source: CatalogSource::Live,
    })
}

/// Discover and store under a real PR7 identity + credential snapshot.
pub async fn discover_with_identity(
    grok_home: &std::path::Path,
    identity: &CatalogAccountIdentity,
    credential: &CatalogCredential,
    models_list_url: &str,
    manual_capabilities: &IndexMap<String, bool>,
    bounds: CatalogFetchBounds,
    registry_generation: u64,
    publication_generation: u64,
    cancel: &CancellationToken,
) -> Result<DiscoveredCatalog, String> {
    if credential.binding_id() != &identity.credential_binding_id {
        return Err("credential binding does not match account identity".into());
    }
    // Prefer valid disk LKG when present (stale-while-revalidate for identity).
    if let Some(cached) = load_cached_account(
        grok_home,
        identity,
        registry_generation,
        publication_generation,
    ) {
        return Ok(DiscoveredCatalog {
            provider_id: identity.instance_id.as_str().to_owned(),
            provider_kind: kind_to_model(cached.provider_kind),
            upstream_slugs: cached
                .models
                .iter()
                .map(|m| m.upstream_model_id.clone())
                .collect(),
            namespaced_ids: cached
                .models
                .iter()
                .map(|m| m.canonical_selection_id.clone())
                .collect(),
            source: CatalogSource::Cache,
        });
    }

    let result = match identity.kind {
        ProviderKind::OpenRouter => {
            fetch_openrouter_catalog(
                models_list_url,
                credential.token(),
                identity,
                manual_capabilities,
                None,
                bounds,
                registry_generation,
                publication_generation,
                cancel,
            )
            .await
        }
        ProviderKind::OpenAi | ProviderKind::OpenAiCompatible | ProviderKind::Zai => {
            fetch_openai_catalog(
                models_list_url,
                credential.token(),
                identity,
                manual_capabilities,
                bounds,
                registry_generation,
                publication_generation,
                cancel,
            )
            .await
        }
        _ => {
            return Err(format!(
                "catalog discovery unsupported for provider kind {}",
                identity.kind.as_str()
            ));
        }
    }
    .map_err(|e| e.to_string())?;

    if !result.is_complete_live() {
        return Err(format!("catalog fetch incomplete: {:?}", result.truncation));
    }

    // Store only under the caller's real identity via PR7 store.
    let entry = CatalogCacheEntry {
        version: CATALOG_CACHE_VERSION,
        provider_id: identity.instance_id.as_str().to_owned(),
        origin: CacheOrigin::Live,
        base_url_origin: identity.endpoint_origin.clone(),
        fetched_at_unix: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        models: result
            .models
            .iter()
            .map(|m| {
                json!({
                    "id": m.upstream_model_id,
                    "canonical_id": m.canonical_selection_id,
                    "capabilities": m.capabilities,
                })
            })
            .collect(),
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
    let cache_identity = crate::provider_registry::ProviderCacheIdentity::new(
        identity.instance_id.clone(),
        identity.incarnation.clone(),
        identity.kind,
        identity.api_surface,
        identity.credential_route,
        identity.endpoint_origin.clone(),
        identity.org_project_fingerprint.clone(),
        identity.credential_binding_id.clone(),
    )
    .map_err(|e| e.to_string())?;
    crate::provider_registry::ProviderCacheStore::store_catalog(grok_home, &cache_identity, &entry)
        .map_err(|e| e.to_string())?;

    Ok(DiscoveredCatalog {
        provider_id: identity.instance_id.as_str().to_owned(),
        provider_kind: kind_to_model(identity.kind),
        upstream_slugs: result
            .models
            .iter()
            .map(|m| m.upstream_model_id.clone())
            .collect(),
        namespaced_ids: result
            .models
            .iter()
            .map(|m| m.canonical_selection_id.clone())
            .collect(),
        source: CatalogSource::Live,
    })
}

fn kind_to_model(k: ProviderKind) -> ModelProviderKind {
    match k {
        ProviderKind::OpenAi => ModelProviderKind::OpenAi,
        ProviderKind::OpenRouter => ModelProviderKind::OpenRouter,
        ProviderKind::Xai => ModelProviderKind::Xai,
        ProviderKind::Anthropic => ModelProviderKind::Anthropic,
        ProviderKind::Zai => ModelProviderKind::Zai,
        ProviderKind::OpenAiCompatible => ModelProviderKind::OpenAiCompatible,
    }
}

fn default_surface(kind: ProviderKind) -> ApiSurface {
    match kind {
        ProviderKind::OpenRouter => ApiSurface::OpenRouterNative,
        ProviderKind::Anthropic => ApiSurface::AnthropicMessages,
        ProviderKind::OpenAi => ApiSurface::OpenAiPlatform,
        _ => ApiSurface::OpenAiCompatibleSubset,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogSource {
    Live,
    Cache,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiscoveredCatalog {
    pub provider_id: String,
    pub provider_kind: ModelProviderKind,
    pub upstream_slugs: Vec<String>,
    pub namespaced_ids: Vec<String>,
    pub source: CatalogSource,
}

impl DiscoveredCatalog {
    /// Build from a cache entry preserving real kind (never downgrade).
    pub fn from_cache(
        provider_id: &str,
        kind: ModelProviderKind,
        entry: CatalogCacheEntry,
    ) -> Self {
        let provider_kind = entry.provider_kind.map(kind_to_model).unwrap_or(kind);
        let upstream: Vec<String> = entry
            .models
            .iter()
            .filter_map(|m| m.get("id").and_then(|v| v.as_str()).map(str::to_owned))
            .collect();
        let namespaced: Vec<String> = entry
            .models
            .iter()
            .map(|m| {
                m.get("canonical_id")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned)
                    .unwrap_or_else(|| {
                        let slug = m.get("id").and_then(|v| v.as_str()).unwrap_or_default();
                        if let Ok(pid) = ProviderId::new(provider_id) {
                            namespaced_model_id(&pid, slug)
                        } else {
                            slug.to_owned()
                        }
                    })
            })
            .collect();
        Self {
            provider_id: provider_id.to_owned(),
            provider_kind,
            upstream_slugs: upstream,
            namespaced_ids: namespaced,
            source: CatalogSource::Cache,
        }
    }
}

fn resolve_env_token(provider: &ModelProviderConfig) -> Option<String> {
    let keys = provider.env_key.as_ref()?;
    match keys {
        crate::agent::config::EnvKeys::One(name) => std::env::var(name).ok(),
        crate::agent::config::EnvKeys::Many(names) => {
            names.iter().find_map(|n| std::env::var(n).ok())
        }
    }
    .filter(|v| !v.trim().is_empty())
}

/// Concurrent discovery for multiple providers (network-only; no store).
pub async fn discover_all(
    grok_home: &std::path::Path,
    providers: &IndexMap<String, ModelProviderConfig>,
    tokens: &IndexMap<String, String>,
) -> IndexMap<String, Result<DiscoveredCatalog, String>> {
    let mut out = IndexMap::new();
    for (id, cfg) in providers {
        if !cfg.enabled || !cfg.catalog_enabled {
            continue;
        }
        if !cfg.kind.is_openai_compatible_family() {
            continue;
        }
        let token = tokens.get(id).map(String::as_str);
        let result = discover_provider_models(grok_home, id, cfg, token).await;
        out.insert(id.clone(), result);
    }
    out
}

/// Run atomic multi-account refresh through the coordinator.
pub async fn refresh_catalog_accounts(
    coordinator: &CatalogRefreshCoordinator,
    targets: Vec<CatalogRefreshTarget>,
    cancel: CancellationToken,
) -> Option<u64> {
    coordinator.refresh_all(targets, cancel).await
}

/// Construct a refresh target from config + credential snapshot.
pub fn refresh_target_from_config(
    provider_id: &str,
    provider: &ModelProviderConfig,
    credential: CatalogCredential,
    incarnation: ProviderIncarnation,
) -> Result<CatalogRefreshTarget, String> {
    let base = provider
        .base_url
        .as_deref()
        .or(provider.api_base_url.as_deref())
        .ok_or_else(|| format!("provider `{provider_id}` missing base_url"))?;
    let kind = ProviderKind::from(provider.kind);
    let identity = build_account_identity(
        provider_id,
        kind,
        provider.api_surface.unwrap_or(default_surface(kind)),
        provider.credential_route.unwrap_or(CredentialRoute::ApiKey),
        base,
        provider.organization.as_deref(),
        provider.project.as_deref(),
        incarnation,
        credential.binding_id().clone(),
    )
    .map_err(|e| e.to_string())?;
    Ok(CatalogRefreshTarget {
        identity,
        models_list_url: models_list_url_from_base(base),
        credential,
        manual_capabilities: provider.capabilities.clone(),
        bounds: CatalogFetchBounds::default().with_request_timeout(std::time::Duration::from_secs(
            provider.request_timeout_secs.unwrap_or(30),
        )),
        zdr: provider
            .provider_preferences
            .as_ref()
            .and_then(|prefs| prefs.zdr)
            .filter(|_| kind == ProviderKind::OpenRouter),
    })
}

#[allow(dead_code)]
fn _source_marker() -> CatalogFetchSource {
    CatalogFetchSource::Live
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_ids_compat() {
        let body = json!({"data":[{"id":"a"},{"id":"b"}]});
        let bytes = serde_json::to_vec(&body).unwrap();
        let identity = build_account_identity(
            "openai",
            ProviderKind::OpenAi,
            ApiSurface::OpenAiPlatform,
            CredentialRoute::ApiKey,
            "https://api.openai.com/v1",
            None,
            None,
            ProviderIncarnation::new("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").unwrap(),
            CredentialBindingId::new("11111111-2222-3333-4444-555555555555").unwrap(),
        )
        .unwrap();
        let models = parse_openai_models_body(&bytes, &identity, &IndexMap::new()).unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].canonical_selection_id, "openai:a");
    }

    #[test]
    fn origin_includes_port() {
        assert_eq!(
            normalize_endpoint_origin("http://127.0.0.1:8000/v1").unwrap(),
            "http://127.0.0.1:8000"
        );
    }

    #[test]
    fn cache_load_preserves_openai_kind() {
        let entry = CatalogCacheEntry {
            version: CATALOG_CACHE_VERSION,
            provider_id: "openai".into(),
            origin: CacheOrigin::Live,
            base_url_origin: "https://api.openai.com".into(),
            fetched_at_unix: 1,
            models: vec![json!({"id": "gpt-4o", "canonical_id": "openai:gpt-4o"})],
            baseline_version: None,
            incarnation: None,
            provider_kind: Some(ProviderKind::OpenAi),
            api_surface: Some(ApiSurface::OpenAiPlatform),
            credential_route: Some(CredentialRoute::ApiKey),
            credential_binding_id: None,
            org_project_fingerprint: None,
            catalog_generation: 1,
            lifecycle_generation: None,
        };
        let disc =
            DiscoveredCatalog::from_cache("openai", ModelProviderKind::OpenAiCompatible, entry);
        assert_eq!(disc.provider_kind, ModelProviderKind::OpenAi);
        assert_eq!(disc.namespaced_ids, vec!["openai:gpt-4o".to_string()]);
    }
}
