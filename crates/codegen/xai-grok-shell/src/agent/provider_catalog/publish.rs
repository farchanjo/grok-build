//! Atomic multi-instance catalog publication into a generation-tagged snapshot.
//!
//! Independent accounts may fetch concurrently, but publication is one atomic
//! complete generation. Open readers observe either the previous complete
//! snapshot or the new complete snapshot — never a mixed/partial merge.
//!
//! Multi-account rows remain gated. The single projection API is
//! [`CatalogSnapshot::gated_projection`]: gate-off omits additional accounts
//! entirely; gate-on publishes them as visible and user-selectable. Callers
//! must not search raw `accounts` for user-facing selection.

use super::project::is_built_in_compatibility_instance;
use super::types::{CatalogFetchSource, DiscoveredModel, InstanceCatalogResult};
use crate::agent::config::{ModelEntry, ModelInfo};
use crate::agent::model_providers::{ModelProviderKind, ResolvedModelProvider};
use crate::provider_registry::{ProviderKind, multi_account_rollout_enabled};
use indexmap::IndexMap;
use parking_lot::RwLock;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Gated, user-facing projection of a catalog snapshot.
#[derive(Debug, Clone)]
pub struct GatedCatalogProjection {
    /// Accounts visible under the current gate (complete only).
    pub accounts: IndexMap<String, InstanceCatalogResult>,
    /// Selection-keyed entries. Gate-on includes additional accounts as
    /// visible + selectable. Gate-off omits them entirely.
    pub selection_entries: IndexMap<String, ModelEntry>,
    /// Canonical ids that are present only as gate-hidden additional rows.
    /// Empty when Gate D is open (additional accounts are selectable).
    pub hidden_additional_ids: Vec<String>,
    /// Collision diagnostics (additional account lost to built-in key).
    pub collisions: Vec<String>,
}

impl GatedCatalogProjection {
    /// User-selectable entries for `/model` and ACP.
    pub fn visible_entries(&self) -> IndexMap<String, ModelEntry> {
        self.selection_entries
            .iter()
            .filter(|(_, e)| e.info.user_selectable && !e.info.hidden)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Look up by canonical id. Returns `None` for hidden / non-selectable rows
    /// when the caller asks for selectable-only (default for `/model`).
    pub fn get_selectable(&self, canonical_id: &str) -> Option<&ModelEntry> {
        self.selection_entries
            .get(canonical_id)
            .filter(|e| e.info.user_selectable && !e.info.hidden)
    }

    /// Internal lookup including hidden additional rows (diagnostics only).
    pub fn get_any(&self, canonical_id: &str) -> Option<&ModelEntry> {
        self.selection_entries.get(canonical_id)
    }
}

/// One atomic published catalog generation.
#[derive(Debug, Clone)]
pub struct CatalogSnapshot {
    pub generation: u64,
    pub registry_generation: u64,
    /// All complete account results retained for this generation (pre-gate).
    /// Prefer [`Self::gated_projection`] for any user-facing surface.
    accounts_raw: IndexMap<String, InstanceCatalogResult>,
    projection: GatedCatalogProjection,
}

impl CatalogSnapshot {
    pub fn empty(generation: u64, registry_generation: u64) -> Self {
        Self {
            generation,
            registry_generation,
            accounts_raw: IndexMap::new(),
            projection: GatedCatalogProjection {
                accounts: IndexMap::new(),
                selection_entries: IndexMap::new(),
                hidden_additional_ids: Vec::new(),
                collisions: Vec::new(),
            },
        }
    }

    /// Single gated projection API for all consumers.
    pub fn gated_projection(&self) -> &GatedCatalogProjection {
        &self.projection
    }

    /// Visible entries (delegates to gated projection).
    pub fn visible_entries(&self) -> IndexMap<String, ModelEntry> {
        self.projection.visible_entries()
    }

    /// Selectable-only get (never returns gate-hidden additional rows).
    pub fn get(&self, canonical_id: &str) -> Option<&ModelEntry> {
        self.projection.get_selectable(canonical_id)
    }

    /// Account count after gate filtering.
    pub fn gated_account_count(&self) -> usize {
        self.projection.accounts.len()
    }

    /// Raw complete accounts (tests / diagnostics). Not for `/model` selection.
    #[cfg(test)]
    pub fn raw_accounts_for_test(&self) -> &IndexMap<String, InstanceCatalogResult> {
        &self.accounts_raw
    }

