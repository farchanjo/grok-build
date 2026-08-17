//! Account-aware model identity resolution.
//!
//! One explicit resolver covers every routing-boundary lookup:
//! exact canonical, permanent compatibility alias, unique legacy
//! upstream/display alias, ambiguous candidates, or missing.
//!
//! Exact catalog keys always win. There is no first/last iteration fallback.
//! Additional-account models cannot steal built-in or explicit user IDs.
//!
//! Multi-account publication origins live in a crate-private side map
//! ([`CatalogOrigins`]), never on public [`ModelEntry`].

use std::fmt;

use indexmap::IndexMap;
use xai_grok_models::{
    CanonicalModelId, ModelRouteProvenance, UpstreamModelId, is_builtin_provider_prefix,
    split_first_colon,
};

use crate::agent::config::ModelEntry;

/// Publication origin of a catalog selection entry (private side-map value).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CatalogEntryOrigin {
    #[default]
    LegacyBuiltIn,
    ExplicitUser,
    GeneratedBuiltIn,
    GeneratedAdditionalAccount,
}

impl CatalogEntryOrigin {
    pub const fn is_generated_additional_account(self) -> bool {
        matches!(self, Self::GeneratedAdditionalAccount)
    }
}

/// Crate-private origin side map keyed by canonical catalog id.
pub type CatalogOrigins = IndexMap<String, CatalogEntryOrigin>;

