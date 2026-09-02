//! Per-instance typed CLI resolution via ProviderService + ApiSurface gates.
//!
//! Every OpenAI/OpenRouter typed operation is bound to one explicit provider
//! instance from an injected or config-derived [`ProviderService`] snapshot.
//! Declared [`ApiSurface`] is enforced *before* credential resolution and
//! network. Dry-run is credential-free and only emits safe selected-instance
//! metadata plus the merged typed request.

use super::generated_ops::CliOperation;
use crate::agent::model_providers::parse_model_providers;
use crate::provider_registry::id::ProviderId;
use crate::provider_registry::instance::{
    ApiSurface, CredentialRoute, ProviderInstanceDescriptor, ProviderKind,
};
use crate::provider_registry::secrets::{
    admin_key_scope_for_kind, application_key_scope_for_kind, read_provider_secret,
};
use crate::provider_registry::{ProviderService, ProviderServiceError};
use indexmap::IndexMap;
use serde_json::{Value, json};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use xai_grok_inference::{PlatformClientConfig, TransportPolicy};

/// Explicit ChatGPT / custom OpenAI-compatible subset allowlist.
///
/// Derived from the declared OpenAI↔OpenRouter intersection plus the required
/// streaming companions. Full OpenAI Platform ops are *not* admitted here.
pub const OPENAI_COMPATIBLE_SUBSET_OPERATION_IDS: &[&str] = &[
    "listModels",
    "createChatCompletion",
    "createChatCompletion_stream",
    "createResponse",
    "createResponse_stream",
    "createEmbedding",
];

/// Safe view of the selected provider instance for dry-run / diagnostics.
#[derive(Debug, Clone)]
pub struct SelectedInstance {
    pub id: String,
    pub kind: ProviderKind,
    pub display_name: String,
    pub base_url: String,
    pub admin_base_url: Option<String>,
    pub api_surface: ApiSurface,
    pub credential_route: CredentialRoute,
    pub enabled: bool,
    pub env_keys: Vec<String>,
    pub admin_env_key: Option<String>,
    pub extra_headers: IndexMap<String, String>,
}

impl SelectedInstance {
    /// Credential-free JSON for dry-run / status (never tokens or scopes).
    pub fn safe_json(&self) -> Value {
        json!({
            "id": self.id,
            "kind": self.kind.as_str(),
            "display_name": self.display_name,
            "base_url": self.base_url,
            "admin_base_url": self.admin_base_url,
            "api_surface": self.api_surface.as_str(),
            "credential_route": self.credential_route.as_str(),
            "enabled": self.enabled,
        })
    }
}

/// Optional test/injection override for the ProviderService snapshot.
///
/// Production paths leave this empty and load from `GROK_HOME` config.
fn service_override() -> &'static Mutex<Option<ProviderService>> {
    static OVERRIDE: OnceLock<Mutex<Option<ProviderService>>> = OnceLock::new();
    OVERRIDE.get_or_init(|| Mutex::new(None))
}

/// Install a ProviderService for the current process (tests). Returns a guard
/// that clears the override on drop.
#[cfg(test)]
pub struct ProviderServiceOverrideGuard;

#[cfg(test)]
impl Drop for ProviderServiceOverrideGuard {
    fn drop(&mut self) {
        if let Ok(mut slot) = service_override().lock() {
            *slot = None;
        }
    }
}

/// Override the ProviderService used by typed CLI dispatch (tests only).
#[cfg(test)]
pub fn override_provider_service(service: ProviderService) -> ProviderServiceOverrideGuard {
    *service_override().lock().unwrap_or_else(|p| p.into_inner()) = Some(service);
    ProviderServiceOverrideGuard
}

/// Load the ProviderService snapshot for CLI dispatch.
///
/// Uses the injected test override when present; otherwise parses
/// `[model_providers]` from `$GROK_HOME/config.toml` and builds a
/// [`ProviderService`]. Missing/unreadable config falls back to built-in
/// product descriptors only (never invents unconfigured custom ids).
pub fn load_provider_service(home: &Path) -> Result<ProviderService, String> {
    if let Ok(guard) = service_override().lock()
        && let Some(svc) = guard.as_ref()
    {
        return Ok(svc.clone());
    }
    let cfg_path = home.join("config.toml");
    match std::fs::read_to_string(&cfg_path) {
        Ok(raw) => {
            // Prefer configured instances when config is well-formed. Use
            // `toml::from_str` (serde) — `str::parse` is not the supported path
            // for `toml::Value` in this workspace. Unparseable documents fall
            // back to product built-ins only so typed dry-run stays hermetic.
            let Ok(val) = toml::from_str::<toml::Value>(&raw) else {
                return Ok(ProviderService::default());
            };
            let (entries, _warnings) = parse_model_providers(&val);
            ProviderService::from_model_providers(&entries).map_err(service_err)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ProviderService::default()),
        Err(e) => Err(format!("read config.toml: {e}")),
    }
}

