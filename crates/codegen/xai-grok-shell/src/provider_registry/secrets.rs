//! Per-provider application, administration, and OAuth secret scopes.

use super::id::{
    BuiltInProviderId, ProviderId, ProviderIdError, is_reserved_configured_id,
    validate_provider_id_str,
};
use super::instance::{ProviderIncarnation, ProviderKind};
use crate::auth::{
    ANTHROPIC_API_KEY_SCOPE, OPENAI_API_KEY_SCOPE, OPENROUTER_ADMIN_KEY_SCOPE,
    OPENROUTER_API_KEY_SCOPE, OPENROUTER_MANAGEMENT_KEY_SCOPE, clear_provider_api_key,
    read_provider_api_key, store_provider_api_key,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Which credential a scope refers to.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCredentialKind {
    Application,
    Admin,
    /// Bearer token for a remote vector-store mirror
    /// (`milvus::<store-id>::token`). Never written by the provider CLI.
    Token,
}

impl ProviderCredentialKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Application => "api_key",
            Self::Admin => "admin_key",
            Self::Token => "token",
        }
    }
}

/// Vault namespace for a configured-instance API key or admin key.
///
/// Extra `kind=openrouter` instances use [`Self::OpenRouter`]
/// (`openrouter::<id>::api_key`). Other configured hosts stay on
/// [`Self::OpenAiCompatible`]. Built-in OpenRouter remains the two-part
/// `openrouter::api_key` product scope and never uses this enum.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderSecretNamespace {
    OpenAiCompatible,
    OpenRouter,
    /// Remote vector-store mirror (`milvus::<store-id>::token`). The only
    /// namespace that accepts kind `token`; it rejects `api_key`/`admin_key`.
    Milvus,
}

impl ProviderSecretNamespace {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpenAiCompatible => "openai_compatible",
            Self::OpenRouter => "openrouter",
            Self::Milvus => "milvus",
        }
    }
}

/// Canonical secret scope for a configured provider instance.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ProviderSecretScope {
    pub provider_id: ProviderId,
    pub kind: ProviderCredentialKind,
    pub namespace: ProviderSecretNamespace,
}

impl ProviderSecretScope {
    pub fn new(provider_id: ProviderId, kind: ProviderCredentialKind) -> Self {
        Self {
            provider_id,
            kind,
            namespace: ProviderSecretNamespace::OpenAiCompatible,
        }
    }

    pub fn openrouter(provider_id: ProviderId, kind: ProviderCredentialKind) -> Self {
        Self {
            provider_id,
            kind,
            namespace: ProviderSecretNamespace::OpenRouter,
        }
    }

    /// `openai_compatible::<id>::api_key` / `...::admin_key`, or
    /// `openrouter::<id>::api_key` / `...::admin_key` for extra OpenRouter
    /// accounts. Never the built-in two-part `openrouter::api_key`.
    pub fn as_scope_string(&self) -> String {
        format!(
            "{}::{}::{}",
            self.namespace.as_str(),
            self.provider_id.as_str(),
            self.kind.as_str()
        )
    }
}

/// Canonical OAuth secret scope for a configured provider instance.
///
/// `provider::<instance-id>::oauth`. Distinct from the built-in `openai::oauth`
/// scope so a configured provider's ChatGPT OAuth credential never falls back
/// to (or is masked by) a built-in subscription token.
pub fn oauth_scope_string(provider_id: &ProviderId) -> String {
    format!("provider::{}::oauth", provider_id.as_str())
}

/// Private sidecar scope holding the secret-free binding record for a
/// configured provider's OAuth account. Never accepted by API-key or OAuth
/// token APIs; never a token.
pub(crate) fn oauth_meta_scope_string(provider_id: &ProviderId) -> String {
    format!("provider::{}::oauth::meta", provider_id.as_str())
}

/// Parse a configured-provider OAuth token scope (`provider::<id>::oauth`).
///
/// Returns `None` for the built-in `openai::oauth` scope, the private
/// `provider::<id>::oauth::meta` sidecar, and any malformed form, so the
/// configured-OAuth allowlist can never admit a built-in or metadata scope.
pub(crate) fn parse_configured_oauth_scope(scope: &str) -> Option<ProviderId> {
    let rest = scope.strip_prefix("provider::")?;
    let id_part = rest.strip_suffix("::oauth")?;
    ProviderId::new(id_part).ok()
}

