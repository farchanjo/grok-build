//! Production provider route resolution for the sampler sidecar.
//!
//! Builds a credential-free [`ProviderRouteContext`] from the authoritative
//! provider config / service descriptor and the **live** credential binding
//! generation. Does not fabricate authoritative generations or credential
//! routes from kind/hostname alone.

use std::path::Path;

use xai_grok_inference::{
    ProviderRouteContext, RouteApiSurface, RouteAuthority, RouteCredentialRoute,
    RoutePacingOverride, RouteProviderKind,
};

use crate::agent::model_providers::{
    ModelProviderConfig, ModelProviderKind, ResolvedModelProvider,
};
use crate::auth::{
    ANTHROPIC_API_KEY_SCOPE, OPENAI_API_KEY_SCOPE, OPENAI_OAUTH_SCOPE, OPENROUTER_API_KEY_SCOPE,
    lookup_auth, read_auth_json, read_provider_api_key, read_provider_api_key_binding,
    read_provider_oauth_auth, read_provider_oauth_binding,
};
use crate::provider_registry::ProviderService;
use crate::provider_registry::id::{BuiltInProviderId, ProviderId};
use crate::provider_registry::instance::{ApiSurface, CredentialRoute, ProviderIncarnation};
use crate::provider_registry::secrets::application_key_scope;

/// Inputs for one production route resolution.
#[derive(Clone, Copy)]
pub struct RouteResolutionInputs<'a> {
    pub inference: &'a xai_grok_inference::InferenceConfig,
    pub resolved: Option<&'a ResolvedModelProvider>,
    pub provider_config: Option<&'a ModelProviderConfig>,
    pub registry_generation: u64,
    pub descriptor_incarnation: Option<&'a str>,
    pub descriptor_api_surface: Option<ApiSurface>,
    pub descriptor_credential_route: Option<CredentialRoute>,
    pub grok_home: Option<&'a Path>,
}

/// Resolve the production sampler route for the selected model/provider.
pub fn resolve_production_route_context(inputs: RouteResolutionInputs<'_>) -> ProviderRouteContext {
    let Some(resolved) = inputs.resolved else {
        return ProviderRouteContext::legacy_from_config(inputs.inference);
    };

    let kind = map_kind(resolved.kind);
    let instance_id = resolved.id.as_str();
    let (api_surface, credential_route) = resolve_surface_and_route(inputs, kind, instance_id);

    let incarnation = inputs.descriptor_incarnation.filter(|s| !s.is_empty());
    let (binding_generation, authority) =
        resolve_binding_authority(inputs, instance_id, credential_route, incarnation);

    let mut builder = ProviderRouteContext::builder()
        .instance_id(instance_id)
        .provider_kind(kind)
        .api_surface(api_surface)
        .credential_route(credential_route)
        .registry_generation(inputs.registry_generation)
        .binding_generation(binding_generation)
        .credential_generation(binding_generation)
        .authority(authority)
        .origin_from_base_url(inputs.inference.base_url.as_str())
        .model_partition(inputs.inference.model.as_str());

    if let Some(inc) = incarnation {
        builder = builder.incarnation(inc);
    }

    let pacing_enabled = resolved.openrouter_pacing
        || kind.is_openrouter()
        || matches!(api_surface, RouteApiSurface::OpenRouterNative)
        || inputs.inference.openrouter_pacing;
    if pacing_enabled {
        builder = builder.pacing(RoutePacingOverride {
            enabled: Some(true),
            ..RoutePacingOverride::default()
        });
    }

    builder
        .build()
        .unwrap_or_else(|_| ProviderRouteContext::legacy_from_config(inputs.inference))
}

/// Compatibility helper: uses ModelsManager picker/default id.
/// Production sessions must use [`resolve_for_models_manager_with_selection`].
pub fn resolve_for_models_manager(
    inference: &xai_grok_inference::InferenceConfig,
    models_manager: &crate::agent::models::ModelsManager,
    grok_home: Option<&Path>,
) -> ProviderRouteContext {
    let selection = models_manager.current_model_id();
    resolve_for_models_manager_with_selection(
        inference,
        models_manager,
        selection.0.as_ref(),
        grok_home,
    )
}

/// Production route resolution from an explicit **canonical selection** id.
///
/// Catalog lookup is exact canonical first — never by upstream wire id in
/// `InferenceConfig.model`. When the selection is present in the catalog, the
/// selection's upstream wire model and provider origin are projected onto the
/// inference inputs so a parent session's wire model / base URL cannot skew
/// `model_partition`, origin, or surface selection for an explicit override.
pub fn resolve_for_models_manager_with_selection(
    inference: &xai_grok_inference::InferenceConfig,
    models_manager: &crate::agent::models::ModelsManager,
    canonical_selection_id: &str,
    grok_home: Option<&Path>,
) -> ProviderRouteContext {
    let projected = project_inference_for_canonical_selection(
        inference,
        models_manager,
        canonical_selection_id,
    );
    resolve_for_models_manager_with_selection_projected(
        &projected,
        models_manager,
        canonical_selection_id,
        grok_home,
    )
}