pub fn origin_for_key(origins: &CatalogOrigins, key: &str) -> CatalogEntryOrigin {
    origins
        .get(key)
        .copied()
        .unwrap_or(CatalogEntryOrigin::LegacyBuiltIn)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelIdentityProvenance {
    ExactCanonical,
    PermanentCompatibility,
    UniqueLegacyAlias,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedModelIdentity {
    pub canonical_id: CanonicalModelId,
    pub upstream_id: UpstreamModelId,
    pub provenance: ModelIdentityProvenance,
}

impl ResolvedModelIdentity {
    pub fn catalog_key(&self) -> &str {
        self.canonical_id.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelIdentityResolution {
    Resolved(ResolvedModelIdentity),
    Ambiguous {
        input: String,
        candidates: Vec<CanonicalModelId>,
    },
    Missing {
        input: String,
    },
}

impl ModelIdentityResolution {
    pub fn resolved(self) -> Option<ResolvedModelIdentity> {
        match self {
            Self::Resolved(resolved) => Some(resolved),
            Self::Ambiguous { .. } | Self::Missing { .. } => None,
        }
    }

    pub fn as_resolved(&self) -> Option<&ResolvedModelIdentity> {
        match self {
            Self::Resolved(resolved) => Some(resolved),
            Self::Ambiguous { .. } | Self::Missing { .. } => None,
        }
    }

    pub fn is_ambiguous(&self) -> bool {
        matches!(self, Self::Ambiguous { .. })
    }
}

impl fmt::Display for ModelIdentityResolution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resolved(resolved) => write!(
                f,
                "resolved {} → {} ({:?})",
                resolved.canonical_id, resolved.upstream_id, resolved.provenance
            ),
            Self::Ambiguous { input, candidates } => {
                let ids: Vec<&str> = candidates.iter().map(CanonicalModelId::as_str).collect();
                write!(f, "ambiguous model id `{input}`: {}", ids.join(", "))
            }
            Self::Missing { input } => write!(f, "unknown model id `{input}`"),
        }
    }
}

pub fn is_additional_account_key(key: &str, origins: &CatalogOrigins) -> bool {
    origin_for_key(origins, key).is_generated_additional_account()
}

pub fn build_catalog_origins(
    catalog: &IndexMap<String, ModelEntry>,
    user_authored_keys: &std::collections::HashSet<String>,
) -> CatalogOrigins {
    let mut origins = CatalogOrigins::with_capacity(catalog.len());
    for key in catalog.keys() {
        origins.insert(
            key.clone(),
            classify_catalog_origin(key, user_authored_keys),
        );
    }
    origins
}

pub fn classify_catalog_origin(
    catalog_key: &str,
    user_authored_keys: &std::collections::HashSet<String>,
) -> CatalogEntryOrigin {
    if user_authored_keys.contains(catalog_key) {
        return CatalogEntryOrigin::ExplicitUser;
    }
    match split_first_colon(catalog_key) {
        None => CatalogEntryOrigin::LegacyBuiltIn,
        Some((prefix, remainder)) if !remainder.is_empty() => {
            if is_builtin_provider_prefix(prefix) {
                CatalogEntryOrigin::GeneratedBuiltIn
            } else {
                CatalogEntryOrigin::GeneratedAdditionalAccount
            }
        }
        Some(_) => CatalogEntryOrigin::LegacyBuiltIn,
    }
}

pub fn apply_multi_account_publication_gate(
    catalog: &mut IndexMap<String, ModelEntry>,
    origins: &CatalogOrigins,
) {
    if crate::provider_registry::multi_account_rollout_enabled() {
        return;
    }
    for (key, entry) in catalog.iter_mut() {
        if is_additional_account_key(key, origins) {
            entry.info.hidden = true;
            entry.info.user_selectable = false;
        }
    }
}

fn additional_account_gated(key: &str, origins: &CatalogOrigins) -> bool {
    !crate::provider_registry::multi_account_rollout_enabled()
        && is_additional_account_key(key, origins)
}

/// Compatibility resolver without origin metadata (never over-hides).
pub fn resolve_model_identity(
    models: &IndexMap<String, ModelEntry>,
    requested: &str,
) -> ModelIdentityResolution {
    resolve_model_identity_with_origins(models, &CatalogOrigins::new(), requested)
}

/// Origin-aware production resolver.
pub fn resolve_model_identity_with_origins(
    models: &IndexMap<String, ModelEntry>,
    origins: &CatalogOrigins,
    requested: &str,
) -> ModelIdentityResolution {
    let requested = requested.trim();
    if requested.is_empty() {
        return ModelIdentityResolution::Missing {
            input: requested.to_owned(),
        };
    }

    if let Some(entry) = models.get(requested) {
        if additional_account_gated(requested, origins) {
            return ModelIdentityResolution::Missing {
                input: requested.to_owned(),
            };
        }
        return match resolved_from_entry(requested, entry, ModelIdentityProvenance::ExactCanonical)
        {
            Ok(resolved) => ModelIdentityResolution::Resolved(resolved),
            Err(_) => ModelIdentityResolution::Missing {
                input: requested.to_owned(),
            },
        };
    }

    let reserved_matches =
        collect_alias_matches(models, origins, requested, AliasScope::ReservedOnly);
    match reserved_matches.len() {
        1 => {
            return resolution_from_matches(
                requested,
                reserved_matches,
                ModelIdentityProvenance::PermanentCompatibility,
            );
        }
        n if n > 1 => return ambiguous(requested, reserved_matches),
        _ => {}
    }

    let all_matches = collect_alias_matches(models, origins, requested, AliasScope::All);
    match all_matches.len() {
        0 => ModelIdentityResolution::Missing {
            input: requested.to_owned(),
        },
        1 => resolution_from_matches(
            requested,
            all_matches,
            ModelIdentityProvenance::UniqueLegacyAlias,
        ),
        _ => ambiguous(requested, all_matches),
    }
}

pub fn resolve_catalog_key_str(
    models: &IndexMap<String, ModelEntry>,
    requested: &str,
) -> Option<String> {
    resolve_model_identity(models, requested)
        .as_resolved()
        .map(|resolved| resolved.canonical_id.as_str().to_owned())
}

pub fn resolve_catalog_key_str_with_origins(
    models: &IndexMap<String, ModelEntry>,
    origins: &CatalogOrigins,
    requested: &str,
) -> Option<String> {
    resolve_model_identity_with_origins(models, origins, requested)
        .as_resolved()
        .map(|resolved| resolved.canonical_id.as_str().to_owned())
}

pub fn find_resolved_model<'a>(
    models: &'a IndexMap<String, ModelEntry>,
    requested: &str,
) -> Option<&'a ModelEntry> {
    let key = resolve_catalog_key_str(models, requested)?;
    models.get(&key)
}