/// Secret-free binding of a configured-provider OAuth credential to one exact
/// provider instance and generation.
///
/// Carries only the validated provider id, an optional incarnation token
/// (canonical UUID, never a secret), and a monotonic per-account generation
/// counter. It is safe to persist in `auth.json` metadata and never contains
/// token material.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProviderOAuthBinding {
    pub provider_id: ProviderId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incarnation: Option<ProviderIncarnation>,
    /// Monotonic per-account credential generation. Rotating one account's
    /// tokens increments only this account's generation; siblings are
    /// untouched. Survives logout so the next store increments rather than
    /// resets.
    #[serde(default)]
    pub generation: u64,
}

impl ProviderOAuthBinding {
    pub fn new(provider_id: ProviderId) -> Self {
        Self {
            provider_id,
            incarnation: None,
            generation: 0,
        }
    }

    pub fn with_incarnation(mut self, incarnation: Option<ProviderIncarnation>) -> Self {
        self.incarnation = incarnation;
        self
    }

    pub fn with_generation(mut self, generation: u64) -> Self {
        self.generation = generation;
        self
    }

    /// `provider::<id>::oauth` for this binding.
    pub fn scope_string(&self) -> String {
        oauth_scope_string(&self.provider_id)
    }
}

pub fn application_key_scope(provider_id: &ProviderId) -> String {
    ProviderSecretScope::new(provider_id.clone(), ProviderCredentialKind::Application)
        .as_scope_string()
}

pub fn admin_key_scope(provider_id: &ProviderId) -> String {
    ProviderSecretScope::new(provider_id.clone(), ProviderCredentialKind::Admin).as_scope_string()
}

/// Extra `kind=openrouter` application key: `openrouter::<id>::api_key`.
///
/// Distinct from the built-in two-part `openrouter::api_key` product scope.
pub fn extra_openrouter_application_key_scope(provider_id: &ProviderId) -> String {
    ProviderSecretScope::openrouter(provider_id.clone(), ProviderCredentialKind::Application)
        .as_scope_string()
}

/// Extra `kind=openrouter` admin key: `openrouter::<id>::admin_key`.
pub fn extra_openrouter_admin_key_scope(provider_id: &ProviderId) -> String {
    ProviderSecretScope::openrouter(provider_id.clone(), ProviderCredentialKind::Admin)
        .as_scope_string()
}

/// Whether `(kind, id)` is an extra OpenRouter account (not the built-in product).
pub fn is_extra_openrouter_instance(kind: ProviderKind, id: &str) -> bool {
    kind == ProviderKind::OpenRouter && BuiltInProviderId::parse(id).is_none()
}

/// Application-key vault scope for a configured instance of `kind`.
///
/// Extra OpenRouter accounts store `openrouter::<id>::api_key` and never
/// `openai_compatible::<id>::api_key`. Built-in product ids must use
/// [`built_in_application_scope`] instead of this helper.
pub fn application_key_scope_for_kind(provider_id: &ProviderId, kind: ProviderKind) -> String {
    if is_extra_openrouter_instance(kind, provider_id.as_str()) {
        extra_openrouter_application_key_scope(provider_id)
    } else {
        application_key_scope(provider_id)
    }
}

/// Admin-key vault scope for a configured instance of `kind`.
pub fn admin_key_scope_for_kind(provider_id: &ProviderId, kind: ProviderKind) -> String {
    if is_extra_openrouter_instance(kind, provider_id.as_str()) {
        extra_openrouter_admin_key_scope(provider_id)
    } else {
        admin_key_scope(provider_id)
    }
}

/// Clear leftover configured-instance API/admin keys after metadata is gone.
///
/// Tries both the openai_compatible and extra-OpenRouter schemes so a
/// tombstoned extra account cannot leave `openrouter::<id>::api_key` behind.
/// Never touches built-in two-part scopes (`openrouter::api_key`).
pub fn clear_configured_instance_secrets(grok_home: &Path, provider_id: &ProviderId) {
    let _ = clear_provider_secret(grok_home, &application_key_scope(provider_id));
    let _ = clear_provider_secret(grok_home, &admin_key_scope(provider_id));
    let _ = clear_provider_secret(
        grok_home,
        &extra_openrouter_application_key_scope(provider_id),
    );
    let _ = clear_provider_secret(grok_home, &extra_openrouter_admin_key_scope(provider_id));
}

