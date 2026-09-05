//! Remote vector-mirror resolution and process-level registration.
//!
//! Resolves a named `[vector_stores.<id>]` entry plus its bearer token into
//! a connected [`MirrorHandle`] bound to one collection, and keeps resolved
//! handles in a process-level registry so `/context` inspection can report
//! live mirror state without reaching into session internals.
//!
//! Token ordering mirrors `resolve_application_credential`: the
//! `MILVUS_TOKEN_FOR_<ID>` environment variable wins, then one vault read
//! of `milvus::<store-id>::token` from `auth.json`. The token is passed
//! only to the backend client and is never logged.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use xai_grok_config_types::VectorStoresConfig;
use xai_grok_memory::mirror::{MirrorHandle, mirror_timeout};
use xai_grok_memory::workspace_identity::workspace_identity_hash16;

use super::memory::MemoryBackendParams;

/// Process-level registry of resolved mirrors keyed by collection name.
static RESOLVED_MIRRORS: OnceLock<parking_lot::RwLock<HashMap<String, Arc<MirrorHandle>>>> =
    OnceLock::new();

fn registry() -> &'static parking_lot::RwLock<HashMap<String, Arc<MirrorHandle>>> {
    RESOLVED_MIRRORS.get_or_init(|| parking_lot::RwLock::new(HashMap::new()))
}

/// Register a resolved mirror under its collection name (idempotent per
/// collection: the latest resolution wins, which is the same handle in
/// practice).
pub fn register_mirror(handle: Arc<MirrorHandle>) {
    registry()
        .write()
        .insert(handle.collection().to_owned(), handle);
}

/// Look up a previously registered mirror by collection name.
#[must_use]
pub fn registered_mirror(collection: &str) -> Option<Arc<MirrorHandle>> {
    registry().read().get(collection).cloned()
}

/// Snapshot of every registered mirror (for `/context` reporting).
#[must_use]
pub fn registered_mirrors() -> Vec<Arc<MirrorHandle>> {
    registry().read().values().cloned().collect()
}

/// `MILVUS_TOKEN_FOR_<ID>` environment variable name for a store id
/// (non-alphanumeric characters become underscores).
#[must_use]
pub fn milvus_token_env_name(store_id: &str) -> String {
    let suffix: String = store_id
        .to_ascii_uppercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("MILVUS_TOKEN_FOR_{suffix}")
}

/// Resolve the Milvus bearer token for a store: env override first, then a
/// single vault read (`milvus::<store-id>::token`). Never logs the value.
#[must_use]
pub fn resolve_milvus_token(home: &Path, store_id: &str) -> Option<String> {
    let env_name = milvus_token_env_name(store_id);
    if let Ok(value) = std::env::var(&env_name)
        && !value.trim().is_empty()
    {
        return Some(value);
    }
    let scope = format!("milvus::{store_id}::token");
    crate::provider_registry::secrets::read_provider_secret(home, &scope)
        .ok()
        .flatten()
        .filter(|value| !value.trim().is_empty())
}

/// Resolve the mirror for one collection from the effective config.
///
/// Reads `[vector_stores.<store_id>]` from `config`, validates it, resolves
/// the token, connects the backend, and registers the resulting handle
/// under `collection`. Bounded by the store's own timeout (connect
/// included); never blocks past it. Returns `None` when the store is
/// misconfigured, the token is missing, or the server is unreachable —
/// SQLite keeps serving in every failure case.
pub async fn resolve_mirror_for_collection(
    config: &toml::Value,
    home: &Path,
    store_id: &str,
    collection: String,
) -> Option<Arc<MirrorHandle>> {
    if let Some(existing) = registered_mirror(&collection) {
        return Some(existing);
    }
    let store = selected_store(config, store_id)?;
    let timeout = mirror_timeout(store.timeout_secs);
    let token = resolve_milvus_token(home, store_id);
    let connect =
        xai_grok_memory::mirror_milvus::connect_store(store.uri.trim(), token.as_deref(), timeout);
    match tokio::time::timeout(timeout, connect).await {
        Ok(Ok(backend)) => {
            let handle = Arc::new(MirrorHandle::new(backend, collection));
            register_mirror(handle.clone());
            Some(handle)
        }
        Ok(Err(e)) => {
            // Single-line, secret-free diagnostic; detail is redacted.
            tracing::warn!(
                target: xai_grok_telemetry::memory_log::TARGET,
                store_id,
                error = %e,
                "vector mirror connect failed; SQLite keeps serving"
            );
            None
        }
        Err(_) => {
            tracing::warn!(
                target: xai_grok_telemetry::memory_log::TARGET,
                store_id,
                "vector mirror connect timed out; SQLite keeps serving"
            );
            None
        }
    }
}