pub fn find_resolved_model_with_origins<'a>(
    models: &'a IndexMap<String, ModelEntry>,
    origins: &CatalogOrigins,
    requested: &str,
) -> Option<&'a ModelEntry> {
    let key = resolve_catalog_key_str_with_origins(models, origins, requested)?;
    models.get(&key)
}

pub fn upstream_id_for_entry(
    entry: &ModelEntry,
) -> Result<UpstreamModelId, xai_grok_models::ModelIdError> {
    UpstreamModelId::new(entry.info.model.as_str())
}

pub fn canonical_id_for_key(key: &str) -> Result<CanonicalModelId, xai_grok_models::ModelIdError> {
    CanonicalModelId::new(key)
}

pub fn discovered_canonical_id(
    provider_instance_id: &str,
    upstream: &str,
) -> Result<CanonicalModelId, xai_grok_models::ModelIdError> {
    let upstream = UpstreamModelId::new(upstream)?;
    CanonicalModelId::discovered(provider_instance_id, &upstream)
}

pub fn provenance_from_entry(
    entry: &ModelEntry,
    provider_instance_id: Option<&str>,
    incarnation: Option<&str>,
    provider_kind: Option<&str>,
    api_surface: Option<&str>,
    registry_generation: Option<u64>,
    canonical_id: Option<&CanonicalModelId>,
) -> Option<ModelRouteProvenance> {
    let upstream = upstream_id_for_entry(entry).ok()?;
    let instance = provider_instance_id
        .or_else(|| entry.model_provider.as_ref().map(|p| p.id.as_str()))
        .unwrap_or("xai");
    if incarnation.is_some() && registry_generation.unwrap_or(0) == 0 {
        return None;
    }
    let mut provenance = ModelRouteProvenance::new(
        instance,
        incarnation,
        provider_kind.or_else(|| entry.model_provider.as_ref().map(|p| kind_label(p.kind))),
        api_surface,
        &upstream,
        registry_generation.unwrap_or(0),
    )
    .ok()?;
    if let Some(canonical) = canonical_id {
        provenance = provenance.with_canonical_model(canonical);
    }
    Some(provenance)
}

fn kind_label(kind: crate::agent::model_providers::ModelProviderKind) -> &'static str {
    use crate::agent::model_providers::ModelProviderKind;
    match kind {
        ModelProviderKind::Xai => "xai",
        ModelProviderKind::OpenAi => "openai",
        ModelProviderKind::OpenRouter => "openrouter",
        ModelProviderKind::Anthropic => "anthropic",
        ModelProviderKind::Zai => "zai",
        ModelProviderKind::OpenAiCompatible => "openai_compatible",
    }
}

#[derive(Clone, Copy)]
enum AliasScope {
    ReservedOnly,
    All,
}

struct AliasMatch<'a> {
    key: &'a str,
    entry: &'a ModelEntry,
}

fn collect_alias_matches<'a>(
    models: &'a IndexMap<String, ModelEntry>,
    origins: &CatalogOrigins,
    requested: &str,
    scope: AliasScope,
) -> Vec<AliasMatch<'a>> {
    let mut matches = Vec::new();
    for (key, entry) in models {
        if additional_account_gated(key, origins) {
            continue;
        }
        if !alias_in_scope(key, scope) {
            continue;
        }
        if entry_matches_alias(key, entry, requested) {
            matches.push(AliasMatch { key, entry });
        }
    }
    matches
}