/// Built-in scopes retained for migration and first-class product providers.
pub fn built_in_application_scope(provider: super::id::BuiltInProviderId) -> Option<&'static str> {
    match provider {
        super::id::BuiltInProviderId::OpenAi => Some(OPENAI_API_KEY_SCOPE),
        super::id::BuiltInProviderId::OpenRouter => Some(OPENROUTER_API_KEY_SCOPE),
        super::id::BuiltInProviderId::Anthropic => Some(ANTHROPIC_API_KEY_SCOPE),
        super::id::BuiltInProviderId::Xai => None,
    }
}

/// Parse and validate a provider secret scope. Never accepts arbitrary scopes.
///
/// Built-in product scopes, configured openai_compatible scopes, and extra
/// OpenRouter instance scopes (`openrouter::<configured-id>::api_key`).
/// Built-in aliases (`openai`, `chatgpt`, `xai`, `grok`, `anthropic`, plus the
/// rest of the reserved configured-id set) are never admitted as extra
/// OpenRouter ids. Configured OAuth is parsed separately via
/// [`parse_configured_oauth_scope`] and is never an API-key scope. OpenRouter
/// admin/management scopes are distinct from the application key and never
/// alias it.
pub fn parse_secret_scope(scope: &str) -> Result<ParsedSecretScope, ScopeParseError> {
    if scope == OPENAI_API_KEY_SCOPE {
        return Ok(ParsedSecretScope::BuiltInOpenAiApp);
    }
    if scope == OPENROUTER_API_KEY_SCOPE {
        return Ok(ParsedSecretScope::BuiltInOpenRouterApp);
    }
    if scope == OPENROUTER_ADMIN_KEY_SCOPE {
        return Ok(ParsedSecretScope::BuiltInOpenRouterAdmin);
    }
    if scope == OPENROUTER_MANAGEMENT_KEY_SCOPE {
        return Ok(ParsedSecretScope::BuiltInOpenRouterManagement);
    }
    if scope == ANTHROPIC_API_KEY_SCOPE {
        return Ok(ParsedSecretScope::BuiltInAnthropicApp);
    }
    if scope == "openai::admin_key" {
        return Ok(ParsedSecretScope::BuiltInOpenAiAdmin);
    }
    if let Some(parsed) = parse_openai_compatible_scope(scope)? {
        return Ok(parsed);
    }
    if let Some(parsed) = parse_extra_openrouter_scope(scope)? {
        return Ok(parsed);
    }
    if let Some(parsed) = parse_milvus_token_scope(scope)? {
        return Ok(parsed);
    }
    Err(ScopeParseError::UnknownScheme)
}

/// Parse `milvus::<store-id>::token` — the vault scope for a remote
/// vector-store mirror bearer token. The kind is fixed (`token`); sibling
/// kinds (`api_key` / `admin_key`) under the `milvus` namespace are
/// rejected by [`configured_scope`].
fn parse_milvus_token_scope(scope: &str) -> Result<Option<ParsedSecretScope>, ScopeParseError> {
    let Some(rest) = scope.strip_prefix("milvus::") else {
        return Ok(None);
    };
    let Some((id_part, kind_part)) = rest.rsplit_once("::") else {
        return Err(ScopeParseError::Malformed);
    };
    Ok(Some(ParsedSecretScope::Configured(configured_scope(
        id_part,
        kind_part,
        ProviderSecretNamespace::Milvus,
    )?)))
}

fn parse_openai_compatible_scope(
    scope: &str,
) -> Result<Option<ParsedSecretScope>, ScopeParseError> {
    let Some(rest) = scope.strip_prefix("openai_compatible::") else {
        return Ok(None);
    };
    let (id_part, kind_part) = rest.rsplit_once("::").ok_or(ScopeParseError::Malformed)?;
    Ok(Some(ParsedSecretScope::Configured(configured_scope(
        id_part,
        kind_part,
        ProviderSecretNamespace::OpenAiCompatible,
    )?)))
}

fn parse_extra_openrouter_scope(scope: &str) -> Result<Option<ParsedSecretScope>, ScopeParseError> {
    let Some(rest) = scope.strip_prefix("openrouter::") else {
        return Ok(None);
    };
    let Some((id_part, kind_part)) = rest.rsplit_once("::") else {
        // Two-part leftover (`openrouter::something`) is not an extra-instance
        // scope; built-ins were already matched.
        return Err(ScopeParseError::Malformed);
    };
    if is_reserved_configured_id(id_part) || BuiltInProviderId::parse(id_part).is_some() {
        return Err(ScopeParseError::InvalidId(ProviderIdError::Reserved {
            id: id_part.to_owned(),
        }));
    }
    Ok(Some(ParsedSecretScope::Configured(configured_scope(
        id_part,
        kind_part,
        ProviderSecretNamespace::OpenRouter,
    )?)))
}

