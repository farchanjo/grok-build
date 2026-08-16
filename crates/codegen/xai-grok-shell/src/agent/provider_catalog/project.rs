//! Capability projection and canonical selection-id assignment.

use super::types::{CatalogAccountIdentity, DiscoveredModel, ProjectedCapabilities};
use crate::provider_registry::ProviderKind;
use crate::provider_registry::lifecycle::namespaced_model_id;
use indexmap::IndexMap;

/// Build the canonical selection id for a discovered upstream model.
///
/// Built-in OpenAI / OpenRouter compatibility accounts keep the historical
/// `openai:<slug>` / `openrouter:<slug>` form so existing configs, sessions,
/// and eligibility rules continue to resolve. Additional configured accounts
/// always use `<instance_id>:<verbatim-upstream>`.
pub fn canonical_selection_id(identity: &CatalogAccountIdentity, upstream: &str) -> String {
    let upstream = upstream.trim();
    if identity.is_built_in_compatibility {
        match identity.kind {
            ProviderKind::OpenAi => format!("openai:{upstream}"),
            ProviderKind::OpenRouter => format!("openrouter:{upstream}"),
            // Other built-ins fall through to instance-qualified form.
            _ => namespaced_model_id(&identity.instance_id, upstream),
        }
    } else {
        namespaced_model_id(&identity.instance_id, upstream)
    }
}

/// Whether this instance id is a product built-in that uses compatibility ids.
///
/// Only the exact slugs `openai` and `openrouter` qualify. Legacy aliases
/// (`chatgpt`, `codex`) and user-configured instances remain instance-qualified
/// so they cannot silently collide with built-in catalog keys.
pub fn is_built_in_compatibility_instance(instance_id: &str, kind: ProviderKind) -> bool {
    match (instance_id, kind) {
        ("openai", ProviderKind::OpenAi) => true,
        ("openrouter", ProviderKind::OpenRouter) => true,
        _ => false,
    }
}

/// True when `instance_id` is exactly a built-in compatibility product id.
pub fn is_exact_built_in_slug(instance_id: &str) -> bool {
    matches!(instance_id, "openai" | "openrouter")
}

/// Apply explicit manual capability overrides from provider config.
///
/// Manual embeddings/rerank flags are authoritative. Generic auto-projection
/// never invents them; when the provider config sets them, they appear here.
pub fn apply_manual_capability_overrides(
    mut projected: ProjectedCapabilities,
    manual: &IndexMap<String, bool>,
) -> ProjectedCapabilities {
    if manual.is_empty() {
        return projected;
    }
    projected.manual_overrides = manual.clone();
    if let Some(v) = manual.get("embeddings").copied() {
        projected.supports_embeddings = Some(v);
    }
    if let Some(v) = manual
        .get("rerank")
        .or_else(|| manual.get("reranker"))
        .copied()
    {
        projected.supports_rerank = Some(v);
    }
    if let Some(v) = manual
        .get("tools")
        .or_else(|| manual.get("chat_completions"))
        .copied()
    {
        projected.supports_tools = Some(v);
    }
    projected
}

/// Default OpenAI capability projection: identity-only `/models` does not
/// establish tools, reasoning, modalities, embeddings, or rerank.
pub fn project_openai_capabilities(manual: &IndexMap<String, bool>) -> ProjectedCapabilities {
    // OpenAI GET /models is identity-only. Leave tool/reasoning/modality as
    // unknown (None) so curated presets can still supply product-blessed
    // capabilities and experimental discoveries stay non-agent by default.
    let projected = ProjectedCapabilities {
        supports_tools: None,
        supports_reasoning_effort: None,
        // Explicitly absent — never guess retrieval capabilities.
        supports_embeddings: None,
        supports_rerank: None,
        ..ProjectedCapabilities::default()
    };
    apply_manual_capability_overrides(projected, manual)
}

/// OpenRouter capability projection from advertised parameters/modalities.
pub fn project_openrouter_capabilities(
    supported_parameters: &[String],
    input_modalities: &[String],
    reasoning_efforts: Vec<String>,
    default_reasoning_effort: Option<String>,
    supports_reasoning_effort: bool,
    manual: &IndexMap<String, bool>,
) -> ProjectedCapabilities {
    let supported: Vec<String> = supported_parameters
        .iter()
        .map(|p| p.to_ascii_lowercase())
        .collect();
    let supports_tools = supported
        .iter()
        .any(|p| matches!(p.as_str(), "tools" | "tool_choice" | "function_calling"));
    let modality = |name: &str| {
        (!input_modalities.is_empty()).then(|| {
            input_modalities
                .iter()
                .any(|m| m.eq_ignore_ascii_case(name))
        })
    };
    let projected = ProjectedCapabilities {
        supports_tools: Some(supports_tools),
        supports_reasoning_effort: Some(supports_reasoning_effort),
        reasoning_efforts,
        default_reasoning_effort,
        supports_image_input: modality("image"),
        supports_audio_input: modality("audio"),
        supports_video_input: modality("video"),
        // Never invent embeddings/rerank from generic model list parameters.
        supports_embeddings: None,
        supports_rerank: None,
        manual_overrides: IndexMap::new(),
    };
    apply_manual_capability_overrides(projected, manual)
}