/// Project the catalog selection's upstream wire model and provider origin onto
/// a copy of the parent inference config. Parent session model/base_url are
/// retained only when the selection is absent from the catalog.
pub(crate) fn project_inference_for_canonical_selection(
    inference: &xai_grok_inference::InferenceConfig,
    models_manager: &crate::agent::models::ModelsManager,
    canonical_selection_id: &str,
) -> xai_grok_inference::InferenceConfig {
    let models = models_manager.models();
    let Some(entry) = models
        .get(canonical_selection_id)
        .or_else(|| crate::agent::config::find_model_by_id(&models, canonical_selection_id))
    else {
        return inference.clone();
    };

    let mut projected = inference.clone();
    match entry.upstream_wire_id() {
        Ok(upstream) => projected.model = upstream.into_string(),
        Err(_) if !entry.info().model.is_empty() => {
            projected.model = entry.info().model.clone();
        }
        Err(_) => {}
    }
    // Prefer live credential-resolved base (ChatGPT OAuth Codex vs Platform)
    // over a bare catalog URL so surface selection matches final resolve.
    let credentials = crate::agent::config::resolve_credentials(entry, None);
    if !credentials.base_url.is_empty() {
        projected.base_url = credentials.base_url;
    } else if !entry.info().base_url.is_empty() {
        projected.base_url = entry.info().base_url.clone();
    }
    projected
}

fn resolve_for_models_manager_with_selection_projected(
    inference: &xai_grok_inference::InferenceConfig,
    models_manager: &crate::agent::models::ModelsManager,
    canonical_selection_id: &str,
    grok_home: Option<&Path>,
) -> ProviderRouteContext {
    let cfg = models_manager.config_snapshot();
    let models = models_manager.models();
    let entry = models
        .get(canonical_selection_id)
        .or_else(|| crate::agent::config::find_model_by_id(&models, canonical_selection_id));
    let resolved = entry.and_then(|m| m.model_provider.clone());

    let service = ProviderService::from_model_providers(&cfg.model_providers).ok();
    let registry_generation = service.as_ref().map(|s| s.generation()).unwrap_or(0);

    let provider_id = resolved.as_ref().map(|r| r.id.clone());
    let descriptor = provider_id
        .as_deref()
        .and_then(|id| service.as_ref().and_then(|s| s.get(id)));
    let descriptor_incarnation =
        descriptor.and_then(|d| d.incarnation.as_ref().map(|i| i.as_str()));
    // Surface-aware selection: Codex vs Platform API by live base URL, not
    // primary-first (which would mis-attribute concurrent OAuth + API-key routes).
    let selected_route = descriptor.and_then(|d| select_descriptor_route(d, &inference.base_url));
    let (descriptor_api_surface, descriptor_credential_route) = selected_route
        .map(|r| (Some(r.api_surface), Some(r.credential_route)))
        .unwrap_or((None, None));

    let provider_config = provider_id
        .as_deref()
        .and_then(|id| cfg.model_providers.get(id));

    resolve_production_route_context(RouteResolutionInputs {
        inference,
        resolved: resolved.as_ref(),
        provider_config,
        registry_generation,
        descriptor_incarnation,
        descriptor_api_surface,
        descriptor_credential_route,
        grok_home,
    })
}

/// Pick the descriptor route that matches the live API surface / base URL.
///
/// Codex hosts prefer ChatGPT OAuth; `api.openai.com` prefers Platform API-key;
/// otherwise the first descriptor route is used.
fn select_descriptor_route<'a>(
    desc: &'a crate::provider_registry::instance::ProviderInstanceDescriptor,
    base_url: &str,
) -> Option<&'a crate::provider_registry::instance::ProviderRouteDescriptor> {
    if crate::auth::chatgpt_oauth::is_codex_base_url(base_url)
        && let Some(r) = desc.routes.iter().find(|r| {
            matches!(r.credential_route, CredentialRoute::ChatGptOauth)
                || matches!(r.api_surface, ApiSurface::ChatGptInference)
        })
    {
        return Some(r);
    }
    if base_url.contains("api.openai.com")
        && let Some(r) = desc.routes.iter().find(|r| {
            matches!(r.api_surface, ApiSurface::OpenAiPlatform)
                || matches!(
                    r.credential_route,
                    CredentialRoute::ApiKey | CredentialRoute::OpenAiPlatform
                )
        })
    {
        return Some(r);
    }
    desc.routes.first()
}

