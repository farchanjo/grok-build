//! Provider-specific catalog adapters and atomic multi-account publication.
//!
//! OpenAI and OpenRouter each have their own documented `/models` shapes and
//! pagination metadata. This module implements bounded authenticated adapters
//! for both, projects capabilities without guessing embeddings/rerank, stores
//! complete account catalogs through PR7's incarnation-safe
//! [`crate::provider_registry::ProviderCacheStore`], and publishes one atomic
//! [`CatalogSnapshot`] generation.
//!
//! Multi-account selection is **default-enabled** after Gate D. Explicit
//! `GROK_MULTI_ACCOUNT_ROLLOUT=0|false|off|no` is a rollback kill switch that
//! omits additional accounts from projection and retained raw. The single
//! user-facing API is [`CatalogSnapshot::gated_projection`].

mod bounds;
mod http_body;
mod openai_adapter;
mod openrouter_adapter;
mod origin;
mod project;
mod publish;
mod refresh;
mod types;

#[cfg(test)]
mod tests;

pub use bounds::{
    CatalogBoundError, CatalogFetchBounds, CatalogFetchBudget, DEFAULT_MAX_DURATION,
    DEFAULT_MAX_MODELS, DEFAULT_MAX_PAGE_BYTES, DEFAULT_MAX_PAGES, DEFAULT_MAX_TOTAL_BYTES,
    DEFAULT_PAGE_SIZE, DEFAULT_REQUEST_TIMEOUT,
};
pub use openai_adapter::{fetch_openai_catalog, parse_openai_models_body};
pub use openrouter_adapter::{
    fetch_openrouter_bounded_list_body, fetch_openrouter_catalog, parse_openrouter_models_body,
};
pub use project::{
    apply_manual_capability_overrides, canonical_selection_id,
    conservative_openrouter_context_window, conservative_openrouter_max_output_ceiling,
    dedupe_and_sort_models, is_built_in_compatibility_instance, is_exact_built_in_slug,
    project_openai_capabilities, project_openrouter_capabilities,
};
pub use publish::{
    AccountRefreshOutcome, CatalogPublisher, CatalogSnapshot, GatedCatalogProjection,
    merge_account_results,
};
pub use refresh::{
    CatalogCredential, CatalogRefreshCoordinator, CatalogRefreshTarget, RegistryGenerationFn,
    build_account_identity, load_cached_account, models_list_url_from_base,
};
pub use types::{
    CatalogAccountIdentity, CatalogAdapterError, CatalogFetchSource, CatalogTruncationReason,
    DiscoveredModel, InstanceCatalogResult, ProjectedCapabilities,
};

// Gate-env test lock lives in `provider_registry::gate` and is re-exported
// from `provider_registry` so catalog tests and gate tests share one mutex.
