//! Validated provider identifiers and references.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Maximum length of a configured provider slug.
pub const MAX_PROVIDER_ID_LEN: usize = 64;

/// Built-in provider identities with first-class product flows.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuiltInProviderId {
    Xai,
    OpenAi,
    OpenRouter,
    Anthropic,
}

impl BuiltInProviderId {
    pub const ALL: [Self; 4] = [Self::Xai, Self::OpenAi, Self::OpenRouter, Self::Anthropic];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Xai => "xai",
            Self::OpenAi => "openai",
            Self::OpenRouter => "openrouter",
            Self::Anthropic => "anthropic",
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Xai => "xAI",
            Self::OpenAi => "OpenAI",
            Self::OpenRouter => "OpenRouter",
            Self::Anthropic => "Anthropic",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "xai" | "grok" => Some(Self::Xai),
            "openai" | "chatgpt" | "codex" => Some(Self::OpenAi),
            "openrouter" => Some(Self::OpenRouter),
            // Reserve only the product id `anthropic`; generic "claude" is not reserved.
            "anthropic" => Some(Self::Anthropic),
            _ => None,
        }
    }
}

impl fmt::Display for BuiltInProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Validated stable provider slug used in config, caches, credentials, and CLI.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProviderId(String);

impl ProviderId {
    /// Parse and validate a provider ID (ASCII slug).
    pub fn new(raw: impl AsRef<str>) -> Result<Self, ProviderIdError> {
        let s = raw.as_ref().trim();
        validate_provider_id_str(s)?;
        Ok(Self(s.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }

    /// Built-in IDs are also valid configured-style slugs.
    pub fn from_built_in(id: BuiltInProviderId) -> Self {
        Self(id.as_str().to_owned())
    }
}

impl fmt::Display for ProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ProviderId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl FromStr for ProviderId {
    type Err = ProviderIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

/// Owned provider reference: built-in product provider or user-configured instance.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum ProviderRef {
    BuiltIn(BuiltInProviderId),
    Configured(ProviderId),
}

impl ProviderRef {
    pub fn parse(raw: &str) -> Result<Self, ProviderIdError> {
        let trimmed = raw.trim();
        if let Some(b) = BuiltInProviderId::parse(trimmed) {
            return Ok(Self::BuiltIn(b));
        }
        Ok(Self::Configured(ProviderId::new(trimmed)?))
    }

    pub fn id_str(&self) -> &str {
        match self {
            Self::BuiltIn(b) => b.as_str(),
            Self::Configured(id) => id.as_str(),
        }
    }

    pub fn display_name(&self) -> String {
        match self {
            Self::BuiltIn(b) => b.display_name().to_owned(),
            Self::Configured(id) => id.as_str().to_owned(),
        }
    }

    pub fn is_built_in(&self) -> bool {
        matches!(self, Self::BuiltIn(_))
    }

    pub fn as_provider_id(&self) -> ProviderId {
        match self {
            Self::BuiltIn(b) => ProviderId::from_built_in(*b),
            Self::Configured(id) => id.clone(),
        }
    }
}

impl fmt::Display for ProviderRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderIdError {
    Empty,
    TooLong { len: usize },
    InvalidChar { ch: char },
    MustStartWithLetter,
    Reserved { id: String },
}

impl fmt::Display for ProviderIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "provider id is empty"),
            Self::TooLong { len } => write!(
                f,
                "provider id length {len} exceeds maximum {MAX_PROVIDER_ID_LEN}"
            ),
            Self::InvalidChar { ch } => write!(
                f,
                "provider id contains invalid character `{ch}` (use [a-z0-9_-])"
            ),
            Self::MustStartWithLetter => {
                write!(f, "provider id must start with a lowercase letter")
            }
            Self::Reserved { id } => write!(
                f,
                "provider id `{id}` is reserved; use a different configured id"
            ),
        }
    }
}

impl std::error::Error for ProviderIdError {}

/// Validate a provider ID string without allocating.
pub fn validate_provider_id_str(s: &str) -> Result<(), ProviderIdError> {
    if s.is_empty() {
        return Err(ProviderIdError::Empty);
    }
    if s.len() > MAX_PROVIDER_ID_LEN {
        return Err(ProviderIdError::TooLong { len: s.len() });
    }
    let mut chars = s.chars();
    let first = chars.next().expect("non-empty");
    if !first.is_ascii_lowercase() {
        return Err(ProviderIdError::MustStartWithLetter);
    }
    for ch in std::iter::once(first).chain(chars) {
        let ok = ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-';
        if !ok {
            return Err(ProviderIdError::InvalidChar { ch });
        }
    }
    Ok(())
}

/// Reserved IDs that must not be used for user-configured providers when
/// creating new entries (built-ins already own these names).
pub fn is_reserved_configured_id(id: &str) -> bool {
    matches!(
        id,
        "xai"
            | "grok"
            | "openai"
            | "chatgpt"
            | "codex"
            | "openrouter"
            | "anthropic"
            | "admin"
            | "local"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_slugs() {
        assert!(ProviderId::new("local_vllm").is_ok());
        assert!(ProviderId::new("zai-model-api").is_ok());
        assert!(ProviderId::new("a").is_ok());
    }

    #[test]
    fn rejects_invalid_slugs() {
        assert!(matches!(ProviderId::new(""), Err(ProviderIdError::Empty)));
        assert!(matches!(
            ProviderId::new("1abc"),
            Err(ProviderIdError::MustStartWithLetter)
        ));
        assert!(matches!(
            ProviderId::new("Has Caps"),
            Err(ProviderIdError::MustStartWithLetter)
        ));
        assert!(ProviderId::new("a".repeat(MAX_PROVIDER_ID_LEN + 1)).is_err());
    }

    #[test]
    fn provider_ref_parses_builtins_and_configured() {
        assert_eq!(
            ProviderRef::parse("openrouter").unwrap(),
            ProviderRef::BuiltIn(BuiltInProviderId::OpenRouter)
        );
        assert_eq!(
            ProviderRef::parse("anthropic").unwrap(),
            ProviderRef::BuiltIn(BuiltInProviderId::Anthropic)
        );
        // Generic "claude" is not reserved as a built-in product id.
        assert!(BuiltInProviderId::parse("claude").is_none());
        assert!(!is_reserved_configured_id("claude"));
        assert!(is_reserved_configured_id("anthropic"));
        assert_eq!(
            ProviderRef::parse("local_vllm").unwrap(),
            ProviderRef::Configured(ProviderId::new("local_vllm").unwrap())
        );
        assert_eq!(
            BuiltInProviderId::ALL.as_slice(),
            &[
                BuiltInProviderId::Xai,
                BuiltInProviderId::OpenAi,
                BuiltInProviderId::OpenRouter,
                BuiltInProviderId::Anthropic,
            ]
        );
    }
}