fn map_kind(kind: ModelProviderKind) -> RouteProviderKind {
    match kind {
        ModelProviderKind::Xai => RouteProviderKind::Xai,
        ModelProviderKind::OpenAi => RouteProviderKind::OpenAi,
        ModelProviderKind::OpenRouter => RouteProviderKind::OpenRouter,
        ModelProviderKind::Anthropic => RouteProviderKind::Anthropic,
        ModelProviderKind::OpenAiCompatible => RouteProviderKind::OpenAiCompatible,
        ModelProviderKind::Zai => RouteProviderKind::Zai,
    }
}

fn map_api_surface(surface: ApiSurface) -> RouteApiSurface {
    match surface {
        ApiSurface::OpenAiPlatform => RouteApiSurface::OpenAiPlatform,
        ApiSurface::OpenRouterNative => RouteApiSurface::OpenRouterNative,
        ApiSurface::ChatGptInference => RouteApiSurface::ChatGptInference,
        ApiSurface::OpenAiCompatibleSubset => RouteApiSurface::OpenAiCompatibleSubset,
        ApiSurface::RetrievalOnly => RouteApiSurface::RetrievalOnly,
        ApiSurface::AnthropicMessages => RouteApiSurface::AnthropicMessages,
    }
}

fn map_credential_route(route: CredentialRoute) -> RouteCredentialRoute {
    match route {
        CredentialRoute::XaiSession => RouteCredentialRoute::XaiSession,
        CredentialRoute::ApiKey => RouteCredentialRoute::ApiKey,
        CredentialRoute::OpenAiPlatform => RouteCredentialRoute::OpenAiPlatform,
        CredentialRoute::ChatGptOauth => RouteCredentialRoute::ChatGptOauth,
        CredentialRoute::AuthHelper => RouteCredentialRoute::AuthHelper,
        CredentialRoute::None => RouteCredentialRoute::None,
    }
}

fn resolve_surface_and_route(
    inputs: RouteResolutionInputs<'_>,
    kind: RouteProviderKind,
    instance_id: &str,
) -> (RouteApiSurface, RouteCredentialRoute) {
    if let (Some(surface), Some(route)) = (
        inputs.descriptor_api_surface,
        inputs.descriptor_credential_route,
    ) {
        return (map_api_surface(surface), map_credential_route(route));
    }
    if let Some(cfg) = inputs.provider_config {
        return (
            derive_api_surface(kind, inputs.inference),
            derive_credential_route(cfg, kind, instance_id, inputs.inference),
        );
    }
    (
        derive_api_surface(kind, inputs.inference),
        derive_credential_route_from_kind(kind, instance_id, inputs.inference),
    )
}

fn derive_api_surface(
    kind: RouteProviderKind,
    inference: &xai_grok_inference::InferenceConfig,
) -> RouteApiSurface {
    if crate::auth::chatgpt_oauth::is_codex_base_url(&inference.base_url) {
        return RouteApiSurface::ChatGptInference;
    }
    match kind {
        RouteProviderKind::Xai => RouteApiSurface::OpenAiCompatibleSubset,
        RouteProviderKind::OpenAi => RouteApiSurface::OpenAiPlatform,
        RouteProviderKind::OpenRouter => RouteApiSurface::OpenRouterNative,
        RouteProviderKind::Anthropic => RouteApiSurface::AnthropicMessages,
        RouteProviderKind::OpenAiCompatible
        | RouteProviderKind::Zai
        | RouteProviderKind::Custom => RouteApiSurface::OpenAiCompatibleSubset,
    }
}

fn derive_credential_route(
    cfg: &ModelProviderConfig,
    kind: RouteProviderKind,
    instance_id: &str,
    inference: &xai_grok_inference::InferenceConfig,
) -> RouteCredentialRoute {
    if cfg.auth.is_some() || cfg.auth_provider.is_some() {
        return RouteCredentialRoute::AuthHelper;
    }
    if crate::auth::chatgpt_oauth::is_codex_base_url(&inference.base_url) {
        return RouteCredentialRoute::ChatGptOauth;
    }
    derive_credential_route_from_kind(kind, instance_id, inference)
}

fn derive_credential_route_from_kind(
    kind: RouteProviderKind,
    instance_id: &str,
    inference: &xai_grok_inference::InferenceConfig,
) -> RouteCredentialRoute {
    if crate::auth::chatgpt_oauth::is_codex_base_url(&inference.base_url) {
        return RouteCredentialRoute::ChatGptOauth;
    }
    match kind {
        RouteProviderKind::Xai if instance_id == "xai" => RouteCredentialRoute::XaiSession,
        RouteProviderKind::Xai => RouteCredentialRoute::ApiKey,
        RouteProviderKind::OpenAi => RouteCredentialRoute::OpenAiPlatform,
        RouteProviderKind::OpenRouter
        | RouteProviderKind::Anthropic
        | RouteProviderKind::Zai
        | RouteProviderKind::OpenAiCompatible => RouteCredentialRoute::ApiKey,
        RouteProviderKind::Custom => RouteCredentialRoute::None,
    }
}