fn alias_in_scope(key: &str, scope: AliasScope) -> bool {
    match scope {
        AliasScope::All => true,
        AliasScope::ReservedOnly => is_reserved_catalog_key(key),
    }
}

pub fn is_reserved_catalog_key(key: &str) -> bool {
    match split_first_colon(key) {
        None => true,
        Some((prefix, remainder)) => !remainder.is_empty() && is_builtin_provider_prefix(prefix),
    }
}

fn entry_matches_alias(key: &str, entry: &ModelEntry, requested: &str) -> bool {
    if entry.info.model == requested {
        return true;
    }
    if entry
        .info
        .id
        .as_deref()
        .is_some_and(|id| id == requested && id != key)
    {
        return true;
    }
    if entry
        .info
        .name
        .as_deref()
        .is_some_and(|name| name == requested)
    {
        return true;
    }
    false
}

fn resolution_from_matches(
    requested: &str,
    matches: Vec<AliasMatch<'_>>,
    provenance: ModelIdentityProvenance,
) -> ModelIdentityResolution {
    let Some(matched) = matches.into_iter().next() else {
        return ModelIdentityResolution::Missing {
            input: requested.to_owned(),
        };
    };
    match resolved_from_entry(matched.key, matched.entry, provenance) {
        Ok(resolved) => ModelIdentityResolution::Resolved(resolved),
        Err(_) => ModelIdentityResolution::Missing {
            input: requested.to_owned(),
        },
    }
}

fn ambiguous(requested: &str, matches: Vec<AliasMatch<'_>>) -> ModelIdentityResolution {
    let mut candidates: Vec<CanonicalModelId> = matches
        .into_iter()
        .filter_map(|matched| CanonicalModelId::new(matched.key).ok())
        .collect();
    candidates.sort();
    candidates.dedup();
    ModelIdentityResolution::Ambiguous {
        input: requested.to_owned(),
        candidates,
    }
}

fn resolved_from_entry(
    key: &str,
    entry: &ModelEntry,
    provenance: ModelIdentityProvenance,
) -> Result<ResolvedModelIdentity, xai_grok_models::ModelIdError> {
    Ok(ResolvedModelIdentity {
        canonical_id: CanonicalModelId::new(key)?,
        upstream_id: upstream_id_for_entry(entry)?,
        provenance,
    })
}

/// Require an exact live route when persisted provenance pins an exact route.
pub fn require_exact_route(
    stored: &ModelRouteProvenance,
    live_instance_id: Option<&str>,
    live_incarnation: Option<&str>,
    live_kind: Option<&str>,
    live_surface: Option<&str>,
    live_upstream: Option<&str>,
    live_registry_generation: Option<u64>,
) -> Result<(), RouteResumeError> {
    if !stored.requires_exact_route() {
        if live_instance_id.is_some()
            && !stored.matches_live(
                live_instance_id,
                live_incarnation,
                live_kind,
                live_surface,
                live_upstream,
                live_registry_generation,
            )
        {
            return Err(RouteResumeError::IncarnationMismatch);
        }
        return Ok(());
    }
    if stored.matches_live(
        live_instance_id,
        live_incarnation,
        live_kind,
        live_surface,
        live_upstream,
        live_registry_generation,
    ) {
        Ok(())
    } else if live_instance_id.is_none() {
        Err(RouteResumeError::MissingRoute)
    } else {
        Err(RouteResumeError::IncarnationMismatch)
    }
}

/// Validate a persisted companion against a live production route context.
///
/// Uses instance, incarnation, kind, surface, upstream, and registry generation
/// from the live freeze — the same axes `matches_live` / exact resume require.
pub fn validate_companion_against_live_route(
    stored: &ModelRouteProvenance,
    live: &xai_grok_inference::ProviderRouteContext,
    live_upstream: &str,
) -> Result<(), RouteResumeError> {
    require_exact_route(
        stored,
        Some(live.instance_id()),
        live.incarnation(),
        Some(live.provider_kind().as_str()),
        Some(live.api_surface().as_str()),
        Some(live_upstream),
        Some(live.registry_generation()),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteResumeError {
    MissingRoute,
    IncarnationMismatch,
}

impl fmt::Display for RouteResumeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRoute => write!(
                f,
                "persisted model route is no longer available; start a new session"
            ),
            Self::IncarnationMismatch => write!(
                f,
                "persisted model route incarnation does not match the live account; start a new session"
            ),
        }
    }
}

