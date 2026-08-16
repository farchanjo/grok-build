//! Per-provider application, administration, and OAuth secret scopes.

use super::id::{ProviderId, ProviderIdError, validate_provider_id_str};
use super::instance::ProviderIncarnation;
use crate::auth::{
    ANTHROPIC_API_KEY_SCOPE, OPENAI_API_KEY_SCOPE, OPENROUTER_API_KEY_SCOPE,
    clear_provider_api_key, read_provider_api_key, store_provider_api_key,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Which credential a scope refers to.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCredentialKind {
    Application,
    Admin,
}

impl ProviderCredentialKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Application => "api_key",
            Self::Admin => "admin_key",
        }
    }
}

/// Canonical secret scope for a configured OpenAI-compatible provider.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ProviderSecretScope {
    pub provider_id: ProviderId,
    pub kind: ProviderCredentialKind,
}

impl ProviderSecretScope {
    pub fn new(provider_id: ProviderId, kind: ProviderCredentialKind) -> Self {
        Self { provider_id, kind }
    }

    /// `openai_compatible::<id>::api_key` or `...::admin_key`.
    pub fn as_scope_string(&self) -> String {
        format!(
            "openai_compatible::{}::{}",
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
/// Public shape matches PR1: five variants only. Configured OAuth is parsed
/// separately via [`parse_configured_oauth_scope`] and is never an API-key scope.
pub fn parse_secret_scope(scope: &str) -> Result<ParsedSecretScope, ScopeParseError> {
    if scope == OPENAI_API_KEY_SCOPE {
        return Ok(ParsedSecretScope::BuiltInOpenAiApp);
    }
    if scope == OPENROUTER_API_KEY_SCOPE {
        return Ok(ParsedSecretScope::BuiltInOpenRouterApp);
    }
    if scope == ANTHROPIC_API_KEY_SCOPE {
        return Ok(ParsedSecretScope::BuiltInAnthropicApp);
    }
    if scope == "openai::admin_key" {
        return Ok(ParsedSecretScope::BuiltInOpenAiAdmin);
    }
    let prefix = "openai_compatible::";
    let rest = scope
        .strip_prefix(prefix)
        .ok_or(ScopeParseError::UnknownScheme)?;
    let (id_part, kind_part) = rest.rsplit_once("::").ok_or(ScopeParseError::Malformed)?;
    validate_provider_id_str(id_part).map_err(ScopeParseError::InvalidId)?;
    let kind = match kind_part {
        "api_key" => ProviderCredentialKind::Application,
        "admin_key" => ProviderCredentialKind::Admin,
        _ => return Err(ScopeParseError::UnknownKind),
    };
    let provider_id = ProviderId::new(id_part).map_err(ScopeParseError::InvalidId)?;
    Ok(ParsedSecretScope::Configured(ProviderSecretScope {
        provider_id,
        kind,
    }))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedSecretScope {
    BuiltInOpenAiApp,
    BuiltInOpenAiAdmin,
    BuiltInOpenRouterApp,
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
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn rejects_arbitrary_scopes() {
        assert!(parse_secret_scope("evil::api_key").is_err());
        assert!(parse_secret_scope("openai_compatible::BAD::api_key").is_err());
        assert!(parse_secret_scope("openai_compatible::ok::other").is_err());
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
            parse_secret_scope(ANTHROPIC_API_KEY_SCOPE).unwrap(),
            ParsedSecretScope::BuiltInAnthropicApp
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
}
