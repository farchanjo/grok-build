//! Per-provider application and administration secret scopes.

use super::id::{ProviderId, ProviderIdError, validate_provider_id_str};
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
pub fn is_allowed_provider_scope(scope: &str) -> bool {
    parse_secret_scope(scope).is_ok()
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
    store_provider_api_key(grok_home, scope, secret)
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
}