    #[cfg(test)]
    pub fn hidden_additional_ids(&self) -> &[String] {
        &self.projection.hidden_additional_ids
    }

    #[cfg(test)]
    pub fn selection_entries_for_test(&self) -> &IndexMap<String, ModelEntry> {
        &self.projection.selection_entries
    }
}

/// Generation-tagged publisher.
#[derive(Debug)]
pub struct CatalogPublisher {
    generation: AtomicU64,
    current: RwLock<Arc<CatalogSnapshot>>,
}

impl Default for CatalogPublisher {
    fn default() -> Self {
        Self::new()
    }
}

impl CatalogPublisher {
    pub fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            current: RwLock::new(Arc::new(CatalogSnapshot::empty(0, 0))),
        }
    }

    pub fn begin_generation(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::AcqRel) + 1
    }

    pub fn current_generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    pub fn load(&self) -> Arc<CatalogSnapshot> {
        self.current.read().clone()
    }

    pub fn publish_if_current(
        &self,
        expected_generation: u64,
        registry_generation: u64,
        accounts: IndexMap<String, InstanceCatalogResult>,
    ) -> bool {
        let current_gen = self.generation.load(Ordering::Acquire);
        if expected_generation != current_gen {
            return false;
        }
        for result in accounts.values() {
            if result.publication_generation != expected_generation {
                return false;
            }
            // Refuse truncated rows at the publication boundary.
            if !result.is_complete_publishable() {
                return false;
            }
        }
        let projection = build_gated_projection(&accounts, multi_account_rollout_enabled());
        // Gate-off: drop additional from raw retained map as well so accounts
        // cannot leak multi-account IDs through any consumer of the snapshot.
        let accounts_retained = if multi_account_rollout_enabled() {
            accounts
        } else {
            accounts
                .into_iter()
                .filter(|(_, r)| {
                    is_built_in_compatibility_instance(&r.provider_instance_id, r.provider_kind)
                })
                .collect()
        };
        let snapshot = Arc::new(CatalogSnapshot {
            generation: expected_generation,
            registry_generation,
            accounts_raw: accounts_retained,
            projection,
        });
        let mut guard = self.current.write();
        let live_gen = self.generation.load(Ordering::Acquire);
        if expected_generation != live_gen {
            return false;
        }
        *guard = snapshot;
        true
    }

    pub fn publish_complete(
        &self,
        registry_generation: u64,
        accounts: IndexMap<String, InstanceCatalogResult>,
    ) -> u64 {
        let pub_gen = self.begin_generation();
        let mut tagged = accounts;
        for result in tagged.values_mut() {
            result.publication_generation = pub_gen;
        }
        let _ = self.publish_if_current(pub_gen, registry_generation, tagged);
        pub_gen
    }
}

/// Build the gated projection. Canonical selection collisions between a
/// built-in and an additional account fail the additional account loudly
/// (collision recorded; additional row dropped) rather than silent first-wins.
fn build_gated_projection(
    accounts: &IndexMap<String, InstanceCatalogResult>,
    gate_open: bool,
) -> GatedCatalogProjection {
    let mut gated_accounts = IndexMap::new();
    let mut selection = IndexMap::new();
    let mut collisions = Vec::new();

    // Pass 1: built-in compatibility accounts first (deterministic order).
    for (id, result) in accounts {
        if !result.is_complete_publishable() {
            continue;
        }
        let is_built_in =
            is_built_in_compatibility_instance(&result.provider_instance_id, result.provider_kind);
        if !is_built_in {
            continue;
        }
        gated_accounts.insert(id.clone(), result.clone());
        for model in &result.models {
            let entry = discovered_to_model_entry(model);
            selection.insert(model.canonical_selection_id.clone(), entry);
        }
    }

    // Pass 2: additional accounts only when gate is open (Gate D: visible + selectable).
    if gate_open {
        for (id, result) in accounts {
            if !result.is_complete_publishable() {
                continue;
            }
            let is_built_in = is_built_in_compatibility_instance(
                &result.provider_instance_id,
                result.provider_kind,
            );
            if is_built_in {
                continue;
            }
            // Detect selection-key collisions with built-ins before insert.
            let mut account_ok = true;
            for model in &result.models {
                if selection.contains_key(&model.canonical_selection_id) {
                    collisions.push(format!(
                        "canonical id `{}` collides with an existing entry; \
                         dropping additional account `{}`",
                        model.canonical_selection_id, result.provider_instance_id
                    ));
                    account_ok = false;
                    break;
                }
            }
            if !account_ok {
                continue;
            }
            gated_accounts.insert(id.clone(), result.clone());
            for model in &result.models {
                let entry = discovered_to_model_entry(model);
                selection.insert(model.canonical_selection_id.clone(), entry);
            }
        }
    }

    collisions.sort();
    GatedCatalogProjection {
        accounts: gated_accounts,
        selection_entries: selection,
        // Gate D open: additional accounts are selectable; nothing is gate-hidden.
        // Field retained for diagnostics / rollback compatibility.
        hidden_additional_ids: Vec::new(),
        collisions,
    }
}

