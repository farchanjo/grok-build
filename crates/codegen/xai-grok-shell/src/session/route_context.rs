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
use crate::provider_registry::id::{BuiltInProviderId, ProviderId, canonical_descriptor_id};
use crate::provider_registry::instance::{ApiSurface, CredentialRoute, ProviderIncarnation};
use crate::provider_registry::secrets::{
    application_key_scope, application_key_scope_for_kind, extra_openrouter_application_key_scope,
};

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
    let instance_id = canonical_descriptor_id(resolved.id.as_str());
    let (api_surface, credential_route) = resolve_surface_and_route(inputs, kind, instance_id);

    let incarnation = inputs.descriptor_incarnation.filter(|s| !s.is_empty());
    let (binding_generation, authority) =
        resolve_binding_authority(inputs, instance_id, credential_route, incarnation, kind);

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
) -> Result<ProviderRouteContext, crate::provider_registry::route_guard::RouteGuardError> {
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
/// Returns `Err` when a configured provider is disabled, tombstoned,
/// incarnation-mismatched, or lifecycle-corrupt. True legacy (no configured
/// provider instance / no `grok_home`) still yields a non-authoritative context.
pub fn resolve_for_models_manager_with_selection(
    inference: &xai_grok_inference::InferenceConfig,
    models_manager: &crate::agent::models::ModelsManager,
    canonical_selection_id: &str,
    grok_home: Option<&Path>,
) -> Result<ProviderRouteContext, crate::provider_registry::route_guard::RouteGuardError> {
    resolve_for_models_manager_with_selection_opts(
        inference,
        models_manager,
        canonical_selection_id,
        grok_home,
        false,
    )
}

/// Same as [`resolve_for_models_manager_with_selection`] with explicit retry flag.
pub fn resolve_for_models_manager_with_selection_opts(
    inference: &xai_grok_inference::InferenceConfig,
    models_manager: &crate::agent::models::ModelsManager,
    canonical_selection_id: &str,
    grok_home: Option<&Path>,
    is_retry: bool,
) -> Result<ProviderRouteContext, crate::provider_registry::route_guard::RouteGuardError> {
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
        is_retry,
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
    let credentials = crate::agent::config::resolve_credentials(entry, None);
    if !credentials.base_url.is_empty() {
        projected.base_url = credentials.base_url;
    } else if !entry.info().base_url.is_empty() {
        projected.base_url = entry.info().base_url.clone();
    }
    // Project the entry's provider origin too. The legacy route path (entries
    // without a `model_provider`) derives `provider_kind` from the config's
    // `provider_identity`, so a stale session-spawn snapshot — captured while
    // the session ran a foreign provider — would otherwise mint a foreign
    // route for a first-party selection and fail the exact-route drift check
    // that re-validates against the child's fresh entry-derived config.
    projected.provider_identity = crate::agent::config::provider_identity_for_model(entry);
    projected
}

fn resolve_for_models_manager_with_selection_projected(
    inference: &xai_grok_inference::InferenceConfig,
    models_manager: &crate::agent::models::ModelsManager,
    canonical_selection_id: &str,
    grok_home: Option<&Path>,
    is_retry: bool,
) -> Result<ProviderRouteContext, crate::provider_registry::route_guard::RouteGuardError> {
    let cfg = models_manager.config_snapshot();
    let models = models_manager.models();
    let entry = models
        .get(canonical_selection_id)
        .or_else(|| crate::agent::config::find_model_by_id(&models, canonical_selection_id));
    let mut resolved = entry.and_then(|m| m.model_provider.clone());
    if let Some(provider) = resolved.as_mut() {
        let canonical = canonical_descriptor_id(&provider.id);
        if canonical != provider.id {
            provider.id = canonical.to_owned();
        }
    }

    let (service, registry_generation) = if let Some(home) = grok_home {
        match crate::provider_registry::runtime_cache::load_runtime(home) {
            Ok((svc, _life, registry_gen)) => (Some(svc), registry_gen),
            Err(_) if resolved.is_some() => {
                return Err(
                    crate::provider_registry::route_guard::RouteGuardError::LifecycleCorrupt {
                        id: resolved
                            .as_ref()
                            .map(|r| r.id.clone())
                            .unwrap_or_else(|| "unknown".into()),
                    },
                );
            }
            Err(_) => (None, 0),
        }
    } else {
        (
            ProviderService::from_model_providers(&cfg.model_providers).ok(),
            0,
        )
    };

    let provider_id = resolved.as_ref().map(|r| r.id.clone());
    let descriptor = provider_id
        .as_deref()
        .and_then(|id| service.as_ref().and_then(|s| s.get(id)));
    let descriptor_incarnation =
        descriptor.and_then(|d| d.incarnation.as_ref().map(|i| i.as_str()));
    let selected_route = descriptor.and_then(|d| select_descriptor_route(d, &inference.base_url));
    let (descriptor_api_surface, descriptor_credential_route) = selected_route
        .map(|r| (Some(r.api_surface), Some(r.credential_route)))
        .unwrap_or((None, None));
    let provider_config = provider_id
        .as_deref()
        .and_then(|id| cfg.model_providers.get(id));

    if let (Some(home), Some(svc), Some(pid)) =
        (grok_home, service.as_ref(), provider_id.as_deref())
    {
        use crate::provider_registry::route_guard::{RouteGuardRequest, assert_route_usable};
        let guard = RouteGuardRequest {
            provider_instance_id: pid,
            provenance_incarnation: descriptor_incarnation,
            session_registry_generation: Some(registry_generation).filter(|g| *g != 0),
            is_retry,
        };
        assert_route_usable(home, svc, &guard)?;
    }

    // True legacy only when no configured provider instance is selected.
    if resolved.is_none() {
        return Ok(ProviderRouteContext::legacy_from_config(inference));
    }

    Ok(resolve_production_route_context(RouteResolutionInputs {
        inference,
        resolved: resolved.as_ref(),
        provider_config,
        registry_generation,
        descriptor_incarnation,
        descriptor_api_surface,
        descriptor_credential_route,
        grok_home,
    }))
}

/// Explicit next-request / retry boundary using the cached runtime snapshot.
pub fn assert_live_route_usable(
    home: &Path,
    route: &ProviderRouteContext,
    is_retry: bool,
) -> Result<(), crate::provider_registry::route_guard::RouteGuardError> {
    use crate::provider_registry::route_guard::{RouteGuardRequest, assert_route_usable};
    let (service, _life, _gen) = crate::provider_registry::runtime_cache::load_runtime(home)
        .map_err(
            |_| crate::provider_registry::route_guard::RouteGuardError::LifecycleCorrupt {
                id: route.instance_id().to_owned(),
            },
        )?;
    let req = RouteGuardRequest {
        provider_instance_id: route.instance_id(),
        provenance_incarnation: route.incarnation(),
        session_registry_generation: Some(route.registry_generation()).filter(|g| *g != 0),
        is_retry,
    };
    assert_route_usable(home, &service, &req)
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
    kind: RouteProviderKind,
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
            live_api_key_binding(
                inputs.grok_home,
                instance_id,
                credential_route,
                incarnation,
                kind,
            )
        }
    }
}