fn resolve_binding_authority(
    inputs: RouteResolutionInputs<'_>,
    instance_id: &str,
    credential_route: RouteCredentialRoute,
    incarnation: Option<&str>,
) -> (u64, RouteAuthority) {
    match credential_route {
        RouteCredentialRoute::None => (0, RouteAuthority::Authoritative),
        RouteCredentialRoute::AuthHelper => (0, RouteAuthority::Unverified),
        RouteCredentialRoute::XaiSession => {
            if instance_id == "xai" && incarnation.is_none() {
                (0, RouteAuthority::Authoritative)
            } else {
                (0, RouteAuthority::Unverified)
            }
        }
        RouteCredentialRoute::ChatGptOauth => {
            live_oauth_binding(inputs.grok_home, instance_id, incarnation)
        }
        RouteCredentialRoute::ApiKey | RouteCredentialRoute::OpenAiPlatform => {
            live_api_key_binding(inputs.grok_home, instance_id, credential_route, incarnation)
        }
    }
}

fn live_api_key_binding(
    grok_home: Option<&Path>,
    instance_id: &str,
    credential_route: RouteCredentialRoute,
    incarnation: Option<&str>,
) -> (u64, RouteAuthority) {
    if incarnation.is_some() {
        return (0, RouteAuthority::Unverified);
    }
    let Some(home) = grok_home else {
        return (0, RouteAuthority::Unverified);
    };
    let Some(scope) = api_key_scope_for_instance(instance_id, credential_route) else {
        return (0, RouteAuthority::Unverified);
    };
    let key_present = matches!(read_provider_api_key(home, &scope), Ok(Some(_)));
    if !key_present {
        return (0, RouteAuthority::Unverified);
    }
    match read_provider_api_key_binding(home, &scope) {
        Ok(Some(binding)) => (binding.generation, RouteAuthority::Authoritative),
        Ok(None) => (0, RouteAuthority::Authoritative),
        Err(_) => (0, RouteAuthority::Unverified),
    }
}

fn live_oauth_binding(
    grok_home: Option<&Path>,
    instance_id: &str,
    incarnation: Option<&str>,
) -> (u64, RouteAuthority) {
    let Some(home) = grok_home else {
        return (0, RouteAuthority::Unverified);
    };
    if instance_id == "openai" && incarnation.is_none() {
        let path = home.join("auth.json");
        return match read_auth_json(&path) {
            Ok(store) if lookup_auth(&store, OPENAI_OAUTH_SCOPE).is_some() => {
                let generation =
                    crate::auth::chatgpt_oauth::read_builtin_oauth_binding_generation(home)
                        .ok()
                        .flatten()
                        .unwrap_or(0);
                (generation, RouteAuthority::Authoritative)
            }
            _ => (0, RouteAuthority::Unverified),
        };
    }
    let Ok(id) = ProviderId::new(instance_id) else {
        return (0, RouteAuthority::Unverified);
    };
    let expected_inc = incarnation.and_then(|raw| ProviderIncarnation::new(raw).ok());
    match read_provider_oauth_binding(home, &id) {
        Ok(Some(binding)) if binding.incarnation == expected_inc => {
            (binding.generation, RouteAuthority::Authoritative)
        }
        Ok(None) if expected_inc.is_none() => (0, RouteAuthority::Unverified),
        _ => (0, RouteAuthority::Unverified),
    }
}

/// Re-resolve the live durable binding generation for the exact credential
/// source frozen when repair started.
pub fn live_binding_generation_for_route(
    grok_home: &Path,
    provider_id: &str,
    credential_route: &str,
    incarnation: Option<&str>,
) -> Option<u64> {
    match credential_route {
        "chatgpt_oauth" => {
            if provider_id == "openai" && incarnation.is_none() {
                let path = grok_home.join("auth.json");
                let store = read_auth_json(&path).ok()?;
                lookup_auth(&store, OPENAI_OAUTH_SCOPE)?;
                return Some(
                    crate::auth::chatgpt_oauth::read_builtin_oauth_binding_generation(grok_home)
                        .ok()
                        .flatten()
                        .unwrap_or(0),
                );
            }
            let id = ProviderId::new(provider_id).ok()?;
            let expected = incarnation.and_then(|raw| ProviderIncarnation::new(raw).ok());
            let _auth = read_provider_oauth_auth(grok_home, &id, expected.as_ref()).ok()??;
            let binding = read_provider_oauth_binding(grok_home, &id).ok()??;
            (binding.incarnation == expected).then_some(binding.generation)
        }
        "api_key" | "openai_platform" => {
            if incarnation.is_some() {
                return None;
            }
            let scope = match provider_id {
                "openai" => OPENAI_API_KEY_SCOPE.to_owned(),
                "openrouter" => OPENROUTER_API_KEY_SCOPE.to_owned(),
                "anthropic" => ANTHROPIC_API_KEY_SCOPE.to_owned(),
                id => application_key_scope(&ProviderId::new(id).ok()?),
            };
            if !matches!(read_provider_api_key(grok_home, &scope), Ok(Some(_))) {
                return None;
            }
            match read_provider_api_key_binding(grok_home, &scope) {
                Ok(Some(binding)) => Some(binding.generation),
                Ok(None) => Some(0),
                Err(_) => None,
            }
        }
        "xai_session" | "none" | "auth_helper" => None,
        _ => None,
    }
}