fn service_err(e: ProviderServiceError) -> String {
    e.to_string()
}

/// Resolve one exact configured/built-in instance and the route that will serve
/// `op`. Fails closed when the id is absent (no silent built-in fallback to a
/// different provider), disabled, or has no compatible ApiSurface.
pub fn resolve_selected_instance(
    service: &ProviderService,
    provider: &str,
    op: &CliOperation,
) -> Result<SelectedInstance, String> {
    let desc = service.get(provider).ok_or_else(|| {
        format!(
            "provider instance `{provider}` is not configured (no built-in fallback for unknown ids)"
        )
    })?;
    if !desc.enabled {
        return Err(format!("provider instance `{provider}` is disabled"));
    }
    let route = select_route_for_operation(desc, op)?;
    let meta = service.snapshot().get(provider);
    let base_url = desc
        .base_url
        .clone()
        .or_else(|| meta.and_then(|m| m.base_url.clone()))
        .ok_or_else(|| format!("provider `{provider}` has no base_url"))?;
    crate::provider_registry::lifecycle::validate_http_base_url(&base_url)
        .map_err(|e| e.to_string())?;
    let admin_base_url = desc
        .admin_base_url
        .clone()
        .or_else(|| meta.and_then(|m| m.admin_base_url.clone()));
    if let Some(ref a) = admin_base_url {
        crate::provider_registry::lifecycle::validate_http_base_url(a)
            .map_err(|e| e.to_string())?;
    }
    let mut env_keys = desc.env_keys.clone();
    if env_keys.is_empty()
        && let Some(ek) = meta.and_then(|m| m.env_key.clone())
    {
        env_keys.push(ek);
    }
    let admin_env_key = meta.and_then(|m| m.admin_env_key.clone());
    let extra_headers = meta.map(|m| m.extra_headers.clone()).unwrap_or_default();
    crate::provider_registry::lifecycle::validate_extra_headers(&extra_headers)
        .map_err(|e| e.to_string())?;

    Ok(SelectedInstance {
        id: desc.id.as_str().to_owned(),
        kind: desc.kind,
        display_name: desc.display_label().to_owned(),
        base_url,
        admin_base_url,
        api_surface: route.api_surface,
        credential_route: route.credential_route,
        enabled: desc.enabled,
        env_keys,
        admin_env_key,
        extra_headers,
    })
}

/// Choose the route that will serve this operation. OpenAI Platform and
/// ChatGPT OAuth routes never cross; OpenRouter native requires kind+surface.
fn select_route_for_operation<'a>(
    desc: &'a ProviderInstanceDescriptor,
    op: &CliOperation,
) -> Result<&'a crate::provider_registry::instance::ProviderRouteDescriptor, String> {
    match op.provider_namespace {
        "openai_admin" => desc
            .routes
            .iter()
            .find(|r| r.api_surface == ApiSurface::OpenAiPlatform)
            .ok_or_else(|| {
                format!(
                    "provider `{}` does not declare api_surface=openai_platform (required for admin ops)",
                    desc.id.as_str()
                )
            }),
        "openai" => {
            // Prefer full platform, then compatible subset / ChatGPT inference.
            if let Some(r) = desc
                .routes
                .iter()
                .find(|r| r.api_surface == ApiSurface::OpenAiPlatform)
            {
                return Ok(r);
            }
            if let Some(r) = desc.routes.iter().find(|r| {
                matches!(
                    r.api_surface,
                    ApiSurface::OpenAiCompatibleSubset | ApiSurface::ChatGptInference
                )
            }) {
                return Ok(r);
            }
            Err(format!(
                "provider `{}` has no OpenAI platform/subset surface for operation `{}`",
                desc.id.as_str(),
                op.operation_id
            ))
        }
        "openrouter" => {
            if desc.kind != ProviderKind::OpenRouter {
                return Err(format!(
                    "openrouter operations require kind=openrouter (provider `{}` is {})",
                    desc.id.as_str(),
                    desc.kind.as_str()
                ));
            }
            desc.routes
                .iter()
                .find(|r| r.api_surface == ApiSurface::OpenRouterNative)
                .ok_or_else(|| {
                    format!(
                        "provider `{}` does not declare api_surface=openrouter_native",
                        desc.id.as_str()
                    )
                })
        }
        other => Err(format!("unknown operation namespace `{other}`")),
    }
}