fn discovered_to_model_entry(model: &DiscoveredModel) -> ModelEntry {
    use std::num::NonZeroU64;
    use xai_grok_inference_types::ReasoningEffortOption;

    let kind = match model.provider_kind {
        ProviderKind::OpenAi => ModelProviderKind::OpenAi,
        ProviderKind::OpenRouter => ModelProviderKind::OpenRouter,
        ProviderKind::Xai => ModelProviderKind::Xai,
        ProviderKind::Anthropic => ModelProviderKind::Anthropic,
        ProviderKind::Zai => ModelProviderKind::Zai,
        ProviderKind::OpenAiCompatible => ModelProviderKind::OpenAiCompatible,
    };
    let mut info = ModelInfo::fallback(&model.upstream_model_id);
    info.name = Some(
        model
            .display_name
            .clone()
            .unwrap_or_else(|| model.upstream_model_id.clone()),
    );
    info.description = model.description.clone();
    info.base_url =
        if model.endpoint_origin.contains("/v1") || model.endpoint_origin.contains("/api/") {
            model.endpoint_origin.clone()
        } else {
            format!("{}/v1", model.endpoint_origin.trim_end_matches('/'))
        };
    if let Some(ctx) = model.context_window.and_then(NonZeroU64::new) {
        info.context_window = ctx;
    }
    if let Some(max) = model.max_completion_tokens {
        info.max_completion_tokens = Some(max);
    }
    info.supports_tools = model.capabilities.supports_tools;
    info.supports_reasoning_effort = model.capabilities.supports_reasoning_effort;
    info.reasoning_efforts = model
        .capabilities
        .reasoning_efforts
        .iter()
        .filter_map(|raw| {
            let effort = raw
                .parse::<xai_grok_inference_types::ReasoningEffort>()
                .ok()?;
            Some(ReasoningEffortOption {
                id: raw.clone(),
                value: effort,
                label: raw.clone(),
                description: None,
                default: false,
            })
        })
        .collect();
    info.supports_image_input = model.capabilities.supports_image_input;
    info.supports_audio_input = model.capabilities.supports_audio_input;
    info.supports_video_input = model.capabilities.supports_video_input;
    info.user_selectable = true;
    info.hidden = false;

    ModelEntry {
        info,
        model_provider: Some(ResolvedModelProvider {
            id: model.provider_instance_id.clone(),
            kind,
            openrouter_fallback_models: Vec::new(),
            openrouter_provider_preferences: None,
            openrouter_plugins: Vec::new(),
            openrouter_pacing: false,
            command: Vec::new(),
        }),
        api_key: None,
        env_key: None,
        auth_provider: None,
        api_base_url: None,
    }
}

/// Merge account results: complete live/LKG update; failed-without-prior omits.
///
/// `Ok(complete)` replaces. `Err(Some(lkg))` keeps matching LKG. `Err(None)`
/// omits the key from updates (prior complete LKG stays via merge base).
pub fn merge_account_results(
    prior: &IndexMap<String, InstanceCatalogResult>,
    updates: IndexMap<String, AccountRefreshOutcome>,
) -> IndexMap<String, InstanceCatalogResult> {
    let mut out = prior.clone();
    for (id, outcome) in updates {
        match outcome {
            AccountRefreshOutcome::Complete(live) if live.is_complete_publishable() => {
                out.insert(id, live);
            }
            AccountRefreshOutcome::Complete(_) => {
                // Truncated — never replace prior.
            }
            AccountRefreshOutcome::RetainLkg(lkg) if lkg.is_complete_publishable() => {
                out.insert(id, lkg);
            }
            AccountRefreshOutcome::RetainLkg(_) => {}
            AccountRefreshOutcome::Omit => {
                // No matching complete prior: leave prior as-is (or absent).
            }
        }
    }
    // Drop any non-publishable rows that somehow entered the map.
    out.retain(|_, r| r.is_complete_publishable());
    out
}

