//! Dynamic provider registry: IDs, refs, metadata, TOML lifecycle, and caches.
//!
//! Built-in providers (xAI, OpenAI, OpenRouter) and arbitrary OpenAI-compatible
//! instances share one registry surface while keeping credentials and caches
//! isolated per provider instance.

pub mod cache;
pub mod gate;
#[cfg(test)]
mod gate_d_acceptance;
pub mod id;
pub mod instance;
pub mod lifecycle;
pub mod lifecycle_state;
pub mod management;
pub mod notify;
pub mod references;
/// Exact retrieval route/credential resolution (PR16).
pub mod retrieval_runtime;
pub mod route_guard;
pub mod runtime_cache;
pub mod secrets;
pub mod service;
pub mod toml_edit;

pub use cache::{
    CAPABILITY_CACHE_VERSION, CATALOG_CACHE_VERSION, CacheOrigin, CacheValidationError,
    CapabilityCacheEntry, CapabilityCacheStore, CatalogCacheEntry, CatalogCacheStore,
    CredentialBindingId, FingerprintError, LegacyImportMarker, OriginNormalizeError,
    ProviderCacheIdentity, ProviderCacheState, ProviderCacheStore, ProviderCacheTxnFault,
    STATE_CACHE_VERSION, normalize_endpoint_origin, org_project_fingerprint,
    remove_all_provider_caches, validate_org_project_fingerprint,
};
pub use gate::{
    MULTI_ACCOUNT_ROLLOUT_DEFAULT_ENABLED, MULTI_ACCOUNT_ROLLOUT_ENV, multi_account_rollout_enabled,
};

#[cfg(test)]
pub(crate) use gate::{multi_account_rollout_env_lock, with_multi_account_rollout_env};
pub use id::{
    BuiltInProviderId, ProviderId, ProviderIdError, ProviderRef, validate_provider_id_str,
};
pub use instance::{
    ApiSurface, CredentialRoute, IncarnationError, MAX_INCARNATION_LEN, ProviderIncarnation,
    ProviderInstanceDescriptor, ProviderKind, ProviderRouteDescriptor,
};
pub use lifecycle::{
    ProviderLifecycleError, ProviderMetadata, ProviderRegistrySnapshot, namespaced_model_id,
    parse_namespaced_model_id, resolve_legacy_model_alias,
};
pub use lifecycle_state::{
    InstanceLifecycleRecord, LifecycleStateError, ProviderLifecycleState, ProviderTombstone,
    load_lifecycle_state, provenance_matches_lifecycle, stable_builtin_incarnation,
    store_lifecycle_state,
};
pub use management::{
    ProviderManagementService,
    dto::{
        CapabilityStatusSnapshot, CatalogStatusSnapshot, CredentialPresence, CredentialSlotUpdate,
        ForceRemoveClearOptions, ImpactGroup, ImpactGroupKind, ImpactReference, ProviderAddRequest,
        ProviderCloneRequest, ProviderConflictInfo, ProviderCreditsSnapshot, ProviderDetailDto,
        ProviderEditorPage, ProviderForceRemoveRequest, ProviderListRow, ProviderListSnapshot,
        ProviderMutationResult, ProviderSavePatch, ProviderSaveRequest, ProviderStatusSnapshot,
        ReferenceImpactSnapshot, RegistryGeneration, SecretFieldUpdate,
    },
};
pub use retrieval_runtime::{
    ResolvedRetrievalRuntime, RetrievalResolveCounters, RetrievalResolveOptions,
    RetrievalRuntimeError, embed_with_runtime, embedding_request_from_config,
    rerank_request_from_config, rerank_with_runtime, resolve_embedding_runtime,
    resolve_reranker_runtime,
};
pub use route_guard::{
    RouteGuardError, RouteGuardErrorCategory, RouteGuardRequest, assert_not_sibling_borrow,
    assert_route_usable,
};
pub use secrets::{
    ProviderCredentialKind, ProviderOAuthBinding, ProviderSecretScope, admin_key_scope,
    application_key_scope, clear_provider_secret, is_allowed_oauth_scope, oauth_scope_string,
    parse_secret_scope, read_provider_secret, store_provider_secret,
};
pub use service::{ProviderService, ProviderServiceError};
pub use toml_edit::{
    OpenRouterPrefsPatch, ProviderTomlPatch, apply_openrouter_preferences, apply_provider_patch,
    apply_provider_patch_with_openrouter, disable_provider, enable_provider, remove_provider,
    upsert_provider,
};