/// Enforce ApiSurface before credentials/network. Unsupported calls fail locally.
pub fn assert_surface_allows_operation(
    instance: &SelectedInstance,
    op: &CliOperation,
) -> Result<(), String> {
    match instance.api_surface {
        ApiSurface::OpenAiPlatform => {
            if !matches!(op.provider_namespace, "openai" | "openai_admin") {
                return Err(format!(
                    "api_surface=openai_platform cannot serve namespace `{}`",
                    op.provider_namespace
                ));
            }
            Ok(())
        }
        ApiSurface::OpenRouterNative => {
            if instance.kind != ProviderKind::OpenRouter {
                return Err(format!(
                    "api_surface=openrouter_native requires kind=openrouter (got {})",
                    instance.kind.as_str()
                ));
            }
            if op.provider_namespace != "openrouter" {
                return Err(format!(
                    "api_surface=openrouter_native cannot serve namespace `{}`",
                    op.provider_namespace
                ));
            }
            Ok(())
        }
        ApiSurface::OpenAiCompatibleSubset | ApiSurface::ChatGptInference => {
            if op.provider_namespace != "openai" {
                return Err(format!(
                    "api_surface={} only allows the openai common subset (not `{}` / admin)",
                    instance.api_surface.as_str(),
                    op.provider_namespace
                ));
            }
            if !OPENAI_COMPATIBLE_SUBSET_OPERATION_IDS.contains(&op.operation_id) {
                return Err(format!(
                    "operation `{}` is not in the openai_compatible_subset / chatgpt allowlist \
                     (allowed: {})",
                    op.operation_id,
                    OPENAI_COMPATIBLE_SUBSET_OPERATION_IDS.join(", ")
                ));
            }
            // ChatGPT OAuth never serves the platform API-key client path.
            if instance.api_surface == ApiSurface::ChatGptInference
                && instance.credential_route == CredentialRoute::ChatGptOauth
            {
                return Err(
                    "ChatGPT OAuth route cannot serve platform typed CLI operations \
                     (OpenAI Platform and ChatGPT OAuth never cross)"
                        .into(),
                );
            }
            Ok(())
        }
        ApiSurface::AnthropicMessages | ApiSurface::RetrievalOnly => Err(format!(
            "api_surface={} does not support OpenAI/OpenRouter typed CLI operations",
            instance.api_surface.as_str()
        )),
    }
}

/// Build the dry-run document: selected instance (safe) + redacted request meta.
/// Never includes tokens, env values, or vault material.
pub fn dry_run_document(instance: &SelectedInstance, op: &CliOperation, merged: &Value) -> Value {
    json!({
        "dry_run": true,
        "provider": instance.id,
        "selected_instance": instance.safe_json(),
        "operation_id": op.operation_id,
        "provider_namespace": op.provider_namespace,
        "request_type": op.request_type,
        "response_type": op.response_type,
        "client_method": op.client_method,
        "transports": op.transports,
        "credential_class": op.credential_class,
        "requires_confirmation": op.requires_confirmation,
        "typed_request": merged,
    })
}