/// Per-account refresh outcome for merge.
#[derive(Debug, Clone)]
pub enum AccountRefreshOutcome {
    Complete(InstanceCatalogResult),
    RetainLkg(InstanceCatalogResult),
    Omit,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::provider_catalog::types::CatalogTruncationReason;
    use crate::provider_registry::{ApiSurface, CredentialRoute, ProviderKind};

    fn sample_result(
        instance: &str,
        kind: ProviderKind,
        upstream: &str,
        canonical: &str,
        pub_gen: u64,
    ) -> InstanceCatalogResult {
        InstanceCatalogResult {
            provider_instance_id: instance.into(),
            provider_kind: kind,
            api_surface: ApiSurface::OpenAiPlatform,
            credential_route: CredentialRoute::ApiKey,
            endpoint_origin: "https://api.example.com".into(),
            org_project_fingerprint: String::new(),
            incarnation: None,
            credential_binding_id: None,
            registry_generation: 1,
            catalog_generation: 1,
            publication_generation: pub_gen,
            source: CatalogFetchSource::Live,
            truncation: CatalogTruncationReason::Complete,
            models: vec![DiscoveredModel {
                canonical_selection_id: canonical.into(),
                upstream_model_id: upstream.into(),
                display_name: Some(upstream.into()),
                description: None,
                context_window: None,
                max_completion_tokens: None,
                capabilities: Default::default(),
                provider_instance_id: instance.into(),
                provider_kind: kind,
                api_surface: ApiSurface::OpenAiPlatform,
                credential_route: CredentialRoute::ApiKey,
                endpoint_origin: "https://api.example.com".into(),
            }],
            diagnostic: None,
        }
    }

    #[test]
    fn stale_generation_discarded() {
        let publisher = CatalogPublisher::new();
        let gen1 = publisher.begin_generation();
        let gen2 = publisher.begin_generation();
        let mut accounts = IndexMap::new();
        accounts.insert(
            "openai".into(),
            sample_result(
                "openai",
                ProviderKind::OpenAi,
                "gpt-4o",
                "openai:gpt-4o",
                gen1,
            ),
        );
        assert!(!publisher.publish_if_current(gen1, 1, accounts));
        let mut accounts2 = IndexMap::new();
        accounts2.insert(
            "openai".into(),
            sample_result(
                "openai",
                ProviderKind::OpenAi,
                "gpt-4o",
                "openai:gpt-4o",
                gen2,
            ),
        );
        assert!(publisher.publish_if_current(gen2, 1, accounts2));
        assert_eq!(publisher.load().generation, gen2);
    }

    #[test]
    fn atomic_snapshot_not_partial() {
        let publisher = CatalogPublisher::new();
        let pub_gen = publisher.begin_generation();
        let mut accounts = IndexMap::new();
        accounts.insert(
            "openai".into(),
            sample_result("openai", ProviderKind::OpenAi, "a", "openai:a", pub_gen),
        );
        accounts.insert(
            "openrouter".into(),
            sample_result(
                "openrouter",
                ProviderKind::OpenRouter,
                "b",
                "openrouter:b",
                pub_gen,
            ),
        );
        assert!(publisher.publish_if_current(pub_gen, 1, accounts));
        let snap = publisher.load();
        assert_eq!(snap.gated_account_count(), 2);
        assert!(snap.get("openai:a").is_some());
        assert!(snap.get("openrouter:b").is_some());
    }

