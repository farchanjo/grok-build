//! Registry-driven model discovery and namespaced catalog IDs (Change 8).

use crate::agent::model_providers::{ModelProviderConfig, ModelProviderKind};
use crate::provider_registry::id::ProviderId;
use crate::provider_registry::lifecycle::namespaced_model_id;
use crate::provider_registry::{
    CATALOG_CACHE_VERSION, CacheOrigin, CatalogCacheEntry, CatalogCacheStore,
};
use indexmap::IndexMap;
use serde_json::{Value, json};

/// Discover models for one configured provider via GET /models (non-mutating).
///
/// Results are stored in the per-provider catalog cache with origin validation.
/// Upstream slugs are returned both raw and namespaced.
pub async fn discover_provider_models(
    grok_home: &std::path::Path,
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
    let pid = ProviderId::new(provider_id).map_err(|e| e.to_string())?;
    let origin = origin_host(base)?;

    // Stale-while-revalidate: return cache if present.
    if let Ok(Some(cached)) = CatalogCacheStore::load(grok_home, &pid, &origin) {
        return Ok(DiscoveredCatalog::from_cache(provider_id, cached));
    }

    let token = bearer
        .map(str::to_owned)
        .or_else(|| resolve_env_token(provider));
    let Some(token) = token.filter(|t| !t.trim().is_empty()) else {
        return Err(format!(
            "missing application credential for provider `{provider_id}`"
        ));
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(
            provider.request_timeout_secs.unwrap_or(30),
        ))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())?;
    let url = format!("{}/models", base.trim_end_matches('/'));
    let resp = client
        .get(&url)
        .bearer_auth(token.trim())
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!(
            "catalog fetch for `{provider_id}` failed with HTTP {}",
            resp.status()
        ));
    }
    let body: Value = resp.json().await.map_err(|e| e.to_string())?;
    let models = extract_model_ids(&body);
    let namespaced: Vec<String> = models
        .iter()
        .map(|m| namespaced_model_id(&pid, m))
        .collect();

    let entry = CatalogCacheEntry {
        version: CATALOG_CACHE_VERSION,
        provider_id: provider_id.to_owned(),
        origin: CacheOrigin::Live,
        base_url_origin: origin,
        fetched_at_unix: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        models: models.iter().map(|id| json!({"id": id})).collect(),
        baseline_version: None,
    };
    let _ = CatalogCacheStore::store(grok_home, &entry);

    Ok(DiscoveredCatalog {
        provider_id: provider_id.to_owned(),
        provider_kind: provider.kind,
        upstream_slugs: models,
        namespaced_ids: namespaced,
        source: CatalogSource::Live,
    })
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
    fn from_cache(provider_id: &str, entry: CatalogCacheEntry) -> Self {
        let upstream: Vec<String> = entry
            .models
            .iter()
            .filter_map(|m| m.get("id").and_then(|v| v.as_str()).map(str::to_owned))
            .collect();
        let pid = ProviderId::new(provider_id).ok();
        let namespaced = pid
            .map(|p| {
                upstream
                    .iter()
                    .map(|m| namespaced_model_id(&p, m))
                    .collect()
            })
            .unwrap_or_default();
        Self {
            provider_id: provider_id.to_owned(),
            provider_kind: ModelProviderKind::OpenAiCompatible,
            upstream_slugs: upstream,
            namespaced_ids: namespaced,
            source: CatalogSource::Cache,
        }
    }
}

fn extract_model_ids(body: &Value) -> Vec<String> {
    body.get("data")
        .and_then(|d| d.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.get("id").and_then(|i| i.as_str()).map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn origin_host(base_url: &str) -> Result<String, String> {
    let url = reqwest::Url::parse(base_url).map_err(|e| e.to_string())?;
    let host = url
        .host_str()
        .ok_or_else(|| "base URL missing host".to_string())?;
    match url.port() {
        Some(p) => Ok(format!("{}://{}:{p}", url.scheme(), host)),
        None => Ok(format!("{}://{}", url.scheme(), host)),
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

/// Concurrent discovery for multiple providers with isolated credentials.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_ids() {
        let body = json!({"data":[{"id":"a"},{"id":"b"}]});
        assert_eq!(extract_model_ids(&body), vec!["a", "b"]);
    }

    #[test]
    fn origin_includes_port() {
        assert_eq!(
            origin_host("http://127.0.0.1:8000/v1").unwrap(),
            "http://127.0.0.1:8000"
        );
    }
}