impl std::error::Error for RouteResumeError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::config::{ModelEntry, ModelInfo};
    use crate::agent::execution_backend::ExecutionBackend;
    use crate::agent::model_providers::{ModelProviderKind, ResolvedModelProvider};
    use crate::inference::ApiBackend;
    use std::num::NonZeroU64;
    use xai_grok_inference_types::ReasoningEffortSelection;

    fn entry(model: &str) -> ModelEntry {
        entry_with_provider(model, None)
    }

    fn entry_with_provider(model: &str, provider: Option<(&str, ModelProviderKind)>) -> ModelEntry {
        ModelEntry {
            info: ModelInfo {
                user_selectable: true,
                id: None,
                model: model.to_string(),
                base_url: String::new(),
                name: None,
                description: None,
                max_completion_tokens: None,
                temperature: None,
                top_p: None,
                api_backend: ApiBackend::default(),
                auth_scheme: Default::default(),
                extra_headers: IndexMap::new(),
                context_window: NonZeroU64::new(200_000).unwrap(),
                auto_compact_threshold_percent: None,
                system_prompt_label: None,
                use_concise: false,
                agent_type: crate::agent::config::default_agent_type(),
                inference_idle_timeout_secs: None,
                max_retries: None,
                hidden: false,
                supported_in_api: true,
                reasoning_effort: None,
                supports_reasoning_effort: None,
                reasoning_efforts: Vec::new(),
                reasoning_effort_selection: ReasoningEffortSelection::Unknown,
                supports_backend_search: false,
                compactions_remaining: None,
                compaction_at_tokens: None,
                show_model_fingerprint: false,
                stream_tool_calls: None,
                laziness_detector: crate::agent::config::LazinessDetectorPerModelConfig::default(),
                supports_tools: None,
                supports_native_schema: None,
                supports_strict_tools: None,
                supports_image_input: None,
                supports_audio_input: None,
                supports_video_input: None,
                execution_backend: ExecutionBackend::NativeInference,
            },
            model_provider: provider.map(|(id, kind)| ResolvedModelProvider {
                id: id.to_owned(),
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

    fn catalog(pairs: &[(&str, &str)]) -> IndexMap<String, ModelEntry> {
        pairs
            .iter()
            .map(|(key, model)| ((*key).to_owned(), entry(model)))
            .collect()
    }

    #[test]
    fn exact_canonical_wins_over_slug() {
        let models = catalog(&[("a", "target"), ("target", "other")]);
        let resolved = resolve_model_identity(&models, "target")
            .resolved()
            .expect("exact key");
        assert_eq!(resolved.canonical_id.as_str(), "target");
        assert_eq!(resolved.upstream_id.as_str(), "other");
        assert_eq!(resolved.provenance, ModelIdentityProvenance::ExactCanonical);
    }

    #[test]
    fn user_defined_key_without_colon_is_exact() {
        let models = catalog(&[("my-local", "llama-3")]);
        let resolved = resolve_model_identity(&models, "my-local")
            .resolved()
            .unwrap();
        assert_eq!(resolved.canonical_id.as_str(), "my-local");
        assert!(resolved.canonical_id.is_reserved_compatibility_selection());
    }

    #[test]
    fn curated_and_discovered_builtin_ids_are_byte_stable() {
        let models = catalog(&[
            ("openai-gpt-5.6-sol", "gpt-5.6-sol"),
            ("openai:gpt-custom-preview", "gpt-custom-preview"),
        ]);
        for (key, upstream) in [
            ("openai-gpt-5.6-sol", "gpt-5.6-sol"),
            ("openai:gpt-custom-preview", "gpt-custom-preview"),
        ] {
            let resolved = resolve_model_identity(&models, key).resolved().unwrap();
            assert_eq!(resolved.canonical_id.as_str(), key);
            assert_eq!(resolved.upstream_id.as_str(), upstream);
        }
    }

    #[test]
    fn permanent_alias_beats_additional_account() {
        let models = catalog(&[
            ("openai-gpt-5.6-sol", "gpt-5.6-sol"),
            ("work-openai:gpt-5.6-sol", "gpt-5.6-sol"),
        ]);
        let resolved = resolve_model_identity(&models, "gpt-5.6-sol")
            .resolved()
            .unwrap();
        assert_eq!(resolved.canonical_id.as_str(), "openai-gpt-5.6-sol");
        assert_eq!(
            resolved.provenance,
            ModelIdentityProvenance::PermanentCompatibility
        );
    }

    #[test]
    fn ambiguous_alias_lists_deterministic_candidates() {
        let models = catalog(&[
            ("default-grok-build", "grok-4.5"),
            ("user-grok-build", "grok-4.5"),
        ]);
        match resolve_model_identity(&models, "grok-4.5") {
            ModelIdentityResolution::Ambiguous { candidates, .. } => {
                let ids: Vec<&str> = candidates.iter().map(CanonicalModelId::as_str).collect();
                assert_eq!(ids, vec!["default-grok-build", "user-grok-build"]);
            }
            other => panic!("expected ambiguous, got {other:?}"),
        }
        assert!(resolve_catalog_key_str(&models, "grok-4.5").is_none());
    }

    #[test]
    fn four_duplicate_upstreams_get_distinct_canonical_ids() {
        let models = catalog(&[
            ("openai:gpt-4o", "gpt-4o"),
            ("work-openai:gpt-4o", "gpt-4o"),
            ("home-openai:gpt-4o", "gpt-4o"),
            ("lab-openai:gpt-4o", "gpt-4o"),
        ]);
        assert_eq!(models.len(), 4);
        match resolve_model_identity(&models, "gpt-4o") {
            ModelIdentityResolution::Resolved(resolved) => {
                assert_eq!(resolved.canonical_id.as_str(), "openai:gpt-4o");
            }
            other => panic!("built-in openai: prefix must stay bound, got {other:?}"),
        }
    }

    #[test]
    fn missing_input_is_missing() {
        let models = catalog(&[("grok-4.5", "grok-4.5")]);
        match resolve_model_identity(&models, "no-such-model") {
            ModelIdentityResolution::Missing { input } => assert_eq!(input, "no-such-model"),
            other => panic!("expected missing, got {other:?}"),
        }
    }

    #[test]
    fn additional_account_gated_when_rollout_off() {
        use crate::provider_registry::MULTI_ACCOUNT_ROLLOUT_ENV;
        use std::sync::{Mutex, OnceLock};
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let _guard = LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let previous = std::env::var(MULTI_ACCOUNT_ROLLOUT_ENV).ok();
        unsafe { std::env::remove_var(MULTI_ACCOUNT_ROLLOUT_ENV) };

        let mut models = IndexMap::new();
        models.insert("openai-gpt-5.6-sol".into(), entry("gpt-5.6-sol"));
        models.insert(
            "work-openai:gpt-5.6-sol".into(),
            entry_with_provider(
                "gpt-5.6-sol",
                Some(("work-openai", ModelProviderKind::OpenAi)),
            ),
        );
        let mut origins = CatalogOrigins::new();
        origins.insert(
            "openai-gpt-5.6-sol".into(),
            CatalogEntryOrigin::LegacyBuiltIn,
        );
        origins.insert(
            "work-openai:gpt-5.6-sol".into(),
            CatalogEntryOrigin::GeneratedAdditionalAccount,
        );
        apply_multi_account_publication_gate(&mut models, &origins);
        assert!(models["work-openai:gpt-5.6-sol"].info.hidden);
        assert!(!models["openai-gpt-5.6-sol"].info.hidden);
        assert!(
            resolve_model_identity_with_origins(&models, &origins, "work-openai:gpt-5.6-sol")
                .resolved()
                .is_none()
        );
        // Compatibility path never over-hides.
        assert!(
            resolve_model_identity(&models, "work-openai:gpt-5.6-sol")
                .resolved()
                .is_some()
        );

        match previous {
            Some(v) => unsafe { std::env::set_var(MULTI_ACCOUNT_ROLLOUT_ENV, v) },
            None => unsafe { std::env::remove_var(MULTI_ACCOUNT_ROLLOUT_ENV) },
        }
    }

    #[test]
    fn model_entry_base_shape_literal_has_no_origin_field() {
        let entry = entry("gpt-4o");
        let json = serde_json::to_string(&entry).unwrap();
        assert!(!json.contains("catalog_origin"));
        let back: ModelEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.info.model, "gpt-4o");
    }

    #[test]
    fn inference_config_receives_only_upstream_id() {
        let models = catalog(&[("openai-gpt-5.6-sol", "gpt-5.6-sol")]);
        let resolved = resolve_model_identity(&models, "openai-gpt-5.6-sol")
            .resolved()
            .unwrap();
        assert_ne!(
            resolved.canonical_id.as_str(),
            resolved.upstream_id.as_str()
        );
    }

    #[test]
    fn discovered_ids_are_distinct_for_same_upstream() {
        let a = discovered_canonical_id("openai", "gpt-4o").unwrap();
        let b = discovered_canonical_id("work-openai", "gpt-4o").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn validate_companion_against_live_route_rejects_incarnation_and_registry_drift() {
        use xai_grok_inference::{
            ProviderRouteContext, RouteApiSurface, RouteAuthority, RouteCredentialRoute,
            RouteProviderKind,
        };
        use xai_grok_models::{ModelRouteProvenance, UpstreamModelId};

        let upstream = UpstreamModelId::new("gpt-4o").unwrap();
        let stored = ModelRouteProvenance::new(
            "openai",
            Some("11111111-1111-1111-1111-111111111111"),
            Some("openai"),
            Some("openai_platform"),
            &upstream,
            3,
        )
        .unwrap();

        let live = |instance: &str, incarnation: &str, registry: u64| {
            ProviderRouteContext::builder()
                .instance_id(instance)
                .provider_kind(RouteProviderKind::OpenAi)
                .api_surface(RouteApiSurface::OpenAiPlatform)
                .credential_route(RouteCredentialRoute::ApiKey)
                .incarnation(incarnation)
                .registry_generation(registry)
                .binding_generation(1)
                .authority(RouteAuthority::Authoritative)
                .model_partition("gpt-4o")
                .build()
                .unwrap()
        };

        assert!(
            validate_companion_against_live_route(
                &stored,
                &live("openai", "11111111-1111-1111-1111-111111111111", 3),
                "gpt-4o"
            )
            .is_ok()
        );
        assert_eq!(
            validate_companion_against_live_route(
                &stored,
                &live("openai", "22222222-2222-2222-2222-222222222222", 3),
                "gpt-4o"
            ),
            Err(RouteResumeError::IncarnationMismatch)
        );
        assert_eq!(
            validate_companion_against_live_route(
                &stored,
                &live("openai", "11111111-1111-1111-1111-111111111111", 9),
                "gpt-4o"
            ),
            Err(RouteResumeError::IncarnationMismatch)
        );
        assert_eq!(
            validate_companion_against_live_route(
                &stored,
                &live("openai_work", "11111111-1111-1111-1111-111111111111", 3),
                "gpt-4o"
            ),
            Err(RouteResumeError::IncarnationMismatch)
        );
    }
}