    #[test]
    fn gate_open_additional_accounts_are_selectable() {
        use crate::provider_registry::{MULTI_ACCOUNT_ROLLOUT_ENV, multi_account_rollout_env_lock};

        let _gate = multi_account_rollout_env_lock();
        let previous = std::env::var(MULTI_ACCOUNT_ROLLOUT_ENV).ok();
        unsafe { std::env::remove_var(MULTI_ACCOUNT_ROLLOUT_ENV) };

        let publisher = CatalogPublisher::new();
        let pub_gen = publisher.begin_generation();
        let mut accounts = IndexMap::new();
        accounts.insert(
            "openai".into(),
            sample_result(
                "openai",
                ProviderKind::OpenAi,
                "gpt-4o",
                "openai:gpt-4o",
                pub_gen,
            ),
        );
        accounts.insert(
            "openai_work".into(),
            sample_result(
                "openai_work",
                ProviderKind::OpenAi,
                "gpt-4o",
                "openai_work:gpt-4o",
                pub_gen,
            ),
        );
        assert!(publisher.publish_if_current(pub_gen, 1, accounts));
        let snap = publisher.load();
        let proj = snap.gated_projection();
        assert!(proj.accounts.contains_key("openai_work"));
        assert!(snap.get("openai_work:gpt-4o").is_some());
        assert!(proj.hidden_additional_ids.is_empty());
        let entry = proj.get_selectable("openai_work:gpt-4o").unwrap();
        assert!(entry.info.user_selectable);
        assert!(!entry.info.hidden);

        match previous {
            Some(v) => unsafe { std::env::set_var(MULTI_ACCOUNT_ROLLOUT_ENV, v) },
            None => unsafe { std::env::remove_var(MULTI_ACCOUNT_ROLLOUT_ENV) },
        }
    }

    #[test]
    fn gate_off_omits_additional_and_built_in_collision_wins() {
        use crate::provider_registry::{MULTI_ACCOUNT_ROLLOUT_ENV, multi_account_rollout_env_lock};

        let _gate = multi_account_rollout_env_lock();
        let previous = std::env::var(MULTI_ACCOUNT_ROLLOUT_ENV).ok();
        unsafe { std::env::set_var(MULTI_ACCOUNT_ROLLOUT_ENV, "0") };

        let publisher = CatalogPublisher::new();
        let pub_gen = publisher.begin_generation();
        let mut accounts = IndexMap::new();
        accounts.insert(
            "openai".into(),
            sample_result(
                "openai",
                ProviderKind::OpenAi,
                "gpt-4o",
                "openai:gpt-4o",
                pub_gen,
            ),
        );
        accounts.insert(
            "openai_work".into(),
            sample_result(
                "openai_work",
                ProviderKind::OpenAi,
                "gpt-4o",
                "openai_work:gpt-4o",
                pub_gen,
            ),
        );
        assert!(publisher.publish_if_current(pub_gen, 1, accounts));
        let snap = publisher.load();
        assert!(snap.get("openai:gpt-4o").is_some());
        assert!(snap.get("openai_work:gpt-4o").is_none());
        assert!(!snap.gated_projection().accounts.contains_key("openai_work"));
        assert!(
            !snap
                .gated_projection()
                .selection_entries
                .contains_key("openai_work:gpt-4o")
        );

        // Built-in-first collision under gate-open (restore enable).
        unsafe { std::env::remove_var(MULTI_ACCOUNT_ROLLOUT_ENV) };
        let publisher2 = CatalogPublisher::new();
        let gen2 = publisher2.begin_generation();
        let mut collision = IndexMap::new();
        collision.insert(
            "openai".into(),
            sample_result(
                "openai",
                ProviderKind::OpenAi,
                "shared",
                "openai:shared",
                gen2,
            ),
        );
        // Additional account reuses built-in canonical id → dropped + recorded.
        collision.insert(
            "openai_work".into(),
            sample_result(
                "openai_work",
                ProviderKind::OpenAi,
                "shared",
                "openai:shared",
                gen2,
            ),
        );
        assert!(publisher2.publish_if_current(gen2, 1, collision));
        let snap2 = publisher2.load();
        assert!(snap2.get("openai:shared").is_some());
        assert!(
            !snap2
                .gated_projection()
                .accounts
                .contains_key("openai_work")
        );
        assert!(!snap2.gated_projection().collisions.is_empty());

        match previous {
            Some(v) => unsafe { std::env::set_var(MULTI_ACCOUNT_ROLLOUT_ENV, v) },
            None => unsafe { std::env::remove_var(MULTI_ACCOUNT_ROLLOUT_ENV) },
        }
    }
}