fn api_key_scope_for_instance(
    instance_id: &str,
    credential_route: RouteCredentialRoute,
) -> Option<String> {
    match instance_id {
        "openai"
            if matches!(
                credential_route,
                RouteCredentialRoute::ApiKey | RouteCredentialRoute::OpenAiPlatform
            ) =>
        {
            Some(OPENAI_API_KEY_SCOPE.to_owned())
        }
        "openrouter" => Some(OPENROUTER_API_KEY_SCOPE.to_owned()),
        "anthropic" => Some(ANTHROPIC_API_KEY_SCOPE.to_owned()),
        id if BuiltInProviderId::parse(id).is_none() => {
            let pid = ProviderId::new(id).ok()?;
            Some(application_key_scope(&pid))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::store_provider_api_key;
    use agent_client_protocol as acp;
    use tempfile::tempdir;
    use xai_grok_inference::InferenceConfig;

    fn cfg(base_url: &str, model: &str) -> InferenceConfig {
        InferenceConfig {
            base_url: base_url.into(),
            model: model.into(),
            provider_identity: xai_grok_inference::config::ProviderIdentity::OpenRouter,
            ..InferenceConfig::default()
        }
    }

    fn resolved(id: &str, kind: ModelProviderKind) -> ResolvedModelProvider {
        ResolvedModelProvider {
            id: id.into(),
            kind,
            openrouter_fallback_models: vec![],
            openrouter_provider_preferences: None,
            openrouter_plugins: vec![],
            openrouter_pacing: kind == ModelProviderKind::OpenRouter,
            command: vec![],
        }
    }

    fn catalog_entry(wire_model: &str, provider_id: &str) -> crate::agent::config::ModelEntry {
        crate::agent::config::ModelEntry {
            info: crate::agent::config::ModelInfo::fallback(wire_model),
            model_provider: Some(resolved(provider_id, ModelProviderKind::OpenAiCompatible)),
            api_key: None,
            env_key: None,
            auth_provider: None,
            api_base_url: None,
        }
    }

    #[test]
    fn global_picker_change_cannot_alter_other_session_route_context() {
        let dir = tempdir().unwrap();
        let mut config = crate::agent::config::Config::default();
        for provider_id in ["account-a", "account-b"] {
            config.model_providers.insert(
                provider_id.to_owned(),
                ModelProviderConfig {
                    base_url: Some(format!("https://{provider_id}.example/v1")),
                    ..ModelProviderConfig::default()
                },
            );
        }
        let mut models = indexmap::IndexMap::new();
        models.insert(
            "session-a-selection".to_owned(),
            catalog_entry("shared-wire-model", "account-a"),
        );
        models.insert(
            "session-b-selection".to_owned(),
            catalog_entry("shared-wire-model", "account-b"),
        );
        let manager = crate::agent::models::ModelsManager::new(
            None,
            models,
            acp::ModelId::new("session-a-selection"),
            std::sync::Arc::new(crate::auth::AuthManager::new(
                dir.path(),
                crate::auth::GrokComConfig::default(),
            )),
            config,
        );
        let inference = cfg("https://account-a.example/v1", "shared-wire-model");

        let session_a_before = resolve_for_models_manager_with_selection(
            &inference,
            &manager,
            "session-a-selection",
            Some(dir.path()),
        );
        let session_b = resolve_for_models_manager_with_selection(
            &inference,
            &manager,
            "session-b-selection",
            Some(dir.path()),
        );
        assert_eq!(session_a_before.instance_id(), "account-a");
        assert_eq!(session_b.instance_id(), "account-b");

        manager.set_current_model_id(acp::ModelId::new("session-b-selection"));
        assert_eq!(
            resolve_for_models_manager(&inference, &manager, Some(dir.path())).instance_id(),
            "account-b",
            "compatibility resolution must demonstrate that the shared picker moved",
        );
        let session_a_after = resolve_for_models_manager_with_selection(
            &inference,
            &manager,
            "session-a-selection",
            Some(dir.path()),
        );
        assert_eq!(session_a_after.instance_id(), "account-a");
        assert_eq!(session_a_after, session_a_before);
    }

    #[test]
    fn xai_session_built_in_is_authoritative() {
        let inference = InferenceConfig {
            base_url: "https://api.x.ai/v1".into(),
            model: "grok".into(),
            provider_identity: xai_grok_inference::config::ProviderIdentity::Xai,
            ..InferenceConfig::default()
        };
        let r = resolved("xai", ModelProviderKind::Xai);
        let ctx = resolve_production_route_context(RouteResolutionInputs {
            inference: &inference,
            resolved: Some(&r),
            provider_config: None,
            registry_generation: 0,
            descriptor_incarnation: None,
            descriptor_api_surface: None,
            descriptor_credential_route: None,
            grok_home: None,
        });
        assert_eq!(ctx.credential_route(), RouteCredentialRoute::XaiSession);
        assert_eq!(ctx.authority(), RouteAuthority::Authoritative);
    }

    #[test]
    fn missing_key_is_unverified_not_authoritative_zero() {
        let dir = tempdir().unwrap();
        let inference = cfg("https://openrouter.ai/api/v1", "m");
        let r = resolved("openrouter", ModelProviderKind::OpenRouter);
        let ctx = resolve_production_route_context(RouteResolutionInputs {
            inference: &inference,
            resolved: Some(&r),
            provider_config: None,
            registry_generation: 0,
            descriptor_incarnation: None,
            descriptor_api_surface: None,
            descriptor_credential_route: None,
            grok_home: Some(dir.path()),
        });
        assert_eq!(ctx.authority(), RouteAuthority::Unverified);
    }

    #[test]
    fn built_in_openrouter_with_stored_key_is_authoritative_api_key() {
        let dir = tempdir().unwrap();
        let generation =
            store_provider_api_key(dir.path(), OPENROUTER_API_KEY_SCOPE, "or-key-aaaaaaaa")
                .unwrap();
        assert_eq!(generation, 1);
        let inference = cfg("https://openrouter.ai/api/v1", "m");
        let r = resolved("openrouter", ModelProviderKind::OpenRouter);
        let ctx = resolve_production_route_context(RouteResolutionInputs {
            inference: &inference,
            resolved: Some(&r),
            provider_config: None,
            registry_generation: 0,
            descriptor_incarnation: None,
            descriptor_api_surface: None,
            descriptor_credential_route: None,
            grok_home: Some(dir.path()),
        });
        assert_eq!(ctx.authority(), RouteAuthority::Authoritative);
        assert_eq!(ctx.binding_generation(), 1);
        assert_eq!(
            live_binding_generation_for_route(dir.path(), "openrouter", "api_key", None),
            Some(1)
        );
    }

    #[test]
    fn clear_after_store_is_unverified_despite_retained_meta() {
        let dir = tempdir().unwrap();
        let _ =
            store_provider_api_key(dir.path(), OPENROUTER_API_KEY_SCOPE, "or-to-clear-zzzzzzzz")
                .unwrap();
        crate::auth::clear_provider_api_key(dir.path(), OPENROUTER_API_KEY_SCOPE).unwrap();
        let inference = cfg("https://openrouter.ai/api/v1", "m");
        let r = resolved("openrouter", ModelProviderKind::OpenRouter);
        let ctx = resolve_production_route_context(RouteResolutionInputs {
            inference: &inference,
            resolved: Some(&r),
            provider_config: None,
            registry_generation: 0,
            descriptor_incarnation: None,
            descriptor_api_surface: None,
            descriptor_credential_route: None,
            grok_home: Some(dir.path()),
        });
        assert_eq!(ctx.authority(), RouteAuthority::Unverified);
        assert!(
            live_binding_generation_for_route(dir.path(), "openrouter", "api_key", None).is_none()
        );
    }

    #[test]
    fn auth_helper_is_unverified_not_host_fallback() {
        assert_eq!(
            resolve_binding_authority(
                RouteResolutionInputs {
                    inference: &cfg("https://example.com", "m"),
                    resolved: None,
                    provider_config: None,
                    registry_generation: 0,
                    descriptor_incarnation: None,
                    descriptor_api_surface: None,
                    descriptor_credential_route: None,
                    grok_home: None,
                },
                "helper",
                RouteCredentialRoute::AuthHelper,
                None,
            ),
            (0, RouteAuthority::Unverified)
        );
    }

    #[test]
    fn no_auth_route_is_authoritative_with_none_credential() {
        assert_eq!(
            resolve_binding_authority(
                RouteResolutionInputs {
                    inference: &cfg("https://example.com", "m"),
                    resolved: None,
                    provider_config: None,
                    registry_generation: 0,
                    descriptor_incarnation: None,
                    descriptor_api_surface: None,
                    descriptor_credential_route: None,
                    grok_home: None,
                },
                "public",
                RouteCredentialRoute::None,
                None,
            ),
            (0, RouteAuthority::Authoritative)
        );
    }

    fn entry_on_provider(
        wire_model: &str,
        provider_id: &str,
        kind: ModelProviderKind,
        base_url: &str,
    ) -> crate::agent::config::ModelEntry {
        let mut entry = catalog_entry(wire_model, provider_id);
        entry.info.base_url = base_url.to_owned();
        entry.model_provider = Some(resolved(provider_id, kind));
        entry
    }

    /// F-PR5-2: parent session wire model/base_url must not skew mint-time
    /// partition/origin for an explicit child selection.
    #[test]
    fn selection_projects_child_upstream_and_origin_not_parent_wire_model() {
        let dir = tempdir().unwrap();
        let mut config = crate::agent::config::Config::default();
        config.model_providers.insert(
            "openrouter".to_owned(),
            ModelProviderConfig {
                kind: ModelProviderKind::OpenRouter,
                base_url: Some("https://openrouter.ai/api/v1".into()),
                ..ModelProviderConfig::default()
            },
        );
        let mut models = indexmap::IndexMap::new();
        models.insert(
            "openrouter:gpt-4o".to_owned(),
            entry_on_provider(
                "gpt-4o",
                "openrouter",
                ModelProviderKind::OpenRouter,
                "https://openrouter.ai/api/v1",
            ),
        );
        let manager = crate::agent::models::ModelsManager::new(
            None,
            models,
            acp::ModelId::new("openrouter:gpt-4o"),
            std::sync::Arc::new(crate::auth::AuthManager::new(
                dir.path(),
                crate::auth::GrokComConfig::default(),
            )),
            config,
        );
        // Parent is an xAI session route; child override is OpenRouter.
        let parent = InferenceConfig {
            base_url: "https://api.x.ai/v1".into(),
            model: "grok-3".into(),
            provider_identity: xai_grok_inference::config::ProviderIdentity::Xai,
            ..InferenceConfig::default()
        };
        let projected =
            project_inference_for_canonical_selection(&parent, &manager, "openrouter:gpt-4o");
        assert_eq!(projected.model, "gpt-4o");
        assert_eq!(projected.base_url, "https://openrouter.ai/api/v1");
        assert_ne!(projected.model, parent.model);
        assert_ne!(projected.base_url, parent.base_url);

        let ctx = resolve_for_models_manager_with_selection(
            &parent,
            &manager,
            "openrouter:gpt-4o",
            Some(dir.path()),
        );
        assert_eq!(ctx.instance_id(), "openrouter");
        assert_eq!(ctx.model_partition(), Some("gpt-4o"));
        assert_eq!(ctx.provider_kind(), RouteProviderKind::OpenRouter);
        // ExactRoute construction requires partition == upstream.
        let canonical = xai_grok_models::CanonicalModelId::new("openrouter:gpt-4o").unwrap();
        let upstream = xai_grok_models::UpstreamModelId::new("gpt-4o").unwrap();
        assert!(
            crate::agent::subagent::exact_route::ExactRoute::new(canonical, upstream, ctx)
                .is_some(),
            "mint must accept an override whose upstream differs from the parent wire model"
        );
    }

    /// F-PR5-1: omitting grok_home zeros binding generation / Unverified;
    /// isolated auth home preserves non-zero generation through re-resolve.
    #[test]
    fn goal_byok_binding_generation_requires_grok_home_and_matches_final() {
        let dir = tempdir().unwrap();
        let generation =
            store_provider_api_key(dir.path(), OPENROUTER_API_KEY_SCOPE, "or-goal-key-AAAAAAAA")
                .unwrap();
        assert!(generation >= 1);

        let mut config = crate::agent::config::Config::default();
        config.model_providers.insert(
            "openrouter".to_owned(),
            ModelProviderConfig {
                kind: ModelProviderKind::OpenRouter,
                base_url: Some("https://openrouter.ai/api/v1".into()),
                ..ModelProviderConfig::default()
            },
        );
        let mut models = indexmap::IndexMap::new();
        models.insert(
            "openrouter:gpt-4o".to_owned(),
            entry_on_provider(
                "gpt-4o",
                "openrouter",
                ModelProviderKind::OpenRouter,
                "https://openrouter.ai/api/v1",
            ),
        );
        let manager = crate::agent::models::ModelsManager::new(
            None,
            models,
            acp::ModelId::new("openrouter:gpt-4o"),
            std::sync::Arc::new(crate::auth::AuthManager::new(
                dir.path(),
                crate::auth::GrokComConfig::default(),
            )),
            config,
        );
        let parent = InferenceConfig {
            base_url: "https://api.x.ai/v1".into(),
            model: "grok-3".into(),
            provider_identity: xai_grok_inference::config::ProviderIdentity::Xai,
            ..InferenceConfig::default()
        };

        let without_home =
            resolve_for_models_manager_with_selection(&parent, &manager, "openrouter:gpt-4o", None);
        assert_eq!(without_home.binding_generation(), 0);
        assert_eq!(
            without_home.authority(),
            RouteAuthority::Unverified,
            "missing grok_home cannot prove BYOK binding (got credential {:?})",
            without_home.credential_route()
        );

        let mint = resolve_for_models_manager_with_selection(
            &parent,
            &manager,
            "openrouter:gpt-4o",
            Some(dir.path()),
        );
        assert_eq!(mint.credential_route(), RouteCredentialRoute::ApiKey);
        assert_eq!(mint.binding_generation(), generation);
        assert_eq!(mint.authority(), RouteAuthority::Authoritative);

        // Final handle_request re-resolve uses the same isolated home + selection.
        let final_live = resolve_for_models_manager_with_selection(
            &project_inference_for_canonical_selection(&parent, &manager, "openrouter:gpt-4o"),
            &manager,
            "openrouter:gpt-4o",
            Some(dir.path()),
        );
        assert_eq!(mint, final_live);

        let canonical = xai_grok_models::CanonicalModelId::new("openrouter:gpt-4o").unwrap();
        let upstream = xai_grok_models::UpstreamModelId::new("gpt-4o").unwrap();
        let mint_route = crate::agent::subagent::exact_route::ExactRoute::new(
            canonical.clone(),
            upstream.clone(),
            mint,
        )
        .expect("mint route");
        let final_route =
            crate::agent::subagent::exact_route::ExactRoute::new(canonical, upstream, final_live)
                .expect("final route");
        assert!(
            mint_route.matches_live(&final_route),
            "goal mint with grok_home must survive final matches_live"
        );
        assert!(
            !crate::agent::subagent::exact_route::ExactRoute::new(
                xai_grok_models::CanonicalModelId::new("openrouter:gpt-4o").unwrap(),
                xai_grok_models::UpstreamModelId::new("gpt-4o").unwrap(),
                without_home,
            )
            .unwrap()
            .matches_live(&final_route),
            "omitting grok_home must not silently match an authoritative final route"
        );
    }

    /// Explicit multi-account override must not borrow a sibling instance or
    /// its base origin when minting durable exact routes.
    #[test]
    fn explicit_override_rejects_sibling_account_borrow_at_mint() {
        let dir = tempdir().unwrap();
        let mut config = crate::agent::config::Config::default();
        for (id, host) in [
            ("work-openai", "https://work.openai.example/v1"),
            ("home-openai", "https://home.openai.example/v1"),
        ] {
            config.model_providers.insert(
                id.to_owned(),
                ModelProviderConfig {
                    kind: ModelProviderKind::OpenAi,
                    base_url: Some(host.into()),
                    ..ModelProviderConfig::default()
                },
            );
        }
        let mut models = indexmap::IndexMap::new();
        models.insert(
            "work-openai:gpt-4o".to_owned(),
            entry_on_provider(
                "gpt-4o",
                "work-openai",
                ModelProviderKind::OpenAi,
                "https://work.openai.example/v1",
            ),
        );
        models.insert(
            "home-openai:gpt-4o".to_owned(),
            entry_on_provider(
                "gpt-4o",
                "home-openai",
                ModelProviderKind::OpenAi,
                "https://home.openai.example/v1",
            ),
        );
        let manager = crate::agent::models::ModelsManager::new(
            None,
            models,
            acp::ModelId::new("work-openai:gpt-4o"),
            std::sync::Arc::new(crate::auth::AuthManager::new(
                dir.path(),
                crate::auth::GrokComConfig::default(),
            )),
            config,
        );
        // Parent session is parked on home; workflow/goal override selects work.
        let parent_home = InferenceConfig {
            base_url: "https://home.openai.example/v1".into(),
            model: "gpt-4o".into(),
            provider_identity: xai_grok_inference::config::ProviderIdentity::OpenAi,
            ..InferenceConfig::default()
        };
        let work = resolve_for_models_manager_with_selection(
            &parent_home,
            &manager,
            "work-openai:gpt-4o",
            Some(dir.path()),
        );
        let home = resolve_for_models_manager_with_selection(
            &parent_home,
            &manager,
            "home-openai:gpt-4o",
            Some(dir.path()),
        );
        assert_eq!(work.instance_id(), "work-openai");
        assert_eq!(home.instance_id(), "home-openai");
        assert_ne!(work.instance_id(), home.instance_id());

        let work_route = crate::agent::subagent::exact_route::ExactRoute::new(
            xai_grok_models::CanonicalModelId::new("work-openai:gpt-4o").unwrap(),
            xai_grok_models::UpstreamModelId::new("gpt-4o").unwrap(),
            work,
        )
        .unwrap();
        let home_route = crate::agent::subagent::exact_route::ExactRoute::new(
            xai_grok_models::CanonicalModelId::new("home-openai:gpt-4o").unwrap(),
            xai_grok_models::UpstreamModelId::new("gpt-4o").unwrap(),
            home,
        )
        .unwrap();
        assert!(
            !work_route.matches_live(&home_route),
            "sibling account routes must not match_live each other"
        );
    }
}