/// Resolve application/admin tokens for the exact instance and operation.
///
/// - Admin ops never borrow the application key when admin is missing.
/// - ChatGPT OAuth tokens are never used as platform application keys.
/// - OpenAI Platform and ChatGPT OAuth never cross.
pub fn resolve_instance_credentials(
    instance: &SelectedInstance,
    op: &CliOperation,
    home: &Path,
) -> Result<(Option<String>, Option<String>), String> {
    // Platform typed CLI uses API-key routes only. OAuth session material is
    // never injected into PlatformClientConfig.
    if instance.credential_route == CredentialRoute::ChatGptOauth {
        return Err(
            "refusing to resolve ChatGPT OAuth credentials for platform typed CLI \
             (OpenAI Platform and ChatGPT OAuth never cross)"
                .into(),
        );
    }

    let pid = ProviderId::new(&instance.id).map_err(|e| e.to_string())?;
    let want_admin = op.is_admin || op.credential_class == "admin";

    let app_token = if want_admin {
        None
    } else {
        resolve_app_token(instance, home, &pid)
    };
    let admin_token = resolve_admin_token(instance, home, &pid);

    if want_admin
        && admin_token
            .as_ref()
            .map(|s| s.trim().is_empty())
            .unwrap_or(true)
    {
        return Err(format!(
            "admin credential required for {}::{} (never borrowing application key)",
            op.provider_namespace, op.operation_id
        ));
    }

    Ok((app_token, admin_token))
}

fn resolve_app_token(instance: &SelectedInstance, home: &Path, pid: &ProviderId) -> Option<String> {
    for name in &instance.env_keys {
        if let Ok(v) = std::env::var(name)
            && !v.trim().is_empty()
        {
            return Some(v);
        }
    }
    // Built-in env fallbacks (names only; values never logged).
    match instance.id.as_str() {
        "openai" => {
            if let Ok(v) = std::env::var("OPENAI_API_KEY")
                && !v.trim().is_empty()
            {
                return Some(v);
            }
            crate::auth::read_provider_api_key(home, crate::auth::OPENAI_API_KEY_SCOPE)
                .ok()
                .flatten()
        }
        "openrouter" => {
            if let Ok(v) = std::env::var("OPENROUTER_API_KEY")
                && !v.trim().is_empty()
            {
                return Some(v);
            }
            crate::auth::read_provider_api_key(home, crate::auth::OPENROUTER_API_KEY_SCOPE)
                .ok()
                .flatten()
        }
        "zai" | "zai-model-api" => {
            if let Ok(v) = std::env::var(crate::agent::zai::ZAI_ENV_KEY)
                && !v.trim().is_empty()
            {
                return Some(v);
            }
            read_provider_secret(home, &application_key_scope_for_kind(pid, instance.kind))
                .ok()
                .flatten()
        }
        _ => read_provider_secret(home, &application_key_scope_for_kind(pid, instance.kind))
            .ok()
            .flatten(),
    }
}

fn resolve_admin_token(
    instance: &SelectedInstance,
    home: &Path,
    pid: &ProviderId,
) -> Option<String> {
    // Never fall back to the application key when admin is missing.
    if let Some(name) = instance.admin_env_key.as_deref()
        && let Ok(v) = std::env::var(name)
        && !v.trim().is_empty()
    {
        return Some(v);
    }
    match instance.id.as_str() {
        "openrouter" => {
            for env in ["OPENROUTER_ADMIN_API_KEY", "OPENROUTER_MANAGEMENT_API_KEY"] {
                if let Ok(v) = std::env::var(env)
                    && !v.trim().is_empty()
                {
                    return Some(v);
                }
            }
            if let Ok(Some(v)) =
                crate::auth::read_provider_api_key(home, crate::auth::OPENROUTER_ADMIN_KEY_SCOPE)
                && !v.trim().is_empty()
            {
                return Some(v);
            }
            if let Ok(Some(v)) = crate::auth::read_provider_api_key(
                home,
                crate::auth::OPENROUTER_MANAGEMENT_KEY_SCOPE,
            ) && !v.trim().is_empty()
            {
                return Some(v);
            }
            read_provider_secret(home, &admin_key_scope_for_kind(pid, instance.kind))
                .ok()
                .flatten()
        }
        "openai" => {
            if let Ok(v) = std::env::var("OPENAI_ADMIN_KEY")
                && !v.trim().is_empty()
            {
                return Some(v);
            }
            crate::auth::read_provider_api_key(home, crate::auth::OPENAI_ADMIN_KEY_SCOPE)
                .ok()
                .flatten()
                .or_else(|| {
                    read_provider_secret(home, &admin_key_scope_for_kind(pid, instance.kind))
                        .ok()
                        .flatten()
                })
        }
        _ => read_provider_secret(home, &admin_key_scope_for_kind(pid, instance.kind))
            .ok()
            .flatten(),
    }
}