/// Parse and validate the selected `[vector_stores.<store_id>]` entry.
fn selected_store(
    config: &toml::Value,
    store_id: &str,
) -> Option<xai_grok_config_types::VectorStoreConfig> {
    let stores: VectorStoresConfig = config
        .get("vector_stores")
        .and_then(|value| value.clone().try_into().ok())
        .unwrap_or_default();
    let Some(store) = stores.get(store_id) else {
        tracing::warn!(
            target: xai_grok_telemetry::memory_log::TARGET,
            store_id,
            "vector_store selection references an unknown [vector_stores] entry"
        );
        return None;
    };
    if let Err(e) = store.validate() {
        tracing::warn!(
            target: xai_grok_telemetry::memory_log::TARGET,
            store_id,
            error = %e,
            "vector store config invalid; mirror disabled"
        );
        return None;
    }
    Some(store.clone())
}

/// Memory collection name for a workspace (plan scheme:
/// `grok_mem_{identity-hash16}`).
#[must_use]
pub fn memory_collection_for_cwd(cwd: &Path) -> String {
    xai_grok_memory::memory_collection_name(&workspace_identity_hash16(cwd))
}

/// Prime collection names for a workspace, keyed by collection kind.
#[must_use]
pub fn prime_collections_for_cwd(cwd: &Path) -> (String, String) {
    let hash16 = workspace_identity_hash16(cwd);
    (
        xai_grok_memory::prime_collection_name(&hash16, "skills"),
        xai_grok_memory::prime_collection_name(&hash16, "callable_agents"),
    )
}

/// Resolve the memory mirror from the effective config and produce the
/// params field value. `store_id` is the `[memory] vector_store` selection.
pub async fn resolve_memory_mirror(
    config: &toml::Value,
    home: &Path,
    cwd: &Path,
    store_id: &str,
) -> Option<Arc<MirrorHandle>> {
    let collection = memory_collection_for_cwd(cwd);
    resolve_mirror_for_collection(config, home, store_id, collection).await
}

/// Resolve the prime mirror pair from the effective config.
/// `store_id` is the `[prime] vector_store` selection; the skills and
/// callable_agents collections each get their own remote collection.
pub async fn resolve_prime_mirrors(
    config: &toml::Value,
    home: &Path,
    cwd: &Path,
    store_id: &str,
) -> Option<crate::session::prime::index::PrimeMirrorPair> {
    let (skills, agents) = prime_collections_for_cwd(cwd);
    let skills = resolve_mirror_for_collection(config, home, store_id, skills).await?;
    let agents = resolve_mirror_for_collection(config, home, store_id, agents).await?;
    Some(crate::session::prime::index::PrimeMirrorPair {
        skills,
        callable_agents: agents,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_name_maps_non_alnum_to_underscores() {
        assert_eq!(
            milvus_token_env_name("local-milvus"),
            "MILVUS_TOKEN_FOR_LOCAL_MILVUS"
        );
        assert_eq!(
            milvus_token_env_name("prod.cluster/1"),
            "MILVUS_TOKEN_FOR_PROD_CLUSTER_1"
        );
    }

    #[test]
    fn collection_names_use_workspace_identity() {
        let cwd = std::env::temp_dir().join("grok-vector-mirror-names");
        std::fs::create_dir_all(&cwd).unwrap();
        let memory = memory_collection_for_cwd(&cwd);
        assert!(memory.starts_with("grok_mem_"), "{memory}");
        assert_eq!(memory.len(), "grok_mem_".len() + 16);
        let (skills, agents) = prime_collections_for_cwd(&cwd);
        assert!(
            skills.starts_with("grok_prime_") && skills.ends_with("_skills"),
            "{skills}"
        );
        assert!(
            agents.starts_with("grok_prime_") && agents.ends_with("_callable_agents"),
            "{agents}"
        );
    }

    #[test]
    fn unknown_store_selection_fails_closed() {
        let config: toml::Value = toml::from_str("[memory]\nvector_store = \"missing\"").unwrap();
        let store = selected_store(&config, "missing");
        assert!(store.is_none());
    }
}