fn live_api_key_binding(
    grok_home: Option<&Path>,
    instance_id: &str,
    credential_route: RouteCredentialRoute,
    incarnation: Option<&str>,
    kind: RouteProviderKind,
) -> (u64, RouteAuthority) {
    // Lifecycle incarnation is route identity, not an API-key vault key.
    // Built-in providers always attach a stable incarnation via
    // `with_lifecycle_incarnations`; skipping the live generation there
    // froze routes at generation 0 while the vault sat at gen N, so the
    // exact-route resolver fail-closed and stripped Authorization.
    let _ = incarnation;
    let Some(home) = grok_home else {
        return (0, RouteAuthority::Unverified);
    };
    let Some(scope) = api_key_scope_for_instance(instance_id, credential_route, Some(kind)) else {
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
    if crate::provider_registry::lifecycle_state::is_absent_or_stable_builtin_incarnation(
        instance_id,
        incarnation,
    ) && instance_id == "openai"
    {
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
            if provider_id == "openai"
                && crate::provider_registry::lifecycle_state::is_absent_or_stable_builtin_incarnation(
                    provider_id,
                    incarnation,
                )
            {
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
            // Same contract as `lookup_route_credential`: API-key vaults are
            // scoped by provider id. A lifecycle incarnation on the route
            // must not hide the live binding generation.
            let _ = incarnation;
            let scope = match provider_id {
                "openai" => OPENAI_API_KEY_SCOPE.to_owned(),
                "openrouter" => OPENROUTER_API_KEY_SCOPE.to_owned(),
                "anthropic" => ANTHROPIC_API_KEY_SCOPE.to_owned(),
                id => {
                    let pid = ProviderId::new(id).ok()?;
                    let kind = configured_kind_from_home(grok_home, id)
                        .unwrap_or(crate::provider_registry::ProviderKind::OpenAiCompatible);
                    application_key_scope_for_kind(&pid, kind)
                }
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
    kind: Option<RouteProviderKind>,
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
            if matches!(kind, Some(RouteProviderKind::OpenRouter)) {
                Some(extra_openrouter_application_key_scope(&pid))
            } else {
                Some(application_key_scope(&pid))
            }
        }
        _ => None,
    }
}

fn configured_kind_from_home(
    grok_home: &Path,
    id: &str,
) -> Option<crate::provider_registry::ProviderKind> {
    let raw = std::fs::read_to_string(grok_home.join("config.toml")).ok()?;
    let val: toml::Value = toml::from_str(&raw).ok()?;
    let (entries, _) = crate::agent::model_providers::parse_model_providers(&val);
    entries
        .get(id)
        .map(|c| crate::provider_registry::ProviderKind::from(c.kind))
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
            max_completion_tokens: None,
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
        // Materialize the configured providers into the home: with a
        // `grok_home`, the PR13 home-authoritative service loads from the
        // home's config.toml and never consults the in-memory manager config.
        {
            use crate::provider_registry::management::ProviderManagementService;
            use crate::provider_registry::management::dto::ProviderAddRequest;
            use crate::provider_registry::runtime_cache;
            runtime_cache::invalidate_for_home(dir.path());
            let svc = ProviderManagementService::new(dir.path());
            for provider_id in ["account-a", "account-b"] {
                let expected = svc.current_generation();
                assert!(
                    svc.add(ProviderAddRequest {
                        id: provider_id.into(),
                        kind: "openai_compatible".into(),
                        base_url: format!("https://{provider_id}.example/v1"),
                        display_name: None,
                        admin_base_url: None,
                        enabled: true,
                        expected_generation: expected,
                    })
                    .ok
                );
            }
            runtime_cache::invalidate_for_home(dir.path());
        }
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
        )
        .expect("provider route resolve");
        let session_b = resolve_for_models_manager_with_selection(
            &inference,
            &manager,
            "session-b-selection",
            Some(dir.path()),
        )
        .expect("provider route resolve");
        assert_eq!(session_a_before.instance_id(), "account-a");
        assert_eq!(session_b.instance_id(), "account-b");

        manager.set_current_model_id(acp::ModelId::new("session-b-selection"));
        assert_eq!(
            resolve_for_models_manager(&inference, &manager, Some(dir.path()))
                .expect("provider route resolve")
                .instance_id(),
            "account-b",
            "compatibility resolution must demonstrate that the shared picker moved",
        );
        let session_a_after = resolve_for_models_manager_with_selection(
            &inference,
            &manager,
            "session-a-selection",
            Some(dir.path()),
        )
        .expect("provider route resolve");
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
    fn extra_openrouter_work_key_is_authoritative_and_isolated_from_builtin() {
        use crate::provider_registry::management::ProviderManagementService;
        use crate::provider_registry::management::dto::ProviderAddRequest;
        use crate::provider_registry::runtime_cache;
        use crate::provider_registry::secrets::extra_openrouter_application_key_scope;

        let dir = tempdir().unwrap();
        runtime_cache::invalidate_for_home(dir.path());
        let svc = ProviderManagementService::new(dir.path());
        let add = svc.add(ProviderAddRequest {
            id: "openrouter-work".into(),
            kind: "openrouter".into(),
            base_url: "https://openrouter.ai/api/v1".into(),
            display_name: Some("Work".into()),
            admin_base_url: None,
            enabled: true,
            expected_generation: svc.current_generation(),
        });
        assert!(add.ok, "{:?}", add.error);
        let work = crate::provider_registry::ProviderId::new("openrouter-work").unwrap();
        let generation = store_provider_api_key(
            dir.path(),
            &extra_openrouter_application_key_scope(&work),
            "work-or-key-aaaaaaaa",
        )
        .unwrap();
        let _ = store_provider_api_key(dir.path(), OPENROUTER_API_KEY_SCOPE, "builtin-or-key");

        let inference = cfg("https://openrouter.ai/api/v1", "vendor/model");
        let r = resolved("openrouter-work", ModelProviderKind::OpenRouter);
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
        assert_eq!(ctx.instance_id(), "openrouter-work");
        assert_eq!(ctx.provider_kind(), RouteProviderKind::OpenRouter);
        assert_eq!(ctx.authority(), RouteAuthority::Authoritative);
        assert_eq!(ctx.binding_generation(), generation);
        assert_eq!(
            live_binding_generation_for_route(dir.path(), "openrouter-work", "api_key", None),
            Some(generation)
        );
        assert_eq!(
            live_binding_generation_for_route(dir.path(), "openrouter", "api_key", None),
            Some(1),
            "built-in OpenRouter keeps its own scope generation"
        );
        crate::auth::clear_provider_api_key(
            dir.path(),
            &extra_openrouter_application_key_scope(&work),
        )
        .unwrap();
        assert!(
            live_binding_generation_for_route(dir.path(), "openrouter-work", "api_key", None)
                .is_none(),
            "cleared Work key must fail closed"
        );
        assert_eq!(
            live_binding_generation_for_route(dir.path(), "openrouter", "api_key", None),
            Some(1),
            "clearing Work must not drop built-in OpenRouter"
        );
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
                RouteProviderKind::Custom,
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
                RouteProviderKind::Custom,
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
        )
        .expect("provider route resolve");
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

    /// Regression: a first-party selection (no `model_provider`) resolves
    /// through the legacy route path, which derives `provider_kind` from the
    /// config's `provider_identity`. A stale session-spawn snapshot captured
    /// while the session ran a foreign provider must not mint a foreign route:
    /// the projection normalizes the identity from the catalog entry so the
    /// workflow host's assigned route matches the child's fresh
    /// entry-derived config (exact-route drift check).
    #[test]
    fn selection_projection_normalizes_provider_identity_for_first_party_entries() {
        let dir = tempdir().unwrap();
        let config = crate::agent::config::Config::default();
        let mut models = indexmap::IndexMap::new();
        let mut first_party = catalog_entry("grok-4.5", "xai");
        first_party.model_provider = None;
        first_party.info.base_url = "https://cli-chat-proxy.grok.com/v1".to_owned();
        models.insert("grok-4.5".to_owned(), first_party);
        let manager = crate::agent::models::ModelsManager::new(
            None,
            models,
            acp::ModelId::new("grok-4.5"),
            std::sync::Arc::new(crate::auth::AuthManager::new(
                dir.path(),
                crate::auth::GrokComConfig::default(),
            )),
            config,
        );
        // Stale snapshot captured while the session ran a ChatGPT route.
        let stale = InferenceConfig {
            base_url: "https://chatgpt.com/backend-api/codex".into(),
            model: "chatgpt-gpt-5.6-sol".into(),
            provider_identity: xai_grok_inference::config::ProviderIdentity::Custom,
            ..InferenceConfig::default()
        };
        // Fresh entry-derived config the child will build.
        let fresh = InferenceConfig {
            base_url: "https://cli-chat-proxy.grok.com/v1".into(),
            model: "grok-4.5".into(),
            provider_identity: xai_grok_inference::config::ProviderIdentity::Xai,
            ..InferenceConfig::default()
        };

        let projected = project_inference_for_canonical_selection(&stale, &manager, "grok-4.5");
        assert_eq!(
            projected.provider_identity,
            xai_grok_inference::config::ProviderIdentity::Xai,
            "projection must adopt the entry's provider origin"
        );

        let host_route = resolve_for_models_manager_with_selection(
            &stale,
            &manager,
            "grok-4.5",
            Some(dir.path()),
        )
        .expect("host route resolve");
        let child_route = resolve_for_models_manager_with_selection(
            &fresh,
            &manager,
            "grok-4.5",
            Some(dir.path()),
        )
        .expect("child route resolve");
        assert_eq!(
            host_route, child_route,
            "stale and fresh configs must project to the same route for one selection"
        );
        assert_eq!(host_route.provider_kind(), RouteProviderKind::Xai);
        assert_eq!(host_route.instance_id(), "xai");
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
            resolve_for_models_manager_with_selection(&parent, &manager, "openrouter:gpt-4o", None)
                .expect("provider route resolve");
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
        )
        .expect("provider route resolve");
        assert_eq!(mint.credential_route(), RouteCredentialRoute::ApiKey);
        assert_eq!(mint.binding_generation(), generation);
        assert_eq!(mint.authority(), RouteAuthority::Authoritative);

        // Final handle_request re-resolve uses the same isolated home + selection.
        let final_live = resolve_for_models_manager_with_selection(
            &project_inference_for_canonical_selection(&parent, &manager, "openrouter:gpt-4o"),
            &manager,
            "openrouter:gpt-4o",
            Some(dir.path()),
        )
        .expect("provider route resolve");
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
        // Materialize the sibling accounts into the home: with a `grok_home`,
        // the PR13 home-authoritative service loads from the home's
        // config.toml and never consults the in-memory manager config.
        {
            use crate::provider_registry::management::ProviderManagementService;
            use crate::provider_registry::management::dto::ProviderAddRequest;
            use crate::provider_registry::runtime_cache;
            runtime_cache::invalidate_for_home(dir.path());
            let svc = ProviderManagementService::new(dir.path());
            for (id, host) in [
                ("work-openai", "https://work.openai.example/v1"),
                ("home-openai", "https://home.openai.example/v1"),
            ] {
                let expected = svc.current_generation();
                assert!(
                    svc.add(ProviderAddRequest {
                        id: id.into(),
                        kind: "openai".into(),
                        base_url: host.into(),
                        display_name: None,
                        admin_base_url: None,
                        enabled: true,
                        expected_generation: expected,
                    })
                    .ok
                );
            }
            runtime_cache::invalidate_for_home(dir.path());
        }
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
        )
        .expect("provider route resolve");
        let home = resolve_for_models_manager_with_selection(
            &parent_home,
            &manager,
            "home-openai:gpt-4o",
            Some(dir.path()),
        )
        .expect("provider route resolve");
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

    /// PR13: enable → resolve ok → disable → next-turn assert / resolve fails closed
    /// (no legacy remask into kind identity).
    #[test]
    fn enable_then_disable_blocks_next_turn_resolve_without_legacy_remask() {
        use crate::provider_registry::management::ProviderManagementService;
        use crate::provider_registry::management::dto::ProviderAddRequest;
        use crate::provider_registry::runtime_cache;

        let dir = tempdir().unwrap();
        runtime_cache::invalidate_for_home(dir.path());
        let svc = ProviderManagementService::new(dir.path());
        let g0 = svc.current_generation();
        assert!(
            svc.add(ProviderAddRequest {
                id: "lab".into(),
                kind: "openai_compatible".into(),
                base_url: "http://127.0.0.1:9/v1".into(),
                display_name: None,
                admin_base_url: None,
                enabled: true,
                expected_generation: g0,
            })
            .ok
        );
        runtime_cache::invalidate_for_home(dir.path());

        let mut config = crate::agent::config::Config::default();
        config.model_providers.insert(
            "lab".to_owned(),
            ModelProviderConfig {
                kind: ModelProviderKind::OpenAiCompatible,
                base_url: Some("http://127.0.0.1:9/v1".into()),
                enabled: true,
                ..ModelProviderConfig::default()
            },
        );
        let mut models = indexmap::IndexMap::new();
        models.insert(
            "lab:gpt".to_owned(),
            entry_on_provider(
                "gpt",
                "lab",
                ModelProviderKind::OpenAiCompatible,
                "http://127.0.0.1:9/v1",
            ),
        );
        let manager = crate::agent::models::ModelsManager::new(
            None,
            models,
            acp::ModelId::new("lab:gpt"),
            std::sync::Arc::new(crate::auth::AuthManager::new(
                dir.path(),
                crate::auth::GrokComConfig::default(),
            )),
            config,
        );
        let inference = InferenceConfig {
            base_url: "http://127.0.0.1:9/v1".into(),
            model: "gpt".into(),
            provider_identity: xai_grok_inference::config::ProviderIdentity::Custom,
            ..InferenceConfig::default()
        };

        let ok = resolve_for_models_manager_with_selection(
            &inference,
            &manager,
            "lab:gpt",
            Some(dir.path()),
        )
        .expect("enabled route must resolve");
        assert_eq!(ok.instance_id(), "lab");
        assert_live_route_usable(dir.path(), &ok, false).expect("enabled live assert");

        // Disable → next turn must fail closed (not remask to legacy kind id).
        assert!(svc.set_enabled("lab", false, svc.current_generation()).ok);
        runtime_cache::invalidate_for_home(dir.path());

        let err = resolve_for_models_manager_with_selection(
            &inference,
            &manager,
            "lab:gpt",
            Some(dir.path()),
        )
        .expect_err("disabled route must not resolve to legacy");
        assert!(
            err.to_string().contains("disabled") || err.to_string().contains("lab"),
            "guard error must identify disabled provider, got {err}"
        );
        // Retry boundary also fails closed.
        let err_retry = assert_live_route_usable(dir.path(), &ok, true).expect_err("retry");
        assert!(
            err_retry.to_string().contains("disabled") || err_retry.to_string().contains("lab"),
            "retry assert must fail closed, got {err_retry}"
        );
    }

    /// Controlled error mapping for workflow/model-switch style callers:
    /// guard failure becomes typed Err, never panic.
    #[test]
    fn route_guard_err_maps_to_controlled_caller_errors() {
        use crate::provider_registry::management::ProviderManagementService;
        use crate::provider_registry::management::dto::ProviderAddRequest;
        use crate::provider_registry::runtime_cache;

        let dir = tempdir().unwrap();
        runtime_cache::invalidate_for_home(dir.path());
        let svc = ProviderManagementService::new(dir.path());
        let g0 = svc.current_generation();
        assert!(
            svc.add(ProviderAddRequest {
                id: "lab".into(),
                kind: "openai_compatible".into(),
                base_url: "http://127.0.0.1:9/v1".into(),
                display_name: None,
                admin_base_url: None,
                enabled: true,
                expected_generation: g0,
            })
            .ok
        );
        assert!(svc.set_enabled("lab", false, svc.current_generation()).ok);
        runtime_cache::invalidate_for_home(dir.path());

        let mut config = crate::agent::config::Config::default();
        config.model_providers.insert(
            "lab".to_owned(),
            ModelProviderConfig {
                kind: ModelProviderKind::OpenAiCompatible,
                base_url: Some("http://127.0.0.1:9/v1".into()),
                enabled: false,
                ..ModelProviderConfig::default()
            },
        );
        let mut models = indexmap::IndexMap::new();
        models.insert(
            "lab:gpt".to_owned(),
            entry_on_provider(
                "gpt",
                "lab",
                ModelProviderKind::OpenAiCompatible,
                "http://127.0.0.1:9/v1",
            ),
        );
        let manager = crate::agent::models::ModelsManager::new(
            None,
            models,
            acp::ModelId::new("lab:gpt"),
            std::sync::Arc::new(crate::auth::AuthManager::new(
                dir.path(),
                crate::auth::GrokComConfig::default(),
            )),
            config,
        );
        let inference = InferenceConfig {
            base_url: "http://127.0.0.1:9/v1".into(),
            model: "gpt".into(),
            provider_identity: xai_grok_inference::config::ProviderIdentity::Custom,
            ..InferenceConfig::default()
        };
        let err = resolve_for_models_manager_with_selection(
            &inference,
            &manager,
            "lab:gpt",
            Some(dir.path()),
        )
        .expect_err("disabled must be Err");
        // Workflow host mapping.
        let host_msg = format!("provider route unusable for workflow model: {err}");
        assert!(host_msg.contains("lab") || host_msg.contains("disabled"));
        // Model-switch mapping.
        let switch_msg = format!("provider route unusable for model switch: {err}");
        assert!(switch_msg.contains("lab") || switch_msg.contains("disabled"));
        // Assigned spawn mapping.
        let spawn_msg = format!("provider route unusable for assigned spawn: {err}");
        assert!(spawn_msg.contains("lab") || spawn_msg.contains("disabled"));
    }

    /// Catalog presets used to stamp `grok_build_openrouter`. That id is a
    /// config source, not a descriptor; resolve must use canonical `openrouter`.
    #[test]
    fn grok_build_openrouter_alias_resolves_to_canonical_openrouter() {
        use crate::provider_registry::runtime_cache;

        let dir = tempdir().unwrap();
        runtime_cache::invalidate_for_home(dir.path());
        let mut config = crate::agent::config::Config::default();
        config.model_providers.insert(
            "grok_build_openrouter".to_owned(),
            crate::agent::model_providers::grok_build_openrouter_config(),
        );
        let mut models = indexmap::IndexMap::new();
        models.insert(
            "openrouter:moonshotai/kimi-k3".to_owned(),
            entry_on_provider(
                "moonshotai/kimi-k3",
                "grok_build_openrouter",
                ModelProviderKind::OpenRouter,
                "https://openrouter.ai/api/v1",
            ),
        );
        let manager = crate::agent::models::ModelsManager::new(
            None,
            models,
            acp::ModelId::new("openrouter:moonshotai/kimi-k3"),
            std::sync::Arc::new(crate::auth::AuthManager::new(
                dir.path(),
                crate::auth::GrokComConfig::default(),
            )),
            config,
        );
        let inference = InferenceConfig {
            base_url: "https://openrouter.ai/api/v1".into(),
            model: "moonshotai/kimi-k3".into(),
            provider_identity: xai_grok_inference::config::ProviderIdentity::OpenRouter,
            ..InferenceConfig::default()
        };
        let ok = resolve_for_models_manager_with_selection(
            &inference,
            &manager,
            "openrouter:moonshotai/kimi-k3",
            Some(dir.path()),
        )
        .expect("internal OpenRouter alias must resolve to the canonical built-in");
        assert_eq!(ok.instance_id(), "openrouter");
    }

    /// Production resolve attaches the stable builtin incarnation. That stamp
    /// must not zero the live API-key generation, or `RouteBoundBearerResolver`
    /// fail-closes and the sampler sends OpenRouter requests with no Bearer.
    #[test]
    fn builtin_openrouter_lifecycle_incarnation_keeps_live_api_key_generation() {
        use crate::auth::attribution::lookup_route_credential;
        use crate::provider_registry::lifecycle_state::stable_builtin_incarnation;
        use crate::provider_registry::runtime_cache;

        let dir = tempdir().unwrap();
        runtime_cache::invalidate_for_home(dir.path());
        let generation =
            store_provider_api_key(dir.path(), OPENROUTER_API_KEY_SCOPE, "or-live-key-BBBBBBBB")
                .unwrap();
        assert_eq!(generation, 1);

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
            "openrouter:deepseek/deepseek-v4-flash-0731".to_owned(),
            entry_on_provider(
                "deepseek/deepseek-v4-flash-0731",
                "openrouter",
                ModelProviderKind::OpenRouter,
                "https://openrouter.ai/api/v1",
            ),
        );
        let manager = crate::agent::models::ModelsManager::new(
            None,
            models,
            acp::ModelId::new("openrouter:deepseek/deepseek-v4-flash-0731"),
            std::sync::Arc::new(crate::auth::AuthManager::new(
                dir.path(),
                crate::auth::GrokComConfig::default(),
            )),
            config,
        );
        let inference = InferenceConfig {
            base_url: "https://openrouter.ai/api/v1".into(),
            model: "deepseek/deepseek-v4-flash-0731".into(),
            provider_identity: xai_grok_inference::config::ProviderIdentity::OpenRouter,
            ..InferenceConfig::default()
        };
        let ctx = resolve_for_models_manager_with_selection(
            &inference,
            &manager,
            "openrouter:deepseek/deepseek-v4-flash-0731",
            Some(dir.path()),
        )
        .expect("builtin OpenRouter must resolve");
        assert_eq!(ctx.instance_id(), "openrouter");
        assert_eq!(ctx.credential_route(), RouteCredentialRoute::ApiKey);
        let expected_incarnation =
            stable_builtin_incarnation("openrouter").expect("builtin OpenRouter incarnation");
        assert_eq!(
            ctx.incarnation(),
            Some(expected_incarnation.as_str()),
            "production resolve stamps the stable builtin incarnation"
        );
        assert_eq!(ctx.binding_generation(), generation);
        assert_eq!(ctx.authority(), RouteAuthority::Authoritative);
        let snap = lookup_route_credential(dir.path(), &ctx)
            .expect("exact-route lookup must return the vault key with incarnation present");
        assert_eq!(snap.key, "or-live-key-BBBBBBBB");
    }

    /// Re-enable after disable: resolve succeeds again (session re-enableable).
    #[test]
    fn disable_then_reenable_allows_subsequent_resolve() {
        use crate::provider_registry::management::ProviderManagementService;
        use crate::provider_registry::management::dto::ProviderAddRequest;
        use crate::provider_registry::runtime_cache;

        let dir = tempdir().unwrap();
        runtime_cache::invalidate_for_home(dir.path());
        let svc = ProviderManagementService::new(dir.path());
        let g0 = svc.current_generation();
        assert!(
            svc.add(ProviderAddRequest {
                id: "lab".into(),
                kind: "openai_compatible".into(),
                base_url: "http://127.0.0.1:9/v1".into(),
                display_name: None,
                admin_base_url: None,
                enabled: true,
                expected_generation: g0,
            })
            .ok
        );
        runtime_cache::invalidate_for_home(dir.path());

        let mut config = crate::agent::config::Config::default();
        config.model_providers.insert(
            "lab".to_owned(),
            ModelProviderConfig {
                kind: ModelProviderKind::OpenAiCompatible,
                base_url: Some("http://127.0.0.1:9/v1".into()),
                enabled: true,
                ..ModelProviderConfig::default()
            },
        );
        let mut models = indexmap::IndexMap::new();
        models.insert(
            "lab:gpt".to_owned(),
            entry_on_provider(
                "gpt",
                "lab",
                ModelProviderKind::OpenAiCompatible,
                "http://127.0.0.1:9/v1",
            ),
        );
        let manager = crate::agent::models::ModelsManager::new(
            None,
            models,
            acp::ModelId::new("lab:gpt"),
            std::sync::Arc::new(crate::auth::AuthManager::new(
                dir.path(),
                crate::auth::GrokComConfig::default(),
            )),
            config,
        );
        let inference = InferenceConfig {
            base_url: "http://127.0.0.1:9/v1".into(),
            model: "gpt".into(),
            provider_identity: xai_grok_inference::config::ProviderIdentity::Custom,
            ..InferenceConfig::default()
        };

        assert!(
            resolve_for_models_manager_with_selection(
                &inference,
                &manager,
                "lab:gpt",
                Some(dir.path()),
            )
            .is_ok()
        );
        assert!(svc.set_enabled("lab", false, svc.current_generation()).ok);
        runtime_cache::invalidate_for_home(dir.path());
        assert!(
            resolve_for_models_manager_with_selection(
                &inference,
                &manager,
                "lab:gpt",
                Some(dir.path()),
            )
            .is_err()
        );
        // Re-enable: subsequent prepare/resolve must succeed.
        assert!(svc.set_enabled("lab", true, svc.current_generation()).ok);
        runtime_cache::invalidate_for_home(dir.path());
        // Config snapshot in models_manager still has enabled:true from construction;
        // live assert uses runtime_cache / config.toml which was re-enabled.
        assert_live_route_usable(
            dir.path(),
            &resolve_for_models_manager_with_selection(
                &inference,
                &manager,
                "lab:gpt",
                Some(dir.path()),
            )
            .expect("re-enable must allow resolve"),
            false,
        )
        .expect("re-enable must allow live assert");
    }

    /// True legacy only when no configured provider instance is selected.
    #[test]
    fn true_legacy_only_without_configured_provider_selection() {
        let dir = tempdir().unwrap();
        let config = crate::agent::config::Config::default();
        let models = indexmap::IndexMap::new();
        let manager = crate::agent::models::ModelsManager::new(
            None,
            models,
            acp::ModelId::new("orphan-model"),
            std::sync::Arc::new(crate::auth::AuthManager::new(
                dir.path(),
                crate::auth::GrokComConfig::default(),
            )),
            config,
        );
        let inference = InferenceConfig {
            base_url: "https://api.example.com/v1".into(),
            model: "orphan-model".into(),
            ..InferenceConfig::default()
        };
        // Selection not in catalog → no resolved provider → true legacy Ok.
        let route = resolve_for_models_manager_with_selection(
            &inference,
            &manager,
            "orphan-model",
            Some(dir.path()),
        )
        .expect("true legacy must remain Ok when no configured route");
        // legacy_from_config uses kind-level identity, not a configured instance.
        assert!(
            route.instance_id() != "lab",
            "legacy must not invent configured instance id"
        );
    }
}