/// Build a PlatformClientConfig for the selected instance and credential slot.
pub fn build_platform_client_config(
    instance: &SelectedInstance,
    op: &CliOperation,
    app_token: Option<String>,
    admin_token: Option<String>,
) -> PlatformClientConfig {
    let want_admin = op.is_admin || op.credential_class == "admin";
    PlatformClientConfig {
        provider_id: instance.id.clone(),
        display_name: instance.display_name.clone(),
        base_url: instance.base_url.clone(),
        admin_base_url: instance.admin_base_url.clone(),
        application_token: if want_admin { None } else { app_token },
        admin_token,
        extra_headers: instance
            .extra_headers
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        policy: TransportPolicy::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::model_providers::{ModelProviderConfig, ModelProviderKind};
    use crate::cli::generated_ops::find_cli_operation;

    fn custom_subset_service(id: &str) -> ProviderService {
        let mut m = IndexMap::new();
        m.insert(
            id.to_owned(),
            ModelProviderConfig {
                kind: ModelProviderKind::OpenAiCompatible,
                base_url: Some("https://custom.example/v1".into()),
                display_name: Some("Custom".into()),
                env_key: Some(crate::agent::config::EnvKeys::single("CUSTOM_KEY")),
                ..Default::default()
            },
        );
        ProviderService::from_model_providers(&m).unwrap()
    }

    #[test]
    fn subset_allowlist_includes_response_stream() {
        assert!(OPENAI_COMPATIBLE_SUBSET_OPERATION_IDS.contains(&"createResponse_stream"));
        assert!(OPENAI_COMPATIBLE_SUBSET_OPERATION_IDS.contains(&"createChatCompletion_stream"));
        assert!(OPENAI_COMPATIBLE_SUBSET_OPERATION_IDS.contains(&"listModels"));
    }

    #[test]
    fn openai_platform_allows_full_surface() {
        let svc = ProviderService::default();
        let op = find_cli_operation("openai", "deleteModel").unwrap();
        let inst = resolve_selected_instance(&svc, "openai", op).unwrap();
        assert_eq!(inst.api_surface, ApiSurface::OpenAiPlatform);
        assert_surface_allows_operation(&inst, op).unwrap();
        let admin = find_cli_operation("openai_admin", "admin-api-keys-list").unwrap();
        let inst_a = resolve_selected_instance(&svc, "openai", admin).unwrap();
        assert_surface_allows_operation(&inst_a, admin).unwrap();
    }

    #[test]
    fn custom_subset_rejects_admin_and_non_allowlisted() {
        let svc = custom_subset_service("my-proxy");
        let list = find_cli_operation("openai", "listModels").unwrap();
        let inst = resolve_selected_instance(&svc, "my-proxy", list).unwrap();
        assert_eq!(inst.api_surface, ApiSurface::OpenAiCompatibleSubset);
        assert_surface_allows_operation(&inst, list).unwrap();

        let delete = find_cli_operation("openai", "deleteModel").unwrap();
        let inst2 = resolve_selected_instance(&svc, "my-proxy", delete).unwrap();
        let err = assert_surface_allows_operation(&inst2, delete).unwrap_err();
        assert!(err.contains("allowlist"), "{err}");

        let admin = find_cli_operation("openai_admin", "admin-api-keys-list").unwrap();
        let err = resolve_selected_instance(&svc, "my-proxy", admin).unwrap_err();
        assert!(err.contains("openai_platform"), "{err}");
    }

    #[test]
    fn openrouter_requires_kind_and_native_surface() {
        let svc = ProviderService::default();
        let op = find_cli_operation("openrouter", "getCurrentKey").unwrap();
        let inst = resolve_selected_instance(&svc, "openrouter", op).unwrap();
        assert_eq!(inst.api_surface, ApiSurface::OpenRouterNative);
        assert_eq!(inst.kind, ProviderKind::OpenRouter);
        assert_surface_allows_operation(&inst, op).unwrap();

        // OpenAI instance cannot serve openrouter namespace.
        let err = resolve_selected_instance(&svc, "openai", op).unwrap_err();
        assert!(err.contains("openrouter") || err.contains("kind"), "{err}");
    }

    #[test]
    fn unknown_provider_has_no_builtin_fallback() {
        let svc = ProviderService::default();
        let op = find_cli_operation("openai", "listModels").unwrap();
        let err = resolve_selected_instance(&svc, "definitely-not-configured", op).unwrap_err();
        assert!(err.contains("not configured"), "{err}");
        assert!(err.contains("no built-in fallback"), "{err}");
    }

    #[test]
    fn dry_run_document_is_credential_free() {
        let svc = ProviderService::default();
        let op = find_cli_operation("openai", "listModels").unwrap();
        let inst = resolve_selected_instance(&svc, "openai", op).unwrap();
        let doc = dry_run_document(&inst, op, &json!({"limit": "1"}));
        let s = doc.to_string();
        assert!(s.contains("dry_run"));
        assert!(s.contains("selected_instance"));
        assert!(s.contains("listModels"));
        assert!(!s.contains("OPENAI_API_KEY"));
        assert!(!s.contains("sk-"));
        assert!(!s.contains("application_token"));
        assert!(!s.contains("admin_token"));
    }

    #[test]
    fn anthropic_surface_rejects_openai_ops() {
        let svc = ProviderService::default();
        let op = find_cli_operation("openai", "listModels").unwrap();
        let err = resolve_selected_instance(&svc, "anthropic", op).unwrap_err();
        assert!(
            err.contains("no OpenAI") || err.contains("surface") || err.contains("platform"),
            "{err}"
        );
    }

    #[test]
    fn missing_admin_never_borrows_application_message() {
        let svc = ProviderService::default();
        let op = find_cli_operation("openrouter", "listBYOKKeys").unwrap();
        let inst = resolve_selected_instance(&svc, "openrouter", op).unwrap();
        // No admin env set — must fail closed without borrowing app key.
        let home = std::env::temp_dir().join(format!("grok-cli-cred-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&home);
        let err = resolve_instance_credentials(&inst, op, &home).unwrap_err();
        assert!(err.contains("never borrowing application key"), "{err}");
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    #[serial_test::serial]
    fn extra_openrouter_instance_credentials_read_own_scope_only() {
        use crate::auth::OPENROUTER_API_KEY_SCOPE;
        use crate::provider_registry::secrets::{
            application_key_scope, clear_provider_secret, extra_openrouter_application_key_scope,
            store_provider_secret,
        };

        let home = tempfile::tempdir().unwrap();
        let mut providers = IndexMap::new();
        providers.insert(
            "openrouter-work".to_owned(),
            ModelProviderConfig {
                kind: ModelProviderKind::OpenRouter,
                base_url: Some("https://openrouter.ai/api/v1".into()),
                enabled: true,
                ..ModelProviderConfig::default()
            },
        );
        let svc = ProviderService::from_model_providers(&providers).unwrap();
        let op = find_cli_operation("openrouter", "getModels").unwrap();
        let work = crate::provider_registry::ProviderId::new("openrouter-work").unwrap();

        store_provider_secret(
            home.path(),
            &extra_openrouter_application_key_scope(&work),
            "work-key",
        )
        .unwrap();
        // Decoys the extra instance must never borrow.
        store_provider_secret(
            home.path(),
            &application_key_scope(&work),
            "openai-compatible-decoy",
        )
        .unwrap();
        store_provider_secret(home.path(), OPENROUTER_API_KEY_SCOPE, "builtin-decoy").unwrap();
        unsafe {
            std::env::set_var("OPENROUTER_API_KEY", "env-builtin-decoy");
        }

        let inst = resolve_selected_instance(&svc, "openrouter-work", op).unwrap();
        assert_eq!(inst.kind, ProviderKind::OpenRouter);
        let (app, admin) = resolve_instance_credentials(&inst, op, home.path()).unwrap();
        assert_eq!(
            app.as_deref(),
            Some("work-key"),
            "extra OpenRouter must read openrouter::<id>::api_key only"
        );
        assert!(
            admin.is_none(),
            "application op must not resolve admin token"
        );

        // Clearing the Work key must fail closed: no built-in scope, env var,
        // or openai_compatible sibling fallback for a typed application op.
        clear_provider_secret(home.path(), &extra_openrouter_application_key_scope(&work)).unwrap();
        let (app, admin) = resolve_instance_credentials(&inst, op, home.path()).unwrap();
        assert!(
            app.is_none(),
            "missing extra OpenRouter key must fail closed (no builtin/env sibling borrow)"
        );
        assert!(admin.is_none());

        unsafe {
            std::env::remove_var("OPENROUTER_API_KEY");
        }
    }
}