fn configured_scope(
    id_part: &str,
    kind_part: &str,
    namespace: ProviderSecretNamespace,
) -> Result<ProviderSecretScope, ScopeParseError> {
    validate_provider_id_str(id_part).map_err(ScopeParseError::InvalidId)?;
    let kind = match (kind_part, namespace) {
        // `token` is exclusively a Milvus mirror scope; the Milvus namespace
        // carries nothing else — provider kinds can never masquerade as
        // mirror tokens and mirror tokens can never become API keys.
        ("token", ProviderSecretNamespace::Milvus) => ProviderCredentialKind::Token,
        ("token", _) => return Err(ScopeParseError::UnknownKind),
        (_, ProviderSecretNamespace::Milvus) => return Err(ScopeParseError::UnknownKind),
        ("api_key", _) => ProviderCredentialKind::Application,
        ("admin_key", _) => ProviderCredentialKind::Admin,
        _ => return Err(ScopeParseError::UnknownKind),
    };
    let provider_id = ProviderId::new(id_part).map_err(ScopeParseError::InvalidId)?;
    Ok(ProviderSecretScope {
        provider_id,
        kind,
        namespace,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedSecretScope {
    BuiltInOpenAiApp,
    BuiltInOpenAiAdmin,
    BuiltInOpenRouterApp,
    /// OpenRouter management key (`openrouter::admin_key`).
    BuiltInOpenRouterAdmin,
    /// Alias management scope (`openrouter::management_key`).
    BuiltInOpenRouterManagement,
    BuiltInAnthropicApp,
    Configured(ProviderSecretScope),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeParseError {
    UnknownScheme,
    Malformed,
    UnknownKind,
    InvalidId(ProviderIdError),
}

impl std::fmt::Display for ScopeParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownScheme => write!(f, "unknown secret scope scheme"),
            Self::Malformed => write!(f, "malformed provider secret scope"),
            Self::UnknownKind => write!(f, "unknown credential kind in scope"),
            Self::InvalidId(e) => write!(f, "invalid provider id in scope: {e}"),
        }
    }
}

impl std::error::Error for ScopeParseError {}

/// Whether `scope` is an allowed provider API-key scope for auth.json.
///
/// API-key storage never accepts OAuth routes: Platform API keys and ChatGPT
/// OAuth must not cross routes.
pub fn is_allowed_provider_scope(scope: &str) -> bool {
    parse_secret_scope(scope).is_ok()
}

/// Whether `scope` is an allowed configured-provider OAuth scope
/// (`provider::<id>::oauth`).
///
/// The built-in `openai::oauth` scope and the private `...::oauth::meta`
/// sidecar are never accepted here: configured OAuth must not fall back to
/// (or be written to) the built-in ChatGPT route.
pub fn is_allowed_oauth_scope(scope: &str) -> bool {
    parse_configured_oauth_scope(scope).is_some()
}

pub fn read_provider_secret(grok_home: &Path, scope: &str) -> std::io::Result<Option<String>> {
    if !is_allowed_provider_scope(scope) {
        return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput));
    }
    // Delegate to generalized storage once validate_provider_scope accepts us.
    read_provider_api_key(grok_home, scope)
}

pub fn store_provider_secret(grok_home: &Path, scope: &str, secret: &str) -> std::io::Result<()> {
    if !is_allowed_provider_scope(scope) {
        return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput));
    }
    if secret.trim().is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "secret must not be empty",
        ));
    }
    store_provider_api_key(grok_home, scope, secret).map(|_| ())
}

