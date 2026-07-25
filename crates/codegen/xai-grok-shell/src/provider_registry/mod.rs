//! Dynamic provider registry: IDs, refs, metadata, TOML lifecycle, and caches.
//!
//! Built-in providers (xAI, OpenAI, OpenRouter) and arbitrary OpenAI-compatible
//! instances share one registry surface while keeping credentials and caches
//! isolated per provider instance.

pub mod cache;
pub mod id;
pub mod lifecycle;
pub mod secrets;
pub mod toml_edit;

pub use cache::{
    CAPABILITY_CACHE_VERSION, CATALOG_CACHE_VERSION, CacheOrigin, CacheValidationError,
    CapabilityCacheEntry, CapabilityCacheStore, CatalogCacheEntry, CatalogCacheStore,
    remove_all_provider_caches,
};
pub use id::{
    BuiltInProviderId, ProviderId, ProviderIdError, ProviderRef, validate_provider_id_str,
};
pub use lifecycle::{
    ProviderLifecycleError, ProviderMetadata, ProviderRegistrySnapshot, namespaced_model_id,
    parse_namespaced_model_id, resolve_legacy_model_alias,
};
pub use secrets::{
    ProviderCredentialKind, ProviderSecretScope, admin_key_scope, application_key_scope,
    clear_provider_secret, parse_secret_scope, read_provider_secret, store_provider_secret,
};
pub use toml_edit::{
    ProviderTomlPatch, apply_provider_patch, disable_provider, enable_provider, remove_provider,
    upsert_provider,
};