/// Deterministic first-wins duplicate policy within one account.
///
/// Models are returned in first-seen order; later rows with the same
/// `upstream_model_id` are dropped. Final order is then sorted by
/// `canonical_selection_id` for cache stability.
pub fn dedupe_and_sort_models(mut models: Vec<DiscoveredModel>) -> Vec<DiscoveredModel> {
    let mut seen = std::collections::HashSet::new();
    models.retain(|m| seen.insert(m.upstream_model_id.clone()));
    models.sort_by(|a, b| a.canonical_selection_id.cmp(&b.canonical_selection_id));
    models
}

/// Build a discovered model row for an OpenAI-family identity-only entry.
pub fn openai_discovered_model(
    identity: &CatalogAccountIdentity,
    upstream_id: &str,
    manual: &IndexMap<String, bool>,
) -> Option<DiscoveredModel> {
    let upstream = upstream_id.trim();
    if upstream.is_empty() {
        return None;
    }
    Some(DiscoveredModel {
        canonical_selection_id: canonical_selection_id(identity, upstream),
        upstream_model_id: upstream.to_owned(),
        display_name: None,
        description: None,
        context_window: None,
        max_completion_tokens: None,
        capabilities: project_openai_capabilities(manual),
        provider_instance_id: identity.instance_id.as_str().to_owned(),
        provider_kind: identity.kind,
        api_surface: identity.api_surface,
        credential_route: identity.credential_route,
        endpoint_origin: identity.endpoint_origin.clone(),
    })
}

/// Build a discovered model row from OpenRouter-rich metadata.
#[allow(clippy::too_many_arguments)]
pub fn openrouter_discovered_model(
    identity: &CatalogAccountIdentity,
    upstream_id: &str,
    display_name: Option<String>,
    description: Option<String>,
    context_window: Option<u64>,
    max_completion_tokens: Option<u32>,
    supported_parameters: &[String],
    input_modalities: &[String],
    reasoning_efforts: Vec<String>,
    default_reasoning_effort: Option<String>,
    supports_reasoning_effort: bool,
    manual: &IndexMap<String, bool>,
) -> Option<DiscoveredModel> {
    let upstream = upstream_id.trim();
    if upstream.is_empty() {
        return None;
    }
    Some(DiscoveredModel {
        canonical_selection_id: canonical_selection_id(identity, upstream),
        upstream_model_id: upstream.to_owned(),
        display_name,
        description,
        context_window: context_window.filter(|n| *n > 0),
        max_completion_tokens,
        capabilities: project_openrouter_capabilities(
            supported_parameters,
            input_modalities,
            reasoning_efforts,
            default_reasoning_effort,
            supports_reasoning_effort,
            manual,
        ),
        provider_instance_id: identity.instance_id.as_str().to_owned(),
        provider_kind: identity.kind,
        api_surface: identity.api_surface,
        credential_route: identity.credential_route,
        endpoint_origin: identity.endpoint_origin.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider_registry::{
        ApiSurface, CredentialBindingId, CredentialRoute, ProviderId, ProviderIncarnation,
        ProviderKind,
    };

    fn sample_identity(id: &str, kind: ProviderKind, built_in: bool) -> CatalogAccountIdentity {
        CatalogAccountIdentity {
            instance_id: ProviderId::new(id).unwrap(),
            kind,
            api_surface: match kind {
                ProviderKind::OpenRouter => ApiSurface::OpenRouterNative,
                _ => ApiSurface::OpenAiPlatform,
            },
            credential_route: CredentialRoute::ApiKey,
            endpoint_origin: "https://api.example.com".into(),
            org_project_fingerprint: String::new(),
            incarnation: ProviderIncarnation::new("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").unwrap(),
            credential_binding_id: CredentialBindingId::new("11111111-2222-3333-4444-555555555555")
                .unwrap(),
            is_built_in_compatibility: built_in,
        }
    }

    #[test]
    fn built_in_openai_keeps_compatibility_id() {
        let id = sample_identity("openai", ProviderKind::OpenAi, true);
        assert_eq!(canonical_selection_id(&id, "gpt-4o"), "openai:gpt-4o");
    }

    #[test]
    fn additional_account_is_instance_qualified() {
        let id = sample_identity("openai_work", ProviderKind::OpenAi, false);
        assert_eq!(canonical_selection_id(&id, "gpt-4o"), "openai_work:gpt-4o");
    }

    #[test]
    fn embeddings_not_guessed_from_empty_manual() {
        let caps = project_openai_capabilities(&IndexMap::new());
        assert_eq!(caps.supports_embeddings, None);
        assert_eq!(caps.supports_rerank, None);
    }

    #[test]
    fn manual_embeddings_authoritative() {
        let mut manual = IndexMap::new();
        manual.insert("embeddings".into(), true);
        let caps = project_openai_capabilities(&manual);
        assert_eq!(caps.supports_embeddings, Some(true));
        assert_eq!(caps.supports_rerank, None);
    }

    #[test]
    fn first_wins_dedupe_then_sort() {
        let id = sample_identity("openai_work", ProviderKind::OpenAi, false);
        let a = openai_discovered_model(&id, "b-model", &IndexMap::new()).unwrap();
        let mut a2 = a.clone();
        a2.display_name = Some("second".into());
        let c = openai_discovered_model(&id, "a-model", &IndexMap::new()).unwrap();
        let out = dedupe_and_sort_models(vec![a.clone(), c.clone(), a2]);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].upstream_model_id, "a-model");
        assert_eq!(out[1].upstream_model_id, "b-model");
        assert_eq!(out[1].display_name, None); // first wins
    }
}