pub fn clear_provider_secret(grok_home: &Path, scope: &str) -> std::io::Result<()> {
    if !is_allowed_provider_scope(scope) {
        return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput));
    }
    clear_provider_api_key(grok_home, scope)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_configured_scopes() {
        let scope = application_key_scope(&ProviderId::new("local_vllm").unwrap());
        assert_eq!(scope, "openai_compatible::local_vllm::api_key");
        match parse_secret_scope(&scope).unwrap() {
            ParsedSecretScope::Configured(s) => {
                assert_eq!(s.provider_id.as_str(), "local_vllm");
                assert_eq!(s.kind, ProviderCredentialKind::Application);
                assert_eq!(s.namespace, ProviderSecretNamespace::OpenAiCompatible);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn parses_extra_openrouter_instance_scopes() {
        let work = ProviderId::new("openrouter-work").unwrap();
        let home = ProviderId::new("openrouter-home").unwrap();
        let app = extra_openrouter_application_key_scope(&work);
        let admin = extra_openrouter_admin_key_scope(&home);
        assert_eq!(app, "openrouter::openrouter-work::api_key");
        assert_eq!(admin, "openrouter::openrouter-home::admin_key");
        assert_eq!(
            application_key_scope_for_kind(&work, ProviderKind::OpenRouter),
            app
        );
        assert_eq!(
            application_key_scope_for_kind(&work, ProviderKind::OpenAiCompatible),
            "openai_compatible::openrouter-work::api_key"
        );
        match parse_secret_scope(&app).unwrap() {
            ParsedSecretScope::Configured(s) => {
                assert_eq!(s.provider_id.as_str(), "openrouter-work");
                assert_eq!(s.kind, ProviderCredentialKind::Application);
                assert_eq!(s.namespace, ProviderSecretNamespace::OpenRouter);
            }
            other => panic!("unexpected {other:?}"),
        }
        match parse_secret_scope(&admin).unwrap() {
            ParsedSecretScope::Configured(s) => {
                assert_eq!(s.provider_id.as_str(), "openrouter-home");
                assert_eq!(s.kind, ProviderCredentialKind::Admin);
                assert_eq!(s.namespace, ProviderSecretNamespace::OpenRouter);
            }
            other => panic!("unexpected {other:?}"),
        }
        assert!(is_allowed_provider_scope(&app));
        assert!(is_allowed_provider_scope(&admin));
        // Built-in two-part scope is never an extra-instance scope.
        assert!(matches!(
            parse_secret_scope(OPENROUTER_API_KEY_SCOPE).unwrap(),
            ParsedSecretScope::BuiltInOpenRouterApp
        ));
        assert_ne!(app, OPENROUTER_API_KEY_SCOPE);
    }

    #[test]
    fn extra_openrouter_scope_rejects_reserved_builtin_aliases() {
        for id in [
            "openai",
            "chatgpt",
            "xai",
            "grok",
            "anthropic",
            "codex",
            "openrouter",
            "admin",
            "local",
        ] {
            let scope = format!("openrouter::{id}::api_key");
            let err = parse_secret_scope(&scope).expect_err(id);
            assert!(
                matches!(
                    err,
                    ScopeParseError::InvalidId(ProviderIdError::Reserved { .. })
                ),
                "{id}: {err:?}"
            );
            assert!(
                !is_allowed_provider_scope(&scope),
                "allowlist must reject reserved OpenRouter id `{id}`"
            );
        }
    }

    #[test]
    fn extra_openrouter_sibling_scopes_are_isolated() {
        let work = ProviderId::new("openrouter-work").unwrap();
        let home = ProviderId::new("openrouter-home").unwrap();
        assert_ne!(
            extra_openrouter_application_key_scope(&work),
            extra_openrouter_application_key_scope(&home)
        );
        assert_ne!(
            extra_openrouter_application_key_scope(&work),
            application_key_scope(&work)
        );
        let dir = tempfile::tempdir().unwrap();
        store_provider_secret(
            dir.path(),
            &extra_openrouter_application_key_scope(&work),
            "work-only-key",
        )
        .unwrap();
        store_provider_secret(
            dir.path(),
            &extra_openrouter_application_key_scope(&home),
            "home-only-key",
        )
        .unwrap();
        store_provider_secret(dir.path(), OPENROUTER_API_KEY_SCOPE, "builtin-or-key").unwrap();
        store_provider_secret(
            dir.path(),
            &application_key_scope(&work),
            "openai-compatible-decoy",
        )
        .unwrap();

        assert_eq!(
            read_provider_secret(dir.path(), &extra_openrouter_application_key_scope(&work))
                .unwrap()
                .as_deref(),
            Some("work-only-key")
        );
        assert_eq!(
            read_provider_secret(dir.path(), &extra_openrouter_application_key_scope(&home))
                .unwrap()
                .as_deref(),
            Some("home-only-key")
        );
        assert_eq!(
            read_provider_secret(dir.path(), OPENROUTER_API_KEY_SCOPE)
                .unwrap()
                .as_deref(),
            Some("builtin-or-key")
        );
        // Extra OpenRouter never aliases the openai_compatible scheme.
        assert_eq!(
            read_provider_secret(dir.path(), &application_key_scope(&work))
                .unwrap()
                .as_deref(),
            Some("openai-compatible-decoy")
        );
        clear_provider_secret(dir.path(), &extra_openrouter_application_key_scope(&work)).unwrap();
        assert_eq!(
            read_provider_secret(dir.path(), &extra_openrouter_application_key_scope(&work))
                .unwrap(),
            None
        );
        assert_eq!(
            read_provider_secret(dir.path(), &extra_openrouter_application_key_scope(&home))
                .unwrap()
                .as_deref(),
            Some("home-only-key")
        );
        assert_eq!(
            read_provider_secret(dir.path(), OPENROUTER_API_KEY_SCOPE)
                .unwrap()
                .as_deref(),
            Some("builtin-or-key")
        );
    }

    #[test]
    fn rejects_arbitrary_scopes() {
        assert!(parse_secret_scope("evil::api_key").is_err());
        assert!(parse_secret_scope("openai_compatible::BAD::api_key").is_err());
        assert!(parse_secret_scope("openai_compatible::ok::other").is_err());
        assert!(parse_secret_scope("openrouter::BAD::api_key").is_err());
        assert!(parse_secret_scope("openrouter::openrouter-work::other").is_err());
        assert!(parse_secret_scope("openrouter::openrouter-work").is_err());
    }

    #[test]
    fn preserves_builtin_scopes() {
        assert!(matches!(
            parse_secret_scope(OPENAI_API_KEY_SCOPE).unwrap(),
            ParsedSecretScope::BuiltInOpenAiApp
        ));
        assert!(matches!(
            parse_secret_scope(OPENROUTER_API_KEY_SCOPE).unwrap(),
            ParsedSecretScope::BuiltInOpenRouterApp
        ));
        assert!(matches!(
            parse_secret_scope(OPENROUTER_ADMIN_KEY_SCOPE).unwrap(),
            ParsedSecretScope::BuiltInOpenRouterAdmin
        ));
        assert!(matches!(
            parse_secret_scope(OPENROUTER_MANAGEMENT_KEY_SCOPE).unwrap(),
            ParsedSecretScope::BuiltInOpenRouterManagement
        ));
        assert!(matches!(
            parse_secret_scope(ANTHROPIC_API_KEY_SCOPE).unwrap(),
            ParsedSecretScope::BuiltInAnthropicApp
        ));
    }

    #[test]
    fn openrouter_admin_scope_never_aliases_application() {
        assert_ne!(OPENROUTER_ADMIN_KEY_SCOPE, OPENROUTER_API_KEY_SCOPE);
        assert_ne!(OPENROUTER_MANAGEMENT_KEY_SCOPE, OPENROUTER_API_KEY_SCOPE);
        assert!(!matches!(
            parse_secret_scope(OPENROUTER_ADMIN_KEY_SCOPE).unwrap(),
            ParsedSecretScope::BuiltInOpenRouterApp
        ));
    }

    #[test]
    fn parses_configured_oauth_scope_distinct_from_builtin() {
        let scope = oauth_scope_string(&ProviderId::new("corp-gateway").unwrap());
        assert_eq!(scope, "provider::corp-gateway::oauth");
        assert_eq!(
            parse_configured_oauth_scope(&scope).unwrap().as_str(),
            "corp-gateway"
        );
        // Private meta sidecar is never a token scope.
        assert!(
            parse_configured_oauth_scope(&oauth_meta_scope_string(
                &ProviderId::new("corp-gateway").unwrap()
            ))
            .is_none()
        );
        // Built-in openai::oauth is never a configured route.
        assert!(parse_configured_oauth_scope(crate::auth::OPENAI_OAUTH_SCOPE).is_none());
        // Public API-key parser still rejects OAuth scopes.
        assert!(parse_secret_scope(&scope).is_err());
        assert!(parse_secret_scope(crate::auth::OPENAI_OAUTH_SCOPE).is_err());
    }

    #[test]
    fn allowlists_never_cross_routes() {
        assert!(!is_allowed_provider_scope(crate::auth::OPENAI_OAUTH_SCOPE));
        assert!(!is_allowed_provider_scope("provider::ok::oauth"));
        assert!(is_allowed_provider_scope(OPENAI_API_KEY_SCOPE));
        assert!(is_allowed_provider_scope("openai_compatible::ok::api_key"));
        assert!(is_allowed_provider_scope(
            "openrouter::openrouter-work::api_key"
        ));
        assert!(!is_allowed_provider_scope("openrouter::openai::api_key"));
        assert!(!is_allowed_provider_scope("openrouter::chatgpt::api_key"));
        assert!(is_allowed_oauth_scope("provider::corp-gateway::oauth"));
        assert!(!is_allowed_oauth_scope(crate::auth::OPENAI_OAUTH_SCOPE));
        assert!(!is_allowed_oauth_scope("openai::api_key"));
        assert!(!is_allowed_oauth_scope(
            "provider::corp-gateway::oauth::meta"
        ));
        assert!(!is_allowed_oauth_scope("not-a-scope"));
    }

    #[test]
    fn malformed_oauth_scopes_fail_closed() {
        assert!(parse_configured_oauth_scope("provider::Bad::oauth").is_none());
        assert!(parse_configured_oauth_scope("provider::ok::other").is_none());
        assert!(parse_configured_oauth_scope("provider::ok").is_none());
        assert!(parse_configured_oauth_scope("provider::").is_none());
        assert!(parse_configured_oauth_scope("provider::ok::oauth::extra").is_none());
        assert!(parse_configured_oauth_scope("provider::ok::oauth::api_key").is_none());
        assert!(parse_configured_oauth_scope("provider::ok::oauth::meta").is_none());
    }

    #[test]
    fn oauth_binding_is_secret_free_and_round_trips() {
        let incarnation = ProviderIncarnation::new("123e4567-e89b-12d3-a456-426614174000").unwrap();
        let binding = ProviderOAuthBinding::new(ProviderId::new("corp-gateway").unwrap())
            .with_incarnation(Some(incarnation))
            .with_generation(7);
        assert_eq!(binding.scope_string(), "provider::corp-gateway::oauth");
        let json = serde_json::to_string(&binding).unwrap();
        assert!(!json.contains("sk-"), "no secret-looking value: {json}");
        let back: ProviderOAuthBinding = serde_json::from_str(&json).unwrap();
        assert_eq!(back, binding);
        assert!(
            serde_json::from_value::<ProviderOAuthBinding>(serde_json::json!({
                "provider_id": "corp-gateway",
                "incarnation": "etc/passwd",
                "generation": 0,
            }))
            .is_err()
        );
    }

    #[test]
    fn milvus_token_scope_is_accepted_and_routed() {
        let scope = "milvus::local-milvus::token";
        assert!(
            is_allowed_provider_scope(scope),
            "milvus token must be allowed"
        );
        match parse_secret_scope(scope).unwrap() {
            ParsedSecretScope::Configured(parsed) => {
                assert_eq!(parsed.namespace, ProviderSecretNamespace::Milvus);
                assert_eq!(parsed.kind, ProviderCredentialKind::Token);
                assert_eq!(parsed.provider_id.as_str(), "local-milvus");
                assert_eq!(parsed.as_scope_string(), scope);
            }
            other => panic!("unexpected parse: {other:?}"),
        }
        // read/store/clear secret paths accept the scope (they only validate
        // the allowlist before touching auth.json).
        let home = tempfile::tempdir().unwrap();
        assert_eq!(
            read_provider_secret(home.path(), scope).unwrap(),
            None,
            "unset vault entry reads as None"
        );
    }

    #[test]
    fn milvus_scope_rejects_sibling_kinds_and_namespaces_reject_token() {
        // The milvus namespace only ever carries tokens.
        assert!(!is_allowed_provider_scope("milvus::local-milvus::api_key"));
        assert!(!is_allowed_provider_scope(
            "milvus::local-milvus::admin_key"
        ));
        // Provider namespaces never accept token kind: a bearer token cannot
        // masquerade as an API-key scope.
        assert!(!is_allowed_provider_scope(
            "openai_compatible::local_vllm::token"
        ));
        assert!(!is_allowed_provider_scope("openrouter::extra-acct::token"));
        // Malformed shapes fail closed.
        assert!(!is_allowed_provider_scope("milvus::local-milvus"));
        assert!(!is_allowed_provider_scope("milvus::"));
        assert!(!is_allowed_provider_scope("milvus::a::b::token"));
    }
}
